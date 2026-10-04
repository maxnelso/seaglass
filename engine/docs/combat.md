# Combat Phase Specification

This is the exact specification of how Seaglass resolves a single battle. It is the source of truth: the engine implements this document, and this document is updated **before** behavior changes (see the "No Guessing" principle in [`../README.md`](../README.md)).

The combat loop is **card-agnostic**: it knows only about units, keywords, and general trigger hooks. Individual card definitions live in the card catalog ([`tavern.md`](tavern.md)) and plug into the phases defined below.

---

## 1. Mechanics & Trigger Hooks

### Core Keywords

- **Vanilla stats** — Attack / Health; attacking, taking damage, dying.
- **Taunt** — Must be targeted before non-Taunt minions.
- **Divine Shield** — Absorbs the first instance of damage $> 0$, then pops.
- **Windfury** — A unit selected to attack strikes a **second time**, immediately, if it is still alive after the first strike and the enemy board is non-empty. Windfury is read **once**, before the first strike (§5.3), so Windfury granted mid-turn does not add a strike.
- **Reborn** — On death, resummons a base (or Golden-base) copy of the unit at the dying unit's slot with `health = 1` and `reborn = false` (plus any "wherever this is" / combat summon auras), respecting the 7-unit board cap. Resolved **after** that same unit's Deathrattle. If the unit has an inherent (printed) Divine Shield (`inherent_divine_shield`), the resummoned copy has its shield restored even if it had already popped; a shield or stat buff that was merely granted to the dying instance is not carried over (§5.3.1 step 9).

### Trigger Phases

All trigger effects double in magnitude (or repeat) when the source unit is Golden (`is_golden == true`).

1. **Start of Combat (§5.1 step 6)** — Resolved once before the first attack turn, Side A first, then Side B, in left-to-right board order. Used for pre-battle board/tribal buffs (`Electric Synthesizer`), combat-long tribal attack auras (`Humming Bird`), and hand summons (`Flighty Scout`).
2. **On-Attack / Rally (§5.3.1 step 2)** — Fires when an attacker declares a strike, **before target selection and before any damage**. May buff the attacker (`Glim Guardian`, `Tusked Camper`), summon a unit adjacent to the attacker (`Flittering Bat`, `Expert Aviator` from hand), generate a hand card (`Roadboar`), or trigger friendly "whenever another friendly minion attacks" observers (`Prodigious Tusker`) on the attacking board in left-to-right order.
3. **On-Damage Taken (§5.5)** — Fires whenever a unit loses health (`amount > 0` and not absorbed by Divine Shield), e.g. `Very Hungry Winterfinner` buffing a random minion in the controlling player's hand.
4. **Deathrattle (§5.3.1 step 9)** — Fires when a unit dies (`health <= 0`), **before** that unit's Reborn. May summon token(s) at the dying unit's slot (`Buzzing Vermin`, `Cord Puller`, `Harmless Bonehead`, `Forest Rover`, `Underrot Spawn`), buff friendly unit(s) (`Scarlet Skull`, `Underrot Spawn`), or increment game-long death counters (`Eternal Knight`).
5. **Stat-Threshold Observers** — Checked immediately whenever a unit's stats increase during combat (e.g., `Scarlet Survivor` gaining Divine Shield at 6 Attack).
6. **Combat-to-Tavern Persistence** — `Tarecgosa` permanently retains Bonus Keywords and (`double` if Golden) stats gained during combat; hand buffs (`Very Hungry Winterfinner`), generated hand cards (`Roadboar`), and game-long counters such as `Eternal Knight` deaths (card-keyed `PlayerAuras::card_counters`) persist back to `TavernState`.

---

## 2. Definitions

- **Unit** — The combat-relevant state of a minion:
  - `id` — A `UnitId` assigned at setup (§5.1), stable for the whole battle even as board positions shift. Tokens summoned mid-battle receive fresh IDs.
  - `attack` (integer $\ge 0$) and `health` (current health; mutates during combat).
  - `tavern_tier` (integer $\ge 1$, default `1`) — Used for hero-damage calculation.
  - `tribe` — Used by tribal triggers and auras. `Tribe::None` matches nothing (not even another `None`); dual-tribe units match either constituent tribe.
  - `taunt`, `divine_shield`, `windfury`, `reborn` — Active combat keyword flags.
  - `inherent_divine_shield` — `true` when Divine Shield is printed on the card rather than granted by an effect. Unlike `divine_shield`, it never clears when popped, allowing Reborn to restore printed shields (§5.3.1 step 9).
  - `is_golden` — Doubles the effect of the unit's triggers.
  - Trigger hooks / descriptors for Start of Combat, Rally, Deathrattle, and stat-threshold effects.
