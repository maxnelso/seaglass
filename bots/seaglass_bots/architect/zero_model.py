"""ZeroValueNet & ZeroFeaturizer: Pure, Non-Hardcoded Patch 37.0.2 Set-Transformer.

Encodes the COMPLETE Rust engine state (Board, Hand, Shop, Trinkets, Hero, all 53
player_counters, dark_discoveries_left, 5 lobby tribes, opponent strength) PLUS
exact Rust physics forward-rollout velocity signals (`eot_stat_delta`, `perm_combat_delta`,
`eot_hand_delta`, `sot_gold_next`, `combat_win_rate`, `combat_dmg_mean`) with
ZERO hardcoded card names, hero tier lists, or trinket tier lists!
"""

from __future__ import annotations

import math
import os
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Tuple

import numpy as np
import seaglass

try:
    import torch
    import torch.nn as nn
    import torch.nn.functional as F
except ImportError:  # pragma: no cover
    torch = None
    nn = None
    F = None


ZERO_MAX_TOKENS = 28
ZERO_TOKEN_NUM_DIM = 42
ZERO_PHYSICS_DIM = 11
ZERO_NUM_COUNTERS = len(seaglass.player_counter_names())  # 53 in Patch 37.0.2
ZERO_GLOBAL_DIM = 20 + 16 + 8 + ZERO_NUM_COUNTERS + ZERO_PHYSICS_DIM  # 108

TRIBE_LIST: Tuple[str, ...] = (
    "aberration",
    "beast",
    "demon",
    "dragon",
    "elemental",
    "mech",
    "murloc",
    "pirate",
    "quilboar",
    "undead",
)
TRIBE_INDEX: Dict[str, int] = {t: i for i, t in enumerate(TRIBE_LIST)}

# Laser-sharp 1st-place tournament utility weights
ZERO_PLACE_WEIGHTS = np.array(
    [55.0, 18.0, 5.0, -3.0, -11.0, -19.0, -27.0, -35.0], dtype=np.float32
)

DEFAULT_ZERO_NPZ_PATH = str(
    Path(__file__).resolve().parent.parent / "data" / "zero_value_v1.npz"
)


def _symlog(x: float) -> float:
    if x == 0.0:
        return 0.0
    return math.copysign(math.log1p(abs(x)), x)


class ZeroVocab:
    """Deterministic vocabulary mapping EVERY Rust card/spell/trinket/hero ID -> [1..N]."""

    def __init__(self) -> None:
        ids: List[int] = []
        seen: set[int] = set()
        for cid in sorted(seaglass.all_card_ids()):
            ci = int(cid)
            if ci not in seen:
                seen.add(ci)
                ids.append(ci)
        for hid, _, _ in sorted(seaglass.heroes_table(), key=lambda r: int(r[0])):
            hi = int(hid)
            if hi not in seen:
                seen.add(hi)
                ids.append(hi)
        for tid, _, _, _, _ in sorted(seaglass.trinkets_table(), key=lambda r: int(r[0])):
            ti = int(tid)
            if ti not in seen:
                seen.add(ti)
                ids.append(ti)
        self.id_to_idx: Dict[int, int] = {cid: i + 1 for i, cid in enumerate(ids)}
        self.size: int = len(ids) + 1  # 0 = PAD/UNK

        # Precompute 6 engine-native card hook tags directly from Rust `card_tags()`
        # AND full static card feature table (all engine hook tags + 32-dim PCA rules-text embeddings)!
        tag_names, tag_table = seaglass.card_tags()
        t_idx = {n: i for i, n in enumerate(tag_names)}
        want_tags = ("battlecry", "deathrattle", "end_of_turn", "rally", "flag_cleave", "flag_avenge")
        cols = [t_idx.get(t, -1) for t in want_tags]
        self.card_hook_feats: Dict[int, Tuple[float, float, float, float, float, float]] = {}
        for cid, row in tag_table.items():
            self.card_hook_feats[int(cid)] = tuple(
                1.0 if (c >= 0 and row[c] > 0.5) else 0.0 for c in cols
            )  # type: ignore[assignment]

        tag_dim = len(tag_names)
        text_dim = 32
        text_map: Dict[int, np.ndarray] = {}
        text_path = (
            Path(__file__).resolve().parents[3]
            / "rl"
            / "seaglass_rl"
            / "data"
            / "card_text_embeddings.npz"
        )
        if text_path.exists():
            try:
                td = np.load(text_path)
                emb = td["emb"].astype(np.float32)
                if emb.ndim == 2 and emb.shape[1] == text_dim:
                    for i_c, cid_v in enumerate(td["ids"]):
                        text_map[int(cid_v)] = emb[i_c]
            except Exception:
                pass

        static_dim = tag_dim + text_dim
        static_tbl = np.zeros((self.size, static_dim), dtype=np.float32)
        for cid, vidx in self.id_to_idx.items():
            tr = tag_table.get(cid)
            if tr is not None:
                static_tbl[vidx, :tag_dim] = tr
            te = text_map.get(cid)
            if te is not None:
                static_tbl[vidx, tag_dim:] = te
        self.static_dim: int = static_dim
        self.static_table: np.ndarray = static_tbl

        self.hero_names: Dict[int, str] = {int(h[0]): str(h[1]) for h in seaglass.heroes_table()}
        self.trinket_names: Dict[int, str] = {int(t[0]): str(t[1]) for t in seaglass.trinkets_table()}
        self.trinket_impl: Dict[int, bool] = {int(t[0]): bool(t[4]) for t in seaglass.trinkets_table()}


