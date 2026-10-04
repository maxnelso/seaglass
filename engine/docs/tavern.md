# Tavern Phase (Recruit Phase) Specification

This document defines the exact rules and state transitions for **Seaglass's Tavern Phase (Recruit Phase)**. Like [`combat.md`](combat.md), this specification is the single source of truth: every rule, cost, and state transition is pinned here before implementation.

The Tavern Phase is modeled as a **card-agnostic sequential decision state machine**: one Tavern turn consists of a start-of-turn sequence (`start_turn`), followed by a sequence of discrete player actions (`TavernAction`) ending with `EndTurn`. Individual card definitions live in the card catalog and plug into the general hooks defined below.

---

## 1. Scope

### Included
- **Economy & Progression**: Start-of-turn gold refill (`3` to `10` gold), bonus gold carry-over (`bonus_gold_next_turn`), Tavern Tiers `1..=6`, and start-of-turn upgrade cost decay.
- **Shared Finite Card Pool (`CardPool`)**: Tier-dependent copy counts (`15 / 15 / 13 / 11 / 9 / 7`), uniform sampling without replacement across remaining copies, and pool return on shop refresh, minion sale, and unchosen Discover options.
- **Bob's Shop**: Tier-dependent shop capacity, Refresh (`1` gold), and Freeze/Unfreeze toggle (`0` gold).
- **Core Actions**: `Buy` (`3` gold, Shop $\to$ Hand), `Play` (`0` gold, Hand $\to$ Board slot or spell target), `Sell` (`+1` gold, Board $\to$ Pool), `Reposition` (`0` gold, reorder Board slots), `ChooseDiscover`, and `EndTurn`.
- **Triples, Discover & Choose One Sub-State**: Automatic combination of 3 non-golden copies across `board` and `hand` into 1 Golden copy in `hand` with summed buffs, plus a `discover_pending` / `discover_queue` sub-state for Triple rewards, Discover effects (`Clever Castaway`, `Patient Scout`, `A New Sprout`, `Search Through Time`), and `Choose One` cards (`Crater Miner`, `Intrepid Botanist`, `Gem Day`, `Alliance Flag`).
- **Tavern Spells (Tiers 1–2)**: All 8 Tier 1 and 7 Tier 2 Tavern Spells (`src/cards/spells/`, one file per spell), including shop spell offers, spell cost discounts (`Ominous Seer`), health-cost spells (`Hasty Excavation`), free refreshes (`Leaf Through the Pages`), max-gold increases (`Strike Oil`), hand-locked discovers (`Search Through Time`), next-combat win buffs (`Winner's Bread`), and spell stat scaling (`Intrepid Botanist`).
- **General Tavern Hooks (Tiers 1–2)**: Battlecries, after-play/magnetize board observers (`Wrath Weaver`, `Mechagnome Interpreter`), Volumizer scaling (`Blue` / `Green` / `Red Volumizer`), `Activate` abilities targeting board/hand/shop (`Suspicious Prisonguard`, `Brain Rotter`, `Clever Castaway`, `Decoy Conjurer`, `Lurking Lionfish`), `Lockbox` countdown (`Bilgewater Breakout`), `Demon Fodder` refreshes (`Laboratory Assistant`), on-sell triggers (`Zoatroid`, `Fire Baller`, `Snow Baller`, `Patient Scout`, `Sellemental`, `Tad`, `Wandering Willbreaker`), end-of-turn triggers (`Lullabot`, `Surfing Sylvar`), and hero damage rewind (`Soul Rewinder`).

### Deferred to Later Phases
- Hero Powers, Buddy meters, Quests, Trinkets, Darkmoon Prizes, and Anomalies.
- Tier 3–6 minions and Tier 3–6 Tavern Spells.

---

## 2. Shared Finite Card Pool (`CardPool`)

All players in a match draw from a shared `CardPool`. Each unique purchasable minion template (`CardId`) starts with a fixed number of copies in the pool based on its Tavern Tier:

| Minion Tavern Tier | Copies in Pool per Unique Card |
| :---: | :---: |
| **Tier 1** | 15 copies |
| **Tier 2** | 15 copies |
| **Tier 3** | 13 copies |
| **Tier 4** | 11 copies |
| **Tier 5** | 9 copies |
| **Tier 6** | 7 copies |

### Pool Draw & Return Rules

