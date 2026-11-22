# Neural Opponents: Near-Term Sprint

*Written 2026-11-21. Measurements were taken at commit `711794e` in a 4-core Linux container with CPU PyTorch, on the default Solo lobby.*

This is the working plan for making the neural opponents stronger.

- [Section 1](#1-aims-and-decisions) states the aims and the decisions that scope the sprint.
- [Section 2](#2-the-sprint) is the sprint: five stages, each with a gate.
- [Section 3](#3-why-this-order-the-critique) is the critique of the current approach that the plan answers.
- [Section 4](#4-measurements) holds the measurements, and [Section 5](#5-caveats) says how far to trust them.

## 1. Aims and Decisions

The project goals are in the [top-level README](../README.md#project-goals). These decisions scope the sprint.

| Question | Decision |
| :--- | :--- |
| What comes first? | Strength. Difficulty levels and play-style variety are not goals. Speed at play time is welcome later and is not required now. |
| What is the objective? | Linear in placement: one point per opponent beaten, 7 for 1st down to 0 for 8th. |
| Which mode? | Solos on one fixed patch. Duos and patch updates come later. |
| What is the target design? | AlphaZero-style: learned policy and value, self-play, and a search designed by researchers. No hand-written play rules and no hand-engineered features at the level of single cards or situations. |
| What about the hand-written bots? | They keep their own utilities and stay as they are. They are opponents to bootstrap from and fixed points to measure against. |
| What hardware? | An Apple-silicon Mac for now. The CUDA path must keep working for other machines. |
| Is `bots/` protected? | No. It can change on this fork, and anything needed later can be recovered from git history. |

The existing PPO reward already matches the objective. `play.placement_reward` gives +1 for 1st down to -1 for 8th, which is `2 * points / 7 - 1`.

Reports use average place, where 4.5 is level and lower is better. Average points are 8 minus the average place.

## 2. The Sprint

Each stage has a gate. A later stage should not start until the gate before it passes, except Stage 3, which runs alongside Stages 1 and 2.

| Stage | Deliverable | Gate |
| :--- | :--- | :--- |
| 1. Measure | One command that rates any agent on a fixed ladder | Two runs of the same agent agree within the stated interval |
| 2. Fix the learner, then scale | New PPO defaults and one run of 100,000 games or more | A pure network beats 9kgamerbot at milliseconds per decision |
| 3. Raise throughput (parallel) | Observation encoding out of Python, batched inference | 5 times more games per core, with 10 times as the target |
| 4. Search inside the turn | Policy-guided search scored by the learned critic | Search plus net beats the net alone, then the distilled net beats the old net |
| 5. Freeze `bots/` | A pinned reference opponent, useful ideas ported to `rl/` | No hand-written card knowledge in the training path |

### Stage 1: Build the Measuring Instrument

Every later choice depends on telling a real gain from noise, and the current evaluation cannot do that. The trainer checks itself on 96 games, which is good to about ±0.45 places at two sigma.

Build one command that rates any agent, and use it for every claim. `seaglass_rl.evaluate` and `play.Player` already do most of the work.

- **Fast ladder.** Seat the agent in 2 of 8 seats, with two each of `9kgamerbot`, `9kgamerbot-tempo` and `heuristic`. Play 500 lobbies on fixed seeds with shuffled seats. That is 1,000 seat-games and about ±0.15 places at two sigma.
- **Slow ladder.** Add one `sovereign` seat and play about 100 lobbies. SovereignBot costs 30 to 50 seconds of CPU per game, so run this rarely.
- **Pin the anchors.** The ladder bots must not change between runs. Record the commit with every result.
- **Report** average place with its standard error, top-4 rate and first-place rate. Also report behaviour by turn: Tavern tier, Gold left unspent and actions taken.
- **Add a per-turn probe.** Fight each end-of-turn board against a frozen library of reference boards for the same turn, and report the win rate by turn. The combat simulator runs about 40,000 fights per second on one core, so this is nearly free.
- **Raise the trainer's own check.** `train_lobby --eval-games` defaults to 96 against seven HeuristicBots. Point it at the fast ladder instead.

The probe matters because placement arrives once per game and is noisy. Board strength by turn is a far denser signal, and it shows directly whether an agent is trading early strength for late strength.

**Gate.** Two runs of the same agent on different seeds agree within the stated interval. The rule-bot order from [Section 4](#4-measurements) reproduces.

#### Stage 1 Result (2026-11-21): the gate passes

The instrument is `python -m seaglass_rl.ladder` (`rl/seaglass_rl/ladder.py`, `probe.py`), described in the [`rl` README](../rl/README.md#rating-agents-ladderpy). `train_lobby` now rates itself on the fast ladder. The result files are in [`results/stage1/`](results/stage1/), taken at commit `2004905`, engine build `34fd1ee4a7df`, on the Mac mini's CPU with 2 to 3 worker processes. A fast-ladder rating took 8 seconds for a rule bot and 24 seconds for the MLP.

Two seeds of the same agent agree. The ± figures are one standard error, clustered by lobby. The difference column is at two sigma.

| Agent on the fast ladder | Seed 5,000,000 | Seed 5,500,000 | Difference |
| :--- | :--- | :--- | :--- |
| HeuristicBot | 5.06 ± 0.06 | 5.08 ± 0.06 | -0.03 ± 0.17 |
| 9kgamerbot | 4.32 ± 0.07 | 4.34 ± 0.08 | -0.02 ± 0.20 |
| Clone of HeuristicBot (60 games, MLP) | 5.88 ± 0.05 | 5.88 ± 0.06 | -0.00 ± 0.15 |

The rule-bot order reproduces. Rating `random` seats two each of four bots, the composition of the 400-lobby ranking in [Section 4](#how-the-rule-bots-rank). The anchors in that run came out at 3.04, 3.20 and 4.28 for 9kgamerbot, 9kgamerbot-tempo and HeuristicBot, against 3.13, 3.13 and 4.29 there. Random placed 7.48, against 7.46.

| Agent on the fast ladder | Average place | Top 4 | First |
| :--- | :--- | :--- | :--- |
| 9kgamerbot-tempo | 4.14 ± 0.07 | 56% | 14% |
| 9kgamerbot | 4.32 ± 0.07 | 54% | 16% |
| HeuristicBot | 5.06 ± 0.06 | 40% | 3% |
| Clone of HeuristicBot | 5.88 ± 0.05 | 23% | 3% |
| Random | 7.48 ± 0.01 | 0% | 0% |

The slow ladder took 12 minutes for 100 lobbies on 3 workers, about 21 seconds of CPU per lobby. Its file is marked dirty only because this document was being edited during the run. SovereignBot placed 1.27 ± 0.07 with 81% firsts, against 1.40 in the 48-lobby table of [Section 4](#how-the-search-bots-rank). 9kgamerbot as the agent placed 5.21 ± 0.15, and the anchors 4.71 to 5.31. So the slow ladder spends most of its resolution on SovereignBot's dominance, and it will only matter once an agent is near the top of the fast ladder.

Pairing two agents on the same seeds barely narrows the interval. 9kgamerbot against HeuristicBot is -0.73 ± 0.17 at two sigma paired and ± 0.18 unpaired. Once the agent's own play differs, a shared seed shares little luck, so the 500-lobby size is what buys the precision.

The probe shows each bot's curve, and it reproduces to within 0.02 across seeds. It is the win rate of the agent's end-of-turn board against 32 reference boards from the same turn.

| Probe by turn | 2 | 4 | 6 | 8 | 10 | 12 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| HeuristicBot | 0.46 | 0.63 | 0.59 | 0.46 | 0.34 | 0.32 |
| 9kgamerbot | 0.45 | 0.19 | 0.31 | 0.65 | 0.59 | 0.59 |
| 9kgamerbot-tempo | 0.45 | 0.51 | 0.55 | 0.54 | 0.47 | 0.49 |
| Clone of HeuristicBot | 0.37 | 0.45 | 0.41 | 0.32 | 0.29 | 0.34 |

HeuristicBot is strongest on turns 4 to 6 and fades. 9kgamerbot gives up board strength on turns 4 to 6 and is the strongest from turn 8. That is the tempo-against-greed trade the probe was meant to show. The clone is weaker than its teacher on every turn, which fits its deficit of 0.8 places.

### Stage 2: Fix the Learner's Defaults, Then Run It at Scale

The PPO pipeline in `rl/` is the right base. Its defaults push it toward buying stats every turn, and its runs are far too short. Change the defaults in one pass, then spend the compute on a single long run.

| Change | Where | Why |
| :--- | :--- | :--- |
| Apply λ once per turn, and set the per-turn γ to 1 | `ppo.gae`, `train_lobby --gamma` | Today λ = 0.95 decays per action, so credit halves every one to three turns. A level-up on turn 5 gets almost no credit for a win on turn 12. `architect/train_fast_dist_ppo.py` has a turn-level version. |
| Replace the scalar critic with an 8-way placement distribution | `model.py`, `transformer.py`, `ppo.py` | It is a better-conditioned target, and the linear value is its expectation. The Architect trainers have one, but with first-place-heavy atoms. Use linear atoms here. |
| Turn off stat-sum shaping | `train_lobby --shaping` (default 0.3) | The potential is a log of summed Attack and Health. It is blind to Deathrattles, auras and scaling, and it rewards stats now. If shaping is still needed, use the Stage 1 probe as the potential. |
| Take positioning out of the policy | `train_lobby --no-reposition`, plus a solver in the environment | The simulator can score a board order exactly. `bots/seaglass_bots/sovereign_positioning.py` already does this. |
| Use a stronger warm start | `pretrain.py` | It clones HeuristicBot today. 9kgamerbot is a full place better and plays 13 games per second. SovereignBot is about four places better again but slow, so use it for a smaller dataset and to relabel the learner's own states. |
| Widen the league | `train_lobby --p-heuristic`, `--pool-size` | Keep both 9kgamerbot variants in the field permanently. Sample past snapshots from the whole history, not only the last ten. |
| Add an auxiliary head for future board strength | model and loss | It gives the network a reason to represent scaling before the placement signal can teach it. |
| Pick the model by a matched run | `train_lobby --model` | The entity transformer scores each action from the card it touches, so it should generalise better than the flat MLP. Test that on the ladder before making it the default. |

Then make one run of 100,000 to 1,000,000 games. At the speed measured here, 100,000 games take about ten hours on four cores, so the low end is possible today.

The warm start and the rule-bot opponents are scaffolding. Once this stage passes its gate, repeat the run with `train_lobby --pure` and no warm start. That run is the check that the method works with no hand-written play at all.

**Gate.** A pure network, with no search, beats 9kgamerbot on the fast ladder.

#### Stage 2 Progress (2026-11-21): the gate passes on short runs, and the long run is next

The changes are in. Each one is a flag, and `rl/README.md` lists the flags that restore the old behaviour.

| Change | Status |
| :--- | :--- |
| λ once per turn, γ = 1 | Done: `--lam-per turn` and `--gamma 1` are the defaults. |
| 8-way placement critic, linear atoms | Done for the MLP and the transformer: `--value-head placement`, the default. Old scalar checkpoints still load. |
| Shaping off | Done: `--shaping 0`. |
| Positioning out of the policy | `--no-reposition` plus `--auto-position`, a budgeted search with the combat simulator that names no cards. It measured no gain, so it is off by default (below). |
| Stronger warm start | Done: `pretrain --teacher 9kgamerbot` is the default, with the trainer's move-rule flags. |
| Wider league | Done: both 9kgamerbots stay in the field (`--bots`, `--p-bot 0.3`), and snapshots come from the whole history (`--pool-recent 4 --pool-history 4`). |
| Auxiliary head for future board strength | Not started. |
| Model chosen by a matched run | Done on a short run: the entity transformer wins by 1.7 places (below). |

The result files are in [`results/stage2/`](results/stage2/). All runs are on the Mac mini's CPU with 3 workers. The ± figures are one standard error.

**The warm start.** A clone of 9kgamerbot from 150 lobbies places 5.07 ± 0.07 on the fast ladder. The HeuristicBot clone from Stage 1 places 5.88. Two settings mattered.

| Clone of 9kgamerbot, 150 lobbies | Held-out action accuracy | Fast ladder |
| :--- | :--- | :--- |
| 4 epochs, critic trained | 74% | 6.03 ± 0.06 |
| 12 epochs, critic trained | 78% | 5.22 ± 0.06 |
| 12 epochs, critic untrained and zeroed | 77% | 5.07 ± 0.07 |

The cloned critic memorises each game's outcome. Every decision of a game shares its final place, so the old split by decision hid this. On whole held-out games its place cross-entropy rose from 2.17 after one epoch to 7.1 after twelve, against 2.08 for a uniform guess. PPO from that clone started with a value loss of 5.0, and its ladder place got worse, to 5.63 at iteration 25, before it recovered. `pretrain` now holds out whole games and, by default, leaves the critic untrained with its last layer zeroed. Memory caps the clone at about 150 lobbies: 157,000 decisions took a peak of 8 GB. Stage 3's packed storage would lift that cap.

**Positioning by search.** The 12-epoch clone placed 5.22 without `--auto-position`, 5.21 with it, and 5.25 with a search 16 times larger: 8 opponent boards, 8 fights each and 48 orders a turn. The moves were applied, about one a turn, and changed the outcome of 475 of 500 lobbies, but not the average. At this level of play, board order is not where the places are.

**A short PPO run passes the gate.** From the 12-epoch clone, with the new defaults, `--no-reposition` and `--lr 1e-4`, 150 iterations of 64 lobbies (9,600 games) took 34 minutes. The trainer's own fast-ladder check:

| Iteration | 25 | 50 | 75 | 100 | 125 | 150 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| Average place | 5.63 | 4.87 | 4.28 | 4.00 | 3.82 | 3.74 |

On a fresh seed the iteration-150 network places 3.82 ± 0.06. 9kgamerbot on the same seed places 4.34 ± 0.08. The paired difference is -0.52 ± 0.18 at two sigma, z = -5.7. The network plays without search, so this meets the gate as written. It was still improving by about 0.1 places every 25 iterations.

Its style is narrow. Its mean Tavern tier reaches 3.8 by turn 8 and stays there, and from turn 9 it rolls 4 to 7 times a turn. The anchors reach tier 5.5 to 6. Its probe is 0.60 to 0.74 from turn 6 to turn 11 and fades to about 0.5 after turn 12. That is a tempo strategy that out-tempos the rule bots. Whether it holds against stronger play is what the long run, the slow ladder and the `--pure` repeat should show.

Speed: an iteration took 7 seconds at the start and 13 seconds by iteration 150, as games got longer and snapshots took opponent seats. The update was 40% to 60% of it.

**The transformer is far stronger.** The matched run cloned 9kgamerbot into the default entity transformer (64 wide, 3 layers) the same way, then ran the same 150 iterations. Its clone already placed 4.82 ± 0.07, against 5.07 for the MLP clone. After PPO:

| Iteration | 25 | 50 | 75 | 100 | 125 | 150 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| Transformer | 3.48 | 2.66 | 2.44 | 2.22 | 2.18 | 2.04 |
| MLP | 5.63 | 4.87 | 4.28 | 4.00 | 3.82 | 3.74 |

The MLP run started from the clone whose critic had memorised its games, so the first 25 iterations are not a fair match. The gap at iteration 150 is too large for that to explain.

On a fresh seed the iteration-150 transformer places 2.08 ± 0.04, with 94% top 4 and 40% firsts. On the slow ladder it places 2.88 ± 0.08, second to SovereignBot at 1.55 ± 0.11, and well clear of the rule bots at 5.46 to 6.03. With 9kgamerbot in the agent seats SovereignBot placed 1.27 and took 81% of firsts; against the network it took 69%.

Its play differs from the MLP's. It often skips the turn-2 level, buying a second minion instead, and its probe on turn 2 is 0.78. It reaches a mean Tavern tier of 4.0 by turn 9 and keeps levelling, to 5.9. Its probe stays between 0.68 and 0.86 from turn 6 on, so it is not trading late strength for early strength.

It costs more to train. On 3 CPU threads its update took 20 to 32 seconds an iteration against 4 to 7 for the MLP, and the 150 iterations took 79 minutes. At that rate 100,000 games take about 14 hours on the CPU. The GPU gave the 64-wide transformer no speed-up in an earlier test, so that needs measuring before the long run.

Still to do in this stage: the 100,000-game run with the transformer, the `--pure` repeat and the auxiliary head.

### Stage 3: Raise Throughput (in Parallel)

The engine is fast and the Python around it is slow. Engine calls take about 3% of rollout time. Building the observation in Python takes 43% to 59%.

Device selection is already done. Since `f425ecd` the gradient update runs on MPS or CUDA through `rl/seaglass_rl/device.py`, and self-play stays on CPU workers. On MPS the update of a 224-wide, 6-layer transformer runs about twice as fast, and the small default models gain little.

What remains is the rollout path.

- **Move observation encoding and the legal-action mask out of Python.** `features.encode_observation` and `actions.action_mask` should write straight into NumPy buffers from Rust. Test the result for exact parity with the Python encoder.
- **Step many lobbies per call** and run the network once per batch of decisions.
- **Store observations packed.** The dense format costs 40 KB per decision and is over 97% zeros. The documented 400-game pretrain ran out of memory in a 15 GB container because of it.

The README's rule is that Rust never hardcodes a neural network feature. To keep that true for the engine, put the encoder in a separate crate or module that depends on the engine. Where it lives is an open design point for the maintainers.

The gate is 5 times because of where the time goes. Removing encoding, masking and packing from Python leaves about 30% of today's cost. Batched inference and multi-lobby stepping are what take it toward 10 times.

**Gate.** Network rollouts run at 5 times today's games per core, from a baseline of 3.6 games per second with the MLP.

#### Stage 3 Result (2026-11-22): done at about 2x

James called this stage done on 2026-11-22. The 5x gate is not met, for the reasons below. The forward pass is where later speed-ups should come from.

Three of the items are in.

- **The encoder and the mask are in Rust.** `seaglass.Encoder` lives in the Python-bindings layer (`engine/src/python/encode.rs`), not in the game logic. Python still owns the layout: `features.py` passes in the vocabularies, the action table and every offset, and the Rust side holds only the formulas. Its output is bit-identical to `features.encode_observation` and `actions.action_mask` over 17,815 checked decisions, Solo and Duos. One call takes 8 µs against 86 µs in Python. `PlayOptions(rust_encoder=False)` keeps the Python path, which stays the reference.
- **Trajectories are packed.** The slot features travel as their nonzeros (about 109 a decision), the mask as bits and the card ids as uint16. That is 1.5 KB a decision against 21.5 KB, and the encoder hands the nonzeros over directly. Cloning a bot on 40 games peaked at 0.9 GB instead of 2.7 GB, and a 16-lobby rollout process at 0.4 GB instead of 1.1 GB.
- **The baseline moved.** This Mac is about 2.5 times faster per core than the container the baseline came from, so the comparisons below are against the Python path on the same Mac. Each is one core with 8 recorded seats.

| Policy | Python encoder, dense | Rust encoder, packed | Speed-up |
| :--- | :--- | :--- | :--- |
| Transformer, 64 wide, 3 layers | 2.2 games/s | 3.8 to 4.2 games/s | about 1.8x |
| MLP, 256 wide | about 9 games/s (fresh weights) | 13.4 to 13.9 games/s (cloned weights) | about 2.5x |

The MLP figures come from different weights, because a fresh network loses fast and plays shorter games, so treat that row as approximate.

Batched multi-lobby stepping is not done, and it cannot reach 5x. The network's forward pass is now the largest cost: 43% of an MLP rollout and 42% of a transformer one. The batches are already about 100 decisions wide, because `play_games` runs every lobby in a worker in lockstep. What is left outside the network is about a third of the time.

Training is also not rollout-bound now. A transformer iteration of 64 lobbies spends about 6 seconds playing and 30 seconds in the update, and more CPU threads did not shorten the update. So the next speed-up for training is in the update: the GPU when it is free, larger minibatches, or fewer epochs. More rollout speed would not help much.

### Stage 4: Add Search Only Where the Simulator Is Exact

Search pays when the model of the world is exact. In this game that means combat and the rest of the current turn. The future shop is hidden and random, so deep multi-turn search mostly measures its own guesses.

The ladder shows what search inside the turn is worth. SovereignBot's search with a hand-written evaluator finished about four places ahead of the best rule bot. This stage keeps that strength while a learned critic replaces the hand-written evaluator.

- Ship the positioning solver as part of the environment, from Stage 2.
- Add policy-guided search within a turn. Score each line with the learned placement critic at the start of the next turn.
- Once the critic has seen at least 100,000 games, use the search as the teacher. The Gumbel-style targets in `architect/gumbel_az.py` are the right tool.
- Keep `belief_tavern` as the only way search sees the future. It is what keeps the search honest about hidden information.

This is the AlphaZero loop the repo is already reaching for. The difference is that the value function is learned from enough games first, and every step is judged on the ladder. Search comes after Stages 1 to 3 because it needs their critic and their speed.

**Gate.** Search plus net beats the net alone, outside the interval. Then the net distilled from that search beats the previous net.

### Stage 5: Freeze `bots/`

`bots/` should become a pinned reference, not the place where new work lands. Its evaluators hard-code card names and trinket priors from samples as small as 12 games, so part of it expires with every patch.

- Pin SovereignBot as the ladder's top anchor and as a teacher.
- Port four ideas into `rl/` with tests: the placement critic, turn-level advantage estimation, auxiliary heads and Gumbel targets.
- Keep hand-written card knowledge out of the training path. The static card features already give the network what it needs.

**Gate.** Nothing on the training path names a card.

### Not in This Sprint

- Duos.
- Keeping a trained network useful across patches.
- Play-time speed.
- Grading human replays.

Difficulty levels and play-style variety are not planned at all.

## 3. Why This Order: the Critique

The direction of the repo is right and the order of work was wrong. It is building toward an AlphaZero-style learner, which suits a game with an exact combat simulator. It added search and hand-written evaluators before it could measure strength or train at scale.

### What Kind of Game This Is

Six properties decide what a training method has to be good at.

- **It is mostly eight solo games that meet in combat.** A player's shop, Gold and board are their own. Opponents matter through pairings, a shared card pool and a few Hero Powers. What self-play mainly supplies is the field: how strong boards are on each turn.
- **The signal per game is weak.** A seat makes about 100 decisions and gets one place out of 8. Shop rolls, Discover offers, pairings and combat rolls all add luck. Training needs many games, and so does telling two agents apart.
- **The key trade-off pays off late.** Levelling the Tavern or buying a scaling engine costs strength now and returns it 3 to 6 turns later, if the player survives. Buying the most stats each turn is a strong local optimum, and it is where RL gets stuck.
- **The content is large and changes every few weeks.** The engine has 252 minions, 110 heroes, 236 trinkets and 47 Dark Gifts. Anything written per card expires with the patch.
- **State and actions are sets of cards.** The board, hand and shop are variable-size sets, and most actions point at a card. That fits an attention encoder with per-card action scoring better than a flat vector with 330 fixed logits.
- **The simulator is exact where search needs it.** Inside a turn, buying, playing, selling and positioning are deterministic. Combat against a known board runs at about 40,000 fights per second. The cross-turn question of tempo against greed needs a learned value.

### What the Repo Does Today

There is no earlier roadmap document, so this is read off the code and its history.

| Layer | What it is | Where |
| :--- | :--- | :--- |
| RL pipeline | Behaviour cloning of a simple rule bot, then PPO in full lobbies against its own snapshots and that bot. A flat MLP by default, an entity transformer as an option. | `rl/seaglass_rl`, about 4,600 lines |
| Search bots | Beam search over the bot's own turn in a fair copy of its Tavern, scored by a hand-written evaluator, combat rollouts and a small value network. Three generations: Beam, Apex, Sovereign. | `bots/seaglass_bots`, about 18,000 lines |
| Architect experiments | 14 bot variants and 11 training scripts: composition planners, cloning the search bots into the transformer, PPO against Sovereign seats, an 8-way placement critic, Gumbel search with self-play. | `bots/seaglass_bots/architect`, about 22,000 lines |

Three facts about the current state shape the rest.

- **The strongest bots are not neural.** SovereignBot averaged place 1.40 against the rule bots. It scores states with a hand-tuned evaluator that names specific cards, and its value network is a small correction weighted 0.2.
- **No trained policy ships with the repo.** Checkpoints are git-ignored, and the only committed weights are five small value networks.
- **The experiments' own notes report the failures.** Docstrings record that search "Goodharts" the value network and that self-play PPO settles into low-tier tempo play. They also record held-out placement cross-entropy of 1.6 to 1.98, against 2.08 for a uniform guess.

### What It Gets Right

- **The engine.** It is deterministic, exact and fast, with a scenario file per card and tools that replay real game logs beside the engine's version. A whole 8-player game costs about 15 ms of engine time.
- **The boundary between rules and ML.** Python never reimplements a rule. Bots see only what a player sees, through `context` and `belief_tavern`.
- **The entity transformer.** It has one token per card and scores each action from the cards it involves.
- **Static card features.** Engine tags plus a rules-text embedding let rare and new cards borrow from similar ones. This is the main defence against patch churn.
- **Careful MDP choices.** These include per-turn discounting, the prior over action kinds, a separate positioning step and a zero-sum placement reward.
- **The right ingredients, already prototyped.** The Architect experiments contain a placement critic, auxiliary targets, turn-level GAE, positioning by simulation and on-policy search targets.

### Where It Goes Wrong

Six problems, ranked by cost. The first two cause most of the others.

**1. Results are judged on too few games, against too weak a baseline.**

- One seat's place has a standard deviation near 2.3, so a 96-game average is good to about ±0.45 places.
- HeuristicBot is the weak rule bot. It averaged place 4.29 in mixed lobbies against 3.13 for 9kgamerbot.
- The arena script defaults to 6 games. Commit claims such as "beating all existing bots" have no script, seeds or intervals behind them. The ladder here bears that claim out, but nothing in the repo could have shown it.
- Hard-coded priors rest on tiny samples. `architect/hybrid.py` scores trinkets from records like "41.7% 1st across n=12".

**2. Training runs are 100 to 10,000 times too small.**

- Value networks train on 64 to 320 games by default. The Architect PPO and Gumbel trainers play 1,000 to 19,000 games in all.
- A game gives each seat one outcome. 320 games are 2,560 outcomes, spread over 110 heroes, 236 trinkets and 252 possible tribe sets.
- The reported failures follow from this: search exploits value errors, and held-out cross-entropy barely beats a uniform guess.
- The runs were small because the teachers are slow. The search bots are over 1,000 times slower than the rule bots.

**3. The Python wrapper wastes the engine, and a GPU alone does not fix that.**

- Python feature encoding takes 59% of rollout time with the MLP and 43% with the transformer. The engine takes 3%.
- That gives 3.6 games per second per core with the MLP. The engine could supply about 20 times more.
- The "GPU-vectorised" Architect trainer reports 1,500 to 3,000 games an hour. That is slower than one CPU core of the plain pipeline, because one Python process encodes every observation.

**4. The RL setup leans toward the tempo trap.**

- Credit assignment is short, because λ decays per action.
- Shaping pays for raw stats.
- The warm start clones a bot that buys the biggest stats and plays everything.
- The opponent pool is the last 10 snapshots plus 30% HeuristicBots. If the learner drifts to tempo, so does its whole field, and greed never pays.
- Fixes exist in `architect/` but never came back to the `rl/` trainer.
- Positioning is learned from the final place through 42 Reposition logits, although the simulator can score an ordering directly.

**5. `bots/` has become hard to build on.**

- It is about 40,000 lines in three copy-forward generations, and 22,000 of them landed in one commit.
- Hand-written knowledge sits at its core: a composition registry, hero and trinket tier lists, and card names inside evaluators.
- None of it can produce data at the scale a network needs.

It is worth keeping for two jobs: a strong reference opponent, and a slow but strong teacher.

**6. The objective was unsettled.**

`rl/` rewards place linearly. The search bots use first-place-heavy tables, for example 2.20 for 1st, 0.55 for 2nd and -1.00 for 8th. Section 1 now settles this for the neural opponent: linear.

## 4. Measurements

The ± figures are one standard error.

### Where Rollout Time Goes

Share of single-core rollout time, by policy network.

| Share of rollout time | MLP | Transformer |
| :--- | :--- | :--- |
| Building the observation in Python | 59% | 43% |
| Network forward pass | 17% | 40% |
| Packing the trajectory | 8% | 5% |
| Engine calls | 3.5% | 2.8% |

| Workload on one core | Games per second |
| :--- | :--- |
| 8 HeuristicBots, nothing recorded | 41 |
| 8 HeuristicBots, observations recorded | 4.8 |
| 8 seats of the MLP policy | 3.6 |
| 8 seats of the transformer policy | 2.5 |

A PPO iteration of 48 games on four cores spent 4.9 seconds playing and 12.8 seconds on the update. The update was about 72% of an iteration on CPU.

### How the Rule Bots Rank

400 mixed lobbies with two seats per bot, shuffled every game.

| Bot | Average place | Top 4 | First |
| :--- | :--- | :--- | :--- |
| 9kgamerbot | 3.13 ± 0.06 | 73% | 25% |
| 9kgamerbot-tempo | 3.13 ± 0.06 | 76% | 21% |
| HeuristicBot | 4.29 ± 0.05 | 50% | 4% |
| Random | 7.46 ± 0.02 | 0% | 0% |

### How the Search Bots Rank

48 lobbies with one SovereignBot, one ApexBot and two each of the three rule bots.

| Bot | Average place | Top 4 | First |
| :--- | :--- | :--- | :--- |
| SovereignBot | 1.40 ± 0.08 | 100% | 62% |
| ApexBot | 1.98 ± 0.16 | 98% | 35% |
| 9kgamerbot-tempo | 5.14 ± 0.18 | 44% | 1% |
| 9kgamerbot | 5.43 ± 0.20 | 33% | 0% |
| HeuristicBot | 5.75 ± 0.17 | 24% | 0% |

The two search bots together used about 75 seconds of CPU per game. A rule bot uses well under a second.

### What the Documented Pipeline Produces

Average place over 400 games, with the agent in one seat.

| Agent | Against 7 HeuristicBots | Against 7 9kgamerbots |
| :--- | :--- | :--- |
| HeuristicBot itself | 4.50 by symmetry | 5.48 ± 0.09 |
| Clone of HeuristicBot (120 games) | 5.51 ± 0.12 | 6.14 ± 0.08 |
| PPO from the clone, iteration 20 | 5.29 ± 0.11 | 5.81 ± 0.09 |

- The clone matches its teacher on 91.4% of held-out decisions and still finishes last in 118 of 400 games. Small errors compound over about 100 decisions per game.
- Twenty PPO iterations, about 960 games, recovered a quarter of a place. The policy still sat on Tavern tier 2 through turn 5 and left 3.5 Gold unspent per turn.
- The trainer's own 96-game check scored that snapshot at 4.98 ± 0.23. The 400-game run put it at 5.29, so the built-in check was off by more than the whole gain.

### How These Were Taken

The scripts were throwaway and are not checked in. Stage 1 replaces them, and its first job is to reproduce these tables.

- **Speeds.** One process with `torch.set_num_threads(1)`, `play.play_games` on a default `LobbyConfig()`, eight seats of one controller. Shares come from `cProfile`.
- **Ladders.** `play.Player.play` with `GameSpec(seed=base + g, controllers=shuffled)` and `record=()`. The rule-bot ladder used base seed 5,000,000 and the search-bot ladder 6,000,000. Seats were shuffled with `random.Random(base)`.
- **Agent results.** `evaluate.evaluate(agent, [opponent], LobbyConfig(), games=400, agent_units=1, seed=2_000_000)`.
- **Clone.** `python -m seaglass_rl.pretrain --out runs/bc-solo.pt --games 120 --workers 4 --threads 4`.
- **PPO.** `python -m seaglass_rl.train_lobby --run-dir runs/solo --init runs/bc-solo.pt --lr 1e-4 --workers 4 --threads 4 --games 48 --eval-every 20`, stopped after iteration 20.

## 5. Caveats

- Every timing comes from a four-core cloud container. The ratios should carry over to other machines, and the absolute numbers will not.
- The search bots were rated on 48 games. The four-place gap is safe at that sample size, and the exact figures are not.
- The clone used 120 games because the documented 400 ran out of memory. The PPO run stopped at iteration 20. Both show where the pipeline starts, not its ceiling.
- The checkpoints that the `architect/` scripts refer to are not in the repo, so the neural policies they describe could not be rated.
- `bots/` was read through its entry points, trainers and docstrings, not every line.
- The sprint is a recommendation from measurements and code reading. None of its stages has been implemented or tested yet.