_CACHED_VOCAB: Optional[ZeroVocab] = None


def get_zero_vocab() -> ZeroVocab:
    global _CACHED_VOCAB
    if _CACHED_VOCAB is None:
        _CACHED_VOCAB = ZeroVocab()
    return _CACHED_VOCAB


class ZeroFeaturizer:
    """Converts `(obs, ctx, physics_vec)` batches into dense NumPy arrays for `ZeroValueNet`."""

    def __init__(self) -> None:
        self.vocab = get_zero_vocab()

    def featurize_batch(
        self,
        obs_list: Sequence[seaglass.TavernObservation],
        ctx: Optional[seaglass.LobbyContext] = None,
        physics_list: Optional[Sequence[Sequence[float]]] = None,
        end_of_turn: bool = True,
    ) -> Tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
        B = len(obs_list)
        v_arr = np.zeros((B, ZERO_MAX_TOKENS), dtype=np.int32)
        z_arr = np.zeros((B, ZERO_MAX_TOKENS), dtype=np.int8)
        p_arr = np.zeros((B, ZERO_MAX_TOKENS), dtype=np.int8)
        n_arr = np.zeros((B, ZERO_MAX_TOKENS, ZERO_TOKEN_NUM_DIM), dtype=np.float32)
        m_arr = np.zeros((B, ZERO_MAX_TOKENS), dtype=np.bool_)
        g_arr = np.zeros((B, ZERO_GLOBAL_DIM), dtype=np.float32)

        id_to_idx = self.vocab.id_to_idx
        hook_map = self.vocab.card_hook_feats

        # Precompute shared lobby context features once per batch
        alive_units = 8.0
        next_is_ghost = 0.0
        health_rank = 4.0
        dmg_cap = 15.0
        has_cap = 1.0
        lobby_tribe_vec = np.zeros(10, dtype=np.float32)
        opp_tier = 1.0
        opp_stats = 0.0
        opp_max_hp = 0.0
        opp_seen_dt = 6.0
        lobby_max_stats = 0.0

        if ctx is not None:
            alive_units = float(ctx.alive_units)
            next_is_ghost = 1.0 if ctx.next_is_ghost else 0.0
            health_rank = float(ctx.health_rank)
            if ctx.damage_cap is not None:
                dmg_cap = float(ctx.damage_cap)
                has_cap = 1.0
            else:
                dmg_cap = 0.0
                has_cap = 0.0
            if ctx.tribes:
                for tr in ctx.tribes:
                    ti = TRIBE_INDEX.get(str(tr))
                    if ti is not None:
                        lobby_tribe_vec[ti] = 1.0
            if ctx.opponents:
                for opp in ctx.opponents:
                    if opp.seen_board:
                        st = float(sum(max(0, u.attack) + max(0, u.health) for u in opp.seen_board))
                        if opp.alive and st > lobby_max_stats:
                            lobby_max_stats = st
                        if opp.is_next:
                            opp_tier = float(opp.tier)
                            opp_stats = st
                            opp_max_hp = float(max((u.health for u in opp.seen_board), default=0))
                            opp_seen_dt = float(max(0, (obs_list[0].turn - opp.seen_turn) if opp.seen_turn is not None else 6))
                    elif opp.is_next:
                        opp_tier = float(opp.tier)

        for b, obs in enumerate(obs_list):
            tok = 0

            def _write_unit(u: seaglass.Unit, zone: int, pos: int) -> None:
                nonlocal tok
                if tok >= ZERO_MAX_TOKENS:
                    return
                v_arr[b, tok] = id_to_idx.get(int(u.card_id), 0)
                z_arr[b, tok] = zone
                p_arr[b, tok] = min(9, pos)
                m_arr[b, tok] = True
                nf = n_arr[b, tok]
                nf[0] = math.log1p(max(0, u.attack)) / 8.0
                nf[1] = math.log1p(max(0, u.health)) / 8.0
                nf[2] = math.log1p(max(0, u.base_attack)) / 4.0
                nf[3] = math.log1p(max(0, u.base_health)) / 4.0
                nf[4] = u.tavern_tier / 7.0
                nf[5] = 1.0 if u.is_golden else 0.0
                nf[6] = 1.0 if u.is_spell else 0.0
                nf[7] = min(6.0, float(u.spell_cost)) / 6.0
                nf[8] = 1.0 if u.costs_health else 0.0
                nf[9] = 1.0 if u.unplayable else 0.0
                nf[10] = 1.0 if u.activated_this_turn else 0.0
                nf[11] = 1.0 if u.taunt else 0.0
                nf[12] = 1.0 if u.divine_shield else 0.0
                nf[13] = 1.0 if u.windfury else 0.0
                nf[14] = 1.0 if u.reborn else 0.0
                nf[15] = 1.0 if u.venomous else 0.0
                nf[16] = 1.0 if u.stealth else 0.0
                nf[17] = 1.0 if u.magnetic else 0.0
                nf[18] = min(3.0, float(u.extra_deathrattles)) / 3.0
                nf[19] = min(2.0, float(len(u.dark_gifts))) / 2.0
                ps = u.perm_stats
                nf[20] = math.log1p(max(0, (ps[0] + ps[1]) if isinstance(ps, tuple) else int(ps))) / 6.0
                nf[21] = math.log1p(max(0, u.end_of_turn_stats)) / 5.0
                nf[22] = min(5.0, float(max(0, u.stacks))) / 5.0
                nf[23] = min(3.0, float(max(0, u.charges))) / 3.0
                nf[24] = min(10.0, float(max(0, u.blood_gems_played))) / 10.0
                nf[25] = min(4.0, float(max(0, u.avenge_counter))) / 4.0
                hk = hook_map.get(int(u.card_id))
                if hk is not None:
                    nf[26:32] = hk
                for tr in u.tribes:
                    if tr == "all":
                        nf[32:42] = 1.0
                        break
                    ti = TRIBE_INDEX.get(tr)
                    if ti is not None:
                        nf[32 + ti] = 1.0
                tok += 1

            for idx, u in enumerate(obs.board):
                _write_unit(u, 0, idx)
            for idx, c in enumerate(obs.hand):
                _write_unit(c, 1, idx)
            for idx, s in enumerate(obs.shop):
                _write_unit(s, 2, idx)
            for idx, tid in enumerate(obs.trinkets):
                if tok < ZERO_MAX_TOKENS:
                    v_arr[b, tok] = id_to_idx.get(int(tid), 0)
                    z_arr[b, tok] = 3
                    p_arr[b, tok] = min(9, idx)
                    m_arr[b, tok] = True
                    tok += 1
            if obs.hero_id and tok < ZERO_MAX_TOKENS:
                v_arr[b, tok] = id_to_idx.get(int(obs.hero_id), 0)
                z_arr[b, tok] = 4
                p_arr[b, tok] = 0
                m_arr[b, tok] = True
                tok += 1

            # Global features
            gf = g_arr[b]
            turn = max(1, int(obs.turn))
            my_stats = float(sum(max(0, u.attack) + max(0, u.health) for u in obs.board))
            my_max_hp = float(max((u.health for u in obs.board), default=0))
            gf[0] = min(20.0, float(turn)) / 20.0
            gf[1] = float(obs.tavern_tier) / 6.0
            gf[2] = min(11.0, float(obs.upgrade_cost)) / 11.0
            gf[3] = min(15.0, float(obs.gold)) / 15.0
            gf[4] = min(15.0, float(obs.max_gold)) / 15.0
            gf[5] = min(5.0, float(obs.bonus_gold_next_turn)) / 5.0
            gf[6] = min(4.0, float(obs.minion_cost)) / 4.0
            gf[7] = min(2.0, float(obs.refresh_cost)) / 2.0
            gf[8] = min(5.0, float(obs.free_refreshes)) / 5.0
            gf[9] = 1.0 if obs.is_frozen else 0.0
            gf[10] = max(0.0, float(obs.health)) / 40.0
            gf[11] = max(0.0, float(obs.armor)) / 25.0
            gf[12] = max(0.0, float(obs.health + obs.armor)) / 50.0
            gf[13] = float(len(obs.board)) / 7.0
            gf[14] = float(len(obs.hand)) / 10.0
            gf[15] = float(obs.dark_discoveries_left) / 3.0
            gf[16] = 1.0 if obs.dark_discovery_usable else 0.0
            gf[17] = 1.0 if obs.hero_power_usable else 0.0
            gf[18] = min(3.0, float(obs.hero_power_cost)) / 3.0
            gf[19] = 1.0 if end_of_turn else 0.0

            gf[20] = alive_units / 8.0
            gf[21] = next_is_ghost
            gf[22] = health_rank / 8.0
            gf[23] = dmg_cap / 15.0
            gf[24] = has_cap
            gf[25:35] = lobby_tribe_vec
            gf[35] = opp_tier / 6.0

            gf[36] = math.log1p(opp_stats) / 10.0
            gf[37] = math.log1p(opp_max_hp) / 8.0
            gf[38] = min(6.0, opp_seen_dt) / 6.0
            gf[39] = math.log1p(lobby_max_stats) / 10.0
            gf[40] = math.log1p(my_stats) / 10.0
            gf[41] = math.log1p(my_max_hp) / 8.0
            gf[42] = min(4.0, my_stats / (opp_stats + 1.0)) / 4.0
            gf[43] = min(4.0, my_stats / (lobby_max_stats + 1.0)) / 4.0

            # All 53 Rust player_counters!
            pc = obs.player_counters
            n_pc = min(ZERO_NUM_COUNTERS, len(pc))
            for ci in range(n_pc):
                val = pc[ci]
                if val != 0:
                    gf[44 + ci] = _symlog(float(val)) / 5.0

            # Exact Rust Physics features (11)
            if physics_list is not None and b < len(physics_list):
                pv = physics_list[b]
                k = min(ZERO_PHYSICS_DIM, len(pv))
                gf[44 + ZERO_NUM_COUNTERS : 44 + ZERO_NUM_COUNTERS + k] = pv[:k]

        return v_arr, z_arr, p_arr, n_arr, m_arr, g_arr


