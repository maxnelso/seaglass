"""GrandmasterBot: Pure First-Principles Patch 37.0.2 Hearthstone Battlegrounds Bot.

Zero inheritance from `SovereignBot`! Built from scratch around how top human
Battlegrounds players actually crush naive 1-turn bots:
  1. Disciplined Macro Leveling Curve (T2->2, T5->3, T7->4, T8/9->5, T10->6 + Ghost free-levels)
  2. Patch 37.0.2 `DarkDiscovery` Discipline: holds all 3 charges for Turn 9-12 targeted
     Tier 5/6 `FirstTwo` (most-common-tribe) discovers + Late Dark Gifts (while SovereignBot
     wastes all 3 charges on Turn 5-7 Tier 2/3 trash!)
  3. Dynamic 10-Engine Build Director + Strict Late-Game Buy Filter + Aggressive Rolling:
     stops buying random vanilla minions on Turn 7+ and rolls 5-8x/turn for true exponential
     engines (`Gatekeeper Amalgam + Balinda + Felfire/Harbinger + Drakkari`, `Beatboxer +
     Drone Duplicator + Utility Drone`, `Flaming Enforcer + Drakkari`, `Persistent Poet +
     Broodmother/Vindicator`, `Victorious Geomant + Surveyor + Roogug`, `Snazzy Phantom +
     Butcher + Banshee`, `Goldrinn + Titus + Slamma`, `Brann + Murk-Eye + Choral`,
     `The Last One Standing`, etc.)
  4. Exact Mechanical Engine Sequencing (Activate `Drone Duplicator` before magnetizing onto it,
     target `Gatekeeper Amalgam` with EVERY targeted spell/Blood Gem, `Snazzy Phantom` right-most
     prep before `Dead Bellringer`/`Butchering`, `Cataclysmic Harbinger` payoff spell LAST,
     `Living Prison` activate before buy, Golden `Greedy Conniver` -> Sell for T7, Triple Holding!)
  5. Comp-Invariant + Exact Rust `equity_against` Combat Positioning & Ruthless Finals Scam-Hunt.
"""

from __future__ import annotations

import random
from dataclasses import dataclass
from typing import Dict, FrozenSet, List, Optional, Sequence, Set, Tuple

import seaglass

from .comps import CompRegistry, get_comp_registry


# =============================================================================
# 1. Patch 37.0.2 Ground-Truth Knowledge Tables
# =============================================================================

# Proven Patch 37.0.2 Hero Power & Economy Rankings (Empirical-Bayes calibrated + human tier list)
GM_HERO_SCORES: Dict[str, float] = {
    "Trade Prince Gallywix": 28.0,
    "Scabbs Cutterbutter": 26.0,
    "Galewing": 25.5,
    "Tess Greymane": 24.5,
    "Enhance-o Mechano": 24.0,
    "Tae'thelan Bloodwatcher": 23.5,
    "C'Thun": 22.5,
    "Dancin' Deryl": 22.0,
    "Death Speaker Blackthorn": 22.0,
    "King Mukla": 21.5,
    "Rokara": 21.5,
    "Nozdormu": 21.5,
    "Patchwerk": 20.5,
    "Ysera": 20.5,
    "Mutanus the Devourer": 20.0,
    "Shudderwock": 20.0,
    "Queen Wagtoggle": 19.5,
    "Farseer Nobundo": 19.5,
    "Doctor Holli'dae": 19.5,
    "George the Fallen": 19.0,
    "Ragnaros the Firelord": 19.0,
    "Zerek, Master Cloner": 19.0,
    "Master Nguyen": 19.0,
    "E.T.C., Band Manager": 18.5,
    "Sire Denathrius": 18.5,
    "Reno Jackson": 18.5,
    "Arch-Villain Rafaam": 18.5,
    "Infinite Toki": 18.0,
    "Cookie the Cook": 18.0,
    "The Lich King": 18.0,
    "N'Zoth": 18.0,
    "Ozumat": 17.5,
    "Sneed": 17.5,
    "Dinotamer Brann": 17.5,
    "Millhouse Manastorm": 17.0,
    "Yogg-Saron, Hope's End": 17.0,
    "Heistbaron Togwaggle": 17.0,
    "A. F. Kay": 16.5,
    "Galakrond": 16.5,
    "Malygos": 16.5,
    "Edwin VanCleef": 16.0,
    "Y'Shaarj": 16.0,
    "Rakanishu": 15.5,
    "Al'Akir": 15.5,
    "Illidan Stormrage": 15.5,
    "Kael'thas Sunstrider": 15.5,
    # Weak / Trap heroes to avoid
    "The Rat King": 11.0,
    "Tavish Stormpike": 10.5,
    "Thorim, Stormlord": 11.5,
    "Vol'jin": 10.5,
    "Cap'n Hoggarr": 10.5,
    "Chenvaala": 10.0,
    "Forest Warden Omu": 11.0,
    "Varden Dawngrasp": 9.5,
    "Greybough": 9.0,
    "Rock Master Voone": 9.0,
    "Deathwing": 8.0,
    "Tras'tath, Soul Parasite": 7.5,
    "Forest Lord Cenarius": 7.0,
    "Mister Clocksworth": 6.5,
    "Sir Finley Mrrgglton": 7.0,
    "Zephrys, the Great": 8.0,
    "Kurtrus Ashfallen": 8.5,
    "Skycap'n Kragg": 8.5,
}

# Proven Patch 37.0.2 Trinket Tier List (1st-place win-condition focus!)
GM_TRINKET_SCORES: Dict[str, float] = {
    # S+ Win-Condition Trinkets (30-45% 1st-place rate in Patch 37.0.2!)
    "Yogg-Tastic Pastry": 48.0,
    "Inductive Gyroblade": 46.0,
    "Wax Lance": 44.0,
    "Tableware Portrait": 42.0,
    "Ornate Clock": 40.0,
    "Flaming Portrait": 39.0,
    "Beatboxer Portrait": 39.0,
    "Sky Golem Portrait": 38.0,
    "Drakkari Portrait": 37.0,
    "Bronzebeard Portrait": 37.0,
    "Titus Portrait": 36.0,
    "Nomi Sticker": 36.0,
    "Morgl Portrait": 36.0,
    "Sinister Invitation": 35.0,
    "Azeroth Model Globe": 34.0,
    "Curator Sticker": 32.0,
    "Pagle's Fishing Rod": 31.0,
    "Souvenir Stand": 31.0,
    "Azerite Portrait": 31.0,
    "Volumizer Portrait": 30.0,  # Huge if we actually activate Drone Duplicator + Beatboxer!
    "Great Boar Sticker": 30.0,
    "Hoggy Bank": 29.0,
    "Quilligraphy Set": 29.0,
    "Miniature Ship": 28.0,
    "Dragonwing Glider": 28.0,
    "Boom's Monster Portrait": 28.0,
    "Training Certificate": 27.0,
    "Bronze Timepiece": 27.0,
    "Holy Mallet": 27.0,
    "Maldraxxus Dagger": 26.0,
    "Trusty Crowbar": 26.0,
    "Booty Bay Brew": 24.0,
    "Trip Vouchers": 24.0,
    "Guiding Candle": 23.0,
    "Tickatus Sticker": 23.0,
    "Mindslasher Portrait": 25.0,
    "Rockin' Music Box": 22.0,
    "Defensive Shell": 22.0,
    "Book of Medivh": 22.0,
    "Accord-o-Tron Portrait": 25.0,
    "Murky Sticker": 26.0,
    "Young Murk-Eye Sticker": 26.0,
    "Wolfhead Flail": 25.0,
    "Nerglish Phrasebook": 25.0,
    "Dramaloc Sticker": 25.0,
    "Bluegill Flippers": 24.0,
    # Confirmed 0%-1st Traps (AVOID!)
    "Innkeeper's Hearth": 8.0,
    "Reinvigorating Light": 10.0,
    "Explorer's Binoculars": 8.0,
    "Magician's Top Hat": 7.0,
    "Gold Mallet": 5.0,
    "Sinstone Sticker": 5.0,
    "Rendle Sticker": 4.0,
    "Ghastly Sticker": 4.0,
    "Colorful Compass": 3.0,
    "Manipulator Portrait": 2.0,
    "Safety Patch": 2.0,
    "Bob's Tip Jar": 2.0,
    "Hammer of Twilight": 2.0,
    "Bartend-o-Tron's Oilcan": 2.0,
    "Gold-plated Compass": 2.0,
    "Goblin Wallet": 2.0,
    "Portable Factory": 4.0,
    "Transcribing Typewriter": 2.0,
    "Worn Treasure Map": 2.0,
    "Shrine of Evolution": -5.0,
    "Warband Whistle": 3.0,
}

# Tier-1 Opening Priority (Economy / Token / Scaling > Pure Stats)
T1_OPENING_SCORE: Dict[str, float] = {
    "Southsea Busker": 24.0,      # +1g next turn!
    "Ominous Seer": 22.0,         # Spell discount!
    "Razorfen Geomancer": 21.0,   # Blood Gem economy
    "Cord Puller": 20.5,          # Divine Shield + Magnetic token
    "Aureate Laureate": 20.0,     # Divine Shield + sells for 2g+
    "Bubble Gunner": 19.0,        # Keywords + Murloc
    "Dune Dweller": 18.5,         # 3/3 + Tavern buff
    "Scarlet Survivor": 18.5,     # 3/3 + Spell scaling
    "Flighty Scout": 18.0,        # 3/3 + handbuff
    "Suspicious Prisonguard": 18.0,
    "Harmless Bonehead": 17.5,    # 2 bodies / Undead
    "Buzzing Vermin": 17.0,       # 2 bodies / Beast
    "Risen Rider": 17.0,          # Taunt + Reborn
    "Crackling Cyclone": 17.0,    # Divine Shield + Windfury
    "Lullabot": 16.5,             # Magnetic + EOT
    "Tusked Camper": 16.5,        # 2/3 Rally Blood Gem
    "Joyous": 16.5,
    "Zoatroid": 16.0,
    "Glim Guardian": 15.5,
    "Flittering Bat": 15.0,
    "Wrath Weaver": 14.5,         # Only good with Soul Rewinder/Demons
}

# Universal S+ God Cards (Always high priority when seen on Tier 5/6/7!)
UNIVERSAL_GOD_CARDS: Dict[str, float] = {
    "The Last One Standing": 100.0,
    "Gatekeeper Amalgam": 92.0,
    "Balinda Stonehearth": 88.0,
    "Polarizing Beatboxer": 92.0,
    "Tide Oracle Morgl": 86.0,
    "Captain Sanders": 84.0,
    "Champion of Sargeras": 84.0,
    "Stone Age Slab": 85.0,
    "Sha of Fear": 82.0,
    "Stalwart Kodo": 82.0,
    "Obsidian Ravager": 82.0,
    "Futurefin": 80.0,
    "Highkeeper Ra": 80.0,
    "Stitched Salvager": 78.0,
    "Jailbird Juggernaut": 78.0,
}

# Economy / Gold-Positive / Cheap Cycle Cards
ECONOMY_MINIONS: FrozenSet[str] = frozenset({
    "Southsea Busker",
    "Tad",
    "Primalfin Lookout",
    "Rodeo Performer",
    "Tavern Tempest",
    "Refreshing Anomaly",
    "Doubloon Grifter",
    "Auto Accelerator",
    "Gem Rat",
    "Treasure Parrot",
    "Azsharan Cutlassier",
    "Firescale Hoarder",
    "Mindbending Recruiter",
    "Silent Deliverer",
    "Elite Navigator",
    "Greedy Conniver",
    "Shipwrecked Rascal",
    "Bigwig Bandit",
    "Captain Sanders",
    "Imposing Percussionist",
    "Ominous Seer",
    "Razorfen Geomancer",
    "Maw Caster",
    "Disguised Graverobber",
})

