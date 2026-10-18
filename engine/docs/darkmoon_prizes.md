# Darkmoon Prizes Specification

This document specifies Battlegrounds **Darkmoon Prizes** in Seaglass: the 32 prize cards, the
**prize Discover** that gives them, what each prize does when cast, and the hero and trinkets that
give them (`Tickatus`, `Tickatus Sticker`, `Corrupted Tome`). It builds on [`tavern.md`](tavern.md),
[`combat.md`](combat.md), [`heroes.md`](heroes.md), [`trinkets.md`](trinkets.md),
[`duos_tavern.md`](duos_tavern.md) and [`duos.md`](duos.md), and like them it is updated **before**
behavior changes. The engines stay card-agnostic: what a prize does lives in its own file
(`src/darkmoon/<slug>.rs`) and reaches the engine only through the card hooks every spell uses.

Rules marked **(confirmed)** were settled with the user. Rules marked **(engine decision)** are
details nobody has settled yet: the engine implements them as written, each in one place, so that
it is a small change if the answer differs (§11).

**A player who never gets a prize** (no Tickatus, no prize trinket: the default) plays exactly as
before this document: no new legal action, no RNG draw, no new field in any serialized state. Solo
and Duos results are byte-identical for them. (A lobby's hero and trinket offers draw among more
heroes and trinkets once Tickatus and the trinkets are implemented, as for every hero and trinket
added before.)

---

## 1. Scope

### Included
- The prize table (`src/darkmoon/table.rs`), generated from `CardDefs.xml` (§9): the 32 cards with
  tag 1735 (`BATTLEGROUNDS_DARKMOON_PRIZE_TURN`), whose value is the prize's **Tier** (1–4): 31
  spells `BGS_Treasures_*` and one minion, `All-Seeing Elder` (`BG33_300`) **(confirmed: all 32 are
  in the game)**.
- The prize Discover (`darkmoon::discover_prize`, §3), the prize cards (§2.2), each prize's text
  (§6).
- `Tickatus` (`TB_BaconShop_HERO_94`), `Tickatus Sticker` and `Corrupted Tome` with its `Triple
  Prize` (§7).

### Deferred
[`deferred.md`](deferred.md) is the engine's single list of what it defers.
- `Ticket Collector`, Tickatus's Buddy: Buddies are deferred **(confirmed)**.
- The anomalies `Up-Prizing` and `A Faire Reward`: anomalies are deferred **(confirmed)**.
- `Timewarped Big Winner!`: Timewarp is deferred **(confirmed)**.

---

## 2. The Prize

### 2.1 Prize Data

Every prize is a `PrizeData` row of the generated table (`darkmoon::table()`):

| Field | Meaning |
| :--- | :--- |
| `id` | Its dbfId (`65236` for `Gacha Gift`): the prize's engine `CardId` (engine card ids are below 11000, so they never collide). The minion prize's engine minion has the same id. |
| `card_id`, `name` | Its card ID (`BGS_Treasures_004`) and name. |
| `tier` | Its prize Tier, 1–4 (tag 1735). |
| `minion` | `true` for the minion prize (`All-Seeing Elder`), `false` for a spell. |
| `text` | Its text, rendered as for Hero Powers (`heroes.md` §9): the numbers from its tags 2 and 3 (`TAG_SCRIPT_DATA_NUM_1/2`) where the text has `{0}`/`{1}`. |

The catalog API (`src/darkmoon/mod.rs`): `darkmoon::table()`, `data(id)`, `by_name(name)`,
`by_card_id(card_id)`, `slug(p)`, `is_implemented(id)`, `offerable(tier)` (the implemented prizes
of a Tier, in table order), `is_prize(card_id)`, `make_prize(id)` (a new card of the prize).

### 2.2 Prize Cards

- A **prize spell** is a card in the hand (`Unit::is_spell`) that costs **0** to cast and stays in
  the hand until the player casts it **(confirmed)**: it is not temporary, and still in the hand at
  the end of the turn it stays there. It is cast with `Play` like every spell (an untargeted one
  with `board_pos: 0`; a targeted one on a valid target, §6), and a targeted prize with no valid
  target can't be cast yet.
