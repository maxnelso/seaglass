# Wheel of Yogg-Saron Specification

This document specifies the Battlegrounds **Wheel of Yogg-Saron** in Seaglass: the spin, its six
outcomes and their odds, and the trinket that spins it (`Yogg-Tastic Pastry`). It builds on
[`tavern.md`](tavern.md), [`trinkets.md`](trinkets.md) and
[`darkmoon_prizes.md`](darkmoon_prizes.md), and like them it is updated **before** behavior
changes. The engines stay card-agnostic: what an outcome does lives in its own file
(`src/yogg/<slug>.rs`) and reaches the engine only through the card hooks every spell uses.

Rules marked **(confirmed)** were settled with the user. Rules marked **(engine decision)** are
details nobody has settled yet: the engine implements them as written, each in one place, so that
it is a small change if the answer differs (§7).

**A player who never spins the Wheel** (no `Yogg-Tastic Pastry`: the default) plays exactly as
before this document: no new legal action, no RNG draw, no new field in any serialized state. Solo
and Duos results are byte-identical for them. (A lobby's trinket offers draw among one more
trinket once the Pastry is implemented, as for every trinket added before.)

---

## 1. Scope

### Included
- The Battlegrounds Wheel: its outcome table (`src/yogg/mod.rs`, §2), the spin
  (`yogg::spin_wheel`, §3), and each outcome's effect (§4).
- `Yogg-Tastic Pastry` (`BG30_MagicItem_994`, Lesser, 5 Gold) (§5).

### Deferred
[`deferred.md`](deferred.md) is the engine's single list of what it defers. Other cards spin the
Wheel; they reuse `yogg::spin_wheel` once they are in:
- `Acolyte of Yogg-Saron` (`TB_BaconShop_HERO_35_Buddy`), Yogg-Saron's Buddy: Buddies are
  deferred **(confirmed)**.
- `Timewarped Acolyte` (`BG34_Giant_591_G`): Timewarp is deferred **(confirmed)**.
- `Jaws of Fate` (`BG28_870`), a Tier 5 Tavern spell: left out by the user **(confirmed)**.
- `Mysterybox` (`TB_BaconShop_HERO_35_Buddy_t1`, "your Hero Power triggers an extra time"), the
  older first outcome: the Wheel uses the newer `Mysterybox` (`_t7`) **(confirmed)**.

---

## 2. The Outcomes

The outcomes are the `Acolyte of Yogg-Saron` Buddy's token spells in `CardDefs.xml`. Each is a
row of `yogg::OUTCOMES`, in this order, and has its own file under its dbfId (§6):

| Outcome | Card ID | dbfId | Weight | Text |
| :--- | :--- | ---: | ---: | :--- |
| `Mysterybox` | `TB_BaconShop_HERO_35_Buddy_t7` | 93390 | 19 | Make a random minion in the Tavern Golden. |
| `Hand of Fate` | `TB_BaconShop_HERO_35_Buddy_t2` | 80434 | 19 | Get 2 random Darkmoon Prizes. |
| `Curse of Flesh` | `TB_BaconShop_HERO_35_Buddy_t3` | 80435 | 19 | A random friendly minion gains the stats of another friendly minion. |
| `Devouring Hunger` | `TB_BaconShop_HERO_35_Buddy_t5` | 80437 | 19 | Consume all minions in the Tavern and give their stats to your minions. Then Refresh it. |
| `Mindflayer Goggles` | `TB_BaconShop_HERO_35_Buddy_t8` | 93391 | 19 | Cast 4 random Tavern spells. |
| `Rod of Roasting` | `TB_BaconShop_HERO_35_Buddy_t6` | 80438 | 5 | Cast 'Pyrobuff' randomly to give minions +10/+10 until one hits your bartender or hero. |

The weights are the odds in percent: 19% each, 5% for `Rod of Roasting` **(confirmed)**. The text
is HearthstoneJSON's, without its markup and line breaks. (`Mindflayer Goggles` shares its name
with a Darkmoon Prize, `BGS_Treasures_039`, a different card: the outcomes are not in the card
index, so names never collide.)

The outcomes are never cards in a hand, a shop or a pool: they are only resolved by a spin, so
they are not in the RL vocabulary.

---

## 3. The Spin (`yogg::spin_wheel`)

1. **One draw**: `r = rng.below(100)`, the sum of the weights; the outcome is the first row of the
   table whose cumulative weight exceeds `r` (`Mysterybox` for `0..19`, ..., `Rod of Roasting`
   for `95..100`) **(engine decision: one draw, in table order)**.
2. **The outcome resolves at once** (`yogg::resolve`): its spell's `cast` hook, for the player who
   spun. The outcome itself is **not a spell the player casts** **(engine decision)**: no spell
   counter or spell-cast observer sees it (the Tavern spells `Mindflayer Goggles` casts are casts:
   §4).
3. A spin is a Tavern effect: it happens in the Tavern, for one player (in Duos, the player who
   spun; Duos spells in Duos).

---

## 4. What Each Outcome Does