# High-value Payoff Tavern Spells (Cast LAST with Cataclysmic Harbinger!)
PAYOFF_SPELLS: Dict[str, float] = {
    "Staff of Enrichment": 92.0,
    "Azerite Empowerment": 90.0,
    "Misplaced Tea Set": 88.0,
    "Natural Blessing": 86.0,
    "Eyes of the Earth Mother": 94.0,
    "Perfect Vision": 82.0,
    "Upper Hand": 80.0,
    "Butchering": 78.0,
    "Channel the Devourer": 76.0,
    "Chef's Choice": 72.0,
    "Bountiful Treasures": 74.0,
    "Cloning Conch": 72.0,
    "Hasty Excavation": 70.0,
    "Strike Oil": 78.0,
    "Careful Investment": 72.0,
    "Tavern Coin": 70.0,
    "Fluidity": 60.0,
    "Overconfidence": 68.0,
    "Tavern Dish Banana": 62.0,
    "Pointy Arrow": 55.0,
    "Fortify": 52.0,
    "Selfish Bounty": 64.0,
    "Bounty": 60.0,
    "Winner's Bread": 60.0,
    "Them Apples": 54.0,
    "Leaf Through the Pages": 66.0,
    "Land Lubber": 55.0,
    "A New Sprout": 55.0,
    "Blood Gem": 50.0,
}

# Targeted buff spells that trigger `Gatekeeper Amalgam`'s free `Misplaced Tea Set`!
AMALGAM_TRIGGER_SPELLS: FrozenSet[str] = frozenset({
    "Blood Gem",
    "Tavern Dish Banana",
    "Fortify",
    "Pointy Arrow",
    "Natural Blessing",
    "Perfect Vision",
    "Winner's Bread",
    "A New Sprout",
    "Selfish Bounty",
    "Enchanted Lasso",
    "Shiny Ring",
    "Glowing Crown",
    "Armor Stash",
    "Vitality Shift",
    "Reckless Investment",
    "Staff of Enrichment",
})

# Spells that should NEVER be cast blindly onto our own carry
DANGEROUS_SELF_SPELLS: FrozenSet[str] = frozenset({
    "Robust Evolution",
    "Channel the Devourer",
    "Mutate",
    "Butchering",
    "Fluidity",
    "Tomb Turning",
})

# Finals Scam / Counter-Tech Units
SCAM_NAMES: FrozenSet[str] = frozenset({
    "Leeroy the Reckless",
    "Deadly Spore",
    "Bile Spitter",
    "Sin'dorei Straight Shot",
    "Tide Oracle Morgl",
    "Boom-in-a-Box",
})

# Dead Tavern-Only Scalers to sell in Top-2 Finals for Scam/Tech
FINALS_DEAD_SCALERS: FrozenSet[str] = frozenset({
    "Brann Bronzebeard",
    "Drakkari Enchanter",
    "Soul Rewinder",
    "Ashen Corruptor",
    "Kalecgos, Arcane Aspect",
    "Sky Admiral Rogers",
    "Proud Privateer",
    "Living Azerite",
    "Felfire Conjurer",
    "Cataclysmic Harbinger",
    "Humon'gozz",
    "Gem Rat",
    "Auto Accelerator",
    "Drone Duplicator",
    "Mindbending Recruiter",
    "Brain Rotter",
    "Abyssal Envoy",
    "Treasure Parrot",
    "Kelp Keeper",
    "Balinda Stonehearth",
    "Hot-Air Surveyor",
    "Victorious Geomant",
    "Rodeo Performer",
})

# Spent one-shot Battlecries (no board value once played unless Murk-Eye/Kelp Keeper present)
SPENT_BATTLECRIES: FrozenSet[str] = frozenset({
    "Captain Sanders",
    "Elite Navigator",
    "Silent Deliverer",
    "Primalfin Lookout",
    "Rodeo Performer",
    "Imposing Percussionist",
    "Tavern Tempest",
    "Maw Caster",
    "Disguised Graverobber",
    "Fetid Corroder",
    "Joyous",
    "Razorfen Geomancer",
    "Nerubian Deathswarmer",
    "Azsharan Cutlassier",
    "Refreshing Anomaly",
    "En-Djinn Blazer",
    "Firelands Fugitive",
    "Lovesick Balladist",
    "Ominous Seer",
    "Mind Muck",
    "Bubble Gunner",
    "Southsea Busker",
    "Tad",
})

# Goldenizing cards (play target = highest-value non-golden engine!)
GOLDENIZER_NAMES: FrozenSet[str] = frozenset({
    "Captain Sanders",
    "Elite Navigator",
    "Eyes of the Earth Mother",
})

# =============================================================================
# 2. The 10 Exponential Patch 37.0.2 Build Engines
# =============================================================================

@dataclass(frozen=True)
class EngineSpec:
    key: str
    primary_tribe: Optional[str]          # Tribe to align for Turn 9-12 DarkDiscovery!
    required_tribes: FrozenSet[str]       # At least one must be in lobby (empty = always available!)
    lynchpins: Dict[str, float]           # Core build-defining cards (trigger commitment!)
    multipliers: Dict[str, float]         # Force multipliers (2x-4x scaling!)
    scalers: Dict[str, float]             # Supporting scalers & carries
    fuel_tribes: FrozenSet[str]           # Tribes whose minions fuel the engine
    wants_spells: bool = False
    wants_magnetics: bool = False
    wants_gems: bool = False
    wants_battlecries: bool = False
    wants_reborn: bool = False


ENGINES: Tuple[EngineSpec, ...] = (
    EngineSpec(
        key="spells_amalgam",
        primary_tribe=None,
        required_tribes=frozenset(),
        lynchpins={
            "Gatekeeper Amalgam": 28.0,
            "Balinda Stonehearth": 26.0,
            "Felfire Conjurer": 24.0,
            "Cataclysmic Harbinger": 24.0,
            "The Last One Standing": 30.0,
            "Nightmare Par-tea Guest": 18.0,
        },
        multipliers={
            "Drakkari Enchanter": 24.0,
            "Humon'gozz": 16.0,
            "Brann Bronzebeard": 16.0,
        },
        scalers={
            "Felboar": 18.0,
            "Runic Arcanist": 18.0,
            "Flaming Enforcer": 20.0,
            "Living Prison": 16.0,
            "Timecap'n Hooktail": 15.0,
            "Azsharan Cutlassier": 15.0,
            "Blue Whelp": 14.0,
            "Scarlet Survivor": 14.0,
            "Firescale Hoarder": 14.0,
            "Rodeo Performer": 15.0,
            "Dark Paradox": 14.0,
            "Enchanted Sentinel": 14.0,
            "Vicious Mindslasher": 14.0,
            "Gearfin": 14.0,
        },
        fuel_tribes=frozenset({"all"}),
        wants_spells=True,
    ),
    EngineSpec(
        key="mech_magnetic",
        primary_tribe="mech",
        required_tribes=frozenset({"mech"}),
        lynchpins={
            "Polarizing Beatboxer": 32.0,
            "Drone Duplicator": 26.0,
            "Utility Drone": 26.0,
            "Resourceful Robot": 25.0,
            "Charging Czarina": 20.0,
            "Falling Sky Golem": 20.0,
        },
        multipliers={
            "Drakkari Enchanter": 26.0,
            "Titus Rivendare": 16.0,
            "Balinda Stonehearth": 16.0,
        },
        scalers={
            "Glambot": 18.0,
            "Spark Snapper": 18.0,
            "Auto Reveille": 18.0,
            "Auto Accelerator": 17.0,
            "Mechagnome Interpreter": 18.0,
            "Gearfin": 16.0,
            "Cord Puller": 15.0,
            "Accord-o-Tron": 16.0,
            "Green Volumizer": 15.0,
            "Red Volumizer": 15.0,
            "Blue Volumizer": 15.0,
            "Conveyor Construct": 14.0,
            "Relentless Deflector": 14.0,
            "Lullabot": 12.0,
            "Dr. Boom's Monster": 22.0,
        },
        fuel_tribes=frozenset({"mech", "all"}),
        wants_magnetics=True,
        wants_spells=True,
    ),
    EngineSpec(
        key="shop_eat_elem_demon",
        primary_tribe="demon",
        required_tribes=frozenset({"demon", "elemental"}),
        lynchpins={
            "Flaming Enforcer": 28.0,
            "Champion of Sargeras": 30.0,
            "Living Azerite": 24.0,
            "En-Djinn Blazer": 22.0,
            "Unbound Tempest": 22.0,
            "Ultraviolet Ascendant": 22.0,
            "Eredar Escapist": 22.0,
            "Stone Age Slab": 28.0,
            "Arcane Behemoth": 26.0,
        },
        multipliers={
            "Drakkari Enchanter": 25.0,
            "Brann Bronzebeard": 22.0,
            "Soul Rewinder": 18.0,
        },
        scalers={
            "Deft Deserter": 19.0,
            "Devilish Distractor": 18.0,
            "Insatiable Ur'zul": 19.0,
            "Tichondrius": 18.0,
            "Sacrificial Wrathguard": 17.0,
            "Soulkeeping Jailer": 17.0,
            "Twisted Wrathguard": 17.0,
            "Felboar": 18.0,
            "Malchezaar, Prince of Dance": 16.0,
            "Elemental of Surprise": 20.0,
            "Air Revenant": 16.0,
            "Firelands Fugitive": 15.0,
            "Tavern Tempest": 15.0,
            "Refreshing Anomaly": 14.0,
            "Crackling Cyclone": 14.0,
            "Wrath Weaver": 12.0,
            "Devout Hellcaller": 15.0,
            "Ashen Corruptor": 15.0,
        },
        fuel_tribes=frozenset({"demon", "elemental", "all"}),
        wants_spells=True,
    ),
    EngineSpec(
        key="dragon_poet",
        primary_tribe="dragon",
        required_tribes=frozenset({"dragon"}),
        lynchpins={
            "Persistent Poet": 28.0,
            "Heroic Broodmother": 28.0,
            "Crimson Vindicator": 26.0,
            "Hopebringer": 24.0,
            "Kalecgos, Arcane Aspect": 24.0,
            "Obsidian Ravager": 28.0,
        },
        multipliers={
            "Brann Bronzebeard": 22.0,
            "Drakkari Enchanter": 18.0,
            "Nadina the Red": 22.0,
            "Humon'gozz": 16.0,
        },
        scalers={
            "Runic Arcanist": 22.0,
            "Tarecgosa": 20.0,
            "Amber Guardian": 17.0,
            "Felfire Conjurer": 20.0,
            "Sky-hatch Runaway": 17.0,
            "Gatekeeper Amalgam": 24.0,
            "Draconic Warden": 15.0,
            "Blue Whelp": 15.0,
            "Bronze Warden": 14.0,
            "Firescale Hoarder": 14.0,
            "Scarlet Survivor": 14.0,
        },
        fuel_tribes=frozenset({"dragon", "all"}),
        wants_battlecries=True,
        wants_spells=True,
    ),
    EngineSpec(
        key="quilboar_gems",
        primary_tribe="quilboar",
        required_tribes=frozenset({"quilboar"}),
        lynchpins={
            "Victorious Geomant": 30.0,
            "Hot-Air Surveyor": 26.0,
            "Sanguine Champion": 26.0,
            "Turbo Hogrider": 25.0,
            "Jailbird Juggernaut": 28.0,
            "Gatekeeper Amalgam": 28.0,
        },
        multipliers={
            "Balinda Stonehearth": 26.0,
            "Brann Bronzebeard": 22.0,
            "Drakkari Enchanter": 22.0,
            "Young Murk-Eye": 24.0,
            "Titus Rivendare": 18.0,
        },
        scalers={
            "Geomagus Roogug": 22.0,
            "Sanguine Refiner": 20.0,
            "Thorned Trailblazer": 19.0,
            "Veteran Brigand": 19.0,
            "Gem Rat": 17.0,
            "Fearless Foodie": 16.0,
            "Bonker": 17.0,
            "Felboar": 18.0,
            "Razorfen Vineweaver": 16.0,
            "Bramble Tunneler": 15.0,
            "Snare Trapper": 15.0,
            "Sly Infiltrator": 15.0,
            "Roadboar": 13.0,
        },
        fuel_tribes=frozenset({"quilboar", "all"}),
        wants_gems=True,
        wants_spells=True,
    ),
    EngineSpec(
        key="undead_reborn",
        primary_tribe="undead",
        required_tribes=frozenset({"undead"}),
        lynchpins={
            "Snazzy Phantom": 30.0,
            "Drustfallen Butcher": 26.0,
            "Barrier Banshee": 25.0,
            "Eternal Summoner": 24.0,
            "Stitched Salvager": 28.0,
        },
        multipliers={
            "Titus Rivendare": 24.0,
            "Drakkari Enchanter": 18.0,
            "Brann Bronzebeard": 16.0,
        },
        scalers={
            "Dead Bellringer": 22.0,
            "Eternal Tycoon": 20.0,
            "Plaguerunner": 18.0,
            "Deathly Striker": 19.0,
            "Forsaken Weaver": 19.0,
            "Lichling Hoarder": 17.0,
            "Mummifier": 16.0,
            "Prosthetic Hand": 16.0,
            "Eternal Knight": 16.0,
            "Handless Forsaken": 14.0,
            "Cadaver Caretaker": 14.0,
            "Maw Caster": 14.0,
            "Disguised Graverobber": 14.0,
        },
        fuel_tribes=frozenset({"undead", "all"}),
        wants_reborn=True,
    ),
    EngineSpec(
        key="murloc_bc_hand",
        primary_tribe="murloc",
        required_tribes=frozenset({"murloc"}),
        lynchpins={
            "Young Murk-Eye": 30.0,
            "Choral Mrrrglr": 28.0,
            "Futurefin": 28.0,
            "Hackerfin": 24.0,
            "Tide Oracle Morgl": 28.0,
            "Magicfin Mycologist": 22.0,
        },
        multipliers={
            "Brann Bronzebeard": 28.0,
            "Drakkari Enchanter": 24.0,
            "Balinda Stonehearth": 18.0,
            "Kelp Keeper": 16.0,
        },
        scalers={
            "Bream Counter": 20.0,
            "Primalfin Lookout": 19.0,
            "Twilight Tidehunter": 18.0,
            "Shamanic Tidecaller": 18.0,
            "Bile Spitter": 20.0,
            "Bassgill": 17.0,
            "Diremuck Forager": 16.0,
            "Gormling Gourmet": 15.0,
            "Costume Enthusiast": 16.0,
            "Sewer Escapee": 15.0,
            "Tad": 13.0,
        },
        fuel_tribes=frozenset({"murloc", "all"}),
        wants_battlecries=True,
    ),
    EngineSpec(
        key="beast_dr_rally",
        primary_tribe="beast",
        required_tribes=frozenset({"beast"}),
        lynchpins={
            "Goldrinn, the Great Wolf": 30.0,
            "Banana Slamma": 28.0,
            "Deathstrider": 28.0,
            "Turquoise Skitterer": 24.0,
            "Stalwart Kodo": 28.0,
        },
        multipliers={
            "Titus Rivendare": 28.0,
            "Lurking Leviathan": 20.0,
        },
        scalers={
            "Ravaging Scorpid": 20.0,
            "Sewer Lord": 19.0,
            "Ghastcoiler": 18.0,
            "Forest Rover": 16.0,
            "Sprightly Scarab": 16.0,
            "Headhunter Gryphon": 15.0,
            "Hoarding Hyena": 15.0,
            "Sky-hatch Runaway": 15.0,
            "Lurking Lionfish": 15.0,
        },
        fuel_tribes=frozenset({"beast", "all"}),
    ),
    EngineSpec(
        key="pirate_apm_golden",
        primary_tribe="pirate",
        required_tribes=frozenset({"pirate"}),
        lynchpins={
            "Sky Admiral Rogers": 28.0,
            "Proud Privateer": 26.0,
            "Hooktusk, Master Marauder": 24.0,
            "Silent Deliverer": 24.0,
            "Captain Sanders": 28.0,
            "Ship Master Eudora": 22.0,
        },
        multipliers={
            "Brann Bronzebeard": 22.0,
            "Titus Rivendare": 18.0,
            "Drakkari Enchanter": 18.0,
        },
        scalers={
            "Elite Navigator": 20.0,
            "Greedy Conniver": 20.0,
            "Enterprising Escapee": 19.0,
            "Doubloon Grifter": 18.0,
            "Bigwig Bandit": 17.0,
            "Shipwrecked Rascal": 17.0,
            "Loyal Mobster": 17.0,
            "Timecap'n Hooktail": 16.0,
            "Azsharan Cutlassier": 15.0,
            "Gunpowder Courier": 15.0,
            "Blade Collector": 16.0,
            "Treasure Parrot": 14.0,
        },
        fuel_tribes=frozenset({"pirate", "all"}),
        wants_spells=True,
    ),
    EngineSpec(
        key="aberration_deity",
        primary_tribe="aberration",
        required_tribes=frozenset({"aberration"}),
        lynchpins={
            "Faceless Converter": 26.0,
            "Mindbender Ghur'sha": 26.0,
            "Dark Puppeteer": 24.0,
            "The Shadow of Doubt": 24.0,
            "Sha of Fear": 28.0,
            "Vicious Mindslasher": 22.0,
        },
        multipliers={
            "Titus Rivendare": 22.0,
            "Drakkari Enchanter": 20.0,
            "Brann Bronzebeard": 18.0,
        },
        scalers={
            "Harbinger Aph'lass": 19.0,
            "N'raqi Frostcaller": 18.0,
            "De-volition-ist": 18.0,
            "Mindbending Recruiter": 17.0,
            "Cutthroat K'Thir": 17.0,
            "Mysterious K'Thir": 16.0,
            "N'raqi Sapper": 16.0,
            "Gearfin": 16.0,
            "Brain Rotter": 15.0,
            "Nightmare Corroder": 16.0,
        },
        fuel_tribes=frozenset({"aberration", "all"}),
        wants_spells=True,
    ),
)

