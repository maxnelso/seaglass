# Declarative Scenario & Matchup File Specification

This document defines the declarative YAML file formats used by Seaglass's test harness and matchup runner. Every scenario struct is deserialized with `#[serde(deny_unknown_fields)]`, so an unknown or misspelled key is a hard parse error—never silently ignored.

Seaglass supports two scenario families:

1. **Combat Scenarios (`tests/combat_scenarios/*.yaml`)** and **Matchups (`examples/matchups/*.yaml`)** — Test or benchmark board-vs-board battles (`combat.md`).
2. **Tavern Scenarios (`tests/tavern_scenarios/*.yaml`)** — Test multi-step Tavern Phase sequences, economy rules, triples, Spellcrafts, and card triggers (`tavern.md`).

---

## Part 1: Combat Scenarios (`tests/combat_scenarios/*.yaml`)

### 1.1 Top-Level Fields

| Field | Type | Required | Default | Meaning |
| :--- | :--- | :---: | :---: | :--- |
| `name` | string | yes | — | Label for the scenario; used in failure messages. |
| `team_a` | list of unit specs | yes | — | Side A's board, left $\to$ right (index `0` = leftmost). May be empty. |
| `team_b` | list of unit specs | yes | — | Side B's board, left $\to$ right. |
| `seed` | `u64` | no | `0` | Seed for single-battle (exact) mode. Ignored in batch mode. |
| `hero_tier_a` | `u32` | no | `1` | Side A hero tier (used for hero-damage calculation). |
| `hero_tier_b` | `u32` | no | `1` | Side B hero tier. |
| `deity_a` / `deity_b` | string | no | `cthun` | Side's Old God Deity (`cthun`, `yshaarj`, or `none`). |
| `deity_stats_a` / `deity_stats_b` | `[i32, i32]` | no | `[1, 1]` | Attack and Health of the side's Deity when it awakens. |
| `defaults` | mapping | no | `{}` | Baseline unit fields (`tavern_tier`). |
| `expect` | mapping | mode 1 | — | Exact expectations for a single deterministic battle (§1.4). |
| `batch` | mapping | mode 2 | — | Batch settings (`base_seed`, `n`) (§1.5). |
| `expect_stats` | mapping | mode 2 | — | Statistical rate bounds over a batch rollout (§1.5). |

### 1.2 Compact Unit Spec Syntax

Every entry in `team_a`, `team_b`, and `survivors_*` is a whitespace-separated string:

```text
"<attack>/<health> [token] [token] ..."
```

- **First token**: `<attack>/<health>` (signed integers, e.g. `"3/2"` or `"0/4"`).
- **Optional trailing tokens** (in any order):
  - **Card template**: `card:<slug>` (e.g. `card:crackling_cyclone`, `card:risen_rider`, `card:zoatroid`), which populates `card_id`, `name`, `tavern_tier`, `tribe`, and printed keywords from the catalog.
  - **Keywords**: `taunt`, `divine_shield`, `windfury`, `reborn`, `venomous`, `stealth`, `magnetic`, `golden`. (`divine_shield` sets `inherent_divine_shield = true`.)
  - **Tribes**: `aberration`, `beast`, `demon`, `dragon`, `elemental`, `mech`, `murloc`, `pirate`, `quilboar`, `undead`, `all`.
  - **Tavern tier**: `tier:<N>` or `t<N>` (e.g., `tier:1` or `t3`), overriding `defaults.tavern_tier`.
  - Any unrecognized token is an immediate parse error.

### 1.3 Combat Scenario Modes

A combat YAML file operates in one of three modes:
1. **Exact (single battle)** — Sets `expect` (and neither `batch` nor `expect_stats`). Runs one battle at `seed` and asserts the exact outcome.
2. **Statistical (batch)** — Sets both `batch` and `expect_stats` (and no `expect`). Runs `batch.n` battles from `batch.base_seed` and checks win/draw rate bounds.
3. **Matchup (report-only, `examples/matchups/*.yaml`)** — Sets `team_a` and `team_b` (and optional `batch`), with no `expect` or `expect_stats`.

### 1.4 Exact Expectations (`expect`)

```yaml
name: divine_shield_absorbs_first_hit
seed: 42
team_a:
  - "3/2 divine_shield"
team_b:
  - "2/3"
expect:
  result: team_a_win        # team_a_win | team_b_win | draw (required)
  survivors_a: ["3/2"]      # optional: exact surviving board (attack, current health, taunt, divine_shield)
  survivors_b: []           # optional: exact surviving board
  hero_damage: 2            # optional: exact hero damage dealt to loser
```

### 1.5 Statistical Expectations (`batch` + `expect_stats`)

```yaml
name: coin_flip_first_attacker_is_fair
batch:
  base_seed: 1
  n: 4000
team_a:
  - "0/3 taunt"
  - "2/2"
team_b:
  - "0/2 taunt"
  - "2/2"
expect_stats:
  a_win_rate: { min: 0.45, max: 0.55 }
  draw_rate:  { min: 0.45, max: 0.55 }
  b_win_rate: { min: 0.0,  max: 0.0 }
```

