"""Entity-Transformer Actor-Critic (`EntityTransformer`): one token per card slot, attention across
them, and a policy that scores each action from the tokens it involves.

The observation (`features.encode_observation`) is a set of card slots in zones: the board, the
hand, the shop, the Discover options, the opponents' boards, ... (`features.py`'s layout
constants). The MLP flattens them, so it must learn the same thing separately for every slot. Here
each slot is a token:

    token = Linear([card embedding, static card features (`card_features.py`), slot numeric features])
            + zone embedding + position-in-zone embedding

The position embedding exists only for the zones whose order matters in combat (the player's own
board, the teammate's board and the opponents' boards); every other slot shares one "unordered"
position, so the encoder treats e.g. the hand as a set. A player token (`Linear(player_vec)` + its
own zone embedding) carries the rest of the state. Empty slots are left out of attention.

A pre-norm `nn.TransformerEncoder` mixes the tokens. The value comes from the player token's
output. Each of the `ACTION_SPACE_SIZE` actions is scored from the tokens it is about (a Buy from
its shop slot, a Play from its hand slot and board position, a Spell on a shop minion from both
cards, End Turn from the player token, ...; `_action_table`):

    logit = w · relu(type_emb[kind] + W_src h[src] + W_tgt h[tgt] + pos_emb[position]
                     + target_emb[target index] + W_ctx h[player] + b) + c

so the same weights score "play this card" for every hand slot, and the logits move with the cards
(permuting the hand permutes the Play logits). Then the kind prior and the legality mask, as in
`BattlegroundsMLP`.

Three things keep it cheap enough for CPU rollouts and updates:
- Each row's non-empty slots are packed to the front (`_pack`), and the rows are run through the
  encoder in buckets of similar length (`_encode`), each cut to its longest row. A typical
  observation has ~25 non-empty slots of 101. An empty slot is never attended to, so this changes
  nothing but the outputs of the empty slots' own tokens, which only score actions about empty
  slots, which are illegal. Such an action is scored from the player token instead.
- With an action mask, only the legal actions are scored (~17 of 330 on average); the others get
  -1e9 as in the MLP, so the probabilities and gradients are the same.
- The card part of a slot token (embedding and static features through the input layer) depends
  only on the card: it is computed once for the whole vocabulary and looked up (`card_tokens`).
"""

from __future__ import annotations

import numpy as np
import torch
import torch.nn as nn

from .actions import (
    ACTION_KIND_IDS,
    ACTION_SPACE_SIZE,
    ACTIONS,
    MAX_BOARD,
    MAX_DISCOVER,
    MAX_HAND,
    MAX_HERO_OPTIONS,
    MAX_SHOP,
    MAX_TRINKET_OPTIONS,
    NUM_ACTION_KINDS,
)
from .card_features import static_card_table
from .model import sample_categorical
from .features import (
    CARD_COUNTER_OFFSET,
    EFFECT_OFFSET,
    HERO_OFFSET,
    MAX_CARD_COUNTERS,
    MAX_EFFECTS,
    MAX_OPPONENTS_SLOTS,
    MAX_TRINKETS,
    MAX_WAITING,
    NUM_SLOTS,
    OPPONENT_BOARD_OFFSET,
    OPPONENT_HERO_OFFSET,
    PLAYER_DIM,
    SLOT_NUM_DIM,
    TAVERN_SLOTS,
    TEAMMATE_OFFSET,
    TEAMMATE_SHOP_OFFSET,
    TEAMMATE_SHOP_SLOTS,
    VOCAB_SIZE,
)

