# Scenario & Matchup File Specification

Seaglass's tests are declarative YAML **scenarios** (`tests/scenarios/`, run by
`tests/scenarios.rs`). Board-vs-board **matchup** files (`examples/matchups/*.yaml`, run by
`combat_cli`) are a separate, simpler format (§9).

Every key is checked: an unknown or misspelled key, step, field, card name or matcher is an
error (usually with a "did you mean" hint), never silently ignored.

---

## 1. The suite

```text
tests/scenarios/
├── catalog.yaml          # the expected card lists (§7)
├── combat/<card|topic>.yaml   # combat-phase minions and combat mechanics
└── tavern/<card|topic>.yaml   # tavern-phase minions, spells, and Tavern mechanics
```

Every scenario is its own test, named `<dir>::<file>::<scenario>`:

```sh
cargo test -p seaglass --test scenarios                       # everything
cargo test -p seaglass --test scenarios -- tavern::joyous::   # one card
cargo test -p seaglass --test scenarios -- --list             # list the tests
SCENARIO_TRACE=1 cargo test -p seaglass --test scenarios -- tavern::turn_flow:: --nocapture
```

`SCENARIO_TRACE=1` prints every step and a summary of the state after it. A file that does
not load is reported as the test `<dir>::<file>::<file>`.

---

## 2. Scenario files

```yaml
card: Joyous                   # what the file is about: `card: <name>` or `topic: <text>`
defaults:                      # optional: merged into every scenario
  seed: 101
scenarios:
  - name: battlecry_buffs_the_deity          # snake_case, unique in the file
    description: Joyous gives the deity +2/+1. # optional
    tavern: {gold: 10}                       # the setup (§2.1)
    steps:                                   # optional (§2.2)
      - add_to_hand: Joyous
      - play: 0
    expect:                                  # optional: checked after the steps (§5)
      auras: {deity: {attack: 3, health: 2}}
```

| Key | Meaning |
| :--- | :--- |
| `name` | Required. `[a-z0-9_]+`, unique within the file. |
| `description` | Free text. |
| `known_bug` | Marks a scenario that reproduces a known engine bug (§6.2). |
| `seed` / `seeds` | The seed (default `0`), or several: a list, `"1..50"` or `"1..=50"` (at most 10000). The scenario runs once per seed. |
| `tavern` / `combat` / `batch` | The setup (§2.1). |
| `steps` | Steps, run in order (§2.2). |
| `expect` | A final expectation, the same as a last `expect:` step. |

`defaults` is deep-merged into each scenario: mappings merge key by key, other values
replace. A scenario's own `combat:` drops a default `tavern:` (and its own `tavern:` drops a
default `combat:` and `batch:`), so a file can default to a Tavern and still hold combats.

### 2.1 Modes

**Tavern** (the default; `tavern:` is optional): a fresh `TavernState::new()`, patched by the
`tavern:` mapping (§4), with an RNG seeded by the scenario seed and a card pool. `catalog:`
picks the pool (`full` by default, or `tier1`..`tier7`, `solo_tier_1`, ...: see
`cards::catalog_for`).

```yaml
tavern: {catalog: tier1, gold: 10, tavern_tier: 3, board: [Joyous, "golden Zoatroid"]}
```

Nothing happens until a step runs: use `start_turn` to begin a turn (gold, shop roll,
start-of-turn triggers).

**Combat**: a standalone battle (`sim::simulate`), run before the steps. Steps may only be
`simulate`, `expect`, `if` and `repeat`.

```yaml
combat:
  seed: 7                        # optional: defaults to the scenario seed
  board_a: ["3/2 card:zoatroid", Joyous]
  board_b: ["4/22 taunt"]
  hand_a: []                     # optional
  hand_b: []
  hero_tier_a: 1                 # optional (default 1)
  hero_tier_b: 1
  auras_a: {deity: {kind: cthun, attack: 6, health: 6}}   # a PlayerAuras patch (§4)
  auras_b: {}
expect: {outcome: a_win}
```

**Batch**: `combat:` plus `batch: {base_seed, n}` runs `sim::simulate_batch` (`n` battles).
A batch scenario has no steps, only `expect` (against the distribution, §5.1).

### 2.2 Steps

A step is a name (`refresh`) or a single-key mapping (`buy: 0`).

