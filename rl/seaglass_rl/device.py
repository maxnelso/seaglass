"""Where training runs (`--device`) and how much of the machine it takes (`--workers`, `--threads`).

Self-play always runs on the CPU, in worker processes (the models' NumPy fast paths). Only the
gradient updates move to an accelerator. `--device auto`, the default, picks CUDA, then Apple's
Metal GPU (MPS), then the CPU.

Each default can be set for the whole machine with an environment variable, for example while
another job shares it:

    export SEAGLASS_DEVICE=cpu      # auto, cpu, mps, cuda or cuda:N
    export SEAGLASS_WORKERS=4       # self-play worker processes
    export SEAGLASS_THREADS=2       # torch threads in the trainer process

Without them, workers and threads default to half the CPU cores (threads at most 8). A flag on the
command line wins over both.
"""

from __future__ import annotations

import argparse
import os
from typing import Any

import torch

DEVICE_ENV = "SEAGLASS_DEVICE"
WORKERS_ENV = "SEAGLASS_WORKERS"
THREADS_ENV = "SEAGLASS_THREADS"

DEVICE_HELP = (
    "where the updates run: auto (CUDA, then MPS, then CPU), cpu, mps, cuda or cuda:N "
    f"(default: ${DEVICE_ENV}, else auto)"
)


def mps_available() -> bool:
    """Whether Apple's Metal GPU backend can run here."""
    mps = getattr(torch.backends, "mps", None)
    return bool(mps is not None and mps.is_available())


def pick_device(name: str | None = None) -> torch.device:
    """The device `name` names: `auto`, `cpu`, `mps`, `cuda` or `cuda:N`. With `None`,
    `$SEAGLASS_DEVICE`, else `auto`. Raises `ValueError` for a device this machine doesn't have."""
    if name is None:
        name = os.environ.get(DEVICE_ENV) or "auto"
    name = name.strip().lower()
    if name == "auto":
        if torch.cuda.is_available():
            return torch.device("cuda")
        if mps_available():
            return torch.device("mps")
        return torch.device("cpu")
    try:
        device = torch.device(name)
    except RuntimeError as e:
        raise ValueError(f"unknown device {name!r} (auto, cpu, mps, cuda or cuda:N)") from e
    if device.type == "cuda" and not torch.cuda.is_available():
        raise ValueError(f"device {name!r}: CUDA is not available on this machine")
    if device.type == "mps" and not mps_available():
        raise ValueError(f"device {name!r}: MPS (Apple's Metal GPU) is not available on this machine")
    if device.type not in ("cpu", "cuda", "mps"):
        raise ValueError(f"unsupported device {name!r} (auto, cpu, mps, cuda or cuda:N)")
    return device


def add_device_arg(p: argparse.ArgumentParser) -> None:
    """`--device` (`pick_device(args.device)` resolves it)."""
    p.add_argument("--device", default=None, help=DEVICE_HELP)


def _env_count(name: str) -> int | None:
    raw = os.environ.get(name, "").strip()
    if not raw:
        return None
    try:
        n = int(raw)
    except ValueError:
        raise ValueError(f"${name} must be a positive integer, not {raw!r}") from None
    if n < 1:
        raise ValueError(f"${name} must be a positive integer, not {raw!r}")
    return n


def cpu_count() -> int:
    return os.cpu_count() or 4


def default_workers(cap: int | None = None) -> int:
    """Self-play worker processes: `$SEAGLASS_WORKERS`, else half the CPU cores, at most `cap`
    (a trainer whose workers are heavy keeps its lower count)."""
    env = _env_count(WORKERS_ENV)
    if env is not None:
        return env
    n = max(1, cpu_count() // 2)
    return min(n, cap) if cap is not None else n


def default_threads() -> int:
    """Torch threads in the trainer process: `$SEAGLASS_THREADS`, else half the CPU cores, at
    most 8."""
    env = _env_count(THREADS_ENV)
    if env is not None:
        return env
    return max(1, min(8, cpu_count() // 2))


def cpu_state_dict(model: torch.nn.Module) -> dict[str, torch.Tensor]:
    """A detached CPU copy of `model`'s weights, wherever it trains: what self-play workers,
    evaluation and checkpoints take."""
    return {k: v.detach().to("cpu", copy=True) for k, v in model.state_dict().items()}


def to_cpu(obj: Any) -> Any:
    """`obj` with every tensor in it moved to the CPU, through dicts, lists and tuples (e.g. an
    optimizer's `state_dict()`)."""
    if isinstance(obj, torch.Tensor):
        return obj.detach().to("cpu")
    if isinstance(obj, dict):
        return {k: to_cpu(v) for k, v in obj.items()}
    if isinstance(obj, (list, tuple)):
        return type(obj)(to_cpu(v) for v in obj)
    return obj


def empty_cache(device: torch.device) -> None:
    """Give the accelerator's cached memory back (e.g. after an update, before self-play). A no-op
    on the CPU."""
    if device.type == "cuda":
        torch.cuda.empty_cache()
    elif device.type == "mps":
        torch.mps.empty_cache()