- Casting it is **casting a spell** (the `after_spell_cast` observers, `trinket_after_spell`, ...),
  but **not a Tavern spell** **(confirmed)**: it doesn't count toward `tavern_spells_cast`, isn't the
  last Tavern spell cast, and the `spell_cast` observers don't see it (`CardFlags::NOT_TAVERN_SPELL`).
  So the Tavern spell bonuses (`Intrepid Botanist`, `Crystallization`) don't apply to the stats a
  prize gives.
- No shop or random spell draw offers a prize (`CardFlags::NOT_IN_POOL`), and prizes never come
  from or go back to the card pool.
- The **minion prize** (`All-Seeing Elder`) is a minion card in no pool, like the minions only
  trinkets give (`trinkets.md` §11.9): `cards::trinket_minion_catalog()` holds it. It is played,
  tripled and sold like any minion (selling it returns nothing to the pool).
- `spells::spell_by_id` knows the prize spells (a fresh one), so the effects that copy a spell
  (`Lovely Locket`, Rock Master Voone) copy a prize too.

---

## 3. The Prize Discover

`darkmoon::discover_prize(state, tier, pool, rng)` offers a Discover of prizes of a Tier:

1. **The options**: **3 different prizes of that Tier, equally likely** **(confirmed)**, among the
   implemented prizes of the Tier (`offerable(tier)`): one `rng.below(k)` per option over the list
   left (in table order, without the options drawn), none when one is left **(engine decision:
   uniform draws without replacement)**. Each option is a new prize card (`make_prize`), with the
   player's "wherever this is" auras for a minion. An empty list offers nothing.
2. The offer is a regular Discover (`push_discover`, `ChooseDiscover`): it waits behind a pending
   one, a trinket offer comes first (`trinkets.md` §4).
3. **Choosing**: the chosen prize goes to the hand; the others are discarded (they never came from
   the pool). With a full hand, the chosen prize is lost: a Discover still happens with a full
   hand, and its chosen card meets the full hand like any Discovered card (`tavern.md` §2)
   **(confirmed: the existing rule)**. Like every Discover, it is seen by the Discover observers
   (`on_card_discovered`) and the trinkets (`trinket_after_discover`: `Sinstone Sticker`, `Portable
   Factory`).
4. A prize's own Discovers of prizes (`Big Winner!`) are queued in order.

---

## 4. The Pieces the Prizes Add to the Core

All are generic: no core file names a prize.

### 4.1 Player Effects

A prize whose text lasts ("this game", "this turn", "at the start of each turn") records a player
effect (`PlayerAuras::effects`, its card the prize, `EffectDuration::Game` or `Turn`), which acts
through the prize's `player_*` hooks, as the Tavern spells' next-combat effects do. New hooks:

| Hook | Fires | For |
| :--- | :--- | :--- |
| `player_minion_cost` | The price of a minion in the Tavern (`TavernState::minion_buy_cost`), after the Hero Power's rule, before `next_minion_discount`: `(effect, price) -> price`. | `Friends and Family Discount`. |
| `player_spell_cost` | The price of a spell in the Tavern (`TavernState::effective_spell_buy_cost`), before `next_spell_discount`: `(effect, cost) -> cost`. | `Reserve Prices`. |
| `player_shop_slots` | The minions a rolled shop holds (`TavernState::shop_minion_slots`), after the Hero Power's rule, before the trinkets': `(effect, state, slots) -> slots`. | `New Recruit`. |
| `player_extra_battlecries` | A friendly Battlecry is about to resolve in the Tavern: extra triggers, added to the board's and the trinkets': `(effect) -> u32`. | `Big Brann Play`. |
| `player_end_turn` | The player ends their turn, after the trinkets' `trinket_end_turn` (not repeated by the end-of-turn repeats): `(state, effect, pool, rng)`. | `The Unlimited Coin`. |

The existing `player_turn_start` and `player_after_refresh` hooks serve `Rocking and Rolling`,
`Open Bar` and `New Recruit`.

### 4.2 Unit Fields

- `Unit::end_of_turn_stats`: `+N/+N` the minion gains at the end of each of its player's turns, from
  an effect given to it (`Gruul Rules`). It resolves with the minion's end-of-turn triggers, before
  them, and repeats with them (`tavern.md` §7.5). It is part of the minion: it stays on it in the
  hand, through combats, and when it is copied as it is; a Triple sums the copies'; a plain copy has
  none. Serde leaves it out while it is 0.
