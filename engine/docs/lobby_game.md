# Full Lobbies: `LobbyGame`

A `LobbyGame` (`src/lobby_game.rs`) plays a whole Battlegrounds game: 8 Solo players, or Duos (4
teams of 2), from the hero choice to the last unit standing. It covers what a single Tavern
(`tavern.md`) or a single Duos team (`duos_tavern.md`) leaves to its caller:
- hero offers;
- one card pool for the lobby;
- pairings;
- ghosts;
- knockouts;
- placements.

It runs on a recorded `Match` (`replays.md`), so every game it plays is a replay that
`match_replay` can show.

The players' decisions come from outside: a bot, a policy, a person. `LobbyGame` only takes one
action at a time and enforces the rules.

**Units** are what fights and gets placed: players in Solo, teams in Duos. Players `2t` and
`2t + 1` are team `t`.

---

## 1. Setup (`LobbyConfig`)

| Field | Default | Meaning |
| :--- | :--- | :--- |
| `duos` | `false` | Duos: 4, 6 or 8 players. Solo: 2..=8. |
| `players` | 8 | |
| `max_turns` | 30 | After this turn's combats, the units still in are placed by Health plus Armor. |
| `heroes`, `hero_offers` | on, 2 | Each player is offered `hero_offers` heroes (§2). Without heroes: 5 Armor per player (a Duos team 10), no Hero Power. |
| `trinkets`, `dark_gifts` | on, on | As for every Tavern (`trinkets.md`, `dark_gifts.md`). |
| `catalog` | `"full"` / `"duos"` | |
| `deity` | none | Every player's Deity. |
| `first_fighter_switch_chance` | 0.75 | Duos (`duos_tavern.md` §4.3). |
| `avoid_recent` | 3 | Pairings avoid a unit's last 3 opponents while they can (§3). |
| `damage_caps` | on | Cap combat damage as the game does (§5). |
| `tribes` | 5 | The minion types in the lobby, drawn at random from the 10 (`LOBBY_TRIBES`); 0: every type. |
| `seed` | 0 | Seeds the shared pool and RNG, the hero offers and the pairings. |
| `names`, `meta` | `p0`, `p1`, ... | The replay's player names and labels. |

**One pool.** All players draw from one shared card pool and RNG (`PoolMode::Shared`), so players
of the same tribe compete for its copies. In Duos the four teams share it too.

**Five minion types.** As in the game, each lobby has 5 of the 10 minion types, drawn uniformly
from the lobby's own RNG and recorded in the replay header (`MatchSetup::lobby_tribes`). The pool
keeps a minion if it has one of the lobby's types (a two-type minion through either of them: a
Dragon/Naga is in a lobby with Dragons), no type, or every type (`game::in_lobby`). Every effect
that asks whether the lobby has a type reads the pool (`CardPool::lobby_has`: the Dark Gift
filters, "a random lobby type"). Every player sees the types from the start (`context(p).tribes`).
Only minions are filtered: Tavern spells, heroes and trinkets are offered whatever the types.

---

## 2. Phases

```text
LobbyGame::new ──► [hero phase] ──► turn 1 ──► turn 2 ──► ... ──► over
                     begin_phase / players act / end_phase, for each phase
```

Every phase is `begin_phase()`, then the players act, then `end_phase()`.

- **Players act.** `active_players()` lists who is still to act. `step(p, action, note)` returns
  `true` once player `p` is done with the phase. `force_end(p)` closes a player's phase for them.
  The engine checks legality as usual: `valid_actions(p)`, `is_legal(p, action)`. Before
  `begin_phase()` and after a player is done, they have no legal actions.
- **Hero phase** (with heroes). Every player is offered `hero_offers` distinct heroes, drawn by
  `heroes::draw_offers` (`heroes.md` §6) and recorded in the replay header
  (`PlayerSetup::hero_options`). Players start with no Armor; the chosen hero gives it, and in
  Duos the team's pool takes its Duos Armor. A player is done once they pick (`ChooseHero`, the
  only legal action until then).
- **A turn.** `begin_phase()` does four things:
  1. Draws the pairings (§3) and records them (`Match::set_pairs`, so every player's lobby view
     knows their next opponent).
  2. Starts every living unit's turn.
  3. Delivers the lobby gifts.
  4. Refreshes the leaderboard (§4).

  Then every living player acts until they end their turn. A Duos player who has ended waits for
  their teammate.
- **`end_phase()` after a turn:**
  1. Forces any player still acting to end their turn.
  2. Delivers the lobby gifts.
  3. Fights every pair, then the ghost (§3).
  4. Records the knockouts (§5).

  It returns the units placed.

`force_end(p)` resolves whatever keeps the phase open, taking the first option each time:
- the first offered hero, in the hero phase;
- the first option of a pending Discover, trinket offer or other prompt;
- then End Turn.

Every action it takes is recorded as forced (`Note::forced`).

---

## 3. Pairings and ghosts

Each turn the living units are paired at random (engine decision):
- `LobbyGame` tries 16 random matchings from its own RNG and keeps the one with the fewest
  repeats.
- Pairing a unit with an opponent it fought `i` combats ago costs `avoid_recent - i`.
- Its RNG is never an engine RNG. The pairings are recorded, so a replay re-simulates without it.

