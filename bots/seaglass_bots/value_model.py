"""Set-Transformer Synergy & 8-Way Placement Value Model (`SynergyValueNet`)."""

from __future__ import annotations

import math
import os
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Tuple

import numpy as np
import seaglass
import torch
import torch.nn as nn
import torch.nn.functional as F

from .catalog_meta import CardMetaDB, get_meta_db

MAX_TOKENS = 16  # 1 GLOBAL + up to 7 board + up to 5 hand + up to 2 trinkets + 1 hero power
TOKEN_NUM_DIM = 20
GLOBAL_HEADER_DIM = 22

# Expected tournament utility weights for places 1..8 (zero-centered, aligned with ICM scale).
PLACEMENT_UTILITY_WEIGHTS = np.array(
    [30.0, 17.0, 7.5, 1.5, -4.5, -11.0, -18.5, -26.0], dtype=np.float32
)

DEFAULT_CHECKPOINT_PATH = (
    Path(__file__).resolve().parent.parent / "checkpoints" / "synergy_value_v1.pt"
)


def _signed_log(x: float, scale: float = 3.0) -> float:
    if x == 0:
        return 0.0
    v = math.log1p(abs(float(x))) / scale
    return v if x > 0 else -v


class StateFeaturizer:
    """Featurizes `TavernObservation` + `LobbyContext` into compact Set-Transformer tensors."""

    def __init__(self, catalog_name: str = "full") -> None:
        self.meta: CardMetaDB = get_meta_db(catalog_name)
        self.counter_dim: int = len(self.meta.counter_names)
        self.global_dim: int = GLOBAL_HEADER_DIM + self.counter_dim

    def featurize_into(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext],
        end_of_turn: bool,
        out_vocab: np.ndarray,
        out_slot: np.ndarray,
        out_pos: np.ndarray,
        out_num: np.ndarray,
        out_mask: np.ndarray,
        out_global: np.ndarray,
    ) -> None:
        """Populate pre-allocated single-state buffers in-place."""
        out_vocab.fill(0)
        out_slot.fill(0)
        out_pos.fill(0)
        out_num.fill(0.0)
        out_mask.fill(False)
        out_global.fill(0.0)

        # Token 0 is always the active [GLOBAL] summary token.
        out_slot[0] = 1
        out_mask[0] = True
        cursor = 1

        # 1. Board minions (up to 7, preserving exact left-to-right adjacency position 1..7).
        for b_idx, u in enumerate(obs.board[:7]):
            out_vocab[cursor] = self.meta.vocab_index(u.card_id)
            out_slot[cursor] = 2
            out_pos[cursor] = b_idx + 1
            self._fill_unit_num(u, out_num[cursor])
            out_mask[cursor] = True
            cursor += 1

        # 2. Hand cards (up to 5: prioritize Golden cards, playable minions, and high-tier spells).
        hand_cards = list(obs.hand)
        if len(hand_cards) > 5:
            hand_cards.sort(
                key=lambda c: (c.is_golden, not c.is_spell, c.tavern_tier, c.attack + c.health),
                reverse=True,
            )
            hand_cards = hand_cards[:5]
        for c in hand_cards:
            if cursor >= 13:
                break
            out_vocab[cursor] = self.meta.vocab_index(c.card_id)
            out_slot[cursor] = 3
            out_pos[cursor] = 0
            self._fill_unit_num(c, out_num[cursor])
            out_mask[cursor] = True
            cursor += 1

        # 3. Owned Trinkets (up to 2).
        trinket_ids = obs.trinkets
        trinket_ctrs = obs.trinket_counters
        trinket_trs = obs.trinket_tribes
        for t_idx in range(min(2, len(trinket_ids))):
            if cursor >= 15:
                break
            tid = trinket_ids[t_idx]
            out_vocab[cursor] = self.meta.vocab_index(tid)
            out_slot[cursor] = 4
            out_pos[cursor] = 0
            ctr_val = trinket_ctrs[t_idx] if t_idx < len(trinket_ctrs) else 0
            tr_val = trinket_trs[t_idx] if t_idx < len(trinket_trs) else 0
            out_num[cursor, 18] = _signed_log(ctr_val, 3.0)
            out_num[cursor, 19] = float(tr_val) / 10.0
            out_mask[cursor] = True
            cursor += 1

        # 4. Active Hero / Hero Power token.
        hp_id = obs.hero_power_id if obs.hero_power_id != 0 else obs.hero_id
        if hp_id != 0 and cursor < MAX_TOKENS:
            out_vocab[cursor] = self.meta.vocab_index(hp_id)
            out_slot[cursor] = 5
            out_pos[cursor] = 0
            out_num[cursor, 4] = float(obs.hero_power_cost) / 4.0
            out_num[cursor, 5] = float(obs.hero_power_uses_left > 0)
            out_num[cursor, 6] = float(obs.hero_power_armed > 0)
            out_num[cursor, 18] = _signed_log(obs.hero_power_stacks, 3.0)
            out_num[cursor, 19] = _signed_log(obs.hero_power_counter, 3.0)
            out_mask[cursor] = True

        # 5. Global context vector.
        hp_total = float(obs.health + obs.armor)
        alive_units = 8
        median_hp = 30.0
        max_opp_tier = float(obs.tavern_tier)
        if ctx is not None:
            alive_units = ctx.alive_units
            opp_hps = sorted(o.health + o.armor for o in ctx.opponents if o.alive)
            if opp_hps:
                median_hp = float(opp_hps[len(opp_hps) // 2])
                max_opp_tier = float(max(o.tier for o in ctx.opponents if o.alive))

        out_global[0] = min(1.5, obs.turn / 16.0)
        out_global[1] = max(0.0, 15.0 - obs.turn) / 14.0
        out_global[2] = hp_total / 40.0
        out_global[3] = obs.health / 30.0
        out_global[4] = obs.armor / 20.0
        tier_clamped = max(1, min(6, obs.tavern_tier))
        out_global[5 + tier_clamped - 1] = 1.0
        out_global[11] = obs.upgrade_cost / 11.0
        out_global[12] = obs.gold / 12.0
        out_global[13] = obs.max_gold / 12.0
        out_global[14] = obs.bonus_gold_next_turn / 6.0
        out_global[15] = obs.free_refreshes / 4.0
        out_global[16] = 1.0 if obs.is_frozen else 0.0
        out_global[17] = 1.0 if end_of_turn else 0.0
        out_global[18] = alive_units / 8.0
        out_global[19] = (hp_total - median_hp) / 30.0
        out_global[20] = (obs.tavern_tier - max_opp_tier) / 5.0

        cc_sum = 0.0
        for _, v1, v2 in obs.card_counters:
            cc_sum += abs(_signed_log(v1, 3.0)) + abs(_signed_log(v2, 3.0))
        out_global[21] = min(4.0, cc_sum / 3.0)

        p_ctrs = obs.player_counters
        limit = min(self.counter_dim, len(p_ctrs))
        for i in range(limit):
            val = p_ctrs[i]
            if val != 0:
                out_global[GLOBAL_HEADER_DIM + i] = _signed_log(val, 3.0)

    @staticmethod
    def _fill_unit_num(u: seaglass.Unit, dst: np.ndarray) -> None:
        atk = max(0, u.attack)
        hp = max(0, u.health)
        dst[0] = math.log1p(atk) / 5.0
        dst[1] = math.log1p(hp) / 5.0
        dst[2] = min(4.0, atk / 50.0)
        dst[3] = min(4.0, hp / 50.0)
        dst[4] = u.tavern_tier / 6.0
        dst[5] = 1.0 if u.is_golden else 0.0
        dst[6] = 1.0 if u.is_spell else 0.0
        dst[7] = 1.0 if u.taunt else 0.0
        dst[8] = 1.0 if u.divine_shield else 0.0
        dst[9] = 1.0 if u.windfury else 0.0
        dst[10] = 1.0 if u.reborn else 0.0
        dst[11] = 1.0 if u.venomous else 0.0
        dst[12] = 1.0 if u.stealth else 0.0
        dst[13] = 1.0 if u.magnetic else 0.0
        dst[14] = math.log1p(u.blood_gems_played) / 3.0
        dst[15] = math.log1p(max(0, u.end_of_turn_stats)) / 2.5
        dst[16] = min(3.0, float(u.extra_deathrattles)) / 2.0
        dst[17] = min(3.0, float(len(u.dark_gifts))) / 2.0
        dst[18] = _signed_log(u.stacks, 3.0)
        dst[19] = _signed_log(u.counter, 3.0)

    def featurize_batch(
        self,
        obs_list: Sequence[seaglass.TavernObservation],
        ctx: Optional[seaglass.LobbyContext],
        end_of_turn: bool,
    ) -> Tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
        b = len(obs_list)
        vocab = np.zeros((b, MAX_TOKENS), dtype=np.int64)
        slot = np.zeros((b, MAX_TOKENS), dtype=np.int64)
        pos = np.zeros((b, MAX_TOKENS), dtype=np.int64)
        num = np.zeros((b, MAX_TOKENS, TOKEN_NUM_DIM), dtype=np.float32)
        mask = np.zeros((b, MAX_TOKENS), dtype=np.bool_)
        glob = np.zeros((b, self.global_dim), dtype=np.float32)
        for i, obs in enumerate(obs_list):
            self.featurize_into(
                obs, ctx, end_of_turn, vocab[i], slot[i], pos[i], num[i], mask[i], glob[i]
            )
        return vocab, slot, pos, num, mask, glob


class SetAttentionBlock(nn.Module):
    """Fast Pre-Norm Multi-Head Self-Attention block with fused QKV projection."""

    def __init__(self, d_model: int = 64, n_heads: int = 4, ffn_dim: int = 128) -> None:
        super().__init__()
        assert d_model % n_heads == 0
        self.d_model = d_model
        self.n_heads = n_heads
        self.head_dim = d_model // n_heads
        self.scale = 1.0 / math.sqrt(self.head_dim)

        self.norm1 = nn.LayerNorm(d_model)
        self.qkv = nn.Linear(d_model, 3 * d_model)
        self.out_proj = nn.Linear(d_model, d_model)

        self.norm2 = nn.LayerNorm(d_model)
        self.ffn = nn.Sequential(
            nn.Linear(d_model, ffn_dim),
            nn.GELU(),
            nn.Linear(ffn_dim, d_model),
        )

    def forward(self, x: torch.Tensor, mask: torch.Tensor) -> torch.Tensor:
        # x: [B, T, D], mask: [B, T] (True = valid token)
        bsz, seqlen, _ = x.shape
        h = self.norm1(x)
        qkv = self.qkv(h).view(bsz, seqlen, 3, self.n_heads, self.head_dim)
        q, k, v = qkv.unbind(dim=2)
        q = q.transpose(1, 2)  # [B, H, T, Dh]
        k = k.transpose(1, 2)
        v = v.transpose(1, 2)

        scores = torch.matmul(q, k.transpose(-2, -1)) * self.scale  # [B, H, T, T]
        key_invalid = ~mask[:, None, None, :]  # [B, 1, 1, T]
        scores = scores.masked_fill(key_invalid, -1e9)
        attn = torch.softmax(scores, dim=-1)
        ctx = torch.matmul(attn, v).transpose(1, 2).contiguous().view(bsz, seqlen, self.d_model)

        x = x + self.out_proj(ctx)
        x = x + self.ffn(self.norm2(x))
        return x * mask.unsqueeze(-1).to(x.dtype)


class SynergyValueNet(nn.Module):
    """
    Set-Transformer modeling multi-card combos, adjacent board synergies, trinkets, heroes,
    and overarching build scaling to predict 8-way final tournament placement probabilities.
    """

    def __init__(
        self,
        vocab_size: int,
        tag_matrix: np.ndarray,
        global_dim: int,
        d_model: int = 64,
        n_heads: int = 4,
        n_layers: int = 2,
    ) -> None:
        super().__init__()
        tag_dim = int(tag_matrix.shape[1])
        self.register_buffer(
            "tag_table", torch.tensor(tag_matrix, dtype=torch.float32), persistent=True
        )
        self.register_buffer(
            "place_weights",
            torch.tensor(PLACEMENT_UTILITY_WEIGHTS, dtype=torch.float32),
            persistent=False,
        )

        self.card_emb = nn.Embedding(vocab_size, d_model, padding_idx=0)
        self.tag_proj = nn.Linear(tag_dim, d_model, bias=False)
        self.slot_emb = nn.Embedding(6, d_model, padding_idx=0)
        self.pos_emb = nn.Embedding(8, d_model, padding_idx=0)
        self.num_proj = nn.Linear(TOKEN_NUM_DIM, d_model)
        self.global_proj = nn.Sequential(
            nn.Linear(global_dim, d_model),
            nn.GELU(),
            nn.Linear(d_model, d_model),
        )
        self.in_norm = nn.LayerNorm(d_model)

        self.blocks = nn.ModuleList(
            [SetAttentionBlock(d_model=d_model, n_heads=n_heads, ffn_dim=2 * d_model) for _ in range(n_layers)]
        )

        # Readout combines [GLOBAL token, masked mean pool, masked max pool] + skip global features.
        readout_in = 3 * d_model + global_dim
        self.trunk = nn.Sequential(
            nn.Linear(readout_in, 128),
            nn.LayerNorm(128),
            nn.GELU(),
            nn.Linear(128, 64),
            nn.GELU(),
        )
        self.place_head = nn.Linear(64, 8)
        self.power_head = nn.Linear(64, 1)

    def forward(
        self,
        vocab_ids: torch.Tensor,
        slot_ids: torch.Tensor,
        pos_ids: torch.Tensor,
        num_feats: torch.Tensor,
        mask: torch.Tensor,
        global_feats: torch.Tensor,
    ) -> Tuple[torch.Tensor, torch.Tensor]:
        tags = self.tag_table[vocab_ids]
        tok = (
            self.card_emb(vocab_ids)
            + self.tag_proj(tags)
            + self.slot_emb(slot_ids)
            + self.pos_emb(pos_ids)
            + self.num_proj(num_feats)
        )
        # Inject global context vector into token 0 ([GLOBAL]).
        g_tok = self.global_proj(global_feats)
        tok = tok.clone()
        tok[:, 0, :] = tok[:, 0, :] + g_tok
        tok = self.in_norm(tok) * mask.unsqueeze(-1).to(tok.dtype)

        for blk in self.blocks:
            tok = blk(tok, mask)

        g_out = tok[:, 0, :]
        mask_f = mask.unsqueeze(-1).to(tok.dtype)
        denom = mask_f.sum(dim=1).clamp(min=1.0)
        mean_pool = (tok * mask_f).sum(dim=1) / denom
        masked_for_max = tok.masked_fill(~mask.unsqueeze(-1), -1e4)
        max_pool = masked_for_max.max(dim=1).values

        h = self.trunk(torch.cat([g_out, mean_pool, max_pool, global_feats], dim=-1))
        place_logits = self.place_head(h)
        power_pred = self.power_head(h).squeeze(-1)
        return place_logits, power_pred

    def expected_utility(
        self,
        vocab_ids: torch.Tensor,
        slot_ids: torch.Tensor,
        pos_ids: torch.Tensor,
        num_feats: torch.Tensor,
        mask: torch.Tensor,
        global_feats: torch.Tensor,
    ) -> torch.Tensor:
        logits, _ = self.forward(vocab_ids, slot_ids, pos_ids, num_feats, mask, global_feats)
        probs = F.softmax(logits, dim=-1)
        return torch.matmul(probs, self.place_weights)


class SynergyValuePredictor:
    """Fast CPU inference wrapper around `SynergyValueNet` for beam search & leaf rollouts."""

    def __init__(
        self,
        catalog_name: str = "full",
        checkpoint_path: Optional[os.PathLike | str] = None,
    ) -> None:
        torch.set_num_threads(1)
        self.catalog_name = catalog_name
        self.featurizer = StateFeaturizer(catalog_name)
        self.meta = self.featurizer.meta
        self.model: Optional[SynergyValueNet] = None
        self.loaded_path: Optional[str] = None

        ckpt = Path(checkpoint_path) if checkpoint_path is not None else DEFAULT_CHECKPOINT_PATH
        if ckpt.is_file():
            self.load(ckpt)

    @property
    def is_ready(self) -> bool:
        return self.model is not None

    def load(self, ckpt_path: os.PathLike | str) -> bool:
        payload = torch.load(str(ckpt_path), map_location="cpu", weights_only=False)
        d_model = int(payload.get("d_model", 64))
        n_heads = int(payload.get("n_heads", 4))
        n_layers = int(payload.get("n_layers", 2))
        tag_mat = np.asarray(self.meta.tag_matrix, dtype=np.float32)

        net = SynergyValueNet(
            vocab_size=self.meta.vocab_size,
            tag_matrix=tag_mat,
            global_dim=self.featurizer.global_dim,
            d_model=d_model,
            n_heads=n_heads,
            n_layers=n_layers,
        )
        net.load_state_dict(payload["state_dict"], strict=True)
        net.eval()
        self.model = net
        self.loaded_path = str(ckpt_path)
        return True

    def predict_batch(
        self,
        obs_list: Sequence[seaglass.TavernObservation],
        ctx: Optional[seaglass.LobbyContext] = None,
        end_of_turn: bool = True,
    ) -> List[float]:
        if self.model is None or not obs_list:
            return [0.0] * len(obs_list)
        vocab, slot, pos, num, mask, glob = self.featurizer.featurize_batch(
            obs_list, ctx, end_of_turn
        )
        with torch.inference_mode():
            vals = self.model.expected_utility(
                torch.from_numpy(vocab),
                torch.from_numpy(slot),
                torch.from_numpy(pos),
                torch.from_numpy(num),
                torch.from_numpy(mask),
                torch.from_numpy(glob),
            )
            return vals.tolist()

    def predict_one(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext] = None,
        end_of_turn: bool = True,
    ) -> float:
        if self.model is None:
            return 0.0
        return self.predict_batch([obs], ctx=ctx, end_of_turn=end_of_turn)[0]

    def top_learned_synergy_pairs(
        self,
        min_tier: int = 2,
        top_k: int = 15,
        max_cards: int = 90,
    ) -> List[Tuple[float, str, str]]:
        """
        Probe `SynergyValueNet` for non-linear 2-card interaction lift:
          Lift(A, B) = V({A, B}) - V({A}) - V({B}) + V({})
        evaluated on a clean Turn-9 Tier-5 tavern state.
        """
        if self.model is None:
            return []

        cands = [
            t
            for t in self.meta.templates_by_id.values()
            if t.tavern_tier >= min_tier
        ]
        # Focus on cards with scaling/combat/engine hooks so probe finishes in < 1s.
        cands.sort(
            key=lambda t: (
                self.meta.profile(t.card_id).engine_weight
                if self.meta.profile(t.card_id)
                else 0.0,
                t.tavern_tier,
            ),
            reverse=True,
        )
        cands = cands[:max_cards]

        def _make_state_obs(units: Sequence[seaglass.CardTemplate]) -> seaglass.TavernObservation:
            g = seaglass.TavernGame(seed=101, catalog=self.catalog_name)
            g.start_turn()
            # Build clean mid-game reference tavern via unit templates placed through synthetic tensors.
            return g.observe()

        base_obs = _make_state_obs([])
        # Construct batched tensors directly for empty, singletons, and pairs!
        base_vocab = np.zeros(MAX_TOKENS, dtype=np.int64)
        base_slot = np.zeros(MAX_TOKENS, dtype=np.int64)
        base_pos = np.zeros(MAX_TOKENS, dtype=np.int64)
        base_num = np.zeros((MAX_TOKENS, TOKEN_NUM_DIM), dtype=np.float32)
        base_mask = np.zeros(MAX_TOKENS, dtype=np.bool_)
        base_glob = np.zeros(self.featurizer.global_dim, dtype=np.float32)
        self.featurizer.featurize_into(
            base_obs, None, True, base_vocab, base_slot, base_pos, base_num, base_mask, base_glob
        )
        # Set Turn 9, Tier 5, 30 HP reference context.
        base_glob[0] = 9.0 / 16.0
        base_glob[1] = 6.0 / 14.0
        base_glob[2] = 30.0 / 40.0
        base_glob[3] = 1.0
        base_glob[5:11] = 0.0
        base_glob[5 + 4] = 1.0  # Tier 5
        base_glob[12] = 0.0

        unit_vecs: List[Tuple[int, np.ndarray, str]] = []
        for t in cands:
            u = t.instantiate()
            nf = np.zeros(TOKEN_NUM_DIM, dtype=np.float32)
            self.featurizer._fill_unit_num(u, nf)
            unit_vecs.append((self.meta.vocab_index(t.card_id), nf, f"{t.name} (T{t.tavern_tier})"))

        # Use a fixed 7-unit counterfactual reference board (5 neutral Tier-1 fillers + 2 test slots)
        # so board token count stays identically 7 (in-distribution for Turn 9).
        filler_tmpl = next(
            (t for t in self.meta.templates_by_id.values() if t.tavern_tier == 1 and t.tribe == "none"),
            next(iter(self.meta.templates_by_id.values())),
        )
        filler_vid = self.meta.vocab_index(filler_tmpl.card_id)
        filler_nf = np.zeros(TOKEN_NUM_DIM, dtype=np.float32)
        self.featurizer._fill_unit_num(filler_tmpl.instantiate(), filler_nf)

        ref_vocab = np.zeros(MAX_TOKENS, dtype=np.int64)
        ref_slot = np.zeros(MAX_TOKENS, dtype=np.int64)
        ref_pos = np.zeros(MAX_TOKENS, dtype=np.int64)
        ref_num = np.zeros((MAX_TOKENS, TOKEN_NUM_DIM), dtype=np.float32)
        ref_mask = np.zeros(MAX_TOKENS, dtype=np.bool_)
        ref_slot[0] = 1
        ref_mask[0] = True
        for b_i in range(1, 8):
            ref_vocab[b_i] = filler_vid
            ref_slot[b_i] = 2
            ref_pos[b_i] = b_i
            ref_num[b_i] = filler_nf
            ref_mask[b_i] = True

        # Evaluate baseline (all fillers), slot-6 singletons, and slot-7 singletons.
        n_c = len(unit_vecs)
        total_s = 1 + 2 * n_c
        s_vocab = np.repeat(ref_vocab[None, :], total_s, axis=0)
        s_slot = np.repeat(ref_slot[None, :], total_s, axis=0)
        s_pos = np.repeat(ref_pos[None, :], total_s, axis=0)
        s_num = np.repeat(ref_num[None, :, :], total_s, axis=0)
        s_mask = np.repeat(ref_mask[None, :], total_s, axis=0)
        s_glob = np.repeat(base_glob[None, :], total_s, axis=0)

        for idx, (vid, nf, _) in enumerate(unit_vecs):
            # Slot 6 replacement
            r6 = 1 + idx
            s_vocab[r6, 6] = vid
            s_num[r6, 6] = nf
            # Slot 7 replacement
            r7 = 1 + n_c + idx
            s_vocab[r7, 7] = vid
            s_num[r7, 7] = nf

        with torch.inference_mode():
            s_vals = self.model.expected_utility(
                torch.from_numpy(s_vocab),
                torch.from_numpy(s_slot),
                torch.from_numpy(s_pos),
                torch.from_numpy(s_num),
                torch.from_numpy(s_mask),
                torch.from_numpy(s_glob),
            ).numpy()

        v_empty = float(s_vals[0])
        v_slot6 = s_vals[1 : 1 + n_c]
        v_slot7 = s_vals[1 + n_c :]

        pair_indices: List[Tuple[int, int]] = []
        for i in range(n_c):
            for j in range(i + 1, n_c):
                pair_indices.append((i, j))

        lifts: List[Tuple[float, str, str]] = []
        chunk = 1024
        for start in range(0, len(pair_indices), chunk):
            sub = pair_indices[start : start + chunk]
            m = len(sub)
            p_vocab = np.repeat(ref_vocab[None, :], m, axis=0)
            p_slot = np.repeat(ref_slot[None, :], m, axis=0)
            p_pos = np.repeat(ref_pos[None, :], m, axis=0)
            p_num = np.repeat(ref_num[None, :, :], m, axis=0)
            p_mask = np.repeat(ref_mask[None, :], m, axis=0)
            p_glob = np.repeat(base_glob[None, :], m, axis=0)

            for k, (i, j) in enumerate(sub):
                vi, nfi, _ = unit_vecs[i]
                vj, nfj, _ = unit_vecs[j]
                p_vocab[k, 6] = vi
                p_num[k, 6] = nfi
                p_vocab[k, 7] = vj
                p_num[k, 7] = nfj

            with torch.inference_mode():
                p_vals = self.model.expected_utility(
                    torch.from_numpy(p_vocab),
                    torch.from_numpy(p_slot),
                    torch.from_numpy(p_pos),
                    torch.from_numpy(p_num),
                    torch.from_numpy(p_mask),
                    torch.from_numpy(p_glob),
                ).numpy()

            for k, (i, j) in enumerate(sub):
                lift = float(p_vals[k] - v_slot6[i] - v_slot7[j] + v_empty)
                lifts.append((lift, unit_vecs[i][2], unit_vecs[j][2]))

        lifts.sort(key=lambda x: x[0], reverse=True)
        return lifts[:top_k]


_PREDICTOR_CACHE: Dict[Tuple[str, str], SynergyValuePredictor] = {}


def get_value_predictor(
    catalog_name: str = "full",
    checkpoint_path: Optional[os.PathLike | str] = None,
    force_reload: bool = False,
) -> SynergyValuePredictor:
    resolved = str(Path(checkpoint_path) if checkpoint_path else DEFAULT_CHECKPOINT_PATH)
    key = (catalog_name, resolved)
    if force_reload or key not in _PREDICTOR_CACHE:
        _PREDICTOR_CACHE[key] = SynergyValuePredictor(
            catalog_name=catalog_name, checkpoint_path=resolved
        )
    return _PREDICTOR_CACHE[key]
