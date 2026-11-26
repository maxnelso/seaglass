# Seaglass Python Search Bots (`bots/`)

> **How the RL work uses these bots** (Stage 5 of [`docs/neural-opponents-sprint.md`](../docs/neural-opponents-sprint.md)). The neural opponents in `rl/` treat the bots here as fixed reference points: SovereignBot is the slow ladder's top anchor (`python -m seaglass_rl.ladder rate ... --ladder slow`), and the bots are opponents and teachers to measure against. The learning itself happens in `rl/`, whose pure training path names no card (`rl/tests/test_stage5.py`). Development of the bots here goes on as before. The ladder fingerprints the anchors' sources (`anchors_fingerprint` in every result), so when an anchor changes, comparisons across that change are flagged.

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

## Training devices

- The trainers (`train_value.py`, `train_apex.py`, `train_sovereign.py`, and `architect/train_*.py`) run their gradient updates on CUDA if present, else Apple's GPU (MPS), else the CPU. Pass `--device cpu|mps|cuda|cuda:N|auto` or set `$SEAGLASS_DEVICE`; the flag wins.
- MPS trains in float32 (FP16 autocast and the grad scaler are CUDA-only). Self-play, data collection and inference inside bots stay on the CPU, in worker processes.
- `--workers` defaults to half the CPU cores (capped at each trainer's old default) and the trainer process uses half the cores (at most 8) for torch threads. Override with `$SEAGLASS_WORKERS` / `$SEAGLASS_THREADS`, for example while another job shares the machine.
- Always pass an explicit output path (`--out`, `--out-weights`, `--out-ckpt`, `--run-dir`): the defaults overwrite the bundled weights in `seaglass_bots/data/`.

## Why did SovereignBot do that? (decision tracing)

- Every `SovereignBot` decision fills `bot.last_trace` (`seaglass_bots/sovereign_trace.py`): the lines of play the root compared with their scores, each score split into combat, next-turn prior, next-turn value net and discipline terms, the hand-written gates that blocked options (for example "gold 1 < 3 needed" for a roll), and the end-of-turn rule that fired. Tracing only records numbers the search already computes; a re-run of `arena --mode solo --bot sovereign --games 1 --seed 1000` plays identically with it.
- `bot.last_note` (what the arena writes into the replay) carries a summary: up to 6 `candidates` (the best line per first action, with its score, label and planned steps) and extras such as `combat`, `macro_prior`, `macro_net`, `roll_gated`, `freeze_value` and `plan_value`. `match_replay html` shows them in the Note panel, and the one-player page (`--player p0`) behind its "why" button.
- `python -m seaglass_bots.explain <replay> --player p0 --turn 5` re-runs that seat through the recorded game (the other seats replay their recorded actions), checks that every action still matches, and prints the full trace for the turn. `--what-if` scores end turn, freeze, roll and roll-then-freeze where a gate blocked a roll; `--activations out.npz` adds the value net's placement distribution and per-card contributions (and saves raw activations); `--json` writes everything. The arena now records each seat's bot and seed in the replay meta (`seats`); older arena replays fall back to the arena's seeding convention, and `--bot-seed` overrides both.