if nn is not None:

    class _ZeroTransformerBlock(nn.Module):
        def __init__(self, d_model: int, n_heads: int, d_ff: int, dropout: float = 0.15) -> None:
            super().__init__()
            self.n_heads = n_heads
            self.d_head = d_model // n_heads
            self.ln1 = nn.LayerNorm(d_model)
            self.qkv = nn.Linear(d_model, 3 * d_model)
            self.proj = nn.Linear(d_model, d_model)
            self.drop1 = nn.Dropout(dropout)
            self.ln2 = nn.LayerNorm(d_model)
            self.fc1 = nn.Linear(d_model, d_ff)
            self.fc2 = nn.Linear(d_ff, d_model)
            self.drop2 = nn.Dropout(dropout)

        def forward(self, x: torch.Tensor, mask: torch.Tensor) -> torch.Tensor:
            B, T, D = x.shape
            h = self.ln1(x)
            qkv = self.qkv(h).view(B, T, 3, self.n_heads, self.d_head)
            q = qkv[:, :, 0].transpose(1, 2)
            k = qkv[:, :, 1].transpose(1, 2)
            v = qkv[:, :, 2].transpose(1, 2)
            scores = torch.matmul(q, k.transpose(-2, -1)) / math.sqrt(self.d_head)
            key_mask = (~mask).unsqueeze(1).unsqueeze(2)
            scores = scores.masked_fill(key_mask, -1e4)
            attn = F.softmax(scores, dim=-1)
            out = torch.matmul(attn, v).transpose(1, 2).contiguous().view(B, T, D)
            x = x + self.drop1(self.proj(out))
            h2 = self.ln2(x)
            x = x + self.drop2(self.fc2(F.gelu(self.fc1(h2))))
            return x

    class ZeroValueNet(nn.Module):
        """3-Layer Pre-LN Set-Transformer Value Network predicting P(place=1..8) + 3-turn future power."""

        def __init__(
            self,
            vocab_size: Optional[int] = None,
            d_model: int = 128,
            n_heads: int = 4,
            n_layers: int = 3,
            d_ff: int = 256,
            dropout: float = 0.18,
            use_static_card_feats: bool = True,
        ) -> None:
            super().__init__()
            zvocab = get_zero_vocab()
            if vocab_size is None:
                vocab_size = zvocab.size
            self.vocab_size = vocab_size
            self.d_model = d_model
            self.n_heads = n_heads
            self.n_layers = n_layers
            self.d_ff = d_ff
            self.use_static_card_feats = use_static_card_feats

            self.tok_emb = nn.Embedding(vocab_size, d_model, padding_idx=0)
            if use_static_card_feats and zvocab.static_dim > 0 and zvocab.static_table.shape[0] == vocab_size:
                self.register_buffer(
                    "static_table",
                    torch.from_numpy(np.ascontiguousarray(zvocab.static_table)),
                    persistent=False,
                )
                self.static_proj = nn.Linear(zvocab.static_dim, d_model, bias=False)
            else:
                self.static_table = None
                self.static_proj = None
            self.zone_emb = nn.Embedding(6, d_model)
            self.pos_emb = nn.Embedding(12, d_model)
            self.num_proj = nn.Linear(ZERO_TOKEN_NUM_DIM, d_model)
            self.blocks = nn.ModuleList(
                [_ZeroTransformerBlock(d_model, n_heads, d_ff, dropout=dropout) for _ in range(n_layers)]
            )
            self.tok_ln = nn.LayerNorm(d_model)
            self.pool_q = nn.Parameter(torch.randn(2, d_model) * 0.05)

            self.glob_fc1 = nn.Linear(ZERO_GLOBAL_DIM, d_model)
            self.glob_ln1 = nn.LayerNorm(d_model)
            self.glob_fc2 = nn.Linear(d_model, d_model)

            self.trunk_fc1 = nn.Linear(3 * d_model, 256)
            self.trunk_ln1 = nn.LayerNorm(256)
            self.trunk_drop1 = nn.Dropout(dropout)
            self.trunk_fc2 = nn.Linear(256, 128)
            self.trunk_ln2 = nn.LayerNorm(128)
            self.trunk_drop2 = nn.Dropout(dropout)
            self.place_head = nn.Linear(128, 8)
            self.growth_head = nn.Linear(128, 1)

            self.register_buffer(
                "place_weights",
                torch.tensor(ZERO_PLACE_WEIGHTS, dtype=torch.float32),
            )

        def _baked_tok_table(self) -> torch.Tensor:
            w = self.tok_emb.weight
            if self.static_proj is not None and self.static_table is not None:
                w = w + self.static_proj(self.static_table)
                # Keep PAD row 0 strictly zeroed
                pad_zero = torch.zeros_like(w[:1])
                w = torch.cat([pad_zero, w[1:]], dim=0)
            return w

        def forward(
            self,
            vocab: torch.Tensor,
            zone: torch.Tensor,
            pos: torch.Tensor,
            num: torch.Tensor,
            mask: torch.Tensor,
            glob: torch.Tensor,
        ) -> Tuple[torch.Tensor, torch.Tensor]:
            tbl = self._baked_tok_table()
            x = (
                F.embedding(vocab.long(), tbl, padding_idx=0)
                + self.zone_emb(zone.long())
                + self.pos_emb(pos.long())
                + self.num_proj(num)
            )
            for blk in self.blocks:
                x = blk(x, mask)
            x = self.tok_ln(x)

            # 2-query attention pooling over valid tokens
            q = self.pool_q.unsqueeze(0).expand(x.shape[0], -1, -1)  # (B, 2, D)
            attn_s = torch.matmul(q, x.transpose(-2, -1)) / math.sqrt(self.d_model)
            attn_s = attn_s.masked_fill((~mask).unsqueeze(1), -1e4)
            attn_w = F.softmax(attn_s, dim=-1)
            pooled = torch.matmul(attn_w, x).reshape(x.shape[0], 2 * self.d_model)

            g = F.gelu(self.glob_ln1(self.glob_fc1(glob)))
            g = F.gelu(self.glob_fc2(g))

            h = torch.cat([pooled, g], dim=-1)
            h = self.trunk_drop1(F.gelu(self.trunk_ln1(self.trunk_fc1(h))))
            h = self.trunk_drop2(F.gelu(self.trunk_ln2(self.trunk_fc2(h))))
            return self.place_head(h), self.growth_head(h).squeeze(-1)

        def expected_utility(
            self,
            vocab: torch.Tensor,
            zone: torch.Tensor,
            pos: torch.Tensor,
            num: torch.Tensor,
            mask: torch.Tensor,
            glob: torch.Tensor,
        ) -> torch.Tensor:
            logits, _ = self.forward(vocab, zone, pos, num, mask, glob)
            probs = F.softmax(logits, dim=-1)
            return torch.matmul(probs, self.place_weights)

        def save_npz(self, path: str, use_fp16: bool = True) -> None:
            Path(path).parent.mkdir(parents=True, exist_ok=True)
            arrays: Dict[str, np.ndarray] = {
                "_meta_vocab": np.array([self.vocab_size], dtype=np.int32),
                "_meta_d_model": np.array([self.d_model], dtype=np.int32),
                "_meta_n_heads": np.array([self.n_heads], dtype=np.int32),
                "_meta_n_layers": np.array([self.n_layers], dtype=np.int32),
                "_meta_d_ff": np.array([self.d_ff], dtype=np.int32),
            }
            # Bake static card tags + rules-text projections into tok_emb.weight for ZERO-cost NumPy inference!
            with torch.no_grad():
                baked = self._baked_tok_table().detach().cpu().numpy()
            for k, v in self.state_dict().items():
                if k in ("place_weights", "static_table", "static_proj.weight"):
                    continue
                arr = baked if k == "tok_emb.weight" else v.detach().cpu().numpy()
                if use_fp16 and arr.ndim >= 2 and arr.dtype == np.float32:
                    arr = arr.astype(np.float16)
                arrays[k] = arr
            # Also store raw tok_emb & static_proj if warm-starting PyTorch training cleanly
            if self.static_proj is not None:
                arrays["_raw_tok_emb.weight"] = self.tok_emb.weight.detach().cpu().numpy().astype(np.float16 if use_fp16 else np.float32)
                arrays["_raw_static_proj.weight"] = self.static_proj.weight.detach().cpu().numpy().astype(np.float16 if use_fp16 else np.float32)
            np.savez_compressed(path, **arrays)

        def load_npz(self, path: str) -> None:
            data = np.load(path)
            sd = self.state_dict()
            new_sd = {}
            has_raw = "_raw_tok_emb.weight" in data and "_raw_static_proj.weight" in data and "static_proj.weight" in sd
            for k, v in sd.items():
                src_k = k
                if has_raw and k == "tok_emb.weight":
                    src_k = "_raw_tok_emb.weight"
                elif has_raw and k == "static_proj.weight":
                    src_k = "_raw_static_proj.weight"
                if src_k in data:
                    arr = data[src_k].astype(np.float32)
                    if tuple(arr.shape) == tuple(v.shape):
                        new_sd[k] = torch.from_numpy(arr)
                    else:
                        new_sd[k] = v
                else:
                    new_sd[k] = v
            self.load_state_dict(new_sd)


