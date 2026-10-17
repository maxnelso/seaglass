# Dark Gifts Specification

This document specifies Battlegrounds **Dark Gifts** in Seaglass: the **Dark Discovery** button every player has, the offers it makes (three minions, each with a different Dark Gift), what a Dark Gift is on a minion, which gift a minion can be offered, how gifts reach the Tavern and combat, the heroes, trinkets and minion that give them, the RL action and observation, the scenario DSL, and the Power.log replay. It builds on [`tavern.md`](tavern.md), [`combat.md`](combat.md), [`heroes.md`](heroes.md), [`trinkets.md`](trinkets.md), [`duos_tavern.md`](duos_tavern.md) and [`duos.md`](duos.md), and like them it is updated **before** behavior changes. The engines stay card-agnostic: what a gift does lives in its own file (`src/dark_gifts/<slug>.rs`) and reaches the engine only through hooks, as trinkets do.

Rules marked **(confirmed)** were settled with the user. Rules marked **(engine decision)** are details nobody has settled yet: the engine implements them as written, each in one place, so that it is a small change if the answer differs (§13). The rules source is the Battlegrounds team's Dark Gift post (with Hotfix 36.2.1) for eligibility, and the card data (`CardDefs.xml`, Patch 36.6.3, newer than the post) for every number: stats, turn windows, weights.

**A game without Dark Gifts** (the default: `TavernState::dark_gifts_enabled` is `false`, no minion has a gift) plays exactly as before this document: no new legal action, no RNG draw, no new field in any serialized state. Solo and Duos results are byte-identical for it **(engine decision: the button is behind a flag, off by default in `TavernState::new`, `DuosTavern` and the Python constructors, so existing tests and RL runs are unchanged; the fuzzers turn it on in half their games)**.

---

## 1. Scope

### Included
- The gift table (`src/dark_gifts/table.rs`), generated from `CardDefs.xml` (§9): the 47 gift cards `BG36_MidGameEffect_000t*` (43 names: four have an early and a late strength).
- The Dark Discovery button: the `DarkDiscovery` action, its legality and its price (§3).
- The offers: the minions' Tiers and filters, the gifts' turn windows, weights and eligibility (§4, §5).
- A minion's gifts (`Unit::dark_gifts`, §2), the gift hooks (§6), gifts in combat (§7), Duos, Python / RL and the scenario DSL (§8), the replay (§10).
- The gifts implemented so far (§11); `Nightmare Lord Xavius` and `Tras'tath, Soul Parasite`, the trinkets `Ominous Stone`, `Sinister Invitation`, `Maldraxxus Dagger` and `Wax Lance`, and `Dark Paradox` (§12).

### Deferred
- The Dark Gift anomaly (`BG36_Anomaly_002`) **(confirmed: out of scope)**.
- A gift the engine doesn't implement is in the table but never offered (§4.4) **(engine decision, as for heroes and trinkets)**.

---

## 2. The Gift

### 2.1 Gift Data

Every gift is a `DarkGiftData` row of the generated table (`dark_gifts::table()`):

| Field | Meaning |
| :--- | :--- |
| `id` | Its dbfId (`133421` for `Fortitude`): the gift's engine `CardId`, the card whose hooks implement it. |
| `card_id`, `name` | Its card ID (`BG36_MidGameEffect_000t73`) and name. |
| `variant` | `Early` / `Late` for the two strengths of a shared name (`Battle Scars`: `+2/+2` on turns 4–6, `+3/+3` on turns 7–10), else none. |
| `min_turn`, `max_turn` | The game turns it can be offered on (CardDefs tags 4853 and 4854; no 4853: from the first turn, no 4854: no last turn). |
| `weight` | Its weight in a draw (tag 4856: 50 or 200; 100 without it). |
| `trastath` | `Tras'tath`'s Discover can give it (tag 4874; §12.2). |
| `text` | Its text, rendered as for Hero Powers (`heroes.md` §9); a number the card has no tag for stays as written (`Steady Growth`: `+{1}/+{2}`). |
| `enchantments` | Its enchantment card IDs (`...t9e`, `...t9e2`): what a Power.log attaches to a minion with the gift (§10). |

The catalog API (`src/dark_gifts/mod.rs`): `dark_gifts::table()`, `data(id)`, `by_card_id(card_id)`, `by_enchantment(card_id)`, `by_name(name)` (display name, slug or card ID), `display_name`, `slug`, `is_implemented(id)`, `vocabulary()`.

### 2.2 A Minion's Gifts (`Unit::dark_gifts`)

A minion's gifts are `Unit::dark_gifts: Vec<DarkGift>`, in the order it got them (serde: skipped while empty, so a minion without gifts serializes as before). A `DarkGift` is:

| Field | Meaning |
| :--- | :--- |
| `card_id` | The gift (its dbfId: its hooks). |
| `counter` | A countdown or count of the gift's own (`Replication`'s turns, `Toreth's Blessing`'s hits). |
| `stacks` | The gift's strength, set when it is given (`Steady Growth`'s offering turn). |

The engine reads only `card_id`; the gift's own hooks read the rest. A gift is a permanent enchantment of the minion it is on:

- **More than one.** A minion can have several gifts (a Xavius or trinket gift on top of a button pick), the same one twice too; each acts **(confirmed)**.
- **Giving.** `dark_gifts::give(unit, id, turn)` appends the gift (its `stacks` the game turn it is given on) and runs its `gift_given` hook: the keywords, stats or Golden it gives at once (`Harpy's Talons`, `Fortitude`, `Gilding`).
- **Wherever it goes.** The gifts are part of the minion: they stay on it in the hand, on the board, through a combat and back, when it is returned to the hand, Passed to a teammate (Duos) or copied as it is ("a copy of this": `Doppelganger`, `Double Vision`) **(engine decision)**. A plain copy (`cards::instantiate_plain_copy`, "a plain copy") and a Reborn copy have none (`Persisting Horror` aside, §11). A sold minion's gifts leave with it.
- **Triples.** The Golden a Triple makes has every copy's gifts, in copy order **(confirmed: tripling combines the gifts)**. What a gift gave as stats are buffs, which the Golden keeps as it keeps every buff (`tavern.md` §5.1): a `Fortitude` copy makes a Golden `+5/+5` stronger, not `+10/+10` **(engine decision)**.
- **Golden.** A gift's effect is the same on a Golden minion: gifts are not card text, and Golden doesn't double them (as for trinkets) **(engine decision)**. `Dark Paradox`'s own gift is card text and doubles (§12.4).

### 2.3 The Gift's Turn

A gift's turn window is the player's game turn (`TavernState::turn`, turn 1 the first) **(confirmed)**. A Power.log's `TURN` is `2 × turn − 1` (§10).

---

## 3. The Dark Discovery Button

### 3.1 The Rules **(confirmed)**

Every player has the button in every game (with `dark_gifts_enabled`):