ENGINE_BY_KEY: Dict[str, EngineSpec] = {e.key: e for e in ENGINES}


# =============================================================================
# 3. GrandmasterBot Controller
# =============================================================================

@dataclass
class ActiveBuild:
    engine: EngineSpec
    score: float
    committed: bool
    has_amalgam: bool
    has_balinda: bool
    has_duplicator: bool
    has_beatboxer: bool
    has_snazzy: bool
    has_harbinger: bool
    has_poet: bool
    has_geomant: bool
    has_surveyor: bool
    has_last_one: bool
    has_murloc_handbuff: bool
    is_finals: bool
    scam_hunt: bool
    enemy_max_hp: int


class GrandmasterBot:
    """
    Pure First-Principles Patch 37.0.2 Battlegrounds Bot (zero SovereignBot inheritance).
    Fast (~20-60ms/turn), disciplined macro curve, Turn 9-12 DarkDiscovery targeting,
    strict late-game buy filter + deep rolling, exact engine sequencing, and Rust
    Monte Carlo combat positioning.
    """

    def __init__(
        self,
        catalog_name: str = "full",
        seed: int = 2029,
        **kwargs,
    ) -> None:
        self.catalog_name = catalog_name
        self.rng = random.Random(seed)
        self.reg: CompRegistry = get_comp_registry(catalog_name)
        self.last_note: Optional[Dict[str, float]] = None

        # Preload lightweight catalog lookup maps (hero names, trinket names, card tags)
        self._hero_names: Dict[int, str] = {}
        try:
            for h in seaglass.heroes():
                self._hero_names[int(h.id)] = str(h.name)
        except Exception:
            pass
        self._trinket_names: Dict[int, str] = {}
        try:
            for t in seaglass.trinkets():
                self._trinket_names[int(t.id)] = str(t.name)
        except Exception:
            pass

        tag_names, tag_table = seaglass.card_tags()
        t_idx = {n: i for i, n in enumerate(tag_names)}
        self._bc_ids: FrozenSet[int] = self._tag_set(tag_table, t_idx.get("battlecry"))
        self._dr_ids: FrozenSet[int] = self._tag_set(tag_table, t_idx.get("deathrattle"))
        self._eot_ids: FrozenSet[int] = self._tag_set(tag_table, t_idx.get("end_of_turn"))
        self._cleave_ids: FrozenSet[int] = self._tag_set(tag_table, t_idx.get("cleave"))
        self._rally_ids: FrozenSet[int] = self._tag_set(tag_table, t_idx.get("rally"))
        self._avenge_ids: FrozenSet[int] = self._tag_set(tag_table, t_idx.get("avenge"))

        # Per-player turn state
        self._positioned_turn: Dict[int, Tuple[int, int]] = {}
        self._refreshes_this_turn: Dict[Tuple[int, int], int] = {}
        self._last_build: Optional[ActiveBuild] = None
        self._last_plan = None  # Compatibility with bench_1v7 logging

    @staticmethod
    def _tag_set(tag_table: Dict[int, Sequence[float]], col: Optional[int]) -> FrozenSet[int]:
        if col is None:
            return frozenset()
        return frozenset(cid for cid, row in tag_table.items() if row[col] > 0.5)

    # -------------------------------------------------------------------------
    # Build Director: Identify our active exponential scaling engine
    # -------------------------------------------------------------------------
    def _analyze_build(
        self,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext],
    ) -> ActiveBuild:
        lobby_tribes = set(ctx.tribes) if ctx is not None and ctx.tribes else set()
        board = obs.board
        hand = obs.hand
        trinket_names = [self._trinket_names.get(int(t), "") for t in obs.trinkets]
        board_names = {u.name for u in board}
        hand_names = {c.name for c in hand if not c.is_spell}
        all_names = board_names | hand_names

        best_eng = ENGINES[0]
        best_sc = -1e9
        for eng in ENGINES:
            if lobby_tribes and eng.required_tribes and not (eng.required_tribes & lobby_tribes):
                continue
            sc = 0.0
            for u in board:
                mult = 1.45 if u.is_golden else 1.0
                if u.name in eng.lynchpins:
                    sc += eng.lynchpins[u.name] * mult
                elif u.name in eng.multipliers:
                    sc += eng.multipliers[u.name] * 0.70 * mult
                elif u.name in eng.scalers:
                    sc += eng.scalers[u.name] * 0.45 * mult
                elif eng.primary_tribe and eng.primary_tribe in u.tribes:
                    sc += 3.5
            for c in hand:
                if not c.is_spell:
                    if c.name in eng.lynchpins:
                        sc += eng.lynchpins[c.name] * 0.75
                    elif c.name in eng.multipliers:
                        sc += eng.multipliers[c.name] * 0.45
            # Trinket synergy boost
            for tn in trinket_names:
                if eng.key == "mech_magnetic" and tn in (
                    "Volumizer Portrait", "Beatboxer Portrait", "Sky Golem Portrait",
                    "Boom's Monster Portrait", "Accord-o-Tron Portrait", "Inductive Gyroblade",
                ):
                    sc += 26.0
                elif eng.key == "spells_amalgam" and tn in (
                    "Tableware Portrait", "Yogg-Tastic Pastry", "Ornate Clock", "Curator Sticker",
                ):
                    sc += 18.0
                elif eng.key == "shop_eat_elem_demon" and tn in (
                    "Flaming Portrait", "Nomi Sticker", "Azerite Portrait", "Rewinder Portrait",
                ):
                    sc += 24.0
                elif eng.key == "quilboar_gems" and tn in ("Great Boar Sticker", "Quilligraphy Set", "Hoggy Bank"):
                    sc += 24.0
                elif eng.key == "murloc_bc_hand" and tn in (
                    "Morgl Portrait", "Murky Sticker", "Young Murk-Eye Sticker",
                    "Nerglish Phrasebook", "Dramaloc Sticker", "Bluegill Flippers",
                ):
                    sc += 24.0
                elif eng.key == "dragon_poet" and tn in ("Dragonwing Glider", "Training Certificate"):
                    sc += 20.0
                elif eng.key == "beast_dr_rally" and tn in ("Titus Portrait", "Wolfhead Flail"):
                    sc += 20.0
                elif eng.key == "aberration_deity" and tn in ("Mindslasher Portrait",):
                    sc += 22.0
            if sc > best_sc:
                best_sc = sc
                best_eng = eng

        alive_units = ctx.alive_units if ctx is not None else 8
        next_is_ghost = bool(ctx.next_is_ghost) if ctx is not None else False
        is_finals = alive_units <= 2 and not next_is_ghost

        enemy_max_hp = 0
        enemy_total_stats = 0
        if ctx is not None and ctx.opponents:
            for opp in ctx.opponents:
                if opp.is_next and opp.seen_board:
                    enemy_max_hp = max((u.health for u in opp.seen_board), default=0)
                    enemy_total_stats = sum(u.attack + u.health for u in opp.seen_board)
                    break
            if enemy_max_hp == 0:
                for opp in ctx.opponents:
                    if opp.alive and opp.seen_board:
                        enemy_max_hp = max(enemy_max_hp, max((u.health for u in opp.seen_board), default=0))

        my_stats = sum(u.attack + u.health for u in board)
        # Enter Scam-Hunt Mode in Top-3 / Finals if an opponent is huge and we're behind on raw stats!
        scam_hunt = (
            (is_finals or (alive_units <= 3 and obs.turn >= 12))
            and enemy_max_hp >= 220
            and (enemy_total_stats > my_stats * 1.15 or enemy_max_hp >= 500)
        )

        committed = best_sc >= 24.0 or (obs.turn >= 9 and best_sc >= 16.0)
        has_murloc_handbuff = any(
            n in all_names for n in ("Choral Mrrrglr", "Futurefin", "Bream Counter", "Gormling Gourmet", "Diremuck Forager")
        )

        build = ActiveBuild(
            engine=best_eng,
            score=best_sc,
            committed=committed,
            has_amalgam="Gatekeeper Amalgam" in board_names,
            has_balinda="Balinda Stonehearth" in board_names,
            has_duplicator="Drone Duplicator" in board_names,
            has_beatboxer="Polarizing Beatboxer" in board_names,
            has_snazzy="Snazzy Phantom" in board_names,
            has_harbinger="Cataclysmic Harbinger" in board_names,
            has_poet="Persistent Poet" in board_names,
            has_geomant="Victorious Geomant" in board_names,
            has_surveyor="Hot-Air Surveyor" in board_names,
            has_last_one="The Last One Standing" in board_names,
            has_murloc_handbuff=has_murloc_handbuff,
            is_finals=is_finals,
            scam_hunt=scam_hunt,
            enemy_max_hp=enemy_max_hp,
        )
        self._last_build = build
        # Tiny object for bench_1v7 logging compatibility
        self._last_plan = type("_PlanStub", (), {"primary_id": best_eng.key})()
        return build

    # -------------------------------------------------------------------------
    # Hero, Trinket, and Discover Selectors
    # -------------------------------------------------------------------------
    def _choose_hero(
        self,
        obs: seaglass.TavernObservation,
        legal: Sequence[seaglass.TavernAction],
        ctx: Optional[seaglass.LobbyContext],
    ) -> seaglass.TavernAction:
        lobby_tribes = set(ctx.tribes) if ctx is not None and ctx.tribes else set()
        best_act = legal[0]
        best_sc = -1e9
        for act in legal:
            if act.kind != "choose_hero":
                continue
            idx = act.args[0]
            if idx is None or idx >= len(obs.hero_options):
                continue
            hid = int(obs.hero_options[idx])
            hname = self._hero_names.get(hid, "")
            sc = GM_HERO_SCORES.get(hname, 14.5)
            # Lobby tribe conditioning
            if hname in ("Trade Prince Gallywix", "Millhouse Manastorm", "Edwin VanCleef"):
                sc += 1.6 * len(lobby_tribes & {"elemental", "pirate", "murloc", "quilboar"})
            elif hname in ("Shudderwock", "Dinotamer Brann"):
                sc += 1.8 * len(lobby_tribes & {"murloc", "dragon", "elemental", "pirate"})
            elif hname in ("The Lich King", "N'Zoth", "Teron Gorefiend"):
                sc += 1.8 * len(lobby_tribes & {"undead", "beast", "mech"})
            elif hname == "Ysera" and "dragon" in lobby_tribes:
                sc += 3.5
            elif hname == "Death Speaker Blackthorn" and "quilboar" in lobby_tribes:
                sc += 3.0
            if sc > best_sc:
                best_sc = sc
                best_act = act
        return best_act

    def _choose_trinket(
        self,
        lobby: seaglass.LobbyGame,
        player: int,
        obs: seaglass.TavernObservation,
        legal: Sequence[seaglass.TavernAction],
        build: ActiveBuild,
    ) -> seaglass.TavernAction:
        ehp = obs.health + obs.armor
        trinket_costs = list(obs.trinket_option_costs) if obs.trinket_option_costs else []
        board_tribes: Dict[str, int] = {}
        for u in obs.board:
            for tr in u.tribes:
                board_tribes[tr] = board_tribes.get(tr, 0) + 1

        best_act = legal[0]
        best_sc = -1e9
        for act in legal:
            if act.kind != "choose_trinket":
                continue
            idx = act.args[0]
            if idx is None or idx >= len(obs.trinket_options):
                continue
            tid = int(obs.trinket_options[idx])
            tname = self._trinket_names.get(tid, "")
            cost = int(trinket_costs[idx]) if idx < len(trinket_costs) else 0
            sc = GM_TRINKET_SCORES.get(tname, 16.0) - 0.85 * cost

            # Strong active-comp synergy multipliers
            ek = build.engine.key
            if tname == "Drakkari Portrait" and ek in ("mech_magnetic", "spells_amalgam", "shop_eat_elem_demon", "quilboar_gems", "murloc_bc_hand"):
                sc += 14.0
            elif tname == "Titus Portrait" and ek in ("beast_dr_rally", "undead_reborn", "aberration_deity", "mech_magnetic"):
                sc += 14.0
            elif tname == "Bronzebeard Portrait" and ek in ("murloc_bc_hand", "dragon_poet", "quilboar_gems", "pirate_apm_golden"):
                sc += 14.0
            elif tname in ("Volumizer Portrait", "Beatboxer Portrait", "Sky Golem Portrait", "Boom's Monster Portrait", "Accord-o-Tron Portrait"):
                if ek == "mech_magnetic" or board_tribes.get("mech", 0) >= 2:
                    sc += 14.0
                if tname == "Volumizer Portrait" and build.has_duplicator:
                    sc += 12.0
            elif tname in ("Flaming Portrait", "Nomi Sticker", "Azerite Portrait"):
                if ek == "shop_eat_elem_demon" or board_tribes.get("elemental", 0) + board_tribes.get("demon", 0) >= 2:
                    sc += 12.0
            elif tname in ("Great Boar Sticker", "Quilligraphy Set") and (ek == "quilboar_gems" or board_tribes.get("quilboar", 0) >= 2):
                sc += 12.0
            elif tname in ("Morgl Portrait", "Murky Sticker", "Young Murk-Eye Sticker", "Nerglish Phrasebook", "Dramaloc Sticker"):
                if ek == "murloc_bc_hand" or board_tribes.get("murloc", 0) >= 2:
                    sc += 12.0
            elif tname == "Maldraxxus Dagger" and ehp <= 22:
                sc -= 20.0
            elif tname == "Souvenir Stand" and ehp <= 20:
                sc -= 18.0
            elif tname == "Safety Patch" and ehp <= 12:
                sc += 14.0

            if sc > best_sc:
                best_sc = sc
                best_act = act
        return best_act

    def _choose_discover(
        self,
        obs: seaglass.TavernObservation,
        legal: Sequence[seaglass.TavernAction],
        build: ActiveBuild,
        ctx: Optional[seaglass.LobbyContext],
    ) -> seaglass.TavernAction:
        dp = obs.discover_pending
        options = getattr(dp, "options", None) if dp is not None else None
        if not options:
            return legal[0]

        owned_counts: Dict[int, int] = {}
        for u in list(obs.board) + list(obs.hand):
            if not u.is_golden and not u.is_spell:
                owned_counts[u.card_id] = owned_counts.get(u.card_id, 0) + 1

        best_act = legal[0]
        best_sc = -1e9
        for act in legal:
            if act.kind != "choose_discover":
                continue
            idx = act.args[0]
            if idx is None or idx >= len(options):
                continue
            card = options[idx]
            sc = self._score_candidate_card(card, obs, build, owned_counts, from_discover=True)
            # Bonus for Dark Gifts on discovered minions!
            if card.dark_gifts:
                sc += 12.0
            if card.is_golden:
                sc += 28.0
            if sc > best_sc:
                best_sc = sc
                best_act = act
        return best_act

    # -------------------------------------------------------------------------
    # Core Card Desire & Board Keep-Value Valuation
    # -------------------------------------------------------------------------
    def _score_candidate_card(
        self,
        card: seaglass.Unit,
        obs: seaglass.TavernObservation,
        build: ActiveBuild,
        owned_counts: Dict[int, int],
        from_discover: bool = False,
    ) -> float:
        name = card.name
        turn = obs.turn
        eng = build.engine

        # Spells
        if card.is_spell:
            # Avoid off-tribe spells like Cloning Conch (Murlocs) or Butchering (Undead)
            if name == "Cloning Conch" and not any("murloc" in u.tribes or "all" in u.tribes for u in obs.board):
                return 0.0
            if name == "Butchering" and not any("undead" in u.tribes or "all" in u.tribes for u in obs.board):
                return 0.0
            if name in DANGEROUS_SELF_SPELLS and not (name == "Butchering" and build.has_snazzy):
                return 5.0
            base = PAYOFF_SPELLS.get(name, 30.0)
            if build.has_amalgam and name in AMALGAM_TRIGGER_SPELLS:
                base += 28.0 * (1.8 if build.has_balinda else 1.0)
            if build.has_harbinger and name in PAYOFF_SPELLS and PAYOFF_SPELLS[name] >= 75.0:
                base += 16.0
            if eng.wants_spells:
                base += 14.0
            if name == "Strike Oil" and turn <= 9:
                base += 18.0
            elif name == "Tavern Coin":
                base = 75.0
            elif name == "Upper Hand" and (build.is_finals or build.scam_hunt):
                base = 88.0
            elif name == "Eyes of the Earth Mother":
                base = 92.0
            # Penalize expensive low-impact spells early
            if not from_discover and card.spell_cost >= 3 and base < 65.0 and turn <= 6:
                base -= 18.0
            if not from_discover and card.costs_health and "Soul Rewinder" not in {u.name for u in obs.board}:
                if obs.health + obs.armor <= 18:
                    return -50.0
            return base

        # Minions
        sc = 0.0
        # 1. Universal Tier-6/7 God Cards
        if name in UNIVERSAL_GOD_CARDS:
            sc = max(sc, UNIVERSAL_GOD_CARDS[name])

        # 2. Finals / Scam-Hunt Counter-Tech
        if name in SCAM_NAMES or card.venomous:
            if build.scam_hunt or build.is_finals:
                sc = max(sc, 92.0 if name in ("Leeroy the Reckless", "Bile Spitter", "Tide Oracle Morgl") else 86.0)
            elif turn >= 10 and build.enemy_max_hp >= 180:
                sc = max(sc, 64.0)

        # 3. Active Engine Lynchpins / Multipliers / Scalers
        if name in eng.lynchpins:
            sc = max(sc, 62.0 + eng.lynchpins[name])
        elif name in eng.multipliers:
            # Don't buy duplicate non-stacking aura, and only buy Brann/Drakkari/Titus if Turn <= 10 OR >= 2 matching board units!
            copies = owned_counts.get(card.card_id, 0)
            ok_mult = True
            if turn >= 10:
                if name == "Brann Bronzebeard" and sum(1 for u in obs.board if u.card_id in self._bc_ids) < 2 and eng.key not in ("murloc_bc_hand", "pirate_apm_golden"):
                    ok_mult = False
                elif name == "Titus Rivendare" and sum(1 for u in obs.board if u.card_id in self._dr_ids or u.extra_deathrattles > 0) < 2:
                    ok_mult = False
                elif name == "Drakkari Enchanter" and sum(1 for u in obs.board if u.card_id in self._eot_ids or u.end_of_turn_stats > 0) < 1:
                    ok_mult = False
            if ok_mult:
                if name in ("Brann Bronzebeard", "Drakkari Enchanter", "Soul Rewinder") and copies == 1:
                    sc = max(sc, 44.0)  # Pair potential only
                else:
                    sc = max(sc, 58.0 + eng.multipliers[name])
        elif name in eng.scalers:
            sc = max(sc, 42.0 + eng.scalers[name] * (1.2 if build.committed else 0.85))

        # 4. Pivot Lynchpin from ANOTHER available engine (if not deeply committed yet, or Tier 5/6/7 megacarry!)
        if not build.committed or turn <= 10:
            for other in ENGINES:
                if other.key == eng.key:
                    continue
                if name in other.lynchpins and other.lynchpins[name] >= 24.0:
                    pivot_w = 56.0 + other.lynchpins[name] * (1.0 if not build.committed else 0.70)
                    sc = max(sc, pivot_w)

        # 5. Direct Engine Fuel!
        if card.magnetic and (build.has_duplicator or build.has_beatboxer or eng.wants_magnetics):
            sc = max(sc, 72.0 if (build.has_duplicator or build.has_beatboxer) else 58.0)
        if name in GOLDENIZER_NAMES:
            # Elite Navigator / Captain Sanders -> massive if we have a high-tier engine!
            if any(u.tavern_tier >= 4 and not u.is_golden for u in obs.board):
                sc = max(sc, 82.0)
        if name in ECONOMY_MINIONS:
            if turn <= 6:
                sc = max(sc, 52.0)
            elif "Brann Bronzebeard" in {u.name for u in obs.board} and card.card_id in self._bc_ids:
                sc = max(sc, 64.0)
            elif name in ("Primalfin Lookout", "Rodeo Performer", "Tavern Tempest", "Doubloon Grifter"):
                sc = max(sc, 54.0)
        if eng.wants_battlecries and card.card_id in self._bc_ids and "Brann Bronzebeard" in {u.name for u in obs.board}:
            sc = max(sc, 56.0)
        if eng.wants_reborn and (card.reborn or name in ("Dead Bellringer", "Mummifier", "Prosthetic Hand")):
            sc = max(sc, 60.0)
        if build.has_murloc_handbuff and ("murloc" in card.tribes or "all" in card.tribes):
            sc = max(sc, 52.0)
        if eng.key == "shop_eat_elem_demon" and ("elemental" in card.tribes or "demon" in card.tribes):
            if "Arcane Behemoth" in {u.name for u in obs.board} or "Living Azerite" in {u.name for u in obs.board}:
                sc = max(sc, 54.0)

        # 6. Triple Completion!
        copies = owned_counts.get(card.card_id, 0)
        if not card.is_golden and copies >= 2:
            if sc >= 45.0 or card.tavern_tier >= max(2, obs.tavern_tier - 1) or turn <= 8:
                sc = max(sc, 84.0 + 2.0 * card.tavern_tier)
        elif not card.is_golden and copies == 1 and turn <= 7 and sc >= 35.0:
            sc += 8.0

        # 7. Early Game Tempo (Turns 1-6 or filling open board slots)
        if turn == 1:
            sc = max(sc, T1_OPENING_SCORE.get(name, 14.0 + card.attack + card.health))
        elif turn <= 6 or len(obs.board) < 7:
            stat_val = (card.attack + card.health) * 1.6 + (8.0 if card.divine_shield else 0.0) + (5.0 if card.reborn else 0.0)
            tier_val = card.tavern_tier * 6.5
            # Don't buy off-tribe Soul Rewinder with 0 Demons!
            if name == "Soul Rewinder" and not any("demon" in u.tribes for u in obs.board):
                stat_val -= 14.0
            sc = max(sc, tier_val + min(34.0, stat_val))
        else:
            # Late game (Turn 7+ on full 7-unit board): huge shop stats can still matter if we're eating/buffing shop
            st = card.attack + card.health
            weakest = min((u.attack + u.health for u in obs.board), default=0)
            if st > max(60, weakest * 1.8):
                sc = max(sc, 52.0)

        # Avoid health-cost trap without Soul Rewinder
        if not from_discover and card.costs_health and "Soul Rewinder" not in {u.name for u in obs.board}:
            if obs.health + obs.armor <= 18:
                return -50.0

        return sc

    def _board_keep_value(
        self,
        u: seaglass.Unit,
        obs: seaglass.TavernObservation,
        build: ActiveBuild,
    ) -> float:
        """How valuable is keeping unit `u` on our board? Lowest keep-value is sold first!"""
        name = u.name
        eng = build.engine
        stats = u.attack + u.health

        # Finals Scam-Hunt: dead Tavern scalers become expendable!
        if (build.is_finals or build.scam_hunt) and obs.turn >= 10:
            if name in SCAM_NAMES or u.venomous:
                return 5000.0 + stats
            if name in FINALS_DEAD_SCALERS and stats < 220:
                return 10.0 + 0.1 * stats
            return stats * (1.45 if u.divine_shield else 1.0) * (1.25 if u.reborn else 1.0)

        # Normal scaling game: protect core engines, multipliers, huge carries, and golden Greedy Conniver!
        val = float(stats) * (1.25 if u.divine_shield else 1.0) + (15.0 if u.reborn else 0.0)
        if u.is_golden:
            val += 80.0
        if name == "The Last One Standing":
            return 1e6 + val
        if name in UNIVERSAL_GOD_CARDS:
            val += 320.0
        if name in eng.lynchpins:
            val += 240.0 + 4.0 * eng.lynchpins[name]
        elif name in eng.multipliers:
            val += 220.0 + 3.0 * eng.multipliers[name]
        elif name in eng.scalers:
            val += 90.0 + 2.0 * eng.scalers[name]

        # Spent one-shot Battlecries have near-zero keep value (ideal cycle slots!)
        if name in SPENT_BATTLECRIES and not any(bu.name in ("Young Murk-Eye", "Kelp Keeper") for bu in obs.board):
            if stats < 70:
                val *= 0.25

        # Non-stacking duplicate aura: 2nd copy of Brann/Drakkari/Rewinder when 1st is already Golden
        if name in ("Brann Bronzebeard", "Drakkari Enchanter", "Soul Rewinder"):
            same = [bu for bu in obs.board if bu.name == name]
            if len(same) >= 2 and any(bu.is_golden for bu in same) and not u.is_golden:
                val = 5.0 + 0.1 * stats

        # Scam units have high keep value on Turn 10+
        if (name in SCAM_NAMES or u.venomous) and obs.turn >= 10:
            val += 220.0

        return val

    def _weakest_board_slot(
        self,
        obs: seaglass.TavernObservation,
        build: ActiveBuild,
    ) -> Tuple[int, float]:
        best_i = 0
        best_v = 1e18
        for i, u in enumerate(obs.board):
            v = self._board_keep_value(u, obs, build)
            if v < best_v:
                best_v = v
                best_i = i
        return best_i, best_v

    # -------------------------------------------------------------------------
    # Macro Leveling Curve & DarkDiscovery Timing
    # -------------------------------------------------------------------------
    def _should_upgrade(
        self,
        obs: seaglass.TavernObservation,
        build: ActiveBuild,
        ctx: Optional[seaglass.LobbyContext],
    ) -> bool:
        tier = obs.tavern_tier
        turn = obs.turn
        cost = obs.upgrade_cost
        gold = obs.gold
        ehp = obs.health + obs.armor
        next_is_ghost = bool(ctx.next_is_ghost) if ctx is not None else False
        if tier >= 6 or gold < cost:
            return False

        hname = self._hero_names.get(int(obs.hero_id), "")

        # Turn 2 (4g): Standard Curve -> ALWAYS Upgrade to Tier 2!
        if turn == 2 and tier == 1:
            if hname == "Millhouse Manastorm":
                return False
            return True

        # Turn 3 (5g): Upgrade if still Tier 1! Or 3-on-3 if cheap!
        if turn == 3:
            if tier == 1:
                return True
            if tier == 2 and cost <= gold and (next_is_ghost or hname in ("Forest Warden Omu", "Galewing")):
                return True
            return False

        # Turn 4 (6g): Upgrade if still Tier 1! (Never stay Tier 1!)
        if turn == 4:
            return tier <= 1 or (tier == 2 and cost <= 4)

        # Turn 5 (7g): Standard Curve -> UPGRADE TO TIER 3!
        if turn == 5 and tier <= 2:
            return cost <= 5 or next_is_ghost

        # Turn 6 (8g): Upgrade to Tier 3 if still T2; push Tier 4 if cheap & healthy!
        if turn == 6:
            if tier <= 2:
                return True
            if tier == 3 and cost <= 6 and (ehp >= 26 or next_is_ghost):
                return True
            return False

        # Turn 7 (9g): Standard Curve -> UPGRADE TO TIER 4!
        if turn == 7:
            if tier <= 3 and (cost <= 7 or ehp >= 20 or next_is_ghost):
                return True
            return False

        # Turn 8 (10g): Push Tier 5 early if healthy or Ghost!
        if turn == 8:
            if tier <= 3:
                return True
            if tier == 4 and (next_is_ghost or (ehp >= 22 and cost <= 8) or cost <= 5):
                return True
            return False

        # Turn 9 (10g+): GUARANTEE Tier 5 (unless dying at <= 13 HP); push Tier 6 if already T5 & healthy!
        if turn == 9:
            if tier <= 4 and (next_is_ghost or ehp >= 15 or cost <= 6):
                return True
            if tier == 5 and (next_is_ghost or (ehp >= 25 and cost <= 7) or cost <= 4):
                return True
            return False

        # Turn 10+: Push to Tier 6 whenever safe (never stall on Tier 4/5!)
        if turn >= 10 and not build.is_finals:
            if tier <= 4 and (next_is_ghost or ehp >= 14 or cost <= 6):
                return True
            if tier == 5 and (next_is_ghost or (ehp >= 18 and cost <= 8) or cost <= 5):
                return True

        return False

    def _should_use_dark_discovery(
        self,
        obs: seaglass.TavernObservation,
        build: ActiveBuild,
        ctx: Optional[seaglass.LobbyContext],
    ) -> bool:
        """
        Patch 37.0.2 Secret Weapon:
          - Turn 5-7 offers Tier 3/4 random trash -> NEVER waste charges early!
          - Turn 9 offers Tier 4/5/6 with `FirstTwo` matching our most common tribe!
          - Turn 10+ offers TIER 5 & TIER 6 ONLY with `FirstTwo` matching our most common tribe + Late Dark Gifts!
        """
        if not obs.dark_discovery_usable or obs.dark_discoveries_left <= 0:
            return False
        if obs.gold < 3 or len(obs.hand) >= 10:
            return False
        turn = obs.turn
        ehp = obs.health + obs.armor
        # Emergency survival on Turn 8 only if critically low HP
        if turn == 8 and ehp <= 14:
            return True
        # Turn 9: use 1 charge if we still have >= 2 left (saves 2 charges for pure Tier 5/6 on Turn 10+!)
        if turn == 9 and obs.dark_discoveries_left >= 2:
            return True
        # Turn 10+: ALWAYS fire DarkDiscovery (3g -> guaranteed Tier 5/6 targeted tribe Dark Gift!)
        if turn >= 10:
            return True
        return False

    # -------------------------------------------------------------------------
    # Target Selection Helpers (Exact Engine Mechanics!)
    # -------------------------------------------------------------------------
    def _best_spell_target(
        self,
        spell: seaglass.Unit,
        obs: seaglass.TavernObservation,
        build: ActiveBuild,
    ) -> int:
        board = obs.board
        if not board:
            return 0
        sname = spell.name

        # 1. Goldenizer spell (`Eyes of the Earth Mother`) -> best non-golden Tier 4-6 engine!
        if sname == "Eyes of the Earth Mother":
            best_i = 0
            best_v = -1e9
            for i, u in enumerate(board):
                if u.is_golden or u.tavern_tier > 6:
                    continue
                v = self._board_keep_value(u, obs, build) + 150.0 * u.tavern_tier
                if u.name == "Greedy Conniver":
                    v += 500.0  # Golden Greedy Conniver -> Sell for Tier 7!
                if v > best_v:
                    best_v = v
                    best_i = i
            return best_i

        # 2. `Butchering` -> target a Reborn / Plaguerunner Undead (NOT our right-most carry!)
        if sname == "Butchering":
            for i, u in enumerate(board):
                if i < len(board) - 1 and ("undead" in u.tribes or "all" in u.tribes) and (u.reborn or u.name == "Plaguerunner"):
                    return i
            for i, u in enumerate(board):
                if "undead" in u.tribes:
                    return i
            return 0

        # 3. `Gatekeeper Amalgam` on board -> ALWAYS target Gatekeeper Amalgam for targeted buff spells!
        if build.has_amalgam and sname in AMALGAM_TRIGGER_SPELLS:
            for i, u in enumerate(board):
                if u.name == "Gatekeeper Amalgam":
                    return i

        # 4. `Geomagus Roogug` on board for Blood Gems!
        if sname == "Blood Gem":
            for i, u in enumerate(board):
                if u.name == "Geomagus Roogug":
                    return i

        # 5. Otherwise target our best combat carry (Last One Standing > Cleave/DS > Biggest scaler)
        best_i = 0
        best_v = -1e9
        for i, u in enumerate(board):
            v = float(u.attack + u.health)
            if u.name == "The Last One Standing":
                v += 10000.0
            elif u.divine_shield:
                v += 220.0
            elif u.card_id in self._cleave_ids:
                v += 200.0
            elif u.name in ("Tarecgosa", "Heroic Broodmother", "Crimson Vindicator", "Barrier Banshee", "Drone Duplicator", "Polarizing Beatboxer", "Flaming Enforcer"):
                v += 180.0
            if u.name in FINALS_DEAD_SCALERS and not u.divine_shield:
                v -= 150.0
            if v > best_v:
                best_v = v
                best_i = i
        return best_i

    def _best_magnetic_target(
        self,
        obs: seaglass.TavernObservation,
        build: ActiveBuild,
    ) -> Optional[int]:
        """Always magnetize onto Drone Duplicator first; NEVER directly onto Polarizing Beatboxer!"""
        board = obs.board
        # Priority 1: Drone Duplicator (especially if activated!)
        for i, u in enumerate(board):
            if u.name == "Drone Duplicator":
                return i
        # Priority 2: Best non-Beatboxer Mech/All
        best_i = None
        best_v = -1e9
        for i, u in enumerate(board):
            if not ("mech" in u.tribes or "all" in u.tribes):
                continue
            if u.name == "Polarizing Beatboxer" and build.has_beatboxer:
                continue
            v = float(u.attack + u.health) + (200.0 if u.divine_shield else 0.0) + (150.0 if u.name in ("Utility Drone", "Resourceful Robot", "Charging Czarina", "Cord Puller") else 0.0)
            if v > best_v:
                best_v = v
                best_i = i
        if best_i is not None:
            return best_i
        # Fallback: Beatboxer only if it's literally the only Mech
        for i, u in enumerate(board):
            if "mech" in u.tribes or "all" in u.tribes:
                return i
        return None

    # -------------------------------------------------------------------------
    # End-of-Turn Comp-Invariant + Exact Rust Monte Carlo Positioning
    # -------------------------------------------------------------------------
    def _heuristic_perm_score(
        self,
        board: Sequence[seaglass.Unit],
        perm: Sequence[int],
        obs: seaglass.TavernObservation,
    ) -> float:
        n = len(perm)
        if n <= 1:
            return 0.0
        ordered = [board[i] for i in perm]
        score = 0.0
        has_poet = any(u.name == "Persistent Poet" for u in ordered)
        has_snazzy = any(u.name == "Snazzy Phantom" for u in ordered)
        has_deathstrider = any(u.name == "Deathstrider" for u in ordered)
        has_murkeye = any(u.name == "Young Murk-Eye" for u in ordered)

        for pos, u in enumerate(ordered):
            frac = pos / max(1, n - 1)
            name = u.name
            # 1. The Last One Standing MUST attack FIRST (Slot 0)!
            if name == "The Last One Standing":
                score += 60.0 * (1.0 - frac)
            # 2. Sin'dorei / Bile Spitter / Sanguine Refiner / Cleave / Rally attack early!
            elif name == "Sin'dorei Straight Shot":
                score += 45.0 * (1.0 - frac)
            elif name in ("Bile Spitter", "Sanguine Refiner", "Sanguine Champion"):
                score += 32.0 * (1.0 - frac)
            elif u.card_id in self._cleave_ids or u.card_id in self._rally_ids:
                score += 24.0 * (1.0 - frac)
            # 3. Fragile backline multipliers protected on right!
            elif name in ("Titus Rivendare", "Banana Slamma", "Brann Bronzebeard", "Drakkari Enchanter", "Lurking Leviathan"):
                if not u.taunt:
                    score += 30.0 * frac
            elif u.card_id in self._avenge_ids and not u.taunt:
                score += 18.0 * frac
            # 4. Taunt on edge to minimize enemy Cleave splash!
            if u.taunt:
                if pos in (0, n - 1):
                    score += 6.0
                else:
                    score -= 4.0
            # 5. Divine Shield big carries attack early
            if u.divine_shield and u.attack >= 20:
                score += 8.0 * (1.0 - frac)

        # Persistent Poet strict interior Dragon sandwich!
        if has_poet:
            for pos, u in enumerate(ordered):
                if u.name == "Persistent Poet":
                    adj = 0
                    for nb_i in (pos - 1, pos + 1):
                        if 0 <= nb_i < n:
                            nb = ordered[nb_i]
                            if ("dragon" in nb.tribes or "all" in nb.tribes) and nb.name not in ("Persistent Poet", "Tarecgosa"):
                                adj += 1
                                if nb.name in ("Heroic Broodmother", "Crimson Vindicator", "Runic Arcanist"):
                                    score += 22.0
                    if adj == 2:
                        score += 28.0
                    elif pos in (0, n - 1):
                        score -= 22.0

        # Snazzy Phantom -> right-most Undead carry + Reborn left!
        if has_snazzy:
            rm_u = None
            for i in range(n - 1, -1, -1):
                if "undead" in ordered[i].tribes or "all" in ordered[i].tribes:
                    rm_u = i
                    break
            if rm_u is not None:
                c = ordered[rm_u]
                if c.name == "Barrier Banshee" or c.divine_shield:
                    score += 26.0
                for pos, u in enumerate(ordered):
                    if u.reborn and pos < rm_u:
                        score += 6.0

        # Deathstrider -> left-most Deathrattle MUST be Goldrinn / Turquoise Skitterer!
        if has_deathstrider:
            for u in ordered:
                if u.card_id in self._dr_ids or u.extra_deathrattles > 0:
                    if u.name in ("Goldrinn, the Great Wolf", "Turquoise Skitterer"):
                        score += 36.0
                    elif u.name in ("Eternal Summoner", "Sanguine Champion"):
                        score += 18.0
                    else:
                        score -= 12.0
                    break

        # Young Murk-Eye adjacent to Battlecries!
        if has_murkeye:
            for pos, u in enumerate(ordered):
                if u.name == "Young Murk-Eye":
                    for nb_i in (pos - 1, pos + 1):
                        if 0 <= nb_i < n and ordered[nb_i].card_id in self._bc_ids:
                            score += 22.0

        return score

    def _compute_best_perm(
        self,
        lobby: seaglass.LobbyGame,
        player: int,
        obs: seaglass.TavernObservation,
        ctx: Optional[seaglass.LobbyContext],
    ) -> List[int]:
        board = obs.board
        n = len(board)
        if n <= 1:
            return list(range(n))

        # 1. Fast steepest-ascent 2-opt on heuristic comp invariants
        perm = list(range(n))
        best_h = self._heuristic_perm_score(board, perm, obs)
        for _ in range(3):
            improved = False
            for i in range(n):
                for j in range(i + 1, n):
                    cand = list(perm)
                    cand[i], cand[j] = cand[j], cand[i]
                    sc = self._heuristic_perm_score(board, cand, obs)
                    if sc > best_h + 1e-6:
                        best_h = sc
                        perm = cand
                        improved = True
            if not improved:
                break

        # 2. If we have an observed opponent board (especially Top-4 / Finals!), refine with exact Rust combat!
        opp_board = None
        opp_tier = min(6, max(1, obs.tavern_tier))
        if ctx is not None and ctx.opponents:
            for opp in ctx.opponents:
                if opp.is_next and opp.seen_board:
                    opp_board = list(opp.seen_board)
                    opp_tier = opp.tier
                    break
        if opp_board and obs.turn >= 6 and (ctx is None or not ctx.next_is_ghost):
            try:
                base_g = lobby.tavern(player)
                cands: List[Tuple[int, ...]] = [tuple(perm)]
                seen_c = {tuple(perm)}
                # Generate swaps & shifts around heuristic best
                for i in range(n):
                    for j in range(i + 1, n):
                        c = list(perm)
                        c[i], c[j] = c[j], c[i]
                        ct = tuple(c)
                        if ct not in seen_c:
                            seen_c.add(ct)
                            cands.append(ct)
                alive_units = ctx.alive_units if ctx is not None else 8
                max_c = 28 if alive_units <= 3 else 14
                combats = 20 if alive_units <= 2 else 10
                # Rank candidates by heuristic first to keep top `max_c`
                cands.sort(key=lambda p: self._heuristic_perm_score(board, p, obs), reverse=True)
                cands = cands[:max_c]

                best_p = list(cands[0])
                best_ev = -1e18
                for cand in cands:
                    g_sim = base_g.copy()
                    for act in self._perm_to_actions(cand):
                        if g_sim.is_legal(act):
                            g_sim.step(act)
                    if g_sim.is_legal(seaglass.TavernAction.end_turn()):
                        g_sim.end_turn()
                    dist = g_sim.equity_against(
                        opp_board,
                        opponent_tier=opp_tier,
                        n=combats,
                        base_seed=obs.turn * 7919 + 13,
                    )
                    # Blend combat winrate + small comp-invariant tiebreaker (so Poet/Murk-Eye/Snazzy stay intact!)
                    ev = 100.0 * dist.a_win_rate + 35.0 * dist.draw_rate + 0.8 * dist.damage_mean + 0.25 * self._heuristic_perm_score(board, cand, obs)
                    if ev > best_ev:
                        best_ev = ev
                        best_p = list(cand)
                return best_p
            except Exception:
                pass
        return perm

    @staticmethod
    def _perm_to_actions(target_perm: Sequence[int]) -> List[seaglass.TavernAction]:
        cur = list(range(len(target_perm)))
        acts: List[seaglass.TavernAction] = []
        for dst, orig_idx in enumerate(target_perm):
            src = cur.index(orig_idx)
            if src != dst:
                acts.append(seaglass.TavernAction.reposition(src, dst))
                val = cur.pop(src)
                cur.insert(dst, val)
        return acts

    # -------------------------------------------------------------------------
    # Main Lobby Action Entrypoint
    # -------------------------------------------------------------------------
    def act_in_lobby(
        self,
        lobby: seaglass.LobbyGame,
        player: int,
    ) -> seaglass.TavernAction:
        obs = lobby.observe(player)
        ctx = lobby.context(player)
        legal = lobby.legal_actions(player)
        self.last_note = None
        if not legal:
            return seaglass.TavernAction.end_turn()
        if len(legal) == 1:
            return legal[0]

        # 1. Mandatory Prompts (Hero, Trinket, Discover)
        if legal[0].kind == "choose_hero":
            return self._choose_hero(obs, legal, ctx)

        build = self._analyze_build(obs, ctx)

        if legal[0].kind == "choose_trinket":
            return self._choose_trinket(lobby, player, obs, legal, build)

        if legal[0].kind == "choose_discover":
            return self._choose_discover(obs, legal, build, ctx)

        # Drain any queued end-of-turn positioning swaps immediately
        rq = getattr(self, "_repo_queue", {}).get(player)
        if rq:
            nxt = rq.pop(0)
            if lobby.is_legal(player, nxt):
                return nxt
            rq.clear()

        # Helper to find exact legal action
        def _legal(act: seaglass.TavernAction) -> Optional[seaglass.TavernAction]:
            return act if lobby.is_legal(player, act) else None

        turn = obs.turn
        board = obs.board
        hand = obs.hand
        shop = obs.shop
        n_board = len(board)
        ehp = obs.health + obs.armor
        next_is_ghost = bool(ctx.next_is_ghost) if ctx is not None else False

        owned_counts: Dict[int, int] = {}
        for u in list(board) + list(hand):
            if not u.is_golden and not u.is_spell:
                owned_counts[u.card_id] = owned_counts.get(u.card_id, 0) + 1

        # 2. Free Gold Spells FIRST (`Tavern Coin`, `Strike Oil`, `Careful Investment` if free/0g)
        for hi, hc in enumerate(hand):
            if hc.is_spell and not hc.unplayable and hc.name in ("Tavern Coin",):
                act = _legal(seaglass.TavernAction.play(hi, 0))
                if act is not None:
                    return act

        # 3. Golden `Greedy Conniver` -> SELL IMMEDIATELY for a FREE Tier 7 Discover!!
        for bi, bu in enumerate(board):
            if bu.name == "Greedy Conniver" and bu.is_golden:
                act = _legal(seaglass.TavernAction.sell(bi))
                if act is not None:
                    return act

        # 4. Macro Leveling Curve (Decide BEFORE spending gold on random shop cards!)
        if self._should_upgrade(obs, build, ctx):
            up_act = _legal(seaglass.TavernAction.upgrade_tavern())
            if up_act is not None:
                return up_act
        # Turn 5-9 sell weak non-engine filler on 6/7-board if `gold + 1 == upgrade_cost` and we want to level!
        if (
            turn >= 5
            and obs.tavern_tier < 6
            and obs.gold + 1 == obs.upgrade_cost
            and n_board >= 5
            and (next_is_ghost or ehp >= 22)
        ):
            # Temporarily check if upgrading would be desired with +1 gold
            want_up = (
                (turn == 5 and obs.tavern_tier <= 2)
                or (turn == 7 and obs.tavern_tier <= 3)
                or (turn >= 8 and obs.tavern_tier <= 4)
                or (turn >= 10 and obs.tavern_tier == 5)
            )
            if want_up:
                wi, wv = self._weakest_board_slot(obs, build)
                wu = board[wi]
                if not wu.is_golden and wv < 50.0 and (wu.attack + wu.health) < 28:
                    act = _legal(seaglass.TavernAction.sell(wi))
                    if act is not None:
                        return act

        # 5. Patch 37.0.2 Secret Weapon: Targeted Turn 9-12 `DarkDiscovery`!
        if self._should_use_dark_discovery(obs, build, ctx):
            # Tribe alignment (MAX 1 sell, ONLY if board is full 7 units and selling 1 tiny <20-stat filler strictly wins majority!)
            pt = build.engine.primary_tribe
            if pt is not None and build.committed and n_board == 7:
                t_counts: Dict[str, int] = {}
                for u in board:
                    for tr in u.tribes:
                        if tr != "all":
                            t_counts[tr] = t_counts.get(tr, 0) + 1
                my_cnt = t_counts.get(pt, 0)
                max_other = max((v for k, v in t_counts.items() if k != pt), default=0)
                if my_cnt >= 2 and max_other == my_cnt:
                    for bi, bu in enumerate(board):
                        if pt not in bu.tribes and "all" not in bu.tribes:
                            if any(t_counts.get(tr, 0) == max_other for tr in bu.tribes):
                                if self._board_keep_value(bu, obs, build) < 40.0 and (bu.attack + bu.health) < 20:
                                    s_act = _legal(seaglass.TavernAction.sell(bi))
                                    if s_act is not None:
                                        return s_act
            dd_act = _legal(seaglass.TavernAction.dark_discovery())
            if dd_act is not None:
                return dd_act

        # 6. Exact Mechanical Engine Activations BEFORE Playing Hand / Magnetics!
        # (6a) `Drone Duplicator`: Activate (1g) BEFORE magnetizing ANY Magnetic in hand!
        has_hand_magnetic = any(not c.is_spell and c.magnetic and not c.unplayable for c in hand)
        if has_hand_magnetic and obs.gold >= 1:
            for bi, bu in enumerate(board):
                if bu.name == "Drone Duplicator" and not bu.activated_this_turn:
                    act = _legal(seaglass.TavernAction.activate(bi))
                    if act is not None:
                        return act

        # (6b) `Snazzy Phantom` right-most prep BEFORE `Dead Bellringer` / `Butchering`!
        if build.has_snazzy and n_board >= 2:
            best_c = None
            best_cs = -1e9
            for i, u in enumerate(board):
                if u.name == "Dead Bellringer":
                    continue
                if "undead" in u.tribes or "all" in u.tribes:
                    cs = (100.0 if u.name == "Barrier Banshee" else 0.0) + (40.0 if u.divine_shield else 0.0) + (u.attack + u.health)
                    if cs > best_cs:
                        best_cs = cs
                        best_c = i
            has_bell = any(u.name == "Dead Bellringer" and not u.activated_this_turn for u in board) and obs.gold >= 1
            has_butcher_sp = any(c.is_spell and c.name == "Butchering" and not c.unplayable for c in hand)
            if (has_bell or has_butcher_sp) and best_c is not None and best_c != n_board - 1:
                rep = _legal(seaglass.TavernAction.reposition(best_c, n_board - 1))
                if rep is not None:
                    return rep

        # (6c) `Victorious Geomant` Activate (2g) (with Amalgam/Roogug slot-0 prep on overflow!)
        if obs.gold >= 2:
            for bi, bu in enumerate(board):
                if bu.name == "Victorious Geomant" and not bu.activated_this_turn:
                    if len(hand) >= 6 and not build.has_surveyor:
                        for ti, tu in enumerate(board):
                            if ti != 0 and tu.name in ("Gatekeeper Amalgam", "Geomagus Roogug"):
                                rep = _legal(seaglass.TavernAction.reposition(ti, 0))
                                if rep is not None:
                                    return rep
                    act = _legal(seaglass.TavernAction.activate(bi))
                    if act is not None:
                        return act

        # (6d) `Dead Bellringer` Activate (1g) on Reborn/Plaguerunner Undead!
        if obs.gold >= 1:
            for bi, bu in enumerate(board):
                if bu.name == "Dead Bellringer" and not bu.activated_this_turn:
                    for ti, tu in enumerate(board):
                        if ti == bi or (build.has_snazzy and ti == n_board - 1):
                            continue
                        if ("undead" in tu.tribes or "all" in tu.tribes) and (tu.reborn or tu.name == "Plaguerunner"):
                            act = _legal(seaglass.TavernAction.activate(bi, ti))
                            if act is not None:
                                return act

        # (6e) Other high-value board Activations (`Living Prison` before buying big shop unit, `Suspicious Prisonguard`, `Deft Deserter`, `Soulkeeping Jailer`, `Sacrificial Wrathguard`, `Fruit Vendor`, `Clever Castaway`)
        for act in legal:
            if act.kind == "activate":
                bi = act.args[0]
                if bi is not None and bi < n_board:
                    bname = board[bi].name
                    if bname == "Living Prison" and obs.gold >= 1 + obs.minion_cost and shop:
                        return act
                    if bname in ("Suspicious Prisonguard", "Clever Castaway", "Fruit Vendor", "Hired Mount", "Deft Deserter", "Soulkeeping Jailer", "Sacrificial Wrathguard"):
                        return act

        # 7. Play Hand Cards (Magnetics -> Minions -> Spells with Harbinger Ordering!)
        # (7a) Magnetics FIRST (directly onto Drone Duplicator or best non-Beatboxer Mech — works even at 7 board!)
        for hi, hc in enumerate(hand):
            if not hc.is_spell and hc.magnetic and not hc.unplayable:
                mt = self._best_magnetic_target(obs, build)
                if mt is not None:
                    act = _legal(seaglass.TavernAction.play(hi, mt))
                    if act is not None:
                        return act

        # (7b) Goldenizers from hand (`Captain Sanders`, `Elite Navigator`)!
        for hi, hc in enumerate(hand):
            if not hc.is_spell and not hc.unplayable and hc.name in ("Captain Sanders", "Elite Navigator"):
                if n_board < 7:
                    # Target best non-golden engine!
                    best_t = 0
                    best_tv = -1e9
                    for bi, bu in enumerate(board):
                        if not bu.is_golden and (hc.name == "Captain Sanders" or "pirate" in bu.tribes or "all" in bu.tribes):
                            tv = self._board_keep_value(bu, obs, build) + (500.0 if bu.name == "Greedy Conniver" else 0.0)
                            if tv > best_tv:
                                best_tv = tv
                                best_t = bi
                    act = _legal(seaglass.TavernAction.play(hi, best_t))
                    if act is not None:
                        return act

        # (7c) Play Minions from Hand!
        hold_triple_for_next_tier = (
            obs.tavern_tier in (3, 4)
            and turn <= 8
            and ehp >= 18
            and obs.gold < obs.upgrade_cost
            and obs.upgrade_cost <= 7
        )
        for hi, hc in enumerate(hand):
            if hc.is_spell or hc.unplayable:
                continue
            # Keep Bream Counter in hand if handbuff Murlocs active (until Finals)!
            if hc.name == "Bream Counter" and build.has_murloc_handbuff and not build.is_finals and turn <= 13:
                continue
            # Hold Golden Triple for higher-tier Discover if we're upgrading next turn!
            if hc.is_golden and hold_triple_for_next_tier and n_board >= 6:
                continue

            if n_board < 7:
                # Choose smart target position for targeted Battlecries / Faceless Manipulator
                pos = n_board
                if hc.name == "Faceless Manipulator" and board:
                    pos = max(range(n_board), key=lambda i: self._board_keep_value(board[i], obs, build))
                act = _legal(seaglass.TavernAction.play(hi, pos))
                if act is not None:
                    return act
                # Try any legal play for this hand index
                for la in legal:
                    if la.kind == "play" and la.args[0] == hi:
                        return la
            else:
                # Full 7-board: sell weakest slot ONLY if hand minion is a strict upgrade!
                wi, wv = self._weakest_board_slot(obs, build)
                cand_sc = self._score_candidate_card(hc, obs, build, owned_counts, from_discover=True)
                hc_keep = self._board_keep_value(hc, obs, build)
                should_swap = (
                    hc.is_golden
                    or hc.name in UNIVERSAL_GOD_CARDS
                    or hc_keep > wv + 20.0
                    or (cand_sc >= 68.0 and wv < 280.0)
                    or (cand_sc >= 46.0 and wv < 110.0)
                    or ((build.is_finals or build.scam_hunt) and (hc.name in SCAM_NAMES or hc.venomous) and wv < 360.0)
                    or ((hc.name in SPENT_BATTLECRIES or hc.name in ECONOMY_MINIONS) and wv < 65.0)
                )
                if should_swap:
                    s_act = _legal(seaglass.TavernAction.sell(wi))
                    if s_act is not None:
                        return s_act

        # (7d) Cast Spells from Hand (Harbinger ordering: filler spells FIRST, best payoff spell LAST!)
        playable_spells: List[Tuple[int, seaglass.Unit, float]] = []
        for hi, hc in enumerate(hand):
            if hc.is_spell and not hc.unplayable:
                if hc.name in DANGEROUS_SELF_SPELLS and not (hc.name == "Butchering" and build.has_snazzy):
                    continue
                pv = PAYOFF_SPELLS.get(hc.name, 40.0)
                playable_spells.append((hi, hc, pv))
        if playable_spells:
            if build.has_harbinger and len(playable_spells) >= 2:
                # Cast LOWEST payoff value first so HIGHEST payoff spell is cast LAST for Cataclysmic Harbinger!
                playable_spells.sort(key=lambda x: x[2])
            else:
                # Cast economy/targeted buff spells cleanly
                playable_spells.sort(key=lambda x: (x[1].name != "Blood Gem", -x[2]))
            for hi, hc, _ in playable_spells:
                tgt = self._best_spell_target(hc, obs, build)
                act = _legal(seaglass.TavernAction.play(hi, tgt))
                if act is not None:
                    return act
                for la in legal:
                    if la.kind in ("play", "play_on_shop") and la.args[0] == hi:
                        return la

        # 8. Hero Power Usage (Free / 1g / High-Value Scaling Hero Powers!)
        if obs.hero_power_usable and obs.gold >= obs.hero_power_cost:
            hp_acts = [a for a in legal if a.kind in ("hero_power", "hero_power_2")]
            if hp_acts:
                hname = self._hero_names.get(int(obs.hero_id), "")
                use_hp = (
                    obs.hero_power_cost == 0
                    or (obs.hero_power_cost == 1 and (turn >= 3 or hname in ("Arch-Villain Rafaam", "King Mukla", "Maiev Shadowsong", "Galewing")))
                    or (obs.hero_power_cost == 2 and turn >= 4 and hname in (
                        "Scabbs Cutterbutter", "Tess Greymane", "C'Thun", "Mutanus the Devourer",
                        "Infinite Toki", "Yogg-Saron, Hope's End", "Zerek, Master Cloner",
                        "Farseer Nobundo", "Doctor Holli'dae", "Shudderwock", "Reno Jackson",
                        "George the Fallen", "Heistbaron Togwaggle", "E.T.C., Band Manager",
                    ))
                )
                # Don't hurt ourselves at low HP with Lich Baz'hial
                if hname == "Lich Baz'hial" and ehp <= 18 and "Soul Rewinder" not in {u.name for u in board}:
                    use_hp = False
                if use_hp:
                    # Pick best target for targeted Hero Powers
                    best_hp = hp_acts[0]
                    if len(hp_acts) > 1 and board:
                        if hname in ("Mutanus the Devourer", "The Jailer"):
                            # Target weakest unit
                            wi, _ = self._weakest_board_slot(obs, build)
                            for a in hp_acts:
                                if a.args[0] == wi:
                                    best_hp = a
                                    break
                        elif hname in ("Reno Jackson", "Zerek, Master Cloner", "George the Fallen", "The Lich King"):
                            # Target strongest/highest-priority carry!
                            bi = max(range(n_board), key=lambda i: self._board_keep_value(board[i], obs, build))
                            for a in hp_acts:
                                if a.args[0] == bi:
                                    best_hp = a
                                    break
                    return best_hp

        # 9. SHOP BUYING vs AGGRESSIVE COMP-FOCUSED ROLLING!!
        # Score every affordable shop card!
        best_buy_act: Optional[seaglass.TavernAction] = None
        best_buy_sc = -1e9
        best_shop_card: Optional[seaglass.Unit] = None

        for act in legal:
            if act.kind != "buy":
                continue
            si = act.args[0]
            if si is None or si >= len(shop):
                continue
            scard = shop[si]
            # Avoid clogging hand if full
            if len(hand) >= 10:
                continue
            # Avoid buying minions onto a full 7-board that won't be played, magnetized, or tripled!
            if n_board >= 7 and not scard.is_spell and not scard.magnetic:
                is_trip = (not scard.is_golden) and owned_counts.get(scard.card_id, 0) >= 2
                if not is_trip and not (scard.name == "Bream Counter" and build.has_murloc_handbuff):
                    wi_chk, wv_chk = self._weakest_board_slot(obs, build)
                    cand_keep = self._board_keep_value(scard, obs, build)
                    cand_sc_tmp = self._score_candidate_card(scard, obs, build, owned_counts, from_discover=False)
                    can_play = (
                        scard.name in UNIVERSAL_GOD_CARDS
                        or cand_keep > wv_chk + 20.0
                        or (cand_sc_tmp >= 68.0 and wv_chk < 280.0)
                        or (cand_sc_tmp >= 46.0 and wv_chk < 110.0)
                        or ((build.is_finals or build.scam_hunt) and (scard.name in SCAM_NAMES or scard.venomous) and wv_chk < 360.0)
                        or ((scard.name in SPENT_BATTLECRIES or scard.name in ECONOMY_MINIONS) and wv_chk < 65.0)
                    )
                    if not can_play:
                        continue
            sc = self._score_candidate_card(scard, obs, build, owned_counts, from_discover=False)
            # Discount by spell cost if expensive low-impact spell
            if scard.is_spell:
                sc -= 2.5 * max(0, scard.spell_cost - 1)
            if sc > best_buy_sc:
                best_buy_sc = sc
                best_buy_act = act
                best_shop_card = scard

        # Dynamic Strict Buy Threshold:
        #   - Turns 1-6 or < 7 units on board: buy strong tempo & economy to fill board!
        #   - Turn 7+ on a full 7-unit board: ONLY buy real engine pieces, triples, fuel, or scam!
        #     Otherwise REFRESH DEEP (up to 10 refreshes/turn) to find exponential engines!
        turn_key = (player, turn)
        refs_used = self._refreshes_this_turn.get(turn_key, 0)
        can_refresh = (
            _legal(seaglass.TavernAction.refresh()) is not None
            and refs_used < 14
            and (obs.free_refreshes > 0 or obs.gold >= obs.refresh_cost + obs.minion_cost)
        )
        can_cheap_refresh = (
            _legal(seaglass.TavernAction.refresh()) is not None
            and refs_used < 14
            and (obs.free_refreshes > 0 or obs.gold >= obs.refresh_cost + 2)
        )

        if turn <= 4 or n_board < 7:
            buy_thresh = 10.0 if n_board < 7 else 24.0
        elif turn <= 6:
            buy_thresh = 30.0
        elif build.scam_hunt or build.is_finals:
            buy_thresh = 62.0 if can_refresh else 42.0
        else:
            # Turn 7+ full 7-board: STRICT HUMAN FILTER! Don't buy vanilla trash when we can roll!
            buy_thresh = 48.0 if can_refresh else 38.0

        # Turn 3 (5g) special human tempo play: Sell weak T1 (<= 6 stats) to buy TWO Tier 2 minions!
        if (
            turn == 3
            and obs.tavern_tier == 2
            and obs.gold == 2
            and n_board == 2
            and best_buy_act is None
            and any(not s.is_spell and s.tavern_tier >= 2 for s in shop)
        ):
            wi, _ = self._weakest_board_slot(obs, build)
            if board[wi].tavern_tier == 1 and (board[wi].attack + board[wi].health) <= 7:
                s_act = _legal(seaglass.TavernAction.sell(wi))
                if s_act is not None:
                    return s_act

        if best_buy_act is not None and best_buy_sc >= buy_thresh:
            return best_buy_act

        # In Finals / Scam-Hunt: if we have a dead scaler on board & gold == refresh_cost + minion_cost - 1, SELL dead scaler to roll!
        if (
            (build.is_finals or build.scam_hunt)
            and turn >= 10
            and n_board == 7
            and not can_refresh
            and obs.gold >= max(1, obs.refresh_cost + obs.minion_cost - 1)
        ):
            wi, wv = self._weakest_board_slot(obs, build)
            if wv < 60.0 and board[wi].name in FINALS_DEAD_SCALERS:
                s_act = _legal(seaglass.TavernAction.sell(wi))
                if s_act is not None:
                    return s_act

        # Otherwise AGGRESSIVELY REFRESH to hunt our exponential engine / triples / scam!
        if can_refresh or (can_cheap_refresh and turn >= 7 and best_buy_sc < buy_thresh):
            ref = _legal(seaglass.TavernAction.refresh())
            if ref is not None:
                self._refreshes_this_turn[turn_key] = refs_used + 1
                return ref

        # Spend leftover gold on a decent spell if we can't afford a minion/roll
        if best_buy_act is not None and best_shop_card is not None and best_shop_card.is_spell and best_buy_sc >= 34.0:
            return best_buy_act

        # 10. Smart Freeze (ONLY freeze real Triples, Core Engine Lynchpins >= 74, or Finals Scam — NEVER random T1 junk!)
        if not obs.is_frozen and shop and turn >= 2:
            best_fz = max(
                (self._score_candidate_card(s, obs, build, owned_counts, from_discover=False) for s in shop),
                default=0.0,
            )
            has_triple_in_shop = any(
                not s.is_spell and not s.is_golden and owned_counts.get(s.card_id, 0) >= 2
                for s in shop
            )
            should_freeze = (
                (has_triple_in_shop and (turn >= 3 or best_fz >= 60.0))
                or (best_fz >= 76.0 and turn >= 5)
            )
            if should_freeze:
                fz = _legal(seaglass.TavernAction.toggle_freeze())
                if fz is not None:
                    return fz

        # Free leftover refresh if not freezing and dead gold >= refresh_cost + 2 (so next turn starts clean!)
        if not obs.is_frozen and turn >= 3 and obs.free_refreshes > 0 and refs_used < 14:
            ref = _legal(seaglass.TavernAction.refresh())
            if ref is not None:
                self._refreshes_this_turn[turn_key] = refs_used + 1
                return ref

        # 11. End-of-Turn Exact Combat Positioning!
        pos_key = (id(lobby), turn)
        if self._positioned_turn.get(player) != pos_key and n_board >= 2:
            self._positioned_turn[player] = pos_key
            perm = self._compute_best_perm(lobby, player, obs, ctx)
            repo_acts = self._perm_to_actions(perm)
            if repo_acts:
                first = _legal(repo_acts[0])
                if first is not None:
                    # Apply one reposition per step? Keep queued? Wait: simpler to reset flag only when done!
                    # Actually, let's store remaining repo steps in a tiny queue!
                    self._repo_queue = getattr(self, "_repo_queue", {})
                    self._repo_queue[player] = repo_acts[1:]
                    return first

        # Drain any queued positioning steps
        rq = getattr(self, "_repo_queue", {}).get(player)
        if rq:
            nxt = rq.pop(0)
            if lobby.is_legal(player, nxt):
                return nxt
            rq.clear()

        return _legal(seaglass.TavernAction.end_turn()) or legal[0]
