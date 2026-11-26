# Policy and Value by Game Phase, and What an Endgame Solver Can Be

*Written 2026-11-21. Measurements were taken at commit `31ce92b` in a 4-core Linux container, in lobbies of rule bots, on the default Solo lobby.*

This document answers two questions about the neural opponent of the [sprint plan](neural-opponents-sprint.md).

- How much will the policy network and the value network each matter early, in the middle and late in a game?
- Can Battlegrounds have an endgame solver, or training that starts from late positions?

It is laid out as follows.

- [Section 1](#1-the-short-answer) is the short answer.
- [Section 2](#2-what-makes-a-network-matter) says what makes each network matter.
- [Section 3](#3-measurements) measures it, by forking every turn of 300 bot lobbies into 16 futures.
- [Section 4](#4-phase-by-phase) is the opinion, phase by phase.
- [Section 5](#5-why-the-late-game-comes-first) says why the late game has to be good first.
- [Section 6](#6-is-an-endgame-solver-possible) answers the solver question, and [Section 7](#7-starting-from-late-positions) covers training from late positions.
- [Section 8](#8-what-to-do-next) lists what to do, and [Section 9](#9-caveats) says how far to trust all this.

Sections 4 to 8 are opinion backed by the measurements and the code. Nothing here has been tried in training yet.

## 1. The Short Answer

- **The two networks matter at different times.** The policy matters as the search's guide wherever one turn holds more action sequences than a search can try. That grows all game: a seat takes 3.5 actions a turn early, 10 in the middle and 16 late. The value network matters where a decision pays off past what the simulator can see, and that is the middle of the game.
- **Early, turns 1 to 4: the policy, as a memorised opening.** A turn is small enough to enumerate, so a search needs no guide. The turns do matter: skipping one costs 0.6 to 0.8 places. But nothing can judge them sharply, because 92% of a value target at these turns is luck that has not happened yet. No search belongs here.
- **Middle, turns 5 to 9: the value network.** This is where the game is decided. The final place is 13% settled at the start of turn 5 and 78% settled at the start of turn 10. The choices are level or roll, which direction, which trinket. They pay off three to six turns later, the simulator cannot score them, and the turn is now too big to enumerate. Both networks are needed, and the value network is the bottleneck.
- **Late, turn 10 on: search and the simulator, guided by the policy.** The horizon is short, fight odds are exact, and playing a position out to the end costs 5 to 35 ms. A learned value is least needed here. The turn is at its largest, so the policy matters most as a guide.
- **An exact endgame solver is not possible. A rollout solver is, and it is cheap.** The engine could already copy a lobby, but a copy replayed the same future. `LobbyGame.reseed` was added with this document. It deals a copy a different future, and the study in Section 3 is its first use.
- **Late-game skill does come first, for a sharper reason than noise.** An agent with weak late play really does get less from a greedy mid-game board, so it learns tempo. That is a bias, and more games do not fix it.
- **Two cautions from the numbers.** The final two is a poor place to start training: two thirds of those positions are already decided. And among rule bots a late turn is worth only 0.05 to 0.4 places, so how much late-game skill is worth has to be measured before it is built on. [Section 8](#8-what-to-do-next) says how.

## 2. What Makes a Network Matter

In an AlphaZero-style player a search looks ahead. The policy network tells it which moves are worth trying, and the value network scores the positions where it stops. Three properties of a position decide how much each one matters.

| Property | When it is high | What carries the load |
| :--- | :--- | :--- |
| Action sequences inside one turn | The search cannot try them all | The policy, as the search's guide |
| Distance from a decision to its payoff, past what the simulator can see | The search has to stop and judge | The value network |
| Luck between the position and the final place | A value target is mostly noise | Neither network. Only more samples help |

Chess and Go have the first two. Battlegrounds adds the third, and it adds something those games lack: an exact simulator for part of the future.

A turn in this engine is a one-player planning problem with a fight at the end (`engine/docs/lobby_game.md` §2).

1. `LobbyGame.begin_phase` deals every shop and draws the pairings. Each player now knows their next opponent.
2. The player acts until End Turn. Buy, Play, Sell and Reposition are deterministic. Refresh, Discover and some card effects are chance.
3. `LobbyGame.end_phase` fights every pair. Combat has no decisions, only dice.

So there are three kinds of future, and they need different tools.

| Future | Known to the player? | Tool |
| :--- | :--- | :--- |
| The rest of this turn, between rolls | Exactly | Search |
| This turn's fight, against the board the opponent had last time | Exactly, by sampling (`seaglass.simulate_batch`, about 40,000 fights per second) | Simulation |
| This turn's rolls and Discovers | In distribution (`LobbyGame.belief_tavern` deals a fair copy of the player's Tavern) | Sampling |
| What the opponent is building right now | No | A belief, or a best response to several guesses |
| Every later turn | No | The value network, or rollouts |

The natural place for a search to stop is the end of the turn. The value network's question is then "how good is this end-of-turn state, counting everything after the fight".

## 3. Measurements

### The Fork Study

The engine can copy a whole lobby (`LobbyGame.copy`), and a copy costs under 0.1 ms. A copy carries the same random state, so it replays the same future. `LobbyGame.reseed`, added with this document, deals the rest of the game from another seed and leaves everything already on the table alone. Copies of one position, each reseeded and played on, are independent samples of how the game goes from there.

`rl/scripts/phase_study.py` uses this.

- It plays 300 lobbies of rule bots: four `9kgamerbot` and four `9kgamerbot-tempo`, which place level in the sprint plan's [ranking](neural-opponents-sprint.md#how-the-rule-bots-rank).
- At the start of every turn it forks the lobby 16 times and plays every fork to the end.
- It forks 16 more times with one living seat ending that turn at once, to price a turn.

For a seat still in the game, the spread of its final place across the 16 forks is **noise**: the part of the outcome that the position does not decide. No critic can explain it, whatever it knows. The spread of the fork averages across positions is **signal**: what a critic could know.

### How to Read the Tables

- **Noise** is the standard deviation, in places, of a living seat's final place across the forks of one position. A whole game has 2.29.
- **Signal** is the standard deviation, across positions, of a living seat's expected place.
- **Critic ceiling** is signal over signal plus noise, as variances. It is the most that a critic could explain of a final-place target at this turn, even one that sees the whole lobby.
- **Settled** is the share of the lobby's final-place variance that is already decided. A seat that is out, and a game that is over, count as decided.
- **A skipped turn costs** is the places a living seat loses by ending this turn at once, with one standard error.

### How Settled the Game Is, by Turn

| Turn | Players left | Noise | Signal | Critic ceiling | Settled | A skipped turn costs |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| 1 | 8.0 | 2.21 | 0.60 | 7% | 7% | 0.62 ± 0.05 |
| 2 | 8.0 | 2.21 | 0.59 | 7% | 7% | 0.76 ± 0.05 |
| 3 | 8.0 | 2.21 | 0.59 | 7% | 7% | 0.70 ± 0.05 |
| 4 | 8.0 | 2.17 | 0.75 | 11% | 11% | 0.58 ± 0.05 |
| 5 | 8.0 | 2.14 | 0.81 | 13% | 13% | 0.88 ± 0.04 |
| 6 | 8.0 | 1.99 | 1.14 | 25% | 25% | 0.85 ± 0.05 |
| 7 | 8.0 | 1.84 | 1.36 | 35% | 35% | 0.72 ± 0.05 |
| 8 | 7.9 | 1.73 | 1.48 | 42% | 43% | 0.74 ± 0.05 |
| 9 | 7.5 | 1.41 | 1.69 | 59% | 64% | 0.73 ± 0.05 |
| 10 | 6.8 | 1.17 | 1.65 | 67% | 78% | 0.42 ± 0.04 |
| 11 | 5.8 | 0.96 | 1.48 | 70% | 87% | 0.30 ± 0.04 |
| 12 | 4.6 | 0.75 | 1.30 | 75% | 94% | 0.26 ± 0.03 |
| 13 | 3.5 | 0.55 | 1.07 | 79% | 97% | 0.14 ± 0.02 |
| 14 | 2.7 | 0.42 | 0.81 | 79% | 99% | 0.08 ± 0.01 |
| 15 | 2.3 | 0.33 | 0.65 | 80% | 100% | 0.05 ± 0.01 |

- **The first five turns barely move the outcome.** At turn 1 the position (the hero, the first shop, the seat's bot) explains 7% of a seat's final place. At the start of turn 5 it explains 13%.
- **Turns 5 to 9 decide it.** The settled share goes from 13% to 78% across those five turns. Almost nobody is out yet: 7.9 players are left at turn 8.
- **A turn of play is worth about the same through turn 9**, 0.6 to 0.9 places. Then it falls fast.
- **By turn 12 the game is 94% settled**, with 4.6 players left.

A game lasts 15.0 turns on average, and between 12 and 19. Four or fewer players are left from turn 12.6 on average, for 3.4 turns. The final two starts at turn 14.5 and lasts 1.4 turns.

### How Big a Turn Is

| Turn | Actions per turn | Rolls per turn | Legal actions per decision | Fights decided | One rollout to the end |
| :--- | :--- | :--- | :--- | :--- | :--- |
| 1 | 3.1 | 0.0 | 5 | 30% | 60 ms |
| 2 | 2.1 | 0.0 | 7 | 32% | 59 ms |
| 3 | 4.4 | 0.0 | 7 | 56% | 59 ms |
| 4 | 4.4 | 0.0 | 11 | 72% | 58 ms |
| 5 | 5.2 | 0.0 | 13 | 77% | 57 ms |
| 6 | 9.0 | 0.3 | 16 | 69% | 56 ms |
| 7 | 10.3 | 0.6 | 19 | 63% | 53 ms |
| 8 | 11.8 | 1.2 | 20 | 70% | 48 ms |
| 9 | 15.9 | 1.9 | 20 | 72% | 41 ms |
| 10 | 14.9 | 2.4 | 21 | 72% | 33 ms |
| 11 | 17.5 | 4.0 | 22 | 77% | 25 ms |
| 12 | 17.7 | 4.7 | 22 | 79% | 18 ms |
| 13 | 14.8 | 3.7 | 22 | 84% | 11 ms |
| 14 | 16.5 | 5.5 | 21 | 85% | 8 ms |
| 15 | 16.3 | 6.0 | 21 | 87% | 6 ms |

- **Actions per turn** are a rule bot's. They average 16 late, and one turn in ten takes 25 or more. A model's turn is capped at 30 actions (`PlayOptions.max_steps_per_turn`), which 2% of the bots' late turns exceed. Stronger late play uses more actions than these bots do, so that cap is worth watching.
- **Legal actions** leave out Repositions. A full board alone has 42 of those, and the trainer already moves them to a separate end-of-turn step.
- **Fights decided** is the share of fights where the favourite's win rate is at least 0.8 above the underdog's, board against board. Fights are near coin flips on turns 1 and 2, and mostly one-sided from turn 4.
- **One rollout to the end** is with rule bots in every seat, on one core.

### By Phase

| Phase | Turns | Noise | Critic ceiling | Actions per turn | Rolls per turn | Legal actions per decision |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| Early | 1 to 4 | 2.20 | 8% | 3.5 | 0.0 | 8 |
| Middle | 5 to 9 | 1.84 | 34% | 10.4 | 0.8 | 18 |
| Late | 10 on | 0.86 | 76% | 16.2 | 4.0 | 21 |

Scored over every decision of every game, the way a PPO log scores its critic, the ceiling is 52%. A critic near that figure has learned what there is to learn about these games. A critic's early-turn outputs should be nearly flat, and that is correct.

### By Players Left

| Players left | Typical turn | Noise | Signal | One rollout to the end |
| :--- | :--- | :--- | :--- | :--- |
| 8 | 4.9 | 2.02 | 1.08 | 55 ms |
| 7 | 9.8 | 1.27 | 1.55 | 37 ms |
| 6 | 10.8 | 0.96 | 1.41 | 26 ms |
| 5 | 11.7 | 0.78 | 1.18 | 22 ms |
| 4 | 12.6 | 0.57 | 0.96 | 14 ms |
| 3 | 13.8 | 0.43 | 0.69 | 10 ms |
| 2 | 14.9 | 0.25 | 0.43 | 5 ms |

**The final two is mostly over before it starts.** At the first turn with two players left, the favourite wins 93% of the forks on average.

| Final-two position | Share |
| :--- | :--- |
| Decided: all 16 forks have the same winner | 65% |
| Leaning: the favourite wins 12 to 15 of 16 | 23% |
| Open: the favourite wins fewer than 12 of 16 | 12% |

With four or fewer left it is more open: 24% of seats finish in the same place in every fork, and the rest still move.

### A Second Field

The same study on the ladder's own mix of bots (three `9kgamerbot`, three `9kgamerbot-tempo`, two HeuristicBots, 120 lobbies) has the same shape. The game is 10% settled at turn 1, 19% at turn 5, 81% at turn 10 and 94% at turn 12.

### How These Were Taken

```bash
python rl/scripts/phase_study.py --games 300 --forks 16 --workers 4 \
    --out docs/results/phase-study/summary.json
python rl/scripts/phase_study.py --games 120 --forks 16 --workers 4 --no-skips --seed 7500000 \
    --field 9kgamerbot,9kgamerbot-tempo,heuristic --out docs/results/phase-study/summary-ladder-field.json
```

The result files are in [`results/phase-study/`](results/phase-study/). The first run took 24 minutes on four cores. The base lobbies use seeds 7,000,000 to 7,000,299, and fork `k` of turn `t` is reseeded from the lobby's seed, `t` and `k`, so a run reproduces exactly apart from its timings.

## 4. Phase by Phase

| | Early, turns 1 to 4 | Middle, turns 5 to 9 | Late, turn 10 on |
| :--- | :--- | :--- | :--- |
| The decisions | The curve: level or buy, which minion, the Hero Power | Level or roll, which direction, the trinkets, Discovers | Run the engine: rolls, triples, the order of plays, positioning, lethal |
| The policy, as a guide to search | Not needed | Needed | Needed most |
| The value network | Flat, and buried in luck | The bottleneck | Least needed |
| What can stand in for a network | Opening tables made by rollouts | Nothing | The simulator and rollouts |
| Search | None | Once the critic is near its ceiling | First, and with rollouts |

This matches the human reading of the game that prompted the question: tempo and levelling early, finding a direction and a scaling engine in the middle, then optimising that engine and collecting small edges late. The measurements add where each network earns its keep.

### Early: Turns 1 to 4

- **The policy is not needed as a guide.** A turn has 2 to 4 actions with 5 to 11 legal choices each and no rolls. A search could try every line.
- **The decisions are value decisions, and the stakes are real.** Skipping a turn costs 0.6 to 0.8 places, as much as skipping a mid-game turn.
- **But the value cannot be learned sharply.** A target at these turns is 92% luck. Telling apart two opening lines that differ by 0.1 places, at two sigma, takes about 3,900 rollouts each. That is four minutes of one core per line.
- **So a search here would be choosing by noise.** The lines differ by less than any learned critic's error. This is the failure the Architect experiments call Goodharting.

What plays the early game well is a policy that has memorised the curve for each hero. It gets there slowly, by averaging over a long run, and it can only be as right as the play that follows it. The early game is learned last.

If opening play ever needs checking, rollouts can do it offline. An opening line can be priced to 0.1 places in about four minutes of one core, and the answer is exact for the policy that plays the rest of the game.

### Middle: Turns 5 to 9

- **This is where the game is decided.** The final place goes from 13% settled to 78% settled in five turns, with nearly every player still in.
- **The decisions pay off late.** Levelling, rolling for a build-around card and picking a trinket (turns 6 and 9) cost strength now and return it three to six turns later. Board strength now is the wrong measure: `9kgamerbot` gives up strength on turns 4 to 6 and is the strongest bot from turn 8.
- **Only a value function can score them.** The simulator sees this turn's fight. It does not see whether the engine comes together.
- **The value function now has something to learn.** The critic ceiling rises from 13% to 59% across these turns.
- **The policy becomes necessary too.** A turn grows from 5 to 16 actions with about 18 legal choices each, and rolls begin. That is too many lines to enumerate.

The value network is the bottleneck here. A good guide with a poor judge searches confidently in the wrong direction, and a search with a weak critic is worse than no search. The pick-one decisions sit here too: the trinkets and most Discovers. They have two to four options, so no guide is needed and the value decides alone.

### Late: Turn 10 On

- **A learned value is least needed.** Once four players are left the game has 3.4 turns to run. Fight odds against a known board are exact. A rollout to the end costs 5 to 35 ms, and the noise on it is a quarter of the early game's.
- **The policy is needed most, as a guide.** A turn has 15 to 18 actions and about four of them are rolls. It cannot be planned as one sequence. It is a loop of roll, look and decide, and each "roll again or stop" needs a guess about shops not yet seen.
- **This is where search is strongest.** Everything inside the turn can be simulated, and what cannot be seen is short.
- **Telling two late lines apart is cheap.** The same 0.1 places takes about 260 rollouts with four players left, or four seconds of one core. That is over 60 times cheaper than at turn 1.

The small edges of late play are the ones the simulator scores directly: which minion gets Taunt, where the Cleave lands, whether a Venomous minion is worth a slot. The Stage 2 work found no measurable gain from an end-of-turn positioning solver against the rule bots. These edges are tie-breakers between boards of equal strength, so they will matter against strong opponents and not before.

One caution. Among rule bots a late turn is worth little: 0.42 places at turn 10 and 0.14 at turn 13. That may be the game, or it may be these bots, whose boards stop growing. A player with a real scaling engine may be able to overturn positions that are settled between rule bots. The study cannot tell which, and [Section 8](#8-what-to-do-next) proposes the test.

### Compared with Chess and Go

The quoted advice says the policy matters most in Go, because of the branching, and the value matters most in chess, above all early.

- **Like Go, but only late.** The branching that makes a policy matter is small early here and large late.
- **Unlike chess, the early value is not usable.** It matters in principle and is buried in luck.
- **Unlike both, the late value can be computed.** The simulator and rollouts replace most of it.
- **The chess warning carries over.** A policy can be a burden when the right move looks bad: a sacrifice in chess, and here levelling on an empty board or selling a strong minion for a build-around card. A policy learned from tempo play ranks those low, and a search with few tries never looks at them. The fix is the same as in chess: more search, and a value network that has seen greed pay.

## 5. Why the Late Game Comes First

The claim to test is this: if the agent is not yet good at the late game, training on the early and middle game is very noisy. The measurements split it into two effects. The second one matters more.

- **Noise.** A value target at turn N carries all the luck after turn N. That luck belongs to the game. Better late play does not remove it, and only more samples average it out. This is the noise column of [Section 3](#3-measurements).
- **Bias.** A position is worth what the play that follows makes of it. An agent that cannot convert a scaling board (it misses triples, stops rolling too early, levels at the wrong time) really does get less from a board that gives up strength now for strength later. It then learns, correctly for itself, that greed does not pay. That is the tempo trap of the [sprint plan](neural-opponents-sprint.md#3-why-this-order-the-critique).

A value learned on top of weak late play is not noisy. It is wrong, and more games do not fix it.

The Stage 1 probe shows the trade is real. `9kgamerbot` gives up board strength on turns 4 to 6 and is the strongest bot from turn 8. It can afford that because it has a late plan. The first PPO run measured in the sprint plan had none, and it sat on Tavern Tier 2 through turn 5.

So late-game skill is the foundation, and the order to build and check things is from the end of the game backwards. Two notes on how this applies here.

- **Stage 2's PPO is mostly protected from the bootstrapping version of the problem.** With λ applied once per turn and γ at 1, its targets lean on the real final place far more than on its own critic's guess. It still has the bias above.
- **Stage 4 is not protected.** A search that stops at the end of the turn and asks the value network is bootstrapping in exactly the sense of the quoted advice. Its value network has to be right late before it can be trusted earlier.

## 6. Is an Endgame Solver Possible?

Not an exact one. Something that does the same job is possible, and most of its parts exist.

### Why Not an Exact Solver

- **There is no small endgame.** Chess tablebases work because few pieces are left. In Battlegrounds the late game is the largest state of the game: seven minions a side with unbounded stats, a hand, a shop, a Hero Power and trinkets. There is nothing small to enumerate backwards from.
- **The finalists move at the same time and cannot see each other.** Each knows the other's board from the last fight, not what they are buying now. The exact answer to such a game is a mix of plans, not a best move. Positioning is the clearest case: where to put the Taunt depends on where the opponent puts the Cleave.
- **Chance is wide.** A roll draws up to six cards from a shared pool, and late turns hold about four rolls.

### What a Solver Is For, and What Stands In for It

| Wanted from a solver | Here | In the code |
| :--- | :--- | :--- |
| The exact result of a final position | Fight odds for two boards, to any precision | `seaglass.simulate_batch`, `TavernGame.equity_against` |
| "Mate in one" | Lethal this turn: the chance to win the fight by at least the opponent's Health | The odds exist. The damage tail does not: `BattleDistribution` returns only the minimum, maximum and mean damage (`engine/src/sim.rs`) |
| The best move against a known reply | The best end-of-turn board against the opponent's last board | `SovereignEvaluator.evaluate_turn_end` does this with hand-written scoring. The version with no game knowledge is Stage 4 |
| The best order of the board | Search the orders, scored by simulation | `bots/seaglass_bots/sovereign_positioning.py`, and the end-of-turn solver of the Stage 2 work |
| The true value of a late position | Rollouts: fork the position and play it out many times | New: `LobbyGame.copy` plus `LobbyGame.reseed` |

The last row is the new part. A rollout from a position with four players left takes 14 ms between rule bots, and one from the final two takes 5 ms. Sixteen of them pin a late position's value to about ±0.15 places. No learned value function is involved.

### Solver v0: Rollouts in Place of the Value Network

Late in the game, a search does not need a value network at all.

1. **Candidates.** Sample N whole-turn plans from the policy. Play each in a `LobbyGame.belief_tavern` copy, so that its rolls are fair draws and not the real future.
2. **Score.** Fight each plan's final board against the opponent's last board. Add K rollouts from the state after the fight for everything that comes later.
3. **Act and learn.** Play the best plan. The scores are also a training target for the policy, the same way the Gumbel targets of Stage 4 are.

With 16 candidates and 8 rollouts each, that is about 128 rollouts for one turn. At rule-bot speed it is about two seconds for a position with four players left. That is slow for play and fine for making training targets. With the network as the rollout policy it needs the throughput work of Stage 3.

Backgammon programs have worked this way since the 1990s: the network plays, and rollouts are the ground truth it is checked against and improved with. Backgammon is the closest classical relative of this problem, because it too has dice between every decision.

### Hidden Information

There are two uses, with different rules.

- **Training targets** may use the true lobby: the real opponents, the real pool. The fork study does this. A critic that sees more than the player is safe, because only training uses it.
- **Play** may use only what the player sees. `LobbyGame.belief_tavern` gives the player's own Tavern with an honest pool, and `LobbyGame.context` gives each opponent's board as last fought (`seen_board`). The opponent's current turn has to be guessed.

The final two is the friendliest case for play. There is one opponent, fought every turn, so the board on file is one turn old. With three or four left, the next opponent's board may be two or three turns old.

A search that best-responds to one remembered board can be exploited by a player who changes it. SovereignBot fights several guessed boards (`sovereign_belief.py`), and a learned version should too. A full game-theoretic treatment of final-two positioning is possible: with both sets of minions fixed it is a table of orders against orders, each cell filled by simulation. It is low on the list, because positioning by simulation showed no measurable gain against the rule bots in the Stage 2 work.

## 7. Starting from Late Positions

The quoted advice is to pick starting positions near the end of the game, train there, keep them as tests, avoid positions that are already won, and move the start earlier as things work. All of it can be done here.

### Where the Positions Come From

Any game will do: a bot lobby or the learner's own. Play to the turn wanted, or until a set number of players are left, and copy the lobby. No human games are needed. The learner's own games give the same benefit the advice sees in human data: the starting point can slide toward turn 1 one turn at a time.

One thing is missing. A `LobbyGame` cannot be saved to disk or sent to a worker process, so today a library of positions has to be rebuilt in each process by replaying its games. That is cheap for bot games, about 50 ms per lobby. The replay format already records every action (`engine/proto/replay.proto`), so a `LobbyGame` built from a replay up to a given turn is the natural engine addition.

### Avoiding Decided Positions

The advice warns against too many won positions, and the warning applies strongly here.

- **The final two is a poor starting point.** In 65% of final-two positions every fork has the same winner, and only 12% are open.
- **Four players left is better.** Most seats can still move, the horizon is 3.4 turns, and the noise is 0.57 places.
- **The filter is cheap.** Fork each candidate position a few times and keep the ones whose forks disagree. A position's own spread is the natural weight for sampling it.

The linear objective helps. With one point per place there is something to play for until the final two, so a search that has lost first place can still fight for second. In the final two the problem returns, because one place is all that is left.

### What Late Starts Improve

- **Targets.** The average of K forks replaces one outcome, so the target's noise falls by the square root of K. Late forks are cheap.
- **Comparing actions from the same position.** With a simulator that can be reset, two actions can be tried from the identical position. Trust-region policy optimisation called this the vine method. Few game environments allow it. This one does.
- **Tests.** Two agents can be rated from the same late positions. The noise per game there is 0.57 places against 2.3 for a whole game, so the same precision in places takes about 16 times fewer games, and each of them is four times shorter. Stage 4's gate, search plus net against net alone, could be decided this way for the late game.

### What They Do Not Improve

Throughput. Late turns are already where the time goes: a seat takes about 16 actions a turn late against 3.5 early. A rollout from four players left costs 14 ms and a whole game 60 ms. Late starts buy cleaner signal, not many more decisions per second.

### Risks

- **Positions from bot games are not the learner's positions.** Mix the sources, and refresh the library from the learner's recent games.
- **A rule bot cannot take over a seat in the middle of a game.** `9kgamerbot` remembers the direction it committed to. The network has no memory, so it can take any seat at any time.
- **Small games mislead.** `LobbyConfig` already allows 2 to 8 players, fewer minion types, no heroes, no trinkets and a turn limit. A two-player, two-type lobby is a fast check that a training loop learns anything. As the advice says, what it learns there should not be assumed of the full game.

## 8. What to Do Next

In order. Items 1 to 5 need no training run.

| # | Step | Why | Cost |
| :--- | :--- | :--- | :--- |
| 1 | Keep `LobbyGame.reseed` | Every item below needs forks | Done with this document |
| 2 | Re-run the fork study with the Stage 2 network in the seats | Every number here comes from rule bots. The low stakes of late turns may be a property of those bots | A small change to the script, then an hour or two of CPU |
| 3 | Report the critic's explained variance by turn, next to the ceiling in [Section 3](#3-measurements) | Shows where the value network is weak, and whether the Stage 2 critic is already near what the game allows | Hours, on games already played |
| 4 | Build a late-position ladder: open positions at the first turn with four or fewer players, rated by forks | A low-noise instrument for everything late | A day |
| 5 | Try solver v0 on that ladder: sampled plans scored by fight odds and rollouts, against the raw policy | It is the first half of Stage 4's gate, it needs no value network, and it says how much late-game skill is still on the table | Two or three days |
| 6 | If item 5 wins, train on its targets from late positions first, then move the start earlier | Builds the late game the rest depends on | A training run |
| 7 | Score the pick-one decisions by forks, offline: the hero, the trinkets on turns 6 and 9, each Discover | These are pure value decisions with two to four options. Forks give a clean target, and show how often the policy picks the best | A day |
| 8 | Two engine additions: the damage tail in `BattleDistribution`, and a `LobbyGame` rebuilt from a replay | Exact lethal odds, and position libraries that can be saved | Small |

Items 2 and 5 are the test of the late-game-first order. If a stronger field still shows late turns worth little, and the best of 16 sampled plans barely beats the policy's own, then late-game skill is not where the points are. The effort should then go straight to the middle game's value function.

This suggests one change to the sprint plan. Stage 4 today is one step: search inside the turn, scored by the learned critic. It would be safer as two.

- **Stage 4a: the late game, with rollouts and no critic.** It can start as soon as the policy is good enough to supply candidate plans, and it cannot be fooled by a weak value network.
- **Stage 4b: the middle game, with the critic.** It starts when item 3 shows the critic close to its ceiling there.

Early turns need no search at all.

## 9. Caveats

- **The field is two rule bots.** Every number in Section 3 describes games between `9kgamerbot` and `9kgamerbot-tempo`. A stronger or more varied field will move them. The shape, with noise falling and tree size rising through the game, comes from the rules and should hold.
- **The rule bots are deterministic.** All the noise measured here is the game's. A sampling policy adds its own.
- **A skipped turn is a blunt tool.** It prices a whole turn of play against none. The gap between a good turn and the best turn is smaller, and nothing here measures it.
- **The ceiling is for one kind of critic.** It assumes a critic that sees the whole lobby. A critic that sees only what the player sees has a lower ceiling.
- **The fight edge is board against board.** Like the Stage 1 probe, it leaves out Hero Powers and trinkets.
- **Timings come from a four-core cloud container**, with rule bots as the rollout policy. A network policy is slower until the Stage 3 work lands.
- **Solver v0, the late-position ladder and late-start training are proposals.** None has been built.
