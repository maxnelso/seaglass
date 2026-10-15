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
| `armed` | A next-combat effect the player paid for (Y'Shaarj's Embrace Your Rage). It lasts until the player's next combat, where the power uses it up. |
| `stored` | Cards the power keeps (Teron's exact copy waiting for room in combat). |

All are generic: the engine never reads `counter`, `stacks`, `armed` or `stored`; the power's own hooks do.

### 3.3 Targets

An active power's target domain is its hooks' `activate_target` (the `ActivateTargetKind` of minion Activates, `tavern.md` §7): `None` (untargeted), `BoardAny` (a friendly minion: Teron, Floop), `ShopCard` (a card in the Tavern, minion or spell), `HandCard`. `HeroPower { target }` names the index in that zone. Its `hero_power_target` hook narrows the domain (a non-Golden minion for Reno, a minion in the Tavern for Xyrella). A power that needs a choice among options (Bru'kan's Elements) is untargeted: its `hero_power` hook offers a Choose One (`discover_pending`), answered with `ChooseDiscover` like any Choose One (`tavern.md` §5.2).

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
| `hero_power_usable` | Extra conditions for the action (unlock turns, once per game): `(state) -> bool`. | Once per game (Reno, Kragg, Zerek), room in the hand. |
| `hero_power_target` | Which targets in the domain the action can take (§3.3): `(state, target) -> bool`. | Reno's non-Golden minion, George's minion without Divine Shield. |
| `hero_turn_reset` | Start of turn, after `uses_left` is reset (§3.4): `(hero)`. | Bru'kan's Element reverting, twice-per-turn powers. |
| `hero_chosen` | The player gets this hero (§2.3): `(state, rng)`. | Start-of-game text. |
| `hero_gold_spent` | Gold was spent: `(state, amount)`. | Wax Warband's improvement. |
| `hero_start_of_combat` | Start of Combat (§7): `(ctx)`, a `HeroCombatCtx` (the player's side as a `BoardCtx`, the enemy board, damage to enemy minions). | Al'Akir, Deathwing, Illidan, Y'Shaarj, Teron, Tamsin, Wagtoggle, Bru'kan, Floop. |
| `hero_combat_space` | Room may have opened on the board in combat (after Start of Combat summons and each round of deaths, like `player_combat_space`): `(ctx)`. | Teron's resummon. |
| `hero_enemy_killed` | A friendly minion killed an enemy minion in combat: `(ctx, killer_id, victim)`. The killer may have died in the same exchange: it is then in `ctx.dead` (`BoardCtx::dead`, the side's minions that died this combat) or still on the board with no Health left. | Rokara, Arch-Villain Rafaam. |
| `hero_friendly_death` | A friendly minion died in combat, once its side's round of deaths resolved (summons go to the right end): `(ctx, dying)`. | Onyxia's Avenge. |
| `hero_sat_out` | Duos: the player's warband never stepped in, once the combat is over: `(auras, hero_tier, rng) -> card`; a card it returns goes to the warband's hand if it has room (10 cards). | Y'Shaarj. |
| `hero_combat_summon` | A friendly minion is being summoned in combat (every summon on the player's side: Deathrattles, Reborn copies, hand copies, the Deity): `(hero, summoned)`. | Greybough. |
| `hero_turn_start` | Start of turn, once the shop is filled (before the Spellcraft spells): `(state, pool, rng)`. | Nozdormu. |
| `hero_after_buy` | After a card is bought, once its own text and the `after_buy` observers resolved: `(state, bought, pool, rng)`. | Cap'n Hoggarr, Kael'thas. |
| `hero_after_sell` | After a minion is sold, once its sell triggers resolved: `(state, sold, pool, rng)`. | Gallywix, Ozumat, Flurgl. |
| `hero_after_play` | After a minion is played or Magnetized, once the `after_friendly_play` observers resolved: `(state, played, pool, rng)`. | Chenvaala. |
| `hero_after_upgrade` | After the Tavern is upgraded: `(state, pool, rng)`. | Forest Warden Omu. |

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
5. `target` fits the power's target domain (§3.3): `None` for an untargeted power, a valid index otherwise, which its `hero_power_target` hook (if any) allows.

Effect, in order: `uses_left -= 1`; the `hero_power` hook resolves; then the cost it had when used is spent (`spend_gold`, §3.5). As for a `Buy`, whose card reaches the hand before its Gold is paid, what the power gives arrives first: a card a Gold-spent effect gives as the cost is paid (`Enterprising Escapee`'s Lockbox) meets the hand the power filled, and is lost if it is full **(confirmed**, by analogy with `Buy`**)**. Then the action ends like any other (Triples, aura sync: `tavern.md` §6).

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

The player's `HeroState` travels in their `PlayerAuras`: the Solo `GameState`'s `auras_a` / `auras_b`, a Duos `Warband`'s `auras`. Its combat hooks act for that side, and what they change in the state (Y'Shaarj's `armed`, Teron's `stored`) comes back in the result's auras. A Duos warband that never fought comes back unchanged, except for its Hero Power's `hero_sat_out` hook (Y'Shaarj's card, §11.1), which runs once the combat is over.

### 7.2 Where Hero Powers Fall in Start of Combat

`combat.md` §5.1.1 gains a step **1b**, after the order is decided (step 1): Hero Powers resolve before everything else, before the player effects (`Upper Hand`, ...), `Flighty Scout`'s copies, `Boon of Beetles` and the minions' Start of Combat, the first side's Hero Power first, then the other's **(confirmed)**:

1. **Order** (unchanged). A side's Hero Power with a `hero_start_of_combat` hook counts as a Start of Combat effect for the order coin flip (§4.4).
1b. **Hero Powers**, for the first side, then the other: the side's `hero_start_of_combat` hook. Then, in this order: the damage it deals to enemy minions (one `apply_damage` each, with their damage-taken triggers), the minions it destroys (`BoardCtx::destroy`: each is set to 0 Health and a `Death` is emitted), the deaths (`combat.md` §5.1.1 step 4: the other side first), and its immediate attacks (Illidan's Wingmen), as a minion's `SocAction::Attack` would.
2. **Player effects** (unchanged), then hand copies and open-slot effects, for which the Hero Powers' `hero_combat_space` hooks run with the player effects' `player_combat_space` (Hero Power first).
3. **Minion triggers** (unchanged).

In Duos, a warband stepping in resolves step 1b for its own Hero Power, then its other steps (`duos.md` §3.2).

### 7.3 Kills

When a minion dies in combat and its `killed_by` is an enemy minion (it died from that minion's damage), the killer's player's `hero_enemy_killed` hook fires with the killer's id and the dead minion, whether the killer is still alive or died in the same exchange **(confirmed)**, after the dead minion's side has resolved that round of deaths' Deathrattles (`combat.md` §5.3.1 step 9), before the killer's side's deaths. A minion destroyed by an effect (Venomous counts as the attacker's damage) is killed by whoever dealt the damage.

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

A Duos-only hero's header says `— Duos Armor 15.` instead. The `heroes` tests check that every hero file's header matches the table (name, card ID, Armor, Hero Power card ID, cost and text), that its `ID`/`POWER` constants are the table's dbfIds, and that every hero file has a scenario file `tests/scenarios/heroes/<slug>.yaml` starting with `hero: <Hero Name>`.

---

## 10. Replay

The Power.log validator (`replay/src/{reconstruct,validate,diagnose}.rs`) maps each player's hero, Hero Power and Armor from the log into the reconstruction: the hero's card ID (or skin) to its `HeroId`, the Hero Power's card ID (any form: an Element, another hero's power on Master Nguyen) to its engine `CardId`, and the power's combat state from the log: Teron's and Floop's marked minion from their enchantment on it (`BG25_HERO_103pe`, `BGDUO_HERO_101pe2`), Y'Shaarj's `armed` from the power's `BACON_HERO_POWER_ACTIVATED` at the start of combat, Vanndar's and Drek'Thar's from their `START_OF_COMBAT` (set once they are unlocked). `replay_cli validate` then simulates the implemented combat Hero Powers instead of reporting them as gaps.

---

## 11. The Hero Powers

Golden doesn't apply to Hero Powers. "Your Tier" is the player's Tavern Tier (`hero_tier` in combat).

### 11.1 Combat Hero Powers

| Hero | Hero Power | How the engine reads it |
| :--- | :--- | :--- |
| `Al'Akir` | *Swatting Insects* (passive): Start of Combat: give your left-most minion Windfury, Divine Shield, and Taunt. | Step 1b. Nothing without a minion. |
| `Deathwing` | *ALL Will Burn!* (passive): Start of Combat: give ALL minions +2 Attack permanently. | Step 1b: every minion on both boards, the player's left to right, then the enemy's; permanent (§7.4) on both sides **(confirmed)**. Only the minions on the boards then: later summons don't get it. |
| `Illidan Stormrage` | *Wingmen* (passive): Start of Combat: your left- and right-most minions gain +2/+1 and attack immediately. | Step 1b: the left-most gains +2/+1, then the right-most; then the left-most attacks, then the right-most (immediate attacks, `combat.md` §5.4: they don't move the pointer). With one minion, it gains +2/+1 once and attacks once **(confirmed)**. |
| `Y'Shaarj` | *Embrace Your Rage* (2 Gold, active): Start of Combat: summon and get a minion of your Tier. | Using it arms the next combat only (`armed = 1`): using it again before that combat doesn't stack **(confirmed)**. Step 1b: a random minion of exactly your Tier from the game's catalog (`cards::game_templates`; Duos-exclusive minions in Duos) is summoned at the right end of the board if there is room, and a plain copy goes to your hand under the usual full-hand rule; a full board still gets the card **(confirmed)**. A Duos warband that never steps in still gets the card, once the combat is over (`hero_sat_out`), with no summon **(confirmed)**. One `rng.below(k)`. That combat uses the arm up either way. Once per turn. |
| `Teron Gorefiend` | *Rapid Reanimation* (0 Gold, active, a friendly minion): Start of Combat: destroy it. When you have space, resummon an exact copy. | Using it marks the minion (§3.3). Step 1b: the marked minion, if it is on the board, is destroyed (its Deathrattles and Reborn resolve) and an exact copy of it as it was (stats, keywords, state; full Health) is stored; the copy is summoned at the right end as soon as there is room (`hero_combat_space`: right after the destroy's deaths, or after a later round of deaths). |
| `Tamsin Roame` | *Fragrant Phylactery* (passive): Start of Combat: give your lowest-Attack minion "Deathrattle: give your other minions this minion's stats." | Step 1b: one of the minions with the lowest Attack, at random if they tie (one `rng.below(k)`), gains the extra Deathrattle **(confirmed)**. When it dies, the player's other minions on the board gain its Attack and its Health (`max_health`, the Health it had before damage) **(engine decision)**. |
| `Queen Wagtoggle` | *Wax Warband* (passive): Start of Combat: give a friendly minion of each type +1/+1. (Improves after you spend 10 Gold!) | Step 1b: a friendly minion of each type (`tavern.md` §7.8: the best assignment, as `Misplaced Tea Set`) gains `+N/+N`, `N = 1 + stacks`. Every 10 Gold the player spends (`hero_gold_spent`, `counter`) adds a stack. |
| `Bru'kan` | *Embrace the Elements* (0 Gold, active): Choose an Element. Start of Combat: call upon that Element. | Using it offers a Choose One of the four Elements; the chosen one becomes the Hero Power (`card_id`) for the next combat only, and reverts at the start of the next turn **(confirmed)**. **Earth Invocation**: 4 different random friendly minions (fewer if fewer) gain "Deathrattle: summon a 1/1 Elemental" (`Stone Elemental`) **(confirmed)**. **Fire Invocation**: your left-most minion's Attack doubles. **Water Invocation**: your right-most minion gains Divine Shield and Taunt. **Lightning Invocation**: deal 1 damage to 5 different random enemy minions (fewer if fewer) **(confirmed)**. Once per turn. |
| `Flobbidinous Floop` | *Glorious Gloop* (0 Gold, active, a friendly minion; Duos only): Start of Combat: transform it into your teammate's highest-Tier minion. | Using it marks the minion. Step 1b: the marked minion, if it is on the board, transforms into a copy of the minion with the highest Tier in the teammate's warband as it entered the combat (`duos.md` §2; a tie at random, one draw), as `Sandy` copies (`duos_tavern.md` §8.2): its current stats, keywords and Golden status, the marked minion's place and id, a `UnitTransformed` event. With no teammate minion, nothing happens. |
| `Vanndar Stormpike` | *Stormpike Strength* (passive): When you have space in combat, summon a copy of your highest-Health minion. (Unlocks on Turn 7.) | It counts the turns (`counter`); from the 7th on, each combat is armed. Once per combat **(confirmed)**, as soon as there is room (`hero_combat_space`: at Start of Combat or after a round of deaths), a copy of the friendly minion with the most Health (a tie at random, one draw) is summoned at the right end: its current stats, keywords and state. |
| `Drek'Thar` | *Frostwolf Fervor* (passive): When you have space in combat, summon a copy of your highest-Attack minion. (Unlocks on Turn 7.) | As Vanndar's, with the most Attack. |
| `Greybough` | *Sprout It Out!* (passive): Give +1/+2 and Taunt to minions you summon during combat. | `hero_combat_summon`: every minion summoned on the player's side in combat (Deathrattle and Rally summons, Reborn copies, hand copies, the awakened Deity, Hero Power summons) **(confirmed)** gains +1/+2 and Taunt. |
| `Ozumat` | *Tentacular* (passive): When you have space in combat, summon a 2/2 Tentacle with Taunt. (Gains +1/+1 after you sell a minion!) | Once per combat **(confirmed)**, as soon as there is room (`hero_combat_space`), an `Ozumat's Tentacle` (a Beast with Taunt) of `2 + stacks` at the right end; each minion sold adds a stack. |
| `Onyxia` | *Broodmother* (passive): Avenge (4): Summon a 1/1 Whelp that attacks immediately. Improve this by +1/+1. | `hero_friendly_death`: every 4th friendly minion that dies in combat (the count starts again each combat **(confirmed)**) summons an `Onyxian Whelp` (a Dragon) of `1 + stacks` at the right end, which attacks immediately; each Whelp adds a stack for the rest of the game **(confirmed)**. |
| `Arch-Villain Rafaam` | *I'll Take That!* (1 Gold, active): Next combat, get a plain copy of the first minion you kill. | Using it arms the next combat; there the first enemy minion a friendly minion kills gives a plain copy of it (`cards::plain_instance`) to the hand. An unused arm is gone at the start of the next turn. Once per turn. |
| `Rokara` | *Glory of Combat* (passive): After a friendly minion kills an enemy, give it +1 Attack permanently. | `hero_enemy_killed` (§7.3): the killer gains +1 Attack, permanently (§7.4), even if it died in the same exchange: its Tavern copy keeps it **(confirmed)**. Each kill counts. |
| `Ini Stormcoil` | *MechGyver* (passive): After 9 friendly minions die, get a random Mech. | `hero_friendly_death`: every 9th friendly minion that dies in combat (counted across combats, `counter`) gives the player a random Mech of their Tier or lower from the game's catalog **(engine decision)** in their combat hand (it comes back with the hand, under the usual full-hand rule). One `rng.below(k)`. |

### 11.2 Tavern Hero Powers

| Hero | Hero Power | How the engine reads it |
| :--- | :--- | :--- |
| `Patchwerk` | *All Patched Up*: Start the game with 30 extra Health. | No hooks: his data (60 Health, 0 Armor) does it (§2.3). |
| `Cap'n Hoggarr` | *I'm the Cap'n Now*: After you buy a Pirate, gain 1 Gold. | `hero_after_buy`: a bought minion with the Pirate type gives 1 Gold. |
| `Forest Warden Omu` | *Everbloom*: After you upgrade the Tavern, gain 2 Gold. | `hero_after_upgrade`: 2 Gold. |
| `Trade Prince Gallywix` | *Smart Savings*: After you sell a minion, gain 1 Gold next turn. | Passive (its data cost is never paid). `hero_after_sell`: `bonus_gold_next_turn += 1`. |
| `Nozdormu` | *Clairvoyance*: At the start of your turn, gain a free Refresh. | `hero_turn_start`: `free_refreshes += 1`; an unused one carries over like any free Refresh **(confirmed)**. |
| `Kael'thas Sunstrider` | *Verdant Spheres*: After you buy 3 minions, get a Tavern Coin. | `hero_after_buy`: every 3rd minion bought (counted across turns, `counter`, **(confirmed)**) gives a `Tavern Coin`. |
| `Chenvaala` | *Avalanche*: After you play 3 Elementals, reduce the Cost of upgrading the Tavern by (3). | `hero_after_play`: every 3rd Elemental played from hand (counted across turns **(confirmed)**) makes the upgrade 3 cheaper, never below 0. |
| `Fungalmancer Flurgl` | *Gone Fishing*: After you sell 5 minions, get a random Murloc. | `hero_after_sell`: every 5th minion sold (counted across turns **(confirmed)**) gives a random Murloc of the player's Tier or lower from the game's catalog **(engine decision)**. |
| `Death Speaker Blackthorn` | *Bloodbound* (1 Gold): Get 2 Blood Gems. (Twice per turn.) | Two uses per turn: the first use gives a use back (`counter` counts the turn's uses; `hero_turn_reset` clears it). Each use adds 2 `Blood Gem`s to the hand; it needs room for one **(engine decision)**, and a Gem the hand has no room for is lost. |
| `George the Fallen` | *Boon of Light* (1 Gold, a friendly minion): Give a minion Divine Shield. | The minion gains Divine Shield. A minion that has it is not a target **(engine decision)**. |
| `Doctor Holli'dae` | *Blessing of the Nine Frogs* (1 Gold): Get a random Tavern spell. | A spell drawn as the shop draws one (`spells::draw_shop_spell`: of the player's Tier or lower, Duos spells too in Duos) **(engine decision)** goes to the hand; it needs room in the hand **(engine decision)**. One draw. |
| `Forest Lord Cenarius` | *Wisdom of Ancients* (3 Gold): Increase your maximum Gold by 1. | As `Strike Oil`: `max_gold` and `base_max_gold_bonus` go up by 1 (above 10 too); the Gold itself doesn't. |
| `Edwin VanCleef` | *Sharpen Blades* (1 Gold, a friendly minion): Give a minion +2/+2. Improves after you buy 4 cards. | The minion gains `+(2 + stacks)/+(2 + stacks)`. Every 4th card bought (minions and spells, counted across turns: `hero_after_buy`, `counter`) adds a stack: each improvement is +1/+1 **(engine decision)**. |
| `Reno Jackson` | *Gonna Be Rich!* (0 Gold, a friendly non-Golden minion): Once per game, make a friendly minion Golden. | `Unit::make_golden` (as `Eyes of the Earth Mother`), then the auras resync. Once per game (`stacks` = 1 once used). |
| `Skycap'n Kragg` | *Piggy Bank* (0 Gold): Gain 2 Gold. Increases by 1 each turn. (Once per game.) | Gains `2 + (turns started - 1)` Gold (`counter` counts the turns, `hero_turn_reset`; 2 before the first turn). Once per game. |
| `Lich Baz'hial` | *Graveyard Shift* (2 Gold, a card in the Tavern): Steal a card from the Tavern. Take 2 damage. | The card leaves the shop for the hand (not a buy: no buy triggers), then the hero takes 2 damage (`deal_hero_damage`: Armor first). It needs room in the hand, and can't be used if the damage would bring Health and Armor to 0 **(engine decision)**. |
| `Xyrella` | *See the Light* (2 Gold, a minion in the Tavern): Choose a minion in the Tavern. Set its stats to 2 and add it to your hand. | The minion's Attack and Health (and their maximums) become 2, net of the auras on it (`aura_applied` clears, as for `Robust Evolution`), and it goes to the hand (not a buy), where the auras it has there apply again: a +0/+3 Volumizer bonus makes it 2/5 **(engine decision)**. Passed to a teammate without the aura, it is 2/2. It needs room in the hand. |
| `Pyramad` | *Brick by Brick* (2 Gold): Steal a random minion from the Tavern. Double its Health. | A random minion in the shop (one `rng.below(k)`) gets twice its Health and goes to the hand (not a buy). It needs a minion in the shop and room in the hand. |
| `Zerek, Master Cloner` | *Cloning Gallery* (0 Gold, a friendly minion): Once per game, summon an exact copy of a friendly minion. | An exact copy (stats, keywords, state; not its mark) is summoned to its right (`on_tavern_summon`: summon observers see it); it can make a triple. It needs room on the board. Once per game. |
| `Snake Eyes` | *Lucky Roll* (1 Gold): Roll a 6-sided die. Gain that much Gold. (Cannot be used again for that many turns!) | Gains `1 + rng.below(6)` Gold. The roll is its cooldown (`counter`): at each turn start while it is above 0, the power has no use and the cooldown goes down by 1, so a 3 rolled on turn 5 makes it usable again on turn 9. |
| `The Lich King` | *Reborn Rites* (0 Gold, a friendly minion without Reborn): Give a minion Reborn until next turn. | The minion gains Reborn, temporary (`Unit::temp_reborn`, like `temp_divine_shield`): it ends at the start of the next turn, unless a lasting Reborn was given since. A minion that has Reborn is not a target **(engine decision)**. |
| `Overlord Saurfang` | *For the Horde!* (passive): Minions in the Tavern have +1/+1. Improves after you buy 3 minions. | When the player gets the hero (`hero_chosen`), the Tavern's minions get +1/+1 (`tavern_all_atk` / `tavern_all_hp`, the shop's minions too). Every 3rd minion bought (counted across turns) adds +1/+1 more. |
| `Rakanishu` | *Tavern Lighting* (passive): Your Tavern spells give an extra +1/+1. At the start of every 3 turns, improve this. | When the player gets the hero (`hero_chosen`), spell power +1/+1 (`spell_bonus_atk` / `spell_bonus_hp`). Every 3rd turn start (`hero_turn_start`, `counter`) adds +1/+1 more. |
| `Dinotamer Brann` | *Battle Brand*: After you buy 4 Battlecry minions, get a Brann Bronzebeard. (Once per game.) | `hero_after_buy`: every bought minion with a Battlecry counts (across turns); the 4th gives a `Brann Bronzebeard` (a plain one from the catalog, not the pool: **engine decision**), once per game (`stacks`). |
| `Kurtrus Ashfallen` | *Glaive Ricochet*: Once per turn, after you buy 3 minions, get a plain copy of one of them. | `hero_after_buy`: the minions bought this turn count (the count starts again each turn: **engine decision**) and are remembered (`stored`); the 3rd gives a plain copy (`cards::plain_instance`) of one of the 3, at random (one `rng.below(3)`), once per turn (`armed`). |
| `Patches the Pirate` | *Pirate Parrrrty!* (3 Gold): Get a Pirate. After you buy a Pirate, your next Hero Power costs (1) less. | A random Pirate of the player's Tier or lower from the pool **(engine decision)** goes to the hand; it needs room in the hand. Each Pirate bought makes the power 1 cheaper (never below 0); using it puts the cost back to 3. |
| `Elise Starseeker` | *Lead Explorer* (1 Gold): Discover a minion from your Tier. Costs (1) more after each use. | Discovers among 3 minions of exactly the player's Tier from the pool (`CardPool::draw_discover_options`, as a Triple Reward); it needs room in the hand. Each use makes it 1 more expensive for the rest of the game. |
| `Alexstrasza` | *Queen of Dragons* (1 Gold): Discover a Dragon. (Unlocks at Tier 4.) | From Tavern Tier 4 on: Discovers among 3 Dragons of the player's Tier or lower from the pool; it needs room in the hand. |
| `Maiev Shadowsong` | *Imprison* (1 Gold, a card in the Tavern): Choose a card in the Tavern to lock in your hand. After 2 turns, unlock it. | The card leaves the shop for the hand (not a buy) locked for 2 turns (`locked_turns = 2`: it can't be played or sold until the start of the second turn after). It needs room in the hand. |
| `Captain Eudora` | *Buried Treasure* (1 Gold): Dig for a Golden minion! (4 Digs left.) | Each use is a Dig (`counter`); the 4th gives a Golden minion of exactly the player's Tier from the pool (`Unit::make_golden`), and the count starts again **(engine decision)**. The 4th Dig needs room in the hand. |
| `Jandice Barov` | *Swap, Lock, & Shop It* (0 Gold, a friendly non-Golden minion): Swap a friendly non-Golden minion with a random one in the Tavern. | A random minion in the shop (one `rng.below(k)`) takes the friendly minion's board slot, and the friendly minion takes its shop slot, each as it is (not a buy, a sale or a summon). It needs a minion in the shop. |
| `Mutanus the Devourer` | *Devour* (0 Gold, a friendly minion): Sell a friendly minion. Spit its stats onto another. | The minion is sold (1 Gold, its sell triggers and the sell observers, as the `Sell` action), then a random other friendly minion (one `rng.below(k)`) gains its Attack and Health. It needs another friendly minion **(engine decision)**. |
| `Heistbaron Togwaggle` | *The Perfect Crime* (11 Gold): Steal all cards from the Tavern. Each turn, your next Hero Power costs (1) less. | Every shop card goes to the hand while it has room, left to right (not a buy); the rest stay. At the start of each turn, the first included, it gets 1 cheaper (never below 0) **(engine decision)**; using it puts the cost back to 11. It needs a card in the shop and room in the hand. |

---

## 12. Decisions

The rules the user settled are marked **(confirmed)** above: the Duos team's 30 Health plus both heroes' Duos Armor; Solo and Duos starting Armor as in the data; effects per player unless the text says team or teammate; Patchwerk's 60 Health and 0 Armor, his extra 30 going into the Duos team's pool; Hero Powers not being spells; Bru'kan's Element and Teron's and Floop's chosen minion lasting for the next combat only; Deathwing's permanent Attack on the enemy's minions too; Illidan with one minion; Lightning's and Earth's different minions; 2 hero offers per player; Hero Powers resolving first at Start of Combat; Y'Shaarj's arm (no stacking, the card regardless of the summon or of fighting); Tamsin's random tie; Rokara's dead killers gaining the Attack; Vanndar, Drek'Thar and Ozumat once per combat; Greybough's Reborn copies and Deities; Onyxia's count per combat and her lasting improvement; Nozdormu's carry-over; the counters across turns.

Engine decisions, pending the user's confirmation:

| Decision | Rule | Where |
| :--- | :--- | :--- |
| Hero offers | No rerolls, distinct across the lobby, implemented heroes only (§6). | `heroes::draw_offers` |
| Missing Duos Armor | The 7 heroes without tag 3206 use their Solo Armor in Duos (§2.1). | `tools/gen_heroes.py` |
| Where the state lives | `PlayerAuras::hero` (§2.2). | `model.rs` |
| Tamsin's stats | The dying minion's Attack and `max_health`. | `heroes/tamsin_roame.rs` |
| Deathwing's order | The player's minions, then the enemy's. | `heroes/deathwing.rs` |
| Vanndar and Drek'Thar | A tie at random. | `heroes::summon_copy_of_best` |
| Flurgl's Murloc | Of the player's Tier or lower. | `heroes/fungalmancer_flurgl.rs` |
| Powers that give cards | Blackthorn, Holli'dae, Baz'hial, Xyrella, Pyramad, Maiev, Patches, Elise, Alexstrasza and Togwaggle need room in the hand; a Blood Gem that doesn't fit is lost. | their files |
| Targets | George's and The Lich King's minion doesn't already have the keyword. | their files |
| Holli'dae's spell | Drawn as the shop draws a spell: the player's Tier or lower. | `heroes/doctor_hollidae.rs` |
| Edwin's improvement | +1/+1 per 4 cards bought, spells included. | `heroes/edwin_vancleef.rs` |
| Baz'hial's damage | Not usable if it would bring Health and Armor to 0. | `heroes/lich_bazhial.rs` |
| Kragg's Gold | 2 on the first turn, +1 per turn started since. | `heroes/skycapn_kragg.rs` |
| Xyrella's auras | The minion set to 2/2 gets the auras it has in the hand again (2/5 with a +0/+3 Volumizer bonus), rather than staying 2/2. | `heroes/xyrella.rs` |
| Rakanishu's improvement | Every 3rd turn start, counting the first. | `heroes/rakanishu.rs` |
| Cards from the catalog or the pool | Ini's Mech and Brann's Brann come from the catalog (not the pool); Patches' Pirate (Tier or lower), Elise's, Alexstrasza's and Eudora's minions from the pool. | their files |
| Kurtrus' count | It starts again each turn. | `heroes/kurtrus_ashfallen.rs` |
| Eudora's Digs | The 4th gives a Golden minion of the player's Tier; the count starts again. | `heroes/captain_eudora.rs` |
| Mutanus' target | Only with another friendly minion. | `heroes/mutanus_the_devourer.rs` |
| Togwaggle's discount | From the first turn's start on. | `heroes/heistbaron_togwaggle.rs` |
