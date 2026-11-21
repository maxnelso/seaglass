# Seaglass

Seaglass is a Hearthstone Battlegrounds simulation engine and reinforcement learning system split into two standalone projects:

- **[`engine/`](engine/README.md)** — A deterministic, headless Hearthstone Battlegrounds engine written in **Rust**, modeling both the **Tavern (Recruit) Phase** and the **Combat Phase**, with thin PyO3 bindings.
- **[`rl/`](rl/README.md)** — A **Python** reinforcement learning project containing the Gym-style RL environment, feature encoding, opponent curriculum, Transformer Actor-Critic network, and PPO training loop.

---

## Project Goals

- **Long term:** a patch-friendly simulator, sparring partner, superhuman opponent, and grader of human replays, for Solos and Duos.
- **Short term:** one fixed patch and Solos only.
- **Target architecture for the eventual opponent:** AlphaZero-style, with no human-written search heuristics or play rules.
  - "AlphaZero-style" means researchers, human or agent, design the method: the policy and value networks, the search (MCTS or an equivalent), the shape of the network, and the training loop. The agent then learns to play by self-play.
  - It rules out human game knowledge inside the agent. There are no lines of code like "always roll with 1 Gold left unless you plan to freeze", and no hand-engineered features at that level of specificity.
  - The hand-written bots in [`rl/`](rl/README.md) and [`bots/`](bots/README.md) are scaffolding: strong opponents to bootstrap from and fixed points to measure against.

The neural opponent's objective is linear in placement: one point per opponent beaten, so 7 points for 1st and 0 for 8th.

### Near-Term Sprint

The current sprint is about strength in Solos. The plan, the critique behind it, and the measurements are in [`docs/neural-opponents-sprint.md`](docs/neural-opponents-sprint.md). In order:

1. **Measure.** One command that rates any agent on a fixed ladder of opponents, with enough games to tell agents apart.
2. **Fix the learner, then run it at scale.** Whole-game credit assignment, a placement critic, no stat-sum shaping, a stronger warm start, and one run of 100,000 games or more.
3. **Raise throughput**, in parallel. Move observation encoding out of Python and batch inference.
4. **Add search inside the turn**, scored by the learned critic, and use it as the teacher.
5. **Freeze `bots/`** as a reference opponent and teacher.

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
├── docs/              # Project plans (neural-opponents-sprint.md)
├── engine/            # Rust crate (`seaglass`): Tavern + Combat engine & PyO3 bindings
│   ├── README.md
│   ├── Cargo.toml
│   ├── docs/          # Pinned specifications (combat.md, duos.md, duos_tavern.md, tavern.md, heroes.md, trinkets.md, dark_gifts.md, scenarios.md, replays.md)
│   ├── proto/         # Match replay format (replay.proto)
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