# The player's own Tavern zones, in slot order (`features.TAVERN_SLOTS`).
BOARD_OFFSET: int = 0
HAND_OFFSET: int = BOARD_OFFSET + MAX_BOARD
SHOP_OFFSET: int = HAND_OFFSET + MAX_HAND
DISCOVER_OFFSET: int = SHOP_OFFSET + MAX_SHOP
WAITING_OFFSET: int = DISCOVER_OFFSET + MAX_DISCOVER
TRINKET_OFFSET: int = WAITING_OFFSET + MAX_WAITING
TRINKET_OPTION_OFFSET: int = TRINKET_OFFSET + MAX_TRINKETS
assert TRINKET_OPTION_OFFSET + MAX_TRINKET_OPTIONS == TAVERN_SLOTS, "features.py's Tavern layout changed"
HERO_OPTION_OFFSET: int = HERO_OFFSET + 1

# Every zone: `(name, first slot, slots, ordered)`. An ordered zone gives its slots positions
# 1..slots; the others share position 0. Slots in none of them (if `features.py` gains some before
# this is updated) fall in the "other" zone.
ZONES: list[tuple[str, int, int, bool]] = [
    ("board", BOARD_OFFSET, MAX_BOARD, True),
    ("hand", HAND_OFFSET, MAX_HAND, False),
    ("shop", SHOP_OFFSET, MAX_SHOP, False),
    ("discover", DISCOVER_OFFSET, MAX_DISCOVER, False),
    ("waiting", WAITING_OFFSET, MAX_WAITING, False),
    ("trinkets", TRINKET_OFFSET, MAX_TRINKETS, False),
    ("trinket_options", TRINKET_OPTION_OFFSET, MAX_TRINKET_OPTIONS, False),
    ("hero", HERO_OFFSET, 1, False),
    ("hero_options", HERO_OPTION_OFFSET, MAX_HERO_OPTIONS, False),
    ("teammate_board", TEAMMATE_OFFSET, MAX_BOARD, True),
    ("teammate_hand", TEAMMATE_OFFSET + MAX_BOARD, MAX_HAND, False),
    ("opponent_board_1", OPPONENT_BOARD_OFFSET, MAX_BOARD, True),
    ("opponent_board_2", OPPONENT_BOARD_OFFSET + MAX_BOARD, MAX_BOARD, True),
    ("teammate_shop", TEAMMATE_SHOP_OFFSET, TEAMMATE_SHOP_SLOTS, False),
    ("opponent_heroes", OPPONENT_HERO_OFFSET, MAX_OPPONENTS_SLOTS, False),
    ("card_counters", CARD_COUNTER_OFFSET, MAX_CARD_COUNTERS, False),
    ("effects", EFFECT_OFFSET, MAX_EFFECTS, False),
]
ZONE_NAMES: list[str] = ["player", "other"] + [z[0] for z in ZONES]
MAX_ZONE_POSITIONS: int = max(n for _, _, n, ordered in ZONES if ordered)


def _slot_layout() -> tuple[np.ndarray, np.ndarray]:
    """Each slot's zone (index in `ZONE_NAMES`) and position in its zone (0: unordered)."""
    zone = np.full(NUM_SLOTS, ZONE_NAMES.index("other"), dtype=np.int64)
    pos = np.zeros(NUM_SLOTS, dtype=np.int64)
    for name, start, n, ordered in ZONES:
        assert start + n <= NUM_SLOTS, f"zone {name} runs past the slots"
        zone[start : start + n] = ZONE_NAMES.index(name)
        if ordered:
            pos[start : start + n] = np.arange(1, n + 1)
    return zone, pos


# A slot index meaning "the player token" (the slots are `0..NUM_SLOTS`).
PLAYER_TOKEN: int = NUM_SLOTS
# The "target index" an action can carry (Activate's and the Hero Powers' `t`): 0 for none, else
# `1 + t`.
NUM_TARGET_INDICES: int = 2 + max(
    [a.args[1] for a in ACTIONS if a.kind == "activate" and a.args[1] is not None]
    + [a.args[0] for a in ACTIONS if a.kind in ("hero_power", "hero_power_2") and a.args[0] is not None]
)