---

## Part 2: Tavern Scenarios (`tests/tavern_scenarios/*.yaml`)

Tavern scenarios test multi-step Recruit Phase sequences, pool draws, economy rules, Triples, Activate abilities, and card triggers.

### 2.1 Top-Level Fields

| Field | Type | Required | Default | Meaning |
| :--- | :--- | :---: | :---: | :--- |
| `name` | string | yes | — | Label for the scenario. |
| `seed` | `u64` | no | `0` | Seed for the scenario's `SplitMix64` RNG. |
| `catalog` | string | no | `"tier1"` | Card catalog: `"tier1"` / `"solo_tier_1"` / `"full"`. |
| `steps` | list of steps | yes | — | Ordered sequence of setup commands and `TavernAction`s (§2.2). |
| `expect` | mapping | yes | — | State assertions verified after all `steps` complete (§2.3). |

### 2.2 Step Commands (`steps`)

Each item in `steps` is either a unit variant or a single-key mapping:

| YAML Step | Meaning |
| :--- | :--- |
| `start_turn` | Execute `TavernState::start_turn(&mut pool, &mut rng)`. |
| `set_gold: <u32>` | Test helper: set `state.gold` directly. |
| `set_deity: <cthun\|yshaarj\|none>` | Test helper: set the player's Old God Deity kind. |
| `give_hand: "<Card Name>"` | Test helper: instantiate `<Card Name>` from `catalog`, push to `hand`, and run Triple check. |
| `give_board: "<Card Name>"` | Test helper: instantiate `<Card Name>` from `catalog` and push directly to `board`. |
| `buy: <shop_index>` | Execute `TavernAction::Buy { shop_index }`. |
| `play: { hand: <usize>, pos: <usize> }` | Execute `TavernAction::Play { hand_index: hand, board_pos: pos }`. |
| `sell: <board_pos>` | Execute `TavernAction::Sell { board_pos }`. |
| `reposition: { from: <usize>, to: <usize> }` | Execute `TavernAction::Reposition { from_pos: from, to_pos: to }`. |
| `activate: { pos: <usize>, target: <Option<usize>> }` | Execute `TavernAction::Activate { board_pos: pos, target_pos: target }`. |
| `refresh` | Execute `TavernAction::Refresh`. |
| `upgrade_tavern` | Execute `TavernAction::UpgradeTavern`. |
| `toggle_freeze` | Execute `TavernAction::ToggleFreeze`. |
| `choose_discover: <option_index>` | Execute `TavernAction::ChooseDiscover { option_index }`. |
| `end_turn` | Execute `TavernAction::EndTurn`. |

If any action step is illegal or references an unknown card name, the scenario fails immediately with an error naming the step index.

### 2.3 Tavern Expectations (`expect`)

All fields in `expect` are optional; only the specified fields are asserted after `steps` finish:

| Field | Type | Assertion |
| :--- | :--- | :--- |
| `turn` | `u32` | `state.turn == turn` |
| `tavern_tier` | `u32` | `state.tavern_tier == tavern_tier` |
| `gold` | `u32` | `state.gold == gold` |
| `health` | `i32` | `state.health == health` |
| `bonus_gold_next_turn` | `u32` | `state.bonus_gold_next_turn == bonus_gold_next_turn` |
| `upgrade_cost` | `u32` | `state.upgrade_cost == upgrade_cost` |
| `is_frozen` | `bool` | `state.is_frozen == is_frozen` |
| `board_count` | `usize` | `state.board.len() == board_count` |
| `hand_count` | `usize` | `state.hand.len() == hand_count` |
| `shop_count` | `usize` | `state.shop.len() == shop_count` |
| `board_has_golden` | `bool` | `state.board.iter().any(\|u\| u.is_golden) == board_has_golden` |
| `discover_pending` | `bool` | `state.discover_pending.is_some() == discover_pending` |
| `board_first_stats` | `[i32, i32]` | `[state.board[0].attack, state.board[0].health] == board_first_stats` |
| `board_contains_name` | `String` | `state.board.iter().any(\|u\| u.name == board_contains_name)` |
| `deity_stats` | `[i32, i32]` | `[state.auras.deity.attack, state.auras.deity.health] == deity_stats` |

### 2.4 Worked Tavern Scenario Example

```yaml
name: joyous_buffs_deity_and_zoatroid_sells_tentacle
seed: 20
catalog: tier1
steps:
  - start_turn
  - set_deity: cthun
  - give_hand: "Joyous"
  - give_hand: "Zoatroid"
  - play: { hand: 0, pos: 0 }
  - play: { hand: 0, pos: 1 }
  - sell: 1
  - play: { hand: 0, pos: 1 }
expect:
  deity_stats: [3, 2]
  board_count: 2
  board_contains_name: "Aberrant Tentacle"
```
