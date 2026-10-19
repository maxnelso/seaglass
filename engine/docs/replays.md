# Match Replays

A **replay** records one match so it can be stepped through later, one engine call at a time,
in both directions. It stores the match's *inputs* (its setup, its seeds and every engine call
in order), not its states. A viewer re-simulates the calls to rebuild every state, so a replay
is only about 2–7 KB per game.

Re-simulation is exact because the engine is deterministic (`README.md`, Design Principles).
It is only guaranteed with **the engine build that recorded the replay**. A replay records that
build (`Header.engine`, e.g. `0.1.0+708acd9e1b2c`). Replays are not meant to survive engine
changes, so keep the commit if you want to keep the replay. Uncommitted changes are not part of
the stamp.

---

## 1. Recording: `Match`

`seaglass::Match` (`src/game.rs`) owns every player's `TavernState` together with the card pools
and RNGs they draw from and the `Lobby` they see each other through (`lobby.rs`, `tavern.md`
§3.1). It records each call as the game loop makes it:

| Call                                  | Engine call                                    | Record         |
| ------------------------------------- | ---------------------------------------------- | -------------- |
| `set_pairs(&[(a, b), ...])`           | `Lobby::set_next_pairs`, then `Lobby::apply`   | `SetPairs`     |
| `start_turn(p)`                       | `TavernState::start_turn`                      | `StartTurn`    |
| `deliver_gifts()`                     | `Lobby::deliver_gifts`                         | `DeliverGifts` |
| `step(p, action)` / `step_with_note`  | `TavernState::step`                            | `Act`          |
| `combat(a, b)` / `combat_with_seed`   | `TavernState::resolve_combat_pair`, then `Lobby::record_combat` (both players) and `Lobby::record_outcome` | `Combat` |
| `record_deaths()`                     | `Lobby::record_deaths` (alive = Health > 0)    | `RecordDeaths` |
| `finish()`                            | none: marks the end and names the winner       | `End`          |

A turn of a game loop is: `set_pairs`, every player's `start_turn`, `deliver_gifts`, the players'
actions, `deliver_gifts`, the combats, `record_deaths` (`sim_match`, the RL env). A loop that never
calls `set_pairs` never shows the players the lobby, as with bare `TavernState`s.

```rust
use seaglass::{DeityKind, Match, MatchSetup, PlayerSetup, PoolMode};

let mut game = Match::new(&MatchSetup {
    pool: PoolMode::PerPlayer,
    players: vec![
        PlayerSetup { name: "agent".into(), seed: 42, deity: DeityKind::CThun, ..Default::default() },
        PlayerSetup { name: "bot".into(), seed: 43, deity: DeityKind::YShaarj, ..Default::default() },
    ],
    ..Default::default()
})?;
game.set_pairs(&[(0, 1)]);
game.start_turn(0);
game.start_turn(1);
game.deliver_gifts();
game.step(0, action)?;
// ...
game.deliver_gifts();
game.combat(0, 1);
game.record_deaths();
game.finish();
game.save("replays/game.replay.pb")?;
```

- **Players.** `PlayerSetup` has the options of the Python `TavernGame`: name, seed, Deity, Armor,
  shop spells, `hero` (its Health, Armor and Hero Power; `heroes.md`), `trinkets` and
  `dark_gifts`. Its defaults are `TavernGame`'s: no Deity, no hero, 5 Armor, shop spells,
  trinkets and Dark Gifts on.
- **Pools and seeds.** In `PoolMode::PerPlayer`, every player has their own card pool and an RNG
  seeded by `PlayerSetup::seed`. This is how the Python `TavernGame` works. In `PoolMode::Shared`,
  all players draw from one pool and one RNG seeded by `MatchSetup::shared_seed` (`sim_match`).
- **Combat seeds.** `combat(a, b)` draws the battle seed from the RNG player `a` draws from, which
  is what `TavernGame.resolve_combat_with` does. `combat_with_seed` takes the seed from the caller
  and records it.
- **The same game.** A `Match` makes exactly the engine calls you would make by hand with the
  same seeds (`tests/replays.rs`, `match_plays_like_the_bare_engine`). Moving a game loop onto it
  does not change its games.
