# Duos Tavern Specification

This is the exact specification of a Battlegrounds **Duos** team's Tavern phase: two players who recruit side by side, share one Health pool, can **Pass** cards to each other, and fight as a team. It builds on the Solo Tavern specification, [`tavern.md`](tavern.md): each player's own Tavern works exactly as there, and Solo Taverns are unaffected by this document. The team's combat is the relay of [`duos.md`](duos.md). Like those documents, it is updated **before** behavior changes, and the engine (`src/duos_tavern.rs`) stays card-agnostic: what a card does lives in its card file and hooks.

Rules marked **(confirmed)** were settled with the user. Rules marked **(engine decision)** are details nobody has settled yet: the engine implements them as written, each in one place, so that it is a small change if the answer differs (§11).

---

## 1. Scope

### Included
- The team (`DuosTavern`): two players' Taverns, the shared Health and Armor, simultaneous turns, the first fighter, and the team counters.
- **Pass** (`TavernAction::Pass`): its cost, free Passes (`Wanderer Cho`), the incoming queue, and triples completed by passed cards.
- The team's combat: building both players' warbands, applying each player's result to their own Tavern, and the team damage.
- The Duos catalog (`catalog_for("duos")`) and the Duos-exclusive cards (§8).
- How card text reaches the teammate (§7): team counters, gifts to the teammate, the teammate's observers, the teammate's warband in combat.

### Deferred
- 4-team matches and their pairings: the engine has one team's Tavern; a match of four teams is the caller's job.
- Heroes (Hero Powers, starting Armor values) and the Duos combat rules themselves (`duos.md`).

---

## 2. The Team

- Duos has **4 teams of 2 players** **(confirmed)**. A `DuosTavern` is one team: its players are numbered `0` and `1` (`players[0]`, `players[1]`), and each one's **teammate** is the other.
- **Each player has their own Tavern** **(confirmed)**: their own `TavernState` (`tavern.md` §3), with its Gold, Tavern Tier, shop, board, hand, Discover prompts and player auras.
- **All players share one card pool** **(confirmed)**: a single `CardPool` (`tavern.md` §2) for the whole match, passed to every call, as is the match's RNG. Both teammates buy from it, sell to it and Discover from it.
- **Teammates are not friendly** **(confirmed)**: a player's effects see their own board, hand, shop and auras only ("friendly", "your minions", "your hand"), and "your hero" is the player's own hero. Effects are per player unless a card says *team* or *teammate* **(confirmed)**.

`DuosTavern` fields:

| Field | Meaning |
| :--- | :--- |
| `players` | The two players' `TavernState`s. |
| `health`, `armor` | The team's shared Health and Armor (§3). |
| `passes_this_turn`, `passes_this_game` | The team counters (§7). |
| `incoming` | Each player's queue of passed cards waiting for room in their hand (§5.4). |
| `ended` | Which players have ended their turn (§4.2). |
| `first_fighter` | The player whose warband fights first in the coming combat (§4.3). |
| `first_fighter_switch_chance` | The chance that the first fighter switches each turn (§4.3). |
| `sat_out` | Whose warband did not fight in the last combat (§6.3). |

---

## 3. Shared Health and Armor

1. **One pool** **(confirmed)**. Teammates share one Health pool: `30` Health, plus Armor equal to the sum of both heroes' Armor. Heroes are out of scope, so each hero's starting Armor is a parameter (`DuosTavern::with_armor([a0, a1])`: the team starts with `a0 + a1` Armor).
2. **Damage** takes Armor first, then Health, as in Solo (`tavern.md` §7.2).
3. **"Your Health" is the team's.** Each player's `TavernState.health` and `armor` always hold the team's values: the team copies them in before a player acts and back out after **(engine decision: how the pool is shared)**. So every Solo rule that reads or changes "your" Health or Armor uses the pool:
   - Health costs: a `Hasty Excavation` is payable while the team's Health plus Armor is above its cost; a Health Refresh likewise.
   - "Deal damage to your hero" (`Wrath Weaver`) takes Health from the pool.
   - "Set your Armor to 5" (`Armor Stash`) sets the team's Armor **(engine decision)**.
   - "If you have 15 or less Health" (`Holy Vanguard`) reads the team's Health, for both players.