| Outcome | How the engine reads it |
| :--- | :--- |
| `Mysterybox` | A random minion in the Tavern becomes Golden **(confirmed)**, as `Golden Touch` (`spells::golden_touch::cast`): one `rng.below(k)` among the non-Golden minions in the Tavern (spells don't count), nothing without one; `make_golden` (a Golden made by an effect: played, it still gives a Triple Reward). |
| `Hand of Fate` | 2 random Darkmoon Prizes **(confirmed)**: each one `rng.below(k)` among every implemented prize of any Tier (the 32, `All-Seeing Elder` included), two independent draws (the same prize can come twice) **(engine decision)**. Each is a new prize card (`darkmoon::make_prize`, with the player's "wherever this is" auras) that waits for room in a full hand, as the cards trinkets give (`trinkets.md` §5) **(confirmed)**. |
| `Curse of Flesh` | A random friendly minion gains the current Attack and Health of another random friendly minion, added on top **(confirmed)**: the receiver `rng.below(n)`, then the giver among the others (`rng.below(n − 1)`, none for one) **(engine decision: the order of the draws)**. The giver keeps its stats. Nothing with fewer than 2 minions on the board **(confirmed)**. |
| `Devouring Hunger` | One random friendly minion (`rng.below(n)`, none for one) consumes every minion in the Tavern, left to right, and gains their full stats **(confirmed)**: each is a consume (`trinkets::consume_shop_minion`: it leaves the Tavern, its copy returns to the pool, the consumer gains its Attack and Health, the consume observers see it: `Consuming Claw`). The spells in the Tavern stay. Then the Tavern is Refreshed **(confirmed)**: a free Refresh by a card, which is a Refresh the player does (`refresh_shop_free`, [`tavern.md`](tavern.md) §4.1). With no friendly minion, nothing is consumed and the Tavern is still Refreshed **(engine decision)**. |
| `Mindflayer Goggles` | 4 random Tavern spells of the player's Tier or lower, cast on random targets **(confirmed)**: each as Yogg-Saron's *Puzzle Box* casts one (`spells::cast_random_tavern_spell`, [`heroes.md`](heroes.md) §11): drawn as the shop draws one (the Duos spells in Duos), cast by an effect (`spells::cast_spell_by_effect`: a spell the player casts, which the spell-cast observers and the trinkets see **(confirmed)**); a targeted one on a random valid target (a friendly minion, or a minion in the Tavern for a spell that only targets those; one `rng.below(k)`); with none, that one is not cast **(engine decision)**. The 4 casts are drawn one after the other, each seeing the board the earlier ones left. |
| `Rod of Roasting` | Repeatedly, a random target among the player's minions, their hero and Bob, each equally likely (one `rng.below(n + 2)`, `n` the minions on the board then) **(confirmed)**: a minion gains +10/+10 and the Rod goes on; the hero or Bob ends it **(confirmed)**, and takes nothing **(engine decision)**. The +10/+10 is not a Tavern spell's: the spell bonuses don't add to it **(engine decision)**. With an empty board, the first target ends it. |

---

## 5. `Yogg-Tastic Pastry` (Lesser, 5 Gold)

*Spin the Wheel of Yogg-Saron. At the start of each turn, spin it again.*

- Picked (`trinket_bought`): a spin (§3).
- At each turn start (`trinket_turn_start`, after the Hero Power's turn start, before the
  Spellcraft spells: `trinkets.md` §6): a spin.
- It is offered like every implemented trinket (`trinkets.md` §3).

---

## 6. Files, Tests and Scenarios

- `src/yogg/mod.rs` holds the outcome table (`OUTCOMES`: dbfId, card ID, name, weight, text),
  the spin, `resolve`, and the outcomes' hooks for the card registry (each a 0-Gold spell with the
  flags `NOT_TAVERN_SPELL` and `NOT_IN_POOL`).
- Each outcome has its own file, `src/yogg/<slug>.rs`, whose first lines are its header:

  ```text
  //! `Curse of Flesh` (`TB_BaconShop_HERO_35_Buddy_t3`) — Wheel of Yogg-Saron, 19%.
  //!
  //! A random friendly minion gains the stats of another friendly minion.
  ```

  The `yogg` tests check that every outcome file's header matches the table, that its `ID`
  constant is the table's dbfId, that it has hooks, and that it has a scenario file
  `tests/scenarios/yogg/<slug>.yaml` starting with `wheel: <name>`; that the weights add up to
  100; and that the spin lands on each outcome with its odds.
- Scenarios ([`scenarios.md`](scenarios.md)): the step `spin_wheel` spins the Wheel (§3);
  `spin_wheel: <outcome>` resolves that outcome (`yogg::resolve`), without the draw.
- Knockout (`SEAGLASS_KNOCKOUT=<dbfId>`) disables an outcome: its spin does nothing.

---

## 7. Decisions

| Decision | Rule | Where |
| :--- | :--- | :--- |
| The draw | One `rng.below(100)` over the weights, in table order (§3). | `yogg::spin_wheel` |
| Not a cast | An outcome resolving is not a spell the player casts (§3). | `yogg::resolve` |
| `Hand of Fate` | Any implemented prize of any Tier, the minion prize included; two independent draws. | `yogg/hand_of_fate.rs` |
| `Curse of Flesh` | The receiver drawn first, then the giver. | `yogg/curse_of_flesh.rs` |
| `Devouring Hunger` | No friendly minion: nothing consumed, still Refreshed. | `yogg/devouring_hunger.rs` |
| `Mindflayer Goggles` | Puzzle Box's targets; a spell without a valid target is not cast (it still counts as one of the 4). | `yogg/mindflayer_goggles.rs` |
| `Rod of Roasting` | The hero and Bob take nothing; no spell bonus on the +10/+10. | `yogg/rod_of_roasting.rs` |