- **Bots need their own RNG.** Anything that is not an engine call (a bot's choices, a policy's
  sampling) must not draw from the engine's RNGs. Otherwise re-simulation, which only repeats
  engine calls, falls out of step.
- **Illegal actions** return `Err` and are not recorded.
- **Crashes.** Each call is recorded after its legality check and *before* it runs. If the engine
  panics, save `game.replay()` anyway: the replay then ends with the call that panicked, and
  re-simulating it reproduces the panic (`sim_match` does this).
- **Notes.** `step_with_note` attaches a `Note` to an action: what the actor knew or thought
  (value estimate, log-probability, candidate actions with probabilities, whether the move was
  forced on it, free-form numbers). Notes are shown by viewers and never affect re-simulation.
- **Labels.** `MatchSetup::meta` (or `Match::meta_mut`) stores free-form string labels such as
  the source, run or training iteration.

---

## 2. The file format

A replay file is one protobuf message, `seaglass.replay.Replay` (`proto/replay.proto`), with no
framing. By convention the file name ends in `.replay.pb`. `src/replay/mod.rs` provides
`save`/`load` and `encode`/`decode`. `save` writes to a temporary file and renames it, so an
interrupted save never leaves a truncated replay.

```text
Replay
├── header: Header
│   ├── format            replay::FORMAT_VERSION (2)
│   ├── engine            the recording build: crate version + git commit
│   ├── catalog           cards::catalog_for name ("full")
│   ├── pool              PER_PLAYER | SHARED
│   ├── shared_seed
│   ├── players[]         name, seed, deity, armor, shop_spells, hero, trinkets, dark_gifts
│   └── meta              string labels
└── records[]: one per engine call, in order
    ├── SetPairs          pairs[] (a, b)
    ├── StartTurn         player, turn (the player's turn after the call)
    ├── DeliverGifts
    ├── Act               player, action, card_id, note
    ├── Combat            a, b, seed, external_seed, outcome, hero_damage, check_a, check_b
    ├── RecordDeaths      alive[]
    └── End               winner (unset for a draw), turn, health[]
```

Actions use the same indices as `TavernAction` (shop, hand, board, Discover, hero and trinket
option indices), every `TavernAction` included: Hero Powers, Passes, spells cast on the shop,
hero and trinket choices and Dark Discovery. Format 1 replays (before heroes and the lobby) are
not read.

Some fields are **redundant on purpose**. Re-simulation recomputes them and compares, which
catches a replay re-simulated by the wrong engine at the first call that goes wrong:

| Field                          | Recorded value                                               |
| ------------------------------ | ------------------------------------------------------------ |
| `StartTurn.turn`               | the player's turn number after the call                      |
| `Act.card_id`                  | the card the action buys, plays, casts, sells, repositions, activates, passes or chooses (`replay::action_card`; none for heroes, trinkets and Hero Powers) |
| `Combat.seed`                  | the battle seed (drawn or given)                             |
| `Combat.outcome`, `hero_damage`| the battle result (`outcome` is unset if the battle never finished) |
| `Combat.check_a`, `check_b`    | each player afterwards: health, armor, tier and an FNV-1a fingerprint of board and hand (card, Attack, Health, keywords, Golden; `replay::checkpoint`) |
| `RecordDeaths.alive`           | who is still in the game after the combats                   |
| `End`                          | winner, final turn and every player's health                 |

To look inside a replay without seaglass, run
`protoc --decode seaglass.replay.Replay engine/proto/replay.proto < game.replay.pb`.

---

## 3. Re-simulating: `Timeline`

`seaglass::Timeline` (`src/replay/timeline.rs`) re-simulates a replay. It starts a fresh `Match`
from the header and repeats every recorded call:

```rust
let timeline = seaglass::Timeline::load("replays/game.replay.pb")?;
for decision in timeline.decisions() {
    // decision.before[decision.player] -> decision.action -> decision.after[...]
}
```

- `frames[0]` is the match before any call, and `frames[i]` is the match after record `i - 1`.
  A frame holds every player's `TavernState`, every RNG's state (`Rng::state`) and every pool's
  remaining copies (in `Timeline::card_ids` order). Its `kind` says which call produced it, and
  for a combat it includes the full `BattleResult` with its event log.