def _action_table() -> dict[str, np.ndarray]:
    """For each action (`actions.ACTIONS`): the slot it is about (`src`, or `PLAYER_TOKEN`), the
    slot it targets (`tgt`, -1 for none), its board position (`pos`: 0 for none, else 1 + the
    position) and its target index (`target`: 0 for none, else 1 + the index)."""
    n = len(ACTIONS)
    src = np.full(n, PLAYER_TOKEN, dtype=np.int64)
    tgt = np.full(n, -1, dtype=np.int64)
    pos = np.zeros(n, dtype=np.int64)
    target = np.zeros(n, dtype=np.int64)
    for i, a in enumerate(ACTIONS):
        x, y = a.args
        kind = a.kind
        if kind == "buy":
            src[i] = SHOP_OFFSET + x
        elif kind == "play":
            src[i], pos[i] = HAND_OFFSET + x, 1 + y
        elif kind == "play_on_shop":
            src[i], tgt[i] = HAND_OFFSET + x, SHOP_OFFSET + y
        elif kind == "sell":
            src[i] = BOARD_OFFSET + x
        elif kind == "reposition":
            src[i], pos[i] = BOARD_OFFSET + x, 1 + y
        elif kind == "activate":
            # The target index means a board, shop or hand slot depending on the card: learned.
            src[i], target[i] = BOARD_OFFSET + x, 0 if y is None else 1 + y
        elif kind == "choose_discover":
            src[i] = DISCOVER_OFFSET + x
        elif kind == "pass":
            src[i] = HAND_OFFSET + x
        elif kind in ("hero_power", "hero_power_2"):
            src[i], target[i] = HERO_OFFSET, 0 if x is None else 1 + x
        elif kind == "choose_hero":
            src[i] = HERO_OPTION_OFFSET + x
        elif kind == "choose_trinket":
            src[i] = TRINKET_OPTION_OFFSET + x
        # Everything else (End Turn, Refresh, Upgrade, Freeze, Dark Discovery, and any kind added
        # since) is about the player as a whole: the player token.
    return {"src": src, "tgt": tgt, "pos": pos, "target": target}


