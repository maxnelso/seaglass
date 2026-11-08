"""Saving and loading trained models.

A checkpoint is one `torch.save` file holding:
- the model's class and constructor arguments, and its weights;
- the feature spec it was trained on (`FEATURE_SPEC`);
- optionally, the optimizer state, the iteration, the training arguments and any extras.

`load` refuses a checkpoint whose feature spec doesn't match this code's, because its inputs or
actions would mean something else.
"""

from __future__ import annotations

import os
from pathlib import Path
from typing import Any

import torch

from .actions import ACTION_SPACE_SIZE
from .card_features import STATIC_DIM
from .features import NUM_SLOTS, PLAYER_DIM, SLOT_NUM_DIM, VOCAB_SIZE
from .model import BattlegroundsMLP
from .transformer import EntityTransformer

# What a model's inputs and outputs mean. A model only works with the code that encodes them.
FEATURE_SPEC: dict[str, int] = {
    "player_dim": PLAYER_DIM,
    "num_slots": NUM_SLOTS,
    "slot_num_dim": SLOT_NUM_DIM,
    "vocab_size": VOCAB_SIZE,
    "action_space": ACTION_SPACE_SIZE,
    # The static card features' width (`card_features.py`); checkpoints saved before it existed
    # don't have it and are only checked on the other keys.
    "static_dim": STATIC_DIM,
}

MODELS: dict[str, type[torch.nn.Module]] = {
    "BattlegroundsMLP": BattlegroundsMLP,
    "EntityTransformer": EntityTransformer,
}
# The class each `model.config["arch"]` builds (a config without one is an MLP's).
ARCHS: dict[str, type[torch.nn.Module]] = {"mlp": BattlegroundsMLP, "transformer": EntityTransformer}


def model_from_config(config: dict[str, Any]) -> torch.nn.Module:
    """A fresh model from its `model.config` (the `(config, state_dict)` sources of `play.py`)."""
    return ARCHS[config.get("arch", "mlp")](**config)


def save(
    path: str | os.PathLike[str],
    model: BattlegroundsMLP,
    optimizer: torch.optim.Optimizer | None = None,
    iteration: int = 0,
    args: dict[str, Any] | None = None,
    extra: dict[str, Any] | None = None,
) -> None:
    """Save `model` (and the training state) to `path`, atomically."""
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    ckpt = {
        "model_class": type(model).__name__,
        "model_config": dict(model.config),
        "model_state": model.state_dict(),
        "feature_spec": dict(FEATURE_SPEC),
        "iteration": iteration,
        "args": dict(args or {}),
        "extra": dict(extra or {}),
    }
    if optimizer is not None:
        ckpt["optimizer_state"] = optimizer.state_dict()
    tmp = path.with_suffix(path.suffix + ".tmp")
    torch.save(ckpt, tmp)
    os.replace(tmp, path)


def load(path: str | os.PathLike[str]) -> tuple[BattlegroundsMLP, dict[str, Any]]:
    """Load the model saved at `path`. Returns `(model, checkpoint)`; the model is in eval mode."""
    ckpt = torch.load(path, map_location="cpu", weights_only=False)
    spec = ckpt.get("feature_spec")
    expected = {k: v for k, v in FEATURE_SPEC.items() if k != "static_dim" or (spec and k in spec)}
    if spec != expected:
        diff = {k: (spec.get(k) if spec else None, v) for k, v in expected.items()}
        diff = {k: d for k, d in diff.items() if d[0] != d[1]}
        raise ValueError(f"{path}: trained on other features (checkpoint, now): {diff}")
    cls = MODELS[ckpt["model_class"]]
    model = cls(**ckpt["model_config"])
    model.load_state_dict(ckpt["model_state"])
    model.eval()
    return model, ckpt
