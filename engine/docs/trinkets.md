# Trinkets Specification

This document specifies Battlegrounds **trinkets** in Seaglass: the trinkets a player is offered on turns 6 and 9, how they pick and pay for one, what a trinket's state is, how trinkets reach the Tavern and combat, the RL action and observation, and the Power.log replay. It builds on [`tavern.md`](tavern.md), [`combat.md`](combat.md), [`heroes.md`](heroes.md), [`duos_tavern.md`](duos_tavern.md) and [`duos.md`](duos.md), and like them it is updated **before** behavior changes. The engines stay card-agnostic: what a trinket does lives in its own file (`src/trinkets/<slug>.rs`) and reaches the engine only through hooks, as Hero Powers do.

Rules marked **(confirmed)** were settled with the user. Rules marked **(engine decision)** are details nobody has settled yet: the engine implements them as written, each in one place, so that it is a small change if the answer differs (§12).

**A player without trinkets** (the default: `TavernState::trinkets_enabled` is `false`, `PlayerAuras::trinkets` is empty) plays exactly as before this document: no offer, no new legal action, no RNG draw. Solo and Duos results are byte-identical for them.

---

## 1. Scope

### Included
- The trinket pool (`src/trinkets/table.rs`), generated from `CardDefs.xml` and HearthstoneJSON, or from an HSReplay override file (§9).
- Trinket offers at the start of turns 6 and 9, the `ChooseTrinket` action, the price, owning a trinket for the rest of the game (§3–§5).
- The trinket's state, which travels into combat with the player (§2.2), the trinket hooks (§6), the Start of Combat step for trinkets (§7), Duos (§8.1), the RL encoding (§8.2), the scenario DSL (§8.3), and the Power.log replay (§10).
- The trinkets implemented so far: §11.

### Deferred
Trinkets that need systems the engine doesn't have are in the table but not implemented, and not offered (§3.1) **(confirmed)**: Dark Gifts, Timewarp, Darkmoon Prizes, Bounties, Tier 7 minions, Quests and Buddies. §11.3 lists them. Naga trinkets are never offered (Naga aren't in the game **(confirmed)**). Spellcraft trinkets are implemented when the Spellcraft queue (`tavern.md` §7.4) supports them.

---

## 2. The Trinket

### 2.1 Trinket Data

Every trinket of the pool is a `TrinketData` row of the generated table (`trinkets::table()`):

| Field | Meaning |
| :--- | :--- |
| `id` | Its dbfId (`111388` for the Lesser `Eternal Portrait`): the trinket's engine `CardId`, stable across patches. Engine card ids are below 11000, so they never collide. |
| `card_id`, `name` | Its card ID (`BG30_MagicItem_301`) and name. Two trinkets can share a name (the Lesser and Greater `Eternal Portrait`); their card IDs differ. |
| `greater` | A Greater trinket (`spellSchool` `GREATER_TRINKET`, CardDefs tag 1635 = 12), else Lesser (`LESSER_TRINKET`, 11). |
| `cost` | Its price in Gold (`COST`). |
| `tribes` | The minion types it is associated with (`battlegroundsAssociatedRaces`): none, one, or two (`Bronzebeard Portrait`: Dragon and Murloc). |
| `duos_only` | Only in Duos (`isBattlegroundsDuosExclusive`). |
| `text` | Its text, rendered as HearthstoneJSON does (§9). |

The catalog API (`src/trinkets/mod.rs`): `trinkets::table()`, `trinkets::data(id)`, `trinkets::by_name(name)` (the first of the name, Lesser before Greater; `by_name_greater` picks one), `trinkets::by_card_id(card_id)`, `trinkets::is_implemented(id)`, `trinkets::offerable(duos)`.

### 2.2 Owned Trinkets (`TrinketState`)

A player's trinkets are `PlayerAuras::trinkets: Vec<TrinketState>`, in the order they were picked (the Lesser one first) **(engine decision: where they live)**. They live in the player's auras so that they travel into combat and back with them, as the Hero Power does (`heroes.md` §2.2, `combat.md` §1): a Solo side's `GameState` auras and a Duos `Warband`'s auras carry them, and what combat changes in them comes back.

| Field | Meaning |
| :--- | :--- |
| `card_id` | The trinket (its hooks). |
| `counter` | Progress toward a threshold ("(6 left!)", minions summoned this combat). |
| `stacks` | Improvements so far. |
| `armed` | A per-turn or per-combat charge ("3 times per combat"). |
| `stored` | Cards the trinket keeps (minions to resummon). |

All are generic: the engine never reads `counter`, `stacks`, `armed` or `stored`; the trinket's own hooks do. A player never owns two copies of a trinket (§3.2), so a trinket's hooks find its state by its card id (`trinkets::state_mut(auras, ID)`).

### 2.3 Owning a Trinket

A trinket is kept for the rest of the game **(confirmed)**. Trinkets that replace or copy themselves (`Mystery Cube`, `Trip Vouchers`, `Souvenir Stand`, `Empowered Mirror`, ...) are deferred (§11.3).

---

## 3. Offers

### 3.1 When and What

- A **Lesser** offer at the start of turn 6 and a **Greater** offer at the start of turn 9, 4 options each **(confirmed)**: the player's `turn` (`tavern.md` §3.1), at the end of the start of turn (`tavern.md` §4 step 6), after the Spellcraft spells and the Triples, so the board and the hand are as the turn starts.
- Only for a player with offers enabled (`TavernState::trinkets_enabled`; `DuosTavern::with_trinkets`, Python `trinkets=True`). The default is off.
- The options come from the **offerable** trinkets of that kind (`trinkets::offerable(duos)`): implemented, not deferred, not Naga, Duos-only ones only in Duos, and not one the player owns **(confirmed)**. Only implemented trinkets are offered, as for heroes.

### 3.2 The Rules an Offer Follows

Every rule below is **(confirmed)** but where it says otherwise.

