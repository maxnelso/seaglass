# Seaglass Engine (`engine/`)

`seaglass` is a **deterministic, headless Hearthstone Battlegrounds game engine** written in Rust. It models both phases of a Battlegrounds match:

1. **Tavern (Recruit) Phase** — Gold economy, Tavern Tiers 1–6, a finite shared card pool with copy depletion, Bob's shop, buying/playing/selling/repositioning minions, triples and Discover rewards, Spellcraft spells, and persistent game-long player auras.
2. **Combat Phase** — Exact board-vs-board combat simulation with a structured event log for single battles and fast Monte Carlo rollouts (`simulate_batch`) for win/tie/damage distributions.

The engine contains **only game rules and simulation**. All reinforcement learning policy choices—observation encoding, action-space flattening, reward shaping, and opponent curricula—live in [`../rl/`](../rl/README.md).

---

## Design Principles

1. **Exactness ("No Guessing")**
   Every mechanic, trigger order, and edge case is specified in `docs/` before it is coded. Any unsupported state or illegal action returns an explicit error—never a silent fallback or no-op.
2. **Strict Determinism**
   Every battle and tavern sequence is a pure function of its inputs and a 64-bit seed. All randomness flows through a single version-stable `SplitMix64` PRNG in a pinned draw order—never through system clocks, global RNGs, or hash-map iteration order.
3. **Speed**
   Combat runs in a tight loop over small boards ($\le 7$ units per side). A single battle takes microseconds, enabling thousands of Monte Carlo rollouts per second for RL equity evaluation.
4. **Declarative Testing**
   Mechanics and card interactions are verified through human-readable YAML scenarios under `tests/scenarios/`: one file per card (`minions/`, `spells/`) plus combat and Tavern topics (`docs/scenarios.md`). Adding a test means adding a scenario.

---

## What the Engine Models

