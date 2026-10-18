# Deferred

The single list of what the engine defers: the systems it doesn't have, and the heroes, trinkets,
Dark Gifts and cards that are in its tables but not implemented, each with why. A deferred hero,
trinket or Dark Gift is never offered. The specifications keep only a link here (and their own
rules for what is in scope); update this list whenever something is deferred or implemented.

Status: **deferred** (not planned now) or **out of scope** (not part of the engine).

---

## 1. Systems

| System | Status | Why / what it blocks |
| :--- | :--- | :--- |
| Buddies (companions, the Buddy meter) | Deferred **(confirmed)** | No Buddy system: `E.T.C., Band Manager`, the Buddy trinkets, `Ticket Collector` (Tickatus's Buddy). `Mishmash` (The Curator's Buddy) exists only as the minion `Curator Sticker` gives ([`trinkets.md`](trinkets.md) §11.9). |
| Quests | Deferred **(confirmed)** | `Sire Denathrius`, the Quest trinkets. |
| Timewarp | Deferred **(confirmed)** | `Timewarped Big Winner!` ([`darkmoon_prizes.md`](darkmoon_prizes.md) §1), `Morchie`, `Murozond, Unbounded`, `Timeworn Candelabra` (both), `Poet Portrait` and the other Timewarp trinkets. |
| Triple Prizes | Deferred **(confirmed)** | `Corrupted Tome`. |
| Anomalies | Deferred **(confirmed)**; the Dark Gift anomaly (`BG36_Anomaly_002`) is out of scope **(confirmed)** | Lobby-wide rule changes, the Darkmoon Prize anomalies `Up-Prizing` and `A Faire Reward` included ([`darkmoon_prizes.md`](darkmoon_prizes.md) §1). |
| Secrets | Deferred **(confirmed)** | `The Great Akazamzarak`. (`Safety Patch`'s Ice Block is the trinket's own state, not a Secret: [`trinkets.md`](trinkets.md) §11.9.) |
| Spellcraft outside Duos | Deferred **(confirmed)** | `Lady Vashj`. The Duos Spellcraft minions and the Spellcraft trinkets are implemented ([`duos_tavern.md`](duos_tavern.md) §8.3, [`trinkets.md`](trinkets.md) §11.6). |
| Custom Undead creations | Deferred **(confirmed)** | `Professor Putricide`, `Putricide Sticker`. |
| The Wheel of Yogg-Saron | Deferred **(confirmed)** | `Yogg-Tastic Pastry`. |
| StarCraft multi-piece boards and upgrade spells | Deferred **(confirmed)** | `Jim Raynor`, `Kerrigan, Queen of Blades`, `Artanis`. |
| Naga | Out of scope: no lobby of Patch 36.6.3 has Naga **(confirmed)** | `Queen Azshara`; the Naga-only minions are implemented but in no pool (`Private Chef`, `Storm Splitter`: [`duos_tavern.md`](duos_tavern.md) §8); a trinket whose only type is Naga is never offered ([`trinkets.md`](trinkets.md) §1). |
| 4-team Duos matches and their pairings | Out of scope | The engine has one team's Tavern; a match of four teams is the caller's job ([`duos_tavern.md`](duos_tavern.md) §1). |
| Duos gift delivery during combat | Out of scope | The incoming queue is delivered in the Tavern ([`duos_tavern.md`](duos_tavern.md) §6). |

Bounties are not a missing system: the engine has the 5 Bounty spells (`spells::BOUNTY_SPELL_IDS`, `CardFlags::BOUNTY`), so `Sunken Anchor` and `Privateer Portrait` are implemented. The systems above have no code in the engine (only their cards' table rows); `Silas Darkmoon`'s Darkmoon Tickets are his Hero Power, not Darkmoon Prizes. Darkmoon Prizes are implemented ([`darkmoon_prizes.md`](darkmoon_prizes.md)).

## 2. Heroes

In the hero table, not implemented, never offered ([`heroes.md`](heroes.md) §1, §6).

| Hero | Status | Why |
| :--- | :--- | :--- |
| `Artanis`, `Jim Raynor`, `Kerrigan, Queen of Blades` | Deferred **(confirmed)** | StarCraft boards and upgrade spells. |
| `Buttons`, `Marin the Manager` | Deferred **(confirmed)** | Systems the engine doesn't have ([`heroes.md`](heroes.md) §1). |
| `E.T.C., Band Manager` | Deferred **(confirmed)** | Buddies. |
| `Lady Vashj` | Deferred **(confirmed)** | Spellcraft outside Duos. |
| `Morchie`, `Murozond, Unbounded` | Deferred **(confirmed)** | Timewarp. |
| `Professor Putricide` | Deferred **(confirmed)** | Custom Undead creations. |
| `Queen Azshara` | Out of scope **(confirmed)** | Her Hero Power Discovers Naga. |
| `Sire Denathrius` | Deferred **(confirmed)** | Quests. |
| `The Great Akazamzarak` | Deferred **(confirmed)** | Secrets. |

## 3. Trinkets

In the trinket table, not implemented, never offered ([`trinkets.md`](trinkets.md) §11.3).

| Trinket | Status | Why |
| :--- | :--- | :--- |
| `Timeworn Candelabra` (Lesser and Greater) | Deferred **(confirmed)** | Timewarp. |
| `Tickatus Sticker` | Deferred **(confirmed)** | Darkmoon Prizes. |
| `Corrupted Tome` | Deferred **(confirmed)** | Triple Prizes. |
| `Poet Portrait` | Deferred **(confirmed)** | Timewarp (it gives a Timewarped Poet). |
| `Surprise Portrait` | Deferred **(confirmed)** | Its hidden rule ("More appear when you least expect them") is unknown. |
| `Wisdomball Supply` | Deferred **(confirmed)** | Its spell's "helpful" Refreshes aren't worth a rule now. |
| `Yogg-Tastic Pastry` | Deferred **(confirmed)** | The Wheel of Yogg-Saron (its outcomes and odds) is a system of its own. |
| `Putricide Sticker` | Deferred **(confirmed)** | Custom Undead creations. |
| Any trinket whose only type is Naga | Out of scope **(confirmed)** | Naga. |

## 4. Dark Gifts

| What | Status | Why |
| :--- | :--- | :--- |
| The Dark Gift anomaly (`BG36_Anomaly_002`) | Out of scope **(confirmed)** | Anomalies ([`dark_gifts.md`](dark_gifts.md) §1). |
| A gift of the table the engine doesn't implement | Never offered **(engine decision)** | All 47 gifts are implemented today ([`dark_gifts.md`](dark_gifts.md) §11.3). |