- `turn_start(t)` finds the first frame of turn `t`. `decisions()` yields every action with the
  states before and after it, which is the shape offline training data needs.
- `divergences` lists every call whose recomputed fields (§2) differ from the recorded ones.
  `error` is set if a call could not be re-simulated: an action that is now illegal, a missing
  player, or an engine panic. The timeline then stops before that call. Its frames up to that
  point are still valid.
- `is_faithful()` is true when there are no divergences and no error. `engine_mismatch()` returns
  the recording build if it is not the current one.
- Only the header can make loading fail outright: an unknown format version, catalog, pool mode
  or Deity.

---

## 4. Tools

`sim_match` records its games:

```sh
cargo run --release --bin sim_match -- --seed 7 --record replays/s7.replay.pb
cargo run --release --bin sim_match -- --seed 7 --quiet --record replays/s7.replay.pb  # result line only
```

`sim_match`'s bots draw from their own RNG (seeded from the match seed), not the engine's. This
is what lets replays re-simulate.

`match_replay` reads replays:

```sh
match_replay summary game.replay.pb                 # setup, every combat, the result
match_replay log game.replay.pb                     # the whole game, call by call
match_replay show game.replay.pb --turn 5           # every player's full state at a frame
match_replay step game.replay.pb                    # step forwards and backwards interactively
match_replay tui game.replay.pb                     # play it back in a terminal UI (§4.1)
match_replay cast game.replay.pb                    # record that play-back for asciinema (game.cast)
match_replay verify replays/*.replay.pb             # exit 1 unless every replay re-simulates faithfully
match_replay html game.replay.pb                    # write game.html, a viewer page (-o picks the file)
match_replay scenario game.replay.pb --frame 54     # a scenario that rebuilds frame 54, replays the next action
```

`step` reads one command per line:

| Command   | Effect                                                |
| --------- | ----------------------------------------------------- |
| enter, `n`, `p` | next / previous frame                           |
| `N`, `P`  | next turn / this turn's start (then the previous turn) |
| `g I`, `t T` | go to frame `I` / turn `T`                         |
| `s`       | every player's full state                             |
| `c`       | the current combat's event log                        |
| `d`, `q`  | show the frame again / quit                           |

`--frame N` or `--turn T` picks the starting frame for `show`, `step`, `tui` and `cast`.

`html` writes one self-contained page (nothing else to load, works offline; about 130 KB for a
110-frame game). It shows every player's board, hand, shop and discover with the frame's changes
highlighted, the frame's note (value, probability, candidates) and its combat (boards, outcome and
the full event log). ←/→ step a frame, ↑/↓ (or PgUp/PgDn) a turn, Home/End go to either end; the
slider, the turn and frame lists and the health chart jump anywhere, and `game.html#frame=N` links
to a frame. A banner warns if the replay was recorded by another build or does not re-simulate
faithfully.

`scenario` writes a scenario (`scenarios.md`) that reproduces a frame. `tavern:` patches
`TavernState::new()` into player P's exact Tavern at frame N (units as unit specs, plus whatever
fields no spec gets right), `seed:` resumes the RNG P draws from and a `take_from_pool` step takes
the copies missing from P's pool; P's next K recorded actions (`--steps K`, default 1) follow as
steps, and `expect:` checks Gold, Health, Armor, tier, the zones and whatever else they changed.
P (`--player`, a name or a 0-based index) defaults to whoever acts at frame N+1; `--turn T` picks
the frame before the first action of turn T; `--steps 0` only rebuilds the state (any player, any
frame). P's actions need not be consecutive frames: with a pool and RNG per player, the other
players' calls (their actions and `start_turn`s, combats without P) cannot touch P's Tavern, pool
or RNG, so the export passes over them (the header lists them) and stops early, with a note, only
at P's own `start_turn`, a combat involving P or the end of the match. With a shared pool and RNG,
it stops early at any frame that is not an action by P. Before writing anything (to stdout, or
`-o out.yaml`) it rebuilds the setup in-process, compares P's complete state, pool and RNG with
the replay's at frame N and after each of P's actions, and runs the scenario through the scenario
runner: an export that fails either is an error.