- **Board** — An ordered list of units for one side, index `0` = **leftmost**. Order is significant: it drives attack order, positional triggers, and summon insertion. A board holds at most **7** units; summons that would exceed the cap are dropped.
- **Side** — `A` or `B`. Each side has a `hero_tier` (integer $\ge 1$, default `1`) and side-wide combat auras carried over from the Tavern phase.
- **Living unit** — A unit currently on the board. Dead units are removed during death resolution, so between strikes "on the board" and "living" are equivalent.
- **Death** — A unit dies the moment its current health is $\le 0$.
- **Can attack** — A living unit with `attack > 0`. (`attack == 0` $\to$ cannot attack.)
- **A side can attack** — It has at least one unit that can attack.

---

## 3. Rules Summary

1. **First attacker** — The side with more minions attacks first; on an equal count it is a coin flip (RNG).
2. **Attack order** — Sides strictly alternate turns. Each side attacks with its minions left $\to$ right via a persistent pointer that wraps to the leftmost after the rightmost. Units that cannot attack (`attack == 0`) are skipped.
3. **Target** — A uniformly random living enemy minion, unless the enemy controls living Taunt minions, in which case a uniformly random living **Taunt** minion.
4. **Damage** — Attacker and target deal their attack to each other **simultaneously**. Divine Shield absorbs the first instance of damage $> 0$ and pops instead of losing health. Any unit at $\le 0$ health dies.
5. **End** — Combat ends when at least one side has no minions. Both empty at once $\to$ draw; otherwise the non-empty side wins. Combat also ends in a draw if neither side can attack (§5.2).
6. **Hero damage** — The winner deals `hero_tier + sum(surviving minions' tavern_tier)` to the losing hero. Draw $\to 0$.

---

## 4. Randomness & Determinism

All randomness flows through one seeded PRNG (`SplitMix64`). A battle is a pure function of `(board_a, board_b, state, seed)`, including its event log. Reproducibility depends on the **exact draw order and mapping** below:

1. **First-attacker coin flip** — Consumed **once, at setup, only when both boards have the same minion count.** Draw `v = rng.below(2)`; `v == 0` $\to$ **A** attacks first, `v == 1` $\to$ **B**. If the counts differ, **no draw is made**.
2. **Target selection** — Consumed **per strike, only when the candidate set has more than one unit.** Draw `idx = rng.below(k)` where `k` is the number of candidates; the candidate list is ordered by board position (left $\to$ right), and `target = candidates[idx]`. If there is exactly one candidate, **no draw is made**.
3. **Random-target triggers (e.g., random friendly/enemy buff or damage)** — Consumed **only when the eligible living candidate set has $k > 1$ units.** Draw `idx = rng.below(k)` over the candidates ordered left $\to$ right on the board (during death resolution, the already-rebuilt prefix left $\to$ right followed by living units in the unprocessed tail left $\to$ right). With $0$ or $1$ candidates, **no draw is made**.

Deterministic triggers (positional buffs, board-wide buffs, token summons, Reborn) resolve by board order and consume no RNG.

**Batch mode (`simulate_batch`)** derives each battle's seed deterministically from `base_seed`: initialize one `SplitMix64` RNG from `base_seed`; battle `i` (0-indexed) is run with the `(i + 1)`-th `next_u64()` value drawn from that seeder.

---

## 5. The Battle Algorithm

### 5.1 Setup

1. **Assign IDs.** Units receive `UnitId`s in board order: Side A takes `0 .. len(A) - 1`, Side B continues from `len(A)`. The counter for tokens summoned later in the battle starts at `len(A) + len(B)`.
2. Initialize the PRNG from `seed`.
3. Set each side's **next-attacker pointer** to its leftmost unit. The pointer is tracked by **unit identity**, not by raw index, so it survives board shifts (§5.4); an empty side has no pointer.
4. **First attacker**, decided from the board sizes **before** any Start of Combat effect:
   - `len(A) > len(B)` $\to$ A first.
   - `len(B) > len(A)` $\to$ B first.
   - Equal $\to$ coin flip per §4.1.
