# Seaglass RL (`rl/`)

`seaglass-rl` is the Python reinforcement learning project for training agents to play Hearthstone Battlegrounds on top of the [`seaglass` Rust engine](../engine/README.md).

**All game rules live in Rust; all RL decisions live in Python.**
This package never reimplements a card, cost, legality check, or combat step—it calls the `seaglass` native extension for state transitions and batch combat simulations, while keeping the MDP formulation (observation encoding, action space mapping, reward shaping, and opponent curriculum) and neural network training in Python for rapid iteration.

---

## Responsibilities of `rl/`

1. **RL Environment (`env.py`)**
   Wraps the Rust engine's tavern state machine and batch combat simulator into a single-player or multi-agent RL environment (`BattlegroundsEnv`) with deterministic seeding.
2. **Action Space & Masking (`actions.py`)**
   Defines the discrete action space consumed by the policy network, queries the engine for legal actions at every micro-step, and produces boolean action masks. Illegal actions raise `ValueError` immediately rather than silently no-oping.
3. **Reward Shaping (`rewards.py`)**
   Computes step and end-of-turn rewards in Python using combat equity from `seaglass.simulate_batch`, tavern tier progression, hero health preservation, and micro-action shaping bonuses.
4. **Opponent Curriculum & Self-Play (`opponents.py`)**
   Generates opponent boards for end-of-turn combat evaluation—ranging from deterministic turn-scaling baseline boards to historical ghost boards and self-play checkpoints.
5. **Feature Encoding & Policy Network (`features.py`, `model.py`)**
   Converts raw engine observations into entity tensors and runs them through a Transformer Actor-Critic (`BattlegroundsActorCritic`).
6. **Training & Inspection (`train_ppo.py`, `sandbox_cli.py`)**
   Masked PPO training loop, evaluation benchmarks, and an interactive CLI inspector.

---

## MDP Formulation

### 1. Observation & Feature Encoding
At each step, the Rust engine returns raw state (`turn`, `health`, `gold`, `max_gold`, `tavern_tier`, `upgrade_cost`, `bonus_gold_next_turn`, `is_frozen`, and `PyUnit` lists for `board`, `hand`, `shop`, and `discover`).

`features.py` encodes this into:
- **Player Token (`[PLAYER]`)**: Normalized vector of global economy and turn state:
  `[gold/10, max_gold/10, tavern_tier/6, upgrade_cost/10, turn/max_turns, health/30, bonus_gold_next_turn/5]`.
- **Card Entity Tokens**: One token per card across `board` (`zone=0`), `hand` (`zone=1`), and `shop` (`zone=2`), containing:
  - `card_id`: Dense vocabulary index derived dynamically at import time from `seaglass.catalog("full")` plus token/spell IDs (never a hardcoded ID table).
  - `tribe`: Engine `Tribe` discriminant (`0..11`).
  - `zone`: Zone index (`0..2`).
  - `stats`: `[log1p(attack), log1p(health), tavern_tier / 6, slot / 7]`.
  - `keywords`: Binary flags `[is_golden, taunt, divine_shield, windfury, reborn]`.

### 2. Action Space & Legality Mask
During a Tavern turn, the agent repeatedly takes discrete micro-actions until selecting `EndTurn`.
- Every action index maps deterministically to an engine `TavernAction` (`Buy`, `Play`, `Sell`, `Reposition`, `ChooseDiscover`, `Refresh`, `UpgradeTavern`, `ToggleFreeze`, `EndTurn`).
- The boolean `action_mask` is queried directly from the engine's legality check at every step. When a Triple Reward Discover is pending, all actions except `ChooseDiscover` are masked out.

### 3. Reward Function
Default potential-based reward shaping over turn boundaries:

$$\Phi(s) = w_{\text{eq}} \cdot \text{combat\_equity}(s) + w_{\text{tier}} \cdot (\text{tavern\_tier} - 1) + w_{\text{hp}} \cdot \frac{\text{health}}{30}$$

$$r_t = \Phi(s_t) - \Phi(s_{t-1}) + r_{\text{shaping}}$$

where $\text{combat\_equity}(s) = \text{win\_rate} + 0.5 \cdot \text{draw\_rate}$ is estimated via `seaglass.simulate_batch` against the current turn's opponent board. Because $\Phi(s)$ is computed in Python, reward weights and shaping terms can be configured per experiment without touching the Rust crate.

---

## Model Architecture (`BattlegroundsActorCritic`)

1. **`PlayerEntityEncoder`**: Projects the 7-D player economy vector into a 64-D `[PLAYER]` token (`Entity #0`).
2. **`CardEntityEncoder`**: Embeds `card_id`, `tribe`, and `zone` via `nn.Embedding`, concatenates continuous stats and keyword flags, and projects each card to a 64-D token (`Entities #1..N`).
3. **Transformer Backbone**: Stacks `[PLAYER, Card_1, ..., Card_N]` into a `[1, 1 + N, 64]` sequence and passes it through a 2-layer `nn.TransformerEncoder` so cards attend to the player economy and to each other (synergies, triples, tribal scaling).
4. **Heads (read from transformed `Entity #0`)**:
   - **Actor Head**: Linear projection to action logits, masked with `-1e9` wherever `action_mask == False`.
   - **Critic Head**: Linear projection to scalar state-value estimate $V(s)$.

---

## Package Layout

```text
rl/
├── README.md                  # This file
├── pyproject.toml             # Package metadata & dependencies (torch, pytest, maturin)
├── seaglass_rl/
│   ├── __init__.py
│   ├── env.py                 # BattlegroundsEnv (Gym-like interface over `seaglass`)
│   ├── actions.py             # Action space mapping & mask construction
│   ├── features.py            # Dynamic card vocabulary & observation -> tensor encoding
│   ├── rewards.py             # Potential-based reward shaping & combat equity evaluation
│   ├── opponents.py           # Synthetic scaling opponent & self-play opponent pool
│   ├── model.py               # CardEntityEncoder, PlayerEntityEncoder, BattlegroundsActorCritic
│   ├── train_ppo.py           # PPO rollout collection, GAE/returns, clipped policy update
│   └── sandbox_cli.py         # Interactive terminal inspector for tavern & combat
└── tests/
    └── test_env_and_seam.py   # Verifies Rust<->Python determinism, catalog sync, and masks
```

---

## Quickstart

### 1. Build & install the `seaglass` Rust extension

From the `rl/` directory (inside your Python virtual environment):

```bash
maturin develop --release --manifest-path ../engine/Cargo.toml --features python
```

### 2. Run seam & environment tests

```bash
pytest
```

### 3. Explore interactively in the CLI sandbox

```bash
python -m seaglass_rl.sandbox_cli
```

### 4. Train a PPO agent

```bash
python -m seaglass_rl.train_ppo --iterations 25 --episodes-per-iter 8
```