4. **Per-player triggers** **(confirmed)**. When a player pays Health or takes hero damage in the Tavern, the pool goes down, but only **that player's** "your hero takes damage" minions trigger (`Soul Rewinder`, `Ashen Corruptor`, `Tichondrius` rewind or react; `Eredar Escapist` counts it). The teammate's don't. A rewind (`Soul Rewinder`) gives the pool its Health back.
5. **Combat damage** goes to the losing team's pool **(confirmed)**, once (§6.2), Armor first. As in Solo (`tavern.md` §7.2), it triggers no hero-damage minion, for either player **(confirmed)**: `Soul Rewinder` doesn't rewind it, `Ashen Corruptor` and `Tichondrius` don't trigger, and `Eredar Escapist` doesn't count it (`DuosTavern::take_combat_damage`).
6. **A team at 0** (or less) Health is out **(confirmed)**: `DuosTavern::is_out()`. Ending the match is the caller's job (§1).

---

## 4. A Turn

### 4.1 Start of Turn (`DuosTavern::start_turn`)

1. `passes_this_turn = 0`, and no player has ended their turn.
2. Player 0's start of turn (`tavern.md` §4), then player 1's, with the shared pool and RNG **(engine decision: the order)**. A player whose warband sat out the last combat keeps its next-combat effects (§6.3).
3. The gifts deliver (§7.2), then the queues (§5.4): player 0's, then player 1's.
4. The first fighter is drawn (§4.3).

### 4.2 Simultaneous Turns

- **Both teammates act during the same turn** **(confirmed)**, in any interleaving. The engine takes one action at a time, each tagged with its player: `step(player, action, pool, rng)`. `is_legal(player, action)` and `valid_actions(player)` answer for one player.
- Every action but Pass is the player's own Tavern action (`tavern.md` §6), with that player's legality, through their `TavernState`.
- A passed card **arrives at once** **(confirmed)**, or queues if the receiver's hand is full (§5.4). The receiver can play it **that turn** if they haven't ended theirs **(confirmed)**.
- `EndTurn` ends that player's turn (their end-of-turn triggers, `tavern.md` §7.5). After it the player takes no more actions this turn **(engine decision)**, but cards passed to them still arrive.
- **The team's turn ends when both players have ended theirs** **(confirmed)** (`turn_over()`); the team's combat follows (§6).

### 4.3 The First Fighter

- Each team's first fighter is chosen at random each turn and shown to the players before combat **(confirmed)**: it is drawn at the start of the turn and is visible all turn (`first_fighter`, and `fights_first` in the observation).
- On the first turn it is a fair coin. After that it **usually switches** **(confirmed)**: it switches with probability `first_fighter_switch_chance`, else stays. The real probability is unknown **(confirmed)**, so it is a parameter; the default is `0.75` **(engine decision)**.
- One draw per start of turn, after the players' (step 4 above): `v = rng.next_u64()`. On turn 1, `first_fighter = v % 2`. Later, the first fighter switches if `(v >> 11) / 2^53 < first_fighter_switch_chance` (so `0` never switches and `1` always does).

---

## 5. Pass

### 5.1 The Action

`TavernAction::Pass { hand_index }` moves `hand[hand_index]` from the player's hand to their teammate **(confirmed)**. It is the last `TavernAction` variant, so the earlier ones keep their places. A Solo `TavernState` never allows it: Solo has no teammate.

A Pass is legal (`DuosTavern::is_legal`) when:

1. the player hasn't ended their turn (§4.2);
2. no Discover or Choose One is pending for the player: as for every action, only `ChooseDiscover` is legal then (`tavern.md` §6) **(engine decision)**. The teammate's pending Discover doesn't matter;
3. `hand_index < hand.len()`;
4. the card isn't **locked** (§5.3);
5. the player has a free Pass (§5.2) or at least `1` Gold **(confirmed)**.

Any card can be passed: a minion or a spell, Golden or not **(confirmed)**. There is **no limit per turn** **(confirmed)**, and the teammate's hand may be full: the card then queues (§5.4) **(confirmed)**.

### 5.2 Cost and Free Passes

