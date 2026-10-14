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
- A card of a group (the spells of a `Wandering Willbreaker`, the Aberrations of a `Faceless Operative`: `tavern.md` §6, card groups) is passed as it would be played: it counts down its group, and once the Pass has resolved the rest of the group is discarded from the passer's hand if it is used up, as after a play. The passed card arrives as a plain card, in no group **(confirmed)**.

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

A Pass by a card effect ("Pass it", §7.2) is steps 3–6: the card has already left wherever it was, and nothing is paid: it costs no Gold and uses no free Pass, and it counts as the team's Pass for everything that reacts to Passes (the counters, `after_pass`, `after_team_pass`) **(confirmed)**.

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
| `Card(card)` | The teammate gets `card`: it joins their incoming queue (§5.4) like a passed card, so it arrives at once, or waits if their hand is full; it counts for their triples only once it arrives **(confirmed)**. | `Friendly Saloonkeeper`'s Tavern Coin, `Generous Geomancer`'s Blood Gem, `Bargain Bundle`'s other options. |
| `Pass(card)` | The player's card effect **Passes** `card` ("Pass it"): a Pass in all but the payment (§5.6 steps 3–6: the card's `passed` hook, the team's counters, the `after_pass` and `after_team_pass` observers, the queue). It costs nothing and uses no free Pass **(confirmed)**. | `Jumping Jack`, `Mirror Monster`, `Voidpriest Cloner`, `Private Selection`, `Well Wishes`, the Portals. |
| `Effect { card_id, amount, team }` | Anything else: the giving card's `teammate_gift` hook runs on the teammate's Tavern with `amount`, or on both players' Taverns if `team` (the giver's first). The cards it returns join that player's queue, as `Card`s do. | `Feisty Freshwater`'s free Refreshes, `Wheeled Crewmate`'s cheaper upgrades. |