5. Emit `BattleStart`.
6. **Apply Start of Combat triggers** (§1): Side A first, then Side B, in left-to-right board order.
7. Set `current = first_attacker`.

(If a board starts empty, no attack loop runs; jump straight to outcome §5.6.)

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
2. **Fire On-Attack (Rally) triggers** (§1), *before targeting and before any damage*:
   - Resolve the attacker's own Rally effect (stat buffs, adjacent summons respecting the 7-unit cap, etc.).
   - If the attacker triggered a Rally, resolve any friendly "whenever a Rally triggers" observers on the attacking board in left-to-right order.
3. **Re-locate the attacker** (an on-attack summon may have shifted board indices) and capture `attacker_attack`, its Attack **after** on-attack triggers.
4. **Choose the target** on the defending side:
   - `taunts = living defenders with the taunt keyword`
   - `candidates = taunts if taunts is non-empty else all living defenders`
   - If `candidates` is empty: the strike is a silent no-op (no event).
   - If `len(candidates) == 1`: target it (**no RNG draw**).
   - Else: draw per §4.2 over `candidates` ordered left $\to$ right.
   - Capture `target_attack` here, **before** any damage, so the exchange is simultaneous.
5. **Emit `AttackDeclared`.**
6. **Deal simultaneous damage**, emitting each result in application order—**target first, then attacker**:
   - `apply_damage(target, attacker_attack)`
   - `apply_damage(attacker, target_attack)`
   - Each call emits `DamageDealt` if the unit lost health, `DivineShieldPopped` if its Divine Shield absorbed the hit, or **nothing** if the amount was $\le 0$.
7. **Emit a `Death`** for every unit now at $\le 0$ health, ordered **defending side first (left $\to$ right), then attacking side (left $\to$ right)**.
8. **Re-anchor both pointers** per §5.4 while the dead are still on their boards.
9. **Resolve deaths**, **defending side first, then attacking side**. For one side: walk the board left $\to$ right and rebuild it. Living units are copied across unchanged; for each dead unit, resolve its **Deathrattle** first, then its **Reborn**, inserting each summon at the dead unit's position:
   - Summons respect the 7-unit board cap.
   - **Reborn** instantiates a base (or Golden-base) copy of the unit with `health = 1`, `reborn = false`, printed keywords restored, and global/combat summon auras applied.

### 5.4 Pointer Advance

Both pointers are re-anchored by **unit identity** after damage (§5.3.1 step 6) and before death resolution (§5.3.1 step 9):

- **Attacking side**: Take its units in cyclic order starting immediately after the attacker (`attacker + 1, ..., end, 0, ..., attacker`). The new pointer is the **first unit in that order that is still alive**.
- **Defending side**: Keeps the **same unit** if it is still alive; otherwise moves to the **first still-living unit at or after its position** (cyclically).

Example for the attacking side:
- `[X, Y, Z]`, `X` attacks and nobody dies $\to$ next is `Y`.
- `[X, Y, Z]`, `X` attacks and `X` dies $\to$ board `[Y, Z]`, next is `Y`.
- `[X, Y, Z]`, `Z` attacks and nobody dies $\to$ wraps, next is `X`.

Example for the defending side (pointer on `Y`):
- `[X, Y, Z]`, `X` (left of `Y`) dies while defending $\to$ board `[Y, Z]`, pointer remains `Y`.
- `[X, Y, Z]`, `Y` dies while defending $\to$ board `[X, Z]`, pointer advances to `Z`.

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

- `BattleStart { seed, first_attacker }`
- `AttackDeclared { side, attacker, target }`
- `DamageDealt { unit, amount, from }`
- `DivineShieldPopped { unit }`
- `Death { unit }`
- `TurnSkipped { side }`
- `BattleEnd { outcome, hero_damage }`

---

## 7. Worked Example

Boards (both hero tier 1):
- **Side A**: `["3/2 divine_shield"]`
- **Side B**: `["2/3"]`

Say Side A attacks first. A's `3/2` (Divine Shield) strikes B's `2/3`:
- Damage to B's unit: `3` $\to$ health `3 - 3 = 0` $\to$ dies.
- Damage to A's unit: `2` $\to$ absorbed by Divine Shield $\to$ shield pops, no health lost.

Side B is now empty; Side A survives with a `3/2` (no shield). **Side A wins**, dealing `hero_tier(1) + tavern_tier(1) = 2` hero damage.
