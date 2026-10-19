"""PPO for trajectories with per-turn discounting (`play.py`): GAE and a minibatched update."""

from __future__ import annotations

from typing import Any

import numpy as np
import torch
import torch.nn as nn


def gae(
    rewards: np.ndarray,
    values: np.ndarray,
    turn_end: np.ndarray,
    done: bool,
    gamma: float,
    lam: float,
) -> tuple[np.ndarray, np.ndarray]:
    """Generalized advantage estimation for one trajectory. Returns `(advantages, returns)`.

    Discounting happens only between turns: step `t` discounts the next value by `gamma` (and the
    next advantage by `gamma * lam`) if its action ended a turn, and by 1 (and `lam`) otherwise.
    A trajectory that is not `done` was cut short and bootstraps from its last value.
    """
    T = len(rewards)
    adv = np.zeros(T, dtype=np.float32)
    last = 0.0
    for t in reversed(range(T)):
        g = gamma if turn_end[t] else 1.0
        if t == T - 1:
            next_value = 0.0 if done else values[t]
        else:
            next_value = values[t + 1]
        delta = rewards[t] + g * next_value - values[t]
        last = delta + g * lam * last
        adv[t] = last
    return adv, adv + values


def build_batch(trajectories: list[dict[str, np.ndarray]], gamma: float, lam: float) -> dict[str, Any]:
    """Concatenate trajectories (`Trajectory.arrays`) into one training batch, with advantages and
    returns."""
    cols: dict[str, list[np.ndarray]] = {
        k: [] for k in ("player_vec", "slot_card_ids", "slot_num_feats", "action_mask", "actions", "log_probs")
    }
    advs, rets, values = [], [], []
    for tr in trajectories:
        a, r = gae(tr["rewards"], tr["values"], tr["turn_end"], bool(tr["done"]), gamma, lam)
        advs.append(a)
        rets.append(r)
        values.append(tr["values"])
        for k in cols:
            cols[k].append(tr[k])
    return {
        "player_vec": torch.from_numpy(np.concatenate(cols["player_vec"])),
        "slot_card_ids": torch.from_numpy(np.concatenate(cols["slot_card_ids"]).astype(np.int64)),
        "slot_num_feats": torch.from_numpy(np.concatenate(cols["slot_num_feats"]).astype(np.float32)),
        "action_mask": torch.from_numpy(np.concatenate(cols["action_mask"])),
        "actions": torch.from_numpy(np.concatenate(cols["actions"])),
        "old_log_probs": torch.from_numpy(np.concatenate(cols["log_probs"])),
        "old_values": torch.from_numpy(np.concatenate(values)),
        "advantages": torch.from_numpy(np.concatenate(advs)),
        "returns": torch.from_numpy(np.concatenate(rets)),
    }


def ppo_update(
    model: nn.Module,
    optimizer: torch.optim.Optimizer,
    batch: dict[str, Any],
    epochs: int = 4,
    minibatch_size: int = 4096,
    clip_eps: float = 0.2,
    vf_coef: float = 0.5,
    ent_coef: float = 0.01,
    max_grad_norm: float = 0.5,
    target_kl: float | None = 0.03,
) -> dict[str, float]:
    """Clipped PPO over shuffled minibatches, with the advantages normalized over the batch.

    Stops early once the approximate KL to the rollout policy passes `target_kl`.
    """
    n = batch["actions"].shape[0]
    adv = batch["advantages"]
    adv = (adv - adv.mean()) / (adv.std() + 1e-8) if n > 1 else adv
    stats: dict[str, list[float]] = {"policy_loss": [], "value_loss": [], "entropy": [], "kl": [], "clip_frac": []}
    model.train()
    stop = False
    epochs_run = 0
    for _ in range(epochs):
        perm = torch.randperm(n)
        for start in range(0, n, minibatch_size):
            idx = perm[start : start + minibatch_size]
            logits, values = model(
                batch["player_vec"][idx],
                batch["slot_card_ids"][idx],
                batch["slot_num_feats"][idx],
                batch["action_mask"][idx],
            )
            log_probs = torch.log_softmax(logits, dim=-1)
            new_lp = log_probs.gather(1, batch["actions"][idx].unsqueeze(-1)).squeeze(-1)
            probs = log_probs.exp()
            entropy = -(probs * log_probs.clamp(min=-20.0)).sum(-1).mean()
            log_ratio = new_lp - batch["old_log_probs"][idx]
            ratio = log_ratio.exp()
            a = adv[idx]
            policy_loss = -torch.min(ratio * a, ratio.clamp(1 - clip_eps, 1 + clip_eps) * a).mean()
            value_loss = 0.5 * (values - batch["returns"][idx]).pow(2).mean()
            loss = policy_loss + vf_coef * value_loss - ent_coef * entropy
            optimizer.zero_grad(set_to_none=True)
            loss.backward()
            nn.utils.clip_grad_norm_(model.parameters(), max_grad_norm)
            optimizer.step()
            with torch.no_grad():
                kl = ((ratio - 1) - log_ratio).mean().item()
                stats["kl"].append(kl)
                stats["clip_frac"].append(((ratio - 1).abs() > clip_eps).float().mean().item())
            stats["policy_loss"].append(policy_loss.item())
            stats["value_loss"].append(value_loss.item())
            stats["entropy"].append(entropy.item())
            if target_kl is not None and kl > 1.5 * target_kl:
                stop = True
                break
        epochs_run += 1
        if stop:
            break
    model.eval()
    out = {k: float(np.mean(v)) if v else 0.0 for k, v in stats.items()}
    out["epochs"] = float(epochs_run)
    # How much of the returns' variance the value function explains (1 is perfect).
    with torch.no_grad():
        ret = batch["returns"]
        var = ret.var().item()
        out["explained_var"] = float(1 - (ret - batch["old_values"]).var().item() / var) if var > 0 else 0.0
    return out