def _np_gelu(x: np.ndarray) -> np.ndarray:
    return 0.5 * x * (1.0 + np.tanh(0.7978845608 * (x + 0.044715 * (x * x * x))))


def _np_layernorm(x: np.ndarray, w: np.ndarray, b: np.ndarray, eps: float = 1e-5) -> np.ndarray:
    mean = x.mean(axis=-1, keepdims=True)
    var = ((x - mean) ** 2).mean(axis=-1, keepdims=True)
    return (x - mean) / np.sqrt(var + eps) * w + b


def _np_softmax(x: np.ndarray, axis: int = -1) -> np.ndarray:
    xm = x - x.max(axis=axis, keepdims=True)
    ex = np.exp(xm)
    return ex / np.maximum(1e-12, ex.sum(axis=axis, keepdims=True))


class ZeroNumpyPredictor:
    """Ultra-fast pure-NumPy BLAS Set-Transformer inference (~0.15ms per batch of 16 states)."""

    def __init__(self, weights_path: Optional[str] = None) -> None:
        self.featurizer = ZeroFeaturizer()
        self.weights_path = weights_path or DEFAULT_ZERO_NPZ_PATH
        self.w: Dict[str, np.ndarray] = {}
        self.d_model = 128
        self.n_heads = 4
        self.n_layers = 3
        self.is_ready = False
        if self.weights_path and os.path.exists(self.weights_path):
            self._load(self.weights_path)

    def _load(self, path: str) -> None:
        data = np.load(path)
        self.d_model = int(data["_meta_d_model"][0])
        self.n_heads = int(data["_meta_n_heads"][0])
        self.n_layers = int(data["_meta_n_layers"][0])
        self.w = {
            k: data[k].astype(np.float32)
            for k in data.files
            if not k.startswith("_meta_") and not k.startswith("_raw_")
        }
        # Pre-transpose linear weights for fast `x @ W_T + b`
        self._wt: Dict[str, np.ndarray] = {
            k: v.T.copy() for k, v in self.w.items() if k.endswith(".weight") and v.ndim == 2
        }
        self.is_ready = True

    def forward_arrays(
        self,
        vocab: np.ndarray,
        zone: np.ndarray,
        pos: np.ndarray,
        num: np.ndarray,
        mask: np.ndarray,
        glob: np.ndarray,
    ) -> Tuple[np.ndarray, np.ndarray]:
        """Returns `(expected_utility [B], prob_1st [B])` with exact dynamic `T_eff` slicing."""
        util, p1, _ = self.forward_arrays_full(vocab, zone, pos, num, mask, glob)
        return util, p1

    def forward_arrays_full(
        self,
        vocab: np.ndarray,
        zone: np.ndarray,
        pos: np.ndarray,
        num: np.ndarray,
        mask: np.ndarray,
        glob: np.ndarray,
    ) -> Tuple[np.ndarray, np.ndarray, np.ndarray]:
        """Returns `(expected_utility [B], prob_1st [B], pred_fut_pow [B])`."""
        w = self.w
        wt = self._wt
        B, T_full = vocab.shape
        # ZeroFeaturizer packs valid tokens contiguously from 0..tok-1 -> slice to T_eff (2.5-3x speedup!)
        any_valid = mask.any(axis=0)
        valid_cols = np.flatnonzero(any_valid)
        T = int(valid_cols[-1]) + 1 if valid_cols.size > 0 else 1
        if T < T_full:
            vocab = vocab[:, :T]
            zone = zone[:, :T]
            pos = pos[:, :T]
            num = num[:, :T, :]
            mask = mask[:, :T]

        D = self.d_model
        H = self.n_heads
        Dh = D // H
        inv_sqrt_dh = 1.0 / math.sqrt(Dh)

        # Clamp vocab indices defensively
        v_max = w["tok_emb.weight"].shape[0] - 1
        vc = np.clip(vocab, 0, v_max)
        x = (
            w["tok_emb.weight"][vc]
            + w["zone_emb.weight"][zone]
            + w["pos_emb.weight"][pos]
            + (num @ wt["num_proj.weight"] + w["num_proj.bias"])
        )

        key_mask = (~mask)[:, None, None, :]  # (B, 1, 1, T)
        for li in range(self.n_layers):
            pfx = f"blocks.{li}."
            h = _np_layernorm(x, w[pfx + "ln1.weight"], w[pfx + "ln1.bias"])
            qkv = (h @ wt[pfx + "qkv.weight"] + w[pfx + "qkv.bias"]).reshape(B, T, 3, H, Dh)
            q = qkv[:, :, 0].transpose(0, 2, 1, 3)  # (B, H, T, Dh)
            k = qkv[:, :, 1].transpose(0, 2, 1, 3)
            v = qkv[:, :, 2].transpose(0, 2, 1, 3)
            sc = np.matmul(q, k.transpose(0, 1, 3, 2)) * inv_sqrt_dh
            sc = np.where(key_mask, -1e4, sc)
            attn = _np_softmax(sc, axis=-1)
            out = np.matmul(attn, v).transpose(0, 2, 1, 3).reshape(B, T, D)
            x = x + (out @ wt[pfx + "proj.weight"] + w[pfx + "proj.bias"])

            h2 = _np_layernorm(x, w[pfx + "ln2.weight"], w[pfx + "ln2.bias"])
            ff = _np_gelu(h2 @ wt[pfx + "fc1.weight"] + w[pfx + "fc1.bias"])
            x = x + (ff @ wt[pfx + "fc2.weight"] + w[pfx + "fc2.bias"])

        x = _np_layernorm(x, w["tok_ln.weight"], w["tok_ln.bias"])

        # 2-query attention pooling
        pq = w["pool_q"]  # (2, D)
        att_s = np.einsum("qd,btd->bqt", pq, x) / math.sqrt(D)
        att_s = np.where((~mask)[:, None, :], -1e4, att_s)
        att_w = _np_softmax(att_s, axis=-1)
        pooled = np.matmul(att_w, x).reshape(B, 2 * D)

        g = _np_gelu(
            _np_layernorm(
                glob @ wt["glob_fc1.weight"] + w["glob_fc1.bias"],
                w["glob_ln1.weight"],
                w["glob_ln1.bias"],
            )
        )
        g = _np_gelu(g @ wt["glob_fc2.weight"] + w["glob_fc2.bias"])

        cat = np.concatenate([pooled, g], axis=-1)
        h = _np_gelu(
            _np_layernorm(
                cat @ wt["trunk_fc1.weight"] + w["trunk_fc1.bias"],
                w["trunk_ln1.weight"],
                w["trunk_ln1.bias"],
            )
        )
        h = _np_gelu(
            _np_layernorm(
                h @ wt["trunk_fc2.weight"] + w["trunk_fc2.bias"],
                w["trunk_ln2.weight"],
                w["trunk_ln2.bias"],
            )
        )
        logits = h @ wt["place_head.weight"] + w["place_head.bias"]
        probs = _np_softmax(logits, axis=-1)
        util = probs @ ZERO_PLACE_WEIGHTS
        if "growth_head.weight" in wt:
            fp = (h @ wt["growth_head.weight"] + w["growth_head.bias"]).reshape(B)
        else:
            fp = np.zeros(B, dtype=np.float32)
        return util, probs[:, 0], fp

    def predict_batch(
        self,
        obs_list: Sequence[seaglass.TavernObservation],
        ctx: Optional[seaglass.LobbyContext] = None,
        physics_list: Optional[Sequence[Sequence[float]]] = None,
        end_of_turn: bool = True,
    ) -> Tuple[np.ndarray, np.ndarray]:
        v, z, p, n, m, g = self.featurizer.featurize_batch(
            obs_list, ctx=ctx, physics_list=physics_list, end_of_turn=end_of_turn
        )
        return self.forward_arrays(v, z, p, n, m, g)

    def predict_batch_full(
        self,
        obs_list: Sequence[seaglass.TavernObservation],
        ctx: Optional[seaglass.LobbyContext] = None,
        physics_list: Optional[Sequence[Sequence[float]]] = None,
        end_of_turn: bool = True,
    ) -> Tuple[np.ndarray, np.ndarray, np.ndarray]:
        v, z, p, n, m, g = self.featurizer.featurize_batch(
            obs_list, ctx=ctx, physics_list=physics_list, end_of_turn=end_of_turn
        )
        return self.forward_arrays_full(v, z, p, n, m, g)