- A Pass costs **1 Gold** **(confirmed)**, paid like any Gold cost (`TavernState::spend_gold`): it counts as Gold spent this turn (`gold_spent_this_turn`, `Lovesick Balladist`) and for the minions that react to spent Gold **(confirmed)**.
- **Free Passes** **(confirmed)**: each `Wanderer Cho` on the board makes one Pass each turn free (a Golden one, two). Two Chos make two free Passes, and they come back each turn. A free Pass costs nothing and spends no Gold.
- Each Cho tracks its own free Pass, and a free Pass is always used before paying **(confirmed)**. So a Cho played mid-turn gives its free Pass at once, even after Passes were paid for this turn; a Cho made Golden mid-turn gains its second one; a sold Cho loses its unused one **(confirmed)**.
- The engine sees a free Pass as a charge: a minion with `CardFlags::FREE_PASS` and `charges` left (`Wanderer Cho`: `1` each turn, `2` if Golden, reset at the start of the turn by its `reset_turn_charges`, `+1` when made Golden, as `Thorned Trailblazer`'s charges). A Pass uses a charge of the left-most such minion if there is one, else costs 1 Gold.

### 5.3 What Moves

- The card leaves the passer's hand at once **(confirmed)**. A Pass is neither a play nor a discard: no Battlecry, no "whenever you discard" trigger **(engine decision)**.
- It keeps its stats, buffs, keywords and Golden status **(confirmed: a passed Golden arrives Golden)**. A player's "wherever this is" auras come off as it arrives and the receiver's go on: they are player effects, and teammates are not friendly **(engine decision)**.
- **Locked cards can't be passed** **(confirmed)**: a card locked until the next turn (`locked_turns > 0`: the minion `Search Through Time` Discovered), and a card still in a `Lockbox` (`lockbox_turns_left > 0`, `Bilgewater Breakout`).
- A card of a group (the spells of a `Wandering Willbreaker`, the Aberrations of a `Faceless Operative`: `tavern.md` §6, card groups) leaves its group: the rest stay with the passer, and the passed card is a group of its own in the receiver's hand **(engine decision)**.

### 5.4 Arrival and the Queue

**(confirmed)**

- Each player has an unbounded FIFO queue of incoming passed cards (`incoming`). A passed card joins the end of the receiver's queue, which then delivers.
- **Delivery**: while the receiver has fewer than `10` cards in hand and their queue isn't empty, the first queued card moves into their hand (`TavernState::add_to_hand`: their auras, their "a card is added to your hand" observers, their triple check, §5.5). The queues deliver after every action of either player and at the start of the turn (player 0's queue first). So a card passed to a hand with room arrives at once, and a queued card arrives as soon as its receiver has room, typically once they play a card.
- A queued card is not in the hand: it doesn't count for triples until it arrives, can't be played, is not affected by effects on the hand, and is not part of the warband's hand in combat **(engine decision for combat)**.
- The queue persists across turns. Delivery during combat is out of scope.

### 5.5 Triples with a Passed Card

**(confirmed)** A delivered card goes through the receiver's normal triple check (`tavern.md` §5): with two copies on the receiver's board and/or in their hand, it completes the triple. The receiver gets the Golden in their hand, and playing it gives **the receiver** the Triple Reward: a Discover of their Tavern Tier + 1. A passed Golden likewise gives its Triple Reward to whoever plays it.

### 5.6 Order of a Pass

