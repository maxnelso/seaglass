"""Device tests for the bots trainers: every trainable network and its loss must run forward, backward and
one optimizer step on the CPU and on the accelerator (CUDA or Apple MPS, whichever this machine has).

MPS has no float64 and fewer ops than CUDA, so these tests catch float64 tensors and unsupported ops early.
"""

from __future__ import annotations

import importlib
import sys
from pathlib import Path

import numpy as np
import pytest
import torch
import torch.nn.functional as F

from seaglass_rl.device import mps_available

ACCEL = "cuda" if torch.cuda.is_available() else ("mps" if mps_available() else None)

DEVICES = [
    "cpu",
    pytest.param(ACCEL, marks=pytest.mark.skipif(ACCEL is None, reason="no CUDA or MPS device")),
]

B = 6  # tiny batch


def _check_step(net: torch.nn.Module, dev: torch.device, loss_fn) -> None:
    """Loss finite -> backward -> one AdamW step -> parameters still float32 on `dev`, loss still finite."""
    net.to(dev).train()
    opt = torch.optim.AdamW(net.parameters(), lr=1e-3)
    loss = loss_fn()
    assert loss.dtype == torch.float32
    assert torch.isfinite(loss).item()
    opt.zero_grad(set_to_none=True)
    loss.backward()
    torch.nn.utils.clip_grad_norm_(net.parameters(), 1.0)
    opt.step()
    for p in net.parameters():
        assert p.device.type == dev.type
        assert p.dtype == torch.float32
        assert torch.isfinite(p).all().item()
    assert torch.isfinite(loss_fn()).item()


def _set_inputs(dev, vocab_size: int, max_tokens: int, num_dim: int, glob_dim: int, n_slot: int, n_pos: int):
    g = torch.Generator().manual_seed(0)
    vocab = torch.randint(1, vocab_size, (B, max_tokens), generator=g)
    slot = torch.randint(1, n_slot, (B, max_tokens), generator=g)
    pos = torch.randint(1, n_pos, (B, max_tokens), generator=g)
    num = torch.randn(B, max_tokens, num_dim, generator=g)
    mask = torch.ones(B, max_tokens, dtype=torch.bool)
    mask[:, max_tokens - 3 :] = False
    glob = torch.randn(B, glob_dim, generator=g)
    place = torch.randint(0, 8, (B,), generator=g)
    return [t.to(dev) for t in (vocab, slot, pos, num, mask, glob, place)]


# --------------------------------------------------------------------------- set transformers (value nets)


@pytest.mark.parametrize("device", DEVICES)
def test_synergy_value_net(device: str) -> None:
    from seaglass_bots.value_model import MAX_TOKENS, TOKEN_NUM_DIM, StateFeaturizer, SynergyValueNet

    dev = torch.device(device)
    fz = StateFeaturizer("full")
    tags = np.asarray(fz.meta.tag_matrix, dtype=np.float32)
    net = SynergyValueNet(fz.meta.vocab_size, tags, fz.global_dim, d_model=32, n_heads=4, n_layers=1)
    vocab, slot, pos, num, mask, glob, place = _set_inputs(
        dev, fz.meta.vocab_size, MAX_TOKENS, TOKEN_NUM_DIM, fz.global_dim, 6, 8
    )
    power = torch.randn(B, device=dev)
    w = torch.rand(B, device=dev) + 0.5

    def loss_fn() -> torch.Tensor:  # same terms as `train_synergy_net`
        logits, power_pred = net(vocab, slot, pos, num, mask, glob)
        ce = F.cross_entropy(logits, place, reduction="none", label_smoothing=0.03)
        mse = F.smooth_l1_loss(power_pred, power, reduction="none")
        return ((ce + 0.15 * mse) * w).mean()

    _check_step(net, dev, loss_fn)