1. **Drawing from Pool (`draw_from_pool(max_tier, rng)`)**:
   - Eligible cards are all remaining copies in `CardPool` with `tavern_tier <= max_tier`.
   - One copy is sampled uniformly across all remaining individual copies (a card with 15 copies remaining is $15\times$ as likely to be drawn as a card with 1 copy remaining).
   - The drawn card's remaining count in `CardPool` is decremented by `1`.
2. **Discover Draw (`draw_discover_options(exact_tier, count, rng)`)**:
   - Draws up to `count` (`3`) **distinct `card_id`s** of exact tier `exact_tier`, weighted by remaining copies, decrementing each drawn card's remaining count by `1`.
3. **Returning to Pool (`return_unit(unit)`)**:
   - **Shop Refresh / Unfrozen Turn Start**: Every minion currently in `shop` returns `1` copy to the pool *before* drawing the new shop.
   - **Selling a Non-Golden Minion**: Returns **`1` copy** of its `card_id` to the pool (capped at the tier's base copy limit).
   - **Selling a Golden Minion**: Returns **`3` copies** of its `card_id` to the pool (unless the card template is marked intrinsically Golden, in which case it returns `1` copy).
   - **Unchosen Discover Options**: When 1 of the offered Discover options is chosen, the unchosen options return `1` copy each to the pool.
   - Generated tokens (minions not in the purchasable catalog) do not add copies to `CardPool` when sold.

---

## 3. Tavern State (`TavernState`)

### 3.1 Player & Economy Fields
- `turn: u32` — Current turn number (`0` before game start; `1` on first Tavern turn).
- `health: i32` — Hero health (starts at `30`).
- `tavern_tier: u32` — Current Tavern Tier in `1..=6` (starts at `1`).
- `max_gold: u32` — Base gold cap for the turn: `min(10, 2 + turn)` (`Turn 1 = 3g`, `Turn 2 = 4g`, ..., `Turn 8+ = 10g`).
- `gold: u32` — Current spendable gold (`0..=10`).
- `bonus_gold_next_turn: u32` — Extra gold queued for next turn's `start_turn`.
- `upgrade_cost: u32` — Current gold cost to upgrade to `tavern_tier + 1` (`0` when `tavern_tier == 6`).
- `is_frozen: bool` — Whether Bob's shop is frozen for the next `start_turn`.
- `discover_pending: Option<Vec<Unit>>` — When `Some(options)`, the player must take `ChooseDiscover { option_index }` before any other action is legal.
- `pending_spellcrafts: Vec<Unit>` — FIFO queue of generated Spellcraft spells waiting for an open hand slot (§7.4).
- `auras: PlayerAuras` — Persistent game-long scaling counters (§7.5).

### 3.2 Card Zones
1. **`board`**: Ordered list of `Unit`s (`0..=7` minions, index `0` = leftmost).
2. **`hand`**: Ordered list of `Unit`s (`0..=10` cards: minions and hand spells).
3. **`shop`**: Ordered list of `Unit`s currently offered by Bob (`0..=6` minions).

### 3.3 Tavern Tier Table

| Current Tavern Tier | Shop Minion Capacity | Base Cost to Upgrade to Next Tier |
| :---: | :---: | :---: |
| **1** | 3 | 5 Gold (to Tier 2) |
| **2** | 4 | 7 Gold (to Tier 3) |
| **3** | 4 | 8 Gold (to Tier 4) |
| **4** | 5 | 10 Gold (to Tier 5) |
| **5** | 5 | 10 Gold (to Tier 6) |
| **6** | 6 | 0 (Max Tier) |

---

## 4. Turn Start Sequence (`start_turn`)

At the beginning of Turn `t`:

1. **Expire Previous Turn's Temporary Buffs & Spellcrafts**:
   - Discard all uncast temporary Spellcraft cards from `hand` and clear `pending_spellcrafts`.
   - Revert temporary buffs (`temp_attack`, `temp_health`, `temp_divine_shield`, `temp_taunt`, `temp_windfury`) on every minion on `board`, clamping `health >= 1`, and reset per-turn card flags.
2. **Increment Turn & Refill Gold**:
   - `turn += 1`
   - `max_gold = min(10, 2 + turn)`
   - `gold = min(10, max_gold + bonus_gold_next_turn)`
   - `bonus_gold_next_turn = 0`
3. **Decay Upgrade Cost**:
   - If `turn > 1` and `tavern_tier < 6`: `upgrade_cost = upgrade_cost.saturating_sub(1)`.
4. **Populate Bob's Shop**:
   - Let `cap = shop_capacity(tavern_tier)`.
   - **If `is_frozen == true`**:
     - Keep all minions currently in `shop` in their existing order.
     - Draw up to `cap - shop.len()` new minions from `CardPool` (`tier <= tavern_tier`) and append them to `shop`.
     - Set `is_frozen = false`.
   - **If `is_frozen == false`**:
     - Return all minions currently in `shop` to `CardPool`.
     - Draw `cap` new minions from `CardPool` (`tier <= tavern_tier`).
5. **Generate Start-of-Turn Spellcrafts**:
   - For each Spellcraft-producing minion on `board` (left $\to$ right), generate its Spellcraft spell into `hand` (or `pending_spellcrafts` if `hand.len() == 10`).

---

## 5. Triples & Golden Minion Resolution

Whenever a minion is added to `hand` (via `Buy`, `ChooseDiscover`, or card generation), the engine checks whether the player owns **3 non-golden copies** of the same `card_id` across `board` and `hand`.

### 5.1 Triple Combination (`check_and_resolve_triple`)
If 3 non-golden copies exist:
1. Remove the 3 copies (scanning `board` first, then `hand`, preserving relative order of all other cards).
2. Let `base_atk` and `base_hp` be the card's printed base stats.
3. Sum all buffs accumulated across the 3 copies:
   $$\text{golden\_atk} = 2 \cdot \text{base\_atk} + \sum_{i=1}^{3} (\text{copy}_i.\text{attack} - \text{base\_atk})$$
   $$\text{golden\_hp} = 2 \cdot \text{base\_hp} + \sum_{i=1}^{3} (\text{copy}_i.\text{health} - \text{base\_hp})$$
4. Union boolean keywords (`taunt`, `divine_shield`, `inherent_divine_shield`, `windfury`, `reborn`, `magnetic`) across the 3 copies, preserve triggers and tribe, and set `is_golden = true`.
5. Append the Golden minion to `hand` and drain `pending_spellcrafts` if space opened.

### 5.2 Playing a Golden Minion (Triple Reward Discover)
When a Golden minion is played from `hand` to `board`:
1. The Golden minion is inserted at `board[board_pos]`, running its on-play Battlecries and board triggers (doubled for Golden).
2. If the minion was formed by a Triple (not an intrinsically Golden template):
   - Let `reward_tier = min(6, tavern_tier + 1)`.
   - Draw up to 3 distinct `card_id`s of exact tier `reward_tier` from `CardPool`. If non-empty, set `discover_pending = Some(options)`.
3. While `discover_pending.is_some()`, only `ChooseDiscover { option_index }` (`0..options.len()`) is legal.
4. Executing `ChooseDiscover { option_index }` appends `options[option_index]` to `hand`, returns unchosen options to `CardPool`, sets `discover_pending = None`, and runs `check_and_resolve_triple`.

---

## 6. Tavern Actions & Legality Rules (`TavernAction`)

If `discover_pending.is_some()`, **only `ChooseDiscover` is legal**. Otherwise:

| Action | Preconditions | Effect |
| :--- | :--- | :--- |
| `Buy { shop_index }` | `gold >= 3`, `shop_index < shop.len()`, `hand.len() < 10` | `gold -= 3`; move `shop[shop_index]` to `hand`; check for Triple. |
| `Play { hand_index, board_pos }` (Minion) | `hand_index < hand.len()`, card is a minion, `board.len() < 7`, `board_pos <= board.len()` | Remove from `hand`, apply on-play Battlecries/auras, insert at `board[board_pos]`, run post-play triggers, trigger Discover if tripled Golden, drain pending Spellcrafts. |
| `Play { hand_index, board_pos }` (Targeted Spell) | `hand_index < hand.len()`, card is a targeted spell, `board_pos < board.len()` | Remove spell from `hand`, apply spell effect to `board[board_pos]`, increment `spells_played`, drain pending Spellcrafts. |
| `Play { hand_index, board_pos: 0 }` (Untargeted Spell) | `hand_index < hand.len()`, card is an untargeted spell (e.g. gold coin) | Remove spell from `hand`, apply player/board effect, increment `spells_played`, drain pending Spellcrafts. |
| `Sell { board_pos }` | `board_pos < board.len()` | Remove `board[board_pos]`, return copies to `CardPool`, `gold = min(10, gold + 1)`, run on-sell triggers. |
| `Reposition { from_pos, to_pos }` | `from_pos < board.len()`, `to_pos < board.len()`, `from_pos != to_pos` | Move `board[from_pos]` to `board[to_pos]`. Costs `0` gold. |
| `Refresh` | `gold >= 1` | `gold -= 1`, `is_frozen = false`, return current `shop` to `CardPool`, draw `shop_capacity(tavern_tier)` new minions. |
| `UpgradeTavern` | `tavern_tier < 6`, `gold >= upgrade_cost` | `gold -= upgrade_cost`, `tavern_tier += 1`, `upgrade_cost = base_upgrade_cost(tavern_tier)`. |
| `ToggleFreeze` | Always legal | `is_frozen = !is_frozen`. Costs `0` gold. |
| `ChooseDiscover { option_index }` | `discover_pending == Some(opts)`, `option_index < opts.len()` | Move `opts[option_index]` to `hand`, return others to `CardPool`, clear `discover_pending`, check for Triple. |
| `EndTurn` | Always legal when `discover_pending.is_none()` | Run end-of-turn triggers and conclude the Tavern turn. |

---

## 7. General Tavern Trigger Hooks & Subsystems

All card-specific Tavern abilities plug into one of the following hooks (with magnitudes or repetitions doubled when `is_golden == true`):

### 7.1 On-Play (Battlecries & Board Observers)
When a minion is played from `hand` to `board[board_pos]`:
1. **Pre-insertion Battlecries & Global Aura Application**: Grants economy bonuses (`bonus_gold_next_turn`), generates hand spells/tokens (if `hand.len() < 10`), updates persistent `PlayerAuras`, and applies existing tribal/summon auras to the newly played minion.
2. **Post-insertion Board Triggers**:
   - If the played minion has a Spellcraft ability, it generates its Spellcraft spell immediately.
   - Board-wide or tribal buffs from the played minion apply to other matching friendly minions.
   - Friendly board observers ("whenever/after you play a minion of tribe X") fire in left-to-right board order.

### 7.2 Hero Self-Damage Interception
When a Tavern trigger would deal damage to the friendly hero:
- If the board contains any friendly damage-prevention observer, the hero takes `0` damage and each such observer applies its on-prevent buff instead.
- Otherwise, `state.health` is reduced by the damage amount.

### 7.3 On-Sell Triggers
When a minion at `board[board_pos]` is sold:
- May draw or generate minion(s)/token(s) into `hand` (if `hand.len() < 10`, checking for Triples after each addition).
- May increment persistent scaling counters and buff remaining minions on `board`.

### 7.4 Spellcraft & Hand Spell System
- **Generation & Pending Queue**: Whenever a Spellcraft spell is generated (on play or at `start_turn`), it is appended to `hand` if `hand.len() < 10`, or pushed to `pending_spellcrafts` if `hand` is full. Whenever space opens in `hand`, `drain_pending_spellcrafts()` moves queued spells into `hand` in FIFO order.
- **Temporary vs. Permanent Application**:
  - Permanent hand spells apply their stat/keyword buffs permanently.
  - Temporary Spellcraft spells record their applied stats and newly granted keywords in the target's `temp_*` fields so they revert at the next `start_turn`—unless the target minion has an active "first Spellcraft each turn is permanent" effect, in which case the buff is applied permanently and latches that flag for the rest of the turn.
- **Stat-Threshold Check**: Any stat increase in the Tavern Phase immediately runs the target's stat-threshold observers.

### 7.5 End-of-Turn Triggers (`EndTurn`)
When `EndTurn` is executed:
1. Board end-of-turn triggers fire in left-to-right board order (self-buffs, adjacent buffs).
2. Hand end-of-turn triggers fire in left-to-right hand order (e.g., summoning copies from hand onto `board` if `board.len() < 7`).

### 7.6 Persistent Game-Long Scaling (`PlayerAuras`)
`TavernState` tracks game-long counters that persist across turns and apply to both Tavern actions and Combat simulations:
- Tribal and token stat bonuses (applied to existing and newly played/summoned matching units).
- Summon/death counters for scaling minions.
- Total spells cast (`spells_played`) and sell-scaling stacks.
