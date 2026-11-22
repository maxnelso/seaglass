"""Lightweight Card-Embedding + MLP Actor-Critic (`BattlegroundsMLP`)."""

from __future__ import annotations

import numpy as np
import torch
import torch.nn as nn

from .actions import ACTION_KIND_IDS, ACTION_SPACE_SIZE, NUM_ACTION_KINDS
from .card_features import STATIC_DIM, static_card_table
from .features import NUM_SLOTS, PLAYER_DIM, SLOT_NUM_DIM, VOCAB_SIZE


def sample_categorical(probs: np.ndarray, rng: np.random.Generator) -> np.ndarray:
    """One index per row of `probs` `(B, n)`, drawn with those probabilities; never a 0-probability
    (masked) index.

    The first index whose cumulative probability exceeds `u * total` (scaling by the row's own
    float32 total, which can fall short of 1), capped at the row's last positive index (the
    product can round up to the total).
    """
    cum = probs.cumsum(axis=-1)
    u = rng.random((probs.shape[0], 1), dtype=np.float32)
    acts = (cum <= u * cum[:, -1:]).sum(axis=-1)
    last = probs.shape[1] - 1 - np.argmax(probs[:, ::-1] > 0, axis=-1)
    return np.minimum(acts, last)


def place_atoms(k: int) -> torch.Tensor:
    """The reward of each place 1..k, `play.placement_reward(place, k)`: 1 for first to -1 for last
    (0 for a single place)."""
    if k <= 1:
        return torch.zeros(max(k, 1))
    return torch.tensor([1.0 - 2.0 * i / (k - 1) for i in range(k)], dtype=torch.float32)


