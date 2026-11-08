# 9kgamerbot

`9kgamerbot` (`seaglass_rl/gamerbot.py`, `NineKGamerBot`) is a rule-based bot that plays a simple human plan:

1. **Level.** Level up to Tavern Tier 4, spending the rest of the Gold on the most stats it can get.
   - It uses the Dark Discovery button whenever the shop has nothing it is looking for.
   - It buys no Tavern spells until it commits (it still casts the ones it gets).
   - Once the board is full, it sells its weakest minion for a clearly better one.
   - `9kgamerbot` levels every turn it can afford to. `9kgamerbot-tempo` levels only when it can still buy a minion afterwards (or on turn 2), so it keeps a board.
2. **Roll for a lynchpin.** At Tier 4 with a full board, it rolls until a *lynchpin* shows up. A lynchpin is a build-around card of a direction whose minion type is in the lobby.
   - It always keeps enough Gold to buy what it finds, and freezes a lynchpin it can't afford.
   - If it is still uncommitted on turn 8 (`level5_turn`), it levels to Tier 5 and rolls there for the Tier 5 lynchpins.
3. **Commit.** Once it owns a lynchpin at Tier 4 or higher, it switches to building that direction:
   - it prefers the direction's cards to raw stats, and sells minions outside it as replacements arrive;
   - it keeps pairs for triples, and rolls for direction cards;
   - it levels to Tier 5 and then 6 when it "feels strong": a full board, and Health to spare (12+), plus either a won last combat or 24+ Health. It levels anyway by turn 10 (Tier 5) and turn 13 (Tier 6).

It never passes cards (Duos) and never repositions.

## Directions

`DIRECTIONS` in `gamerbot.py` is the source of truth; this table is a summary. If several lynchpins show up at once, it takes the earliest direction in the list.

| Direction | Needs type | Lynchpins (Tier 5 / Tier ≤ 4) | Payoffs |
|---|---|---|---|
| Undead | Undead | Eternal Summoner, Eternal Tycoon, Barrier Banshee, Lichling Hoarder / Plaguerunner, Eternal Knight | Forsaken Weaver, Snazzy Phantom, Deathly Striker |
| Beasts | Beast | Turquoise Skitterer, Lurking Leviathan, Goldrinn, Ghastcoiler, Sewer Lord / Banana Slamma, Cage Gnawer | Ravaging Scorpid, Deathstrider, Stalwart Kodo |
| Quilboar | Quilboar | Sanguine Refiner, Razorfen Vineweaver / Hot-Air Surveyor, Bonker, Geomagus Roogug | Sanguine Champion, Victorious Geomant, Veteran Brigand |
| Eat the Tavern | Demon or Elemental | Insatiable Ur'zul, Deft Deserter, Devilish Distractor / Flaming Enforcer, Soulkeeping Jailer | Champion of Sargeras, Twisted Wrathguard |
| Elementals | Elemental | Living Azerite, Air Revenant / En-Djinn Blazer, Waveling, Refreshing Anomaly | Unbound Tempest, Ultraviolet Ascendant, Stone Age Slab |
| Pirates: gold | Pirate | Elite Navigator, Enterprising Escapee, Ship Master Eudora / Gunpowder Courier | Sky Admiral Rogers, Hooktusk, Silent Deliverer, Captain Sanders |
| Pirates: Bounties | Pirate | Proud Privateer, Shipwrecked Rascal / Bigwig Bandit | Sky Admiral Rogers |
| Dragons | Dragon | Hopebringer, Kalecgos / Persistent Poet, Tarecgosa | Heroic Broodmother, Nadina the Red, Crimson Vindicator |
| Mechs | Mech | Spark Snapper, Resourceful Robot, Charging Czarina / Drone Duplicator, Auto Accelerator | Utility Drone, Auto Reveille, Polarizing Beatboxer |
| Murlocs | Murloc | Sewer Escapee, Hackerfin, Shamanic Tidecaller, Primalfin Lookout / Kelp Keeper, Bream Counter | Young Murk-Eye, Choral Mrrrglr, Futurefin |
| Battlecries | — | Brann Bronzebeard | (any Battlecry minion) |
| Deathrattles | — | Titus Rivendare | (any Deathrattle minion) |
| End of turn | — | Drakkari Enchanter | (any end-of-turn minion) |
| Tavern spells | — | Cataclysmic Harbinger, Rodeo Performer, Felfire Conjurer, Felboar / Humon'gozz | Balinda Stonehearth |
| Demons: hero damage | Demon | Tichondrius | Eredar Escapist |
| Aberrations | Aberration | Faceless Converter, Mindbender Ghur'sha, Mysterious K'Thir / Cutthroat K'Thir, Parasitic Fleshling | The Shadow of Doubt, Harbinger Aph'lass, Sha of Fear |

The picks come from the engine's card text and the current meta (Patch 36.6). They are meant to be edited.

## Use

Use it like the other bots, by name (`play.BOTS`), e.g.:

```bash
python -m seaglass_rl.evaluate 9kgamerbot --opponents heuristic --games 96
python -m seaglass_rl.evaluate runs/NAME/latest.pt --opponents 9kgamerbot --duos --games 96
```

A bot's turn may take up to `play.BOT_MAX_STEPS_PER_TURN` (100) steps, more than a model's `max_steps_per_turn`, because it rolls, buys and sells many times a turn.
