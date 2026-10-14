# Heroes Specification

This document specifies Battlegrounds **heroes** in Seaglass: the hero a player plays, their starting Health and Armor, their **Hero Power** and its state, how Hero Powers reach the Tavern and combat, the RL action, and hero selection. It builds on [`tavern.md`](tavern.md), [`combat.md`](combat.md), [`duos_tavern.md`](duos_tavern.md) and [`duos.md`](duos.md), and like them it is updated **before** behavior changes. The engines stay card-agnostic: what a Hero Power does lives in its hero's file (`src/heroes/<hero>.rs`) and reaches the engine only through hooks.

Rules marked **(confirmed)** were settled with the user. Rules marked **(engine decision)** are details nobody has settled yet: the engine implements them as written, each in one place, so that it is a small change if the answer differs (§12).

**A player without a hero** (the default: `PlayerAuras::hero` is `None`) plays exactly as before this document: 30 Health, no Armor, no Hero Power, no new legal action, no RNG draw. Solo and Duos results are byte-identical for them.

---

## 1. Scope

### Included
- The hero pool (123 heroes, Patch 36.6.3 data, `src/heroes/table.rs`), generated from `CardDefs.xml` (§9).
- Each hero's starting Health, Solo and Duos Armor, and Hero Power (cost, uses per turn, counters, target).
- The `HeroPower` and `ChooseHero` actions, hero offers, the Duos team's Health and Armor.
- The Hero Power hooks (§4), the Start of Combat step for Hero Powers (§7), the RL encoding (§8), and the Power.log replay (§10).
- The Hero Powers implemented so far: §11.

### Deferred
- Buddies (companions), Trinkets, Quests, Darkmoon Prizes, Anomalies.
- Hero Powers that need another player's state (Scabbs, Tess, Murloc Holmes, Lord Barov, Mr. Bigglesworth, Galewing's flightpaths, ...): their heroes are in the table but not implemented, and not offered (§6).

---

## 2. The Hero

### 2.1 Hero Data

Every hero of the pool is a `HeroData` row of the generated table (`heroes::table()`):