class BattlegroundsMLP(nn.Module):
    """Embeds each slot's `card_id` (`nn.Embedding`), concatenates slot stats + player state, and runs an MLP.

    With `static_features`, a card's vector is its learned embedding plus a learned projection of its
    static features (`card_features.static_card_table()`: effect tags and a rules-text embedding),
    so cards that do similar things start out similar and rare cards borrow from common ones. The
    table is a non-persistent buffer, rebuilt from `card_features` at construction.
    """

    def __init__(
        self,
        vocab_size: int = VOCAB_SIZE,
        card_embed_dim: int = 8,
        hidden_dim: int = 128,
        num_actions: int = ACTION_SPACE_SIZE,
        seed: int = 42,
        kind_prior: bool = False,
        static_features: bool = False,
        value_places: int = 0,
        aux_heads: int = 0,
    ) -> None:
        super().__init__()
        # The constructor's arguments, saved with checkpoints (`checkpoint.py`).
        self.config = {
            "vocab_size": vocab_size,
            "card_embed_dim": card_embed_dim,
            "hidden_dim": hidden_dim,
            "num_actions": num_actions,
            "kind_prior": kind_prior,
            "static_features": static_features,
            "value_places": value_places,
            "aux_heads": aux_heads,
        }
        # Subtract log(number of legal actions of its kind) from each logit, so that an untrained
        # policy picks among the legal kinds evenly (`actions.ACTION_KIND_IDS`).
        self.kind_prior = kind_prior
        kind_onehot = np.zeros((num_actions, NUM_ACTION_KINDS), dtype=np.float32)
        kind_onehot[np.arange(num_actions), ACTION_KIND_IDS[:num_actions]] = 1.0
        self._kind_onehot_np = kind_onehot
        self.register_buffer("kind_onehot", torch.from_numpy(kind_onehot), persistent=False)
        self.card_embed_dim = card_embed_dim
        self.card_embed = nn.Embedding(vocab_size, card_embed_dim, padding_idx=0)
        self.static_features = static_features and STATIC_DIM > 0
        if self.static_features:
            table = torch.from_numpy(static_card_table()[:vocab_size])
            self.register_buffer("static_table", table, persistent=False)
            # No bias, so "no card" (an all-zero row) stays the zero vector.
            self.static_proj = nn.Linear(STATIC_DIM, card_embed_dim, bias=False)
        self.in_dim = PLAYER_DIM + NUM_SLOTS * (card_embed_dim + SLOT_NUM_DIM)
        self.emb_end = PLAYER_DIM + NUM_SLOTS * card_embed_dim

        self.trunk = nn.Sequential(
            nn.Linear(self.in_dim, hidden_dim),
            nn.ReLU(),
            nn.Linear(hidden_dim, hidden_dim),
            nn.ReLU(),
        )
        self.actor = nn.Linear(hidden_dim, num_actions)
        # The value: a scalar, or with `value_places` K > 0 a distribution over the places 1..K
        # whose expectation (over the reward of each place, `value_atoms`) is the value.
        self.value_places = value_places
        self.critic = nn.Linear(hidden_dim, value_places if value_places > 0 else 1)
        if value_places > 0:
            self.register_buffer("value_atoms", place_atoms(value_places), persistent=False)
        # Auxiliary targets (`ppo_update(aux_coef=...)`): `aux_heads` logits from the trunk features,
        # trained only by their own loss. Absent (no parameters) with 0, so old checkpoints load.
        self.aux_heads = aux_heads
        if aux_heads > 0:
            self.aux = nn.Linear(hidden_dim, aux_heads)
        self._np_rng = np.random.default_rng(seed)
        self._x_buf = np.empty((32, self.in_dim), dtype=np.float32)

    def forward(
        self,
        player_vec: torch.Tensor,
        slot_card_ids: torch.Tensor,
        slot_num_feats: torch.Tensor,
        action_mask: torch.Tensor | None = None,
        return_value_logits: bool = False,
        return_aux: bool = False,
    ) -> tuple[torch.Tensor, ...]:
        """Forward pass returning `(masked_logits, value)`; with `return_value_logits`,
        `(masked_logits, value, value_logits)` (the place logits, None for a scalar critic); with
        `return_aux` (either way), `(masked_logits, value, value_logits, aux_logits)` (the aux
        logits `(B, aux_heads)`, None without aux heads)."""
        if self.static_features:
            emb = self.card_table()[slot_card_ids].flatten(start_dim=1)
        else:
            emb = self.card_embed(slot_card_ids).flatten(start_dim=1)
        num_flat = slot_num_feats.flatten(start_dim=1)
        x = torch.cat([player_vec, emb, num_flat], dim=-1)
        h = self.trunk(x)
        logits = self.actor(h)
        if action_mask is not None:
            if self.kind_prior:
                counts = action_mask.float() @ self.kind_onehot  # (B, kinds)
                per_action = counts @ self.kind_onehot.T  # (B, actions)
                logits = logits - torch.log(per_action.clamp(min=1.0))
            logits = logits.masked_fill(~action_mask, -1e9)
        out = self.critic(h)
        if self.value_places > 0:
            value = torch.softmax(out, dim=-1) @ self.value_atoms
            value_logits = out
        else:
            value = out.squeeze(-1)
            value_logits = None
        if return_aux:
            return logits, value, value_logits, (self.aux(h) if self.aux_heads > 0 else None)
        if return_value_logits:
            return logits, value, value_logits
        return logits, value

    def card_table(self) -> torch.Tensor:
        """Every card's vector, `(vocab_size, card_embed_dim)`: the embedding, plus the projected
        static features with `static_features`."""
        if not self.static_features:
            return self.card_embed.weight
        return self.card_embed.weight + self.static_proj(self.static_table)

    def act_batch(
        self,
        p_np: np.ndarray,
        c_np: np.ndarray,
        n_np: np.ndarray,
        m_np: np.ndarray,
        deterministic: bool = False,
    ) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
        """Zero-copy NumPy batched forward + sampling for fast CPU rollouts."""
        B = p_np.shape[0]
        if B > self._x_buf.shape[0]:
            self._x_buf = np.empty((B, self.in_dim), dtype=np.float32)

        with torch.no_grad():
            embed_w = self.card_table().numpy()
        w1 = self.trunk[0].weight.detach().numpy()
        b1 = self.trunk[0].bias.detach().numpy()
        w2 = self.trunk[2].weight.detach().numpy()
        b2 = self.trunk[2].bias.detach().numpy()
        wa = self.actor.weight.detach().numpy()
        ba = self.actor.bias.detach().numpy()
        wc = self.critic.weight.detach().numpy()
        bc = self.critic.bias.detach().numpy()

        x = self._x_buf[:B]
        x[:, :PLAYER_DIM] = p_np
        x[:, PLAYER_DIM : self.emb_end] = embed_w[c_np].reshape(B, -1)
        x[:, self.emb_end :] = n_np.reshape(B, -1)

        h1 = np.maximum(0.0, x @ w1.T + b1)
        h2 = np.maximum(0.0, h1 @ w2.T + b2)
        raw = h2 @ wa.T + ba
        if self.kind_prior:
            m = m_np.astype(np.float32)
            per_action = (m @ self._kind_onehot_np) @ self._kind_onehot_np.T
            raw = raw - np.log(np.maximum(per_action, 1.0))
        logits = np.where(m_np, raw, -1e9)
        vals = h2 @ wc.T + bc
        if self.value_places > 0:
            vals = vals - vals.max(axis=-1, keepdims=True)
            pv = np.exp(vals)
            vals = (pv / pv.sum(axis=-1, keepdims=True)) @ self.value_atoms.numpy()
        else:
            vals = vals.squeeze(-1)

        logits -= logits.max(axis=-1, keepdims=True)
        exp_l = np.exp(logits)
        sums = exp_l.sum(axis=-1, keepdims=True)
        probs = exp_l / sums
        lps = logits - np.log(sums)

        if deterministic:
            acts = logits.argmax(axis=-1)
        else:
            acts = sample_categorical(probs, self._np_rng)

        act_lps = lps[np.arange(B), acts]
        return acts, act_lps, vals

    def act(
        self,
        obs: tuple,
        mask_np: np.ndarray,
        deterministic: bool = False,
        device: str = "cpu",
    ) -> tuple[int, float, float]:
        player_vec, slot_card_ids, slot_num_feats = obs
        acts, lps, vals = self.act_batch(
            player_vec[np.newaxis, :],
            slot_card_ids[np.newaxis, :],
            slot_num_feats[np.newaxis, :, :],
            mask_np[np.newaxis, :],
            deterministic=deterministic,
        )
        return int(acts[0]), float(lps[0]), float(vals[0])