| Step | Effect |
| :--- | :--- |
| `start_turn` | `TavernState::start_turn`. |
| `buy: <shop index>` | `TavernAction::Buy`. |
| `sell: <board index>` | `TavernAction::Sell`. |
| `play: <hand index>` or `play: {hand: <index>, pos: <p>}` | `TavernAction::Play` (`pos` defaults to the end of the board). |
| `reposition: {from: <board index>, to: <p>}` | `TavernAction::Reposition`. |
| `activate: <board index>` or `activate: {pos: <index>, target: <p>}` | `TavernAction::Activate`. |
| `refresh`, `upgrade_tavern`, `toggle_freeze`, `end_turn` | The `TavernAction`s. |
| `choose_discover: <option index>` | `TavernAction::ChooseDiscover` (indexes `discover_pending`). |
| `add_to_hand: <unit spec(s)>` | `TavernState::add_to_hand` for each card (as if it were gained: triples, hand limit). |
| `push_board` / `push_hand` / `push_shop: <unit spec(s)>` | Append the cards directly: no triggers, no auras, no triples. |
| `clear: <zone>` or `clear: [<zones>]` | Empty `board`, `hand` and/or `shop`. |
| `set: {<field>: <value>, ...}` | Patch the Tavern state (§4). |
| `deal_hero_damage: <n>` | `TavernState::deal_hero_damage`. |
| `apply_global_unit_auras: {<zone>: <index>}` | `TavernState::apply_global_unit_auras` on one card. |
| `sync_all_auras` | `TavernState::sync_all_auras`. |
| `take_from_pool: <card(s)>` | `CardPool::take_copy` for each card (it must be in the Tavern's catalog). |
| `play_blood_gems: {<zone>: <index>, count: <n>}` | `Unit::play_blood_gems` on one card, with the Tavern's auras. |
| `fight: {board, tier, auras, hand, seed}` | The Tavern's combat against that opponent (`resolve_combat_against`: start/end-of-combat effects, damage, persistence). `tier` defaults to 1, `seed` to the scenario seed. |
| `simulate: {...}` | A standalone battle, as `combat:` (§2.1). In a Tavern, `board_a: tavern` (or `hand_*`, `auras_*`) copies the Tavern's board (hand, auras). |
| `expect: <matcher>` | Check the state (§5). |
| `expect_error: <action step>` | The engine must reject the action, with a message. |
| `expect_legal` / `expect_illegal: <action step>` | `TavernState::is_legal` must agree. |
| `if: <matcher>`, `then: [...]`, `else: [...]` | Run a branch, depending on whether the state matches. |
| `repeat: <n>`, `steps: [...]` | Run the steps `n` times. |

An **index** is a number (negative counts from the end: `-1` is the last card) or a matcher
naming the first matching card: `play: Joyous`, `sell: {is_golden: true}`.

The engine rejecting an action is a failure (`the engine rejected Buy { .. }: ...`). The last
battle (`fight` or `simulate`) is the view's `combat` (§5.1).

---

## 3. Unit specs

A unit spec is a compact string. In setup it builds a unit; in an expectation it is a
matcher (§5.2).

```text
[golden] <Card Name> {token}     "Joyous", "golden Zoatroid +2/+1 taunt", "Joyous 5/5"
<A>/<H> {token}                  "3/2 divine_shield demon" (a vanilla unit)
```

Card names match any minion, spell, token or deity (case-insensitive). A card unit starts as
a plain copy of the card. Tokens, applied in order:

| Token | Setup | Expectation |
| :--- | :--- | :--- |
| `A/H` | Set the current Attack/Health. | Has exactly these stats. |
| `+A/+H` | A buff (`Unit::add_stats`). | Invalid. |
| `golden` / `-golden` | Set or clear the golden flag only (stats are not doubled; give them explicitly). | Is / is not golden. |
| `taunt`, `divine_shield`, `windfury`, `reborn`, `venomous`, `stealth`, `magnetic` | Grant the keyword. | Has the keyword. |
| `-<keyword>` | Remove it. | Lacks it. |
| `beast`, `demon`, ..., `all`, `none` | Set the tribe. | `Tribe::matches` (`none`: exactly no tribe). |
| `tN` / `tier:N` | Set the Tavern Tier. | Has that tier. |

A vanilla `A/H` unit is built like a matchup unit (§9.1), so `card:<slug>` also works there:
`"3/2 card:zoatroid"` is a 3/2 with Zoatroid's card id, name, tier, tribe and keywords.

Where units are listed, a spec can also be a mapping whose `card:` is a spec and whose other
keys patch the unit's fields (§4): `{card: Joyous, attack: 9, is_golden: true}`.

---

## 4. Patches

`tavern:`, `set:`, the `auras*:` of combats and unit mappings are **patches**: they set some
fields of an engine value (its serde form) and keep the others.

- A mapping patches a nested struct (`auras: {deity: {kind: yshaarj}}`).
- A mapping with indices patches list elements (`board: {0: {attack: 5}}`, `-1` is the last).
- Anything else replaces the field (`gold: 10`, `hero_health: null`).
- Unknown fields are errors; map-typed fields accept any key, and `null` removes an entry.

Fields that hold units (`board`, `hand`, `shop`, `discover_pending`, `stitched_stored`,
`hand_a`, `hand_b`) take unit specs: a list (or a single spec) replaces the field,
`{<index>: <spec>}` replaces one unit, `{<index>: {<field>: ...}}` patches one unit, and
`null` sets the field to null. `discover_queue` takes a list of lists of specs.

`PlayerAuras` patches also take:

- `counters: {<card>: [a, b] | n | null}`: the card's `card_counters` entry (`n` means
  `[n, 0]`, `null` removes it);