@pytest.mark.parametrize("device", DEVICES)
def test_apex_value_net(device: str) -> None:
    from seaglass_bots.apex_model import APEX_MAX_TOKENS, APEX_TOKEN_NUM_DIM, ApexStateFeaturizer, ApexValueNet

    dev = torch.device(device)
    fz = ApexStateFeaturizer("full")
    tags = np.asarray(fz.meta.tag_matrix, dtype=np.float32)
    net = ApexValueNet(fz.meta.vocab_size, tags, fz.global_dim, d_model=32, n_heads=4, n_layers=1)
    vocab, slot, pos, num, mask, glob, place = _set_inputs(
        dev, fz.meta.vocab_size, APEX_MAX_TOKENS, APEX_TOKEN_NUM_DIM, fz.global_dim, 7, 9
    )
    y_lp = torch.randn(B, device=dev)
    y_gr = torch.randn(B, device=dev)
    w = torch.rand(B, device=dev) + 0.5

    def loss_fn() -> torch.Tensor:  # same terms as `train_apex_value_net`
        logits, p_pred, g_pred = net(vocab, slot, pos, num, mask, glob)
        ce = F.cross_entropy(logits, place, reduction="none", label_smoothing=0.03)
        lp = F.smooth_l1_loss(p_pred, y_lp, reduction="none")
        gr = F.smooth_l1_loss(g_pred, y_gr, reduction="none")
        return (w * (ce + 0.15 * lp + 0.35 * gr)).mean()

    _check_step(net, dev, loss_fn)