### Tavern Phase (`docs/tavern.md`)
- **Economy & Tiers**: Turn progression, gold refill (`3` to `10` gold), Tavern Tiers `1..=6`, and start-of-turn upgrade cost decay.
- **Shared Finite Card Pool (`CardPool`)**: Tier-accurate copy counts (`15 / 15 / 13 / 11 / 9 / 7` copies for Tiers `1..=6`), uniform sampling without replacement across remaining copies, and pool return on shop refresh, minion sale (`1` copy, or `3` if Golden), and unchosen Discover options.
- **Core Actions (`TavernAction`)**:
  - `Buy { shop_index }`
  - `Play { hand_index, board_pos }` (places a minion at `board_pos` or casts a targeted/untargeted spell)
  - `PlayOnShop { hand_index, shop_index }` (casts a spell that says "a minion" on a minion in the shop)
  - `Sell { board_pos }`
  - `Reposition { from_pos, to_pos }`
  - `Refresh`
  - `UpgradeTavern`
  - `ToggleFreeze`
  - `ChooseDiscover { option_index }`
  - `EndTurn`
  - `Pass { hand_index }` (Duos only: passes a hand card to the teammate)
  - `HeroPower { target }` (uses the hero's Hero Power, `docs/heroes.md`)
  - `ChooseHero { option_index }` (chooses an offered hero)
  - `DarkDiscovery` (the Dark Discovery button, `docs/dark_gifts.md`)
- **Triples & Discover Sub-State**: Automatic combination of 3 non-golden copies across `board` and `hand` into a Golden minion with summed buffs, plus a `discover_pending` sub-state when playing a Golden minion.
- **Spellcraft & Persistent Auras**: Turn-temporary and permanent (`Lava Lurker`) Spellcraft buffs, full-hand pending queue (`pending_spellcrafts`), and game-long scaling counters (`PlayerAuras`).
- **Duos Taverns (`docs/duos_tavern.md`)**: A team of two players (`DuosTavern`), each with their own Tavern, sharing one Health pool and the card pool, acting in the same turn: `Pass` moves a hand card to the teammate for 1 Gold (free with `Wanderer Cho`), into an unbounded queue if their hand is full, and completes their triples; the team's combat brings each player's result back to their own Tavern.

### Heroes (`docs/heroes.md`)
- **The hero pool**: all 123 heroes of the pool (`heroes::table()`, generated from `CardDefs.xml` by `tools/gen_heroes.py`) with their Health, Solo and Duos Armor and Hero Power; a player's hero lives in their `PlayerAuras::hero` (none by default: a player without a hero plays exactly as before).
- **Hero Powers** are cards with hooks: active ones are the `HeroPower` action (cost, uses per turn, targets), passive ones react to events, and Start of Combat ones resolve before every other Start of Combat effect. Each implemented hero has its own file, `src/heroes/<hero>.rs`, and scenario file, `tests/scenarios/heroes/<hero>.yaml`.
- **Hero selection**: offers (2 per player, distinct across the lobby) and `ChooseHero`; in Duos the team's pool gets both heroes' Duos Armor.

### Dark Gifts (`docs/dark_gifts.md`)
- **The gifts**: the 47 Dark Gift cards (`dark_gifts::table()`, generated from `CardDefs.xml` by `tools/gen_dark_gifts.py`) with their turn windows and weights; a minion's gifts are `Unit::dark_gifts` (permanent enchantments that travel with it and combine on a Triple).
- **The Dark Discovery button** (`TavernAction::DarkDiscovery`, behind `TavernState::dark_gifts_enabled`, off by default): 3 Gold, from turn 3, once per turn, 3 uses per game, a Discover of 3 minions each with a different gift, drawn by the offer rules (`dark_gifts::draw_offer`). Each implemented gift has its own file, `src/dark_gifts/<gift>.rs`, and scenario file, `tests/scenarios/dark_gifts/<gift>.yaml`.

### Combat Phase (`docs/combat.md`)
- **Core Loop**: First-attacker determination (minion count after Start of Combat, then coin flip), alternating turns with identity-tracked attack pointers, Taunt-aware target selection, simultaneous damage, and hero damage (`hero_tier + sum(survivor_tiers)`).
- **Keywords & Triggers**:
  - Keywords: `Taunt`, `Divine Shield` (inherent vs. granted), `Windfury`, `Reborn`, `Magnetic`.
  - Triggers: `Start of Combat`, `Rally` (on-attack), `Deathrattle`, and card-specific latches (e.g., `Scarlet Survivor`).
- **Duos (`docs/duos.md`)**: Team-against-team combats fought as a relay of the players' warbands (`simulate_duos`, `simulate_duos_batch`): a warband steps in, with its Start of Combat, when its teammate's has no minions left, and each player keeps their own auras, hand, Deity and Tavern Tier.

---

## Public API

### Rust API

```rust
use seaglass::{simulate, simulate_batch, GameState, Unit};

// Single deterministic battle with full event log
let result = simulate(&board_a, &board_b, &GameState::default(), 42);

// Batch Monte Carlo rollout (win/tie rates and hero damage distribution)
let dist = simulate_batch(&board_a, &board_b, &GameState::default(), 42, 1000);
```

### Python Bindings (`seaglass`, behind `--features python`)

The PyO3 bindings provide a **marshalling-only** layer exposing the engine to Python:

- `seaglass.simulate(board_a, board_b, seed, hero_tier_a=1, hero_tier_b=1) -> PyBattleResult`
- `seaglass.simulate_batch(board_a, board_b, base_seed, n, hero_tier_a=1, hero_tier_b=1) -> PyDistribution`
- `seaglass.TavernGame(seed=0, catalog="full")` — Stateful handle wrapping `TavernState`, `CardPool`, and `Rng`:
  - `.state()` / `.observe()` — Current turn, health, gold, tier, upgrade cost, freeze flag, board, hand, shop, and discover options.
  - `.is_legal(action)` / `.legal_actions()` — Engine-authoritative legality checks.
  - `.step(action)` — Executes a `TavernAction` (or raises `ValueError` if illegal).
  - `.start_turn()` / `.end_turn()` — Advances turn lifecycle.
- `seaglass.TavernGame(..., hero=None)` / `seaglass.DuosTavernGame(..., heroes=None)` give the players heroes (`docs/heroes.md`), and `seaglass.heroes_table()` lists the pool.
- `seaglass.TavernGame(..., dark_gifts=True)` / `seaglass.DuosTavernGame(..., dark_gifts=True)` (the Python default; `False` turns it off) give the players the Dark Discovery button (`docs/dark_gifts.md`), and `seaglass.dark_gifts_table()` lists the gifts.
- `seaglass.catalog(name="full")` — Returns the engine's `CardTemplate` list so Python never hardcodes card IDs or stats.
- `seaglass.parse_unit(spec)` — Parses compact scenario strings such as `"3/2 divine_shield taunt"`.

---

## Building & Testing

```bash
# Run all tests, including the YAML scenario suite (no Python dependency required)
cargo test

# One card's scenarios (each scenario is its own test)
cargo test -p seaglass --test scenarios -- minions::joyous::

# Build release library + Python extension
cargo build --release --features python
```