The team delivers the gifts (`DuosTavern::deliver_gifts`), each player's in order, the acting player's first, until none is left (a gift's hooks may give more):

1. after every action of either player (§4.2), before the queues deliver;
2. at the start of the turn, after both players' start of turn (§4.1), so the Gold a teammate gives at the start of the turn is not replaced by the receiver's own Gold for the turn;
3. after the team's combat (§6.2), before the damage: in combat a gift waits in its warband's auras and comes back in its `WarbandResult` (`duos.md` §6), so it reaches the teammate's Tavern after the combat, not their warband **(engine decision)**. The RNG of the combat's gifts is derived from the combat seed and the team. A Pass given in combat waits for the next action or start of turn: its observers may draw from the shared pool, which the combat doesn't have.

In Solo nothing delivers them: Solo has no teammate.

### 7.3 The Teammate's Observers

- `after_teammate_sell`: after a player sells a minion (`TavernAction::Sell`), their teammate's observers see it, as it was on the board (`Dark Dazzler`).
- `after_team_pass`: both players' observers, after any Pass of the team (§5.6).

### 7.4 The Teammate's Warband in Combat

In a Duos combat a card can read its teammate's warband as it entered the combat (`BoardCtx::teammate_board`, read-only): their board at the start of the combat, whether it is waiting, fighting or done **(confirmed)**. It is empty in Solo and in the Tavern. (`Sandy`, `Grave Narrator`)

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
- **Duos-exclusive cards are only in a Duos catalog and pool** **(confirmed)**: no Solo catalog (`tierN`, `full`, ...) changes. Likewise the Duos spells are only offered by Duos shops (§8.3).
- They are registered like any card (their hooks, their template, the names unit specs know).
- **Random minions in Duos** **(confirmed)**: in a Duos game, the effects that generate random minions from the catalog draw from the Duos catalog, Duos-exclusive minions included; in Solo they draw from the Solo catalog, as before. A player's auras say whether they are in a Duos game (`PlayerAuras::duos_game`, set by the team's Tavern and carried into combat by the warbands), and those effects draw from `cards::game_templates(auras)`: `Lockbox`, `Deathly Striker`, `Headhunter Gryphon`, `Dark Paradox`, `Ghastcoiler`, and the Duos cards' own (`Shifty Snake`, `Grave Narrator`, ...). Effects that draw from the card pool use the game's pool, which is the Duos one in Duos.
- **Naga-only minions** **(confirmed)**: a minion whose only type is Naga (`Private Chef`, `Storm Splitter`) is in the pool only when Naga is in the game, which can't happen in the lobbies of Patch 36.6.3. They are implemented and registered (unit specs can name them), but they are in no catalog, so neither in the Duos pool nor in its random draws (`cards::duos_out_of_pool_catalog`).
- Their card ids are `10_000 + 100 × tier + n` (the Duos spells' `10_000 + 100 × tier + 50 + n`, the Spellcraft spells' `10_000 + 100 × tier + 90 + n` with their minion's tier), after every Solo card, so that adding them only appends to sorted id lists (the RL card vocabulary).
- Each card's file starts with its card ID (`BGDUO_...`), which the Power.log replay tools read.

### 8.1 `Wanderer Cho`

**(confirmed)** `Wanderer Cho` (`BGDUO_100`): Tier 2, `4/3`, no minion type, Duos-exclusive. *"One Pass each turn is free."* Golden (`BGDUO_100_G`): `8/6`, *"Two Passes each turn are free."* (§5.2)

### 8.2 The Combat Cards

Their text acts in combat (and in the Tavern if they die there). What they give the teammate reaches the teammate's Tavern after the combat (§7.2). Golden doubles the number in the text.

| Card | Text | How the engine reads it |
| :--- | :--- | :--- |
| `Generous Geomancer` (`BGDUO_111`), Tier 2 Quilboar `1/1` | *Deathrattle: You and your teammate each get a Blood Gem.* (Golden `2/2`: 2.) | The player gets the Blood Gems; the teammate's are `Card` gifts. |
| `Wheeled Crewmate` (`BGDUO31_207`), Tier 3 Pirate `6/3` | *Deathrattle: Reduce the Cost of upgrading your team's Taverns by (1).* (Golden `12/6`: 2.) | A team `Effect` gift: each player's `upgrade_cost` goes down, never below 0. |
| `Feisty Freshwater` (`BGDUO_110`), Tier 4 Elemental `6/4` | *Deathrattle: You and your teammate each gain two free Refreshes.* (Golden `12/8`: four.) | The player's `free_refreshes` go up at once (they come back with the auras); the teammate's through an `Effect` gift. |
| `Grave Narrator` (`BGDUO_112`), Tier 4 Undead `2/7` | *Avenge (3): Your teammate gets a random minion of their most common type.* (Golden `4/14`: 2.) | Combat only, like every Avenge. "Their most common type" is read from the teammate's warband as it entered the combat (§7.4), as `Friendly Bounty` reads a board (`tavern.md` §7.8: ties at random, a random lobby type if no minion has one); the minion is of that type and of the Narrator's player's Tavern Tier or lower **(confirmed)**, from the game's catalog (§8): a `Card` gift. |
| `Shifty Snake` (`BGDUO31_203`), Tier 4 Beast `6/1` | *Deathrattle: Your teammate gets a random Deathrattle minion.* (Golden `12/2`: 2.) | Of the Snake's player's Tavern Tier or lower **(engine decision)**, from the game's catalog (§8): a `Card` gift. |
| `San'layn Scribe` (`BGDUO31_208`), Tier 4 Undead `4/4` | *Has +4/+4 for each of your team's San'layn Scribes that died this game (wherever this is).* (Golden `8/8`: +8/+8.) | As `Eternal Knight`: each Scribe that dies (Golden or not, in combat or in the Tavern) counts 1 for its player; the bonus counts the team's (`team_counter`, §7.1), so a Scribe stepping in counts its teammate's Scribes that died before it. |
| `C'Thrax Wrecker` (`BGDUO_701`), Tier 5 Aberration `8/8` | *Battlecry, Deathrattle, and Rally: Give your team's Deities +4/+4.* (Golden `16/16`: +8/+8.) | The player's Deity gains it at once (in combat, an awakened Deity on the board too, as `Drifting Sacrifice`); the teammate's through an `Effect` gift. |
| `Transport Reactor` (`BGDUO31_211`), Tier 6 Mech `3/3`, Magnetic | *Has +3/+3 for each time your team has Passed this game (wherever this is).* (Golden `6/6`: +6/+6.) | Counts `team_passes_this_game` (§7.1): the Passes before it was bought too. Magnetized onto a Mech, the Mech keeps the bonus as its own "wherever this is" aura (`Unit::pass_aura`), growing with the team's later Passes. |
| `Sandy` (`BGDUO_125`), Tier 7 `1/1` | *Start of Combat: Transform into a copy of your teammate's highest-Health minion.* (Golden `2/2`: a Golden copy.) | See below. |

**`Sandy`** **(confirmed)**: at its Start of Combat it reads its teammate's warband as it entered the combat (§7.4), even if that warband has fought, and copies the minion with the most Health there; on a tie, one of them at random (one RNG draw). With no minion there, Sandy stays as it is.

- The copy has the minion's current stats, keywords and card state, and its Golden status. A Golden Sandy makes it Golden (`Unit::make_golden`).
- It takes Sandy's place and combat id, and its player's "wherever this is" auras replace the teammate's, as for a passed card (§5.3).
- It is a transform, not a summon: no summon observer sees it. Its own Start of Combat doesn't trigger: Sandy's has.
- After the combat, Sandy is itself again and keeps nothing from the combat: a minion that transformed in combat keeps nothing from it (`docs/combat.md` §1).

### 8.3 The Pass and Team-Counter Cards

Their Passes are Passes by card effects (§5.6, §7.2): free, using no free Pass, and counted as the team's Passes for everything that reacts to Passes (the counters, `Passenger`, `Mantid King`, `Transport Reactor`, `Storm Splitter`) **(confirmed)**. Golden doubles the number in the text.

| Card | Text | How the engine reads it |
| :--- | :--- | :--- |
| `Passenger` (`BGDUO_114`), Tier 1 `2/2` | *The first time your team Passes each turn, gain +1/+2.* (Golden `4/4`: +2/+4.) | An `after_team_pass` observer: it gains the stats when the team's Passes this turn reach 1. A Passenger played after the team's first Pass of the turn gains nothing that turn. |
| `Jumping Jack` (`BGDUO_115`), Tier 3 All `3/4` | *After the first time this is sold, Pass it.* (Golden `6/8`: the same.) | Its `sell` hook: sold, it gives its Gold and its sell triggers as usual, then is Passed with its stats instead of going back to the pool. It remembers it was sold (`counter`): sold again, it goes back to the pool. Made Golden (`Golden Touch`, a triple), it forgets **(confirmed)**. |
| `Orc-estra Conductor` (`BGDUO_119`), Tier 3 `4/4` | *Battlecry: Give a minion +2/+2 (Improved by each Orc-estra your team has played this game).* (Golden `8/8`: +4/+4.) | Playing it counts 1 for its player (its card counter, once per play: a repeated Battlecry doesn't count again); the Battlecry gives +2/+2 (+4/+4 if Golden) for each Orc-estra the team has played (`team_counter`, §7.1), this one included: the team's first gives +2/+2, its second +4/+4 **(confirmed)**. Its target is a friendly minion, picked as `Elite Navigator`'s (`tavern.md` §7.1): the one at the play position, else the one to its left, else the left-most; none without another minion **(engine decision)**. |
| `Puddle Prancer` (`BGDUO_117`), Tier 3 Murloc `4/4` | *After this is Passed, gain +4/+4.* (Golden `8/8`: +8/+8.) | Its `passed` hook, each time it is Passed (by its player or a card). |
| `Voidpriest Cloner` (`BGDUO_700`), Tier 3 Aberration `5/6` | *After you discard a card, Pass a plain copy of it. (Once per turn.)* (Golden `10/12`: twice per turn.) | An `after_friendly_discard` observer (it sees the discarded card); its `charges` are its uses left this turn (as `Wanderer Cho`'s: `+1` when made Golden). A plain copy is the card's base, non-Golden, unbuffed version (a spell: a new copy of it). Selling, playing or Passing a card is not discarding it; the rest of a used-up group is (§5.3), and so are the cards an "Activate: discard a card" minion discards. |
| `Mantid King` (`BGDUO31_212`), Tier 4 `3/3` | *After your team Passes, randomly gain Venomous, Taunt, or Divine Shield until next turn.* (Golden `6/6`: permanently.) | An `after_team_pass` observer: after each of the team's Passes, one of the three it doesn't have, at random (nothing if it has them all) **(confirmed**, as the Murlocs that add keywords: `Bubble Gunner`, `Sewer Escapee`**)**. Until next turn: it loses them at the start of its player's next turn, on the board or in the hand (`counter` remembers which; Divine Shield is a `temp_divine_shield`). Made Golden, the keywords it has until next turn stay temporary **(engine decision)**. |
| `Mirror Monster` (`BGDUO_108`), Tier 4 All `4/4` | *When you buy or Discover this, get an extra copy and Pass it.* (Golden `8/8`: 2 extra copies.) | Its `obtained` hook ("when you buy or Discover this"): it Passes copies of the card as it reached the hand (stats and Golden status included). The copies are new cards, not from the pool **(confirmed)**; the player keeps the card itself, so the copies can complete the teammate's triple, not the player's. |
| `Private Chef` (`BGDUO31_209`), Tier 4 Naga `5/4` | *Spellcraft: Choose a minion. Get a different random minion of its type, then Pass it.* (Golden `10/8`: 2 different minions.) | In no pool (§8). Its Spellcraft spell, `Private Selection` (`BGDUO31_209t`), targets a minion with a type, on the board or in the Tavern, and draws a minion of one of its types from the pool: another card, of the player's Tavern Tier or lower, as `Chef's Choice`; Golden, 2 different cards. It is Passed at once, without going through the player's hand **(confirmed)**. |
| `Storm Splitter` (`BGDUO_122`), Tier 5 Naga `5/5` | *Once per turn, after you Pass a Tavern spell, get a new copy of it.* (Golden `10/10`: 2 copies.) | In no pool (§8). An `after_pass` observer: after its player Passes a Tavern spell (themselves or by a card), it gets a new copy of the spell, once per turn (`charges`). Blood Gems and Spellcraft spells are not Tavern spells. |
| `Well Wisher` (`BGDUO_120`), Tier 5 `6/6` | *Spellcraft: Pass a different non-Golden minion.* (Golden `12/12`: the same.) | Its Spellcraft spell, `Well Wishes` (`BGDUO_120t`: *"Pass a non-Golden minion (except Well Wisher)"*), targets a friendly non-Golden minion that isn't a `Well Wisher`: it leaves the board and is Passed. |
| `Portal in a Fountain` (`BG31_243`), Tier 3 Tavern spell (`3` Gold) | *Pass a minion in the Tavern.* | Targets a minion in the Tavern, and only there (`SpellTarget::TavernMinion`): it leaves the shop, without going back to the pool, and is Passed. |
| `Portal in a Crystal` (`BG31_244`), Tier 5 Tavern spell (`2` Gold) | *Pass a friendly non-Golden minion.* | Targets a friendly non-Golden minion: it leaves the board and is Passed. |
| `Bargain Bundle` (`BG31_242`), Tier 5 Tavern spell (`5` Gold) | *Discover a minion. Your teammate gets the other options.* | Discovers among 3 minions of the player's Tavern Tier or lower from the pool **(confirmed)**. Its options are marked (`Unit::unchosen_to_teammate`): once one is chosen, the others go to the teammate (`Card` gifts, not Passes) instead of back to the pool. |

A Pass by one of these cards takes the card out of wherever it was (the board for `Well Wishes` and `Portal in a Crystal`, the shop for `Portal in a Fountain`) without returning it to the pool: the receiver gets it with its stats, as from a Pass from the hand (§5.3).

**Spellcraft** (`Private Chef`, `Well Wisher`; no Solo minion of Patch 36.6.3 has it) **(engine decision)**:

- A Spellcraft minion gives its player its Spellcraft spell (its `spellcraft` hook) when it is played, once its play has resolved, and at the start of each turn, after the shop is filled (left to right), if there is room in the hand; with a full hand it gives none (a Spellcraft spell never queues).
- The spell is temporary (`CardFlags::TEMPORARY`): still in the hand when the player ends their turn (`EndTurn`), it is discarded, without discard effects. One that reached the hand after that (a Pass) is discarded at the start of the next turn. A Golden minion's spell is Golden: it does what the Golden text says.
- The spells are token spells, not Tavern spells (`NOT_TAVERN_SPELL`): casting one doesn't count as casting a Tavern spell, and `Storm Splitter` doesn't copy them. They can be Passed like any card.

**The Duos spells** are offered by the Duos shop: in a Duos game (`PlayerAuras::duos_game`) a shop's spell is drawn uniformly among the Tavern spells of its Tier or lower and the Duos spells (`Portal in a Fountain`, `Portal in a Crystal`, `Bargain Bundle`) of its Tier or lower (`spells::draw_shop_spell`). A Solo shop draws as before. Effects that generate random or Discovered Tavern spells draw from the Solo spells **(engine decision)**.

**Engine additions** (generic, card-agnostic):

| Addition | What it does | For |
| :--- | :--- | :--- |
| `obtained` hook | The card's own text, once it is bought (in the hand, after its triple check) or Discovered (after it is added to the hand). | "When you buy or Discover this" (`Mirror Monster`). |
| `spellcraft` hook, `CardFlags::TEMPORARY` | The Spellcraft spell a minion gives; temporary cards are discarded at `EndTurn` (and at the start of the turn). | `Private Chef`, `Well Wisher`. |
| `after_friendly_discard` sees the card | The observers get the discarded card (`CardObserverFn`, like `after_pass`). | `Voidpriest Cloner`. |
| `SpellTarget::TavernMinion`, `spell_target_filter` hook | A spell cast only on a minion in the Tavern; which minions a targeted spell can be cast on, on top of its `spell_target`. | `Portal in a Fountain`; non-Golden targets. |
| `Unit::unchosen_to_teammate` | A Discover option that goes to the teammate if another one is chosen. | `Bargain Bundle`. |
| `CardPool::take_unit` | Takes a unit's copies back out of the pool (the inverse of `return_unit`). | `Jumping Jack`. |

### 8.4 The Teammate-Gift Cards

What they give the teammate are gifts (§7.2): a card is a `Card` gift (it queues like a passed card, and is not a Pass), the rest are `Effect` gifts on the teammate's Tavern, delivered after the action, or at the start of the turn after both players' start of turn. What they give their own player comes at once. Golden doubles the number in the text; a repeated Battlecry or end-of-turn trigger (`Brann Bronzebeard`, `Drakkari Enchanter`) gives again.

| Card | Text | How the engine reads it |
| :--- | :--- | :--- |
| `Friendly Saloonkeeper` (`BGDUO_104`), Tier 2 `3/4` | *Battlecry: Your teammate gets a Tavern Coin.* (Golden `6/8`: 2.) | `Card` gifts of `Tavern Coin`s. |
| `Gathering Stormer` (`BGDUO31_201`), Tier 2 Elemental `5/1` | *When you sell this, your teammate gains 1 Gold. (Improves each turn!)* (Golden `10/2`: 2 Gold.) | It improves at the start of each turn it starts on the board, without a cap (`counter`, as `Patient Scout`; a triple keeps the copies' highest); sold, the teammate gains `1 + counter` Gold, twice that if Golden, so a Golden one improves by 2 **(engine decision)**. |
| `Plunder Pal` (`BGDUO_118`), Tier 3 Pirate `2/2` | *At the start of your turn, you and your teammate each gain 1 Gold.* (Golden `4/4`: 2.) | A `turn_start` observer: its player gains the Gold at once; the teammate's arrives after both players' start of turn (§7.2), so their own Gold for the turn doesn't replace it. |
| `Bottom Feeder` (`BGDUO33_140`), Tier 3 Murloc `3/4` | *At the end of your turn, you and your teammate each get a random Tier 1 card.* (Golden `6/8`: two each.) | Each card is drawn on its own, uniformly among the game's Tier 1 minions (§8: the Duos catalog in Duos) and the Tier 1 Tavern spells the shop can offer; generated, not from the pool **(engine decision)**. |
| `Doting Dracthyr` (`BGDUO_107`), Tier 3 Dragon `4/3` | *At the end of your turn, give your teammate's minions +1 Attack.* (Golden `8/6`: +2.) | The minions on the teammate's board gain it, permanently, whether or not the teammate has ended their turn. |
| `Support System` (`BGDUO_109`), Tier 5 Mech `4/5` | *At the end of your turn, give a minion in your teammate's warband Divine Shield.* (Golden `8/10`: 2 minions.) | Random different minions on the teammate's board that don't have Divine Shield (fewer if fewer lack it). The Divine Shield is permanent. |
| `Man'ari Messenger` (`BGDUO_121`), Tier 5 Demon `9/6` | *Battlecry: Minions in your team's Taverns have +3/+2 this game.* (Golden `18/12`: +6/+4.) | In both players' Taverns, as `Staff of Enrichment` (without spell bonuses): the minions in the Tavern now and those it offers later (`tavern_all_atk` / `tavern_all_hp`). |
| `Selfless Sightseer` (`BGDUO31_205`), Tier 5 Dragon `6/2` | *Battlecry: Increase your team's maximum Gold by (1).* (Golden `12/4`: 2.) | For both players, as `Strike Oil`: `max_gold` and `base_max_gold_bonus` go up (above 10 too), with no Gold this turn. |
| `Loyal Mobster` (`BGDUO31_202`), Tier 6 Quilboar `6/5` | *At the end of your turn, this plays a Blood Gem on all your teammate's minions.* (Golden `12/10`: 2.) | Played Blood Gems (`tavern.md` §7.4: not spells cast) on the minions on the teammate's board, of its own player's Blood Gem size **(confirmed)**: its card counter holds that size, which the teammate's Tavern reads in `teammate_counters` (§7.1). |
| `Dark Dazzler` (`BGDUO33_150`), Tier 6 Demon `4/7` | *After your teammate sells a minion, gain its stats. (Once per turn.)* (Golden `8/14`: twice per turn.) | An `after_teammate_sell` observer (§7.3): it gains the sold minion's current Attack and Health, permanently; its `charges` are its uses left this turn (`+1` when made Golden). |

---

## 9. Determinism

A `DuosTavern` is a pure function of its setup, the seeds and the actions taken: the shared pool and RNG are passed in, every call draws in a fixed order (§4.1, §4.3), a Pass draws nothing itself, and combat seeds come from the caller. The scenario runner runs every Duos Tavern scenario twice per seed and checks both runs end identically (`scenarios.md` §6).

---

## 10. RL

The Python bindings (`DuosTavernGame`, `TavernAction.pass_card`, `simulate_duos`, `simulate_duos_batch`) and how Duos maps onto the MDP are described in [`../../rl/README.md`](../../rl/README.md).

---

## 11. Decisions

The rules the user settled are marked **(confirmed)** above: 4 teams of 2, separate Taverns and one pool, teammates not friendly and effects per player; one Health pool of 30 plus both heroes' Armor; per-player hero-damage triggers, and none for combat damage; simultaneous turns, arrival at once, the turn ending when both have ended; the random first fighter that usually switches; Pass from hand to hand for 1 Gold (Gold spent), with no limit, any card but locked ones (`Search Through Time`, a `Lockbox`); the free Passes of `Wanderer Cho`, one per Cho, used before paying; a passed group card counting down its group, arriving plain; the unbounded incoming queue; triples completed by passed cards, with the receiver's Golden and Triple Reward; combat damage to the losing team's pool; the team's result for both players; a sat-out warband keeping its next-combat effects; Duos-exclusive cards only in Duos pools, Naga-only minions in none; gift cards queuing like passed cards; Passes by card effects, free and counting as Passes; random minions from the Duos catalog in Duos; `Sandy` and `Grave Narrator` reading the teammate's warband as it entered the combat; the Orc-estras' team count (+2/+2, then +4/+4, ...), `Mantid King` gaining only keywords it lacks, `Mirror Monster`'s new copies, `Private Selection` and `Bargain Bundle` drawing from the pool up to the player's Tier (`Private Selection` Passing at once), `Jumping Jack` forgetting its sale when made Golden, and `Loyal Mobster`'s Blood Gems of its own player's size.

Engine decisions, pending the user's confirmation (each is one place in `src/duos_tavern.rs` unless noted):

| Decision | Rule | Where |
| :--- | :--- | :--- |
| Mirrored Health | Each player's `health`/`armor` hold the team's, copied in and out around each action (§3.3). | `DuosTavern::mirror`, `settle` |
| Team Armor | "Set your Armor" sets the team's (§3.3). | (falls out of the mirror) |
| Start-of-turn order | Player 0's start of turn, then player 1's (§4.1). | `DuosTavern::start_turn` |
| After `EndTurn` | No more actions that turn; passed cards still arrive (§4.2). | `DuosTavern::is_legal` |
| First fighter | A fair coin on turn 1, then switching with `first_fighter_switch_chance` (default `0.75`), one RNG draw per turn (§4.3). | `DuosTavern::draw_first_fighter` |
| Pass with a pending Discover | Not legal for the passer; the receiver's doesn't matter (§5.1). | `DuosTavern::can_pass` |
| Not a play or discard | A Pass triggers no Battlecry or discard effect of the passed card (§5.3). | `DuosTavern::pass` |
| Queued cards in combat | Not in the warband's hand (§5.4). | `DuosTavern::warband` |
| Keeping next-combat effects | The unused Start of Combat effects (`Turn`) of a warband that sat out survive the next start of turn (§6.3). | `DuosTavern::start_turn` |
| Team counters | Each player counts their own; the team mirrors them into the teammate's `teammate_counters` (§7.1). | `DuosTavern::mirror` |
| Counters at a step-in | A warband stepping in takes its teammate's counts from this combat (§7.1). | `combat.rs` (`DuosTeamState::step_in`) |
| Gifts in combat | They reach the teammate's Tavern after the combat, not their warband (§7.2). | `DuosTavern::apply_results` |
| `Orc-estra Conductor`'s target | The friendly minion at the play position, else to its left, else the left-most (§8.3). | `orc_estra_conductor.rs` |
| `Mantid King` made Golden | The keywords it has until next turn stay temporary (§8.3). | `mantid_king.rs` |
| Spellcraft | The spell on play and at the start of each turn if there is room in the hand; temporary: discarded at `EndTurn` without discard effects (§8.3). | `tavern.rs` |
| The Duos spells | Only in Duos shops, drawn uniformly with the Tavern spells; random Tavern spells stay Solo ones (§8.3). | `spells/mod.rs` |
| `Gathering Stormer` Golden | Improves by 2 each turn: it gives twice `1 + counter` (§8.4). | `gathering_stormer.rs` |
| `Bottom Feeder`'s cards | Uniform among the game's Tier 1 minions and the shop's Tier 1 Tavern spells, generated (§8.4). | `bottom_feeder.rs` |

### Open Questions

1. What is the real chance that the first fighter switches? (To be measured from game logs.)
2. In combat, does a gift to the teammate reach their warband at once (a `Deity` buff, a `San'layn Scribe` death) or only their Tavern after the combat?