- `Unit::not_from_pool`: a Discover option that is a copy rather than a card drawn from the pool
  (`Time Thief`): unchosen, or chosen into a full hand, it returns nothing to the pool. Cleared once
  chosen. Serde leaves it out while it is `false`.

### 4.3 Hero Power Discover Options That Replace the Hero Power

`heroes::offer_power_discover` can mark its options so that the chosen Hero Power **replaces** the
player's Hero Power (`heroes::install_power`, as Sir Finley's does) whatever their Hero Power is
(`Training Session`), as it already can install a second one (Genn).

### 4.4 Trinkets That Change a Card Before It Reaches the Hand

A new trinket hook, `trinket_replace_gained`: a card is about to enter the hand
(`TavernState::add_to_hand`, in the Tavern, and the combat hand's `BoardCtx::add_to_hand`): `(card)
-> Option<card>`, the card it becomes instead (each trinket in pick order sees the card as the
earlier ones left it). `Corrupted Tome`'s Triple Prize (§7.3).

---

## 5. Cards a Prize Gives

A card a prize gives (`Banana Bunch`, `B.A.N.A.N.A.S.`, `Unfurled Codex`, `Gorgeous Goblet`, `The
Unlimited Coin` coming back) waits for room in a full hand instead of being lost, as a trinket's or
a Dark Gift's card does: it joins the player's `waiting_for_hand` queue
(`TavernState::add_to_hand_or_wait`, `trinkets.md` §5) and keeps waiting across turns **(engine
decision: the user's best guess)**. "Fill your hand" gives cards until the hand holds 10 (counting
the cards already waiting for room as taking their slots: with a full hand or cards waiting, it
gives none). A random Tavern spell is drawn as the shop
draws one, among the Tavern spells of the player's Tier or lower that the shop can offer, as for
trinkets' random cards (`trinkets.md` §11) **(engine decision)**.

---

## 6. The Prizes

"A minion" (not "a friendly minion") can also target a minion in the Tavern (`tavern.md` §7.4,
`SpellTarget::Minion`). A Discover of minions "of Tier N" offers 3 different minions of exactly that
Tier from the pool, as a `Triple Reward` does (or of the highest lower Tier with copies left;
`TavernState::offer_triple_reward_of`). "Your Tier" is the Tavern Tier when the prize is cast.

| Prize | Tier | Text | How the engine reads it |
| :--- | :---: | :--- | :--- |
| `Gacha Gift` | 1 | Discover a Tier 1 minion. | A Discover of 3 Tier 1 minions from the pool. |
| `The Good Stuff` | 1 | Give minions in the Tavern +1/+1 this game. | The minions in the Tavern now gain +1/+1, and every minion the Tavern offers later has it (`tavern_all_atk` / `tavern_all_hp`, as `Staff of Enrichment`). |
| `Rocking and Rolling` | 1 | For the rest of the game, gain 1 free Refresh at the start of each turn. | A game-long effect: from the next turn start on, `free_refreshes` +1 each turn start (per cast). Free Refreshes carry over, as every free Refresh does **(engine decision)**. |
| `New Recruit` | 1 | The Tavern offers an extra minion with +2/+2 this game. | A game-long effect: every rolled shop holds one more minion per cast (`player_shop_slots`; never more than 7 cards with the spell), and after each roll (a Refresh, an unfrozen turn start) its right-most minion per cast gains +2/+2 (`player_after_refresh`) **(engine decision: the extra minion is the right-most one; a frozen shop's kept minions, and the minions that fill a frozen shop's free slots, gain nothing)**. |
| `Banana Bunch` | 1 | Get 2 Tavern Dish Bananas. | 2 `Tavern Dish Banana`s (the Tier 1 Tavern spell; `King Mukla`'s Bananas are other cards). |
| `Unfurled Codex` | 1 | Get a random Tavern spell that costs (2) or more. | A random Tavern spell of the player's Tier or lower whose price is 2 Gold or more (Health-cost spells by their Health cost), uniform (§5). |
| `Crystallization` | 1 | Your Tavern spells give an extra +1/+1 this game. | `spell_bonus_atk` / `spell_bonus_hp` +1/+1 (as `Wicked Tome`'s). |
| `Evolving Tavern` | 2 | Replace all cards in the Tavern with ones of a Tier higher. | Each card in the Tavern, left to right, returns to the pool and is replaced in its place: a minion of Tier `t` by a minion of exactly Tier `min(t + 1, 7)` from the pool (the highest lower Tier with copies left if none), with the Tavern's buffs; a spell of Tier `t` by a Tavern spell of exactly Tier `min(t + 1, 7)` the shop can offer (one draw each). Not a Refresh; the new cards aren't frozen **(engine decision)**. |
| `Gruul Rules` | 2 | Give a friendly minion "At the end of your turn, gain +4/+4." | The target's `end_of_turn_stats` +4 (§4.2): +4/+4 at each end of turn, Golden or not. |
| `Time Thief` | 2 | Discover a minion from your last opponent's warband. | A Discover of up to 3 different minions of the last opponent's last warband (`lobby.last_opponent`), as plain copies (not from the pool: `not_from_pool`), one `rng.below(k)` each among the warband's minions whose card isn't offered yet **(engine decision, as Scabbs Cutterbutter's I Spy, `heroes.md` §11.2)**. Without a last opponent or warband, nothing. |
| `On the House` | 2 | Discover a minion of your Tier. | A Discover of 3 minions of exactly the player's Tier from the pool. |
| `The Unlimited Coin` | 2 | Gain 1 Gold. Return this to your hand at end of turn. | 1 Gold, and a turn-long effect: when the player ends the turn, a new `The Unlimited Coin` per cast goes to the hand (`player_end_turn`), or waits for room (§5). |
| `I'm Still Just a Rat in a Cage` | 2 | Give a minion +2 Attack, then double its Attack. | A minion (friendly, or in the Tavern) gains +2 Attack, then its Attack again. |
| `The Bouncer` | 2 | Give a friendly minion Taunt, then double its Health. | Taunt (a granted Taunt), then it gains its Health. |
| `Big Brann Play` | 2 | Your Battlecries trigger an extra time this turn. | A turn-long effect: +1 trigger per cast for every friendly Battlecry this turn (`player_extra_battlecries`), added to Brann's and the trinkets' **(engine decision)**. |
| `Mageroyal Blossom` | 2 | Discover a Tavern spell of your Tier. | A Discover of 3 different Tavern spells of exactly the player's Tier the shop can offer (`spells::draw_discover_tavern_spells_exact_tier`). |
| `Training Session` | 3 | Discover a new Hero Power. | A Discover of 3 different Hero Powers (`heroes::offer_power_discover`: the implemented ones the player doesn't have, as Sir Finley's); the chosen one replaces the player's Hero Power (§4.3). Without a hero, nothing **(engine decision)**. |
| `Buy the Holy Light` | 3 | Give a friendly minion +10 Attack and Divine Shield. | +10 Attack and Divine Shield. |
| `B.A.N.A.N.A.S.` | 3 | Fill your hand with Tavern Dish Bananas. | `Tavern Dish Banana`s until the hand holds 10. |
| `Top Shelf` | 3 | Discover a minion from a Tier higher (up to Tier 7). | A Discover of 3 minions of exactly Tier `min(Tier + 1, 7)` from the pool. |
| `Repeat Customer` | 3 | Return a friendly non-Golden minion to your hand. Give it +6/+6. | A friendly non-Golden minion gains +6/+6 and leaves the board for the hand (not a sale, not a death; its copies stay with it), where it can complete a Triple. With a full hand (a prize cast by a card), it stays on the board with the +6/+6 **(engine decision)**. |
| `All That Glitters` | 3 | Make a random minion in the Tavern Golden. | As `Golden Touch`: a random non-Golden minion in the Tavern becomes Golden (`make_golden`: a Golden made by an effect). |
| `Mindflayer Goggles` | 3 | Steal all cards from the Tavern, then Refresh it. | Each card in the Tavern, left to right, goes to the hand while it has room (gained, not bought: no buy observers, no price); the rest stays. Then a free Refresh that the player does (`refresh_shop_free`, `tavern.md` §4.1) **(engine decision)**. |
| `Reserve Prices` | 3 | Tavern spells cost (1) less this turn. | A turn-long effect: every spell in the Tavern costs 1 less per cast (never below 0; a Health-cost spell 1 less Health) until the turn ends (`player_spell_cost`). |
| `Raise the Stakes` | 4 | Make a friendly minion Golden and return it to your hand. | A friendly non-Golden minion becomes Golden (`make_golden`: it still gives a Triple Reward when played) and goes to the hand as `Repeat Customer`'s does **(engine decision: a Golden minion is no target)**. |
| `Friends and Family Discount` | 4 | Minions in the Tavern cost (2) this game. | A game-long effect: a minion's price is 2 Gold (`player_minion_cost`), whatever the Hero Power's price; `next_minion_discount` and the trinkets still apply after it. |
| `Open Bar` | 4 | Gain 5 free Refreshes. At the start of each turn, gain 5 more. | `free_refreshes` +5 now, and a game-long effect: +5 at each turn start (per cast). |
| `Fresh Tab` | 4 | Gain 12 Gold. | 12 Gold now (it can exceed 10). |
| `Give a Dog a Bone` | 4 | Give a friendly minion Divine Shield, Windfury, and +15/+15. | Divine Shield, Windfury and +15/+15. |
| `Big Winner!` | 4 | Discover a Darkmoon Prize from each of the previous Prize Tiers. | Three prize Discovers (§3): Tier 1, then 2, then 3, queued in that order **(engine decision: the order)**. |
| `Gorgeous Goblet` | 4 | Fill your hand with random Tavern spells. | Random Tavern spells (§5), one draw each, until the hand holds 10. |
| `All-Seeing Elder` | 4 | Start of Combat: Gain the Attack of your left-most minion and the Health of your right-most minion. | A Tier 4 `1/1` minion (Golden `2/2`) in no pool. Start of Combat (its own trigger, `combat.md` §5.1.1 step 3): it gains the Attack of the left-most living friendly minion and the Health of the right-most, itself if it is there; Golden, it gains them twice, each time as they are then **(engine decision)**. |

---

## 7. The Hero and Trinkets That Give Prizes

### 7.1 `Tickatus`

*Prize Wall* (0 Gold, passive): Every 4 turns, Discover a Darkmoon Prize. (4 turns left!)

- At each turn start (`hero_turn_start`), the power's `counter` counts the turn; on the 4th it
  starts again and the player Discovers a prize (§3): on turns 4, 8, 12, 16, ... for a hero chosen
  before turn 1 **(confirmed)**.
- The prize **Tier follows the game turn**: `min(4, max(1, turn / 4))` (rounded down): turns 4–7
  Tier 1, 8–11 Tier 2, 12–15 Tier 3, 16 and later Tier 4 **(confirmed)**. So Tickatus's Tier goes
  up each time: Tier 1 on turn 4, 2 on turn 8, 3 on turn 12, 4 from turn 16 on **(confirmed)**.
- A Prize Wall the player gets later (Sir Finley's or `Training Session`'s Discover) counts its 4
  turns from the turn it arrives (installed mid-turn, its turn start counts that turn) **(engine
  decision)**; its Tier still reads the game turn **(confirmed)**.

### 7.2 `Tickatus Sticker` (Lesser, 2 Gold)

*Discover a Tier 3 Darkmoon Prize. At the start of every three turns, repeat this. (3 turns
left!)* Picked: a Tier 3 prize Discover; then every 3rd turn start after the pick (`counter`: turns
9, 12, ... for one picked on turn 6), another.

### 7.3 `Corrupted Tome` (Greater, 2 Gold) and `Triple Prize`

*Get a Triple Prize. Whenever you would get a Triple Reward, get a Triple Prize instead.*

- Picked: a `Triple Prize` (`BG35_MagicItem_812t`), to the hand or waiting for room
  (`trinkets.md` §5).
- `trinket_replace_gained` (§4.4): every `Triple Reward` card that would enter the player's hand,
  from any source (a Golden played, a Hero Power, a teammate; in the Tavern or the combat hand),
  is a `Triple Prize` instead **(engine decision: any source)**.
- `Triple Prize`: a 0-Gold token spell (not a Tavern spell, in no pool): *Discover a Tier 3 Darkmoon
  Prize.* It always Discovers a **Tier 3** prize, whatever the Tavern Tier **(confirmed)**.

---

## 8. Duos, Python / RL, Scenarios

### 8.1 Duos

Prizes are per player: each player Discovers into their own hand, and their effects are their own.
A prize is a card like any other: it can be Passed to the teammate (`duos_tavern.md` §5), who can
cast it.

### 8.2 Python / RL

- No new action: a prize is cast with `Play`, and its Discover answered with `ChooseDiscover`.
- The prize ids, the `Triple Prize` and the minion prize are appended to `seaglass.all_card_ids()`
  (`vocab::APPENDED`, append-only), so they have card embeddings in the hand and Discover slots.
- Tickatus's turns and Tier are the observation's `hero_power_counter` and `hero_power_stacks`.

### 8.3 Scenarios

- A prize's scenario file is `tests/scenarios/darkmoon/<slug>.yaml`, starting with `prize: <name>`.
  The prize cards are in the card index: a unit spec can name them (`hand: [Gacha Gift]`), and a
  player effect too (`effects: [{card: Open Bar, duration: Game}]`).
- The step `discover_prize: <tier>` offers a prize Discover of that Tier (§3).
- `All-Seeing Elder` also has a combat scenario, `tests/scenarios/combat/all_seeing_elder.yaml`
  (`card: All-Seeing Elder`), like every minion.

---

## 9. The Prize Table and Its Generator

`src/darkmoon/table.rs` is generated; rerun it each patch:

```sh
python3 tools/gen_darkmoon_prizes.py path/to/CardDefs.xml
```

It reads every card with tag 1735 and writes them sorted by Tier, then name (§2.1).

Each implemented prize has its own file, `src/darkmoon/<slug>.rs`, whose first lines are its
header:

```text
//! `Gacha Gift` (`BGS_Treasures_004`) — Tier 1 Darkmoon Prize.
//!
//! Discover a Tier 1 minion.
```

The `darkmoon` tests check that every prize file's header matches the table (name, card ID, Tier
and text), that its `ID` constant is the table's dbfId, that it is in `IMPLEMENTED`, that a spell
prize has hooks (the minion prize has a minion of its id), and that it has a scenario file
`tests/scenarios/darkmoon/<slug>.yaml` starting with `prize: <name>`.