1. 4 options, none the player owns.
2. At least one costs 2 Gold or less.
3. A typed trinket needs minions of its type in the player's board and hand: at least 2 for a Lesser offer, at least 3 for a Greater offer. A minion counts for each of its types: a two-type minion for both, an `All` minion for every type (`Tribe::has_single`, `tavern.md` §7.8) **(engine decision)**. A spell counts for nothing. A trinket of two types needs it for one of them **(engine decision)**.
4. At least one option has no type.
5. At most one option has a type that isn't the player's most common type, and that option costs 2 less (never below 0). The most common types are those with the most minions (as counted in rule 3); with a tie, every tied type is a most common type **(engine decision)**. A trinket of two types is of a most common type if either is **(engine decision)**.

### 3.3 The Draw (`trinkets::draw_offer`) **(engine decision)**

1. **Counts.** For each lobby type (`LOBBY_TRIBES`), `count[t]` is the number of minions on the board and in the hand that have it (rule 3). The threshold is `T = 2` (Lesser) or `3` (Greater). A type is *eligible* if `count[t] >= T`; the *most common* types are the eligible ones with the highest count.
2. **Candidates**, in table order (by name): every offerable trinket of the kind the player doesn't own, classified as:
   - **Neutral**: no type; its price is its cost.
   - **Main**: a type that is a most common type; its price is its cost.
   - **Off**: no most common type, but an eligible one; its price is its cost minus 2 (never below 0).
   - A typed trinket with no eligible type is not a candidate.
3. **Draws**, each one `rng.below(k)` over the list as it is then (in table order, without the options already drawn); a step whose list is empty draws nothing:
   1. one **Neutral** option (rule 4);
   2. if no option drawn so far costs 2 or less: one option among the candidates whose price is 2 or less (rule 2);
   3. then, until there are 4 options: one among the candidates left (Neutral, Main, and Off as long as no Off option was drawn: rule 5).
4. **Shuffle**: the drawn options are shuffled (Fisher–Yates: for `i` from the last index down to 1, `j = rng.below(i + 1)` and swap), so their place says nothing about why they were drawn.

Every trinket is equally likely among those a step draws from (uniform weights) **(engine decision)**. With a pool too small (early in the engine's life, few implemented trinkets of a kind), an offer can have fewer than 4 options or break rule 2 or 4; an empty offer is no offer.

`TavernState::trinket_options: Vec<TrinketOption>` holds the offer: each option's `card_id` and `cost` (its price, rule 5 applied).

---

## 4. Picking a Trinket: `ChooseTrinket`

`TavernAction::ChooseTrinket { option_index }` is the last variant of `TavernAction` (after `HeroPower2`), so the earlier variants keep their places **(engine decision: its own action rather than `ChooseDiscover`, whose options are cards that go to the hand, and whose RL slots would mix cards and trinkets)**.

1. While `trinket_options` is non-empty, **only** `ChooseTrinket` is legal **(confirmed)**: not `EndTurn`, not a Pass, not `ChooseDiscover`. A hero choice (`hero_options`) still comes first; a trinket offer comes before a pending Discover **(engine decision)**: the turn start's Discovers (Master Nguyen's Hero Powers) wait behind it.
2. The player must take one: there is no skipping **(confirmed)**.
3. `ChooseTrinket { option_index }` is legal if `option_index < trinket_options.len()` and the player has the option's price in Gold. If no option is affordable, the options with the lowest price are legal and free **(engine decision)**.
4. **Effect**, in order: the offer is cleared; the price is paid (`TavernState::spend_gold`: Gold spent, which the Gold-spent observers see, **engine decision**); the trinket joins `auras.trinkets` (a fresh `TrinketState`); its `trinket_bought` hook resolves (its "Get a ..." or "Gain ... Gold" text); the auras resync. So a trinket that reacts to Gold spent doesn't see its own price **(engine decision)**. Then the action ends like any other (`tavern.md` §6: the waiting cards, Triples, aura sync).

`TavernState::give_trinket(id, pool, rng)` gives a trinket without an offer or a price (scenarios, the replay, tests): it joins and its `trinket_bought` hook resolves.

---

## 5. Cards a Trinket Gives: Hand Protection

A card a trinket gives waits for room in a full hand instead of being lost **(confirmed)**: it joins the player's `waiting_for_hand` queue (`TavernState::add_to_hand_or_wait`, `tavern.md` §7.4), behind the cards already waiting, and reaches the hand in order as soon as it has room, once the action that made room has resolved **(engine decision: one queue, in the order the cards were given, Spellcraft spells and trinket cards alike)**. Unlike a Spellcraft spell, a trinket's card is not temporary: still waiting when the player ends their turn, it keeps waiting into the next turns **(engine decision)**; only temporary cards (`CardFlags::TEMPORARY`) are discarded from the queue at the end of the turn and at the start of the next. In Duos, the player's own waiting cards reach the hand before the cards their teammate Passed (`duos_tavern.md` §5.4).

A card a trinket gives in combat goes to the combat hand like any card a combat effect gives (`combat.md` §2).

---

## 6. Hooks

Trinkets reuse the card hook system (`src/cards/hooks.rs`): a trinket is a card whose `CardHooks` table the registry holds under its dbfId. Its hooks are invoked for the player who owns it (`PlayerAuras::trinkets`), in the order the trinkets were picked, never for minions. The **Trinkets** section of `CardHooks`:

| Hook | Fires | For |
| :--- | :--- | :--- |
| `trinket_bought` | The player gets the trinket (§4): `(state, pool, rng)`. | "Get a ...", "Gain 4 Gold". |
| `trinket_start_of_combat` | Start of Combat (§7): `(ctx)`, a `HeroCombatCtx` (the player's side as a `BoardCtx`, the enemy board, damage to enemy minions), as the Hero Power's. | `Holy Mallet`, `Training Certificate`, ... |
| `trinket_turn_start` | Start of turn, after the Hero Power's `hero_turn_start` (before the Spellcraft spells): `(state, pool, rng)`. | "At the start of each turn, get another." |
| `trinket_deity_awakened` | The player's Deity awakened in combat, after its own `awaken` hook: `(ctx, deity_idx)`. | `Evil Experiment`. |
| `trinket_friendly_death` | A friendly minion died in combat, once its side's round of deaths resolved, after the Hero Power's `hero_friendly_death` (summons go to the right end): `(ctx, dying)`. | Avenge, "after N friendly minions die". |
| `trinket_friendly_attack` | A friendly minion attacks (in combat or in the Tavern), after the Hero Power's `hero_friendly_attack`, before it strikes: `(ctx, attacker_id)`. | `Faerie Dragon Scale`, `Lionfish Portrait`. |
| `trinket_after_buy` | After a card is bought, after the Hero Power's `hero_after_buy`: `(state, bought, pool, rng)`. | `Transcribing Typewriter`, `Trickster's Sleeve`. |
| `trinket_after_upgrade` | After the Tavern is upgraded, after the Hero Power's `hero_after_upgrade`: `(state, pool, rng)`. | `Stuffed Coin Purse`. |
| `trinket_after_play` | After a minion is played or Magnetized, after the Hero Power's `hero_after_play`: `(state, played, pool, rng)`. | `Nomi Sticker`, `Water Wheel`. |
| `trinket_after_sell` | After a minion is sold, after the Hero Power's `hero_after_sell`: `(state, sold, pool, rng)`. | `Lava Lamp`. |
| `trinket_end_turn` | The player ends their turn, after the Hero Power's `hero_end_turn`: `(state, pool, rng)`. | `Rendle Sticker`, `Sphere of Memory`. |
| `trinket_spell_cast` | Each cast of a Tavern spell in the Tavern, after the cards' `spell_cast` observers: `(state, pool, rng)`. | `Corrupted Baton`. |
| `trinket_after_refresh` | The Tavern was Refreshed, by any Refresh (the turn start's new shop too: tavern.md §4.1), after the Hero Power's `hero_after_refresh`, before the cards' after-Refresh effects: `(state, pool, rng)`. | `Innkeeper's Stein`. |
| `trinket_after_player_refresh` | The player Refreshed (the `Refresh` action, or a Refresh one of their cards or Hero Powers does; not the turn start's new shop: tavern.md §4.1), after every trinket's `trinket_after_refresh`: `(state, pool, rng)`. | `Frigid Blossom`, `Upstart Embers`, `Dalaran Cheese Wheel`. |
| `trinket_gold_spent` | Gold was spent (`TavernState::spend_gold`: a buy, a Refresh, an Upgrade, ...), after the Hero Power's `hero_gold_spent`: `(state, amount, pool, rng)`. | `Booty Bay Brew`. |
| `trinket_tavern_summon` | A friendly minion is summoned in the Tavern (played, or summoned by a card), after the minions' `friendly_summon` observers: `(trinket, summoned)`. | `Mama Bear Sticker`. |
| `trinket_extra_triggers` | Extra times the effects a `Passive` counts trigger: `(passive)`. Only `EndOfTurnTriggers` is read: on top of the board's for the minions' end-of-turn triggers, and as extra rounds of the Hero Power's and the trinkets' end-of-turn hooks. | `Ghastly Sticker`. |
| `trinket_combat_start` | The player's side of a combat is set up: `(trinket)`, to restart per-combat counts. | Avenge, "3 times per combat". |
| `trinket_combat_summon` | A friendly minion is being summoned in combat, after the other summon modifiers (the auras, the Hero Power's `hero_combat_summon`, the `friendly_summon` observers): `(trinket, summoned)`. | `Slamma Sticker`. |

The dispatch functions (`trinkets::on_start_of_combat`, `trinkets::on_turn_start`, ...) run each owned trinket's hook in turn; the engines call them next to the matching Hero Power dispatch. Extra Deathrattles a trinket gives a minion in combat (`Jarred Frostling`) are cards too, as the Hero Powers' (`heroes.md` §4): `Unit::extra_deathrattles` lists them. More hooks are added with the trinkets that need them (§11).

---

## 7. Combat

### 7.1 Trinkets in Combat

The player's trinkets travel in their `PlayerAuras`: the Solo `GameState`'s `auras_a` / `auras_b`, a Duos `Warband`'s auras. Their combat hooks act for that side, and what they change in their state comes back in the result's auras.

### 7.2 Where Trinkets Fall in Start of Combat

`combat.md` §5.1.1 gains a step **1c**, after the Hero Powers (step 1b) and before the player effects (step 2) **(engine decision)**:

1. **Order** (unchanged). A side with a trinket that has a `trinket_start_of_combat` hook has a Start of Combat effect for the order coin flip.
1b. **Hero Powers** (unchanged), for both sides.
1c. **Trinkets**, for the first side, then the other: each of the side's trinkets with a `trinket_start_of_combat` hook, in the order they were picked (the Lesser one first). After each, as for a Hero Power: the damage it deals to enemy minions, the minions it destroys, the deaths (step 4) and its immediate attacks.
2. **Player effects**, 3. **Minion triggers** (unchanged).

In Duos, a warband stepping in resolves step 1c for its own trinkets after its step 1b (`duos.md` §3.2).

---

## 8. Duos, Python / RL, Scenarios

### 8.1 Duos

Trinkets are per player **(engine decision)**: each player of a team is offered their own trinkets on their turn 6 and 9 (`DuosTavern::with_trinkets` enables both), counts their own board and hand, pays with their own Gold, and owns their own; Duos-only trinkets are offered only in Duos. `DuosTavern::is_legal` / `step` allow `ChooseTrinket` for the player's own Tavern like every action but Pass; a pending offer blocks the player's Passes too. Each warband carries its player's trinkets into a Duos combat.

### 8.2 Python / RL

- `TavernGame(..., trinkets=False)` and `DuosTavernGame(..., trinkets=False)` enable the offers. The default (`False`) is no offer, so existing code is unchanged.
- `TavernAction.choose_trinket(option_index)`, whose `kind` is `"choose_trinket"`.
- The observation (`TavernObservation`, `DuosObservation`) appends `trinkets` (the owned trinkets' ids, in the order picked), `trinket_counters` (their `counter`s), `trinket_options` (the offered trinkets' ids) and `trinket_option_costs` (their prices).
- The RL action table (`rl/seaglass_rl/actions.py`) appends, at the end, `choose_trinket(i)` for `i` in `0..4`.
- The features (`rl/seaglass_rl/features.py`) append to the player vector a pending-offer flag, the number of trinkets owned (/ 2) and the 4 offered prices (/ 10, 0 for none), and 6 card slots at the end: the 2 owned trinkets, then the 4 offered ones (their ids are appended to `seaglass.all_card_ids()`).

`rl/README.md` describes the encoding.

### 8.3 Scenarios

- A trinket's scenario file is `tests/scenarios/trinkets/<slug>.yaml`, starting with `trinket: <Trinket Name>` (Lesser or Greater: `trinket: Eternal Portrait (Greater)` picks the Greater one).
- `auras: {trinkets: [<name>, {trinket: <name>, counter: 3}]}` gives a player trinkets (fresh `TrinketState`s, without their `trinket_bought` text) in a Tavern or combat setup; `trinkets_enabled: true` on a Tavern enables the offers.
- Steps: `give_trinket: <name>` (§4: with its `trinket_bought` text, no price), `offer_trinkets: [<name>, ...]` (an offer of those, at their table cost) or `offer_trinkets: lesser | greater` (an offer drawn by §3.3), and `choose_trinket: <index or name>`.

---

## 9. The Trinket Table and Its Generator

`src/trinkets/table.rs` is generated by `tools/gen_trinkets.py` from HearthstoneJSON's `CardDefs.xml` and `cards.json`; rerun it each patch:

```sh
python3 tools/gen_trinkets.py path/to/CardDefs.xml path/to/cards.json [--hsreplay tools/data/trinkets_hsreplay.tsv]
```

- **The pool**: every `BATTLEGROUND_TRINKET` with CardDefs tag `3407 = 1` (264 trinkets in build 255026: 127 Lesser, 137 Greater, 18 of them Naga, 9 Duos-only), sorted by name, then Lesser before Greater, then card ID.
- **The HSReplay override** (`tools/data/trinkets_hsreplay.tsv`, used if present or given with `--hsreplay`): HSReplay's trinket pages are the source of truth for the current pool and prices. Each line is `<name or card ID>\t<Lesser|Greater>\t<cost>` (`#` starts a comment). When the file is present, it defines the pool (the trinkets it lists, matched by card ID, or by name and kind) and their costs; a line that matches no trinket is an error.
- The text is rendered as for Hero Powers (`heroes.md` §9).

Each implemented trinket has its own file, `src/trinkets/<slug>.rs` (the Greater one of a shared name: `<slug>_greater.rs`), whose first lines are its header:

```text
//! `Holy Mallet` (`BG30_MagicItem_902`) — Lesser, 0 Gold, Dragon.
//!
//! Start of Combat: Give your left and right-most minions Divine Shield.
```

The `trinkets` tests check that every trinket file's header matches the table (name, card ID, kind, cost, types and text), that its `ID` constant is the table's dbfId, that it is in `IMPLEMENTED`, and that it has a scenario file `tests/scenarios/trinkets/<slug>.yaml` starting with `trinket: <name>`.

---

## 10. Replay

The Power.log validator (`replay/src/{reconstruct,validate}.rs`) already reads each player's trinkets at each combat: the `BATTLEGROUND_TRINKET` entities they control in `PLAY` (not the offered ones, `BACON_IS_POTENTIAL_TRINKET`). It maps each one's card ID to its `TrinketData` (`trinkets::by_card_id`) into the side's `PlayerAuras::trinkets`, so `replay_cli validate` simulates the implemented ones; an unimplemented trinket that triggers in the combat stays a gap. In a log, the offers are `ChoiceType=GENERAL` choices whose source is `BG30_Trinket_1st` (Lesser) or `BG30_Trinket_2nd` (Greater), with 4 entities; the pick pays its `COST` (`RESOURCES_USED`), and the hero gets `BACON_FIRST_TRINKET_DATABASE_ID` / `BACON_SECOND_TRINKET_DATABASE_ID`. The log's `TURN` is `2 × round − 1`: round 6 is `TURN 11`, round 9 `TURN 17`.

---

## 11. The Trinkets

"Your minions" are those on the board. Golden doesn't apply to trinkets.

### 11.1 Combat Trinkets

"Get a ..." gives a new plain copy of the minion from the catalog, not from the pool **(confirmed)**, under hand protection (§5). A random card ("get a random Beast") is a new copy of the player's Tier or lower, not taken from the pool **(confirmed)**. "Start of Combat" is step 1c (§7.2).

| Trinket | Text | How the engine reads it |
| :--- | :--- | :--- |
| `Holy Mallet` (Lesser) | Start of Combat: Give your left and right-most minions Divine Shield. | The left-most minion, then the right-most; a single minion once. |
| `Baleful Incense` (Lesser) | Start of Combat: Give your left and right-most Undead Reborn. | The left-most and right-most Undead (a two-type or `All` minion counts); a single one once. |
| `Training Certificate` (Lesser) | Start of Combat: Double the stats of your two lowest-Attack minions. | The two with the lowest Attack gain their Attack and Health; ties are broken at random (one `rng.below(k)` among the tied, for each pick) **(confirmed)**. |
| `Eternal Portrait` (Lesser) | Get an Eternal Knight. Start of Combat: Give your Eternal Knights Taunt and Reborn. | Every friendly `Eternal Knight`, Golden too. |
| `Hoggy Bank` (Lesser) | Start of Combat: Give your Quilboar "Deathrattle: Get 2 Blood Gems." | Every friendly Quilboar gains the extra Deathrattle *Gem in the Bank* (`BG30_MagicItem_411e`) for the combat; it gives 2 `Blood Gem`s to the combat hand, which come back with it. |
| `Summoning Sphere` (Lesser, Duos) | Start of Combat: Summon a copy of your teammate's highest-Health minion. | From the teammate's warband as it entered the combat (`duos.md` §2), a tie at random (one draw), at the right end if there is room, as Vanndar's copy (`heroes.md` §11.1). Nothing without a teammate minion. |
| `Converter Portrait` (Greater) | Get a Faceless Converter. Start of Combat: Give your Faceless Converters Reborn. | Every friendly `Faceless Converter`, Golden too. |
| `Rivendare Portrait` (Greater) | Get a Titus Rivendare. Start of Combat: Double the Health of your Titus Rivendares. | Every friendly `Titus Rivendare` gains its Health. |
| `Bronze Timepiece` (Greater) | Start of Combat: Give each of your minions Health equal to half its Attack. | Left to right, half its Attack rounded down **(engine decision)**. |
| `Ironforge Anvil` (Greater) | Start of Combat: Triple the stats of your minions with no type. | Each minion without a type (not an `All` one) gains twice its Attack and Health. |
| `Protective Ring` (Greater) | Start of Combat: Give 4 random friendly Pirates Divine Shield. | 4 different Pirates (all if fewer), one `rng.below(k)` each; one that already has a shield can be drawn **(engine decision)**. |
| `Lightfeather Sticker` (Greater) | Start of Combat: Give your Rally minions Divine Shield. | Every friendly minion with a Rally. |
| `Dramaloc Sticker` (Greater) | Start of Combat: Give your Murlocs the Attack of the highest-Attack minion in your hand. | Each Murloc gains that much Attack (rather than its Attack becoming it) **(engine decision)**; spells don't count; nothing without a minion in the hand. |
| `Evil Experiment` (Greater) | After your Deity awakens, give it Reborn. | `trinket_deity_awakened`: the Deity gains Reborn as it awakens. |
| `Beetle Band` (Lesser) | Avenge (5): Summon a 2/2 Beetle. Give it Taunt. | Every 5th friendly death in a combat: a `Beetle` (with the player's Beetle bonus) with Taunt at the right end. |
| `Staff of the Scourge` (Lesser) | Avenge (5): Give a random friendly Undead Reborn. | A random living Undead (one that has Reborn can be drawn **(engine decision)**), one draw. |
| `Lucky Tabby` (Lesser) | After 7 friendly minions die, get a random Beast. | Counted across combats **(confirmed)**; a random Beast of the player's Tier or lower from the game's catalog **(confirmed)** to the combat hand. |
| `Stormcoil Sticker` (Lesser) | After 6 friendly minions die, get a random Mech. | As `Lucky Tabby`, a Mech. |
| `Bleeding Heart` (Lesser) | After 8 friendly minions die, get a random Undead. | As `Lucky Tabby`, an Undead. |
| `Lionfish Portrait` (Lesser) | Get a Lurking Lionfish. Whenever a friendly Beast attacks, give it +4/+2. | The attacker gains it before it strikes, in combat or in the Tavern (a Fishbait attack). |
| `Beetle Band` (Greater) | Avenge (7): Summon two 2/2 Beetles. Give them Taunt. | As the Lesser one, two Beetles. |
| `Fridge Magnet` (Greater) | Avenge (3): Get a random Magnetic minion. | Every 3rd friendly death in a combat: a random Magnetic minion of the player's Tier or lower from the catalog, to the combat hand. |
| `Faerie Dragon Scale` (Greater) | Whenever a friendly Dragon attacks, give it Divine Shield. (3 times per combat.) | Before it strikes; a Dragon that still has one keeps it and uses no charge **(confirmed)**. |
| `All-Purpose Kibble` (Greater) | Whenever a friendly Beast attacks, give it +3 Attack and permanently improve this. | `+(3 + stacks)` Attack, then each improvement is +1 for the rest of the game **(confirmed)**. |
| `Slamma Sticker` (Greater) | After you summon a Beast in combat, double its Attack. | Every Beast summoned on the player's side in combat, after the other summon modifiers. |
| `Blood Golem Sticker` (Greater) | Whenever a friendly Quilboar dies, summon a Golem with stats equal to its Blood Gems. | A `Blood Golem` with the stats its Blood Gems gave it (`blood_gem_stats_applied`) at the right end; none without them **(confirmed)**. |
| `Jar o' Gems` (Greater) | After 2 friendly minions attack each combat, this plays a Blood Gem on all your Quilboar. | Once per combat, after the 2nd friendly attack in combat **(engine decision**: not every 2 attacks; Tavern attacks don't count**)**; the Gems are played, not cast. |
| `Herald Sticker` (Greater) | Start of Combat: Trigger all friendly Deathrattles. | Each friendly minion with a Deathrattle (its own or one an effect gave it), left to right as the board was, if alive, triggers them as `Deathstrider` does (repeated by `Titus Rivendare`), summons to its right. |
| `Mama Bear Sticker` (Lesser) | Whenever you summon a Beast, give it +6/+6. | In the Tavern (`trinket_tavern_summon`: a Beast played, or summoned by a card) and in combat (`trinket_combat_summon`). |
| `Quilligraphy Set` (Lesser) | Avenge (3): Your Blood Gems give an extra +1 Health this game. | `blood_gem_bonus_hp` +1; the count starts again each combat **(confirmed)**. |
| `The Eye of Dalaran` (Lesser) | Whenever a friendly minion with no type dies, get a random Tavern spell. | Each friendly minion without a type that dies in combat: a spell of the player's Tier or lower, drawn as the shop draws one, to the combat hand. |
| `Herding Horn` (Lesser) | Whenever a friendly Rally minion attacks, gain a free Refresh. | `auras.free_refreshes` +1, which carries over to the next turn; a Tavern attack (a Fishbait) counts **(confirmed)**. |
| `Quilligraphy Set` (Greater) | Avenge (4): Your Blood Gems give an extra +1/+1 this game. | As the Lesser one: Avenge (4), +1/+1. |
| `Wicked Tome` (Greater) | Avenge (4): Your Tavern spells give an extra +1/+1 this game. | `spell_bonus_atk` / `spell_bonus_hp` +1/+1 (as `Intrepid Botanist`'s); the count starts again each combat **(confirmed)**. |

"Per combat" counts and charges (Avenge, "3 times per combat", "each combat") start again every combat (`trinket_combat_start`) **(confirmed)**; "after N friendly minions die" counts go across combats **(confirmed)**.

### 11.2 Tavern Trinkets

"At the start of each turn" is the trinket's `trinket_turn_start`, after the Hero Power's turn start. A random minion is of the player's Tier or lower from the game's catalog **(engine decision)**.

| Trinket | Text | How the engine reads it |
| :--- | :--- | :--- |
| `Worn Treasure Map` (Lesser) | In two turns, gain 10 Gold. (Once per game.) | At the 2nd turn start after it is picked (turn 8 for one picked on turn 6). |
| `Stuffed Coin Purse` (Lesser) | When you reach Tier 6, gain 12 gold. | The Upgrade to Tier 6 gives 12 Gold, once; picked at Tier 6 already, at once **(engine decision)**. |
| `Bartend-o-Tron's Oilcan` (Lesser) | Reduce the Cost of upgrading the Tavern by (3). At the start of each turn, repeat this. | `upgrade_cost` 3 lower (never below 0) as picked and at each turn start, after the turn's own decay. |
| `Corrupted Coin Portrait` (Lesser) | Get 2 Corrupted Coins. | Fresh `Corrupted Coin`s. |
| `Reflective Pendant` (Lesser) | Get a plain copy of a random friendly minion. At the start of each turn, repeat this. | A plain copy (`cards::plain_instance`) of a random board minion, one draw; nothing with an empty board. |
| `Scraper Sticker` (Lesser) | Get a random Magnetic Mech. At the start of each turn, get another. | Of the player's Tier or lower **(engine decision)**. |
| `Sellemental Portrait` (Lesser) | Get a Sellemental. At the start of each turn, get another. | A plain `Sellemental` from the catalog. |
| `Volumizer Portrait` (Lesser) | Get a random Magnetic Volumizer. At the start of each turn, get another. | A `Blue`, `Green` or `Red Volumizer`, one `rng.below(3)`. |
| `Transcribing Typewriter` (Lesser) | Get an extra copy of the next 2 minions you buy. | `trinket_after_buy`: a plain copy of each of the next 2 minions bought (not from the pool **(engine decision)**); the copy can complete a Triple. |
| `Trickster's Sleeve` (Lesser) | After you buy a minion, gain 1 Gold. (Then swap to Tavern spell!) | It waits for a minion bought, then for a Tavern spell bought, and so on (`armed`); each gives 1 Gold and swaps it. |
| `Willbreaker Sticker` (Lesser) | Get 2 random Tavern spells. When you play one, discard the other. At the start of your turn, repeat this. | 2 spells drawn as the shop draws one, as one `Wandering Willbreaker` group (`tavern.md` §6). |
| `Bob's Tip Jar` (Greater) | Gain 4 Gold. Increase your maximum Gold by 4. | As `Strike Oil` for the maximum (`max_gold`, `base_max_gold_bonus`). |
| `Butcher's Sickle` (Greater) | Get a Butchering. At the start of each turn, get another. | Fresh `Butchering`s. |
| `Transcribing Typewriter` (Greater) | Get an extra copy of the next 4 minions you buy. | As the Lesser one, 4 copies. |
| `Volumizer Portrait` (Greater) | Get 2 random Magnetic Volumizers. At the start of each turn, repeat this. | As the Lesser one, 2 each time. |
| `Nomi Sticker` (Lesser) | After you play an Elemental, give Elementals in the Tavern +3/+2 this game. | Each Elemental played from the hand: the Tavern's Elemental bonus +3/+2 (as `Dune Dweller`'s), the shop's Elementals now. |
| `Corrupted Baton` (Lesser) | After you cast a Tavern spell, give your Deity +4/+4. | Each cast of a Tavern spell in the Tavern (`auras.deity`). |
| `Rendle Sticker` (Lesser) | Steal the highest-Tier card from the Tavern. At the end of each turn, repeat this. | As picked and at each turn end: the Tavern's card of the highest Tier (minion or spell; ties broken at random, one `rng.below(k)` among the tied **(confirmed)**) goes to the hand as it is, not bought (no buy triggers, nothing paid). |
| `Innkeeper's Stein` (Lesser) | The Tavern offers an extra minion of a higher Tier whenever it is Refreshed. | Every Refresh, the turn start's too (tavern.md §4.1), adds a minion of `tavern_tier + 1` (Tier 6 at Tier 6 **(confirmed)**) from the pool (`draw_discover_options`) at the end of the Tavern if it has room. |
| `Water Wheel` (Lesser) | After you play an Elemental, get a random Tavern spell. (Twice per turn.) | The first 2 Elementals played from the hand each turn; the spell is drawn as the shop draws one. |
| `Defensive Shell` (Lesser) | The first minion you play each turn gains Divine Shield. | Once its play resolved; a Magnetization gives it to the minion Magnetized onto **(engine decision)**. |
| `Nomi Sticker` (Greater) | After you play an Elemental, give Elementals in the Tavern +5/+5 this game. | As the Lesser one, +5/+5. |
| `Corrupted Baton` (Greater) | After you cast a Tavern spell, give your Deity +10/+10. | As the Lesser one, +10/+10. |
| `Lava Lamp` (Greater) | After you sell 6 minions, get a random Elemental. | Counted across turns. |
| `Secret Schematic` (Greater) | After you buy a Mech, get a random Tavern spell. | Drawn as the shop draws one. |
| `Recycling Sticker` (Greater) | After you play an Elemental, gain a free Refresh. | `free_refreshes`, which carries over. |
| `Cookie's Stirring Rod` (Greater) | After you play 5 Murlocs, get two random Tavern spells. | Counted across turns; drawn as the shop draws one. |
| `Sphere of Memory` (Greater) | At the end of your turn, get 3 copies of the last Tavern spell you cast. | Fresh copies of `last_tavern_spell_cast`; nothing before any. |
| `Ghastly Sticker` (Greater) | Your end of turn effects trigger an extra time. | Every end-of-turn effect triggers once more **(confirmed)**: the minions' end-of-turn triggers (one more time on top of `Drakkari Enchanter`'s: 3 times with both, **engine decision**), then the Hero Power's `hero_end_turn` twice, then every trinket's `trinket_end_turn` twice. There are no end-of-turn player effects or hand triggers. |
| `Goblin Wallet` (Lesser) | At the end of each turn, increase your maximum Gold by 1. | As `Strike Oil` (`max_gold`, `base_max_gold_bonus`), above 10 too **(confirmed)**. |
| `Explorer's Binoculars` (Lesser) | Get three random Tier 4 minions. | Exactly Tier 4, whatever the Tavern's Tier, one draw each. |
| `Magician's Top Hat` (Lesser) | Get two minions each from Tiers 1, 2, and 3. | 2 random minions of Tier 1, then 2 of Tier 2, then 2 of Tier 3. |
| `Fish Portrait` (Lesser) | Get a Fish of N'Zoth. | N'Zoth's `Fish of N'Zoth` token, which gains Deathrattles in combat as N'Zoth's does. |
| `Rockin' Music Box` (Lesser) | Get a random Battlecry minion. At the start of each turn, get another. | Of the player's Tier or lower. |
| `Rascal Sticker` (Lesser) | Gain 1 Gold for each friendly Golden minion. At the start of your turn, repeat this. | The Golden minions on the board, not in the hand **(confirmed)**. |
| `Frigid Blossom` (Lesser) | After you Refresh, reduce the cost of upgrading the Tavern by (1). | `trinket_after_player_refresh`: a Refresh the player does, not the turn start's new shop **(confirmed**; tavern.md §4.1**)**; never below 0. |
| `Nerglish Phrasebook` (Lesser) | After you play a minion, give the left-most minion in your hand +3/+3. | After each minion played from the hand; spells in the hand are skipped. |
| `Bluegill Flippers` (Lesser) | Whenever you cast a Tavern spell, give the left-most minion in your hand and warband +3/+3. | Each cast of a Tavern spell: the left-most minion in the hand (spells skipped) and `board[0]`. |
| `Flighty Portrait` (Lesser) | Get a Flighty Scout. After you cast a Tavern spell, give Flighty Scouts in your hand and warband +3/+3. | Every `Flighty Scout`, Golden too. |
| `Weighted Gauntlet` (Lesser) | At the end of your turn, give your minions +1/+1. (Doubles at the start of each turn!) | `+2^counter`, doubling at each turn start from the turn after the pick, without a cap: from `2^31` on it is `+2,147,483,647`, and stats saturate there (tavern.md §4.2) **(confirmed)**. |
| `Enigmatic Headstone` (Lesser) | At the end of each turn, your Undead have +2 Attack this game (wherever they are). | `auras.undead_bonus_attack`, as `Nerubian Deathswarmer`'s. |
| `Comfy Coffin` (Lesser) | Whenever you cast a Tavern spell, your Undead have +1 Attack this game (wherever they are). | `auras.undead_bonus_attack`. |
| `Beatboxer Portrait` (Greater) | Get a Polarizing Beatboxer. | A plain one from the catalog. |
| `Nerglish Phrasebook` (Greater) | After you play a minion, give the left-most minion in your hand +6/+6. | As the Lesser one, +6/+6. |
| `Miniature Ship` (Greater) | After you cast a Tavern spell, give your Pirates +3/+3. | The Pirates on the board. |
| `Comfy Coffin` (Greater) | Whenever you cast a Tavern spell, your Undead have +2 Attack this game (wherever they are). | As the Lesser one, +2. |
| `Artisanal Urn` (Greater) | Your Undead have +20 Attack. | `auras.undead_bonus_attack` +20 as picked (the trinket is kept). |
| `Demonic Bloodletter` (Lesser) | Whenever you cast a Tavern spell, give minions in the Tavern +1/+1 this game. | The Tavern's bonus (`tavern_all_atk` / `tavern_all_hp`, as `Staff of Enrichment`'s) +1/+1, the shop's minions now. |
| `Great Boar Sticker` (Lesser) | Get 3 Blood Gems. Your Blood Gems give an extra +2/+1. | The Blood Gem bonus (`blood_gem_bonus_atk` / `_hp`, as `Gem Day`'s) +2/+1 as picked, then 3 Blood Gems. |
| `Glass of Perspective` (Lesser) | Get a random Choose One card. At the start of each turn, get another. | A Choose One minion of the player's Tier or lower. |
| `Conch Portrait` (Lesser) | Get a Cloning Conch. At the start of every 2 turns, repeat this. | Every 2nd turn start after the pick (`counter`). |
| `Orc-estra Portrait` (Lesser, Duos) | Get an Orc-estra Conductor. At the start of each turn, get another. | A plain one from the catalog. |
| `Baller Portrait` (Lesser) | Get a Fire Baller or a Snow Baller. At the start of each turn, repeat this. | One of the two at random, one `rng.below(2)` **(confirmed)**. |
| `Pocket Cyclone` (Lesser) | Cast Easterly Winds. At the start of each turn, cast it again. | Cast by an effect (`spells::cast_spell_by_effect`): a Tavern spell the player casts, which the spell-cast observers (the trinkets' too) see **(confirmed)**. |
| `Emergency Gearblade` (Lesser) | At the end of your turn, cast Repair Job on your left-most Mech. | Cast by an effect on the left-most Mech on the board, a spell the player casts **(confirmed)**; nothing without a Mech. |
| `Spell-powered Wrench` (Lesser) | After you play a Magnetic minion, cast Repair Job on a random friendly Mech. | After a Magnetic minion played from the hand (Magnetized or placed): cast by an effect on a random Mech on the board, one draw, a spell the player casts **(confirmed)**. |
| `Booty Bay Brew` (Lesser) | Whenever you spend Gold, give two friendly Pirates +3/+3. | `trinket_gold_spent`: each spending, whatever the amount; 2 different random Pirates on the board, every Pirate if there are 2 or fewer **(confirmed)**. |
| `Dalaran Cheese Wheel` (Greater) | Minions in the Tavern have +2/+2. Improves every 4 times you Refresh. | The Tavern's bonus +2/+2 as picked, and +2/+2 more every 4th Refresh the player does (`trinket_after_player_refresh`, counted across turns; not the turn start's new shop **(confirmed)**); +2/+2 per improvement **(engine decision)**. |
| `Great Boar Sticker` (Greater) | Get 5 Blood Gems. Your Blood Gems give an extra +3/+3. | As the Lesser one: +3/+3, 5 Blood Gems. |
| `Glass of Perspective` (Greater) | Get 2 random Choose One cards. At the start of each turn, get 2 more. | As the Lesser one, 2 each time. |
| `Baller Portrait` (Greater) | Get a Temperature Shift. After you play 11 Elementals, repeat this. | Every 11th Elemental played from the hand (counted across turns). |
| `Pocket Cyclone` (Greater) | Cast Easterly Winds four times. At the start of each turn, cast it twice more. | As the Lesser one: 4 casts as picked, 2 at each turn start. |
| `Booty Bay Brew` (Greater) | Whenever you spend Gold, give two friendly Pirates +6/+6. | As the Lesser one, +6/+6. |
| `Azeroth Model Globe` (Lesser) | At the start of every two turns, gain 2 Gold and Discover a Tier 6 minion. | Every 2nd turn start after the pick: 2 Gold, then a Discover of 3 Tier 6 minions from the pool (`draw_discover_options`, as a Triple Reward). |
| `Gold Pendant` (Lesser) | Make a random friendly minion from Tier 4 or below Golden. At the start of each turn, repeat this. | A random board minion of Tier 4 or lower that isn't Golden (`Unit::make_golden`, as `Golden Touch`'s: one pool copy); nothing without one. |
| `Innkeeper's Hearth` (Greater) | Discover two Tier 6 minions. Set their stats to 30/30. | Two Discovers of 3 Tier 6 minions from the pool, one after the other; the options are 30/30 before the choice **(engine decision)**. |
| `Upstart Embers` (Greater) | After you Refresh, double the stats of the highest- Health minion in the Tavern. | `trinket_after_player_refresh`: a Refresh the player does, not the turn start's new shop **(confirmed)**; ties broken at random, one `rng.below(k)` among the tied **(confirmed)**. |

### 11.3 Deferred

| Trinkets | Why |
| :--- | :--- |
| `Wax Lance`, `Sinister Invitation`, `Maldraxxus Dagger`, and every trinket that gives or reads a Dark Gift | Dark Gifts **(confirmed)**. |
| `Timeworn Candelabra` (both) and the other Timewarp trinkets | Timewarp **(confirmed)**. |
| `Tickatus Sticker` and the other Darkmoon Prize trinkets | Darkmoon Prizes **(confirmed)**. |
| `Sunken Anchor`, `Privateer Portrait` and the other Bounty trinkets | Bounties **(confirmed)**. |
| The Tier 7 trinkets | Tier 7 minions **(confirmed)**. |
| `Mystery Cube`, `Trip Vouchers`, `Souvenir Stand`, `Empowered Mirror`, `Orb of the Unknown`, `Soul Battery`, `Dragonhead Staff`, `Ornate Clock` | They replace, copy or add trinkets, or move an offer (**engine decision**: later). |

The list grows as the batches go (§11.1, §11.2): every trinket of the table that isn't implemented is not offered.

---

## 12. Decisions

The rules the user settled are marked **(confirmed)** above: per-combat counts restarting every combat and "after N minions die" counts going across combats; Kibble's +1 improvements; Faerie Dragon Scale skipping a shielded Dragon; random cards as new copies of the player's Tier or lower; "Get a ..." cards as new copies, not from the pool; Blood Golem Sticker summoning nothing without Blood Gem stats; "after you Refresh" seeing only the Refreshes the player does, not the turn start's new shop; ties for "highest" / "lowest" broken at random; Innkeeper's Stein offering a Tier 6 minion at Tier 6; Ghastly Sticker repeating every end-of-turn effect; Weighted Gauntlet without a cap, stats saturating at 2,147,483,647; Goblin Wallet above 10 Gold; Rascal Sticker counting the board; Booty Bay Brew on every spending, on 2 different random Pirates; spells a trinket casts counting as spells the player casts; Herding Horn counting Tavern attacks; the Lesser Baller Portrait picking at random; the Lesser offer on turn 6 and the Greater one on turn 9, 4 options each; the pick before anything else, and no skipping; the price paid in Gold as the trinket is picked, and the trinket kept for the rest of the game; at least one option of 2 Gold or less; the 2 (Lesser) and 3 (Greater) minions a typed trinket needs; at least one option without a type; at most one option of a type that isn't the most common one, 2 Gold cheaper; never a trinket the player owns; cards a trinket gives waiting for room in the hand; Naga trinkets never offered; the deferred systems.

Engine decisions, pending the user's confirmation:

| Decision | Rule | Where |
| :--- | :--- | :--- |
| Where the state lives | `PlayerAuras::trinkets`, in pick order (§2.2). | `model.rs` |
| Weights | Uniform among the trinkets a draw step draws from (§3.3). | `trinkets::draw_offer` |
| Type counts | A two-type minion counts for both types, an `All` minion for every type, spells for none; a two-type trinket needs one eligible type, and is of a most common type if either is (§3.2). | `trinkets::type_counts`, `trinkets::draw_offer` |
| Most common type ties | Every tied type is a most common type (§3.2). | `trinkets::draw_offer` |
| The draw | Neutral first, then a cheap one if needed, then the rest, then a shuffle (§3.3). | `trinkets::draw_offer` |
| Unaffordable offers | If no option is affordable, the cheapest ones are legal and free (§4). | `TavernState::is_legal` (`trinkets::option_price`) |
| The action | Its own `ChooseTrinket`, before a pending Discover (§4). | `tavern.rs` |
| The price | Gold spent (the Gold-spent observers see it), paid before the trinket joins (§4). | `trinkets::choose` |
| The waiting queue | One queue in giving order; a trinket's card keeps waiting across turns (§5). | `tavern.rs` |
| Start of Combat | Step 1c, after both sides' Hero Powers, in pick order (§7.2). | `combat.rs` |
| Duos | Per player (§8.1). | `trinkets`, `duos_tavern.rs` |
| Batch 7 | Innkeeper's Hearth's options are 30/30 before the choice. | their files |
| Batch 6 | Dalaran Cheese Wheel improves by +2/+2. | its file |
| Batch 5 | A card's or Hero Power's Refresh is one the player does ("after you Refresh"); Ghastly Sticker adds one more time to `Drakkari Enchanter`'s. | tavern.md §4.1, `ghastly_sticker.rs` |
| Batch 4 | Defensive Shell on a Magnetization; Jar o' Gems ignores Tavern attacks. | their files |
| Batch 3 | Stuffed Coin Purse pays at once at Tier 6; Typewriter's copies don't come from the pool; random Magnetic Mechs of the player's Tier or lower. | their files |
| Batch 2 | Staff of the Scourge may draw an Undead with Reborn; Jar o' Gems once per combat. | their files |
| Batch 1 | Bronze Timepiece rounds down; Protective Ring may draw a shielded Pirate; Dramaloc Sticker's Murlocs gain the Attack. | their files |
