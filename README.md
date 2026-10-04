# Seaglass

Seaglass is a Hearthstone Battlegrounds simulation engine and reinforcement learning system split into two standalone projects:

- **[`engine/`](engine/README.md)** — A deterministic, headless Hearthstone Battlegrounds engine written in **Rust**, modeling both the **Tavern (Recruit) Phase** and the **Combat Phase**, with thin PyO3 bindings.
- **[`rl/`](rl/README.md)** — A **Python** reinforcement learning project containing the Gym-style RL environment, feature encoding, opponent curriculum, Transformer Actor-Critic network, and PPO training loop.

---

## Architecture & Separation of Concerns

Seaglass enforces a strict boundary between **game rules** and **machine learning**:

| Concern | Owner | Why |
| :--- | :--- | :--- |
| **Game Rules & State Transitions** | [`engine/`](engine/README.md) (Rust) | Single source of truth for cards, economy, pool depletion, legality checks, and combat resolution. |
| **Deterministic PRNG & Batch Combat** | [`engine/`](engine/README.md) (Rust) | Simulating 40–1,000 combats per turn for equity estimation takes microseconds in Rust. |
| **PyO3 Native Extension (`seaglass`)** | [`engine/`](engine/README.md) (Rust) | Pure marshalling layer exposing raw engine state, structured actions, and batch combat to Python. |
| **RL Environment & Action Space** | [`rl/`](rl/README.md) (Python) | Translates between agent action indices and engine `TavernAction`s using the engine's legality checks. |
| **Reward Shaping & Opponent Curriculum** | [`rl/`](rl/README.md) (Python) | Potential-based shaping ($\Phi(s)$), synthetic opponent curves, and self-play can be tuned without recompiling Rust. |
| **Features, Model & PPO Training** | [`rl/`](rl/README.md) (Python) | Entity encoders, Transformer Actor-Critic (`PyTorch`), rollout collection, and policy optimization. |

**Rule of thumb:** Python never reimplements a card effect, cost, legality rule, or combat step; Rust never hardcodes a neural network feature, reward weight, or synthetic opponent curve.

---

## Repository Layout

```text
seaglass/
├── README.md          # Workspace overview (this file)
├── engine/            # Rust crate (`seaglass`): Tavern + Combat engine & PyO3 bindings
│   ├── README.md
│   ├── Cargo.toml
│   ├── docs/          # Pinned specifications (combat.md, tavern.md, scenarios.md)
│   ├── src/
│   └── tests/         # YAML scenario suite (one file per card, plus combat & Tavern topics)
└── rl/                # Python package (`seaglass-rl`): RL env, Transformer policy, PPO trainer
    ├── README.md
    ├── pyproject.toml
    ├── seaglass_rl/
    └── tests/
```

---

## Quickstart

### 1. Build and test the Rust engine

```bash
cd engine
cargo test
```

### 2. Build the Python extension and run RL training

```bash
cd rl
maturin develop --release --manifest-path ../engine/Cargo.toml --features python
pytest
python -m seaglass_rl.train_ppo --iterations 25
```
