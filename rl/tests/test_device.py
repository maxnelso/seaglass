"""Device selection (`device.py`) and training on an accelerator (MPS or CUDA) instead of the CPU."""

from __future__ import annotations

import json

import numpy as np
import pytest
import torch

from seaglass_rl import checkpoint
from seaglass_rl.device import (
    cpu_state_dict,
    default_threads,
    default_workers,
    mps_available,
    pick_device,
    to_cpu,
)
from seaglass_rl.model import BattlegroundsMLP
from seaglass_rl.play import GameSpec, LobbyConfig, PlayOptions, play_games
from seaglass_rl.ppo import build_batch, ppo_update
from seaglass_rl.transformer import EntityTransformer

ACCEL = "cuda" if torch.cuda.is_available() else ("mps" if mps_available() else None)
needs_accel = pytest.mark.skipif(ACCEL is None, reason="no CUDA or MPS device")
SMALL = dict(d_model=32, layers=2, heads=2, ff=64, head_dim=32, card_embed_dim=8)


def _make(arch: str) -> torch.nn.Module:
    if arch == "mlp":
        return BattlegroundsMLP(hidden_dim=32, card_embed_dim=4, seed=3)
    return EntityTransformer(seed=3, kind_prior=True, **SMALL)


def _batch(model: torch.nn.Module) -> dict:
    config = LobbyConfig(players=4, max_turns=6)
    specs = [GameSpec(seed=s, controllers=["learner", "heuristic", "learner", "random"]) for s in range(2)]
    results = play_games(config, specs, {"learner": model}, PlayOptions(shaping=0.1))
    return build_batch([t for r in results for t in r.trajectories], gamma=0.97, lam=0.95)


def test_pick_device(monkeypatch) -> None:
    monkeypatch.delenv("SEAGLASS_DEVICE", raising=False)
    assert pick_device("cpu").type == "cpu"
    assert pick_device("auto").type in ("cpu", "cuda", "mps")
    assert pick_device(None).type == pick_device("auto").type
    with pytest.raises(ValueError):
        pick_device("bogus")
    monkeypatch.setenv("SEAGLASS_DEVICE", "cpu")
    assert pick_device(None).type == "cpu"


@pytest.mark.skipif(torch.cuda.is_available(), reason="CUDA is available")
def test_cuda_raises_when_unavailable() -> None:
    with pytest.raises(ValueError):
        pick_device("cuda")


def test_default_workers_and_threads(monkeypatch) -> None:
    monkeypatch.delenv("SEAGLASS_WORKERS", raising=False)
    monkeypatch.delenv("SEAGLASS_THREADS", raising=False)
    assert default_workers() >= 1 and 1 <= default_threads() <= 8
    assert default_workers(cap=1) == 1
    monkeypatch.setenv("SEAGLASS_WORKERS", "3")
    monkeypatch.setenv("SEAGLASS_THREADS", "2")
    assert default_workers() == 3 and default_threads() == 2
    assert default_workers(cap=1) == 3  # an explicit count wins over the cap
    for bad in ("abc", "0", "-2"):
        monkeypatch.setenv("SEAGLASS_WORKERS", bad)
        monkeypatch.setenv("SEAGLASS_THREADS", bad)
        with pytest.raises(ValueError):
            default_workers()
        with pytest.raises(ValueError):
            default_threads()


def test_to_cpu_and_cpu_state_dict_copy() -> None:
    nested = {"a": torch.ones(2), "b": [torch.zeros(1), (torch.ones(1), 5)], "c": "x"}
    out = to_cpu(nested)
    assert out["a"].device.type == "cpu" and isinstance(out["b"][1], tuple) and out["b"][1][1] == 5 and out["c"] == "x"
    model = _make("mlp")
    sd = cpu_state_dict(model)
    before = {k: v.clone() for k, v in model.state_dict().items()}
    for v in sd.values():
        v.zero_()
    assert all(torch.equal(before[k], v) for k, v in model.state_dict().items())


@needs_accel
@pytest.mark.parametrize("arch", ["mlp", "transformer"])
def test_ppo_update_and_checkpoint_on_the_accelerator(arch: str, tmp_path) -> None:
    model = _make(arch)
    batch = _batch(model)
    model.to(ACCEL)
    opt = torch.optim.Adam(model.parameters(), lr=1e-3)
    metrics = ppo_update(model, opt, batch, epochs=2, minibatch_size=128)
    assert all(np.isfinite(v) for v in metrics.values())
    assert all(p.device.type == ACCEL for p in model.parameters())
    path = tmp_path / "m.pt"
    checkpoint.save(path, model, opt, iteration=1)
    loaded, ckpt = checkpoint.load(path)
    assert all(p.device.type == "cpu" for p in loaded.parameters())
    for k, v in cpu_state_dict(model).items():
        assert torch.equal(loaded.state_dict()[k], v)
    state = ckpt["optimizer_state"]["state"]
    assert state and all(t.device.type == "cpu" for s in state.values() for t in s.values() if torch.is_tensor(t))


@needs_accel
@pytest.mark.parametrize("arch", ["mlp", "transformer"])
def test_cpu_and_accelerator_agree_on_one_update(arch: str) -> None:
    cpu_model = _make(arch)
    batch = _batch(cpu_model)
    accel_model = _make(arch)
    accel_model.load_state_dict(cpu_model.state_dict())
    accel_model.to(ACCEL)
    n = batch["actions"].shape[0]
    out = []
    for m in (cpu_model, accel_model):
        torch.manual_seed(0)
        opt = torch.optim.Adam(m.parameters(), lr=1e-3)
        out.append(ppo_update(m, opt, batch, epochs=1, minibatch_size=n, target_kl=None))
    for k in ("policy_loss", "value_loss", "entropy"):
        assert out[0][k] == pytest.approx(out[1][k], rel=1e-3, abs=1e-4), k


@pytest.mark.parametrize("device", ["cpu", pytest.param(ACCEL, marks=needs_accel)])
def test_train_lobby_runs_end_to_end(device: str, tmp_path) -> None:
    from seaglass_rl import train_lobby

    run = tmp_path / "run"
    train_lobby.main(
        ["--run-dir", str(run), "--iterations", "2", "--games", "2", "--workers", "0", "--threads", "1",
         "--minibatch", "256", "--eval-every", "2", "--eval-games", "2", "--snapshot-every", "1",
         "--max-turns", "6", "--players", "4", "--device", str(device)]
    )
    model, ckpt = checkpoint.load(run / "latest.pt")
    assert ckpt["iteration"] == 2 and all(p.device.type == "cpu" for p in model.parameters())
    assert json.loads((run / "args.json").read_text())["device"] == device