1. It costs **3 Gold** (`BG36_Button_DarkGift`'s `COST`).
2. It unlocks on **turn 3**.
3. At most **once per turn**, and **3 uses per game**.
4. It Discovers 1 of **3 minions, each with a different Dark Gift** (§4).

### 3.2 The `DarkDiscovery` Action

`TavernAction::DarkDiscovery` is the last variant of `TavernAction` (after `ChooseTrinket`), so the earlier variants keep their places.

- **Legal** if Dark Gifts are enabled, no prompt is pending (a hero choice, a trinket offer or a Discover come first, as for every other action), the hero doesn't skip the turn, `turn >= 3`, the button wasn't used this turn, fewer than 3 uses so far, and `gold >= 3`.
- **Effect**, in order: the price is paid (`TavernState::spend_gold`: Gold spent, which the Gold-spent observers see) **(engine decision)**; the turn's use and the game's use are counted (`TavernState::dark_discovery_used_this_turn`, `dark_discoveries_used`); the offer is drawn (§4.3) and becomes a Discover (`push_discover`). Then the action ends like any other (`tavern.md` §6).
- An offer can have fewer than 3 options when the pool runs out, or none: the Gold and the use are spent all the same **(engine decision)**.
- The turn's use resets at the start of each turn.

### 3.3 Choosing

The offer is a regular Discover (`ChooseDiscover`): the chosen minion goes to the hand with its gift (it was taken from the pool), and the other options go back to the pool **(engine decision, agreed: the minions come from the shared pool like a normal Discover)**. Like any Discover, it is a card the player Discovered (`trinket_after_discover`, `Sinstone Sticker`; the card's own "when you Discover this", and its gifts' `gift_obtained`: `Double Vision`).

---

## 4. Offers

### 4.1 The Minions **(confirmed)**

**Tier by turn** (the same table for `Nightmare Lord Xavius`):

| Turn | Minion Tier |
| :--- | :--- |
| 3 | 2 |
| 4 | 2 or 3 |
| 5 | 3 |
| 6 | 3 or 4 |
| 7 | 4 |
| 8 | 4 or 5 |
| 9 | 4, 5 or 6 |
| 10+ | 5 or 6 |

(Turns 1 and 2, which only a Hero Power can reach: Tier 2.)

**Filters** — a minion can't be offered if:
1. it has a Battlecry or a Choose One and the turn is before 5;
2. it is Magnetic;
3. it has a "when you sell this" effect (its card has a `sell` hook: `Freedealing Gambler`, `Patient Scout`, ...);
4. it is meant to stay in the hand (its card has a hand hook: `turn_start_in_hand`, `after_friendly_play_in_hand`, `ready_in_hand` or `combat_copies_from_hand`: `Bream Counter`, `Flighty Scout`, ...) **(engine decision: what "meant to stay in hand" means)**;
5. it is banned: `Leeroy the Reckless` (Patch 36.6.3's Leeroy) and `Deadly Spore` (the post's examples), and `Dark Paradox`, which has a gift of its own **(engine decision: the list)**;
6. no gift can be offered on it (§5) **(engine decision: so that every option has a gift)**.

**Most common type.** From turn 6 on, one offered minion is of the player's most common type **(confirmed)**: the type the most minions on their board have (`spells::most_common_tribe`, `tavern.md` §7.8: an `All` minion counts for every type; a tie broken uniformly at random; with no typed minion, a random lobby type) **(engine decision: the board, as every "most common type" in the engine, and the tie rule)**.

### 4.2 The Gifts

Each option gets one gift, drawn by weight among the gifts it can be offered (§5), never one already given to another option of the offer **(confirmed: each with a different gift)**. `Harpy's Talons` is offered more often on Rally minions (the post): its weight is doubled on a Rally minion **(engine decision: the factor)**.

### 4.3 The Draw (`dark_gifts::draw_offer`) **(engine decision, agreed: the minions first, then a gift for each)**

1. **Most common type** (turn 6 and later): `t = spells::most_common_tribe(board, rng)` (it draws only for a tie or a board without types).
2. **Three minions**, one after another, from the pool as a Discover draws (`CardPool::draw_template_where`: weighted by copies left, distinct cards, taken out of the pool). For each:
   - its Tier: one `rng.below(k)` among the turn's `k` Tiers (none for one Tier) **(engine decision: uniform per minion)**; if the pool has no minion for it at that Tier, the turn's other Tiers, from the lowest **(engine decision)**;
   - the candidates: the pool's minions of that Tier that pass the filters (§4.1) and aren't already offered; the first option, from turn 6, also of type `t` (if the pool has none of it, it is drawn like the others) **(engine decision)**.
3. **Gifts**, in option order: for each option, one `rng.below(total weight)` among the gifts it can be offered and the offer hasn't given yet. `Gilding` can only go to an option of the offer's lowest Tier (§5.2), known once the three minions are drawn. An option left with no gift (the others took the only ones it could have) goes back to the pool and is replaced by a new draw as in step 2 (the same type rule, a minion not offered or tried yet, of the offer's lowest Tier or higher, with a gift the offer hasn't given), until one has a gift; if the pool has none, the offer has one option fewer **(engine decision)**.
4. The options keep their draw order (no shuffle) **(engine decision)**. Each option has the player's "wherever this is" auras applied (`apply_global_unit_auras`), as every Discover option does.

`dark_gifts::draw_offer(state, pool, rng, rules)` takes the turn and the Tiers from `OfferRules` so the heroes and trinkets reuse it (§12): `OfferRules::button(turn)` is §4.1.

### 4.4 Only Implemented Gifts

Only implemented gifts are offered (§11) **(engine decision)**. With few implemented gifts, more minions are filtered out (§4.1 rule 6).

---

## 5. Which Gift a Minion Can Be Offered

A gift can be offered on a minion, on turn `turn`, to a player, if every rule below holds. They are the post's rules, as written **(confirmed)**, with the data's numbers **(confirmed)**; where the post's and the data's windows differ, the data's apply (§11 lists them).

### 5.1 General Rules

1. **Turn window**: `min_turn <= turn <= max_turn` (data).
2. **Battlecry and Choose One minions** can only be offered `Double Vision`, `Replication`, `Gilding` and `Echoing Voice` (each gift's `GIFT_FOR_BATTLECRY` flag).
3. **Deathrattle minions** can't be offered a gift that gives stats, but `Sharpened Sword` and `Death's Embrace` (the `GIFT_GIVES_STATS` flag; the stat gifts are listed in §11) **(engine decision: which gifts give stats)**.
4. **The most triggered**: among `Battle Scars`, `Death's Embrace` and `Spell Siphon` (the gifts with a `gift_trigger_count` hook: Battlecries triggered, Deathrattles triggered, Tavern spells cast this game), only the one(s) whose count is the highest can be offered, and none with a count of 0 (Hotfix 36.2.1).
5. **Lobby bans**: `Toughened Shield` not in a Quilboar (or Naga) lobby; `Resistance` and `Transcendence` not in a Dragon lobby (Hotfix 36.2.1). A lobby has a type if its pool has a minion of that type alone (`CardPool::lobby_has`) **(engine decision)**: a full-catalog game has every type, so these three gifts are never offered there.
6. **The gift's own rules** (its `gift_offerable` hook, §5.2).

### 5.2 Each Gift's Rules (the post; Hotfix 36.2.1)

| Gift | Rule |
| :--- | :--- |
| `Sunken Persistence` | Only on Spellcraft minions. |
| `Jaws of Death` | Only on Deathrattle minions. |
| `Affinity`, `Dexterity`, `Incubation`, `Offensive Sacrifice`, `Defensive Sacrifice` | Only on minions with a type. |
| `Sharpened Sword` | Not on Avenge minions. |
| `Time Turning` | Only on minions with end of turn effects. |
| `Furtiveness` | Only on Avenge minions. |
| `Consanguinity` | Only on Quilboar. |
| `Gilding` | Only on the lowest-Tier minion offered; not on Activate minions. |
| `Toreth's Blessing` | Only on Divine Shield minions. |
| `Amalgamation`, `Transcendence` | Only on minions with no type. |
| `Demonology` | Only on Demons. |
| `Polarization` | Only on Mechs; not below Tavern Tier 3. |
| `Tarecgosa's Blessing` | Only on Dragons. |
| `Echoing Voice` | Only on Battlecry minions. |
| `Toxicity` | Only on Murlocs; not on minions that already have Venomous (or Poisonous). |
| `Charisma` | Not on Taunt or Avenge minions; not on Tier 7 minions. |
| `Resistance` | Not on Beasts, Undead, or minions with no type. |
| `Hostility` | Not on Avenge minions, Venomous (or Poisonous) minions, or minions with no type. |
| `Golemancy` | Not on minions with no type; not on `Stitched Salvager`. |
| `Persisting Horror` | Not on minions with no type. |
| `Titanic Strength` | Not on Dragons. |
| `Invulnerability` | Not on Taunt minions or minions with no type. |
| `Worship` | Only for a player with a Deity (`auras.deity.kind` isn't `None`) in a lobby with Aberrations **(engine decision: the post predates it)**. |
| `Efficiency` | Only on Activate minions **(engine decision)**. |
| `Desecration` | Only once the player discarded a card this game **(engine decision)**. |
| `Doppelganger` | No rule of its own **(engine decision)**. |

What a minion is: "with a type" — its tribe isn't `None` (an `All` minion has every type); "a Demon" — `Tribe::matches`; a Battlecry / Choose One / Deathrattle / Rally / Activate / Spellcraft / end of turn minion — its card has that hook (`battlecry`, `choose_one`, `deathrattle`, `rally`, `activate`, `spellcraft`, `end_of_turn`); an Avenge minion — its card has the `AVENGE` flag; Taunt, Divine Shield, Venomous — its card's printed keyword. The minion is judged as a plain card (its template), not as the copy in the pool **(engine decision)**.

---

## 6. Hooks

Gifts reuse the card hook system (`src/cards/hooks.rs`): a gift is a card whose `CardHooks` table the registry holds under its dbfId. Its hooks are invoked for the minion that has it (`Unit::dark_gifts`), in the order the minion got its gifts. The **Dark Gifts** section of `CardHooks`:

| Hook | Fires | For |
| :--- | :--- | :--- |
| `gift_given` | The gift is given to a minion (§2.2): `(unit, gift)`. | Keywords, stats, Golden, a type. |
| `gift_reapply` | What it keeps giving its minion, applied again to the Golden a Triple made: `(unit)`. | `Amalgamation`. |
| `gift_offerable` | Its own offer rules (§5.2, and its lobby ban, §5.1 rule 5): `(ctx, minion)`, an `OfferCtx` (the turn, the player's auras and Tier, the lobby's types, the offer's lowest Tier). | §5.2. |
| `gift_weight_factor` | The factor on its weight for a minion: `(minion)`. | `Harpy's Talons`. |
| `gift_trigger_count` | Its "most triggered" count (§5.1 rule 4): `(auras)`. | `Battle Scars`, `Death's Embrace`, `Spell Siphon`. |
| `gift_aura` | The stats the gift gives the minion wherever it is, kept in sync with the other auras (`Unit::aura_applied`): `(unit, gift, auras)`. | `Battle Scars`, `Desecration`, ... |
| `gift_obtained` | The minion was Discovered (`cards::on_obtained`): `(state, card, pool, rng)`. | `Double Vision`. |
| `gift_card_played` | The player plays a card from the hand (a minion or a spell, as it leaves the hand), for each board minion with the gift: `(state, pos, gift_idx)`. | `Sharpened Sword`, `Dexterity`. |
| `gift_end_of_turn` | The player ends their turn, after the minions' end-of-turn triggers, for each board minion with the gift, left to right: `(state, pos, gift_idx, repeats, pool, rng)`. `repeats` is the end-of-turn repeat count (`tavern.md` §7.5): the payoff repeats, a turn counter advances once. | `Steady Growth`, `Replication`, `Echoing Voice`. |
| `gift_turn_start` | Start of turn, after the minions' own start-of-turn effects, for each minion with the gift on the board and in the hand: `(state, slot, gift_idx, pool, rng)`. | `Incubation`, `Time Turning`. |
| `gift_start_of_combat` | The minion's Start of Combat trigger (§7.2): `(ctx, unit_id, gift_idx)`. | `Jaws of Death`, `Hostility`, ... |
| `gift_deathrattle` | The minion's Deathrattles trigger, after its card's and the ones an effect gave it: `(dying, gift, ctx)`. | `Golemancy`, `Fresh Perspective`. |
| `gift_rally` | The minion's Rally, after its card's: `(ctx)`, a `RallyCtx`. | `Charisma`, `Consanguinity`. |
| `gift_reborn` | The minion is Reborn: the Reborn copy, given the dying minion: `(dying, copy)`. | `Persisting Horror`. |
| `gift_extra_activations` | Extra times its `Activate` triggers: `(unit)`. | `Efficiency`. |
| `gift_combat_start` | A combat starts (its minion's combat copy is set up): `(gift)`, to restart a per-combat count. | `Toreth's Blessing`. |
| `gift_absorb_shield_hit` | Its minion's Divine Shield is hit in combat (`apply_damage`): `true` if the gift absorbs the hit (the shield stays), using up its count: `(gift)`. | `Toreth's Blessing`. |
| `gift_keep_combat_mult` | The multiplier with which the minion keeps the stats it gained in combat (`cards::keep_combat_gains`; the largest of its card's, its neighbours' and its gifts'): `(unit)`. | `Tarecgosa's Blessing`. |

Flags (`CardFlags`) mark the rest: `GIFT_GIVES_STATS`, `GIFT_FOR_DEATHRATTLE` and `GIFT_FOR_BATTLECRY` (§5.1), `GIFT_IMMUNE_ATTACKING` (its minion takes no damage while attacking: `Invulnerability`) and `GIFT_KEEPS_SPELLCRAFT` (its minion's Spellcraft spell is not temporary: `Sunken Persistence`). `AVENGE` marks the Avenge minions (§5.2), `DARK_GIFTS` the Hero Powers and trinkets offered only with Dark Gifts (§12).

The dispatch functions (`dark_gifts::on_end_of_turn`, `dark_gifts::start_of_combat`, ...) run each gift's hook in turn; the engines call them next to the matching card dispatch. A minion with a gift that has a `gift_start_of_combat`, `gift_deathrattle` or `gift_rally` hook has a Start of Combat, a Deathrattle or a Rally for the engine (`cards::has_start_of_combat`, `cards::has_deathrattle`, `cards::has_rally`): it is listed at Start of Combat, `Titus Rivendare` and `Herald Sticker` repeat or trigger its gift's Deathrattle, and it is a Rally minion for `after_friendly_rally` **(engine decision)**.

The core (`tavern.rs`, `combat.rs`, `cards/mod.rs`) never names a gift; `dark_gifts/mod.rs` holds the offer rules and the dispatch, and each gift's file its text.

---

## 7. Combat

### 7.1 Gifts in Combat

A minion's gifts travel with it into combat (they are part of the `Unit`). As for the rest of a minion, what happens to its combat copy doesn't come back to the Tavern (`combat.md` §1), but for the gains a gift makes it keep (`Tarecgosa's Blessing`, through `cards::on_post_combat_unit`); a gift's per-combat count (`Toreth's Blessing`'s hits) starts again each combat.

### 7.2 Start of Combat

A minion with a Start of Combat gift is listed in step 3 of `combat.md` §5.1.1 like a minion with a Start of Combat trigger. Its trigger resolves its card's `start_of_combat` hook, then its `SocAction`, then each of its gifts' `gift_start_of_combat` hooks in gift order, the deaths resolving after each (step 4) **(engine decision: the card's text before the gifts')**.

### 7.3 Deathrattles and Rallies

A dying minion's Deathrattles are its card's, then those an effect gave it (`Unit::extra_deathrattles`), then its gifts' `gift_deathrattle` hooks in gift order, each counting as a Deathrattle triggered (`auras.deathrattles_triggered`) **(engine decision)**. A Rally is its card's, then its gifts' `gift_rally` hooks; their summons go to the attacker's right.

---

## 8. Duos, Python / RL, Scenarios

### 8.1 Duos

The button is per player **(engine decision)**: each player of a team has their own uses, pays with their own Gold and Discovers into their own hand (`DuosTavern::with_dark_gifts` enables both). `DuosTavern::is_legal` / `step` allow `DarkDiscovery` like every action of the player's own Tavern. A gifted card Passed to the teammate keeps its gifts (§2.2) **(engine decision)**. Each warband carries its minions' gifts into a Duos combat.

### 8.2 Python / RL

- `TavernGame(..., dark_gifts=False)` and `DuosTavernGame(..., dark_gifts=False)` enable the button. The default (`False`) is no button, so existing code is unchanged.
- `TavernAction.dark_discovery()`, whose `kind` is `"dark_discovery"`.
- `Unit.dark_gifts`: the gift ids (dbfIds) on a unit, in order. The observation (`TavernObservation`, `DuosObservation`) appends `dark_gifts_enabled`, `dark_discoveries_left` (uses left this game) and `dark_discovery_usable` (`DarkDiscovery` is legal now).
- `seaglass.dark_gifts_table()`: `(id, name, min_turn, max_turn, weight, implemented)` for every gift. The gift ids are appended to `seaglass.all_card_ids()` after the trinkets, append-only (`dark_gifts::vocabulary` keeps the first table's order; a new gift is appended).
- The RL action table (`rl/seaglass_rl/actions.py`) appends `dark_discovery` at the end.
- The features (`rl/seaglass_rl/features.py`) append to the player vector the button's uses left (/ 3) and whether it is usable now, and to every card slot one feature per gift of the vocabulary (the number of copies of that gift on the card), after the existing slot features (append-only).

`rl/README.md` describes the encoding.

### 8.3 Scenarios

- A gift's scenario file is `tests/scenarios/dark_gifts/<slug>.yaml`, starting with `dark_gift: <display name>` (`dark_gift: Battle Scars (late)`).
- A unit spec token `gift:<gift>` (its slug or card ID: `"Wrath Weaver gift:harpys_talons"`) gives the unit that gift in a setup (`dark_gifts::give`, with its `gift_given` text, as on turn 0; `gift:<gift>@<turn>` gives it as on that turn), and in an expectation checks that the unit has it (`-gift:<gift>`: that it doesn't). A unit's view has its `dark_gifts`.
- `dark_gifts_enabled: true` on a Tavern enables the button; the step `dark_discovery: true` takes the action, and `offer_dark_gifts: [<unit spec>, ...]` offers those minions (their gifts given by `gift:` tokens) as a Discover without the button, for the tests of a gift's text.

---

## 9. The Gift Table and Its Generator

`src/dark_gifts/table.rs` is generated; rerun it each patch:

```sh
python3 tools/gen_dark_gifts.py path/to/CardDefs.xml
```

It reads every spell with `IS_NIGHTMARE_BONUS` (tag 3567) whose card ID starts with `BG36_MidGameEffect_000t` (§2.1) and writes them sorted by name, then first turn, then card ID. Two gifts sharing a name are `<name> (early)` and `<name> (late)` (the one with the earlier first turn is early).

Each implemented gift has its own file, `src/dark_gifts/<slug>.rs` (`battle_scars_late.rs` for `Battle Scars (late)`), whose first lines are its header:

```text
//! `Fortitude` (`BG36_MidGameEffect_000t73`) — turns 3–3, weight 100.
//!
//! +5/+5.
```

(`turns 5–` for no last turn, `turns –` for no window.) The `dark_gifts` tests check that every gift file's header matches the table (name, card ID, window, weight and text), that its `ID` constant is the table's dbfId, that it is in `IMPLEMENTED`, and that it has a scenario file `tests/scenarios/dark_gifts/<slug>.yaml` starting with `dark_gift: <display name>`.

---

## 10. Replay

The Power.log validator (`replay/src/{reconstruct,validate}.rs`) reads each combat's minions with their attached enchantments. An enchantment that is a gift's (`dark_gifts::by_enchantment`: `BG36_MidGameEffect_000t9e2` → `Admiration`) gives the reconstructed minion that gift (`Unit::dark_gifts`, without its `gift_given` text: the log's stats and keywords already include it), so `replay_cli validate` simulates the implemented gifts; an unimplemented gift that triggers in the combat stays a gap. A gift's counters come from the enchantment's tags where the engine reads them (`Steady Growth`'s strength doesn't matter in combat). In a log, a Dark Discovery is a `BG36_MidGameEffect_010` Discover of 3 minions, each with a gift's enchantment; the button (`BG36_Button_DarkGift`) unlocks at `TURN 5` (round 3: `REQ_MINIMUM_GAME_TURN errorParam=5`).

---

## 11. The Gifts

"This minion" is the minion with the gift. A random card is a new copy from the game's catalog (not the pool) of the player's Tier or lower, as for trinkets (`trinkets.md` §11) **(engine decision)**. A card a gift gives in the Tavern waits for room in a full hand (`add_to_hand_or_wait`); one it gives in combat goes to the combat hand. **Stats** marks the gifts that give stats (§5.1 rule 3). Windows are the data's (Patch 36.6.3); the post's differ where noted.

### 11.1 Combat Gifts

| Gift | Window, weight | Text | How the engine reads it |
| :--- | :--- | :--- | :--- |
| `Harpy's Talons` | –, 100 | Divine Shield, Windfury | Given: Divine Shield and Windfury. (Post: from turn 3.) |
| `Furtiveness` | 4–, 100 | Stealth | Given: Stealth. |
| `Toxicity` | 9–, 100 | Venomous | Given: Venomous. (Post: from 7.) |
| `Titanic Strength` | 11–, 50 | +1000 Attack. | Given: +1000 Attack. Stats. |
| `Fortitude` | 3–3, 100 | +5/+5. | Given: +5/+5. Stats. (Post: +4/+4.) |
| `Amalgamation` | 8–, 200 | Has all minion types. | Given: its type becomes `All`. (Post: from 5.) |
| `Gilding` | 4–8, 100 | This is Golden, but doesn't give a Triple Reward. | Given: made Golden (`Unit::make_golden`) as an intrinsic Golden (`intrinsic_golden`: no Triple Reward, one pool copy, never in a Triple). (Post: rarer; the data's weight is 100.) |
| `Offensive Sacrifice` | 6–9, 100 | +10 Attack. Deathrattle: Give this minion's Attack to another friendly minion. | Given: +10 Attack. Deathrattle: a random other living friendly minion (one draw) gains its Attack as it died. Stats. |
| `Defensive Sacrifice` | 6–9, 100 | +10 Health. Deathrattle: Give this minion's max Health to another friendly minion. | Given: +10 Health. Deathrattle: as `Offensive Sacrifice`, its maximum Health (`max_health`). Stats. |
| `Fresh Perspective` | 3–5, 100 | Deathrattle: Gain 2 free Refreshes. | `auras.free_refreshes` +2. (Post: 4–5.) |
| `Mystic Essence` | 5–8, 100 | Deathrattle: Get a random Tavern spell. | A spell of the player's Tier or lower, drawn as the shop draws one. |
| `Golemancy` | 9–11, 100 | Deathrattle: Summon a Golem with this minion's stats. | A Golem token with its Attack and maximum Health as it died, in its place. (Post: 9–10.) |
| `Jaws of Death` | 5–, 200 | Start of Combat: Trigger this minion's Deathrattles. | Its Deathrattles (its card's, given ones, its gifts') trigger as `Deathstrider` makes them (repeated by `Titus Rivendare`), summons to its right. (Post: from 3.) |
| `Admiration` | 7–, 100 | Start of Combat: Gain the Attack of the minion to the left. | The friendly minion to its left as Start of Combat reaches it; nothing for the left-most. Stats. |
| `Transcendence` | 8–, 200 | Start of Combat: Triple this minion's stats. | It gains twice its Attack and Health. Stats. (Post: from 7.) |
| `Resistance` | 7–10, 100 | Start of Combat: Double this minion's Health. | It gains its Health. Stats. |
| `Hostility` | 7–10, 100 | Start of Combat: Double this minion's Attack. | It gains its Attack. Stats. |
| `Worship` | 12–, 50 | Start of Combat: Gain the stats of your Deity. | `auras.deity`'s Attack and Health. Stats. (Not in the post.) |
| `Doppelganger` | 6–8, 100 | Start of Combat: Summon a copy of this. | A copy as it is (its gifts too) to its right, if there is room; the copy's own Start of Combat doesn't trigger (it wasn't listed). (Not in the post.) |
| `Consanguinity` | 3–5, 200 | Rally: Get 2 Blood Gems. | 2 `Blood Gem`s to the hand (the combat hand in combat). (Post: 4–5.) |
| `Demonology` | 5–8, 200 | Rally: Add a Fodder to your next 3 Refreshes. | `tokens::add_fodder_to_refreshes` for 3 Refreshes. |
| `Charisma` | 7–11, 100 | Rally: Get a random minion of your most common type. | As `Dark Paradox`'s Rally: a random minion of Tier 6 or lower of the board's most common type. (Post: 6–9 after 36.2.1.) |
| `Toreth's Blessing` | 5–, 200 | This minion's Divine Shield takes 3 hits to break. | Each combat, its Divine Shield absorbs 2 hits without breaking (`gift_shield_hits`), the third breaks it. (Post: 2 more hits.) |
| `Invulnerability` | 12–, 50 | Immune while attacking. | When it attacks, it takes no damage from its target (a Divine Shield it has stays). |
| `Persisting Horror` | 10–, 50 | Reborn. Is Reborn with full stats and Bonus Keywords. | Given: Reborn. Its Reborn copy is the minion as it died, with its Attack and full Health (`max_health`) and its Bonus Keywords but Reborn, and its gifts **(engine decision)**. |
| `Tarecgosa's Blessing` | 6–, 200 | Permanently keeps Bonus Keywords and double stats gained in combat. | `gift_keep_combat_mult` 2 (`cards::keep_combat_gains`). |
| `Battle Scars` (early / late) | 4–6 / 7–10, 100 | Has +2/+2 (+3/+3) for each Battlecry you've triggered this game. | `gift_aura`: per `auras.battlecries_triggered` (every friendly Battlecry trigger in the Tavern, counted from the start of the game). Stats. |
| `Death's Embrace` (early / late) | 4–6 / 7–10, 100 | Has +1/+1 (+2/+2) for each Deathrattle you've triggered this game. | Per `auras.deathrattles_triggered`. Stats. |
| `Spell Siphon` (early / late) | 5–6 / 7–10, 100 | Has +2/+2 (+3/+3) for each Tavern spell you've cast this game. | Per `auras.tavern_spells_cast`. Stats. (Post: early from 4.) |
| `Desecration` | 5–7, 100 | Has +3/+3 for each card you've discarded this game. | Per `auras.cards_discarded`. Stats. (Not in the post.) |

### 11.2 Tavern Gifts

| Gift | Window, weight | Text | How the engine reads it |
| :--- | :--- | :--- | :--- |
| `Sharpened Sword` | 3–6, 100 | Whenever you play a card, gain +3 Attack. | `gift_card_played`: on the board, each card played from the hand (a minion or a spell; not this minion's own play) **(engine decision: the board only)**. Stats. (Post: +2, 3–5.) |
| `Toughened Shield` | 3–6, 100 | Whenever you play a card, gain +3 Health. | As `Sharpened Sword`, Health. Stats. |
| `Dexterity` (early / late) | 7–8 / 9–10, 100 | Whenever you play a card, gain +2/+2 (+4/+4). | As `Sharpened Sword`. Stats. |
| `Steady Growth` | 3–5, 100 | At the end of your turn, gain +{1}/+{2}. | `+1/+2` given on turn 3, `+2/+2` on 4, `+3/+3` on 5 (the post's numbers; the strength is the offering turn, `stacks`). Stats. |
| `Incubation` | 6–8, 100 | +4/+4. In 2 turns, double this minion's stats. | Given: +4/+4. At the 2nd start of turn after it is given, on the board or in the hand, it gains its Attack and Health, once. Stats. |
| `Affinity` | 3–4, 100 | At the end of every 2 turns, get a random minion of this type. | Every 2nd end of turn on the board (`counter`), a random minion of one of its types (a uniform pick among them for several). |
| `Replication` | 4–7, 100 | At the end of every 2 turns, get a plain copy of this. | Every 2nd end of turn on the board: a plain copy (no gifts, `cards::instantiate_plain_copy`). (Post: 4–6.) |
| `Echoing Voice` | 6–, 100 | At the end of your turn, trigger this minion's Battlecries. | `cards::trigger_board_battlecry` on it. |
| `Polarization` | 5–9, 200 | At the end of your turn, Magnetize a random Mech to this. | A random Magnetic Mech of the player's Tier or lower from the catalog, Magnetized by an effect (`cards::magnetize`). (Post: 5–8.) |
| `Time Turning` | 5–8, 200 | This minion's end of turn effects also trigger at start of turn. | `gift_turn_start`: on the board, its card's `end_of_turn` hook and its other gifts' `gift_end_of_turn` hooks, once each, after the start of turn's other effects. (Post: 3–9.) |
| `Double Vision` | 5–, 100 | Get an extra copy of this. | `gift_obtained`: when it is Discovered, a copy of it as offered without `Double Vision`, to the hand (not from the pool) **(engine decision)**. |
| `Efficiency` | 5–, 200 | This minion's Activate triggers twice. | Its `Activate` triggers once more (one price). (Not in the post.) |
| `Sunken Persistence` | 3–, 200 | This minion's Spellcrafts are permanent. | The Spellcraft spell it gives is not temporary: it stays in the hand at the end of the turn **(engine decision)**. |

### 11.3 Implemented

The gifts listed in `IMPLEMENTED` (`src/dark_gifts/mod.rs`) are implemented and offered: all 47 gift cards of §11.1 and §11.2. Each has a scenario under `tests/scenarios/dark_gifts/` that fails with the gift knocked out. `Sunken Persistence` is implemented, but no offer gives it: it goes only on Spellcraft minions, and no minion pool of Patch 36.6.3 has one.

---

## 12. Heroes, Trinkets and Minions with Dark Gifts

The two heroes and four trinkets below exist only in games with Dark Gifts: their cards are flagged `CardFlags::DARK_GIFTS`, and a game offers them (a hero offer, a Discover of Hero Powers, a trinket offer) only to a player with `dark_gifts_enabled` (`heroes::offerable(duos, dark_gifts)`, `heroes::offerable_powers`, `trinkets::candidates`) **(engine decision: so that a game without Dark Gifts draws its heroes and trinkets as before)**. A scenario or a Python game can still give them to anyone by name.

### 12.1 `Nightmare Lord Xavius` — Feel Devastation

"Every 4 turns, Discover a minion with a Dark Gift. (4 turns left!)" Passive: at the start of every 4th turn (turns 4, 8, 12, ...), in its `hero_turn_start` hook (with the other Hero Powers' start of turn effects, before the trinket offer and the gifts' start of turn), a Dark Discovery offer by the button's rules for that turn (§4, **confirmed**: the same Tiers and filters), free, without using the button **(engine decision: the start of turn, counted from turn 0)**.

### 12.2 `Tras'tath, Soul Parasite` — Void Power

"At the start of the game, Discover a Tier 5 minion with a Dark Gift. It unlocks on Turn 7. (6 turns left!)" On the hero's first turn, an offer of 3 Tier 5 minions, each with a different gift from `Tras'tath`'s pool (the gifts with `trastath`: the post's list; the data drops `Toxicity` and adds `Doppelganger`), whatever their turn windows (the post), the other rules of §5 holding for turn 7 **(engine decision)**. The chosen minion is locked in the hand until turn 7 (`Unit::locked_turns`: it can't be played, sold from the hand or Passed) **(engine decision)**.

### 12.3 Trinkets

Each offer is a `dark_gifts::discover` by `OfferRules` of its own (the minion filters and gift windows of the current turn), but `Maldraxxus Dagger`'s.

| Trinket | Text | How the engine reads it |
| :--- | :--- | :--- |
| `Ominous Stone` (Lesser) | Discover a Tier 4 minion of your most common type with a Dark Gift. | As picked: an offer of Tier 4 minions, all of the most common type (as far as the pool has them), gifts by §5 for the turn **(engine decision)**. |
| `Sinister Invitation` (Greater) | Discover a Golden minion of your most common type with a Dark Gift. It doesn't give a Triple Reward. | As picked: an offer by the button's Tiers for the turn (§4.1), all of the most common type (as far as the pool has them), each made Golden as an intrinsic Golden (no Triple Reward) **(engine decision)**. |
| `Maldraxxus Dagger` (Greater) | Discover a plain copy of a friendly minion with a Dark Gift. Repeat at the start of every 2 turns. | As picked and every 2nd turn start after (the trinket's `counter`): an offer of up to 3 different board minions (distinct cards, drawn uniformly), each a plain copy taken out of the pool as a Discover's minion is (a card with no copy left in the pool isn't offered; the unchosen ones go back to the pool, as every Discover's do), each with a different gift by §5 for the turn; a minion no gift fits is dropped (`dark_gifts::gift_options`) **(engine decision)**. |
| `Wax Lance` (Greater) | Discover a Tier 7 minion with a Dark Gift. | As picked: an offer of Tier 7 minions from the pool (the full catalog's 12 Tier 7 minions, 5 copies each), gifts by §5 for the turn. |

### 12.4 `Dark Paradox`

"This has a different Dark Gift, stats, and Tier each game!" The engine's `Dark Paradox` is one fixed form of the card (`BG36_360t5`, Tier 4 `2/6`, "Rally: Get a random minion of your most common type"): the `Charisma` gift is its card text, which a Golden doubles (`BG36_360_Gt5`: 2 minions). It stays as it is **(engine decision)**: its other forms are deferred.

---

## 13. Decisions

The rules the user settled are marked **(confirmed)** above: the button for every player in every game (3 Gold, from turn 3, once per turn, 3 uses per game, 3 minions each with a different gift); the Tier by turn table, for Xavius too; the minion filters (no Battlecry or Choose One before turn 5, no Magnetic, no "when you sell this", none meant to stay in the hand, one of the most common type from turn 6, the banned minions); a gift's window being the game turn; the post's per-gift rules as written; several gifts on a minion, combined on a Triple; the anomaly out of scope.

Engine decisions, pending the user's confirmation:

| Decision | Rule | Where |
| :--- | :--- | :--- |
| The flag | `dark_gifts_enabled`, off by default; on in half the fuzz games. | `tavern.rs`, `python.rs` |
| The draw | Minions first, then a gift each by weight, none twice (agreed); minions from the pool like a Discover (agreed); a uniform Tier per minion; the turn's other Tiers if one has none; no shuffle (§4.3). | `dark_gifts::draw_offer` |
| Most common type | The board's, ties at random (`spells::most_common_tribe`) (§4.1). | `dark_gifts::draw_offer` |
| Filters | "Meant to stay in hand" = a hand hook; the ban list adds `Dark Paradox`; a minion no gift fits is not drawn; an option left without a gift drops (§4.1, §4.3). | `dark_gifts` |
| Harpy's Talons | Weight ×2 on Rally minions (§4.2). | `harpys_talons.rs` |
| Stat gifts | The list of §11 (§5.1 rule 3). | the gifts' flags |
| Lobby types | A type the pool has a minion of alone (§5.1 rule 5). | `CardPool::lobby_has` |
| New gifts | `Worship`, `Efficiency`, `Desecration` need their mechanic; `Doppelganger` has no rule (§5.2). | their files |
| Gifts on a minion | Kept wherever the minion goes, on exact copies and Passes; not on plain or Reborn copies; no doubling for Golden; tripled stats are buffs (§2.2). | `model.rs`, `cards` |
| Price and empty offers | Gold spent like any price; an empty offer still costs (§3.2). | `dark_gifts::use_button` |
| Order | A minion's card's Start of Combat, then its gifts'; Deathrattles and Rallies the same (§7). | `cards` |
| Gift readings | §11's "engine decision"s. | their files |
| Cards a gift gets | `Affinity`, `Replication`, `Double Vision` and `Mystic Essence` add to the hand like any card a minion gets: lost with a full hand; the Rally gifts' cards go to the hand as a Rally minion's do. | their files |
| `Steady Growth` | The strength by offering turn: `+1/+2`, `+2/+2`, `+3/+3` on turns 3, 4, 5 (the post's numbers; the card's text has no tag for them). | `steady_growth.rs` |
| Heroes, trinkets | §12. | their files |
| Their offers | The Dark Gift heroes and trinkets are offered only with Dark Gifts (§12). | `CardFlags::DARK_GIFTS`, `heroes::offerable`, `trinkets::candidates` |