- effects named by card: `effects: [{card: Gem Day, stacks: 2, duration: Turn}]` (`stacks`
  defaults to 1, `duration` to `Game`; `{Refreshes: 3}` for a tagged duration).

---

## 5. Expectations

An expectation is a **matcher** checked against a **view** of the state.

### 5.1 The view

| Mode | The view |
| :--- | :--- |
| Tavern | The `TavernState` fields (`turn`, `health`, `armor`, `tavern_tier`, `gold`, `max_gold`, `board`, `hand`, `shop`, `discover_pending`, `auras`, ...), plus `pool` and `combat`. |
| Combat | The last `BattleResult`: `outcome` (`a_win`, `b_win`, `draw`), `hero_damage`, `survivors_a`/`_b`, `events`, `hand_a`/`_b`, `auras_a`/`_b`, `dead_units_a`/`_b`. |
| Batch | The `BattleDistribution`: `battles`, `a_wins`, `b_wins`, `draws`, `a_win_rate`, `b_win_rate`, `draw_rate`, `damage: {min, max, mean}`. |

Values are the engine's serde forms, with derived keys:

- **Units** also have `stats` (`"A/H"`), `keywords` (a list of the keywords they have), `card`
  (their card's name), and `battlecry`, `deathrattle`, `rally`, `choose_one` (card flags).
- **Player auras** also have `counters` (card counters keyed by card name; `[0, 0]` for cards
  without one) and `effect_stacks` (total stacks of each card's effects; `0` for cards
  without one). Each effect also has `card`.
- **`pool`** (Tavern): the remaining copies of each catalog card, by name.
- **`combat`** (Tavern): the last battle (as in Combat mode), or `null`.
- **Events** are maps with a `type` (`battle_start`, `attack_declared`, `damage_dealt`, ...)
  and the variant's fields (`docs/combat.md`).
- Enum variants with data are single-key maps: `duration: {Refreshes: 3}`.

### 5.2 Matchers

| Matcher | Against | Matches if |
| :--- | :--- | :--- |
| `{field: m, ...}` | a map or unit | each listed field matches (others are not checked). |
| `{<card>: m, ...}` | a card map | each card's entry (or the default) matches. |
| `[m0, m1, ...]` | a list | same length, item by item. |
| `{<list operators>}` | a list | every operator holds (below). |
| `"<unit spec>"` | a unit | the unit spec holds (§3: card, stats, keywords, tribe, tier, golden). |
| `"<condition>"` | a number | the comparison or range holds (below). |
| scalar | a scalar | equal. `null` only matches null. |

**List operators** (in one mapping; `where` applies first and the rest see the filtered list):

| Operator | Meaning |
| :--- | :--- |
| `where: m` | Keep the items matching `m`. |
| `count: m` | The number of items matches `m` (`count: 3`, `count: ">= 1"`). |
| `all: m` / `any: m` / `none: m` | Every / some / no item matches. |
| `contains: [m...]` | Each matcher matches a *different* item (one matcher may be given alone). |
| `sequence: [m...]` | The matchers match items in this order (not necessarily adjacent). |
| `sum: {field: m}` | The total of a numeric field matches. |
| `0`, `1`, `-1`, ... | That item matches (negative: from the end). |

**Conditions** are strings: `">= 3"`, `"< 10"`, `"!= 0"`, `"== 2"`, `"1..4"` (exclusive),
`"0.4..=1.0"` (inclusive). Each bound is a sum of terms: numbers, `$name` (a captured value)
and `@field` (a field of the enclosing object): `"@max_gold - 2"`.

**Combinators** (single-key mappings):

| Combinator | Matches if |
| :--- | :--- |
| `all_of: [m...]` | every matcher matches. |
| `any_of: [m...]` | some matcher matches. |
| `not: m` | `m` does not match. |
| `capture: name` | always; remembers the value as `$name` for later steps. |
| `same_as: name` | the value equals the captured `$name`. |

```yaml
- name: wrath_weaver_grows_when_a_demon_is_played
  tavern: {board: [Wrath Weaver], hand: ["1/1 beast", "1/1 demon"]}
  steps:
    - expect: {board: {0: {attack: {capture: before}}}}
    - play: "1/1 demon"              # the first hand card matching the spec
  expect:
    health: 29
    board:
      count: 2
      0: {card: Wrath Weaver, attack: "$before + 2"}
      any: "1/1 demon"
    hand: {count: 1, none: {tribe: demon}}
```

---

## 6. Running

### 6.1 Results

Each seed runs the scenario **twice**; both runs must end in the same state (Tavern state,
pool, RNG and every battle), so every scenario also checks determinism. A panic is a failure.

A scenario fails in one of two ways:

- **invalid**: it cannot run as written (an unknown key, card, field or step, a bad index
  type, a matcher that does not apply such as a string against a list);
- **mismatch**: it runs, but an expectation does not hold or the engine rejects an action.

Failures name the step (`step 3 `play: 0`: ...`, `step 2.then.1`, `step 4[2/3].1` inside
`repeat`), the path (`board[0].attack: expected 5, got 4`) and, for expectations, summarize
the state (Tavern line, board, hand, shop, Discover options, last combat).

### 6.2 Known bugs

A scenario with `known_bug: <description>` reproduces a known engine bug. It **passes when it
fails with a mismatch**; it fails if it passes (the bug is fixed: remove `known_bug`) or is
invalid. These tests show as `[known_bug]`, and the run ends with a ledger of them.

---

## 7. `catalog` and `coverage`

The `catalog` test checks the card lists against `tests/scenarios/catalog.yaml`:

- `minions`: each tier's catalog, in order (`tierN_catalog()`, `solo_tier_N_catalog()`,
  `catalog_for("tierN")`), every minion's `tavern_tier`, `full_catalog()`; `minion_ids`: each
  tier's card id range;