@pytest.mark.parametrize("device", DEVICES)
def test_sovereign_value_net(device: str) -> None:
    from seaglass_bots.sovereign_model import SOV_MAX_TOKENS, SOV_TOKEN_NUM_DIM, SovereignStateFeaturizer, SovereignValueNet
    from seaglass_bots.train_sovereign import _augment_train_inputs, _build_ordinal_soft_targets

    dev = torch.device(device)
    fz = SovereignStateFeaturizer("full")
    tags = np.asarray(fz.meta.tag_matrix, dtype=np.float32)
    net = SovereignValueNet(fz.meta.vocab_size, tags, fz.global_dim, d_model=32, n_heads=4, n_layers=1)
    vocab, slot, pos, num, mask, glob, place = _set_inputs(
        dev, fz.meta.vocab_size, SOV_MAX_TOKENS, SOV_TOKEN_NUM_DIM, fz.global_dim, 7, 9
    )
    soft_np = _build_ordinal_soft_targets(place.cpu().numpy())
    assert soft_np.dtype == np.float32
    soft = torch.from_numpy(soft_np).to(dev)
    growth = torch.randn(B, device=dev)
    margin = torch.rand(B // 2, device=dev)

    def loss_fn() -> torch.Tensor:  # CE + growth Huber + pairwise ranking, as in `train_model`
        v_in, g_in = _augment_train_inputs(vocab, glob)
        logits, g_pred = net(v_in, slot, pos, num, mask, g_in)
        ce = -(soft * F.log_softmax(logits, dim=-1)).sum(dim=-1).mean()
        gr = F.huber_loss(g_pred, growth, delta=1.2)
        u = net.expected_utility(v_in, slot, pos, num, mask, g_in)
        rk = F.relu(margin - (u[: B // 2] - u[B // 2 :])).mean()
        return ce + 0.28 * rk + 0.25 * gr

    _check_step(net, dev, loss_fn)


@pytest.mark.parametrize("device", DEVICES)
def test_zero_value_net(device: str) -> None:
    from seaglass_bots.architect.train_zero import _build_ordinal_targets
    from seaglass_bots.architect.zero_model import ZERO_GLOBAL_DIM, ZERO_MAX_TOKENS, ZERO_TOKEN_NUM_DIM, ZeroValueNet

    dev = torch.device(device)
    net = ZeroValueNet(d_model=32, n_heads=4, n_layers=1, d_ff=64, dropout=0.1)
    g = torch.Generator().manual_seed(0)
    vocab = torch.randint(1, net.vocab_size, (B, ZERO_MAX_TOKENS), generator=g).to(dev)
    zone = torch.randint(0, 6, (B, ZERO_MAX_TOKENS), generator=g).to(dev)
    pos = torch.randint(0, 12, (B, ZERO_MAX_TOKENS), generator=g).to(dev)
    num = torch.randn(B, ZERO_MAX_TOKENS, ZERO_TOKEN_NUM_DIM, generator=g).to(dev)
    mask = torch.ones(B, ZERO_MAX_TOKENS, dtype=torch.bool, device=dev)
    mask[:, -4:] = False
    glob = torch.randn(B, ZERO_GLOBAL_DIM, generator=g).to(dev)
    place = torch.randint(0, 8, (B,), generator=g).to(dev)
    fut = torch.randn(B, device=dev)
    sw = torch.rand(B, device=dev) + 0.5

    def loss_fn() -> torch.Tensor:  # same terms as `train_zero_net`
        logits, fp = net(vocab, zone, pos, num, mask, glob)
        soft = _build_ordinal_targets(place, sigma=0.80)
        ce = (-(soft * F.log_softmax(logits, dim=-1)).sum(dim=-1) * sw).mean()
        p1 = F.softmax(logits, dim=-1)[:, 0].clamp(1e-5, 1.0 - 1e-5)
        b1 = (F.binary_cross_entropy(p1, (place == 0).float(), reduction="none") * sw).mean()
        fl = (F.smooth_l1_loss(fp, fut, reduction="none") * sw).mean()
        u = net.expected_utility(vocab, zone, pos, num, mask, glob)
        return ce + 0.75 * b1 + 0.35 * fl + 0.01 * u.mean()

    _check_step(net, dev, loss_fn)


# --------------------------------------------------------------------------- entity transformers (policy nets)


def _entity_inputs(dev):
    from seaglass_rl.actions import ACTION_SPACE_SIZE, END_TURN_INDEX
    from seaglass_rl.features import NUM_SLOTS, PLAYER_DIM, SLOT_NUM_DIM, VOCAB_SIZE

    g = torch.Generator().manual_seed(0)
    pv = torch.randn(B, PLAYER_DIM, generator=g)
    ci = torch.randint(1, VOCAB_SIZE, (B, NUM_SLOTS), generator=g)  # every slot occupied
    nf = torch.randn(B, NUM_SLOTS, SLOT_NUM_DIM, generator=g).abs() + 0.1
    mk = torch.zeros(B, ACTION_SPACE_SIZE, dtype=torch.bool)
    mk[:, END_TURN_INDEX] = True
    mk[:, 0] = True
    acts = torch.where(torch.arange(B) % 2 == 0, torch.tensor(END_TURN_INDEX), torch.tensor(0))
    return [t.to(dev) for t in (pv, ci, nf, mk, acts)]


def _small_entity_kwargs() -> dict:
    return dict(card_embed_dim=16, d_model=32, layers=1, heads=4, ff=64, head_dim=32, kind_prior=True, static_features=True, seed=0)


def _ppo_batch(dev) -> dict:
    pv, ci, nf, mk, acts = _entity_inputs("cpu")
    return {
        "player_vec": pv,
        "slot_card_ids": ci,
        "slot_num_feats": nf,
        "action_mask": mk,
        "actions": acts,
        "old_log_probs": torch.full((B,), -1.0),
        "old_values": torch.zeros(B),
        "advantages": torch.randn(B),
        "returns": torch.randn(B),
    }


@pytest.mark.parametrize("device", DEVICES)
def test_entity_transformer_ppo_update(device: str) -> None:
    """`train_policy.ppo_update_cuda` on a tiny batch (the update used by policy PPO and anti-sovereign PPO)."""
    from seaglass_bots.architect.train_policy import ppo_update_cuda
    from seaglass_rl.transformer import EntityTransformer

    dev = torch.device(device)
    model = EntityTransformer(**_small_entity_kwargs()).to(dev)
    ref = EntityTransformer(**_small_entity_kwargs()).to(dev).eval()
    opt = torch.optim.AdamW(model.parameters(), lr=1e-4)
    m = ppo_update_cuda(model, opt, _ppo_batch(dev), dev=dev, epochs=1, minibatch_size=4, ref_model=ref, ref_kl_coef=0.01)
    assert all(np.isfinite(v) for v in m.values())
    assert all(p.device.type == dev.type for p in model.parameters())


@pytest.mark.parametrize("device", DEVICES)
def test_entity_transformer_awbc_loss(device: str) -> None:
    from seaglass_rl.transformer import EntityTransformer

    dev = torch.device(device)
    model = EntityTransformer(**_small_entity_kwargs())
    pv, ci, nf, mk, ac = _entity_inputs(dev)
    ret = torch.randn(B, device=dev)
    w = torch.rand(B, device=dev) + 0.5

    def loss_fn() -> torch.Tensor:  # AWBC loss of `train_awbc_cuda`
        logits, val = model(pv, ci, nf, mk)
        ce = F.cross_entropy(logits, ac, reduction="none")
        return (ce * w).sum() / w.sum().clamp(min=1.0) + 0.5 * F.mse_loss(val, ret)

    _check_step(model, dev, loss_fn)


@pytest.mark.parametrize("device", DEVICES)
def test_dist_entity_transformer_ppo_update(device: str) -> None:
    """`train_dist_ppo.dist_ppo_update_cuda` (also used by the fast vectorized trainer), incl. critic-only warmup."""
    from seaglass_bots.architect.dist_transformer import DistEntityTransformer
    from seaglass_bots.architect.train_dist_ppo import dist_ppo_update_cuda

    dev = torch.device(device)
    model = DistEntityTransformer(**_small_entity_kwargs()).to(dev)
    ref = DistEntityTransformer(**_small_entity_kwargs()).to(dev).eval()
    opt = torch.optim.AdamW(model.parameters(), lr=1e-4)
    place_idx = np.arange(B, dtype=np.int64) % 8
    for critic_only in (True, False):
        m = dist_ppo_update_cuda(
            model, opt, _ppo_batch(dev), place_idx, dev=dev, epochs=1, minibatch_size=32,
            ref_model=ref, ref_kl_coef=0.01, critic_only=critic_only,
        )
        assert all(np.isfinite(v) for v in m.values())
    assert all(p.device.type == dev.type for p in model.parameters())


@pytest.mark.parametrize("device", DEVICES)
def test_dist_entity_transformer_gm_loss(device: str) -> None:
    """The distributional critic and aux head of `train_gm_dist` (behavioural cloning, FP32 loss)."""
    from seaglass_bots.architect.dist_transformer import DistEntityTransformer
    from seaglass_bots.architect.train_gm_dist import _ordinal_target_8

    dev = torch.device(device)
    model = DistEntityTransformer(**_small_entity_kwargs())
    pv, ci, nf, mk, ac = _entity_inputs(dev)
    place = torch.arange(B, device=dev) % 8

    def loss_fn() -> torch.Tensor:
        logits, _val, pl_logits = model.forward_with_dist(pv, ci, nf, mk)
        pol = F.cross_entropy(logits, ac)
        v_ce = -(_ordinal_target_8(place, eps=0.08) * F.log_softmax(pl_logits, dim=-1)).sum(dim=-1).mean()
        return pol + 0.05 * v_ce

    _check_step(model, dev, loss_fn)


@pytest.mark.parametrize("device", DEVICES)
def test_amber_net_gumbel_step(device: str) -> None:
    """`train_gumbel_az.train_gumbel_step` and `eval_unseen_samples` on a synthetic replay of 64 samples."""
    from seaglass_bots.architect.gumbel_az import AmberNet
    from seaglass_bots.architect.train_gumbel_az import eval_unseen_samples, train_gumbel_step

    dev = torch.device(device)
    model = AmberNet(**_small_entity_kwargs()).to(dev)
    pv, ci, nf, mk, ac = (t.cpu() for t in _entity_inputs("cpu"))
    replay = []
    for i in range(64):
        j = i % B
        pi = np.zeros(mk.shape[1], dtype=np.float16)
        pi[int(ac[j])] = 1.0
        replay.append(
            {
                "pv": pv[j].numpy().astype(np.float32),
                "ci": ci[j].numpy().astype(np.int32),
                "nf": nf[j].numpy().astype(np.float16),
                "mk": mk[j].numpy(),
                "pi": pi,
                "v_search": 0.25,
                "place_idx": i % 8,
                "aux": np.array([0.5, 0.5, 1.0], dtype=np.float32),
            }
        )
    opt = torch.optim.AdamW(model.parameters(), lr=1e-4)
    m = train_gumbel_step(model, opt, replay, dev=dev, steps=2, batch_size=16, rng=np.random.default_rng(0))
    assert all(np.isfinite(v) for v in m.values())
    ev = eval_unseen_samples(model, replay, dev)
    assert all(np.isfinite(v) for v in ev.values())
    assert all(p.device.type == dev.type for p in model.parameters())


@pytest.mark.parametrize("device", DEVICES)
def test_scaling_value_transformer(device: str) -> None:
    """The multi-horizon loss shared by `scaling_value`, `train_ranker_value` and `train_dagger_beam`."""
    from seaglass_bots.architect.scaling_value import ScalingValueTransformer, _ordinal_target

    dev = torch.device(device)
    model = ScalingValueTransformer(d_model=32, layers=1, heads=4, ff=64, dropout=0.1)
    pv, ci, nf, _mk, _ac = _entity_inputs(dev)
    pl = torch.arange(B, device=dev) % 8
    fs = torch.randn(B, device=dev)
    fs[0] = float("nan")  # the trainers mask non-finite future-stats labels

    def loss_fn() -> torch.Tensor:
        ev, pl_lg, pred_fs, p1_lg = model(pv, ci, nf)
        t_ord = _ordinal_target(pl, eps=0.10)
        v_ce = -(t_ord * F.log_softmax(pl_lg, dim=-1)).sum(dim=-1).mean()
        bce1 = F.binary_cross_entropy_with_logits(p1_lg, (pl == 0).float())
        ok = torch.isfinite(fs)
        fs_loss = F.smooth_l1_loss(pred_fs[ok], fs[ok])
        sc = ev + 0.50 * torch.sigmoid(p1_lg) + 0.15 * pred_fs  # ranker score
        bt = F.softplus(-1.8 * (sc[: B // 2] - sc[B // 2 :])).mean()
        return v_ce + 0.40 * bce1 + 0.35 * fs_loss + 0.1 * bt

    _check_step(model, dev, loss_fn)


# --------------------------------------------------------------------------- tiny end-to-end trainers


def _seq_game_ids(n_games: int, per_game: int) -> np.ndarray:
    return np.repeat(np.arange(n_games, dtype=np.int64), per_game)


@pytest.mark.parametrize("device", DEVICES)
def test_train_synergy_net_end_to_end(device: str, tmp_path: Path) -> None:
    from seaglass_bots.train_value import train_synergy_net
    from seaglass_bots.value_model import MAX_TOKENS, TOKEN_NUM_DIM, StateFeaturizer

    fz = StateFeaturizer("full")
    n = 120
    rng = np.random.default_rng(0)
    ds = {
        "vocab": rng.integers(1, fz.meta.vocab_size, (n, MAX_TOKENS)).astype(np.int64),
        "slot": rng.integers(1, 6, (n, MAX_TOKENS)).astype(np.int64),
        "pos": rng.integers(1, 8, (n, MAX_TOKENS)).astype(np.int64),
        "num": rng.standard_normal((n, MAX_TOKENS, TOKEN_NUM_DIM)).astype(np.float32),
        "mask": np.ones((n, MAX_TOKENS), dtype=np.bool_),
        "glob": rng.standard_normal((n, fz.global_dim)).astype(np.float32),
        "place": rng.integers(0, 8, n).astype(np.int64),
        "power_delta": rng.standard_normal(n).astype(np.float32),
        "weight": np.ones(n, dtype=np.float32),
        "game_id": _seq_game_ids(20, 6),
    }
    out = tmp_path / "synergy.pt"
    net = train_synergy_net(ds, out, epochs=1, batch_size=64, d_model=32, n_heads=4, n_layers=1, device=device)
    assert all(p.device.type == "cpu" for p in net.parameters())  # returned on the CPU
    ck = torch.load(out, map_location="cpu", weights_only=False)
    assert all(v.device.type == "cpu" for v in ck["state_dict"].values())  # checkpoint loads anywhere


@pytest.mark.parametrize("device", DEVICES)
def test_train_sovereign_model_end_to_end(device: str, tmp_path: Path) -> None:
    from seaglass_bots.sovereign_model import SOV_MAX_TOKENS, SOV_TOKEN_NUM_DIM, SovereignStateFeaturizer
    from seaglass_bots.train_sovereign import train_model

    fz = SovereignStateFeaturizer("full")
    n_games, per_game = 14, 24
    n = n_games * per_game
    rng = np.random.default_rng(0)
    glob = rng.standard_normal((n, fz.global_dim)).astype(np.float32)
    glob[:, 17] = 1.0  # frozen shop flag (keeps the cached slot-6 tokens)
    glob[:, 18] = (np.arange(n) % 4 == 3).astype(np.float32)  # end-of-turn flag
    npz = tmp_path / "sov.npz"
    np.savez(
        npz,
        vocab=rng.integers(1, fz.meta.vocab_size, (n, SOV_MAX_TOKENS)).astype(np.int32),
        slot=rng.integers(1, 7, (n, SOV_MAX_TOKENS)).astype(np.int8),
        pos=rng.integers(1, 9, (n, SOV_MAX_TOKENS)).astype(np.int8),
        num=rng.standard_normal((n, SOV_MAX_TOKENS, SOV_TOKEN_NUM_DIM)).astype(np.float32),
        mask=np.ones((n, SOV_MAX_TOKENS), dtype=np.bool_),
        glob=glob,
        place=np.tile(np.arange(8, dtype=np.int64), n // 8),
        growth=rng.standard_normal(n).astype(np.float32),
        game_id=_seq_game_ids(n_games, per_game),
        turn=(np.arange(n) % 12 + 1).astype(np.int16),
    )
    out = tmp_path / "sov_out.npz"
    metrics = train_model([str(npz)], str(out), epochs=1, batch_size=64, device=device)
    assert np.isfinite(metrics["val_ce"])
    assert out.is_file()


@pytest.mark.parametrize("device", DEVICES)
def test_train_zero_net_end_to_end(device: str, tmp_path: Path) -> None:
    from seaglass_bots.architect.train_zero import train_zero_net
    from seaglass_bots.architect.zero_model import ZERO_GLOBAL_DIM, ZERO_MAX_TOKENS, ZERO_TOKEN_NUM_DIM, get_zero_vocab

    n_games, per_game = 30, 8
    n = n_games * per_game
    rng = np.random.default_rng(0)
    npz = tmp_path / "zero.npz"
    np.savez(
        npz,
        vocab=rng.integers(1, get_zero_vocab().size, (n, ZERO_MAX_TOKENS)).astype(np.int32),
        zone=rng.integers(0, 6, (n, ZERO_MAX_TOKENS)).astype(np.int8),
        pos=rng.integers(0, 12, (n, ZERO_MAX_TOKENS)).astype(np.int8),
        num=rng.standard_normal((n, ZERO_MAX_TOKENS, ZERO_TOKEN_NUM_DIM)).astype(np.float32),
        mask=np.ones((n, ZERO_MAX_TOKENS), dtype=np.bool_),
        glob=rng.standard_normal((n, ZERO_GLOBAL_DIM)).astype(np.float32),
        place=np.tile(np.arange(8, dtype=np.int64), n // 8),
        fut_pow=rng.standard_normal(n).astype(np.float32),
        game_id=_seq_game_ids(n_games, per_game),
        turn=(np.arange(n) % 12 + 1).astype(np.int16),
    )
    out = tmp_path / "zero_out.npz"
    train_zero_net([str(npz)], str(out), epochs=1, batch_size=64, d_model=32, n_layers=1, device=device)
    assert out.is_file()


# --------------------------------------------------------------------------- CLIs


TRAINER_MODULES = [
    "seaglass_bots.train_apex",
    "seaglass_bots.train_sovereign",
    "seaglass_bots.train_value",
    "seaglass_bots.architect.train_anti_sov",
    "seaglass_bots.architect.train_architect",
    "seaglass_bots.architect.train_dagger_beam",
    "seaglass_bots.architect.train_dist_ppo",
    "seaglass_bots.architect.train_fast_dist_ppo",
    "seaglass_bots.architect.train_gm_dist",
    "seaglass_bots.architect.train_gumbel_az",
    "seaglass_bots.architect.train_policy",
    "seaglass_bots.architect.train_ranker_value",
    "seaglass_bots.architect.train_zero",
    "seaglass_bots.architect.scaling_value",
]


@pytest.mark.parametrize("module", TRAINER_MODULES)
def test_trainer_parser_accepts_device(module: str, monkeypatch, capsys) -> None:
    """`--device cpu` parses (before `--help` exits) and the help lists `--device`."""
    mod = importlib.import_module(module)
    monkeypatch.setattr(sys, "argv", ["trainer", "--device", "cpu", "--help"])
    with pytest.raises(SystemExit) as e:
        mod.main()
    assert e.value.code == 0
    assert "--device" in capsys.readouterr().out
