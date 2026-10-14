# Tavern Phase (Recruit Phase) Specification

This document defines the exact rules and state transitions for **Seaglass's Tavern Phase (Recruit Phase)**. Like [`combat.md`](combat.md), this specification is the single source of truth: every rule, cost, and state transition is pinned here before implementation.

**Duos** Taverns (a team of two players with one Health pool, who can Pass cards to each other) are specified in [`duos_tavern.md`](duos_tavern.md), which builds on this document.

The Tavern Phase is modeled as a **card-agnostic sequential decision state machine**: one Tavern turn consists of a start-of-turn sequence (`start_turn`), followed by a sequence of discrete player actions (`TavernAction`) ending with `EndTurn`. Individual card definitions live in the card catalog and plug into the general hooks defined below.

---

## 1. Scope

### Included
- **Economy & Progression**: Start-of-turn gold refill (`3` to `10` gold), bonus gold carry-over (`bonus_gold_next_turn`), Tavern Tiers `1..=6`, and start-of-turn upgrade cost decay.
- **Shared Finite Card Pool (`CardPool`)**: Tier-dependent copy counts (`15 / 15 / 13 / 11 / 9 / 7`), uniform sampling without replacement across remaining copies, and pool return on shop refresh, minion sale, and unchosen Discover options.
- **Bob's Shop**: Tier-dependent shop capacity, Refresh (`1` gold), and Freeze/Unfreeze toggle (`0` gold).
- **Core Actions**: `Buy` (`3` gold, Shop $\to$ Hand), `Play` (`0` gold, Hand $\to$ Board slot or spell target), `PlayOnShop` (`0` gold, a spell cast on a minion in the shop), `Sell` (`+1` gold, Board $\to$ Pool), `Reposition` (`0` gold, reorder Board slots), `ChooseDiscover`, and `EndTurn`.
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
   - **Shop Refresh / Unfrozen Turn Start**: Every minion currently in `shop` returns its copies to the pool (`1` copy, or as a sold Golden below) *before* drawing the new shop.
   - **Selling a Non-Golden Minion**: Returns **`1` copy** of its `card_id` to the pool (capped at the tier's base copy limit).
   - **Selling a Golden Minion**: Returns **`3` copies** of its `card_id` to the pool if it was formed by a Triple. A Golden that an effect made from a single copy (`Golden Touch`, `Captain Sanders`, a `Lockbox`, ...; `golden_from_effect`) or that is always Golden (`Aureate Laureate`, `intrinsic_golden`) returns `1` copy, even though the former still gives a Triple Reward (§5.2).
   - **Unchosen Discover Options**: When 1 of the offered Discover options is chosen, the unchosen options return `1` copy each to the pool.
   - Generated tokens (minions not in the purchasable catalog) do not add copies to `CardPool` when sold.

---

## 3. Tavern State (`TavernState`)

### 3.1 Player & Economy Fields
- `turn: u32` — Current turn number (`0` before game start; `1` on first Tavern turn).
- `health: i32` — Hero health (starts at `30`).
- `tavern_tier: u32` — Current Tavern Tier in `1..=6` (starts at `1`).
- `max_gold: u32` — Gold cap for the turn: `min(10, 2 + turn) + auras.base_max_gold_bonus` (`Turn 1 = 3g`, `Turn 2 = 4g`, ..., `Turn 8+ = 10g`, plus the permanent bonus). "Increase your maximum Gold by `N`" (`Strike Oil`, `Snare Trapper`) adds `N` to both `max_gold` and `base_max_gold_bonus` but gives no Gold right away; `max_gold` can exceed `10`.
- `gold: u32` — Current spendable gold (it can exceed `10`).
- `bonus_gold_next_turn: u32` — Extra gold queued for next turn's `start_turn`.
- `upgrade_cost: u32` — Current gold cost to upgrade to `tavern_tier + 1` (`0` when `tavern_tier == 6`).
- `is_frozen: bool` — Whether Bob's shop is frozen for the next `start_turn`.
- `discover_pending: Option<Vec<Unit>>` — When `Some(options)`, the player must take `ChooseDiscover { option_index }` before any other action is legal.
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

1. **Expire the Previous Turn's Temporary Effects**:
   - Discard the temporary cards still in `hand` (`CardFlags::TEMPORARY`: a Spellcraft spell that reached the hand after the player ended their turn, §7.4), without discard effects.
   - On every minion on `board`: its start-of-turn upkeep (`turn_start_unit`: what lasted until next turn ends), the end of a Divine Shield "until next turn" (`temp_divine_shield`), and its per-turn charges reset. A Divine Shield "until next turn" also expires on minions in `hand`.
2. **Increment Turn & Refill Gold**:
   - `turn += 1`
   - `max_gold = min(10, 2 + turn) + auras.base_max_gold_bonus`
   - `gold = max_gold + bonus_gold_next_turn`
   - `bonus_gold_next_turn = 0`
3. **Decay Upgrade Cost**:
   - If `turn > 1` and `tavern_tier < 6`: `upgrade_cost = upgrade_cost.saturating_sub(1)`.
4. **Populate Bob's Shop**:
   - Let `cap = shop_capacity(tavern_tier)`. The shop has `cap` slots, plus a spell slot when it offers Tavern spells (`include_shop_spells`).
   - **If `is_frozen == true`**:
     - Keep every card currently in `shop` (minions and spells) in its existing order: each one fills a slot, whatever it is.
     - If the shop offers spells and no spell is left in it, the spell slot is free.
     - Fill the other free slots (`cap + 1 - shop.len()` with shop spells, `cap - shop.len()` without, less a free spell slot) with new minions from `CardPool` (`tier <= tavern_tier`), appended to `shop`; then append a random Tavern spell (`tier <= tavern_tier`) for a free spell slot. So in a Tavern of spells (`Saloon's Finest`), each slot bought from refills with a minion, and the spell slot refills with a spell only once no spell is left.
     - Set `is_frozen = false`.
   - **If `is_frozen == false`**:
     - Return all minions currently in `shop` to `CardPool`.
     - Draw `cap` new minions from `CardPool` (`tier <= tavern_tier`), then a random Tavern spell if the shop offers spells.
5. **Spellcraft**:
   - Each Spellcraft minion on `board` (left $\to$ right) gives its Spellcraft spell, if there is room in `hand` (§7.4).

---

## 5. Triples & Golden Minion Resolution

Whenever a minion is added to `hand` (via `Buy`, `ChooseDiscover`, or card generation), the engine checks whether the player owns **3 non-golden copies** of the same `card_id` across `board` and `hand`. Buying still needs a free hand slot (§6), even for the copy that would complete a triple.

While a Tavern trigger pass resolves (the minions that react to one event, left to right: the end-of-turn triggers, the after-buy, Gold-spent or after-play observers, ...), the Triples it completes wait until the pass is over, then resolve. So the minions of the pass keep their places, every one of them triggers, and a trigger keeps acting on its own minion: two `Resourceful Robot`s both Magnetize a Volumizer even when the first one's copy completes a Triple with Volumizers on the board, and so do both Magnetizations of a Golden one.

### 5.1 Triple Combination (`check_and_resolve_triple`)
If 3 non-golden copies exist:
1. Remove the 3 copies (scanning `board` first, then `hand`, preserving relative order of all other cards).
2. Let `base_atk` and `base_hp` be the card's printed base stats.
3. Sum all buffs accumulated across the 3 copies:
   $$\text{golden\_atk} = 2 \cdot \text{base\_atk} + \sum_{i=1}^{3} (\text{copy}_i.\text{attack} - \text{base\_atk})$$
   $$\text{golden\_hp} = 2 \cdot \text{base\_hp} + \sum_{i=1}^{3} (\text{copy}_i.\text{health} - \text{base\_hp})$$
4. Union boolean keywords (`taunt`, `divine_shield`, `inherent_divine_shield`, `windfury`, `reborn`, `magnetic`) across the 3 copies, preserve triggers and tribe, and set `is_golden = true`. A Divine Shield that the copies only have until next turn (`temp_divine_shield`, `Ichoron the Protector`) stays temporary on the Golden: it expires at the start of the next turn, even in `hand`.
5. Append the Golden minion to `hand`.

### 5.2 Playing a Golden Minion (Triple Reward Discover)
When a Golden minion is played from `hand` to `board`:
1. The Golden minion is inserted at `board[board_pos]`, running its on-play Battlecries and board triggers (doubled for Golden).
2. Unless the Golden doesn't give a Triple Reward (`intrinsic_golden`: only cards whose text says so, `Aureate Laureate` and the Golden minions `Silent Deliverer` gives), whether it was formed by a Triple or made Golden by an effect (`Golden Touch`, a `Lockbox`):
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
| `Play { hand_index, board_pos }` (Minion) | `hand_index < hand.len()`, card is a minion, `board.len() < 7`, `board_pos <= board.len()` | Remove from `hand`, apply on-play Battlecries/auras, insert at `board[board_pos]`, run post-play triggers, trigger Discover if tripled Golden. |
| `Play { hand_index, board_pos }` (Targeted Spell) | `hand_index < hand.len()`, card is a targeted spell, `board_pos < board.len()`, `board[board_pos]` is a valid target (§7.4: e.g. an Undead for `Butchering`, a minion with a type for `Chef's Choice`) | Remove spell from `hand`, cast it on `board[board_pos]` (§7.4). |
| `PlayOnShop { hand_index, shop_index }` (Spell on a Tavern minion) | `hand_index < hand.len()`, card is a spell that can target a minion in the Tavern (its text says "a minion", §7.4), `shop[shop_index]` is a minion | Remove spell from `hand`, cast it once on `shop[shop_index]` (§7.4). |
| `Play { hand_index, board_pos: 0 }` (Untargeted Spell) | `hand_index < hand.len()`, card is an untargeted spell (e.g. gold coin) | Remove spell from `hand`, cast it (§7.4: apply player/board effect). |
| `Sell { board_pos }` | `board_pos < board.len()` | Remove `board[board_pos]`, return copies to `CardPool`, `gold = min(10, gold + 1)`, run on-sell triggers. |
| `Reposition { from_pos, to_pos }` | `from_pos < board.len()`, `to_pos < board.len()`, `from_pos != to_pos` | Move `board[from_pos]` to `board[to_pos]`. Costs `0` gold. |
| `Refresh` | `gold >= 1`, `auras.free_refreshes > 0`, or a Health Refresh (`has_health_refresh`: a `HEALTH_REFRESHES` minion has `charges` left and `health + armor > 1` or hero damage is rewound) | Pay: if `free_refreshes > 0`, `free_refreshes -= 1` (a free Refresh still uses a `HEALTH_REFRESHES` charge if there is one: it costs 0 Health instead of 0 Gold); else if a Health Refresh is available, use a charge (left-most minion first) and deal 1 damage to the hero; else `gold -= 1`. Then `is_frozen = false`, return current `shop` to `CardPool`, draw `shop_capacity(tavern_tier)` new minions. |
| `UpgradeTavern` | `tavern_tier < 6`, `gold >= upgrade_cost` | `gold -= upgrade_cost`, `tavern_tier += 1`, `upgrade_cost = base_upgrade_cost(tavern_tier)`. |
| `ToggleFreeze` | Always legal | `is_frozen = !is_frozen`. Costs `0` gold. |
| `ChooseDiscover { option_index }` | `discover_pending == Some(opts)`, `option_index < opts.len()` | Move `opts[option_index]` to `hand`, return others to `CardPool` (Duos: those marked `unchosen_to_teammate` go to the teammate instead, `docs/duos_tavern.md` §8.3), clear `discover_pending`, check for Triple, then the chosen card's "when you Discover this" text (`obtained`) and the `discover` observers. A Choose-One option takes effect instead (once per cast of the spell that offered it, §7.4). |
| `EndTurn` | Always legal when `discover_pending.is_none()` | Run end-of-turn triggers, discard the temporary cards still in `hand` (Spellcraft spells, §7.4) without discard effects, and conclude the Tavern turn. |

**Buying a spell** costs its `spell_cost` in Gold, or in Health if it `costs_health` (`Hasty Excavation`; legal while `health + armor` stays above the cost, or hero damage is prevented), less `auras.next_spell_discount` (`Ominous Seer`, never below `0`). Buying any spell uses up the whole discount; until then it carries over to later turns.

**Buying a minion**: the effects on the minion being bought (`minion_bought`) resolve left to right in board order, before it reaches the hand. Once it is in the hand (and its triple checked), its own "when you buy this" text (`obtained`, Duos `Mirror Monster`) resolves, then the `after_buy` observers (after any card is bought). With a `Stone Age Slab` to the left of an activated `Living Prison`, the Prison gains the stats the Slab gave the minion; to its right, the stats the minion had.

**Card groups** (the spells of a `Wandering Willbreaker`, the Aberrations of a `Faceless Operative`: `willbreaker_group`): playing one of the cards counts the play down for the rest of its group (`willbreaker_remaining`), and once that play has resolved (the spell cast, or the minion played with its Battlecry and the observers of its play), the rest of a used-up group is discarded. So the discarded cards still take their hand slots while the spell is cast, and the played minion sees them discarded (a `Mindbender Ghur'sha` played from a `Faceless Operative` pair triggers).

---

## 7. General Tavern Trigger Hooks & Subsystems

All card-specific Tavern abilities plug into one of the following hooks (with magnitudes or repetitions doubled when `is_golden == true`):

### 7.1 On-Play (Battlecries & Board Observers)
When a minion is played from `hand` to `board[board_pos]`:
1. **Pre-insertion Battlecries & Global Aura Application**: Grants economy bonuses (`bonus_gold_next_turn`), generates hand spells/tokens (if `hand.len() < 10`), updates persistent `PlayerAuras`, and applies existing tribal/summon auras to the newly played minion.
   - A Battlecry triggers `Passive::BattlecryTriggers` times (`Brann Bronzebeard`), each trigger followed by the `after_friendly_battlecry` observers; so do Battlecries triggered by other cards (`Young Murk-Eye`, `Kelp Keeper`).
   - A Battlecry that targets a friendly minion (`battlecry_target`: `Disguised Graverobber`, `Maw Caster`, `Captain Sanders`, `Elite Navigator`, `Lovesick Balladist`, `Mind Muck`, and a `Magicfin Apprentice` taught a spell cast on a minion, among the minions that spell can target) picks its target once, from `board_pos`: every trigger has that same target, and the triggers after it has left the board (destroyed: its Reborn copy is a new minion; transformed) do nothing.
   - The played minion holds its place between its neighbours while its Battlecry resolves (`play_slot`): a minion the Battlecry destroys leaves its place to its Deathrattle and Reborn summons, and the played minion stays on its side (a `Maw Caster` played to the right of the `Harmless Bonehead` it destroys ends right of the two Skeletons, one played to its left ends left of them). Those summons leave room for it: with 6 minions, only one Skeleton is summoned.
   - Once its Battlecry has resolved, the played minion's auras are updated before it joins the board (a `Plaguerunner` its Battlecry destroyed gives it the Undead Attack).
2. **Post-insertion Board Triggers**:
   - Board-wide or tribal buffs from the played minion apply to other matching friendly minions.
   - Friendly board observers ("whenever/after you play a minion of tribe X") fire in left-to-right board order.
   - Then a Spellcraft minion gives its Spellcraft spell (§7.4).
3. **Magnetize**: playing a Magnetic minion onto a friendly minion of its type (`board_pos` is that minion) is a play that Magnetizes it there instead of inserting it. Each Magnetization (the target gains the card's stats, keywords and carried state) is seen by the "after you play" observers (`Played::magnetized`), then by the `after_friendly_magnetize` observers (`Polarizing Beatboxer`). A Magnetization by a card effect (a `Spark Snapper` or `Glambot` Satellite) is seen by both but is not a play (`Played::from_hand`), and neither are the extra Magnetizations of `Drone Duplicator` or the copies `Polarizing Beatboxer` Magnetizes to itself (which the `after_friendly_magnetize` observers don't see).

### 7.2 Hero Self-Damage Interception
When a Tavern trigger would deal damage to the friendly hero:
- If the board contains any friendly damage-prevention observer, the hero takes `0` damage and each such observer applies its on-prevent buff instead.
- Otherwise, `state.health` is reduced by the damage amount. Triggered self-damage can kill: at `1` Health, `Wrath Weaver`'s 1 damage takes the hero to `0` (the hero dies). Only Health costs (Health Refreshes, Health-cost spells) must leave the hero above `0` to be payable.
- Each damage instance is resolved on its own: an effect that deals `1` damage twice (Golden `Wrath Weaver`) triggers the hero-damage observers (`Ashen Corruptor`, `Tichondrius`, ...) twice.
- Combat damage (the hero damage of a lost combat, taken by Armor first, then Health) triggers no hero-damage observers: `Soul Rewinder` and `Ashen Corruptor` don't rewind it, `Tichondrius` doesn't trigger, and `Eredar Escapist` ("after your hero takes 4 damage") doesn't count it.

### 7.3 On-Sell Triggers
When a minion at `board[board_pos]` is sold:
- May draw or generate minion(s)/token(s) into `hand` (if `hand.len() < 10`, checking for Triples after each addition).
- May increment persistent scaling counters and buff remaining minions on `board`.

### 7.4 Spellcraft & Hand Spell System
- **Spellcraft** (`spellcraft` hook; no Solo minion of Patch 36.6.3 has it, the Duos `Private Chef` and `Well Wisher` do: `docs/duos_tavern.md` §8.3): a Spellcraft minion gives its player its Spellcraft spell when it is played (once its play has resolved, §7.1) and at the start of each turn (§4), if there is room in `hand`; with a full hand it gives none (nothing queues). The spell is temporary (`CardFlags::TEMPORARY`): still in `hand` when the player ends their turn, it is discarded without discard effects (one that reached the hand after that is discarded at the start of the next turn). A Golden minion's spell is Golden. Spellcraft spells are token spells, not Tavern spells (`NOT_TAVERN_SPELL`).
- **Stat-Threshold Check**: Any stat increase in the Tavern Phase immediately runs the target's stat-threshold observers.
- **Casting** (`cast_spell`): A spell one of your minions casts (`Air Revenant`'s `Easterly Winds`, `Gatekeeper Amalgam`'s `Misplaced Tea Set`, `Victorious Geomant`'s Blood Gems, ...) counts as you casting it, exactly like one you play; it just isn't played from your hand (`cast_spell_by_effect`; `Hot-Air Surveyor` only repeats Blood Gems played from your hand). Each cast of a Tavern spell increments `tavern_spells_cast` (`Faceless Converter`), becomes the last Tavern spell cast (`Cataclysmic Harbinger`) and runs the `spell_cast` observers (`Charging Czarina`, `Sha of Fear`, ...). Blood Gems aren't Tavern spells, nor are `Lockbox` and the Choose One options (`NOT_TAVERN_SPELL`, `is_tavern_spell`), so they do neither. Then every cast runs the spell's effect, the `after_targeted_spell` observers (a spell cast on a minion that is still there after the cast, on your board or in the Tavern: `Glambot`, `Shamanic Tidecaller`, `Twilight Tidehunter`, ...), then the `after_spell_cast` observers (any spell: `Felboar`). A Blood Gem that a card *plays* on a minion (`Geomagus Roogug`, `Prodigious Tusker`, ...) isn't a spell you cast (**Played Blood Gems** below).
- **Targets**: A targeted spell is cast on a friendly minion: any one (`SpellTarget::Friendly`), one of a type (`FriendlyTribe`: `Butchering` needs an Undead), or one with a minion type if the spell uses its target's type (`FriendlyTyped`: `Chef's Choice`, `Eonar's Favor`, `Lost Staff of Hamuul`, `Natural Blessing`; a two-type minion has both types, an All minion every type: §7.8). A spell whose text says "a minion" rather than "a friendly minion" (`SpellTarget::Minion`: `Tavern Dish Banana`, `Fortify`, `Alliance Flag`, `Robust Evolution`, ...) can also be cast on a minion in the Tavern (`PlayOnShop`; not `Channel the Devourer`, which sells its target), and one about "a minion in the Tavern" only there (`SpellTarget::TavernMinion`: the Duos `Portal in a Fountain`). A spell's `spell_target_filter` can narrow its targets further (the Duos `Portal in a Crystal`: a non-Golden minion). The `after_targeted_spell` observers see those casts too: `Glambot` Magnetizes its Satellite to a Mech in the Tavern, and `Shamanic Tidecaller` triggers on a Murloc in the Tavern (buffing the Murlocs in your hand and board).
- **Repeated Casts**: A spell cast on a friendly minion is cast `TargetedSpellCasts` times (`Balinda Stonehearth`, whoever casts it: a spell your minion casts too, such as `Victorious Geomant`'s Blood Gems or a `Magicfin Apprentice`'s spell; a spell cast on a minion in the Tavern is cast once), a Bounty spell `BountyCasts` times, and a Blood Gem played from your hand `ExtraHandBloodGemCasts` more times (`Hot-Air Surveyor`). Extra casts add up: a Blood Gem played from your hand on a friendly minion with Balinda and a Surveyor is cast 1 + 1 + 1 = 3 times (4 if either is Golden); a Bounty spell's casts multiply the rest. Other spells are cast once. Every cast is a cast of its own: it counts as a spell cast (`Felboar` counts each), and the `after_targeted_spell` observers and `Geomagus Roogug` trigger on each. The casts follow their target (by identity, wherever it moves); once it is gone (destroyed by `Butchering`, sold by `Channel the Devourer`, transformed by `Robust Evolution`, ...), the remaining casts fizzle: they do nothing and trigger nothing. A Choose One spell (`Alliance Flag`) is chosen once and the chosen option applies once per cast (`pending_choice_repeats`), triggering the Choose One observers (`Turbo Hogrider`) once; if the first cast had both effects combined (`Thorned Trailblazer`, using one charge), so do the others.
- **Played Blood Gems**: A card either *plays* Blood Gems on a minion or *casts* a spell, and only a cast is a spell you cast. A Blood Gem that a card plays (`play_blood_gems`: `Bonker`, `Tusked Camper`, `Razorfen Vineweaver`, `Prodigious Tusker`, `Turbo Hogrider`, `Geomagus Roogug`, `Veteran Brigand`'s Brigand's Share, `Gem Confiscation`, `Blood Gem Barrage`, `Winner's Bread`), in combat or in the Tavern (a Rally triggered by `Sky-hatch Runaway`, a `Fishbait` attack, ...), isn't a spell you cast: `Felboar` doesn't count it, the "whenever you cast a spell on" minions (`Glambot`, `Shamanic Tidecaller`, `Twilight Tidehunter`, `Gatekeeper Amalgam`, `Devilish Distractor`) don't react to it, and `Balinda Stonehearth` and `Hot-Air Surveyor` don't repeat it. A spell that plays Blood Gems is just its own cast: `Felboar` counts `Gem Confiscation` once, and `Glambot` sees one spell cast on its target. A played Blood Gem is still a Blood Gem played on the minion (its `blood_gems_played`), and `Geomagus Roogug` ("whenever a Blood Gem is played on this") triggers on every Blood Gem played or cast on it. A spell that a card *casts* is a spell you cast (**Casting** above), and so is a Blood Gem played from your hand: `Gatekeeper Amalgam`'s `Misplaced Tea Set`, `Air Revenant`'s `Easterly Winds`, the Blood Gems `Victorious Geomant` casts because they don't fit in your hand, the `Blood Gem Barrage`s of `Veteran Brigand`'s Brigand's Barrage, a `Magicfin Apprentice`'s spell, a discarded `Sludge Corrosion` or `Energizing Chamber` (cast twice: 2 spells cast).

### 7.5 End-of-Turn Triggers (`EndTurn`)
When `EndTurn` is executed:
1. Board end-of-turn triggers fire in left-to-right board order (self-buffs, adjacent buffs).
2. Hand end-of-turn triggers fire in left-to-right hand order (e.g., summoning copies from hand onto `board` if `board.len() < 7`).

### 7.6 Persistent Game-Long Scaling (`PlayerAuras`)
`TavernState` tracks game-long counters that persist across turns and apply to both Tavern actions and Combat simulations:
- Tribal and token stat bonuses (applied to existing and newly played/summoned matching units).
- Summon/death counters for scaling minions.
- Tavern spells cast (`tavern_spells_cast`, §7.4, including those your minions cast in combat: `docs/combat.md` §2) and sell-scaling stacks.
- What combat changes comes back with its result (`docs/combat.md` §1, Combat-to-Tavern Persistence): a Tavern aura gained in combat (`Living Azerite`'s Elemental bonus) also buffs the minions already in the Tavern.

### 7.7 Rallies and Attacks in the Tavern
A friendly minion's Rally can trigger in the Tavern (`Sky-hatch Runaway`), and a friendly minion can attack a minion in the Tavern: a `Fishbait` (`Lurking Lionfish`, `Snarky Shark`: your left-most Beast attacks it). While one resolves, the minions on your board have distinct ids (given back afterwards), so the hooks that tell minions apart work as in combat (`Wolf Pup`'s "your other minions", `Prodigious Tusker`'s "another friendly minion"). The Blood Gems your minions play during one (a Rally of `Bonker`, `Prodigious Tusker`) aren't spells you cast (§7.4, **Played Blood Gems**).
- **Rally** (`trigger_tavern_rally`): the minion's own Rally (stat buffs, summons to its right, cards to your hand), then, if it is a Rally minion, the `after_friendly_rally` observers (`Deathstrider` triggers your left-most Deathrattle, in the Tavern). A Rally that casts a spell (`Crimson Vindicator`'s `Mighty Dragonbreath`) casts it once the Rally resolves, like any spell your minion casts in the Tavern (§7.4: `Felboar` counts it, it becomes the last Tavern spell you cast, ...). A Rally that needs the attack target (`Heroic Underdog`, `Obsidian Ravager`) does nothing without an attack.
- **Attack** (`tavern_attack`): a regular attack (`docs/combat.md` §5.3.1): the attacker's Rally and the `after_friendly_rally` observers, then the `friendly_attack` observers (`Cage Gnawer`, `Ravaging Scorpid`, `Prodigious Tusker`, `Roaring Recruiter` for a Dragon), then the attacker deals its Attack (after those triggers) to the target. Unless a Divine Shield absorbs it, the damage-dealt hooks count the whole Attack, overkill included (`Treasure Parrot`, `Devout Hellcaller` for a Demon). The attacker is followed by identity: summons to its left (a `Deathstrider` Deathrattle) don't change which minion attacks. A Fishbait has 0 Attack, so the attacker takes no damage; one that dies gives its killer +5/+5 (+10/+10 if Golden) and leaves the Tavern, one that survives stays there damaged. Combat-only rules don't apply: one strike (no Windfury second strike), no Venomous, Cleave, excess damage or after-attack damage.

### 7.8 Minion Types
A minion has no type, one type, two types (`Treasure Parrot`: Beast/Pirate), or every type (`All`: `Dark Paradox`). The types are the 10 lobby types of Patch 36.6.3 (Aberration, Beast, Demon, Dragon, Elemental, Mech, Murloc, Pirate, Quilboar, Undead: `LOBBY_TRIBES`) and Naga (`SINGLE_TRIBES` holds all 11). No lobby has Naga, so the card pool has no minion that is only a Naga, but a minion can have Naga as its second type: `Firescale Hoarder` (Dragon/Naga) and `Ominous Seer` (Demon/Naga). Such a minion is in the pool through its lobby type, and a draw of a type (`Chef's Choice`, `Lost Staff of Hamuul`, a Discover of Dragons, ...) offers it for either of its types. Every effect that looks at minion types sees Naga, and an All minion is a Naga too:
- **"A friendly minion of each type"** (`Misplaced Tea Set`, `The Last One Standing`) and **"per minion type"** (`Menagerie Tableware`): Naga is one more type that a minion can stand for (the best assignment of minions to types): a `Firescale Hoarder` next to another Dragon stands for the Naga.
- **"Your most common type"** (`Friendly Bounty`, `Planar Telescope`, `Dark Paradox`): the type that the most friendly minions have, Naga included (an All minion counts for every type), with ties broken uniformly at random among the tied types: with a `Firescale Hoarder` and an `Ominous Seer`, Naga (2) over Dragon and Demon (1 each). If no friendly minion has a type, it is a random lobby type (never Naga).
- **"A friendly minion's type"** (`Chef's Choice`, `Eonar's Favor`, `Lost Staff of Hamuul`, `Natural Blessing`; §7.4 **Targets**): all of the target's types, Naga included: `Chef's Choice` on an `Ominous Seer` gets a Demon or a Naga (`Firescale Hoarder` included), and `Natural Blessing` on a `Firescale Hoarder` buffs the Dragons and the Nagas (`Ominous Seer` included).