---

## 10. Replay

The Power.log validator reconstructs combats: an `All-Seeing Elder` on a board maps to the engine
minion by its card ID (its module header, as every minion), so `replay_cli validate` simulates it.
Prize spells are Tavern cards and never act in combat.

---

## 11. Decisions

| Decision | Rule | Where |
| :--- | :--- | :--- |
| The draw | Uniform, without replacement, in table order (§3). | `darkmoon::discover_prize` |
| Cards a prize gives | They wait for room in a full hand (the user's best guess); random spells of the player's Tier or lower (§5). | the prizes' files |
| Effects' order | `player_minion_cost` before `next_minion_discount` and the trinkets; `player_shop_slots` before the trinkets; `player_end_turn` last, not repeated (§4.1). | `tavern.rs`, `cards/effects.rs` |
| `Rocking and Rolling`, `Open Bar` | Per cast, from the next turn start; free Refreshes carry over. | their files |
| `New Recruit` | The right-most minion(s) of each roll gain +2/+2; at most 7 cards. | its file |
| `Evolving Tavern` | Left to right, each card returned before its replacement is drawn; up to Tier 7; not a Refresh. | its file |
| `Time Thief` | Plain copies of up to 3 different minions, not from the pool. | its file |
| `The Unlimited Coin` | A new one per cast at end of turn, waiting for room in a full hand. | its file |
| `Big Brann Play` | +1 trigger per cast, added to Brann's. | its file |
| `Training Session` | Nothing without a hero. | its file |
| `Repeat Customer`, `Raise the Stakes` | Buffed / made Golden before it reaches the hand; stays on the board with a full hand; Raise the Stakes needs a non-Golden minion. | their files |
| `Mindflayer Goggles` | Cards taken left to right while the hand has room; then a free player Refresh. | its file |
| `Big Winner!` | Tiers 1, 2, 3 in that order. | its file |
| `All-Seeing Elder` | Itself counts as the left-most or right-most; Golden reads the stats again for its second gain. | its file |
| Tickatus picked later | Counts its 4 turns from the turn it arrives (the Tier reads the game turn: confirmed). | `heroes/tickatus.rs` |
| `Corrupted Tome` | Every Triple Reward entering the hand, from any source, in the Tavern or combat. | `trinkets/corrupted_tome.rs` |