With an odd number of living units, one unit fights the **ghost** of the last unit knocked out
(`Match::ghost_combat`, recorded as `GhostCombat`):
- **The ghost's boards:** those its players entered their last combat with
  (`Lobby::last_warbands`), at their Tavern Tiers, with no hand or auras.
- **The result:** only the fighter takes it and the damage.
- **Who fights it:** preferably not the unit that fought it last turn.
- **No ghost yet** (an odd lobby before any knockout): the odd unit has a bye.

In Duos, each player is paired in the lobby with the opposing team's player in their seat. That
is what their lobby view and the lobby Hero Powers see (`tavern.md` §3.1). The ghost's players
stand in as the next opponents.

---

## 4. What a player knows (`context`)

`context(p)` is player `p`'s view of the lobby as of the start of the turn: what the game shows a
person, and no more. The leaderboard shows each opponent's Health, Armor, Tavern Tier and win
streak, but not their board; a player only knows the boards they fought. RL observations encode it
(`rl/seaglass_rl/features.py`).

| Field | Meaning |
| :--- | :--- |
| `units`, `alive_units` | |
| `health_rank` | 1 + the living units with more Health plus Armor. |
| `next_is_ghost` | This unit fights a ghost this turn. |
| `damage_cap` | This turn's damage cap, if any (§5). |
| `tribes` | The lobby's minion types (§1). |
| `opponents[]` | `player`, `alive`, `health`, `armor`, `tier`, `hero_id`, `win_streak` (the leaderboard), `is_next`, `is_last`, and `seen_board` / `seen_turn`: the board they entered combat with the last time *this player* fought them (Duos: if their warband stepped in), and when. A player never sees an opponent's board otherwise, so it is empty for an opponent never fought. The next opponent comes first (in Duos, the opposing player in this seat, then their teammate), then the living ones by Health plus Armor, then those who are out. |

Health, Armor and Tier are the leaderboard's. They are refreshed at the start of each phase and
after the combats, and a player who is out keeps their last values.

---

## 5. Knockouts and placements

After a turn's combats, every living unit at 0 Health or less is out:
- **Places.** The units out this turn take the worst places left, best first: more Health plus
  Armor first (a knockout's Health is 0 or less), then the lower unit (engine decision).
- **End of game.** If at most one unit is left, or the turn was `max_turns`, every unit still in is
  placed the same way and the game is over.
- **Recording.** `Match::record_deaths` records who is out. The end is recorded with every unit's
  place (`Match::finish_with_placements`, `End.placements`). In Duos the winner is a team.

**Damage caps** (`damage_caps`, on by default), as the game applies them (patch 31.2):

| Mode | Cap on the combat damage a unit takes | Until |
| :--- | :--- | :--- |
| Solo | 5 on turns 1–3, 10 on turns 4–7, 15 from turn 8 | only 4 players are left (then none) |
| Duos | 15 from the first turn | only 2 teams are left (then none) |

- **When the cap is set.** `begin_phase` sets the turn's cap from the turn number and the units left
  at the start of the turn (`LobbyGame::damage_cap`). It records the cap with the pairings
  (`Match::set_pairs_with_damage_cap`, `SetPairs.damage_cap`), so replays re-simulate it.
- **Who sees it.** Every player's lobby view carries it (`LobbyView::damage_cap`).
- **Where it applies.** A losing Solo player's Tavern, or Duos team's pool, takes at most the cap,
  pair combats and ghost combats alike.
- **What gets recorded.** The combat's recorded `hero_damage` is the damage taken (capped).

---

## 6. Python

`seaglass.LobbyGame(players=8, duos=False, seed=0, max_turns=30, heroes=True, hero_offers=2,
trinkets=True, dark_gifts=True, catalog=None, deity=None, first_fighter_switch_chance=0.75,
avoid_recent=3, damage_caps=True, tribes=5, names=None, meta=None)`:

- **Properties:** `phase` (`"hero"`, `"turn"`, `"over"`), `turn`, `done`, `placements` (per unit,
  `None` while in), `alive` (per unit), `pairs` (the pairs, the ghost's fighter), `damage_cap`
  (this turn's, or `None`), `tribes` (the lobby's minion types), `num_players`, `num_units`,
  `duos`.
- **Units:** `unit_of(p)`, `unit_players(u)`, `unit_health(u)`.
- **Phases:** `begin_phase()`, `end_phase()` (returns the units placed), `active_players()`,
  `is_done(p)`.
- **Acting:** `observe(p)` (a `TavernObservation`, or a `DuosObservation` in Duos),
  `legal_actions(p)`, `is_legal(p, action)`, `step(p, action, note=None)` (note: a dict, see
  `Match`), `force_end(p)`.
- **The lobby:** `context(p)` returns a `LobbyContext` with `OpponentInfo`s.
- **Replays:** `save(path)`, `replay_bytes()`, `timeline()`, `meta`, `set_meta(k, v)`.
- **Lookahead:** `copy()` returns an independent copy.

```python
import seaglass

game = seaglass.LobbyGame(duos=True, seed=7)
while not game.done:
    game.begin_phase()
    for p in game.active_players():
        game.force_end(p)        # a policy would call game.step(p, action) instead
    placed = game.end_phase()
game.save("replays/duos.replay.pb")  # match_replay tui replays/duos.replay.pb
```