class EntityTransformer(nn.Module):
    """A Transformer over the card slots and the player, with per-action scoring (module
    docstring). Same interface as `BattlegroundsMLP`."""

    def __init__(
        self,
        vocab_size: int = VOCAB_SIZE,
        card_embed_dim: int = 16,
        d_model: int = 64,
        layers: int = 3,
        heads: int = 4,
        ff: int = 128,
        head_dim: int = 64,
        num_actions: int = ACTION_SPACE_SIZE,
        seed: int = 42,
        kind_prior: bool = False,
        static_features: bool = True,
        arch: str = "transformer",
    ) -> None:
        super().__init__()
        assert arch == "transformer", arch
        assert num_actions == ACTION_SPACE_SIZE, "the action table covers every action"
        # The constructor's arguments, saved with checkpoints (`checkpoint.py`). `arch` tells
        # `checkpoint.model_from_config` which class a config builds.
        self.config = {
            "vocab_size": vocab_size,
            "card_embed_dim": card_embed_dim,
            "d_model": d_model,
            "layers": layers,
            "heads": heads,
            "ff": ff,
            "head_dim": head_dim,
            "num_actions": num_actions,
            "kind_prior": kind_prior,
            "static_features": static_features,
            "arch": arch,
        }
        # The kind prior, as in `BattlegroundsMLP`.
        self.kind_prior = kind_prior
        kind_onehot = np.zeros((num_actions, NUM_ACTION_KINDS), dtype=np.float32)
        kind_onehot[np.arange(num_actions), ACTION_KIND_IDS[:num_actions]] = 1.0
        self.register_buffer("kind_onehot", torch.from_numpy(kind_onehot), persistent=False)

        # What each card is (`card_features.py`): looked up by card id, recomputed on construction
        # rather than saved with the weights. Without `static_features`, an empty table.
        static = static_card_table()[:vocab_size]
        assert static.shape[0] == vocab_size, (static.shape, vocab_size)
        if not static_features:
            static = static[:, :0]
        self.register_buffer("static_table", torch.from_numpy(np.ascontiguousarray(static)), persistent=False)
        zone, pos = _slot_layout()
        self.register_buffer("slot_zone", torch.from_numpy(zone), persistent=False)
        self.register_buffer("slot_pos", torch.from_numpy(pos), persistent=False)

        # Tokens. The slot token's input layer, `Linear([card embedding, static features, numeric
        # features])`, is split in two: the card part depends only on the card, so it is computed
        # once per card for the whole vocabulary (`card_tokens`) and looked up.
        self.card_embed = nn.Embedding(vocab_size, card_embed_dim, padding_idx=0)
        self.card_in = nn.Linear(card_embed_dim + static.shape[1], d_model, bias=False)
        self.num_in = nn.Linear(SLOT_NUM_DIM, d_model)
        self._card_tokens_cache: tuple[tuple, torch.Tensor] | None = None
        self.player_in = nn.Linear(PLAYER_DIM, d_model)
        self.zone_embed = nn.Embedding(len(ZONE_NAMES), d_model)
        self.pos_embed = nn.Embedding(MAX_ZONE_POSITIONS + 1, d_model)
        self.player_zone = ZONE_NAMES.index("player")

        # The encoder: pre-norm, no dropout, with a final norm (pre-norm layers leave the residual
        # stream unnormalized).
        layer = nn.TransformerEncoderLayer(
            d_model, heads, dim_feedforward=ff, dropout=0.0, batch_first=True, norm_first=True
        )
        self.encoder = nn.TransformerEncoder(
            layer, layers, norm=nn.LayerNorm(d_model), enable_nested_tensor=False
        )

        # The value, from the player token.
        self.critic = nn.Sequential(nn.Linear(d_model, d_model), nn.ReLU(), nn.Linear(d_model, 1))

        # The policy: per-action scoring from the tokens each action is about.
        table = _action_table()
        self.register_buffer("act_kind", torch.from_numpy(ACTION_KIND_IDS[:num_actions].copy()), persistent=False)
        self.register_buffer("act_src", torch.from_numpy(table["src"]), persistent=False)
        self.register_buffer("act_pos", torch.from_numpy(table["pos"]), persistent=False)
        self.register_buffer("act_target", torch.from_numpy(table["target"]), persistent=False)
        self.register_buffer("act_tgt", torch.from_numpy(table["tgt"]), persistent=False)
        self.type_embed = nn.Embedding(NUM_ACTION_KINDS, head_dim)
        self.act_pos_embed = nn.Embedding(MAX_BOARD + 1, head_dim)
        self.target_embed = nn.Embedding(NUM_TARGET_INDICES, head_dim)
        self.w_src = nn.Linear(d_model, head_dim, bias=False)
        self.w_tgt = nn.Linear(d_model, head_dim, bias=False)
        self.w_ctx = nn.Linear(d_model, head_dim)
        self.actor_out = nn.Linear(head_dim, 1)
        # Start near the uniform policy (over kinds, with the kind prior).
        nn.init.normal_(self.actor_out.weight, std=0.01)
        nn.init.zeros_(self.actor_out.bias)
        self._np_rng = np.random.default_rng(seed)

    def _pack(
        self, slot_card_ids: torch.Tensor, slot_num_feats: torch.Tensor
    ) -> tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
        """Each row's non-empty slots, in slot order, then its empty ones.

        Returns `(slots (B, NUM_SLOTS), lengths (B,), token_of_slot (B, NUM_SLOTS + 1))`: the slot
        in each packed position, the number of non-empty slots, and the index of each slot's token
        in its row's encoder sequence (`1 + packed position`; 0, the player token, for an empty
        slot and for `PLAYER_TOKEN`).
        """
        present = (slot_card_ids != 0) | (slot_num_feats[..., 0] != 0)
        B, S = present.shape
        slots = torch.argsort((~present).to(torch.uint8), dim=1, stable=True)
        token = torch.arange(1, S + 1, device=slots.device).expand(B, S) * present.gather(1, slots)
        token_of_slot = torch.zeros(B, S + 1, dtype=torch.int64, device=slots.device)
        token_of_slot.scatter_(1, slots, token)
        return slots, present.sum(dim=1), token_of_slot

    def _encode(
        self,
        player_vec: torch.Tensor,
        slot_card_ids: torch.Tensor,
        slot_num_feats: torch.Tensor,
        slots: torch.Tensor,
        lengths: torch.Tensor,
    ) -> tuple[torch.Tensor, torch.Tensor]:
        """Build the tokens and run the encoder, in buckets of rows of similar length.

        The rows' lengths vary a lot (a few cards early on, dozens late in the game), so padding
        every row to the longest wastes most of the compute. Sorted by length, each bucket of
        rows is cut to its own longest row; padding never changes a row's outputs.

        Returns `(flat (N, d), start (B,))`: every bucket's outputs flattened, and where each row's
        sequence starts in them (its player token; its packed position `i` is at `start + 1 + i`).
        """
        B = player_vec.shape[0]
        order = torch.argsort(lengths)
        sorted_lengths = lengths[order].tolist()
        # About `BUCKETS` buckets, of at least `MIN_BUCKET_ROWS` rows: each one costs a call.
        size = max(self.MIN_BUCKET_ROWS, -(-B // self.BUCKETS))
        card_tokens = self.card_tokens()
        player = self.player_in(player_vec) + self.zone_embed.weight[self.player_zone]
        outs: list[torch.Tensor] = []
        starts: list[torch.Tensor] = []
        offset = 0
        for i in range(0, B, size):
            rows = order[i : i + size]
            n = max(1, sorted_lengths[min(i + size, B) - 1])
            sl = slots[rows, :n]  # (b, n)
            ids = slot_card_ids[rows.unsqueeze(1), sl]
            num = slot_num_feats[rows.unsqueeze(1), sl].to(self.num_in.weight.dtype)
            x = card_tokens[ids] + self.num_in(num) + self.zone_embed(self.slot_zone[sl]) + self.pos_embed(self.slot_pos[sl])
            tokens = torch.cat([player[rows].unsqueeze(1), x], dim=1)  # (b, 1 + n, d)
            pad = torch.arange(n + 1, device=sl.device) > lengths[rows].unsqueeze(1)
            h = self.encoder(tokens, src_key_padding_mask=pad)
            outs.append(h.reshape(-1, h.shape[-1]))
            starts.append(offset + (n + 1) * torch.arange(len(rows), device=sl.device))
            offset += len(rows) * (n + 1)
        start = torch.empty(B, dtype=torch.int64, device=player_vec.device)
        start[order] = torch.cat(starts)
        return torch.cat(outs), start

    # How `_encode` splits a batch: fewer, bigger buckets pad more but make fewer calls.
    BUCKETS: int = 8
    MIN_BUCKET_ROWS: int = 16

    def forward(
        self,
        player_vec: torch.Tensor,
        slot_card_ids: torch.Tensor,
        slot_num_feats: torch.Tensor,
        action_mask: torch.Tensor | None = None,
    ) -> tuple[torch.Tensor, torch.Tensor]:
        """Forward pass returning `(masked_logits, value)`.

        With an `action_mask`, only the legal actions are scored (a few dozen of the 330); the
        others get the same -1e9 as in `BattlegroundsMLP`.
        """
        B = player_vec.shape[0]
        slots, lengths, token_of_slot = self._pack(slot_card_ids, slot_num_feats)
        h, start = self._encode(player_vec, slot_card_ids, slot_num_feats, slots, lengths)
        h_player = h[start]
        value = self.critic(h_player).squeeze(-1)

        # Score the (row, action) pairs: gather each action's source (and target) token from the
        # flattened encoder outputs.
        A = self.act_src.shape[0]
        if action_mask is None:
            rows = torch.arange(B, device=h.device).repeat_interleave(A)
            acts = torch.arange(A, device=h.device).repeat(B)
        else:
            rows, acts = action_mask.nonzero(as_tuple=True)
        src = start[rows] + token_of_slot[rows, self.act_src[acts]]
        pre = self.w_src(h[src]) + self.w_ctx(h_player)[rows] + self.per_action_embed()[acts]
        tgt = self.act_tgt[acts]
        has_tgt = tgt >= 0
        if bool(has_tgt.any()):
            with_tgt = has_tgt.nonzero(as_tuple=True)[0]
            r = rows[with_tgt]
            tgt_tok = start[r] + token_of_slot[r, tgt[with_tgt]]
            pre = pre.index_add(0, with_tgt, self.w_tgt(h[tgt_tok]))
        scores = self.actor_out(torch.relu(pre)).squeeze(-1)

        if action_mask is None:
            return scores.view(B, A), value
        if self.kind_prior:
            counts = action_mask.float() @ self.kind_onehot  # (B, kinds)
            scores = scores - torch.log(counts[rows, self.act_kind[acts]].clamp(min=1.0))
        logits = torch.full((B, A), -1e9, dtype=scores.dtype, device=scores.device)
        logits = logits.index_put((rows, acts), scores)
        return logits, value

    def card_tokens(self) -> torch.Tensor:
        """Every card's part of its slot token, `(vocab_size, d_model)`: `card_in` of its learned
        embedding and static features.

        Without gradients (rollouts), it is cached until the weights change: a rollout worker
        computes it once per set of weights rather than once per batch.
        """
        weights = (self.card_embed.weight, self.card_in.weight)
        if torch.is_grad_enabled():
            self._card_tokens_cache = None
            return self.card_in(torch.cat([self.card_embed.weight, self.static_table], dim=-1))
        key = tuple((w.data_ptr(), w._version) for w in weights)
        if self._card_tokens_cache is None or self._card_tokens_cache[0] != key:
            table = self.card_in(torch.cat([self.card_embed.weight, self.static_table], dim=-1))
            self._card_tokens_cache = (key, table)
        return self._card_tokens_cache[1]

    def per_action_embed(self) -> torch.Tensor:
        """Each action's own part of its pre-activation: its kind, board position and target
        index embeddings, `(A, head_dim)`."""
        return self.type_embed(self.act_kind) + self.act_pos_embed(self.act_pos) + self.target_embed(self.act_target)

    def act_batch(
        self,
        p_np: np.ndarray,
        c_np: np.ndarray,
        n_np: np.ndarray,
        m_np: np.ndarray,
        deterministic: bool = False,
    ) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
        """Batched forward + sampling for CPU rollouts. Returns `(actions, log_probs, values)`."""
        with torch.no_grad():
            logits, values = self.forward(
                torch.from_numpy(np.ascontiguousarray(p_np, dtype=np.float32)),
                torch.from_numpy(np.ascontiguousarray(c_np, dtype=np.int64)),
                torch.from_numpy(np.ascontiguousarray(n_np, dtype=np.float32)),
                torch.from_numpy(np.ascontiguousarray(m_np, dtype=np.bool_)),
            )
            lps = torch.log_softmax(logits, dim=-1).numpy()
        B = lps.shape[0]
        if deterministic:
            acts = lps.argmax(axis=-1)
        else:
            acts = sample_categorical(np.exp(lps), self._np_rng)
        return acts, lps[np.arange(B), acts], values.numpy()

    def act(
        self,
        obs: tuple,
        mask_np: np.ndarray,
        deterministic: bool = False,
        device: str = "cpu",
    ) -> tuple[int, float, float]:
        """One observation's `(action, log_prob, value)`."""
        player_vec, slot_card_ids, slot_num_feats = obs
        acts, lps, vals = self.act_batch(
            player_vec[np.newaxis, :],
            slot_card_ids[np.newaxis, :],
            slot_num_feats[np.newaxis, :, :],
            mask_np[np.newaxis, :],
            deterministic=deterministic,
        )
        return int(acts[0]), float(lps[0]), float(vals[0])
