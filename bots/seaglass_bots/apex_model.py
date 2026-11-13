"""Apex Set-Transformer Value & Scaling Trajectory Network (`ApexValueNet`) with `.npz` weights."""

from __future__ import annotations

import math
import os
from functools import lru_cache
from pathlib import Path
from typing import Dict, List, Optional, Sequence, Tuple

import numpy as np
import seaglass
import torch
import torch.nn as nn
import torch.nn.functional as F

from .apex_knowledge import ApexKnowledge, get_apex_knowledge

APEX_MAX_TOKENS = 22
APEX_TOKEN_NUM_DIM = 24
APEX_GLOBAL_HEADER_DIM = 30

# Steep 1st-place tournament utility weights: prioritizes 1st-place finishes over passive Top-4.
APEX_PLACEMENT_WEIGHTS = np.array(
    [36.0, 17.5, 6.5, 0.5, -5.5, -12.5, -20.0, -28.0], dtype=np.float32
)

DEFAULT_APEX_NPZ_PATH = (
    Path(__file__).resolve().parent / "data" / "apex_value_v1.npz"
)


def _signed_log(x: float, scale: float = 3.0) -> float:
    if x == 0:
        return 0.0
    v = math.log1p(abs(float(x))) / scale
    return v if x > 0 else -v


