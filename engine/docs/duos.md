# Duos Combat Specification

The team's Tavern phase (one Health pool, Pass) is specified in [`duos_tavern.md`](duos_tavern.md).

This is the exact specification of how Seaglass resolves a Battlegrounds **Duos** combat: one team of two players against another, fought as a relay of the players' warbands. It builds on the Solo specification, [`combat.md`](combat.md): everything this document does not change works exactly as there, and Solo battles are unaffected by it. Like `combat.md`, it is updated **before** behavior changes, and the combat loop stays card-agnostic.

Rules marked **(confirmed)** were settled with the user. Rules marked **(engine decision)** are details the user has not settled yet: the engine implements them as written, and each lives in one place so that it is a small change if the answer differs (§9).

---

## 1. Definitions

- **Team** — Duos has 4 teams of 2 players. A combat is one team against another: team `A` against team `B`. In the battle, a team's fighting warband is its **side** (`combat.md` §2): side `A` in the event log is team A's fighting warband.
- **Player** — A team's two players are numbered `0` and `1`. Each has their own **warband**: their board, hand, player auras (`PlayerAuras`: their player effects, counters, Deity, ...) and Tavern Tier (`hero_tier`).
- **First warband** — For each team, the warband that fights first. It is an input of the combat (in the real game it is picked at random each turn).
- **Fighting warband** — The warband on its team's side of the battle. A team has at most one at a time; its teammate's warband is either **waiting** (it has not fought yet) or **done** (it fought and has no minions left, or it left the fight after a stall, §3.5).
- **Empty warband** — A warband with no minions on its board when the combat starts. It still fights: it steps in and has its Start of Combat like any warband (§3.2).
- **Player context** — Everything Solo keeps per side (`combat.md` §2, §5) is kept per player: the board and its starting warband, the next-attacker pointer, the hand (with the cards summoned from hand and the cards given this combat), the player auras and counters, the Deity and its friendly Aberration deaths, combat-long bonuses (the Beast Attack bonus), the dead units, the queued immediate attacks, and the Tavern Tier.

---

## 2. Teammates Are Not Friendly

Only the fighting warband is on its team's side: teammates' minions are never on the board together, and teammates are not friendly to each other **(confirmed)**.

- "Friendly" means the player's own warband. An Avenge counts only the deaths of its own player's minions; the "whenever a friendly minion attacks / dies / is summoned" observers (`Prodigious Tusker`) and the auras see only their own warband.
- Each warband keeps its own player context (§1): its player effects, counters, hand, Deity and starting warband are its own player's, and only that player's minions count toward them (a player's Deity awakens after 4 of **that player's** Aberrations died).
- "Enemy" means the other team's fighting warband.
- No attack, damage or minion trigger reaches a waiting warband: it steps in as it was given, but for the team counters it takes when it steps in (§3.2) and its teammate's gifts (below), which reach it at once **(confirmed)**.

**What reaches the teammate.** A card that names the teammate or the team (`duos_tavern.md` §7) reaches them only this way; the rules above don't change:

- **Gifts.** "Your teammate gets ...", "you and your teammate each ...", "your team's ...": the card records a gift in its player's auras (`PlayerAuras::to_teammate`). Every gift given during a combat reaches the teammate in that combat, at once, but for what needs the Tavern **(confirmed)**. The team delivers its warbands' gifts before each check for a step-in (§3.2), the last one included, after which the combat ends (`DuosTeamState::deliver_gifts`): the teammate can't act in between, so for all it can tell a gift arrives at once.
  - **A card** (`TeammateGift::Card`: `Generous Geomancer`'s Blood Gem, `Shifty Snake`'s and `Grave Narrator`'s minions) goes into the teammate's combat hand, as a card added to their hand (their "a card is added to your hand" observers see it). A waiting warband steps in with it: a `Flighty Scout` summons its copy at its Start of Combat, and its hand buffs reach it. A done warband brings it back in its result's hand, and so does a waiting warband that never steps in (§6). A card the hand has no room for (10 cards) waits, with the cards given after it, and the teammate's Tavern gets them after the combat, as it gets passed cards (`duos_tavern.md` §5.4).
  - **An effect** (`TeammateGift::Effect`) whose card has a `teammate_combat_gift` hook (`C'Thrax Wrecker`'s Deity buff, `Feisty Freshwater`'s free Refreshes) acts on the teammate's warband: on its player auras, which come back in its result (§6), and on its minions: a waiting warband's board, which steps in with what it gained, or a done warband's survivors. A waiting warband whose Deity awakens later in the combat awakens it with the buff. Like any combat change, what its minions gain stays in the combat unless the effect says otherwise.
  - **What needs the Tavern** waits for it: an effect on the Gold, the upgrade cost or the shop (its card has no `teammate_combat_gift` hook), a gift to the whole team (`team`, which the team's Tavern gives both players: `Wheeled Crewmate`'s cheaper upgrades), and a Pass. They come back in the giver's result's auras (§6), and the team's Tavern delivers them after the combat (`duos_tavern.md` §7.2) **(confirmed)**.
- **Team counters.** "Your team's ... this game" counts are each player's own card counters plus their teammate's (`PlayerAuras::teammate_counters`, mirrored by the team's Tavern). Each warband adds to its own player's counters; a warband that steps in takes its teammate's counters as they are then (§3.2) **(engine decision)**.
- **The teammate's warband.** A card can read its teammate's warband as it entered the combat (`BoardCtx::teammate_board`): the board as given, whether it is waiting, fighting or done; empty in Solo **(confirmed)**.

---

## 3. The Battle Algorithm

### 3.1 Setup

1. **Assign IDs.** Units receive `UnitId`s in board order: team A's first warband, then team B's first warband, then team A's second warband, then team B's second warband (the opening fight numbers its minions as a Solo battle between the first warbands would). Tokens summoned during the combat take the next IDs, after all four warbands.
2. Initialize the PRNG from `seed`: one PRNG for the whole combat.
3. Set each warband's next-attacker pointer to its leftmost unit (`combat.md` §5.1 step 3).
4. Emit `BattleStart` (its `first_attacker` is the side decided in step 7).
5. **Opening.** Each team's first warband steps in (§3.2), even if it is empty **(confirmed)**. Their Start of Combat resolves together, exactly as in a Solo battle (`combat.md` §5.1.1, the order coin flip included).
6. **Step-ins** that the opening Start of Combat caused (§3.2): a first warband that has no minions after it (an empty one that summoned nothing, or one a `Boom-in-a-Box` cleared) makes way for its teammate.
7. **First attacker** (§3.3).
8. **Turn loop** (§3.4).

### 3.2 Stepping In

When a team's fighting warband has no minions left, its teammate's warband **steps in**, if it has not fought yet **(confirmed)**, even if it is empty: it has a Start of Combat like any warband, which may summon minions for it (`Flighty Scout` in hand, `Boon of Beetles`) **(confirmed)**. This is checked where Solo checks for an empty board: after Start of Combat and after each attack turn (`combat.md` §5.1, §5.2). A step-in:

1. emits `WarbandSteppedIn { team, player, first_attacker }` (§7), once the gifts its teammate gave it so far have reached it (§2);
2. puts the warband on its team's side: its board, with its pointer on its leftmost minion, and its player context, with its teammate's card counters as they are now (`teammate_counters`, §2; its minions' "wherever this is" auras resync if they changed);
3. resolves its **Start of Combat** (`combat.md` §5.1.1): its player's own effects (`Upper Hand`, `Brood of Nozdormu`, `Sharing is Caring`), the copies its hand cards summon (`Flighty Scout`), the player effects that fill open board slots (`Boon of Beetles`), then its minions' triggers, left to right, each followed by the deaths it caused **(confirmed)**. The other warband's Start of Combat effects, its player's included, do not trigger again **(confirmed)**.

- **Both at once.** If both teams' fighting warbands have no minions left at the same check (they died at the same time) and both have a teammate waiting, both teammates step in: team A's `WarbandSteppedIn`, then team B's, then their Start of Combat resolves together, as at the start of a Solo battle (`combat.md` §5.1.1: the side with more minions first, a coin flip on an equal count if both have a Start of Combat effect) **(confirmed)**. If only one of them has a teammate waiting, only that teammate steps in.
- **Chains.** If a Start of Combat leaves a fighting warband with no minions (the stepping-in warband's `Boom-in-a-Box` kills the other team's last minions, or an empty warband summoned nothing), that warband's teammate steps in in turn, if it can, before the first attacker is decided.
- **Between turns.** A step-in happens between attack turns, like a new Start of Combat. A turn whose defending warband runs out of minions ends there: a Windfury attacker's attack is done, so it does not strike the stepping-in warband, and an immediate attack that finds no enemy minion is lost, as in Solo: `Eternal Tycoon`'s Knight is summoned but doesn't attack **(confirmed)**.

### 3.3 Attack Order After a Step-In

1. **First attacker.** Once the step-ins' Start of Combat (chains included) has resolved, the first attacker is decided as in Solo (`combat.md` §5.1 step 6): the side with more minions attacks first; on an equal count it is a coin flip (§5) **(confirmed)**.
2. **The warband that was already fighting carries on** where it was in its attack order: its pointer is not reset (`combat.md` §5.4) **(confirmed)**. If `X` of `[X, Y, Z]` attacked last, its next attacker is `Y`, not its leftmost minion.
3. **The stepping-in warband** attacks from its leftmost minion; its pointer moves on as usual if that minion dies during its Start of Combat (`combat.md` §5.4) **(confirmed)**.

### 3.4 Turn Loop

```text
loop:
    run Solo turns (docs/combat.md §5.2) until a fighting warband has no minions left
    if neither side can attack:
        both fighting warbands leave the fight    # a stall (§3.5)
    step in (§3.2)
    if no warband stepped in:
        break                         # -> outcome (§3.5)
    decide the first attacker (§3.3)
```

### 3.5 Outcome

- The combat ends when a team has **no minions left and no warband left to step in** (its teammate has fought) **(confirmed)**.
- Both teams at once (their fighting warbands died at the same time and neither has a teammate waiting) $\to$ **Draw**, as in Solo **(engine decision)**.
- Only team B $\to$ **team A wins**; only team A $\to$ **team B wins**.
- **Stall.** If neither fighting warband can attack, both **leave the fight**, as if they had run out of minions at the same time: each team's teammate steps in if it has not fought yet (§3.2, both at once) **(confirmed)**. The minions that left are not dead: they come back as their player's survivors (§6). A team with no teammate left to step in is out: if neither team has one it is a **Draw**, and if only one team's teammate steps in, that team wins **(confirmed)**.
- **Empty warband.** An empty warband fights like any other: it steps in, at the opening if it is its team's first warband (§3.1 step 5) or when its teammate runs out, and its Start of Combat resolves: its player's own effects, its hand cards' copies (`Flighty Scout`) and `Boon of Beetles` **(confirmed)**. If it still has no minions, its teammate steps in at once (§3.2, chains), or its team is out. Solo works the same way: a player with no minions still has a Start of Combat (`combat.md` §5.1).

---

## 4. Damage

If a team wins, the losing team takes:

```text
winning warband's player's Tavern Tier + sum(tavern_tier of the counted minions)
```

- The **winning warband** is the winning team's fighting warband at the end of the combat. Only its player's Tavern Tier is added, not its teammate's **(confirmed)**.
- The **counted minions** are its surviving minions. If its teammate never stepped in (the winning warband beat both opposing warbands on its own), its teammate's minions fill out the board: the winning warband's survivors first, then the teammate's minions left to right (its board as given), up to **7** minions in total **(confirmed)**.
- Draw $\to 0$.

---

## 5. Randomness

One seeded PRNG, drawn exactly as in Solo (`combat.md` §4), with these draws for the relay:

1. **First-attacker coin flip** — Consumed after the opening (as in Solo, `combat.md` §4.1), and again each time warbands have stepped in, once their Start of Combat (chains included) has resolved (§3.3), only when both fighting warbands then have the same minion count. Draw `v = rng.below(2)`; `v == 0` $\to$ **A** attacks first, `v == 1` $\to$ **B**.
2. **Start of Combat order coin flip** — At the opening, as in Solo (`combat.md` §4.4), between the two opening warbands; when both teams' warbands step in at once, by the same rule between them. A single warband stepping in makes no draw.

**Batch mode (`simulate_duos_batch`)** derives each combat's seed from `base_seed` exactly as `simulate_batch` does (`combat.md` §4).

---

## 6. Results

A Duos combat (`simulate_duos`) returns a `DuosResult`:

- `outcome` — `a_win` (team A won), `b_win` or `draw`.
- `hero_damage` — The damage the losing team takes (§4; `0` on a draw).
- `events` — The event log (§7).
- `team_a`, `team_b` — Each player's `WarbandResult`, in player order (`0`, `1`), so that each player's Tavern can apply its own result (Combat-to-Tavern Persistence, `combat.md` §1):
  - `fought` — `true` if the warband stepped in.
  - `survivors`, `hand`, `auras`, `dead_units` — As in a Solo `BattleResult`, for this player alone. The minions of a warband that left the fight after a stall (§3.5) are its survivors. The `hand` holds the cards its teammate gave it during the combat, and the `auras` what its teammate's effects gave it (§2). The `auras` also hold the gifts its cards gave the teammate that the team's Tavern still has to deliver (`to_teammate`, §2): those that need the Tavern, and the cards the teammate's full hand didn't take. The gifts that reached the teammate in the combat are not in it, so they are not delivered twice.
  - A warband that never fought comes back unchanged: its whole board as `survivors`, its hand and auras as given (its player effects keep their stacks) plus what its teammate's gifts gave them (§2), no dead units.

---

## 7. Event Log

A Duos combat logs Solo's events (`combat.md` §6), where `side` is the team, plus:

- `WarbandSteppedIn { team, player, first_attacker }` — A warband enters the fight: emitted for each opening warband (right after `BattleStart`, team A's first) and for every step-in (§3.2), before its Start of Combat. `first_attacker` is the side that attacks first when the fight resumes, decided after that Start of Combat (§3.3), like `BattleStart`'s.

`BattleEnd`'s `outcome` is the teams' (`a_win`: team A won), and its `hero_damage` is the damage the losing team takes.

---

## 8. Worked Example

Team A — player 0 (first): `["3/3", "1/1"]`, Tavern Tier 2; player 1: `["5/5", "1/1"]`, Tier 4.
Team B — player 0 (first): `["2/2"]`, Tier 3; player 1: `["4/6"]`, Tier 5.

IDs (§3.1): A0's `3/3` = 0, `1/1` = 1; B0's `2/2` = 2; A1's `5/5` = 3, `1/1` = 4; B1's `4/6` = 5.

1. Nobody has a Start of Combat effect. Team A has more minions (2 v 1): A attacks first.
2. A's `3/3` attacks B's `2/2`: the `2/2` dies, the `3/3` is now `3/1`. B's player 0 has no minions left, so B's player 1 steps in with its `4/6`. A has more minions (2 v 1), so A attacks first, and A's warband carries on in its attack order: its next attacker is the `1/1`, not the `3/3`.
3. A's `1/1` attacks the `4/6`: the `1/1` dies, the `4/6` is now `4/5`.
4. B's `4/5` attacks A's `3/1`: the `3/1` dies, the `4/5` is now `4/2`. A's player 0 has no minions left, so A's player 1 steps in. A has more minions (2 v 1): A attacks first, from its leftmost minion.
5. A's `5/5` attacks the `4/2`: the `4/2` dies, the `5/5` is now `5/1`. Team B has no minions left and no warband left to step in: **team A wins**.

Damage: A's player 1's Tier 4 plus its survivors' tiers ($1 + 1$) $= 6$. Player 0's Tier 2 is not added, and there is no fill-out: player 1's teammate fought.

---

## 9. Decisions

The details the user settled:

1. Only the winning warband's player's Tavern Tier is added to the damage (§4) **(confirmed)**.
2. When a warband steps in, the warband that was already fighting carries on in its attack order; only the stepping-in warband starts from its leftmost minion (§3.3) **(confirmed)**.
3. The stepping-in player's own Start of Combat effects (`Upper Hand`, `Brood of Nozdormu`, `Sharing is Caring`, `Boon of Beetles`) trigger when their warband steps in; the other side's do not trigger again (§3.2) **(confirmed)**.
4. When both fighting warbands die at once, both teammates step in, and their Start of Combat is ordered as at the start of a Solo battle (§3.2) **(confirmed)**.
5. A step-in is like a new Start of Combat: a Windfury attacker's attack is done when the defending warband runs out, and `Eternal Tycoon`'s Knight is summoned but doesn't attack (§3.2) **(confirmed)**.
6. A stall makes both fighting warbands leave the fight, and both teammates step in; if only one team has a teammate waiting, that team wins (§3.5) **(confirmed)**.
7. An empty warband still steps in and has its Start of Combat (`Flighty Scout` in hand, player effects), in Duos as in Solo (§3.5) **(confirmed)**.
8. A card that reads its teammate's warband reads it as it entered the combat, even once it has fought (§2; `Sandy`) **(confirmed)**.
9. Every teammate effect produced during a combat reaches the teammate in that combat, at once: a card goes into their combat hand (it waits for their Tavern if the hand is full), an effect acts on their warband's auras and minions (a waiting warband steps in with them); only what needs the Tavern (Gold, the upgrade cost, the shop) waits for it (§2) **(confirmed)**.

Engine decisions, pending the user's confirmation (each is one place in `combat.rs`):

| Decision | Rule | Where |
| :--- | :--- | :--- |
| Both teams run out at once | Draw, `0` damage, as in Solo (§3.5). | `resolve_duos_battle` |
| Unit IDs | First warbands first, A before B, then the second warbands (§3.1). | `resolve_duos_battle` |
| Fill-out board | The teammate's board as given: its hand cards and player effects (`Flighty Scout`, `Boon of Beetles`) summon nothing (§4). | `DuosTeamState::damage` |
| Counters at a step-in | The stepping-in warband takes its teammate's card counters as they are then (§3.2). | `DuosTeamState::step_in` |