### 4.1 The terminal player: `tui` and `cast`

`tui` plays a replay back full screen, with every card drawn as character art
(`tools/src/render/`): the frame is a heavy line for Taunt and a dashed one for Stealth, a bubble
of tall brackets around the card is Divine Shield (its columns are always reserved, so a board
never shifts when it pops), a badge row shows Windfury `≋`, Venomous `☣`, Reborn `↺`, Magnetic `∪`,
Deathrattle `☠` and triggered effects `ϟ` (Rally, Start of Combat), a Golden card has a `★` and a
yellow frame, and stats are green when buffed and red when damaged. `--ascii` draws with plain
ASCII instead, for fonts that draw some of those symbols two columns wide.

- **Tavern frames** show one player's Tavern: their hero bar (Health, Tier, Gold, hero and Hero
  Power, trinkets), the shop (or the Discover options), the board and the hand, with the cards
  the action brought in highlighted, plus trinket and hero offers and the frames around this one.
  On a wide terminal (160+ columns) a side panel shows the big card of what the action brought in:
  its rules text (from the card sources' doc comments), keywords and base stats. Seven cards
  need 118 columns; narrower or shorter terminals get compact cards.
- **Combat frames** play strike by strike. The combat is re-run from the players' states before
  it with the recorded seed inside `combat::with_combat_trace`, which snapshots both boards before
  and after Start of Combat and after every strike (a thread-local, off unless a trace runs, that
  never changes the combat). Each strike is two steps: the strike (the attacker in reverse video,
  an arrow to its target, the damage on each card hit, popped shields flashing and the minions
  about to die faded to `░`) and the boards after it. The caption lists the strike's events. If
  the re-run's result differs from the replay's, the screen says so.
- **Keys.** ←/→ step (a strike, in a combat), space plays and pauses, +/− change the speed, n/p go
  a whole frame, ↑/↓ a turn, c/C to the next/previous combat, Home/End to either end, Tab shows the
  next player's Tavern, f follows whoever acts (the default) or stays on one player, i inspects
  cards (←/→ pick one, its big card shows beside it), ? lists the keys, q quits.
- `--dump [--step S] [--size WxH] [--plain]` prints one screen (combat step S) instead of starting
  the player, for checking a screen or pasting one somewhere.

**Games from a Hearthstone `Power.log`.** `tui` (and `cast`) also open a `Power.log`
(`match_replay tui ~/Hearthstone/Logs/Power.log --game 2`; the default is the log's last game).
They can't be re-simulated (Blizzard's random draws aren't seaglass's), so they play back what
the log shows, from the `seaglass_replay` parser (`replay/`):

- **Your Tavern, action by action**: the shop, board and hand after every buy, play, sell,
  refresh, Hero Power and so on (`TavernPhaseReplay::snapshots`).
- **Your teammate** (Duos): the log shows nothing of their Tavern phase, only their warband at
  each combat, so the Tavern view adds their board as it entered the last combat.
- **Combats, strike by strike**: both boards at every warband entry and before every attack,
  including the winning hero's (`CombatSetup::frames`), with tag-ins as their own step. The
  damage on a card is its Health drop between snapshots.
- **The engine's version of every combat** (`e` switches): the engine re-runs the combat from
  the warbands that entered it, attacking the real targets while its attacks match the real ones
  (`validate::engine_trace`; of 64 seeds, the run that follows them the longest). Both versions
  name the first strike after which the boards (names and stats) differ, or a different result,
  and the engine's says where it stopped following the real attacks and which minions it doesn't
  implement. A log from an older patch differs wherever the cards changed since.

`compare` checks a `Power.log` against the engine without the UI: for every combat of every game
(`--game G`, `--turn T`), the real and engine results and strike counts, the first strike after
which the boards differ (comparing the boards as they stand before the next strike; names without
the engine's `Golden` prefix; not after the last strike, which the result covers), the minions
that differ on each side, and both versions of that strike. `--full` adds both versions step by
step. `replay_cli validate` complements it with outcome odds over thousands of engine runs.

`cast` records the player's auto-play as an asciinema recording (asciicast v2): `-o` picks the
file (default `game.cast`), `--frames K` stops after K frames, `--speed X` scales the pace and
`--size WxH` sets the screen (default 160x48). Play it with `asciinema play game.cast` or embed it
with asciinema-player. A 10-turn game is about 4 minutes and 2–3 MB.

---

## 5. Python

The Python module (`seaglass`, built with `maturin`, see `README.md`) has both halves:

```python
import seaglass

match = seaglass.Match(
    players=[
        {"name": "agent", "seed": 42, "deity": "cthun"},
        {"name": "bot", "seed": 43, "deity": "yshaarj", "hero": "Ragnaros the Firelord"},
    ],
    catalog="full",
    pool="per_player",            # or "shared" (with shared_seed=...)
    meta={"run": "ppo-7"},
)
agent, bot = match.player(0), match.player(1)
match.set_pairs([(0, 1)])
agent.start_turn()
bot.start_turn()
match.deliver_gifts()
agent.step(action, note={"value": 0.3, "logp": -1.2})
agent.end_turn(note={"forced": True})
match.deliver_gifts()
battle = agent.resolve_combat_with(bot)     # or match.combat(0, 1)
match.record_deaths()
match.finish()
match.save("replays/game.replay.pb")

timeline = seaglass.Timeline.load("replays/game.replay.pb")
frame = timeline[timeline.turn_start(3)]   # kind, turn, player, action, note, players, battle, ...
for index, player, action, note in timeline.decisions():
    before = timeline.player_at(index - 1, player)   # observe(), legal_actions(), is_legal()
```

- **`MatchPlayer`.** `match.player(i)` has the `TavernGame` methods (`observe`, `legal_actions`,
  `is_legal`, `step`, `start_turn`, `end_turn`, `resolve_combat_with`), so bots and
  `seaglass_rl.action_mask` work on it unchanged. With `pool="per_player"`, a player plays exactly
  like `TavernGame(seed, catalog, deity, ...)`. `step` and `end_turn` take an optional note. A
  player's dict takes `TavernGame`'s options: `name`, `seed`, `deity`, `armor`, `shop_spells`,
  `hero` (a name or id), `trinkets` and `dark_gifts`.
- **The lobby.** `match.set_pairs`, `match.deliver_gifts` and `match.record_deaths` are the
  `Lobby` calls of §1 (a `Match` has its own lobby: no `seaglass.Lobby` needed).
- **Notes** are dicts with `value`, `logp`, `forced`, `candidates` (a list of
  `(TavernAction, probability)` pairs) and any other numeric entries (`Note.extra`). A badly typed
  note raises `TypeError`, and nothing is recorded. Values and probabilities are stored as `float32`.
- **`Timeline`** re-simulates on load (`Timeline.load(path)`, `Timeline.from_bytes(data)` or
  `match.timeline()`). `is_faithful`, `divergences`, `error` and `engine_mismatch` work as in §3,
  and `header` is the setup as a dict. `timeline[i]` is a `Frame` (negative indices count from the
  end).
- **`FramePlayer`.** `timeline.player_at(frame, player)` is one player at one frame, read-only, so
  `action_mask` and `encode_observation` work on recorded games.

`rl/` records the games it plays. Every `BattlegroundsEnv` episode is a `Match` (`env.match`:
player 0 is the agent), and so is every self-play game. Each policy action is noted with its
`value` and `logp`, and moves the environment forces are marked `forced`.
`python -m seaglass_rl.train_ppo --replays DIR` saves the evaluation games and the last game of
every logged iteration. `seaglass_rl.replays` turns replays back into training data:

```python
from pathlib import Path
from seaglass_rl import replays

data = replays.to_dataset(sorted(Path("runs").glob("*.replay.pb")), player="agent")
# NumPy arrays, one row per action: player_vec, slot_card_ids, slot_num_feats, action_mask,
# actions (as the trainer's batches), plus value, logp, forced, outcome, player, turn, source, frame.
```

`replays.load` raises `ValueError` unless the replay re-simulates faithfully. `to_dataset`
rebuilds each row from the state before the action, with the environment's mask settings, so
for an environment game it reproduces exactly what the policy saw (`rl/tests/test_replays.py`).