1. The card leaves the passer's hand.
2. Payment: a free Pass's charge, else 1 Gold spent (and its observers).
3. The card's own `passed` hook (§7).
4. `passes_this_turn` and `passes_this_game` go up (and into both players' auras, §7).
5. The passer's `after_pass` observers, then both players' `after_team_pass` observers (the passer's first).
6. The card joins the receiver's queue, and the queues deliver (§5.4).

A Pass by a card effect ("Pass it", §7.2) is steps 3–6: the card has already left wherever it was, and nothing is paid.

---

## 6. The Team's Combat

### 6.1 The Warbands

Each player's warband is built as Solo builds a side (`TavernState::resolve_combat_against`): their board (at most 7 minions), their hand, their player auras and their Tavern Tier, with their auras synced first. The team's first warband is `first_fighter`'s. The combat is `resolve_duos_battle` (`duos.md`), with a seed from the caller. Its unit ids follow `duos.md` §3.1 step 1.

- `DuosTavern::resolve_combat_pair(a, b, seed)`: team `a` (side A) against team `b`; both teams' Taverns take their results.
- `DuosTavern::resolve_combat_against(&opponent, seed)`: this team (side A) against a fixed `DuosTeam` (a ghost, a test opponent); only this team takes its results.

### 6.2 The Results

Each player's `WarbandResult` comes back to **their own** Tavern exactly as a Solo side's result does (Combat-to-Tavern Persistence, `combat.md` §1; `TavernState::apply_combat_return`):

1. their auras after combat (a Tavern aura gained in combat also buffs the minions in their shop);
2. their after-combat player effects (`Overconfidence`), with the **team's** result;
3. their hand after combat: a hand buff (`Very Hungry Winterfinner`) or a card gained in combat (`Roadboar`) stays with that player only **(confirmed: teammates are not friendly)**;
4. their hand cards that became ready (a sped-up `Lockbox`), with an RNG derived from the seed and the player;
5. what their minions keep from combat (`Tarecgosa`, permanent gains, improvements), matched by unit id;
6. their triple check;
7. `last_combat_won` / `last_combat_lost`: the team's result, for both players (`Winner's Bread`, `Tortollan Blue Shell`), even one whose warband sat out: the team's combat is its combat **(confirmed)**.

Then the gifts the players' cards gave in combat deliver (§7.2), and the **damage**: if the team lost, its pool takes `hero_damage` once, Armor first **(confirmed)**; no "your hero takes damage" minion reacts to it (§3.5).

### 6.3 A Warband That Sat Out

A warband that never fought (its teammate won alone, or it had nothing to fight with) comes back unchanged (`duos.md` §6). It **keeps its next-combat effects** **(confirmed)**: its player's Start of Combat effects (`Upper Hand`, `Brood of Nozdormu`, `Sharing is Caring`: "at the start of your next combat") never triggered, so they wait for the next combat it fights. They last until the start of the turn (`EffectDuration::Turn`), so at the next start of turn the team keeps that player's unused Start of Combat effects (those whose card has a `player_start_of_combat` or `player_combat_space` hook) instead of letting them expire **(engine decision: how)**. Its "after your next combat" effects (`Overconfidence`) and `Winner's Bread` use the team's result, like its teammate's (§6.2) **(confirmed)**.

---

## 7. How Card Text Reaches the Teammate

Teammates are not friendly (§2): a player's cards see their own Tavern only. A card reaches the teammate only through the channels below, when its text says *team* or *teammate* **(confirmed)**. The engine stays card-agnostic: the team routes and counts, and what a card does lives in its card file.

### 7.1 Team Counters

- **Passes.** `passes_this_turn` (reset at the start of each turn) and `passes_this_game` count the team's Passes, either player's (a Pass by a card effect too, §7.2). Both players' auras mirror them (`PlayerAuras::team_passes_this_turn`, `team_passes_this_game`), so card hooks and "wherever this is" auras can read them in the Tavern and in combat. They stay `0` in Solo.
- **Card counters.** The other "your team's ... this game" counts (the `San'layn Scribe`s that died, the `Orc-estra Conductor`s played) are card counters (`PlayerAuras::card_counters`, keyed by the card): each player's cards add to their **own** counters, in the Tavern and in combat. The team mirrors each player's card counters into their teammate's `PlayerAuras::teammate_counters` (with the Health and the Passes, §3.3: `DuosTavern::mirror`), so `PlayerAuras::team_counter(key)` (own plus teammate's) is the team's count **(engine decision: how)**. In Solo `teammate_counters` stays empty: a team counter is the player's own.
- In combat each warband counts for its own player; a warband that steps in takes its teammate's counts as they are at that moment, so a `San'layn Scribe` stepping in has the Scribes that died before it in its teammate's warband (`duos.md` §3.2) **(engine decision)**.

### 7.2 Gifts to the Teammate

A card that gives the teammate something ("your teammate gets", "you and your teammate each", "your team's") records a **gift** in its player's `PlayerAuras::to_teammate`, in the order it gives them. A `TeammateGift` is one of:

| Gift | Means | For |
| :--- | :--- | :--- |
| `Card(card)` | The teammate gets `card`: it goes to their hand (`TavernState::add_to_hand`: their auras, observers and triple check), or is lost if their hand is full, as a card gained in Solo; it is not a passed card and doesn't queue **(engine decision)**. | `Friendly Saloonkeeper`'s Tavern Coin, `Generous Geomancer`'s Blood Gem. |
| `Pass(card)` | The player's card effect **Passes** `card` ("Pass it"): a Pass in all but the payment (§5.6 steps 3–6: the card's `passed` hook, the team's counters, the `after_pass` and `after_team_pass` observers, the queue). It costs nothing and uses no free Pass **(engine decision)**. | `Jumping Jack`, `Mirror Monster`, the Portals. |
| `Effect { card_id, amount, team }` | Anything else: the giving card's `teammate_gift` hook runs on the teammate's Tavern with `amount`, or on both players' Taverns if `team` (the giver's first). The cards it returns go to that player's hand, as `Card`s do. | `Feisty Freshwater`'s free Refreshes, `Wheeled Crewmate`'s cheaper upgrades. |

The team delivers the gifts (`DuosTavern::deliver_gifts`), each player's in order, the acting player's first, until none is left (a gift's hooks may give more):

1. after every action of either player (§4.2), before the queues deliver;
2. at the start of the turn, after both players' start of turn (§4.1), so the Gold a teammate gives at the start of the turn is not replaced by the receiver's own Gold for the turn;
3. after the team's combat (§6.2), before the damage: in combat a gift waits in its warband's auras and comes back in its `WarbandResult` (`duos.md` §6), so it reaches the teammate's Tavern after the combat, not their warband **(engine decision)**. The RNG of the combat's gifts is derived from the combat seed and the team. A Pass given in combat waits for the next action or start of turn: its observers may draw from the shared pool, which the combat doesn't have.

In Solo nothing delivers them: Solo has no teammate.

### 7.3 The Teammate's Observers

- `after_teammate_sell`: after a player sells a minion (`TavernAction::Sell`), their teammate's observers see it, as it was on the board (`Dark Dazzler`).
- `after_team_pass`: both players' observers, after any Pass of the team (§5.6).

### 7.4 The Teammate's Warband in Combat

In a Duos combat a card can read its teammate's warband as it entered the combat (`BoardCtx::teammate_board`, read-only): their board at the start of the combat, whether it is waiting, fighting or done **(engine decision)**. It is empty in Solo and in the Tavern. (`Sandy`)

### 7.5 Hooks

`src/cards/hooks.rs`:

| Hook | Fires | For |
| :--- | :--- | :--- |
| `passed` | The passed card's own text, as it is passed (§5.6 step 3). | "After this is Passed, gain +4/+4" (`Puddle Prancer`). |
| `after_pass` | Observers on the passer's board, with the passed card. | "After you Pass a Tavern spell, get a copy" (`Storm Splitter`). |
| `after_team_pass` | Observers on both players' boards, after the counters went up. | "The first time your team Passes each turn" (`Passenger`). |
| `after_teammate_sell` | Observers on the teammate's board, with the sold minion (§7.3). | "After your teammate sells a minion" (`Dark Dazzler`). |
| `teammate_gift` | The giving card's `Effect` gift, on the teammate's Tavern (§7.2). | "You and your teammate each gain two free Refreshes" (`Feisty Freshwater`). |
| (auras) | `team_passes_this_game`, `team_counter(key)` in the player's auras (§7.1). | "+3/+3 for each time your team Passed this game, wherever this is" (`Transport Reactor`). |

---

## 8. The Duos Catalog and Pool

- `catalog_for("duos")` (alias `"duos_full"`) is the full Solo catalog (`full_catalog()`) followed by the implemented Duos-exclusive minions (`duos_exclusive_catalog()`).
- **Duos-exclusive cards are only in a Duos catalog and pool** **(confirmed)**: no Solo catalog (`tierN`, `full`, ...) changes.
- They are registered like any card (their hooks, their template, the names unit specs know), but the effects that generate random minions from the catalog (`Lockbox`, `Deathly Striker`) keep using the Solo catalog **(engine decision; no Duos-exclusive card qualifies for them yet)**.
- Their card ids are `10_000 + 100 × tier + n`, after every Solo card, so that adding them only appends to sorted id lists (the RL card vocabulary).

### 8.1 `Wanderer Cho`

**(confirmed)** `Wanderer Cho` (`BGDUO_100`): Tier 2, `4/3`, no minion type, Duos-exclusive. *"One Pass each turn is free."* Golden (`BGDUO_100_G`): `8/6`, *"Two Passes each turn are free."* (§5.2)

---

## 9. Determinism

A `DuosTavern` is a pure function of its setup, the seeds and the actions taken: the shared pool and RNG are passed in, every call draws in a fixed order (§4.1, §4.3), a Pass draws nothing itself, and combat seeds come from the caller. The scenario runner runs every Duos Tavern scenario twice per seed and checks both runs end identically (`scenarios.md` §6).

---

## 10. RL

The Python bindings (`DuosTavernGame`, `TavernAction.pass_card`, `simulate_duos`, `simulate_duos_batch`) and how Duos maps onto the MDP are described in [`../../rl/README.md`](../../rl/README.md).

---

## 11. Decisions

The rules the user settled are marked **(confirmed)** above: 4 teams of 2, separate Taverns and one pool, teammates not friendly and effects per player; one Health pool of 30 plus both heroes' Armor; per-player hero-damage triggers, and none for combat damage; simultaneous turns, arrival at once, the turn ending when both have ended; the random first fighter that usually switches; Pass from hand to hand for 1 Gold (Gold spent), with no limit, any card but locked ones (`Search Through Time`, a `Lockbox`); the free Passes of `Wanderer Cho`, one per Cho, used before paying; the unbounded incoming queue; triples completed by passed cards, with the receiver's Golden and Triple Reward; combat damage to the losing team's pool; the team's result for both players; a sat-out warband keeping its next-combat effects; Duos-exclusive cards only in Duos pools.

Engine decisions, pending the user's confirmation (each is one place in `src/duos_tavern.rs` unless noted):

| Decision | Rule | Where |
| :--- | :--- | :--- |
| Mirrored Health | Each player's `health`/`armor` hold the team's, copied in and out around each action (§3.3). | `DuosTavern::mirror`, `settle` |
| Team Armor | "Set your Armor" sets the team's (§3.3). | (falls out of the mirror) |
| Start-of-turn order | Player 0's start of turn, then player 1's (§4.1). | `DuosTavern::start_turn` |
| After `EndTurn` | No more actions that turn; passed cards still arrive (§4.2). | `DuosTavern::is_legal` |
| First fighter | A fair coin on turn 1, then switching with `first_fighter_switch_chance` (default `0.75`), one RNG draw per turn (§4.3). | `DuosTavern::draw_first_fighter` |
| Pass with a pending Discover | Not legal for the passer; the receiver's doesn't matter (§5.1). | `DuosTavern::can_pass` |
| Not a play or discard | A Pass triggers no Battlecry or discard effect (§5.3). | `DuosTavern::pass` |
| Card groups | A passed group card leaves its group (§5.3). | `DuosTavern::pass` |
| Queued cards in combat | Not in the warband's hand (§5.4). | `DuosTavern::warband` |
| Keeping next-combat effects | The unused Start of Combat effects (`Turn`) of a warband that sat out survive the next start of turn (§6.3). | `DuosTavern::start_turn` |
| Random minions | `Lockbox` and `Deathly Striker` draw from the Solo catalog even in Duos (§8). | `cards::all_templates` |
| Team counters | Each player counts their own; the team mirrors them into the teammate's `teammate_counters` (§7.1). | `DuosTavern::mirror` |
| Counters at a step-in | A warband stepping in takes its teammate's counts from this combat (§7.1). | `combat.rs` (`DuosTeamState::step_in`) |
| Gift cards | A card given to the teammate is lost if their hand is full, as in Solo; only passed cards queue (§7.2). | `DuosTavern::deliver_gifts` |
| Passes by effects | Free, and no free Pass is used (§7.2). | `DuosTavern::effect_pass` |
| Gifts in combat | They reach the teammate's Tavern after the combat, not their warband (§7.2). | `DuosTavern::apply_results` |
| The teammate's warband | A combat card reads it as it entered the combat, even after it fought (§7.4). | `combat.rs` (`DuosTeamState::new`) |

### Open Questions

1. What is the real chance that the first fighter switches? (To be measured from game logs.)
2. Does a passed `Wandering Willbreaker` spell or `Faceless Operative` Aberration stay linked to the rest of its group in the passer's hand?
3. Can effects that generate random minions (`Lockbox`, `Deathly Striker`) give a Duos-exclusive minion in Duos?
4. Does a card given to a teammate whose hand is full wait like a passed card, or is it lost as in Solo (the engine's choice)?
5. In combat, does a gift to the teammate reach their warband at once (a `Deity` buff, a `San'layn Scribe` death) or only their Tavern after the combat?