class ApexStateFeaturizer:
    """Featurizes Solo & Duos `TavernObservation` + `LobbyContext` into Set-Transformer tensors."""

    def __init__(self, catalog_name: str = "full") -> None:
        self.kb: ApexKnowledge = get_apex_knowledge(catalog_name)
        self.meta = self.kb.meta
        self.counter_dim: int = len(self.meta.counter_names)
        self.global_dim: int = APEX_GLOBAL_HEADER_DIM + self.counter_dim

    def _fill_unit_num(
        self, u: seaglass.Unit, dst: np.ndarray, is_leftmost_hand: bool = False
    ) -> None:
        atk = max(0, u.attack)
        hp = max(0, u.health)
        dst[0] = math.log1p(atk) / 5.5
        dst[1] = math.log1p(hp) / 5.5
        dst[2] = min(5.0, atk / 60.0)
        dst[3] = min(5.0, hp / 60.0)
        dst[4] = u.tavern_tier / 7.0
        dst[5] = 1.0 if u.is_golden else 0.0
        dst[6] = 1.0 if u.is_spell else 0.0
        dst[7] = 1.0 if u.taunt else 0.0
        dst[8] = 1.0 if u.divine_shield else 0.0
        dst[9] = 1.0 if u.windfury else 0.0
        dst[10] = 1.0 if u.reborn else 0.0
        dst[11] = 1.0 if u.venomous else 0.0
        dst[12] = 1.0 if u.stealth else 0.0
        dst[13] = 1.0 if u.magnetic else 0.0
        dst[14] = 1.0 if getattr(u, "unplayable", False) else 0.0
        dst[15] = float(getattr(u, "spell_cost", 0)) / 4.0
        dst[16] = math.log1p(max(0, u.blood_gems_played)) / 3.0
        dst[17] = math.log1p(max(0, u.end_of_turn_stats)) / 2.5
        dst[18] = min(3.0, float(u.extra_deathrattles)) / 2.0
        dg_val = sum(self.kb.dark_gift_weights.get(dg, 4.0) for dg in u.dark_gifts)
        dst[19] = min(3.0, dg_val / 15.0)
        dst[20] = self.kb.engine_bonus_by_id.get(u.card_id, 0.0) / 20.0
        dst[21] = _signed_log(u.stacks, 3.0)
        dst[22] = _signed_log(u.counter, 3.0)
        dst[23] = 1.0 if is_leftmost_hand else 0.0

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
        out_vocab.fill(0)
        out_slot.fill(0)
        out_pos.fill(0)
        out_num.fill(0.0)
        out_mask.fill(False)
        out_global.fill(0.0)

        # Token 0: [GLOBAL]
        out_slot[0] = 1
        out_mask[0] = True
        cursor = 1

        # 1. Board minions (up to 7, exact positions 1..7)
        for b_idx, u in enumerate(obs.board[:7]):
            out_vocab[cursor] = self.meta.vocab_index(u.card_id)
            out_slot[cursor] = 2
            out_pos[cursor] = b_idx + 1
            self._fill_unit_num(u, out_num[cursor], False)
            out_mask[cursor] = True
            cursor += 1

        # 2. Hand cards (up to 8, preserving slot 0 left-most indicator)
        hand_list = list(enumerate(obs.hand))
        if len(hand_list) > 8:
            # Always keep index 0 (left-most hand card), plus highest priority cards
            rest = sorted(
                hand_list[1:],
                key=lambda ic: (
                    ic[1].is_golden,
                    not ic[1].is_spell,
                    ic[1].tavern_tier,
                    ic[1].attack + ic[1].health,
                ),
                reverse=True,
            )[:7]
            hand_list = [hand_list[0]] + rest

        for h_orig_idx, c in hand_list:
            if cursor >= 16:
                break
            out_vocab[cursor] = self.meta.vocab_index(c.card_id)
            out_slot[cursor] = 3
            out_pos[cursor] = min(8, h_orig_idx + 1)
            self._fill_unit_num(c, out_num[cursor], is_leftmost_hand=(h_orig_idx == 0))
            out_mask[cursor] = True
            cursor += 1

        # 3. Owned Trinkets (up to 2)
        trinket_ids = obs.trinkets
        trinket_ctrs = obs.trinket_counters
        trinket_trs = obs.trinket_tribes
        for t_idx in range(min(2, len(trinket_ids))):
            if cursor >= 18:
                break
            tid = trinket_ids[t_idx]
            out_vocab[cursor] = self.meta.vocab_index(tid)
            out_slot[cursor] = 4
            out_pos[cursor] = t_idx + 1
            ctr_val = trinket_ctrs[t_idx] if t_idx < len(trinket_ctrs) else 0
            tr_val = trinket_trs[t_idx] if t_idx < len(trinket_trs) else 0
            out_num[cursor, 21] = _signed_log(ctr_val, 3.0)
            out_num[cursor, 22] = float(tr_val) / 10.0
            out_mask[cursor] = True
            cursor += 1

        # 4. Primary & Secondary Hero Powers (up to 2)
        hp1 = obs.hero_power_id if obs.hero_power_id != 0 else obs.hero_id
        if hp1 != 0 and cursor < 20:
            out_vocab[cursor] = self.meta.vocab_index(hp1)
            out_slot[cursor] = 5
            out_pos[cursor] = 1
            out_num[cursor, 4] = float(obs.hero_power_cost) / 4.0
            out_num[cursor, 5] = float(obs.hero_power_uses_left > 0)
            out_num[cursor, 6] = float(obs.hero_power_armed > 0)
            out_num[cursor, 21] = _signed_log(obs.hero_power_stacks, 3.0)
            out_num[cursor, 22] = _signed_log(obs.hero_power_counter, 3.0)
            out_mask[cursor] = True
            cursor += 1

        hp2 = getattr(obs, "second_hero_power_id", 0)
        if hp2 != 0 and cursor < 20:
            out_vocab[cursor] = self.meta.vocab_index(hp2)
            out_slot[cursor] = 5
            out_pos[cursor] = 2
            out_num[cursor, 4] = float(getattr(obs, "second_hero_power_cost", 0)) / 4.0
            out_num[cursor, 5] = float(getattr(obs, "second_hero_power_uses_left", 0) > 0)
            out_num[cursor, 21] = _signed_log(getattr(obs, "second_hero_power_stacks", 0), 3.0)
            out_num[cursor, 22] = _signed_log(getattr(obs, "second_hero_power_counter", 0), 3.0)
            out_mask[cursor] = True
            cursor += 1

        # 5. Top Shop Highlights (when mid-turn or frozen, up to 2 tokens)
        if (not end_of_turn or obs.is_frozen) and obs.shop and cursor < APEX_MAX_TOKENS:
            shop_sorted = sorted(
                obs.shop,
                key=lambda s: (
                    self.kb.engine_bonus_by_id.get(s.card_id, 0.0),
                    s.tavern_tier,
                    s.attack + s.health,
                ),
                reverse=True,
            )[:2]
            for s_card in shop_sorted:
                if cursor >= APEX_MAX_TOKENS:
                    break
                out_vocab[cursor] = self.meta.vocab_index(s_card.card_id)
                out_slot[cursor] = 6
                out_pos[cursor] = 0
                self._fill_unit_num(s_card, out_num[cursor], False)
                out_mask[cursor] = True
                cursor += 1

        # 6. Global vector (Solo + Duos unified)
        is_duos = hasattr(obs, "team_health")
        if is_duos:
            hp_cur = float(obs.team_health)
            arm_cur = float(obs.team_armor)
        else:
            hp_cur = float(obs.health)
            arm_cur = float(obs.armor)
        hp_total = hp_cur + arm_cur

        alive_units = 4 if is_duos else 8
        median_hp = 35.0 if is_duos else 30.0
        max_opp_tier = float(obs.tavern_tier)
        dmg_cap_norm = 0.5
        if ctx is not None:
            alive_units = ctx.alive_units
            opp_hps = sorted(o.health + o.armor for o in ctx.opponents if o.alive)
            if opp_hps:
                median_hp = float(opp_hps[len(opp_hps) // 2])
                max_opp_tier = float(max(o.tier for o in ctx.opponents if o.alive))
            dmg_cap_norm = float(ctx.damage_cap) / 30.0 if ctx.damage_cap is not None else 1.5

        out_global[0] = min(1.8, obs.turn / 16.0)
        out_global[1] = max(0.0, 16.0 - obs.turn) / 15.0
        out_global[2] = hp_total / 45.0
        out_global[3] = hp_cur / 35.0
        out_global[4] = arm_cur / 20.0
        tier_clamped = max(1, min(7, obs.tavern_tier))
        out_global[5 + tier_clamped - 1] = 1.0  # slots 5..11 (7 tiers!)
        out_global[12] = obs.upgrade_cost / 12.0
        out_global[13] = obs.gold / 14.0
        out_global[14] = obs.max_gold / 14.0
        out_global[15] = obs.bonus_gold_next_turn / 6.0
        out_global[16] = obs.free_refreshes / 4.0
        out_global[17] = 1.0 if obs.is_frozen else 0.0
        out_global[18] = 1.0 if end_of_turn else 0.0
        out_global[19] = alive_units / (4.0 if is_duos else 8.0)
        out_global[20] = (hp_total - median_hp) / 30.0
        out_global[21] = (obs.tavern_tier - max_opp_tier) / 5.0
        out_global[22] = dmg_cap_norm
        out_global[23] = float(getattr(obs, "dark_discoveries_left", 0))

        cc_sum = 0.0
        for _, v1, v2 in obs.card_counters:
            cc_sum += abs(_signed_log(v1, 3.0)) + abs(_signed_log(v2, 3.0))
        out_global[24] = min(4.0, cc_sum / 3.0)

        deity_stats = (
            max(0, obs.deity_attack - 1) + max(0, obs.deity_health - 1)
            if obs.deity != "none"
            else 0
        )
        out_global[25] = math.log1p(deity_stats) / 5.0
        out_global[26] = 1.0 if is_duos else 0.0
        out_global[27] = (
            float(getattr(obs, "teammate_tavern_tier", 0)) / 7.0 if is_duos else 0.0
        )
        out_global[28] = float(getattr(obs, "free_passes", 0)) / 2.0 if is_duos else 0.0
        board_atk_hp = sum(max(0, u.attack) + max(0, u.health) for u in obs.board)
        out_global[29] = math.log1p(board_atk_hp) / 7.5

        p_ctrs = obs.player_counters
        limit = min(self.counter_dim, len(p_ctrs))
        for i in range(limit):
            val = p_ctrs[i]
            if val != 0:
                out_global[APEX_GLOBAL_HEADER_DIM + i] = _signed_log(val, 3.0)

    def featurize_batch(
        self,
        obs_list: Sequence[seaglass.TavernObservation],
        ctx: Optional[seaglass.LobbyContext],
        end_of_turn: bool,
    ) -> Tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray, np.ndarray]:
        b = len(obs_list)
        vocab = np.zeros((b, APEX_MAX_TOKENS), dtype=np.int64)
        slot = np.zeros((b, APEX_MAX_TOKENS), dtype=np.int64)
        pos = np.zeros((b, APEX_MAX_TOKENS), dtype=np.int64)
        num = np.zeros((b, APEX_MAX_TOKENS, APEX_TOKEN_NUM_DIM), dtype=np.float32)
        mask = np.zeros((b, APEX_MAX_TOKENS), dtype=np.bool_)
        glob = np.zeros((b, self.global_dim), dtype=np.float32)
        for i, obs in enumerate(obs_list):
            self.featurize_into(
                obs, ctx, end_of_turn, vocab[i], slot[i], pos[i], num[i], mask[i], glob[i]
            )
        return vocab, slot, pos, num, mask, glob


class ApexAttentionBlock(nn.Module):
    """Pre-Norm Multi-Head Self-Attention block."""

    def __init__(self, d_model: int = 96, n_heads: int = 4, ffn_dim: int = 192) -> None:
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
        bsz, seqlen, _ = x.shape
        h = self.norm1(x)
        qkv = self.qkv(h).view(bsz, seqlen, 3, self.n_heads, self.head_dim)
        q, k, v = qkv.unbind(dim=2)
        q = q.transpose(1, 2)
        k = k.transpose(1, 2)
        v = v.transpose(1, 2)

        scores = torch.matmul(q, k.transpose(-2, -1)) * self.scale
        scores = scores.masked_fill(~mask[:, None, None, :], -1e9)
        attn = torch.softmax(scores, dim=-1)
        ctx = (
            torch.matmul(attn, v)
            .transpose(1, 2)
            .contiguous()
            .view(bsz, seqlen, self.d_model)
        )
        x = x + self.out_proj(ctx)
        x = x + self.ffn(self.norm2(x))
        return x * mask.unsqueeze(-1).to(x.dtype)


class ApexValueNet(nn.Module):
    """
    Dual-head Set-Transformer:
      1. `place_head`: 8-way final tournament placement distribution,
      2. `growth_head`: expected future `log1p(final_power) - log1p(cur_power)` trajectory.
    """

    def __init__(
        self,
        vocab_size: int,
        tag_matrix: np.ndarray,
        global_dim: int,
        d_model: int = 96,
        n_heads: int = 4,
        n_layers: int = 3,
    ) -> None:
        super().__init__()
        self.vocab_size = vocab_size
        self.global_dim = global_dim
        self.d_model = d_model
        self.n_heads = n_heads
        self.n_layers = n_layers

        tag_dim = int(tag_matrix.shape[1])
        self.register_buffer(
            "tag_table", torch.tensor(tag_matrix, dtype=torch.float32), persistent=False
        )
        self.register_buffer(
            "place_weights",
            torch.tensor(APEX_PLACEMENT_WEIGHTS, dtype=torch.float32),
            persistent=False,
        )

        self.card_emb = nn.Embedding(vocab_size, d_model, padding_idx=0)
        self.tag_proj = nn.Linear(tag_dim, d_model, bias=False)
        self.slot_emb = nn.Embedding(7, d_model, padding_idx=0)
        self.pos_emb = nn.Embedding(9, d_model, padding_idx=0)
        self.num_proj = nn.Linear(APEX_TOKEN_NUM_DIM, d_model)
        self.global_proj = nn.Sequential(
            nn.Linear(global_dim, d_model),
            nn.GELU(),
            nn.Linear(d_model, d_model),
        )
        self.in_norm = nn.LayerNorm(d_model)

        self.blocks = nn.ModuleList(
            [
                ApexAttentionBlock(d_model=d_model, n_heads=n_heads, ffn_dim=2 * d_model)
                for _ in range(n_layers)
            ]
        )

        readout_in = 3 * d_model + global_dim
        self.trunk = nn.Sequential(
            nn.Linear(readout_in, 160),
            nn.LayerNorm(160),
            nn.GELU(),
            nn.Linear(160, 96),
            nn.GELU(),
        )
        self.place_head = nn.Linear(96, 8)
        self.power_head = nn.Linear(96, 1)
        self.growth_head = nn.Linear(96, 1)

    def forward(
        self,
        vocab_ids: torch.Tensor,
        slot_ids: torch.Tensor,
        pos_ids: torch.Tensor,
        num_feats: torch.Tensor,
        mask: torch.Tensor,
        global_feats: torch.Tensor,
    ) -> Tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
        tags = self.tag_table[vocab_ids]
        tok = (
            self.card_emb(vocab_ids)
            + self.tag_proj(tags)
            + self.slot_emb(slot_ids)
            + self.pos_emb(pos_ids)
            + self.num_proj(num_feats)
        )
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
        growth_pred = self.growth_head(h).squeeze(-1)
        return place_logits, power_pred, growth_pred

    def expected_utility(
        self,
        vocab_ids: torch.Tensor,
        slot_ids: torch.Tensor,
        pos_ids: torch.Tensor,
        num_feats: torch.Tensor,
        mask: torch.Tensor,
        global_feats: torch.Tensor,
    ) -> torch.Tensor:
        logits, _, growth = self.forward(
            vocab_ids, slot_ids, pos_ids, num_feats, mask, global_feats
        )
        probs = F.softmax(logits, dim=-1)
        place_u = torch.matmul(probs, self.place_weights)
        return place_u + 1.8 * growth

    def save_npz(self, path: os.PathLike | str, use_fp16: bool = True) -> None:
        """Save portable `.npz` weights (tracked cleanly by git, zero pickle usage)."""
        out_path = Path(path)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        arrays: Dict[str, np.ndarray] = {
            "__meta__": np.array(
                [self.vocab_size, self.global_dim, self.d_model, self.n_heads, self.n_layers],
                dtype=np.int64,
            )
        }
        for k, v in self.state_dict().items():
            arr = v.detach().cpu().numpy()
            if use_fp16 and arr.dtype == np.float32:
                arr = arr.astype(np.float16)
            arrays[k] = arr
        np.savez_compressed(str(out_path), **arrays)

    @classmethod
    def load_npz(
        cls,
        path: os.PathLike | str,
        tag_matrix: np.ndarray,
    ) -> "ApexValueNet":
        with np.load(str(path), allow_pickle=False) as data:
            meta_arr = data["__meta__"]
            vocab_size, global_dim, d_model, n_heads, n_layers = [int(x) for x in meta_arr]
            net = cls(
                vocab_size=vocab_size,
                tag_matrix=tag_matrix,
                global_dim=global_dim,
                d_model=d_model,
                n_heads=n_heads,
                n_layers=n_layers,
            )
            state: Dict[str, torch.Tensor] = {}
            for k in net.state_dict().keys():
                if k in data:
                    arr = data[k]
                    if arr.dtype == np.float16:
                        arr = arr.astype(np.float32)
                    state[k] = torch.from_numpy(arr)
            net.load_state_dict(state, strict=True)
            net.eval()
            return net


class ApexValuePredictor:
    """Fast CPU inference wrapper around `ApexValueNet`."""

    def __init__(
        self,
        catalog_name: str = "full",
        weights_path: Optional[os.PathLike | str] = None,
    ) -> None:
        torch.set_num_threads(1)
        self.catalog_name = catalog_name
        # Always featurize against `"full"` vocab so weights apply identically across Solo & Duos!
        self.featurizer = ApexStateFeaturizer("full")
        self.meta = self.featurizer.meta
        self.model: Optional[ApexValueNet] = None
        self.loaded_path: Optional[str] = None

        ckpt = Path(weights_path) if weights_path is not None else DEFAULT_APEX_NPZ_PATH
        if ckpt.is_file():
            self.load(ckpt)

    @property
    def is_ready(self) -> bool:
        return self.model is not None

    def load(self, ckpt_path: os.PathLike | str) -> bool:
        tag_mat = np.asarray(self.meta.tag_matrix, dtype=np.float32)
        net = ApexValueNet.load_npz(ckpt_path, tag_matrix=tag_mat)
        if net.vocab_size != self.meta.vocab_size or net.global_dim != self.featurizer.global_dim:
            return False
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


@lru_cache(maxsize=4)
def get_apex_predictor(
    catalog_name: str = "full",
    weights_path: Optional[str] = None,
) -> ApexValuePredictor:
    return ApexValuePredictor(catalog_name=catalog_name, weights_path=weights_path)