| Field | Meaning |
| :--- | :--- |
| `id` | The hero's `HeroId`: its dbfId (`64403` for Al'Akir), stable across patches. |
| `card_id`, `name` | Its card ID (`TB_BaconShop_HERO_76`) and name. Its skins (`skins`: `..._SKIN_A`, ...) map to it. |
| `health` | Starting Health: 30, or 60 for Patchwerk. |
| `armor` | Starting Armor in Solo (0 for a Duos-only hero). |
| `duos_armor` | Starting Armor in Duos (`CardDefs` tag 3206). |
| `duos_armor_from_solo` | The data has no Duos Armor for this hero: it uses its Solo Armor in Duos **(engine decision)**: Edwin VanCleef, Enhance-o Mechano, Nightmare Lord Xavius, Overlord Saurfang, Patchwerk, Ragnaros, Rakanishu. |
| `duos_only` | Only in Duos (`IS_BACON_DUOS_EXCLUSIVE`): Cho, Gall, Flobbidinous Floop, Madam Goya, The Nameless One. |
| `power` | Its Hero Power: dbfId (the power's engine `CardId`), card ID, name, Gold cost, text. |
| `power_variants` | The other forms of its Hero Power (`BACON_HEROPOWER_BASE_HERO_ID`): Bru'kan's four Elements, upgraded forms. |

The catalog API (`src/heroes/mod.rs`): `heroes::table()`, `heroes::data(id)`, `heroes::by_name(name)`, `heroes::by_card_id(card_id)` (a skin finds its hero), `heroes::is_implemented(id)`, `heroes::offerable(duos)`.

### 2.2 The Hero's State (`HeroState`)

A player's hero is `PlayerAuras::hero: Option<HeroState>` **(engine decision: where it lives)**. It lives in the player's auras so that it travels into combat and back with them, as every other player effect does (Combat-to-Tavern Persistence, `combat.md` §1): a Solo side's `GameState` auras and a Duos `Warband`'s auras carry it, and what combat changes in it comes back. `TavernState::hero()` reads it.

| Field | Meaning |
| :--- | :--- |
| `hero_id` | The hero (`HeroId`). |
| `power` | Its Hero Power's state (`HeroPowerState`, §3.2). |

### 2.3 Health and Armor at Game Start

When a player gets their hero (`TavernState::set_hero`, the `ChooseHero` action, or a constructor):

1. **Solo**: the player starts at the hero's `health` and gains its `armor`: 30 Health and the hero's Armor, or Patchwerk's 60 Health and 0 Armor **(confirmed)**.
2. **Duos**: the team's pool gains the hero's `duos_armor` as Armor, and its Health beyond 30 as Health. So the team starts at **30 Health plus both heroes' Duos Armor** **(confirmed)**, and a team with Patchwerk at 60 Health plus his teammate's Duos Armor **(confirmed)**. Starting Armor differs between Duos and Solo, as in the data **(confirmed)**.
3. A player's auras say whether they are in a Duos game (`PlayerAuras::duos_game`), so the Duos value applies through a team's Tavern (`DuosTavern::set_hero`, `ChooseHero`, `DuosTavern::with_heroes`).
4. The hero's own start-of-game text (its `hero_chosen` hook, §4) then resolves.

`DuosTavern::with_armor([a0, a1])` still sets the team's Armor directly (tests, a team without heroes).

---

## 3. The Hero Power

### 3.1 Kinds

A Hero Power is a **card with hooks** (§4), registered in the card registry under its dbfId. What it does depends on which hooks it has:

- **Active**: it has a `hero_power` hook. The player uses it with the `HeroPower` action (§5), paying its cost, a limited number of times per turn. It may need a target (§3.3).
- **Passive**: no `hero_power` hook. It acts through its other hooks when things happen (a minion is bought, the turn starts, a friendly minion kills an enemy, ...). It is never an action.
- **Start of Combat**: a power (active or passive) with a `hero_start_of_combat` hook acts at Start of Combat (§7). An active one may need to be used first in the Tavern to act in the next combat (Y'Shaarj's `armed`).

### 3.2 State (`HeroPowerState`)

| Field | Meaning |
| :--- | :--- |
| `card_id` | The Hero Power card in use (its hooks). It starts as the hero's `power.id` and may change: a chosen Element (Bru'kan), an upgraded form. |
| `cost` | Its current Gold cost. It starts at the table's cost; hooks may change it. |
| `uses_left` | Uses left this turn: set at the start of each turn (§3.4). |
| `counter` | Progress toward a threshold (Gold spent toward Wax Warband's next improvement, "14 left!"). |
| `stacks` | Improvements so far (Wax Warband's level). |
| `armed` | A next-combat effect the player paid for (Y'Shaarj's Embrace Your Rage: one per use). It lasts until the player's next combat, where the power uses it up. |
| `stored` | Cards the power keeps (Teron's exact copy waiting for room in combat). |

All are generic: the engine never reads `counter`, `stacks`, `armed` or `stored`; the power's own hooks do.

### 3.3 Targets

An active power's target domain is its hooks' `activate_target` (the `ActivateTargetKind` of minion Activates, `tavern.md` §7): `None` (untargeted), `BoardAny` (a friendly minion: Teron, Floop), `ShopCard` (a minion in the Tavern), `HandCard`. `HeroPower { target }` names the index in that zone. A power that needs a choice among options (Bru'kan's Elements) is untargeted: its `hero_power` hook offers a Choose One (`discover_pending`), answered with `ChooseDiscover` like any Choose One (`tavern.md` §5.2).

A minion a power chose to act on later (Teron's Rapid Reanimation, Floop's Glorious Gloop) is **marked** (`Unit::marked`). Choosing again moves the mark. The mark lasts until the player's next combat, where the power acts on the marked minion, and it clears at the start of the next turn **(confirmed)**. It is part of the minion: it leaves with it (sold, Passed) and is not copied by a triple.

### 3.4 Uses per Turn

At the start of each turn (`tavern.md` §4 step 1), each player's Hero Power gets `uses_left = 1` if it is active, `0` if it is passive; then its `hero_turn_reset` hook may change that (twice per turn, unlock turns) and reset turn-long state (Bru'kan's Element reverts, §11). A player who never started a turn has `uses_left = 1` for an active power from `set_hero`, so a power is usable on turn 1.

### 3.5 Hero Powers Are Not Spells

Using a Hero Power is not casting a spell, a Tavern spell, a Battlecry or a play **(confirmed)**: no `spell_cast`, `after_spell_cast`, `after_friendly_play` or Battlecry observer sees it. Its Gold cost is Gold spent (`TavernState::spend_gold`): it counts for `gold_spent_this_turn` and the Gold-spent observers (`Lovesick Balladist`, Wax Warband's improvement).

---

## 4. Hooks

Hero Powers reuse the card hook system (`src/cards/hooks.rs`): a Hero Power is a card whose `CardHooks` table the registry holds under its `CardId` (its dbfId; engine card ids are below 11000, so they never collide). Its hooks are invoked for the player whose hero has it (`PlayerAuras::hero`), never for minions. New hooks in the **Hero Powers** section of `CardHooks`:

| Hook | Fires | For |
| :--- | :--- | :--- |
| `hero_power` | The `HeroPower` action (§5): `(state, target, pool, rng)`. | Active powers. |
| `hero_power_usable` | Extra conditions for the action (unlock turns, once per game): `(state) -> bool`. | "Unlocks on Turn 3". |
| `hero_turn_reset` | Start of turn, after `uses_left` is reset (§3.4): `(hero)`. | Bru'kan's Element reverting, twice-per-turn powers. |
| `hero_chosen` | The player gets this hero (§2.3): `(state, rng)`. | Start-of-game text. |
| `hero_gold_spent` | Gold was spent: `(state, amount)`. | Wax Warband's improvement. |
| `hero_start_of_combat` | Start of Combat (§7): `(ctx)`, a `HeroCombatCtx` (the player's side as a `BoardCtx`, the enemy board, damage to enemy minions). | Al'Akir, Deathwing, Illidan, Y'Shaarj, Teron, Tamsin, Wagtoggle, Bru'kan, Floop. |
| `hero_combat_space` | Room may have opened on the board in combat (after Start of Combat summons and each round of deaths, like `player_combat_space`): `(ctx)`. | Teron's resummon. |
| `hero_enemy_killed` | A friendly minion killed an enemy minion in combat: `(ctx, killer_id, victim)`. | Rokara. |

Extra Deathrattles a Hero Power gives a minion in combat (Tamsin's Fragrant Phylactery, Bru'kan's Earth Invocation) are cards too: `Unit::extra_deathrattles` lists their card ids, and their `deathrattle` hooks resolve after the minion's own Deathrattle, each counting as a Deathrattle triggered and repeated by `Titus Rivendare` like it.

More hooks are added with the Tavern Hero Powers that need them (§11).

---

## 5. The `HeroPower` Action

`TavernAction::HeroPower { target: Option<usize> }` uses the player's Hero Power. It comes after `Pass` in `TavernAction`, so the earlier variants keep their places; `ChooseHero` (§6) comes after it.

It is legal when:

1. no Discover, Choose One or hero choice is pending (as for every action: `tavern.md` §6);
2. the player has a hero whose current power is active (a `hero_power` hook);
3. `uses_left > 0` and `gold >= cost`;
4. its `hero_power_usable` hook (if any) allows it;
5. `target` fits the power's target domain (§3.3): `None` for an untargeted power, a valid index otherwise.

Effect, in order: `uses_left -= 1`; the cost is spent (`spend_gold`, §3.5); the `hero_power` hook resolves. Then the action ends like any other (Triples, aura sync: `tavern.md` §6).

In Duos, `DuosTavern::is_legal` / `step` allow it for the player's own Tavern like every action but Pass (`duos_tavern.md` §4.2).

---

## 6. Hero Selection

- **Offers**: at the start of a game each player is offered heroes to choose from. The engine offers **2** heroes per player in both Solo and Duos **(confirmed; a parameter, `OFFER_COUNT`)**, with **no rerolls** **(engine decision)**, distinct across the lobby (no two players are offered the same hero) **(engine decision)**. Only implemented heroes are offered (`heroes::offerable`), and Duos-only heroes only in Duos.
- `heroes::draw_offers(players, per_player, duos, rng)` draws them: for each player in turn, `per_player` distinct heroes uniformly at random among the offerable heroes no earlier player was offered (one `rng.below(k)` per hero). A match of several Taverns is the caller's job, as for combat pairings.
- `TavernState::offer_heroes(options)` sets `hero_options`. While it is non-empty, **only** `ChooseHero { option_index }` is legal (`option_index < hero_options.len()`), as with a pending Discover.
- `TavernAction::ChooseHero { option_index }` (the last variant) gives the player that hero (§2.3) and clears `hero_options`. In Duos it goes through `DuosTavern::step` like any action, and the team's pool gets the hero's Duos Armor (§2.3).

---

## 7. Combat

### 7.1 The Hero Power in Combat

The player's `HeroState` travels in their `PlayerAuras`: the Solo `GameState`'s `auras_a` / `auras_b`, a Duos `Warband`'s `auras`. Its combat hooks act for that side, and what they change in the state (Y'Shaarj's `armed`, Teron's `stored`) comes back in the result's auras. A Duos warband that never fought comes back unchanged, so its armed power waits for its next combat.

### 7.2 Where Hero Powers Fall in Start of Combat

`combat.md` §5.1.1 gains a step **1b**, after the order is decided (step 1): Hero Powers resolve before everything else, before the player effects (`Upper Hand`, ...), `Flighty Scout`'s copies, `Boon of Beetles` and the minions' Start of Combat, the first side's Hero Power first, then the other's **(confirmed)**:

1. **Order** (unchanged). A side's Hero Power with a `hero_start_of_combat` hook counts as a Start of Combat effect for the order coin flip (§4.4).
1b. **Hero Powers**, for the first side, then the other: the side's `hero_start_of_combat` hook. Then, in this order: the damage it deals to enemy minions (one `apply_damage` each, with their damage-taken triggers), the minions it destroys (`BoardCtx::destroy`: each is set to 0 Health and a `Death` is emitted), the deaths (`combat.md` §5.1.1 step 4: the other side first), and its immediate attacks (Illidan's Wingmen), as a minion's `SocAction::Attack` would.
2. **Player effects** (unchanged), then hand copies and open-slot effects, for which the Hero Powers' `hero_combat_space` hooks run with the player effects' `player_combat_space` (Hero Power first).
3. **Minion triggers** (unchanged).

In Duos, a warband stepping in resolves step 1b for its own Hero Power, then its other steps (`duos.md` §3.2).

### 7.3 Kills

When a minion dies in combat and its `killed_by` is an enemy minion (it died from that minion's damage), the killer's player's `hero_enemy_killed` hook fires with the killer's id and the dead minion, after the dead minion's side has resolved that round of deaths' Deathrattles (`combat.md` §5.3.1 step 9), before the killer's side's deaths. A minion destroyed by an effect (Venomous counts as the attacker's damage) is killed by whoever dealt the damage.

### 7.4 Permanent Combat Gains

"Permanently" in a Hero Power's combat text (Deathwing, Rokara) means `perm_atk_gained` / `perm_hp_gained` on the combat copy: the minion's Tavern copy keeps it after combat, as with `Devout Hellcaller` (`combat.md` §1). It applies to whichever side's minions gained it: Deathwing's +2 Attack is permanent on the enemy's minions too **(confirmed)**.

---

## 8. Python / RL

- `TavernGame(..., hero=None)` and `DuosTavernGame(..., heroes=None)` (a hero per player, a name or a `HeroId`) give the players their heroes at creation. The default (`None`) is no hero, so existing code is unchanged.
- `TavernAction.hero_power(target=None)` and `TavernAction.choose_hero(option_index)`; their `kind`s are `"hero_power"` and `"choose_hero"`.
- The observation (`TavernObservation`, `DuosObservation`) has `hero_id` (0 without a hero), `hero_power_id`, `hero_power_cost`, `hero_power_uses_left`, `hero_power_counter`, `hero_power_stacks`, `hero_power_armed`, `hero_power_usable` (the untargeted or some targeted `HeroPower` is legal) and `hero_options`.
- The RL action table (`rl/seaglass_rl/actions.py`) appends, at the end: `hero_power(None)`, `hero_power(t)` for `t` in `0..10`, then `choose_hero(i)` for `i` in `0..4` (room for more offers than the default 2).

`rl/README.md` describes the encoding.

---

## 9. The Hero Table and Its Generator

`src/heroes/table.rs` is generated by `tools/gen_heroes.py` from HearthstoneJSON's `CardDefs.xml`; rerun it each patch:

```sh
python3 tools/gen_heroes.py path/to/CardDefs.xml
```

It reads the tags listed in §2.1 (the pool is every hero with `BACON_HERO_CAN_BE_DRAFTED`), renders each Hero Power's text as HearthstoneJSON does (the numbers at their starting values, markup removed, whitespace collapsed), and sorts the heroes by name.

Each implemented hero has its own file, `src/heroes/<slug>.rs`, whose first lines are its header:

```text
//! `Al'Akir` (`TB_BaconShop_HERO_76`) — Armor 15 (Duos 13).
//!
//! Hero Power: `Swatting Insects` (`TB_BaconShop_HP_086`), 0 Gold: Start of Combat: Give your left-most minion Windfury, Divine Shield, and Taunt.
```

The `heroes` tests check that every hero file's header matches the table (name, card ID, Armor, Hero Power card ID, cost and text), that its `ID`/`POWER` constants are the table's dbfIds, and that every hero file has a scenario file `tests/scenarios/heroes/<slug>.yaml` starting with `hero: <Hero Name>`.

---

## 10. Replay

The Power.log validator (`replay/src/{reconstruct,validate,diagnose}.rs`) maps each player's hero, Hero Power and Armor from the log into the reconstruction: the hero's card ID (or skin) to its `HeroId`, the Hero Power's card ID (any form: an Element, another hero's power on Master Nguyen) to its engine `CardId`, and the power's combat state from the log (Teron's and Floop's marked minion from their enchantment, Y'Shaarj's `armed` from the power's activation in the Tavern). `replay_cli validate` then simulates the implemented combat Hero Powers instead of reporting them as gaps.

---

## 11. The Hero Powers

Golden doesn't apply to Hero Powers. "Your Tier" is the player's Tavern Tier (`hero_tier` in combat).

### 11.1 Combat Hero Powers

| Hero | Hero Power | How the engine reads it |
| :--- | :--- | :--- |
| `Al'Akir` | *Swatting Insects* (passive): Start of Combat: give your left-most minion Windfury, Divine Shield, and Taunt. | Step 1b. Nothing without a minion. |
| `Deathwing` | *ALL Will Burn!* (passive): Start of Combat: give ALL minions +2 Attack permanently. | Step 1b: every minion on both boards, the player's left to right, then the enemy's; permanent (§7.4) on both sides **(confirmed)**. Only the minions on the boards then: later summons don't get it. |
| `Illidan Stormrage` | *Wingmen* (passive): Start of Combat: your left- and right-most minions gain +2/+1 and attack immediately. | Step 1b: the left-most gains +2/+1, then the right-most; then the left-most attacks, then the right-most (immediate attacks, `combat.md` §5.4: they don't move the pointer). With one minion, it gains +2/+1 once and attacks once **(confirmed)**. |
| `Y'Shaarj` | *Embrace Your Rage* (2 Gold, active): Start of Combat: summon and get a minion of your Tier. | Using it arms the next combat (`armed += 1`). Step 1b, once per arm: a random minion of exactly your Tier from the game's catalog (`cards::game_templates`; Duos-exclusive minions in Duos) is summoned at the right end of the board (if there is room) and a plain copy goes to your hand (if it has room) **(engine decision: both independent)**. One `rng.below(k)` per minion. The arms are used up. Once per turn. |
| `Teron Gorefiend` | *Rapid Reanimation* (0 Gold, active, a friendly minion): Start of Combat: destroy it. When you have space, resummon an exact copy. | Using it marks the minion (§3.3). Step 1b: the marked minion, if it is on the board, is destroyed (its Deathrattles and Reborn resolve) and an exact copy of it as it was (stats, keywords, state; full Health) is stored; the copy is summoned at the right end as soon as there is room (`hero_combat_space`: right after the destroy's deaths, or after a later round of deaths). |
| `Tamsin Roame` | *Fragrant Phylactery* (passive): Start of Combat: give your lowest-Attack minion "Deathrattle: give your other minions this minion's stats." | Step 1b: the left-most of the minions with the lowest Attack gains the extra Deathrattle **(engine decision: ties)**. When it dies, the player's other minions on the board gain its Attack and its Health (`max_health`, the Health it had before damage) **(engine decision)**. |
| `Queen Wagtoggle` | *Wax Warband* (passive): Start of Combat: give a friendly minion of each type +1/+1. (Improves after you spend 10 Gold!) | Step 1b: a friendly minion of each type (`tavern.md` §7.8: the best assignment, as `Misplaced Tea Set`) gains `+N/+N`, `N = 1 + stacks`. Every 10 Gold the player spends (`hero_gold_spent`, `counter`) adds a stack. |
| `Bru'kan` | *Embrace the Elements* (0 Gold, active): Choose an Element. Start of Combat: call upon that Element. | Using it offers a Choose One of the four Elements; the chosen one becomes the Hero Power (`card_id`) for the next combat only, and reverts at the start of the next turn **(confirmed)**. **Earth Invocation**: 4 different random friendly minions (fewer if fewer) gain "Deathrattle: summon a 1/1 Elemental" (`Stone Elemental`) **(confirmed)**. **Fire Invocation**: your left-most minion's Attack doubles. **Water Invocation**: your right-most minion gains Divine Shield and Taunt. **Lightning Invocation**: deal 1 damage to 5 different random enemy minions (fewer if fewer) **(confirmed)**. Once per turn. |
| `Flobbidinous Floop` | *Glorious Gloop* (0 Gold, active, a friendly minion; Duos only): Start of Combat: transform it into your teammate's highest-Tier minion. | Using it marks the minion. Step 1b: the marked minion, if it is on the board, transforms into a copy of the minion with the highest Tier in the teammate's warband as it entered the combat (`duos.md` §2; a tie at random, one draw), as `Sandy` copies (`duos_tavern.md` §8.2): its current stats, keywords and Golden status, the marked minion's place and id, a `UnitTransformed` event. With no teammate minion, nothing happens. |
| `Rokara` | *Glory of Combat* (passive): After a friendly minion kills an enemy, give it +1 Attack permanently. | `hero_enemy_killed` (§7.3): the killer, if it is still on the board, gains +1 Attack, permanently (§7.4). |

---

## 12. Decisions

The rules the user settled are marked **(confirmed)** above: the Duos team's 30 Health plus both heroes' Duos Armor; Solo and Duos starting Armor as in the data; effects per player unless the text says team or teammate; Patchwerk's 60 Health and 0 Armor, his extra 30 going into the Duos team's pool; Hero Powers not being spells; Bru'kan's Element and Teron's and Floop's chosen minion lasting for the next combat only; Deathwing's permanent Attack on the enemy's minions too; Illidan with one minion; Lightning's and Earth's different minions; 2 hero offers per player; Hero Powers resolving first at Start of Combat.

Engine decisions, pending the user's confirmation:

| Decision | Rule | Where |
| :--- | :--- | :--- |
| Hero offers | No rerolls, distinct across the lobby, implemented heroes only (§6). | `heroes::draw_offers` |
| Missing Duos Armor | The 7 heroes without tag 3206 use their Solo Armor in Duos (§2.1). | `tools/gen_heroes.py` |
| Where the state lives | `PlayerAuras::hero` (§2.2). | `model.rs` |
| Y'Shaarj | The summon and the card are independent (a full board still gets the card); arms stack and wait for the next combat fought. | `heroes/yshaarj.rs` |
| Tamsin's ties and stats | The left-most lowest-Attack minion; its Attack and `max_health`. | `heroes/tamsin_roame.rs` |
| Deathwing's order | The player's minions, then the enemy's. | `heroes/deathwing.rs` |
