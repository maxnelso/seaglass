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
- `Queen Azshara`: her Naga Conquest Discovers Naga, and Naga aren't in the game **(confirmed)**; she is in the table but not implemented, and not offered (§6).
- Buddies (companions), Trinkets, Quests, Darkmoon Prizes, Anomalies, Dark Gifts, Timewarp, Spellcraft outside Duos, Secrets, custom Undead creations, and StarCraft multi-piece boards/upgrade spells: the heroes that need them (E.T.C.; Buttons, Marin; Sire Denathrius; Tickatus; Nightmare Lord Xavius, Tras'tath; Morchie, Murozond; Lady Vashj; The Great Akazamzarak; Professor Putricide; Jim Raynor, Kerrigan, Queen of Blades, Artanis) **(confirmed)** are in the table but not implemented, and not offered (§6).

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
| `second_power` | A second Hero Power (`Option<HeroPowerState>`, `None` unless Genn, Worgen King's *King of Duality* gives one, §11.2). Every Hero Power hook runs for `power`, then for `second_power` if present, and `HeroPower2` uses it (§5). |

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
| `picked` | A minion on the board or in the shop the power picked and will act on again (Vol'jin's first pick), by the id `heroes::identify_minion` gave it (Tavern cards have no distinct ids otherwise). It drops once no minion on the board or in the shop has that id. |

All are generic: the engine never reads `counter`, `stacks`, `armed` or `stored`; the power's own hooks do. The observation reads `picked`, as the picked minion's current target (§8).

### 3.3 Targets

An active power's target domain is its hooks' `activate_target` (the `ActivateTargetKind` of minion Activates, `tavern.md` §7): `None` (untargeted), `BoardAny` (a friendly minion: Teron, Floop), `ShopCard` (a card in the Tavern, minion or spell), `HandCard`, `BoardOrShopMinion` (`0..7` indexes a friendly minion at `board[target]`, `7..14` indexes a minion at `shop[target - 7]`: Vol'jin, Cookie the Cook), or `PairSide` (`0` or `1`, the first or second player of the lobby's featured combat pair: Lord Barov). `HeroPower { target }` names the index in that zone. Its `hero_power_target` hook narrows the domain (a non-Golden minion for Reno, a minion in the Tavern for Xyrella). A power that needs a choice among options (Bru'kan's Elements) is untargeted: its `hero_power` hook offers a Choose One (`discover_pending`), answered with `ChooseDiscover` like any Choose One (`tavern.md` §5.2).

A minion a power chose to act on later (Teron's Rapid Reanimation, Floop's Glorious Gloop) is **marked** (`Unit::marked`). Choosing again moves the mark. The mark lasts until the player's next combat, where the power acts on the marked minion, and it clears at the start of the next turn **(confirmed)**. It is part of the minion: it leaves with it (sold, Passed) and is not copied by a triple.

### 3.4 Uses per Turn

At the start of each turn (`tavern.md` §4 step 1), the hero's own Hero Power's `hero_base_turn_reset` hook runs first, whatever power the hero has now (§4). Then each player's Hero Power gets `uses_left = 1` if it is active, `0` if it is passive; then its `hero_turn_reset` hook may change that (twice per turn, unlock turns) and reset turn-long state (Bru'kan's Element reverts, §11). A player who never started a turn has `uses_left = 1` for an active power from `set_hero`, so a power is usable on turn 1.

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
| `hero_base_turn_reset` | Start of turn, before the uses are reset (§3.4), for the hero's own Hero Power (the table's `power`), whatever Hero Power the hero has now: `(state, pool)`. | Master Nguyen taking *Power of the Storm* back. |
| `hero_power_removed` | This Hero Power is replaced (`heroes::replace_power`: Master Nguyen's turn start, Sir Finley's or Genn's new power): `(state, its state, pool)`, to clean up what it keeps and undo what it gave for as long as it was the Hero Power. | Tavish's loaded minion going back to the pool; Millhouse's, Saurfang's and Rakanishu's bonuses coming off; Barov's open wager settled (§11.3). |
| `hero_chosen` | The player gets this hero (§2.3): `(state)`. | Start-of-game text. |
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
| `hero_friendly_attack` | A friendly minion attacks (in combat, or in the Tavern), once its friendly-attack observers resolved: `(ctx, attacker_id)`. | Loh, the Living Legend. |
| `hero_after_combat` | The combat's results are back in the Tavern (`TavernState::apply_combat_return`, Solo and Duos): `(state, dead)`, the player's minions that died in it. | Sylvanas Windrunner. |
| `hero_option_chosen` | The player chose an option of a Discover the Hero Power keeps (`Unit::to_hero_power`): `(state, chosen, unchosen, pool, rng)`. Without it, the chosen card is kept (`stored`) and the others are discarded. | Galakrond, Sir Finley, Master Nguyen, Genn. |
| `hero_end_turn` | The player ends their turn (`EndTurn`), after the cards' end-of-turn triggers: `(state, pool, rng)`. | Rock Master Voone. |
| `hero_tavern_rules` | Whenever a price or Tavern rule is read: `(rules)`, the `TavernRules` to change (the minion and Refresh prices, the Upgrade's extra, fewer shop minions, the copies a Triple takes, whether a Golden gives a `Triple Reward` card; `tavern.md` §3.3). | Sindragosa, Millhouse, Clocksworth. |
| `hero_before_team_turn` | Duos: the team's turn is about to start, before either player's start of turn (`DuosTavern::start_turn`): `(state, teammate's state)`. | The Nameless One. |
| `hero_skips_turn` | Whether the player skips the current turn: `(state) -> bool`. While it is `true` only `EndTurn` and `ChooseDiscover` are legal (`tavern.md` §6). | A. F. Kay, Faelin. |
| `hero_after_refresh` | The Tavern was Refreshed (the `Refresh` action, a Refresh by a card or a Hero Power, the start of turn's new shop; not a frozen shop kept), before the cards' after-Refresh effects: `(state, pool, rng)`. | Ysera, Enhance-o Mechano. |
| `hero_friendly_damage_dealt` | A friendly minion dealt damage (in combat or in the Tavern), after the cards' `on_damage_dealt` observers: `(ctx, dealer_id, amount)`. | Lord Jaraxxus. |

Extra Deathrattles a Hero Power gives a minion in combat (Tamsin's Fragrant Phylactery, Bru'kan's Earth Invocation) are cards too: `Unit::extra_deathrattles` lists their card ids, and their `deathrattle` hooks resolve after the minion's own Deathrattle, each counting as a Deathrattle triggered and repeated by `Titus Rivendare` like it.

More hooks are added with the Tavern Hero Powers that need them (§11).

---

## 5. The `HeroPower` Action

`TavernAction::HeroPower { target: Option<usize> }` uses the player's Hero Power (`hero.power`). It comes after `Pass` in `TavernAction`, so the earlier variants keep their places; `ChooseHero` (§6) comes after it, and `TavernAction::HeroPower2 { target: Option<usize> }` (which uses the player's second Hero Power, `hero.second_power`, under the same rules) comes after `ChooseHero`.

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
- `heroes::draw_offers(players, per_player, duos, rng)` draws them: for each player in turn, `per_player` distinct heroes uniformly at random among the offerable heroes no earlier player was offered (one `rng.below(k)` per hero). In Duos (`duos == true`), if a player's teammate (`p ^ 1`) was already offered `The Nameless One`, `The Nameless One` is excluded from that player's pool so a team can never get two copies of `The Nameless One` **(confirmed)**. A match of several Taverns is the caller's job, as for combat pairings.
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
- `TavernAction.hero_power(target=None)`, `TavernAction.hero_power_2(target=None)` and `TavernAction.choose_hero(option_index)`; their `kind`s are `"hero_power"`, `"hero_power_2"` and `"choose_hero"`.
- The observation (`TavernObservation`, `DuosObservation`) has `hero_id` (0 without a hero), `hero_power_id`, `hero_power_cost`, `hero_power_uses_left`, `hero_power_counter`, `hero_power_stacks`, `hero_power_armed`, `hero_power_usable` (the untargeted or some targeted `HeroPower` is legal), `second_hero_power_id` (0 without a second Hero Power), `second_hero_power_cost`, `second_hero_power_uses_left`, `second_hero_power_counter`, `second_hero_power_stacks`, `second_hero_power_armed`, `second_hero_power_usable`, and `hero_options`. `hero_power_armed` (and `second_hero_power_armed`) is the power's `armed`, or for a power with a `picked` minion that minion's current `BoardOrShopMinion` target + 1 (0 once the pick dropped).
- The RL action table (`rl/seaglass_rl/actions.py`) appends, at the end: `hero_power(None)`, `hero_power(t)` for `t` in `0..10`, then `choose_hero(i)` for `i` in `0..4`, then `hero_power(t)` for `t` in `10..14` (`BoardOrShopMinion` shop slots), then `hero_power_2(None)` and `hero_power_2(t)` for `t` in `0..14`.

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
| `Vanndar Stormpike` | *Stormpike Strength* (passive): When you have space in combat, summon a copy of your highest-Health minion. (Unlocks on Turn 7.) | At each turn start (`hero_turn_start`) it reads the game's turn (`counter`); from turn 7 on, each combat is armed, however long the player has had the Hero Power (Genn's or Nguyen's: **engine decision**). Once per combat **(confirmed)**, as soon as there is room (`hero_combat_space`: at Start of Combat or after a round of deaths), a copy of the friendly minion with the most Health (a tie at random, one draw) is summoned at the right end: its current stats, keywords and state. |
| `Drek'Thar` | *Frostwolf Fervor* (passive): When you have space in combat, summon a copy of your highest-Attack minion. (Unlocks on Turn 7.) | As Vanndar's, with the most Attack. |
| `Greybough` | *Sprout It Out!* (passive): Give +1/+2 and Taunt to minions you summon during combat. | `hero_combat_summon`: every minion summoned on the player's side in combat (Deathrattle and Rally summons, Reborn copies, hand copies, the awakened Deity, Hero Power summons) **(confirmed)** gains +1/+2 and Taunt. |
| `Ozumat` | *Tentacular* (passive): When you have space in combat, summon a 2/2 Tentacle with Taunt. (Gains +1/+1 after you sell a minion!) | Once per combat **(confirmed)**, as soon as there is room (`hero_combat_space`), an `Ozumat's Tentacle` (a Beast with Taunt) of `2 + stacks` at the right end; each minion sold adds a stack. |
| `Onyxia` | *Broodmother* (passive): Avenge (4): Summon a 1/1 Whelp that attacks immediately. Improve this by +1/+1. | `hero_friendly_death`: every 4th friendly minion that dies in combat (the count starts again each combat **(confirmed)**) summons an `Onyxian Whelp` (a Dragon) of `1 + stacks` at the right end, which attacks immediately; each Whelp adds a stack for the rest of the game **(confirmed)**. |
| `Arch-Villain Rafaam` | *I'll Take That!* (1 Gold, active): Next combat, get a plain copy of the first minion you kill. | Using it arms the next combat; there the first enemy minion a friendly minion kills gives a plain copy of it (`cards::plain_instance`) to the hand. An unused arm is gone at the start of the next turn. Once per turn. |
| `Loh, the Living Legend` | *Heroic Inspiration* (passive): After 15 friendly minions attack, get a Triple Reward. | `hero_friendly_attack`: each attack of a friendly minion counts (`counter`, across combats); every 15th gives a `Triple Reward` card (`tavern.md` §5.2) in the combat hand, which comes back with it under the usual full-hand rule. |
| `Rokara` | *Glory of Combat* (passive): After a friendly minion kills an enemy, give it +1 Attack permanently. | `hero_enemy_killed` (§7.3): the killer gains +1 Attack, permanently (§7.4), even if it died in the same exchange: its Tavern copy keeps it **(confirmed)**. Each kill counts. |
| `Ini Stormcoil` | *MechGyver* (passive): After 9 friendly minions die, get a random Mech. | `hero_friendly_death`: every 9th friendly minion that dies in combat (counted across combats, `counter`) gives the player a random Mech of their Tier or lower from the game's catalog **(engine decision)** in their combat hand (it comes back with the hand, under the usual full-hand rule). One `rng.below(k)`. |
| `Tavish Stormpike` | *Lock and Load* (0 Gold, a minion in the Tavern): Remove a minion in the Tavern. When you have space next combat, fire it at a random enemy minion. | Once per turn: the minion leaves the shop and the Hero Power keeps it (`stored`, with its stats; its copy stays out of the pool). Next combat, as soon as the board has room (step 1b, or once a slot opens: `hero_combat_space`), it joins the warband at the right end, a summon (the summon observers trigger), and attacks immediately, as `Eternal Tycoon`'s Knight **(confirmed)**. With no room all combat it never joins and is lost **(confirmed)**. At the next turn start the kept cards' copies go back to the pool, whether they joined or were lost **(confirmed)** (`hero_turn_start`). If *Lock and Load* is replaced before that (Master Nguyen taking his Hero Power back at the next turn start), its `hero_power_removed` hook sends them back then, before the shop is rolled **(engine decision)**. |

### 11.2 Tavern Hero Powers

| Hero | Hero Power | How the engine reads it |
| :--- | :--- | :--- |
| `Patchwerk` | *All Patched Up*: Start the game with 30 extra Health. | His data (60 Health, 0 Armor) does it for him (§2.3). Another hero who gets the Hero Power (Sir Finley's or Genn's Discover, The Nameless One's copy) gains the 30 Health as they get it (`hero_chosen`) **(engine decision)**. |
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
| `Overlord Saurfang` | *For the Horde!* (passive): Minions in the Tavern have +1/+1. Improves after you buy 3 minions. | When the player gets the hero (`hero_chosen`), the Tavern's minions get +1/+1 (`tavern_all_atk` / `tavern_all_hp`, the shop's minions too). Every 3rd minion bought (counted across turns) adds +1/+1 more. Losing the Hero Power (Master Nguyen's next turn, `hero_power_removed`) takes the whole bonus back off the Tavern and its minions **(engine decision)**. |
| `Rakanishu` | *Tavern Lighting* (passive): Your Tavern spells give an extra +1/+1. At the start of every 3 turns, improve this. | When the player gets the hero (`hero_chosen`), spell power +1/+1 (`spell_bonus_atk` / `spell_bonus_hp`). Every 3rd turn start (`hero_turn_start`, `counter`) adds +1/+1 more. Losing the Hero Power (Master Nguyen's next turn, `hero_power_removed`) takes the whole bonus back **(engine decision)**. |
| `Dinotamer Brann` | *Battle Brand*: After you buy 4 Battlecry minions, get a Brann Bronzebeard. (Once per game.) | `hero_after_buy`: every bought minion with a Battlecry counts (across turns); the 4th gives a `Brann Bronzebeard` (a plain one from the catalog, not the pool: **engine decision**), once per game (`stacks`). |
| `Kurtrus Ashfallen` | *Glaive Ricochet*: Once per turn, after you buy 3 minions, get a plain copy of one of them. | `hero_after_buy`: the minions bought this turn count (the count starts again each turn: **engine decision**) and are remembered (`stored`); the 3rd gives a plain copy (`cards::plain_instance`) of one of the 3, at random (one `rng.below(3)`), once per turn (`armed`). |
| `Patches the Pirate` | *Pirate Parrrrty!* (3 Gold): Get a Pirate. After you buy a Pirate, your next Hero Power costs (1) less. | A random Pirate of the player's Tier or lower from the pool **(engine decision)** goes to the hand; it needs room in the hand. Each Pirate bought makes the power 1 cheaper (never below 0); using it puts the cost back to 3. |
| `Elise Starseeker` | *Lead Explorer* (1 Gold): Discover a minion from your Tier. Costs (1) more after each use. | Discovers among 3 minions of exactly the player's Tier from the pool (`CardPool::draw_discover_options`, as a Triple Reward); it needs room in the hand. Each use makes it 1 more expensive for the rest of the game. |
| `Alexstrasza` | *Queen of Dragons* (1 Gold): Discover a Dragon. (Unlocks at Tier 4.) | From Tavern Tier 4 on: Discovers among 3 Dragons of the player's Tier or lower from the pool; it needs room in the hand. |
| `Millificent Manastorm` | *Tinker* (1 Gold): Discover a Magnetic Mech. (Unlocks at Tier 4.) | From Tavern Tier 4 on: Discovers among 3 Magnetic Mechs (a Mech or All minion with Magnetic) of the player's Tier or lower from the pool, as Alexstrasza's; it needs room in the hand. |
| `The Rat King` | *A Tale of Kings* (2 Gold): Discover a minion of a specific minion type. Swaps type each turn. | The type of turn `t` is the `(t - 1) mod 10`-th lobby type, in the order `Aberration, Beast, Demon, Dragon, Elemental, Mech, Murloc, Pirate, Quilboar, Undead` (`LOBBY_TRIBES`; the first before turn 1), set at each turn start (`hero_turn_start`, `counter` = its index; no RNG) **(engine decision)**. Using it Discovers among 3 minions of that type (a dual type or All counts) of the player's Tier or lower from the pool, as Alexstrasza's; it needs room in the hand. The Hero Power keeps its `card_id` (the data's per-type variants, `TB_BaconShop_HP_041a`.., aren't mapped to types). |
| `Sindragosa` | *Stay Frosty* (passive): Minions cost (2). The Tavern offers one fewer minion and Freezes at the end of each turn. | `hero_tavern_rules`: minions cost 2 Gold (`next_minion_discount` still lowers that), the shop holds one minion fewer than its capacity (the spell slot is kept). `hero_end_turn`: the Tavern is Frozen (`is_frozen`), as the player's Freeze, so the next turn start keeps it **(confirmed)**. |
| `Millhouse Manastorm` | *Manastorm* (passive): Minions and Refreshes cost 2 Gold. Upgrading the Tavern costs (1) more. | `hero_tavern_rules`: minions cost 2 Gold and a paid Refresh 2 (free and Health Refreshes are unchanged); each Tier's `upgrade_cost` starts at its base cost + 1 (`TavernState::base_upgrade_price`), and the turn discounts lower it as usual, down to 0 **(confirmed)**. Getting the hero adds the 1 to the current `upgrade_cost` (`hero_chosen`), and losing the Hero Power (Master Nguyen's next turn, `hero_power_removed`) takes it off again **(engine decision)**. |
| `Mister Clocksworth` | *Double Time* (passive): You only need 2 copies to make minions Golden. They give Tavern Coins instead of Triple Rewards. | `hero_tavern_rules`: 2 non-Golden copies form a Triple (an `Elemental of Surprise` counts as one, as usual); the Golden sums both copies' buffs, and selling it returns its 2 copies to the pool (`Unit::golden_copies`). A Golden played from the hand gives a `Tavern Coin` instead of a `Triple Reward` card, whether a Triple or an effect made it **(confirmed)**. |
| `A. F. Kay` | *Procrastinate* (passive): Skip your first two turns, then Discover a minion from Tier 3 and Tier 4. | `hero_skips_turn` on turns 1 and 2: only `EndTurn` is legal (the shop is shown, the Gold is unused). At the start of turn 3 (`hero_turn_start`): two `Triple Reward` cards, locked in at Tier 3 and at Tier 4, each lost with a full hand (`tavern.md` §5.2) **(confirmed)**. |
| `Ambassador Faelin` | *Expedition Plans* (passive): Skip your first turn. Discover minions from Tiers 6, 4, and 2 to get at those Tiers. | `hero_skips_turn` on turn 1. At its start (`hero_turn_start`): three Discovers from the pool, of exactly Tier 6, 4 and 2, in that order, whose picks the Hero Power keeps (`Unit::to_hero_power`, `stored`; the other options go back to the pool). Each kept minion goes to the hand once the player's Tier reaches its Tier, waiting for room **(confirmed)**: at a turn start, after an Upgrade, or after a minion is played **(engine decision)**. |
| `Galewing` | *Dungar's Gryphon* (0 Gold): Choose a new flightpath. Complete it to get a bonus! | Usable while no flightpath is under way: a Choose One of the three flightpaths, which becomes the Hero Power (its card, `BG20_HERO_283p_t1` .. `t3`; `counter` = the turns left). At each turn start one turn passes; at 0 the bonus is given and the power is *Dungar's Gryphon* again, usable that turn: **Westfall** (1 turn) a random 1-Cost Tavern spell, **Ironforge** (2 turns) 2 Gold, **Eastern Plaguelands** (3 turns) a Discover of a minion of the player's Tier from the pool **(confirmed)**. A card needs room in the hand. |
| `Silas Darkmoon` | *Come One, Come All!* (passive): Darkmoon Tickets are in the Tavern! Get 3 to Discover a minion of your Tier. | Each time the Tavern is rolled (a Refresh, an unfrozen turn start: `hero_after_refresh`), each of its minions without a ticket gets one with chance `DARKMOON_TICKET_CHANCE` (1/3; `Unit::marked`; one `rng.below(3)` per minion) **(engine decision)**: a shop can have several. Buying a minion with a ticket gives the ticket (`counter`); every 3rd gives a Discover among 3 minions of exactly the player's Tier from the pool, and the count starts again **(confirmed)**. |
| `Maiev Shadowsong` | *Imprison* (1 Gold, a card in the Tavern): Choose a card in the Tavern to lock in your hand. After 2 turns, unlock it. | The card leaves the shop for the hand (not a buy) locked for 2 turns (`locked_turns = 2`: it can't be played or sold until the start of the second turn after). It needs room in the hand. |
| `Captain Eudora` | *Buried Treasure* (1 Gold): Dig for a Golden minion! (4 Digs left.) | Each use is a Dig (`counter`); the 4th gives a Golden minion of exactly the player's Tier from the pool (`Unit::make_golden`), and the count starts again **(engine decision)**. The 4th Dig needs room in the hand. |
| `Jandice Barov` | *Swap, Lock, & Shop It* (0 Gold, a friendly non-Golden minion): Swap a friendly non-Golden minion with a random one in the Tavern. | A random minion in the shop (one `rng.below(k)`) takes the friendly minion's board slot, and the friendly minion takes its shop slot, each as it is (not a buy, a sale or a summon). It needs a minion in the shop. |
| `Mutanus the Devourer` | *Devour* (0 Gold, a friendly minion): Sell a friendly minion. Spit its stats onto another. | The minion is sold (1 Gold, its sell triggers and the sell observers, as the `Sell` action), then a random other friendly minion (one `rng.below(k)`) gains its Attack and Health. It needs another friendly minion **(engine decision)**. |
| `Heistbaron Togwaggle` | *The Perfect Crime* (11 Gold): Steal all cards from the Tavern. Each turn, your next Hero Power costs (1) less. | Every shop card goes to the hand while it has room, left to right (not a buy); the rest stay. At the start of each turn, the first included, it gets 1 cheaper (never below 0) **(engine decision)**; using it puts the cost back to 11. It needs a card in the shop and room in the hand. |
| `Ysera` | *Dream Portal*: The Tavern offers an extra Dragon whenever it is Refreshed. | `hero_after_refresh`: a Dragon of the player's Tier or lower from the pool (`CardPool::draw_by_tribe`) joins the shop after its minions (before its spell), beyond its usual size, if the Tavern has room (it holds 7 cards, `tavern.md` §3.2). |
| `Enhance-o Mechano` | *Enhancification*: After the Tavern is Refreshed, give a random minion in it a random Bonus Keyword, twice. | `hero_after_refresh`, twice: a random minion in the shop that lacks one of the Bonus Keywords (Taunt, Divine Shield, Windfury, Venomous, Reborn: **engine decision**) gains one it lacks, at random (one `rng.below(k)` for the minion, one for the keyword); the two can be the same minion. |
| `Varden Dawngrasp` | *Twice as Nice*: After the Tavern is Refreshed, copy its highest-Tier minion and Freeze them both. | `hero_after_refresh`: if the Tavern has room (7 cards), an exact copy of the shop minion with the highest Tier (a tie at random: one `rng.below(k)`; **engine decision**) joins the shop to its right (not from the pool), and both are frozen on their own (`Unit::frozen`, `tavern.md` §3.1): they stay through the next turn start. |
| `Infinite Toki` | *Temporal Tavern* (1 Gold): Refresh the Tavern. Include two minions from a Tier higher than yours. | A Refresh (`TavernState::refresh_shop_with_tier_minions`) whose last 2 minions are of exactly the next Tier (Tier 6 at Tier 6; from the pool, falling back to the highest Tier below with copies left, as a Triple Reward's Discover) **(engine decision)**; the rest as any Refresh, its after-Refresh effects included. |
| `Guff Runetotem` | *Natural Balance* (passive): After you buy 20 Tiers' worth of cards, get a Triple Reward. | `hero_after_buy`: each card bought adds its Tier (`counter`, across turns); every 20 give a `Triple Reward` card (`tavern.md` §5.2: its Tier is locked in as it is given) **(confirmed)**, if the hand has room. |
| `Aranna Starseeker` | *Demon Hunter Training* (passive): After 14 friendly minions attack, the first minion you buy each turn is free. | `hero_friendly_attack`: friendly attacks count (`counter`, across combats); the 14th unlocks it (`stacks` = 1). Unlocked, at the start of each turn it makes the next minion bought free (`next_minion_discount` covers the 3 Gold, `tavern.md` §6), which the first minion bought uses up. |
| `Thorim, Stormlord` | *Choose Your Champion* (passive): At the start of the game, Discover a Tier 7 minion to get after you spend 60 Gold. | At the start of the player's first turn (`hero_turn_start`, once: **engine decision**): a Discover among 3 different Tier 7 minions of the game's catalog (not the pool) whose options go to the Hero Power (`Unit::to_hero_power`, `tavern.md` §5.2): the chosen one is kept (`stored`). `hero_gold_spent` counts the Gold spent (`counter`); from 60 on, the kept minion goes to the hand as soon as it has room. |
| `Cho` | *Double Trouble* (passive): Whenever you play a Golden minion, both you and Gall get a Triple Reward. | Duos only. `hero_after_play`: the first 3 Golden minions played from the hand that give a Triple Reward (not `intrinsic_golden`) each give the player a `Triple Reward` card (if the hand has room) and the teammate one (a teammate `Card` gift, whatever their hero: **engine decision**), on top of the Golden's own card. The player's card locks in their Tier; the teammate's locks in the teammate's own Tier when it arrives (`tavern.md` §5.2; the gift is a card without a locked-in Tier, `counter` 0, locked by its `received` hook) **(confirmed)**. |
| `Gall` | *Double Trouble* (passive): Whenever you play a Golden minion, both you and Cho get a Triple Reward. | As Cho's. |
| `C'Thun` | *Saturday C'Thuns!* (1 Gold): At end of turn, give a friendly minion +1/+1. Repeat 0 times. (Improves each turn!) | Using it arms the turn's end (`armed`; once per turn). `hero_end_turn`: `turn` times (once on turn 1, and one more each turn: **engine decision**), a random friendly minion gains +1/+1 (one `rng.below(k)` each). |
| `Time Twister Chromie` | *Mana Per Minute* (0 Gold): Refresh the Tavern with Tavern spells. | A Refresh (`TavernState::refresh_shop_with_spells`) whose every slot (the minion slots and the spell slot) holds a Tavern spell drawn as the shop draws one **(engine decision)**, with the after-Refresh effects of any Refresh. Once per turn. |
| `Madam Goya` | *Efficient Exchange* (2 Gold, a friendly non-Golden minion): Pass a non-Golden minion. | Duos only. The minion is Passed to the teammate as a card Passes one (`cards::pass_board_minion`: it leaves the board with its stats, not back to the pool; `duos_tavern.md` §8.3). It counts toward the team's Passes like any Pass a card makes (`passes_this_turn`, `team_passes_this_game`; Passenger, Transport Reactor see it) **(confirmed**, by analogy with card-effect Passes**)**. |
| `Sylvanas Windrunner` | *Reclaimed Souls* (2 Gold): Discover a plain copy of a minion that died last combat. (Unlocks on Turn 3.) | `hero_after_combat` keeps plain copies of the player's minions that died in their last combat (`stored`). From the 3rd turn on: a Discover among up to 3 of them with different cards, at random (not from the pool). It needs one and room in the hand. |
| `Yogg-Saron, Hope's End` | *Puzzle Box* (passive): At the start of your turn, cast a random Tavern spell. (Unlocks on Turn 3.) | `hero_turn_start`, from the 3rd turn on: a Tavern spell drawn as the shop draws one is cast (`spells::cast_spell_by_effect`); a targeted one on a random valid target (a friendly minion, or a minion in the Tavern for a spell that only targets those), and with none it is not cast **(engine decision)**. |
| `N'Zoth` | *Avatar of N'Zoth* (passive): Start the game with a 2/2 Fish that gains all your Deathrattles in combat. | When the player gets the hero (`hero_chosen`), a `Fish of N'Zoth` (`TB_BaconShop_HP_105t`, a Tier 1 `2/2` Beast token) joins the board if it has room. The Fish: *after a different friendly Deathrattle minion dies in combat, gain its Deathrattle*: its `friendly_death` hook adds the dead minion's Deathrattle (its card's, then those an effect gave it) to its `extra_deathrattles`, for the combat (its Tavern copy keeps nothing). A gained Deathrattle resolves with the Fish as its minion (its Golden, its stats) **(engine decision)**. |
| `Sneed` | *Pilot the Shredder* (passive): Start the game with a 2/1 Shredder that summons a minion from your hand and gives it Divine Shield. | When the player gets the hero, a `Sneed's New Shredder` (`BG21_HERO_030t`, a Tier 1 `2/1` Mech token) joins the board. Its Deathrattle summons a copy of the highest-Health minion in the hand (a tie at random) not yet summoned from the hand this combat, with Divine Shield, for this combat (as `Diremuck Forager`'s hand summons, `combat.md` §2); a Golden one, the two highest **(engine decision)**. |
| `The Curator` | *Menagerist* (passive): Start the game with a 2/2 Amalgam with Venomous and all minion types. | When the player gets the hero, an `Amalgam` (`TB_BaconShop_HP_033t`, a Tier 1 `2/2` token of every type with Venomous) joins the board. |
| `Galakrond` | *Galakrond's Greed* (1 Gold, a minion in the Tavern): Choose a minion in the Tavern. Then choose a higher Tier minion to replace it. | A Discover among 3 minions of exactly the next Tier, `min(6, tier + 1)`, from the pool (as a Triple Reward's), kept by the Hero Power (`to_hero_power`): the chosen one replaces the Tavern minion in its slot (with the shop's auras), whose copies go back to the pool with the other options' (`hero_option_chosen`). A Tier 6 minion is a target too: it is offered Tier 6 minions **(confirmed)**. |
| `Ragnaros the Firelord` | *BUY, INSECT!* (passive): After you buy 12 cards, get Sulfuras. | `hero_after_buy`: every card bought counts (`counter`, across turns); the 12th turns the Hero Power into *Sulfuras* (`TB_BaconShop_HP_087t`; its `card_id` changes, as Bru'kan's Elements do): "At the end of your turn, give your left and right-most minions +8/+8" (`hero_end_turn`; a single minion gains it once **(confirmed)**). |
| `Rock Master Voone` | *Upbeat Harmony* (passive): At the end of every 3 turns, get a plain copy of the left-most card in your hand. | `hero_end_turn`: every 3rd turn end (`counter`, counting from the first) gives a plain copy of the left-most card in the hand (`cards::plain_instance`; a spell: a fresh one) if the hand has room; with an empty hand, nothing **(engine decision)**. |
| `The Jailer` | *Rune of Damnation* (1 Gold, a friendly Undead): Destroy a friendly Undead to get a random Undead. (Unlocks at Tier 2.) | From Tavern Tier 2 on: the Undead is destroyed (`destroy_board_unit`: its Deathrattle resolves), then a random Undead of the player's Tier or lower from the pool goes to the hand **(engine decision)**. It needs room in the hand. |
| `Drest'agath` | *Incubate* (1 Gold, a card in the hand): Discard a card to get a random Aberration. | The card is discarded (`discard_hand_card`: a minion's copies return to the pool, the discard observers see it), then a random Aberration of the player's Tier or lower from the pool goes to the hand **(engine decision)**. A locked card is not a target. |
| `Malygos` | *Arcane Alteration* (0 Gold, a card in the Tavern): Replace a card with a random one of the same Tier. (Twice per turn.) | A card in the Tavern **(engine decision: the Tavern's cards)** is replaced in its slot: a minion by a random minion of exactly its Tier from the pool (its copies return first), a spell by a random Tavern spell of exactly its Tier. Two uses per turn, as Blackthorn's. |
| `Captain Hooktusk` | *Trash for Treasure* (0 Gold, a friendly minion): Remove a friendly minion. Discover one from a Tier lower to get. | The minion leaves the board (its copies return to the pool; not a sale or a death), then a Discover among 3 minions of exactly its Tier minus 1 (Tier 1 for a Tier 1 minion: **engine decision**) from the pool. It needs room in the hand. |
| `Tae'thelan Bloodwatcher` | *Reliquary Research* (passive): Every third Tavern spell you buy costs (0). | `hero_after_buy`: each Tavern spell bought counts (`counter`, across turns); after the 2nd, the next spell bought costs 0 (`next_spell_discount` covers its whole cost), and the 3rd starts the count again. |
| `Exarch Othaar` | *Arcane Knowledge* (0 Gold): The next Tavern spell you buy costs (1) less. (Unlocks on Turn 3.) | Active, once per turn, from the 3rd turn on (`turn >= 3`) **(engine decision: an action)**: `next_spell_discount += 1`. |
| `Shudderwock` | *Snicker-snack* (0 Gold, a friendly Battlecry minion): Trigger a friendly minion's Battlecry. (Unlocks on Turn 3.) | From the 3rd turn on: the minion's Battlecry triggers as `Young Murk-Eye`'s do (`cards::trigger_board_battlecry`). |
| `Kith'ix` | *Dark Ritual* (2 Gold): Get 2 random minions. When you play one, discard the other. | 2 random minions of the player's Tier or lower from the pool go to the hand as one group of a single play (the `Faceless Operative` group, `tavern.md` §6): playing or Passing one discards the other. It needs room in the hand. |
| `Farseer Nobundo` | *The Galaxy's Lens* (3 Gold): Get a copy of the last Tavern spell you cast. Each turn, your next Hero Power costs (1) less. | A fresh copy of `last_tavern_spell_cast` goes to the hand; it needs one and room in the hand. At each turn start, the first included, it gets 1 cheaper (never below 0); using it puts the cost back to 3, as Togwaggle's. |
| `Zephrys, the Great` | *Three Wishes* (3 Gold): If you have two copies of a minion, find the third. (3 Wishes left!) | With two non-Golden copies of a minion on the board and in the hand (the first such pair, board then hand, left to right: **engine decision**), the third (from the pool if it has a copy, else a plain one from the catalog) goes to the hand, where they Triple. Three uses per game (`stacks`). It needs room in the hand. |
| `Inge, the Iron Hymn` | *Major Hymn* (0 Gold, a friendly minion): Twice per turn, give a minion Attack equal to your Tier. (Swaps to Health next turn!) | Two uses per turn: the minion gains Attack equal to the Tavern Tier on odd turns (and before the first), Health on even turns (`armed`, set at each turn start) **(engine decision)**. |
| `Vol'jin` | *Spirit Swap* (0 Gold, 2 minions): Choose 2 minions. They gain each other's Attack until next turn. | Targets any friendly board minion (`0..7`) or Tavern minion (`7..14`, `BoardOrShopMinion`) **(confirmed)**; usable when at least 2 minions exist across `board` and `shop` **(engine decision)**. The first `HeroPower { target: Some(i) }` picks that minion: it gets an id no other card on the board, in the shop or in the hand has (`heroes::identify_minion`), which the power remembers (`picked`), and `uses_left = 1` is restored without spending Gold. The second `HeroPower { target: Some(j) }`, on a different minion, clears `picked` and gives each minion the other's current Attack (`attack.max(0)`) until next turn; they keep it through combat **(confirmed)** (`Unit::until_next_turn_atk`: expires at the next turn start on `board`, `hand`, and kept/frozen `shop` minions; cleared if a minion's Attack is set by Xyrella or `Perfect Vision`; summed on a Triple: **engine decision**). The first pick follows its minion when the board or the shop changes (a Buy, a Sell, a reposition, a play); once that minion is no longer on the board or in the shop (bought, sold, Refreshed away), the pick drops and the next use is a first pick again **(engine decision)**. Ending the turn after only the first pick drops it (`hero_turn_reset`) **(confirmed)**. The observation's `hero_power_armed` is the first pick's current target + 1 (0 without one, §8). |
| `Sir Finley Mrrgglton` | *Adventure!* (0 Gold, passive): At the start of the game, Discover a Hero Power. | At the start of his first turn (`hero_turn_start`), a Discover among 3 distinct Hero Powers (§11.3), the chosen one replacing his own **(confirmed)** for the rest of the game, fresh (counters at 0) **(confirmed)**: `hero_option_chosen` installs it (`heroes::install_power`), running its `hero_chosen` (its "when you get it" text: Millhouse's +1 Upgrade cost **(confirmed)**, N'Zoth's Fish, Patchwerk's 30 Health), `hero_turn_reset` and `hero_turn_start` hooks. |
| `Master Nguyen` | *Power of the Storm* (0 Gold, passive): At the start of every turn, choose from 2 new Hero Powers. | At every turn start, turn 1 included, he chooses 1 of 2 new Hero Powers for that turn **(confirmed)**: *Power of the Storm*'s `hero_base_turn_reset` hook (which runs for the hero's own Hero Power, whatever power replaced it, §4) puts it back in `hero.power` (`heroes::replace_power`: the outgoing power's `hero_power_removed` hook cleans up after it, §11.3), and its `hero_turn_start` offers a Discover of 2 distinct Hero Powers for a single turn (§11.3). The chosen one replaces it for that turn, fresh, so a passive's progress starts again every turn **(confirmed)**, and runs its `hero_chosen`, `hero_turn_reset` and `hero_turn_start` hooks. |
| `Genn, Worgen King` | *King of Duality* (0 Gold, passive): On Turn 4, Discover two Hero Powers to replace this. (3 turns left!) | Starts with `counter = 3`; on turns 1–3, `counter = 4 - turn`. On turn 4 (`hero_turn_start`), a Discover of 3 distinct Hero Powers (§11.3); the chosen one replaces *King of Duality* (`hero.power`), and a second Discover of 3 new ones (without the first) follows at once, whose choice becomes his second Hero Power (`hero.second_power`, `HeroPower2`, §5) **(confirmed)**. He keeps both for the rest of the game, fresh, and both work, passives included **(confirmed)**. |
| `The Nameless One` | *(no name)* (`BGDUO_HERO_100p`, 0 Gold, passive; Duos only): Start of Game: Copy your teammate's Hero Power. | At the start of the team's first turn, before either player's `start_turn` runs (its `hero_before_team_turn` hook, `heroes::before_team_turn`, called by `DuosTavern::start_turn`), `hero.power` becomes a fresh copy of the teammate's hero's Hero Power **(confirmed)** and runs its `hero_chosen` hook, so start-of-game and turn-1 hooks fire normally **(engine decision)**. The copy has no `hero_before_team_turn` hook of its own, so it happens once. A team never has two copies of `The Nameless One` (the copy asserts it) **(confirmed)**: `draw_offers` never offers `The Nameless One` to both players on a team (§6) **(engine decision)**. |
| `Lord Jaraxxus` | *Bloodfury* (0 Gold, passive): After friendly minions deal 150 damage, open a portal to the Twisting Nether! (150 left!) | `hero_friendly_damage_dealt`: adds `amount.max(0)` to `counter` across combats and Tavern triggers; once `counter >= 150`, transforms the Hero Power into *Nether Portal* (`TB_BaconShop_HP_036t`, `123087`, 0 Gold passive: "At the start of each turn, get 2 random Demons.", `hero_turn_start`: draws up to 2 Demons of the player's Tier or lower from the pool to the hand) **(confirmed)**. |
| `Cariel Roame` | *Conviction* (0 Gold, active): Give 1 random friendly minion +1/+1. Passive: After each combat, choose an improvement. | Active untargeted, usable when `!state.board.is_empty()`: gives `1 + armed` distinct random friendly minions `+(1 + counter)/+(1 + stacks)`. `hero_after_combat`: offers a Choose One of *Offensive Stance* (`101567`, +1 Attack: `counter += 1`), *Defensive Stance* (`101575`, +1 Health: `stacks += 1`), and *United Stance* (`104923`, +1 target: `armed += 1`) **(confirmed)**. |
| `Cookie the Cook` | *Stir the Pot* (0 Gold, a friendly or Tavern minion): Throw a minion in your pot. When you've gathered 3, Discover from their types. (3 left!) | `ActivateTargetKind::BoardOrShopMinion`: any friendly or Tavern minion, which stays where it is (not removed or destroyed) **(confirmed)**; a minion without a type is not a target **(engine decision)**. The first two throws keep a plain copy of the minion in the pot (`stored`; `counter` = minions thrown). The 3rd throw (which requires room in the hand) empties the pot (`counter = 0`) and offers a Discover of 3 minions of the player's Tier or lower from the pool, one of each of the 3 gathered types **(confirmed)** (`CardPool::draw_discover_by_tribes`, distinct minions, a minion's whole type as its type: **engine decision**). |
| `Dancin' Deryl` | *Hat Trick* (0 Gold, passive): When you play a minion, give it a +1/+1 hat that passes to a friendly minion when sold. | `hero_after_play`: when `played.from_hand`, the played minion gets a hat: `+1/+1` and `hats += 1` **(confirmed)**. A hat is a Unit mechanic, not the Hero Power's: `Unit::hats` counts the copies of the hat card (*Hats*, `TB_BaconShop_HP_042e`, `model::HAT_CARD`) a minion wears, transferred on Magnetize and summed on a Triple **(confirmed)**. When a minion wearing hats is sold, the sale (`TavernState::sell_minion`, after `hero_after_sell`) runs the hat card's `sell` hook, registered by `dancin_deryl.rs`, whatever the player's Hero Power: each hat independently gives a random remaining friendly board minion `+1/+1` and `hats += 1` **(confirmed)** (one `rng.below(k)` per hat: **engine decision**). So hats still pass when the minion is sold after the player lost *Hat Trick* (a Passed minion, Master Nguyen on a later turn) **(confirmed)**. |
| `Scabbs Cutterbutter` | *I Spy* (2 Gold): Discover a plain copy of a minion from your next opponent's warband. | Active, once per turn; usable when `next_opponent_warband()` (`lobby.next_opponent`'s `last_warband`) is non-empty and the hand has room **(confirmed)**. Offers up to 3 random minions without replacement from `next_opponent_warband()` (`rng.below(k)`), converted to plain copies (`cards::instantiate_plain_copy`), kept by the Hero Power (`to_hero_power`): choosing one (`hero_option_chosen`) adds it to the hand and runs `on_obtained` and `on_card_discovered` without returning unchosen options to the pool **(confirmed)**. |
| `Tess Greymane` | *Bob's Burgles* (1 Gold): Refresh the Tavern with plain copies of your last opponent's warband. | Active, once per turn (`TavernState::refresh_shop_with_warband`): the Tavern's minions are replaced by plain copies of the last opponent's warband as it entered combat, up to the shop's capacity **(confirmed)**. In detail **(engine decision)**: the current shop minions return to the pool, the shop unfreezes, up to `shop_minion_slots()` slots fill with plain copies (`cards::instantiate_plain_copy`, with shop auras applied) of `last_opponent_warband()` (sampling `shop_minion_slots()` without replacement via `rng.below(k)` if the warband has more), plus the shop spell if `include_shop_spells`, and the after-Refresh effects run. Not usable while the last opponent's warband is empty or unknown (`last_opponent_warband()` empty), so it can't be used to empty the Tavern for 1 Gold **(engine decision)**. |
| `Mr. Bigglesworth` | *Kel'Thuzad's Kitty* (0 Gold, passive): After another hero dies, Discover a minion from their warband. It keeps enchantments. | Whenever another player dies, at the start of the player's next turn, a Discover of a minion from their final warband that keeps its stats and enchantments, or a `Consolation Coin` if their warband was empty **(confirmed)**. In detail **(engine decision)**: `hero_turn_start`, for each opponent in `lobby.opponents` with `died_last_combat` (in list order): with an empty `last_warband`, a `Consolation Coin` (`TB_BaconShop_HP_080t`, dbfId `94063`, a 0-Gold token spell: gain 1 Gold) goes to the hand; otherwise a Discover of up to 3 random minions without replacement from `last_warband`, as they were (stats, keywords, Golden, enchantments), kept by the Hero Power (`to_hero_power`; the chosen one goes to the hand). The Coin is not a Tavern spell **(confirmed)**. |
| `Lord Barov` | *Friendly Wager* (1 Gold, `PairSide`): Guess which player will win their next combat. If you're correct, get 3 Tavern Coins. | The player picks one of the two players of the lobby's featured next combat; at their next turn start they get 3 `Tavern Coin`s if that player won, 1 if it was a tie **(confirmed)**. In detail **(engine decision)**: active, once per turn, target `0` (side A of `lobby.featured_pair`) or `1` (side B), usable while `lobby.featured_pair` is set; it records `armed = target + 1`; the next `hero_turn_start` reads `lobby.last_pair_outcome` (`AWin` for `1`, `BWin` for `2`) and clears `armed`; a lost wager gives nothing. A wager still open when the Hero Power is replaced (Master Nguyen's next turn start, `hero_power_removed`) is settled then, the same way. |
| `Murloc Holmes` | *Detective for Hire* (0 Gold): Look at 2 minions. Guess which one your next opponent had last combat for a Tavern Coin. | Active, 0 Gold, once per turn **(confirmed)**. If `next_opponent_warband()` is non-empty, picks 1 random minion from it (`rng.below(k)`) as a plain copy (`cards::instantiate_plain_copy`); otherwise offers the `"No Minions"` prompt card (`BG23_HERO_303pt`, dbfId `92974`). Offers that 1-option `discover_pending` (`to_hero_power`), and `hero_option_chosen` gives 1 `Tavern Coin` to the hand **(confirmed)**. |
| `King Mukla` | *Bananarama* (0 Gold, passive): At the start of your turn, get 2 Bananas and give everyone else one. | `hero_turn_start`: rolls 2 Bananas for Mukla's hand and 1 Banana for each living opponent (`lobby.opponents.iter().filter(|o| o.alive).count()`) into `TavernState::lobby_gifts` **(confirmed)**. Each Banana roll (`rng.below(2)`) is 50% `Big Banana` (`BG_Bananas_Big`, 1-Gold token spell, not a Tavern spell: give a minion +2/+2) and 50% `Bananas` (`BG_Bananas`, 1-Gold token spell, not a Tavern spell: give a minion +1/+1) **(confirmed)**. |

### 11.3 Hero Powers Offered by a Discover

Sir Finley, Master Nguyen and Genn Discover Hero Powers (`heroes::offerable_powers`, `heroes::offer_power_discover`): distinct options, no repeats within one offer **(confirmed)**, among the implemented heroes' Hero Powers, Duos-only ones only in Duos, but not the player's current Hero Power(s) (nor Genn's first pick in his second Discover) **(confirmed)**. A Hero Power's own file flags it when it is never offered (`CardFlags::NOT_DISCOVERABLE_POWER`) or not offered for a single turn (`CardFlags::NOT_A_TURN_POWER`).

Never offered (`NOT_DISCOVERABLE_POWER`):

| Hero Power | Why |
| :--- | :--- |
| Sir Finley's, Master Nguyen's, Genn's, The Nameless One's | They choose or copy Hero Powers themselves **(confirmed)**. |
| A. F. Kay's, Ambassador Faelin's | Their skipped turns would be past **(confirmed)**. |
| Cho's, Gall's | Never **(confirmed)**. |

Sir Finley and Genn may be offered every other one, Patchwerk's, N'Zoth's, Sneed's, The Curator's and Thorim's included **(confirmed)**: their start-of-game text resolves as they get it (Patchwerk's 30 Health, the Fish, the Shredder, the Amalgam, Thorim's Tier 7 Discover).

Master Nguyen offers as many as work for a single turn **(confirmed)**. Not for a single turn **(engine decision)**:

| Hero Power | Why |
| :--- | :--- |
| Patchwerk's (`NOT_A_TURN_POWER`) | Its 30 extra Health would come back every turn. |
| N'Zoth's, Sneed's, The Curator's (`NOT_A_TURN_POWER`) | Their start-of-game token (the Fish, the Shredder, the Amalgam) would come back every turn. |
| Thorim's (`NOT_A_TURN_POWER`) | Its start-of-game Tier 7 Discover would come back every turn, and the kept champion would be lost with the Hero Power. |
| An active one its `hero_power_usable` hook says can't be used now | It wouldn't work that turn: Alexstrasza and Millificent before Tier 4, The Jailer before Tier 2, Exarch Othaar, Shudderwock and Sylvanas before turn 3 (Sylvanas also needs minions that died in the last combat while she was the Hero Power), Scabbs, Tess and Lord Barov without the lobby information they read, a full hand for the powers that need room in it. |

Every other one is offered, the passives that act turn by turn (Millhouse, Sindragosa, Clocksworth, Snake Eyes, Ysera, Varden, Silas, Jaraxxus, Cariel, Cookie, Bigglesworth, Mukla, ...), long-progress ones (Ragnaros, Aranna, Kael'thas, ...) and once-per-game ones (Reno, Kragg, Zerek), whose state starts fresh every turn. What a Hero Power gives for as long as it is the Hero Power comes back off when Nguyen takes his back (`hero_power_removed`, **engine decision**): Millhouse's +1 Upgrade cost, Saurfang's Tavern bonus, Rakanishu's spell bonus; Tavish's loaded minion goes back to the pool, and Barov's open wager is settled. Vanndar's and Drek'Thar's unlock reads the game's turn, not how long the Hero Power was held. Cariel's improvement Choose One, offered after a combat, may be answered once Nguyen's Hero Power is back: it then improves nothing **(engine decision)**.

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
| Powers that give cards | Blackthorn, Holli'dae, Baz'hial, Xyrella, Pyramad, Maiev, Patches, Elise, Alexstrasza, Millificent, the Rat King and Togwaggle need room in the hand; a Blood Gem that doesn't fit is lost. | their files |
| Targets | George's and The Lich King's minion doesn't already have the keyword. | their files |
| Holli'dae's spell | Drawn as the shop draws a spell: the player's Tier or lower. | `heroes/doctor_hollidae.rs` |
| Edwin's improvement | +1/+1 per 4 cards bought, spells included. | `heroes/edwin_vancleef.rs` |
| Baz'hial's damage | Not usable if it would bring Health and Armor to 0. | `heroes/lich_bazhial.rs` |
| Kragg's Gold | 2 on the first turn, +1 per turn started since. | `heroes/skycapn_kragg.rs` |
| Xyrella's auras | The minion set to 2/2 gets the auras it has in the hand again (2/5 with a +0/+3 Volumizer bonus), rather than staying 2/2. | `heroes/xyrella.rs` |
| Rakanishu's improvement | Every 3rd turn start, counting the first. | `heroes/rakanishu.rs` |
| Batch 10 | Faelin's kept minions arrive at a turn start, after an Upgrade or after a minion is played; Silas' ticket chance per rolled minion is 1/3 (`DARKMOON_TICKET_CHANCE`). | `heroes/ambassador_faelin.rs`, `heroes/silas_darkmoon.rs` |
| The Rat King's types | A fixed rotation through the 10 lobby types in `LOBBY_TRIBES` order, one per turn from Aberration on turn 1 (no random start, no types missing from the lobby). | `heroes/the_rat_king.rs` |
| Cards from the catalog or the pool | Ini's Mech and Brann's Brann come from the catalog (not the pool); Patches' Pirate (Tier or lower), Elise's, Alexstrasza's, Millificent's, the Rat King's and Eudora's minions from the pool. | their files |
| Kurtrus' count | It starts again each turn. | `heroes/kurtrus_ashfallen.rs` |
| Eudora's Digs | The 4th gives a Golden minion of the player's Tier; the count starts again. | `heroes/captain_eudora.rs` |
| Mutanus' target | Only with another friendly minion. | `heroes/mutanus_the_devourer.rs` |
| Togwaggle's discount | From the first turn's start on. | `heroes/heistbaron_togwaggle.rs` |
| Enhance-o's Bonus Keywords | Taunt, Divine Shield, Windfury, Venomous and Reborn; only a minion that lacks one is picked. | `heroes/enhance_o_mechano.rs` |
| Varden's tie | The highest-Tier minion is picked at random among ties. | `heroes/varden_dawngrasp.rs` |
| A partly frozen turn start | Counts as a Refresh (its after-Refresh effects run, Varden's included). | `tavern.rs` |
| Batch 7 | Voone with an empty hand gets nothing; the Jailer's Undead and Drest'agath's Aberration are of the player's Tier or lower; Malygos replaces the Tavern's cards; Hooktusk's Tier 1 minion gives a Tier 1 Discover; Othaar is an action; Zephrys takes the first pair; Inge gives Attack on odd turns, Health on even ones. | their files |
| Cho and Gall | The teammate gets the card whatever their hero; only Goldens that give a Triple Reward count. | `heroes/cho.rs`, `heroes/gall.rs` |
| Thorim's minion | Offered at the start of the first turn; it waits for room in the hand once 60 Gold were spent. | `heroes/thorim_stormlord.rs` |
| Batch 9 | C'Thun buffs `turn` times; Chromie's Refresh fills every slot with a spell;  Yogg's targeted spell with no target is not cast. | their files |
| Hero tokens | The start-of-game token needs room on the board (it always has it at game start); the Fish's gained Deathrattles resolve as the Fish's; a Golden Shredder summons two. | `heroes/nzoth.rs`, `heroes/sneed.rs`, `heroes/the_curator.rs` |
| Toki's minions | Exactly the next Tier (Tier 6 at Tier 6), the last 2 of the shop's minions. | `heroes/infinite_toki.rs` |
| Hero Powers for a single turn | Master Nguyen is not offered the start-of-game ones, nor one that can't be used now (§11.3). | the Hero Powers' files, `heroes::offerable_powers` |
| A replaced Hero Power | What it gave for as long as it was the Hero Power comes back off (Millhouse, Saurfang, Rakanishu), Tavish's minion goes back to the pool before the shop is rolled, Barov's open wager is settled. | their files |
| Patchwerk's Hero Power on another hero | Its 30 Health comes with it. | `heroes/patchwerk.rs` |
| Vanndar's and Drek'Thar's unlock | Turn 7 of the game, however long the Hero Power was held. | `heroes::arm_from_turn` |