- `tiers`: each tier's pool copies per minion (`base_copies_for_tier`), shop capacity
  (`shop_capacity`) and upgrade cost (`base_upgrade_cost`; Tier 7 has neither);
- `spells`: each tier's Tavern spells (`spells::tierN_spells()`, `spells_up_to_tier`);
  `not_in_pool`: the Tavern spells the shop never offers;
- `token_minions`, `token_spells`, `deities`;
- card ids and names are unique across all cards, and unit specs can name every card.

The `coverage` test checks that every
`src/cards/minions/<card>.rs` has a `tests/scenarios/{combat,tavern}/<card>.yaml` and every
`src/cards/spells/<card>.rs` has a `tests/scenarios/tavern/<card>.yaml`
starting with `card: <Card Name>` (the file name is the card's slug: `Fandral's Fortune` ->
`fandrals_fortune`), that every scenario file belongs to a card or a topic, and that
`tests/scenarios/` holds nothing else.

---

## 8. Knockout

A card's scenarios should fail without the card. With the `knockout` feature,
`SEAGLASS_KNOCKOUT=<card id>` disables that card's hooks (`cards::registry::hooks` returns an
empty table for it):

```sh
SEAGLASS_KNOCKOUT=101 cargo test -p seaglass --features knockout --test scenarios -- tavern::joyous::
```

Scenarios that still pass do not test the card's behaviour (or test behaviour that lives
outside its hooks).

---

## 9. Matchup files (`examples/matchups/*.yaml`)

A matchup is a board-vs-board battle for `combat_cli` (and `seaglass.parse_unit`,
`teams_and_state` in Python): one battle at `seed`, or a batch.

| Field | Type | Default | Meaning |
| :--- | :--- | :---: | :--- |
| `name` | string | — | Label. |
| `team_a` / `team_b` | list of unit specs (§9.1) | — | The boards, left to right. |
| `seed` | `u64` | `0` | The battle's seed. |
| `hero_tier_a` / `hero_tier_b` | `u32` | `1` | Hero tiers (for hero damage). |
| `deity_a` / `deity_b` | `cthun`, `yshaarj`, `none` | `cthun` | The side's deity. |
| `deity_stats_a` / `deity_stats_b` | `[i32, i32]` | `[1, 1]` | The deity's stats when it awakens. |
| `defaults` | `{tavern_tier}` | `{}` | Defaults for the units. |
| `batch` | `{base_seed, n}` | — | Run `n` battles instead of one. |

### 9.1 Matchup unit specs

```text
"<attack>/<health> [token] ..."      "3/2 divine_shield card:zoatroid"
```

Tokens: `card:<slug>` (a catalog minion's id, name, tier, tribe and keywords; the slug is its
name in lowercase with spaces, `-` and `'` replaced by `_`), a keyword (`taunt`,
`divine_shield`, `windfury`, `reborn`, `venomous`, `stealth`, `magnetic`), `golden`, a tribe
(`beast`, ..., `all`), `tier:<N>` or `t<N>`. Unknown tokens are errors.
