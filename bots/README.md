# Seaglass Python Search Bots (`bots/`)

Standalone Python bot suite built on top of the Rust `seaglass` engine.
All game rules, turn transitions, and batch combat simulations stay in Rust (`engine/`), while search, belief modeling, 1-turn physics rollouts, Set-Transformer synergy learning, positioning optimization, and ICM survival evaluation live in Python (`seaglass_bots/`).

## Modules

- `seaglass_bots/catalog_meta.py`: Queries `seaglass.card_tags()`, `seaglass.catalog()`, and `seaglass.player_counter_names()` dynamically to build card vocabularies, tag matrices, and hook profiles without hardcoding card IDs.
- `seaglass_bots/value_model.py`: **Set-Transformer (`SynergyValueNet`)** in PyTorch that attends jointly across board units, hand cards, owned trinkets, hero powers, and all 54 engine scaling counters to predict 8-way placement probabilities and non-linear unit combos.
- `seaglass_bots/train_value.py`: Parallel multi-core **Expert Iteration** self-play trainer that generates 8-player `LobbyGame` trajectories and trains `SynergyValueNet`.
- `seaglass_bots/belief.py`: Sequential Monte Carlo **Particle Filter** over `LobbyContext` and `OpponentInfo` that tracks opponent warbands and shared `CardPool` depletion.
- `seaglass_bots/positioning.py`: **Successive Halving** board positioning solver (with 1-turn End-of-Turn / combat-persistence physics tie-breaking) plus **Regret-Matching** 1v1 endgame counter-positioning.
- `seaglass_bots/evaluator.py`: Combines **1-Turn Rust Engine Physics Rollouts** (`end_turn` -> `resolve_combat_against` -> `start_turn`), **Tournament ICM** survival equity, and `SynergyValueNet`.
- `seaglass_bots/search.py`: **Shop-Epoch DAG Beam Search** (with batched neural pruning and full-board `Sell + Play` cycle macros) and **Expectimax MCTS with Progressive Widening** over `LobbyGame.belief_tavern`.
- `seaglass_bots/duos.py`: **Sequential Multi-Agent Rollout** coordinator for `DuosTavern` (`Pass` synergy and carry/support coordination).
- `seaglass_bots/baseline.py`: Baseline opponent bots (`SemiRandomBot`, `TempoBot`, and `GreedyLevelBot`) for league evaluation.
- `seaglass_bots/arena.py`: Full 8-player Solo, Head-to-Head (`v2` vs `v1`), and 4-team Duos `LobbyGame` benchmark runner with placement metrics, per-turn latency stats, and `.replay.pb` recording.
