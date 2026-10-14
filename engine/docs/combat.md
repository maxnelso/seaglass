# Combat Phase Specification

This is the exact specification of how Seaglass resolves a single battle. It is the source of truth: the engine implements this document, and this document is updated **before** behavior changes (see the "No Guessing" principle in [`../README.md`](../README.md)).

The combat loop is **card-agnostic**: it knows only about units, keywords, and general trigger hooks. Individual card definitions live in the card catalog ([`tavern.md`](tavern.md)) and plug into the phases defined below.

**Duos** combats (a team of two players against another, fought as a relay of the players' warbands) are specified in [`duos.md`](duos.md), which builds on this document.

---

## 1. Mechanics & Trigger Hooks

### Core Keywords

- **Vanilla stats** — Attack / Health; attacking, taking damage, dying.
- **Taunt** — Must be targeted before non-Taunt minions.
- **Divine Shield** — Absorbs the first instance of damage $> 0$, then pops.
- **Windfury** — A unit selected to attack strikes a **second time**, immediately, if it is still alive after the first strike and the enemy board is non-empty. Windfury is read **once**, before the first strike (§5.3), so Windfury granted mid-turn does not add a strike.
- **Reborn** — On death, resummons a base (or Golden-base) copy of the unit at the dying unit's slot with `health = 1` and `reborn = false` (plus any "wherever this is" / combat summon auras), respecting the 7-unit board cap. Resolved **after** all of that side's Deathrattles from the same deaths, and after the Deity if those deaths awaken it. If the unit has an inherent (printed) Divine Shield (`inherent_divine_shield`), the resummoned copy has its shield restored even if it had already popped; a shield or stat buff that was merely granted to the dying instance is not carried over (§5.3.1 step 9). The copy takes the dying unit's place in its side's attack order (§5.4).
- **Stealth** — Can't be attacked while it has Stealth; a unit loses Stealth when it attacks. If every unit on the defending side has Stealth when an attack picks its target, they all lose Stealth and are attacked normally (§5.3.1 step 2).
- **Venomous** — Destroys any unit it damages in an attack, attacking or defending (a hit absorbed by Divine Shield damages nothing). It is used up by an attack that destroys a unit with it, once that attack's damage is dealt: an attack that also damages the units next to its target (`Blade Collector`) destroys every unit it damages (§5.3.1 step 6).

### Trigger Phases

All trigger effects double in magnitude (or repeat) when the source unit is Golden (`is_golden == true`).

1. **Start of Combat (§5.1.1)** — Resolved once before the first attack turn: the players' Hero Powers first ([`heroes.md`](heroes.md) §7), then the players' own effects, then the minions' triggers, alternating between the sides and left to right within each side. Used for pre-battle board/tribal buffs (`Electric Synthesizer`), combat-long tribal attack auras (`Humming Bird`), board-wide damage (`Boom-in-a-Box`), immediate attacks (`Heroic Broodmother`), hand summons (`Flighty Scout`), and Tavern spell casts (`Runic Arcanist`, §2).
2. **On-Attack / Rally (§5.3.1 step 4)** — Fires when an attacker declares a strike, **after its target is chosen and before any damage**, so a Rally can act on "the target" (`Heroic Underdog`, `Sin'dorei Straight Shot`, `Obsidian Ravager`, `Jailbird Juggernaut`). May buff the attacker (`Glim Guardian`, `Tusked Camper`), summon a unit adjacent to the attacker (`Flittering Bat`, `Expert Aviator` from hand), generate a hand card (`Roadboar`), cast a Tavern spell (`Crimson Vindicator`, §2), or trigger friendly "whenever another friendly minion attacks" observers (`Prodigious Tusker`) on the attacking board in left-to-right order.
3. **On-Damage Taken (§5.5)** — Fires whenever a unit loses health (`amount > 0` and not absorbed by Divine Shield), whatever deals the damage (an attack, a Rally such as `Obsidian Ravager`'s, a Start of Combat effect, ...), e.g. `Very Hungry Winterfinner` buffing a random minion in the controlling player's hand.
4. **Deathrattle (§5.3.1 step 9)** — Fires when a unit dies (`health <= 0`), **before** any Reborn from the same deaths. May summon token(s) at the dying unit's slot, which take its place in the attack order (`Buzzing Vermin`, `Cord Puller`, `Harmless Bonehead`, `Forest Rover`, `Underrot Spawn`), buff friendly unit(s) (`Scarlet Skull`, `Underrot Spawn`), or increment game-long death counters (`Eternal Knight`). A Deathrattle can also be triggered without a death (`Deathstrider`, §5.3.1 step 4). `Titus Rivendare` adds its extra trigger(s) to every friendly Deathrattle, including those `Deathstrider` triggers. Every trigger, extra ones included, counts as a Deathrattle triggered (`PlayerAuras::deathrattles_triggered`), and the "wherever this is" auras are re-synchronized right after each trigger, so the bonuses that count them (`Falling Sky Golem`) apply at once (a `StatBuff` with reason `Death Aura`), not only once the deaths have resolved.
5. **Stat-Threshold Observers** — Checked immediately whenever a unit's stats increase during combat (e.g., `Scarlet Survivor` gaining Divine Shield at 6 Attack).
6. **Combat-to-Tavern Persistence** — Stats a minion gains on the combat board are not kept, unless the effect gives them permanently or `Tarecgosa` (or a `Persistent Poet` next to the Dragon) keeps them: it permanently retains Bonus Keywords and (`double` if Golden) stats gained during combat. A minion that transformed into another card in combat (Duos `Sandy`) keeps nothing from it. Hand buffs (`Very Hungry Winterfinner`), generated hand cards (`Roadboar`) and the player's auras persist back to `TavernState`: game-long counters such as `Eternal Knight` deaths (card-keyed `PlayerAuras::card_counters`) and `tavern_spells_cast`, the Deity's stats (`Faceless Converter`, `Sha of Fear`), "this game" bonuses (`Plaguerunner`'s and `Forsaken Weaver`'s `undead_bonus_attack`, applied to the Undead in the Tavern), and Tavern auras, which also buff the minions already in the Tavern (`tavern_all_*`, and `Living Azerite`'s `tavern_elemental_*` for the Elementals there).

---

## 2. Definitions

- **Unit** — The combat-relevant state of a minion:
  - `id` — A `UnitId` assigned at setup (§5.1), stable for the whole battle even as board positions shift. Tokens summoned mid-battle receive fresh IDs.
  - `attack` (integer $\ge 0$) and `health` (current health; mutates during combat).
  - `tavern_tier` (integer $\ge 1$, default `1`) — Used for hero-damage calculation.
  - `tribe` — The unit's minion types (`docs/tavern.md` §7.8: the 10 lobby types and Naga), used by tribal triggers and auras. `Tribe::None` matches nothing (not even another `None`); dual-tribe units match either constituent tribe, and `All` units match every type, Naga included.
  - `taunt`, `divine_shield`, `windfury`, `reborn` — Active combat keyword flags.
  - `inherent_divine_shield` — `true` when Divine Shield is printed on the card rather than granted by an effect. Unlike `divine_shield`, it never clears when popped, allowing Reborn to restore printed shields (§5.3.1 step 9).
  - `is_golden` — Doubles the effect of the unit's triggers.
  - Trigger hooks / descriptors for Start of Combat, Rally, Deathrattle, and stat-threshold effects.
- **Board** — An ordered list of units for one side, index `0` = **leftmost**. Order is significant: it drives attack order, positional triggers, and summon insertion. A board holds at most **7** units; summons that would exceed the cap are dropped.
- **Side** — `A` or `B`. Each side has a `hero_tier` (integer $\ge 1$, default `1`) and side-wide combat auras carried over from the Tavern phase.
- **Living unit** — A unit currently on the board. Dead units are removed during death resolution, so between strikes "on the board" and "living" are equivalent.
- **Starting warband** — The units on a side's board when the battle starts (§5.1 step 1, before any Start of Combat effect), whether or not they die later. Units summoned during the battle, Start of Combat included, are not part of it (`Lichling Hoarder` copies from it).
- **Death** — A unit dies the moment its current health is $\le 0$.
- **Can attack** — A living unit with `attack > 0`. (`attack == 0` $\to$ cannot attack.)
- **A side can attack** — It has at least one unit that can attack.
- **Spell power in combat** — A Tavern spell cast in combat (`Runic Arcanist`'s Shiny Rings, `Crimson Vindicator`'s Mighty Dragonbreath) gives its stats plus the player's spell power: the permanent "this game" bonuses (`PlayerAuras::spell_bonus_atk` / `spell_bonus_hp` less the board auras last synced in the Tavern, `board_spell_bonus_applied`) plus the spell auras of the friendly units alive when it is cast (`Humon'gozz`, `Enchanted Sentinel`, Magnetized ones included). An aura minion that has died no longer counts, and a board counts its own aura minions even in a standalone battle.
- **Tavern spells cast in combat** — A Tavern spell that one of your minions casts in combat (`Runic Arcanist`'s Shiny Rings, `Crimson Vindicator`'s Mighty Dragonbreath) counts as you casting a Tavern spell. Each cast, in the order of a cast in the Tavern (`docs/tavern.md` §7.4): it counts toward `PlayerAuras::tavern_spells_cast` (`Faceless Converter`), then your living minions' `combat_spell_cast` observers react, left to right (the "whenever you cast a Tavern spell" minions: `Sha of Fear`, `Timecap'n Hooktail`, `Vicious Mindslasher`, `Charging Czarina`, `Forsaken Weaver`, `Living Azerite`), then the spell takes effect. It doesn't become the last Tavern spell you cast (`last_tavern_spell_cast`, `Cataclysmic Harbinger`), and it doesn't count for the observers of every spell cast (`after_spell_cast`: `Felboar`). These spells are untargeted, so no "cast a spell on" minion reacts. What the observers give follows Combat-to-Tavern Persistence (§1): the stats of the minions on the combat board are lost, while the Deity's stats, the Undead Attack bonus and the Tavern Elemental bonus are kept.
- **Summoned from hand** — `Diremuck Forager`, `Expert Aviator` and `Deathly Striker` summon copies of cards in the hand for this combat only. A hand card summoned that way is locked until the start of the next turn: no effect can summon it from hand again this combat. A summon that finds no room on the board doesn't happen and locks nothing, and `Flighty Scout`'s own Start of Combat copy doesn't lock it.

---

## 3. Rules Summary

1. **First attacker** — The side with more minions **after Start of Combat** attacks first; on an equal count it is a coin flip (RNG).
2. **Attack order** — Sides strictly alternate turns. Each side attacks with its minions left $\to$ right via a persistent pointer that wraps to the leftmost after the rightmost. Minions summoned into a dead minion's slot (its Deathrattle summons, its Reborn copy) take its place in that order (§5.4). Units that cannot attack (`attack == 0`) are skipped.
3. **Target** — A uniformly random living enemy minion without Stealth (if they all have Stealth, they lose it first), unless the enemy controls such Taunt minions, in which case a uniformly random one of those **Taunt** minions.
4. **Damage** — Attacker and target deal their attack to each other **simultaneously**. Divine Shield absorbs the first instance of damage $> 0$ and pops instead of losing health. Any unit at $\le 0$ health dies.
5. **End** — Combat ends when at least one side has no minions. Both empty at once $\to$ draw; otherwise the non-empty side wins. Combat also ends in a draw if neither side can attack (§5.2).
6. **Hero damage** — The winner deals `hero_tier + sum(surviving minions' tavern_tier)` to the losing hero. Draw $\to 0$.

---

## 4. Randomness & Determinism

All randomness flows through one seeded PRNG (`SplitMix64`). A battle is a pure function of `(board_a, board_b, state, seed)`, including its event log. Reproducibility depends on the **exact draw order and mapping** below:

1. **First-attacker coin flip** — Consumed **once, after Start of Combat (§5.1 step 6), only when both boards then have the same minion count.** Draw `v = rng.below(2)`; `v == 0` $\to$ **A** attacks first, `v == 1` $\to$ **B**. If the counts differ, **no draw is made**.
2. **Target selection** — Consumed **per strike, only when the candidate set has more than one unit.** Draw `idx = rng.below(k)` where `k` is the number of candidates; the candidate list is ordered by board position (left $\to$ right), and `target = candidates[idx]`. If there is exactly one candidate, **no draw is made**. (Analysis tooling, `combat::with_target_chooser`, can pick the target among the candidates instead, e.g. to replay a real combat's targets; the draw is still made. Combats never use it.)
3. **Random-target triggers (e.g., random friendly/enemy buff or damage)** — Consumed **only when the eligible living candidate set has $k > 1$ units.** Draw `idx = rng.below(k)` over the candidates ordered left $\to$ right on the board (during death resolution, the already-rebuilt prefix left $\to$ right followed by living units in the unprocessed tail left $\to$ right). With $0$ or $1$ candidates, **no draw is made**.
4. **Start of Combat order coin flip** — Consumed **once, at setup before any Start of Combat effect (§5.1.1 step 1), only when both boards have the same minion count and both sides have a Start of Combat effect.** Draw `v = rng.below(2)`; `v == 0` $\to$ **A** goes first, `v == 1` $\to$ **B**. Otherwise **no draw is made**.

Deterministic triggers (positional buffs, board-wide buffs, token summons, Reborn) resolve by board order and consume no RNG.

**Batch mode (`simulate_batch`)** derives each battle's seed deterministically from `base_seed`: initialize one `SplitMix64` RNG from `base_seed`; battle `i` (0-indexed) is run with the `(i + 1)`-th `next_u64()` value drawn from that seeder.

---

## 5. The Battle Algorithm

### 5.1 Setup

1. **Assign IDs.** Units receive `UnitId`s in board order: Side A takes `0 .. len(A) - 1`, Side B continues from `len(A)`. The counter for tokens summoned later in the battle starts at `len(A) + len(B)`.
2. Initialize the PRNG from `seed`.
3. Set each side's **next-attacker pointer** to its leftmost unit. The pointer is tracked by **unit identity**, not by raw index, so it survives board shifts (§5.4); an empty side has no pointer.
4. Emit `BattleStart` (its `first_attacker` is the side decided in step 6).
5. **Start of Combat** (§5.1.1), even for a side with no minions: its player's effects and hand cards still trigger (`Flighty Scout`'s copy, `Boon of Beetles`), and the minions they summon fight.
6. **First attacker**, decided from the board sizes **after** all Start of Combat effects (counting the minions they summoned and killed):
   - `len(A) > len(B)` $\to$ A first.
   - `len(B) > len(A)` $\to$ B first.
   - Equal $\to$ coin flip per §4.1.
7. Set `current = first_attacker`.

(If a board is empty after Start of Combat, no attack loop runs; jump straight to outcome §5.6.)

#### 5.1.1 Start of Combat

1. **Order.** The side with more minions at the start of combat (before any Start of Combat effect) goes first. On an equal count it is a coin flip per §4.4 if both sides have a Start of Combat effect (a minion's, a Hero Power's, a player effect's, or a hand card's); otherwise the order cannot matter, no draw is made, and A goes first.
1b. **Hero Powers** ([`heroes.md`](heroes.md) §7.2), before everything else **(confirmed)**: for the first side, then the other, the player's Hero Power's `hero_start_of_combat` hook (Al'Akir, Illidan, Deathwing, ...), then the damage it deals to enemy minions, the minions it destroys, the deaths (step 4) and its immediate attacks. A player without a hero has nothing here.
2. **Player effects**, for the first side, then the other: the player's own effects (the `player_start_of_combat` hooks of `PlayerAuras::effects`, e.g. `Upper Hand`, `Brood of Nozdormu`, `Sharing is Caring`) in the order they were recorded; then the copies that hand cards summon (`Flighty Scout`), at the right end of the board; then the effects that fill open board slots (the Hero Power's `hero_combat_space`, then `Boon of Beetles`).
3. **Minion triggers.** The minions on each board with a Start of Combat effect (a `start_of_combat` hook or a `SocAction`) are listed now, left to right. They trigger alternately, one per side: the first side's left-most, then the other side's left-most, then the first side's next, and so on; once one side has none left, the other continues in order. A listed minion that is no longer on its board when its turn comes (it died) does not trigger; minions summoned during Start of Combat (including Reborn copies) are not listed. A minion's trigger resolves its `start_of_combat` hook (buffs, summons), then its `SocAction`:
   - `DestroyNeighbors` (`Stitched Salvager`): destroy the friendly minion(s) beside it;
   - `DamageAll` (`Boom-in-a-Box`): deal damage to all other minions, once per wave;
   - `Attack` (`Heroic Broodmother`): attack immediately (a strike per §5.3.1 that does not advance its side's pointer, §5.4), once per attack.
4. **Deaths** caused by an effect resolve right after it, before the next effect: after each hook, destroy, wave of damage and immediate strike, the dead are emitted and resolved per §5.3.1 steps 7 and 9 (the other side first), followed by any queued immediate attacks. So the tokens a first wave of damage summons are hit by the second wave.

### 5.2 Turn Loop

```text
loop:
    if empty(A) or empty(B):
        break                         # -> outcome (§5.6)
    if can_attack(current):
        perform_attack(current)       # §5.3
        current = other(current)
    else:
        if not can_attack(other(current)):
            result = DRAW             # neither side can attack (stall guard)
            break
        current = other(current)      # skip the stalled side's turn
```

- A side that has minions but none that can attack (all `attack == 0`) is skipped and emits `TurnSkipped { side: current }`.
- When neither side can attack, combat ends immediately in a draw (emitting no `TurnSkipped` event).

### 5.3 Performing One Attack Turn (`perform_attack(side)`)

One *turn* is one or two *strikes* by a single attacker:

1. **Select the attacker.** Starting at the side's next-attacker pointer (§5.4), scan right with wraparound; the attacker is the first unit that can attack. Record it by identity, and record **now** whether it has Windfury—this flag is read once, so Windfury granted later in this turn does not add a strike.
2. **Strike once** (§5.3.1).
3. **Windfury second strike.** If the attacker had Windfury, is **still alive**, and the defending board is **non-empty**, strike a second time with the same attacker (driven by recorded identity, not the pointer).

#### 5.3.1 One Strike

1. **Locate the attacker** by identity. If it is no longer on its board the strike is a silent no-op (reachable only on a Windfury second strike).
2. **Choose the target** on the defending side, before any on-attack effect (a Rally can act on "the target"):
   - If every defender has Stealth, they all lose it (§1).
   - `visible = living defenders without Stealth`
   - `taunts = visible defenders with the taunt keyword`
   - `candidates = taunts if taunts is non-empty else visible`
   - If `candidates` is empty: the strike is a silent no-op (no event).
   - If `len(candidates) == 1`: target it (**no RNG draw**).
   - Else: draw per §4.2 over `candidates` ordered left $\to$ right.
3. **Declare the attack**: the attacker loses Stealth, and `AttackDeclared` is emitted.
4. **Fire On-Attack (Rally) triggers** (§1), *before any damage*:
   - Resolve the attacker's own Rally effect (stat buffs, adjacent summons respecting the 7-unit cap, effects on the target, etc.).
   - If the attacker triggered a Rally, resolve any friendly "after a friendly Rally minion attacks" observers (`Deathstrider`) on the attacking board in left-to-right order.
   - Then resolve the friendly "whenever / after a friendly minion attacks" observers (`Ravaging Scorpid`, `Cage Gnawer`, `Prodigious Tusker`), left to right. These also resolve before the damage exchange, so an attacker they buff hits with the buff.
   - Then the deaths these effects caused (`Obsidian Ravager`) are emitted and resolved per steps 7 and 9 (the defending side first).
   - Then a Rally summon that attacks the target first (`Jailbird Juggernaut`'s Golem) strikes it, if both are still alive: it emits its own `AttackDeclared`, triggers the friendly attack observers, and exchanges damage with the target (steps 6, 7 and 9, without the pointer change of step 8).
5. **Re-locate the attacker and the target** (an on-attack summon may have shifted board indices). If the attacker died, the strike ends; if the target died, the strike ends without any damage (the attacking side's pointer still moves past the attacker, §5.4). Capture `attacker_attack` and `target_attack` here, **after** the on-attack triggers and **before** any damage, so the exchange is simultaneous.
6. **Deal simultaneous damage**, emitting each result in application order—**target first, then attacker**:
   - `apply_damage(target, attacker_attack)`
   - `apply_damage(attacker, target_attack)`
   - If the attacker also damages the units next to its target (`Blade Collector`): `apply_damage(neighbour, attacker_attack)` for the target's left, then right neighbour.
   - Each call emits `DamageDealt` if the unit lost health, `DivineShieldPopped` if its Divine Shield absorbed the hit, or **nothing** if the amount was $\le 0$. A Venomous attacker destroys every unit these calls damaged (target and neighbours) and its Venomous is used up if it destroyed any; a Venomous target destroys the attacker if it damaged it, and its Venomous is used up likewise.
   - Then the attacker's "after this attacks" effects (excess damage to the target's neighbours, `Wildfire Elemental`; bonus damage to the highest-Health enemy, `De-volition-ist`). They fire even if the attacker died in the exchange.
7. **Emit a `Death`** for every unit now at $\le 0$ health, ordered **defending side first (left $\to$ right), then attacking side (left $\to$ right)**.
8. **Set the attacking side's pointer** per §5.4 while the dead are still on the board (the defending side's pointer stays on its unit).
9. **Resolve deaths**, **defending side first, then attacking side**. For one side, in four passes (summons respect the 7-unit board cap), then a pointer update:
   1. **Deaths**: walk the board left $\to$ right and rebuild it. Living units are copied across unchanged; for each dead unit, resolve its friendly-death triggers (Avenge), then its **Deathrattle**, inserting summons at the dead unit's position. A Deathrattle may destroy a unit on either board (`Leeroy the Reckless` destroys its killer, even a friendly one): it is destroyed at once (a `Death` is emitted), and its death resolves in a further round of death resolution for its side once this one is over. It stays destroyed until then: a buff that reaches it first (a later Deathrattle's, `Ship Master Eudora` giving your minions +6/+6) doesn't bring it back, and that round removes it whatever its Health.
   2. **Deity**: once the side has had 4 friendly Aberration deaths this combat (`DEITY_SACRIFICE_REQUIREMENT`), its Deity awakens: it is summoned at the right end of the board (if there is room; otherwise after a later round of deaths), `DeityAwakened` is emitted, and its awaken effect resolves (`C'Thun`).
   3. **Reborn**, left $\to$ right: each dead unit with Reborn is resummoned at its position (after its Deathrattle summons) as a base (or Golden-base) copy with `health = 1`, `reborn = false`, printed keywords restored, and global/combat summon auras applied.
   4. **Open slots**: summons that were waiting for room on the board (`Boon of Beetles`, a `Diremuck Forager` that found the board full at Start of Combat) happen at its right end, one per open slot.
   5. **Pointer**: if the side's pointer is on a unit that died, re-anchor it per §5.4.

### 5.4 Pointer Advance

Each side's pointer names, by **unit identity**, the unit it attacks with next (§5.3 step 1 scans right from it, wrapping, to the first unit that can attack).

- **Attacking side**: after a strike's damage (§5.3.1 step 8), its pointer moves to the unit **immediately to the right of the attacker** (wrapping to the leftmost after the rightmost), dead or alive; if the attacker itself died, the pointer stays on the attacker. Immediate attacks (Start of Combat, `Eternal Tycoon`'s Knight) and summons that attack the target first (`Jailbird Juggernaut`'s Golem) don't move it.
- **Defending side**: its pointer stays on the **same unit**.

Whenever a side's deaths resolve (§5.3.1 step 9), a pointer on a unit that died is re-anchored:

1. to the **left-most unit summoned into its slot** (its Deathrattle summons, then its Reborn copy), if any is on the board: they take its place in the attack order;
2. otherwise to the next unit of the board as it was before those deaths (left $\to$ right), applying the same rule if that unit died too;
3. past the rightmost unit, it wraps to the board's leftmost unit.

Units summoned elsewhere (an Avenge summon beside its source, a Deity or Beetles at the right end) don't take a dead unit's place; they attack when the order reaches them.

Example for the attacking side:
- `[X, Y, Z]`, `X` attacks and nobody dies $\to$ next is `Y`.
- `[X, Y, Z]`, `X` attacks and `X` dies $\to$ board `[Y, Z]`, next is `Y`.
- `[X, Y, Z]`, `X` (`Harmless Bonehead`) attacks and dies $\to$ board `[S1, S2, Y, Z]`, next is its left Skeleton `S1`, then `S2`, then `Y`.
- `[X, Y, Z]`, `Z` attacks and nobody dies $\to$ wraps, next is `X`.

Example for the defending side (pointer on `Y`):
- `[X, Y, Z]`, `X` (left of `Y`) dies while defending $\to$ board `[Y, Z]`, pointer remains `Y`.
- `[X, Y, Z]`, `Y` dies while defending $\to$ board `[X, Z]`, pointer advances to `Z`.
- `[X, Y, Z]`, `Y` dies while defending and is Reborn as `Y'` $\to$ board `[X, Y', Z]`, pointer moves to `Y'`.

### 5.5 `apply_damage(unit, amount)`

```text
if amount <= 0:
    return                        # 0 (or negative) damage: no effect, no shield pop
if unit.divine_shield:
    unit.divine_shield = false    # pop; no health lost
else:
    unit.health -= amount
```

### 5.6 Outcome

- Both boards empty $\to$ **Draw**
- Only A empty $\to$ **B wins**
- Only B empty $\to$ **A wins**
- Stall (neither side can attack) $\to$ **Draw**

**Hero damage**: If there is a winner, the loser's hero takes `winner.hero_tier + sum(winner survivors' tavern_tier)`. Draw $\to 0$.

---

## 6. Event Log

Every single-battle simulation produces an ordered, structured event log:

- `BattleStart { seed, first_attacker }` — always the first event; `first_attacker` is the side decided after Start of Combat (§5.1 step 6).
- `AttackDeclared { side, attacker, target }`
- `DamageDealt { unit, amount, from }`
- `DivineShieldPopped { unit }`
- `Death { unit }`
- `TurnSkipped { side }`
- `BattleEnd { outcome, hero_damage }`
- `WarbandSteppedIn { team, player, first_attacker }` — Duos combats only ([`duos.md`](duos.md) §7).
- `UnitTransformed { side, unit, name, attack, health }` — a minion transformed into a copy of another card, keeping its id and place (Duos `Sandy`, [`duos_tavern.md`](duos_tavern.md) §8.2).

---

## 7. Worked Example

Boards (both hero tier 1):
- **Side A**: `["3/2 divine_shield"]`
- **Side B**: `["2/3"]`

Say Side A attacks first. A's `3/2` (Divine Shield) strikes B's `2/3`:
- Damage to B's unit: `3` $\to$ health `3 - 3 = 0` $\to$ dies.
- Damage to A's unit: `2` $\to$ absorbed by Divine Shield $\to$ shield pops, no health lost.

Side B is now empty; Side A survives with a `3/2` (no shield). **Side A wins**, dealing `hero_tier(1) + tavern_tier(1) = 2` hero damage.
