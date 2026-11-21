"""Declarative Patch 37.0.2 Composition Registry (`ALL_COMPS`).

Every major Solo composition is encoded as a `CompSpec` with:
  - Lobby tribe prerequisites & bonus tribes
  - Lynchpins (build-around cards that trigger commitment)
  - Scalers & enablers (cards that drive the comp's engine)
  - Multipliers (cross-tribe / tribal force multipliers)
  - Support / fodder / tempo bridges
  - Key Tavern & token spells
  - Comp-defining Trinkets
  - Target rolling tier, late-game cap ceiling, flex/cycle slot target
  - Tavern sequencing invariant tags & positioning style tags
"""

from __future__ import annotations

from dataclasses import dataclass, field
from functools import lru_cache
from typing import Dict, FrozenSet, Mapping, Tuple

import seaglass


@dataclass(frozen=True)
class CompSpec:
    comp_id: str
    name: str
    # Every tribe listed here MUST be present in `ctx.tribes` (empty = available in any lobby).
    required_tribes: Tuple[str, ...]
    # Optional tribes that significantly boost this comp when present.
    bonus_tribes: Tuple[str, ...] = ()
    # Cards whose ownership signals this build is online (card_name -> weight).
    lynchpins: Mapping[str, float] = field(default_factory=dict)
    # Primary scaling / engine cards.
    scalers: Mapping[str, float] = field(default_factory=dict)
    # Force multipliers (`Brann`, `Titus`, `Drakkari`, `Balinda`, `Beatboxer`, etc.).
    multipliers: Mapping[str, float] = field(default_factory=dict)
    # Tempo bridges, generators, and tribal support.
    support: Mapping[str, float] = field(default_factory=dict)
    # Key Tavern / token spells this comp values above baseline.
    spells: Mapping[str, float] = field(default_factory=dict)
    # Trinkets that strongly enable or reward this comp.
    trinkets: Mapping[str, float] = field(default_factory=dict)
    # Engine `player_counters` names that indicate active investment in this comp (name -> scale).
    counter_signals: Mapping[str, float] = field(default_factory=dict)
    # Primary Tavern Tier to roll/stabilize on before pushing to cap tier.
    home_tier: int = 5
    # Tier where the comp's endgame cap pieces live.
    cap_tier: int = 6
    # Late-game lobby-winning ceiling (0..100).
    cap_ceiling: float = 80.0
    # Desired open flex/cycle slots (0 for pure combat boards, 1 for APM/BC/cycle comps).
    flex_slots: int = 0
    # Whether this comp actively buys Tavern spells.
    wants_spells: bool = False
    # Comp-specific Tavern sequencing & positioning behavior tags.
    rules: FrozenSet[str] = frozenset()


ALL_COMPS: Tuple[CompSpec, ...] = (
    # =========================================================================
    # ABERRATIONS
    # =========================================================================
    CompSpec(
        comp_id="aberration_discard",
        name="Discard Aberrations",
        required_tribes=("aberration",),
        bonus_tribes=("quilboar",),
        lynchpins={
            "Mindbender Ghur'sha": 15.0,
            "Harbinger Aph'lass": 14.0,
            "Cutthroat K'Thir": 12.0,
            "Parasitic Fleshling": 11.5,
            "Mysterious K'Thir": 10.5,
        },
        scalers={
            "Mindbending Recruiter": 9.5,
            "Nightmare Corroder": 9.0,
            "N'raqi Frostcaller": 10.5,
            "The Shadow of Doubt": 13.5,
            "De-volition-ist": 9.5,
            "Mangled Bandit": 9.0,
        },
        multipliers={
            "Drakkari Enchanter": 11.0,
            "Sha of Fear": 18.0,
        },
        support={
            "Brain Rotter": 6.5,
            "Abyssal Envoy": 7.5,
            "Fetid Corroder": 6.5,
            "Wandering Willbreaker": 7.0,
            "Faceless Operative": 8.0,
            "Drifting Sacrifice": 5.5,
            "Underrot Spawn": 4.5,
            "Unwilling Slacker": 5.5,
            "Joyous": 4.0,
        },
        spells={
            "Sludge Corrosion": 10.0,
            "Energizing Chamber": 6.5,
            "Tavern Dish Banana": 3.5,
        },
        trinkets={
            "Willbreaker Sticker": 14.0,
            "Mindslasher Portrait": 15.0,
            "Sludge Portrait": 10.0,
        },
        counter_signals={
            "cards_discarded": 1.4,
            "deity_attack": 0.25,
            "deity_health": 0.25,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=86.0,
        flex_slots=1,
        wants_spells=True,
        rules=frozenset({"discard_payoffs_first", "fleshling_leftmost", "deity_4_sacrifices"}),
    ),
    CompSpec(
        comp_id="aberration_deity_spell",
        name="Spell/Hand Deity Aberrations",
        required_tribes=("aberration",),
        bonus_tribes=("mech", "demon", "dragon"),
        lynchpins={
            "The Shadow of Doubt": 16.0,
            "Faceless Converter": 13.5,
            "Vicious Mindslasher": 10.0,
            "Dark Puppeteer": 12.5,
        },
        scalers={
            "N'raqi Frostcaller": 11.5,
            "N'raqi Sapper": 9.0,
            "De-volition-ist": 10.0,
            "Felfire Conjurer": 11.0,
            "Gearfin": 9.5,
            "Fruit Vendor": 7.5,
        },
        multipliers={
            "Titus Rivendare": 12.5,
            "Brann Bronzebeard": 10.0,
            "Drakkari Enchanter": 10.0,
            "Sha of Fear": 20.0,
        },
        support={
            "Drifting Sacrifice": 6.5,
            "Unwilling Slacker": 6.0,
            "Underrot Spawn": 4.5,
            "Wandering Willbreaker": 6.5,
            "Joyous": 4.0,
            "Abyssal Envoy": 6.5,
        },
        spells={
            "Energizing Chamber": 8.0,
            "Weapons Forge": 7.5,
            "Tavern Dish Banana": 4.0,
            "Shiny Ring": 5.0,
        },
        trinkets={
            "Mindslasher Portrait": 18.0,
        },
        counter_signals={
            "tavern_spells_cast": 0.8,
            "deity_attack": 0.30,
            "deity_health": 0.30,
            "spell_bonus_atk": 1.2,
            "spell_bonus_hp": 1.2,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=87.0,
        flex_slots=1,
        wants_spells=True,
        rules=frozenset({"deity_4_sacrifices", "converter_dies_early"}),
    ),
    # =========================================================================
    # BEASTS
    # =========================================================================
    CompSpec(
        comp_id="beast_beetles",
        name="Beetle Beasts",
        required_tribes=("beast",),
        lynchpins={
            "Turquoise Skitterer": 15.0,
            "Ravaging Scorpid": 16.0,
            "Forest Rover": 9.5,
        },
        scalers={
            "Deathstrider": 15.0,
            "Banana Slamma": 10.5,
            "Lurking Leviathan": 10.0,
            "Sprightly Scarab": 8.5,
        },
        multipliers={
            "Titus Rivendare": 15.0,
            "Brann Bronzebeard": 10.0,
            "Stalwart Kodo": 16.0,
        },
        support={
            "Buzzing Vermin": 5.0,
            "Wolf Pup": 6.0,
            "Hoarding Hyena": 6.5,
            "Headhunter Gryphon": 6.5,
            "Humming Bird": 4.5,
        },
        spells={
            "Boon of Beetles": 12.0,
        },
        trinkets={
            "Rivendare Portrait": 14.0,
            "Slamma Sticker": 13.0,
        },
        counter_signals={
            "deathrattles_triggered": 0.6,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=88.0,
        flex_slots=0,
        rules=frozenset({"deathstrider_leftmost_dr", "leave_summon_space", "protect_backline"}),
    ),
    CompSpec(
        comp_id="beast_goldrinn",
        name="Goldrinn / Slamma Beasts",
        required_tribes=("beast",),
        lynchpins={
            "Goldrinn, the Great Wolf": 16.0,
            "Banana Slamma": 14.5,
            "Lurking Leviathan": 13.0,
            "Sewer Lord": 12.0,
        },
        scalers={
            "Deathstrider": 14.5,
            "Ghastcoiler": 10.0,
            "Sprightly Scarab": 9.0,
            "Cage Gnawer": 8.0,
        },
        multipliers={
            "Titus Rivendare": 16.0,
            "Stalwart Kodo": 20.0,
        },
        support={
            "Hoarding Hyena": 7.0,
            "Wolf Pup": 6.5,
            "Tasty Lobster": 5.5,
            "Humming Bird": 5.0,
            "Buzzing Vermin": 4.5,
            "Flittering Bat": 4.0,
        },
        spells={
            "Boon of Beetles": 7.0,
            "Fortify": 4.5,
            "Defender's Rites": 5.0,
        },
        trinkets={
            "Slamma Sticker": 18.0,
            "Rivendare Portrait": 16.0,
            "Titus Portrait": 16.0,
        },
        counter_signals={
            "deathrattles_triggered": 0.5,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=91.0,
        flex_slots=0,
        rules=frozenset({"goldrinn_first_die", "deathstrider_leftmost_dr", "protect_backline", "leave_summon_space"}),
    ),
    CompSpec(
        comp_id="beast_rally_fishbait",
        name="Rally / Fishbait Beasts",
        required_tribes=("beast",),
        bonus_tribes=("dragon",),
        lynchpins={
            "Cage Gnawer": 12.0,
            "Deathstrider": 14.5,
            "Hoarding Hyena": 11.5,
        },
        scalers={
            "Wolf Pup": 9.0,
            "Headhunter Gryphon": 9.0,
            "Tasty Lobster": 8.0,
            "Snarky Shark": 8.5,
            "Lurking Lionfish": 8.0,
            "Sky-hatch Runaway": 10.0,
            "Sprightly Scarab": 8.5,
        },
        multipliers={
            "Titus Rivendare": 12.0,
            "Goldrinn, the Great Wolf": 12.5,
            "Stalwart Kodo": 16.0,
        },
        support={
            "Banana Slamma": 9.5,
            "Lurking Leviathan": 9.5,
            "Treasure Parrot": 6.5,
        },
        spells={},
        trinkets={
            "Slamma Sticker": 12.0,
        },
        home_tier=4,
        cap_tier=6,
        cap_ceiling=83.0,
        flex_slots=0,
        rules=frozenset({"fishbait_rally_leftmost", "deathstrider_leftmost_dr"}),
    ),
    # =========================================================================
    # DEMONS
    # =========================================================================
    CompSpec(
        comp_id="demon_eat_tavern",
        name="Eat-the-Tavern Demons",
        required_tribes=("demon",),
        bonus_tribes=("elemental", "quilboar"),
        lynchpins={
            "Insatiable Ur'zul": 14.5,
            "Flaming Enforcer": 14.0,
            "Soulkeeping Jailer": 12.0,
            "Felboar": 13.0,
            "Deft Deserter": 12.5,
        },
        scalers={
            "Devilish Distractor": 12.5,
            "Sacrificial Wrathguard": 10.5,
            "Twisted Wrathguard": 11.5,
            "Imp-lusionist": 8.5,
            "Trapped Clapper": 7.5,
            "Felfire Conjurer": 9.5,
            "En-Djinn Blazer": 11.0,
            "Waveling": 8.5,
            "Razorfen Flapper": 9.5,
        },
        multipliers={
            "Champion of Sargeras": 22.0,
            "Drakkari Enchanter": 13.0,
            "Titus Rivendare": 10.5,
            "Brann Bronzebeard": 10.0,
        },
        support={
            "Malchezaar, Prince of Dance": 7.5,
            "Soul Rewinder": 7.0,
            "Imposing Percussionist": 7.5,
            "Mind Muck": 5.5,
            "Laboratory Assistant": 4.5,
            "Ominous Seer": 5.0,
        },
        spells={
            "Staff of Enrichment": 10.5,
            "Eonar's Favor": 9.5,
            "Corrupted Cupcakes": 11.5,
            "Methodical Madness": 10.0,
            "Blood Gem Barrage": 10.5,
            "Easterly Winds": 9.5,
            "Them Apples": 4.5,
        },
        trinkets={
            "Flaming Portrait": 15.0,
            "Dalaran Cheese Wheel": 12.0,
        },
        counter_signals={
            "tavern_all_atk": 0.9,
            "tavern_all_hp": 0.9,
            "demon_tavern_atk": 0.8,
            "demon_tavern_hp": 0.8,
            "refresh_buffs": 1.2,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=90.0,
        flex_slots=1,
        wants_spells=True,
        rules=frozenset({"buff_shop_before_eat", "leave_biggest_shop_for_enforcer"}),
    ),
    CompSpec(
        comp_id="demon_self_damage",
        name="Self-Damage Demons",
        required_tribes=("demon",),
        lynchpins={
            "Tichondrius": 15.0,
            "Eredar Escapist": 14.5,
            "Soul Rewinder": 12.0,
            "Ashen Corruptor": 11.0,
        },
        scalers={
            "Malchezaar, Prince of Dance": 11.0,
            "Imposing Percussionist": 11.0,
            "Wrath Weaver": 8.5,
            "Devout Hellcaller": 9.0,
            "Insatiable Ur'zul": 11.5,
        },
        multipliers={
            "Brann Bronzebeard": 14.0,
            "Champion of Sargeras": 18.0,
        },
        support={
            "Deft Deserter": 9.0,
            "Soulkeeping Jailer": 8.5,
            "Flaming Enforcer": 9.5,
            "Mind Muck": 5.0,
            "Ominous Seer": 4.5,
        },
        spells={
            "Hasty Excavation": 9.5,
            "Corrupted Cupcakes": 10.5,
            "Staff of Enrichment": 7.0,
        },
        trinkets={
            "Rewinder Portrait": 18.0,
        },
        home_tier=4,
        cap_tier=6,
        cap_ceiling=84.0,
        flex_slots=1,
        rules=frozenset({"protect_rewinder", "buff_shop_before_eat"}),
    ),
    # =========================================================================
    # DRAGONS
    # =========================================================================
    CompSpec(
        comp_id="dragon_poet_combat",
        name="Persistent Poet Combat Dragons",
        required_tribes=("dragon",),
        lynchpins={
            "Persistent Poet": 16.0,
            "Hopebringer": 14.5,
            "Heroic Broodmother": 15.0,
            "Crimson Vindicator": 15.5,
            "Tarecgosa": 10.0,
        },
        scalers={
            "Runic Arcanist": 12.0,
            "Sky-hatch Runaway": 11.0,
            "Amber Guardian": 9.5,
            "Nadina the Red": 13.0,
            "Blue Whelp": 8.0,
            "Roaring Recruiter": 7.5,
            "Bronze Timewalker": 8.0,
        },
        multipliers={
            "Obsidian Ravager": 20.0,
            "Felfire Conjurer": 10.5,
            "Humon'gozz": 9.0,
        },
        support={
            "Bronze Warden": 6.5,
            "Draconic Warden": 8.5,
            "Hired Mount": 6.5,
            "Electric Synthesizer": 5.5,
            "Firescale Hoarder": 8.0,
            "Scarlet Survivor": 5.0,
            "Glim Guardian": 4.5,
        },
        spells={
            "Mighty Dragonbreath": 10.0,
            "Shiny Ring": 7.0,
            "Brood of Nozdormu": 8.5,
        },
        trinkets={
            "Dragonwing Glider": 16.0,
            "Wax Lance": 14.0,
        },
        counter_signals={
            "spell_bonus_atk": 0.9,
            "spell_bonus_hp": 0.9,
        },
        home_tier=4,
        cap_tier=6,
        cap_ceiling=92.0,
        flex_slots=0,
        rules=frozenset({"poet_adjacent_dragons", "broodmother_early", "nadina_early_taunt"}),
    ),
    CompSpec(
        comp_id="dragon_kalecgos",
        name="Kalecgos Battlecry Dragons",
        required_tribes=("dragon",),
        bonus_tribes=("murloc",),
        lynchpins={
            "Kalecgos, Arcane Aspect": 16.5,
            "Brann Bronzebeard": 16.0,
        },
        scalers={
            "Draconic Warden": 10.0,
            "Firescale Hoarder": 9.5,
            "Hired Mount": 8.0,
            "Bronze Timewalker": 8.0,
            "Nadina the Red": 13.0,
            "Heroic Broodmother": 11.5,
            "Crimson Vindicator": 12.0,
        },
        multipliers={
            "Young Murk-Eye": 14.0,
            "Kelp Keeper": 10.0,
            "Drakkari Enchanter": 10.5,
            "Obsidian Ravager": 18.0,
        },
        support={
            "Persistent Poet": 9.5,
            "Amber Guardian": 8.0,
            "Bronze Warden": 6.5,
            "Electric Synthesizer": 6.0,
            "Rodeo Performer": 8.5,
            "Primalfin Lookout": 9.5,
        },
        spells={
            "Hired Headhunter": 8.0,
            "Mighty Dragonbreath": 8.0,
        },
        counter_signals={
            "battlecries_triggered": 0.7,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=86.0,
        flex_slots=1,
        rules=frozenset({"keep_cycle_slot", "nadina_early_taunt"}),
    ),
    # =========================================================================
    # ELEMENTALS
    # =========================================================================
    CompSpec(
        comp_id="elemental_shop_eat",
        name="Shop-Buff / Eat Elementals",
        required_tribes=("elemental",),
        bonus_tribes=("demon",),
        lynchpins={
            "Living Azerite": 15.5,
            "En-Djinn Blazer": 13.5,
            "Unbound Tempest": 14.5,
            "Air Revenant": 12.5,
            "Flaming Enforcer": 13.5,
            "Arcane Behemoth": 22.0,
        },
        scalers={
            "Waveling": 10.5,
            "Living Prison": 12.0,
            "Refreshing Anomaly": 9.5,
            "Leyline Surfacer": 9.5,
            "Tavern Tempest": 8.5,
            "Elemental of Surprise": 13.5,
            "Ichoron the Protector": 9.5,
        },
        multipliers={
            "Stone Age Slab": 26.0,
            "Brann Bronzebeard": 14.5,
            "Drakkari Enchanter": 11.5,
            "Titus Rivendare": 10.0,
            "Arcane Behemoth": 20.0,
        },
        support={
            "Wildfire Elemental": 8.5,
            "Crackling Cyclone": 7.5,
            "Sellemental": 6.0,
            "Dune Dweller": 4.5,
            "Deft Deserter": 9.5,
        },
        spells={
            "Easterly Winds": 11.5,
            "Arcane Absorption": 10.0,
            "Staff of Enrichment": 9.5,
            "Eonar's Favor": 9.0,
            "Leaf Through the Pages": 6.0,
            "Tavern Coin": 4.0,
        },
        trinkets={
            "Azerite Portrait": 18.0,
            "Behemoth Portrait": 22.0,
            "Nomi Sticker": 15.0,
            "Pocket Cyclone": 13.0,
        },
        counter_signals={
            "tavern_elemental_atk": 1.1,
            "tavern_elemental_hp": 1.1,
            "elemental_tavern_atk": 1.0,
            "elemental_tavern_hp": 1.0,
            "refresh_buffs": 1.3,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=94.0,
        flex_slots=1,
        wants_spells=True,
        rules=frozenset({"buff_shop_before_eat", "living_prison_before_buy", "slab_best_buy_first", "leave_biggest_shop_for_enforcer"}),
    ),
    CompSpec(
        comp_id="elemental_cycle_ballers",
        name="Cycle / Ballers / Ultraviolet Elementals",
        required_tribes=("elemental",),
        lynchpins={
            "Ultraviolet Ascendant": 16.5,
            "Air Baller": 11.5,
            "Elemental of Surprise": 13.0,
            "Arcane Behemoth": 22.0,
            "Stone Age Slab": 24.0,
        },
        scalers={
            "Fire Baller": 8.0,
            "Snow Baller": 8.0,
            "Tavern Tempest": 9.5,
            "Refreshing Anomaly": 9.0,
            "Ichoron the Protector": 11.0,
            "Firelands Fugitive": 9.5,
            "Unbound Tempest": 12.0,
        },
        multipliers={
            "Stone Age Slab": 24.0,
            "Brann Bronzebeard": 14.0,
            "Arcane Behemoth": 20.0,
        },
        support={
            "Sellemental": 7.5,
            "Wildfire Elemental": 9.0,
            "Crackling Cyclone": 8.5,
            "Dune Dweller": 5.0,
            "Living Prison": 9.0,
        },
        spells={
            "Temperature Shift": 11.5,
            "Conflagration": 8.5,
            "Natural Blessing": 6.5,
            "Lost Staff of Hamuul": 7.5,
        },
        trinkets={
            "Behemoth Portrait": 22.0,
            "Pocket Cyclone": 14.0,
            "Upstart Embers": 14.0,
        },
        counter_signals={
            "elementals_played": 1.1,
        },
        home_tier=4,
        cap_tier=6,
        cap_ceiling=90.0,
        flex_slots=1,
        rules=frozenset({"keep_cycle_slot", "slab_best_buy_first", "cleave_front"}),
    ),
    # =========================================================================
    # MECHS
    # =========================================================================
    CompSpec(
        comp_id="mech_volumizer_magnetic",
        name="Volumizer / Magnetic Mechs",
        required_tribes=("mech",),
        lynchpins={
            "Resourceful Robot": 17.0,
            "Auto Reveille": 15.5,
            "Utility Drone": 16.5,
            "Polarizing Beatboxer": 22.0,
        },
        scalers={
            "Drone Duplicator": 11.5,
            "Auto Accelerator": 9.5,
            "Conveyor Construct": 9.0,
            "Spark Snapper": 12.5,
            "Accord-o-Tron": 10.0,
            "Red Volumizer": 8.5,
            "Blue Volumizer": 8.5,
            "Green Volumizer": 8.5,
        },
        multipliers={
            "Drakkari Enchanter": 16.5,
            "Brann Bronzebeard": 11.5,
            "Dr. Boom's Monster": 18.0,
        },
        support={
            "Annoy-o-Module": 8.5,
            "Mechagnome Interpreter": 7.5,
            "Enchanted Sentinel": 7.0,
            "Relentless Deflector": 7.5,
            "Lullabot": 5.5,
            "Cord Puller": 4.5,
            "Prosthetic Hand": 7.0,
        },
        spells={
            "Repair Job": 5.0,
            "Lost Staff of Hamuul": 7.0,
        },
        trinkets={
            "Beatboxer Portrait": 24.0,
            "Boom's Monster Portrait": 22.0,
            "Accord-o-Tron Portrait": 17.0,
            "Volumizer Portrait": 17.0,
            "Inductive Gyroblade": 16.0,
            "Portable Factory": 13.0,
        },
        counter_signals={
            "magnetizations": 1.6,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=96.0,
        flex_slots=0,
        rules=frozenset({"beatboxer_no_direct_mag", "duplicator_activate_before_mag", "protect_backline"}),
    ),
    CompSpec(
        comp_id="mech_satellite_spell",
        name="Satellite / Spell Mechs",
        required_tribes=("mech",),
        bonus_tribes=("murloc", "quilboar"),
        lynchpins={
            "Glambot": 13.0,
            "Spark Snapper": 14.5,
            "Charging Czarina": 13.5,
            "Utility Drone": 15.0,
        },
        scalers={
            "Gearfin": 11.0,
            "Enchanted Sentinel": 10.0,
            "Rescue Bot": 8.0,
            "Drone Duplicator": 10.0,
            "Resourceful Robot": 13.5,
            "Relentless Deflector": 8.5,
        },
        multipliers={
            "Balinda Stonehearth": 15.0,
            "Drakkari Enchanter": 14.0,
            "Polarizing Beatboxer": 20.0,
            "Dr. Boom's Monster": 16.0,
        },
        support={
            "Annoy-o-Module": 8.0,
            "Accord-o-Tron": 8.5,
            "Fruit Vendor": 7.5,
            "Cord Puller": 4.5,
        },
        spells={
            "Repair Job": 7.5,
            "Tavern Dish Banana": 6.5,
            "Fortify": 6.0,
            "Weapons Forge": 8.5,
            "Pointy Arrow": 6.0,
            "Blood Gem": 6.5,
        },
        trinkets={
            "Czarina Portrait": 17.0,
            "Boom's Monster Portrait": 18.0,
            "Inductive Gyroblade": 14.0,
        },
        counter_signals={
            "magnetizations": 1.3,
            "tavern_spells_cast": 0.7,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=89.0,
        flex_slots=0,
        wants_spells=True,
        rules=frozenset({"beatboxer_no_direct_mag", "spells_on_mechs", "protect_backline"}),
    ),
    CompSpec(
        comp_id="mech_deathrattle_golem",
        name="Falling Sky Golem DR",
        required_tribes=("mech",),
        bonus_tribes=("undead", "beast"),
        lynchpins={
            "Falling Sky Golem": 16.0,
            "Titus Rivendare": 15.0,
        },
        scalers={
            "Conveyor Construct": 9.0,
            "Rescue Bot": 8.0,
            "Prosthetic Hand": 8.5,
            "Annoy-o-Module": 8.0,
            "Relentless Deflector": 8.0,
        },
        multipliers={
            "Stitched Salvager": 18.0,
        },
        support={
            "Cord Puller": 6.0,
            "Cadaver Caretaker": 8.0,
            "Handless Forsaken": 7.5,
            "Sewer Lord": 9.5,
        },
        spells={
            "Contracted Corpse": 8.0,
        },
        trinkets={
            "Sky Golem Portrait": 19.0,
            "Rivendare Portrait": 15.0,
        },
        counter_signals={
            "deathrattles_triggered": 1.2,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=86.0,
        flex_slots=0,
        rules=frozenset({"protect_backline"}),
    ),
    # =========================================================================
    # MURLOCS
    # =========================================================================
    CompSpec(
        comp_id="murloc_handbuff",
        name="Handbuff Murlocs",
        required_tribes=("murloc",),
        bonus_tribes=("quilboar",),
        lynchpins={
            "Choral Mrrrglr": 17.5,
            "Bream Counter": 13.5,
            "Twilight Tidehunter": 13.0,
            "Shamanic Tidecaller": 14.0,
            "Costume Enthusiast": 12.0,
        },
        scalers={
            "Primalfin Lookout": 11.5,
            "Diremuck Forager": 9.0,
            "Gormling Gourmet": 8.5,
            "Murcules": 7.0,
            "Very Hungry Winterfinner": 7.0,
            "Bile Spitter": 12.5,
        },
        multipliers={
            "Futurefin": 21.0,
            "Tide Oracle Morgl": 25.0,
            "Balinda Stonehearth": 15.5,
            "Brann Bronzebeard": 14.0,
            "Drakkari Enchanter": 11.5,
            "Bassgill": 14.0,
        },
        support={
            "Flighty Scout": 7.5,
            "Tad": 6.5,
            "Bubble Gunner": 5.5,
            "Kelp Keeper": 9.5,
            "Sewer Escapee": 10.0,
            "Fruit Vendor": 7.5,
            "Gearfin": 9.0,
        },
        spells={
            "Cloning Conch": 9.0,
            "Tavern Dish Banana": 6.5,
            "Seafood Stew": 7.0,
            "Blood Gem": 6.5,
            "Pointy Arrow": 5.5,
        },
        trinkets={
            "Morgl Portrait": 28.0,
            "Bassgill Portrait": 18.0,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=94.0,
        flex_slots=1,
        wants_spells=True,
        rules=frozenset({"hand_leftmost_bream", "spells_on_tidehunter", "keep_hand_buff_targets"}),
    ),
    CompSpec(
        comp_id="murloc_battlecry_keywords",
        name="Brann / Murk-Eye / Keyword Murlocs",
        required_tribes=("murloc",),
        lynchpins={
            "Young Murk-Eye": 17.5,
            "Brann Bronzebeard": 17.0,
            "Hackerfin": 14.5,
            "Magicfin Mycologist": 15.0,
            "Sewer Escapee": 13.0,
        },
        scalers={
            "Kelp Keeper": 12.5,
            "Primalfin Lookout": 12.5,
            "Gormling Gourmet": 9.5,
            "Bile Spitter": 13.0,
            "Choral Mrrrglr": 12.0,
            "Shoalfin Mystic": 7.5,
        },
        multipliers={
            "Drakkari Enchanter": 15.5,
            "Tide Oracle Morgl": 22.0,
            "Futurefin": 17.0,
        },
        support={
            "Bubble Gunner": 7.0,
            "Tad": 7.0,
            "Costume Enthusiast": 8.5,
            "Annoy-o-Module": 7.5,
            "Prosthetic Hand": 7.0,
            "Rodeo Performer": 9.0,
        },
        spells={
            "Seafood Stew": 9.5,
            "Cloning Conch": 8.5,
            "Azerite Empowerment": 11.0,
            "Menagerie Tableware": 12.0,
            "Hired Headhunter": 8.0,
        },
        trinkets={
            "Bronzebeard Portrait": 20.0,
            "Hackerfin Portrait": 18.0,
            "Morgl Portrait": 24.0,
        },
        counter_signals={
            "battlecries_triggered": 1.0,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=95.0,
        flex_slots=1,
        wants_spells=True,
        rules=frozenset({"murkeye_adjacent_bc", "spread_bonus_keywords", "keep_cycle_slot"}),
    ),
    # =========================================================================
    # PIRATES
    # =========================================================================
    CompSpec(
        comp_id="pirate_apm_bounties",
        name="APM Gold & Bounties Pirates",
        required_tribes=("pirate",),
        lynchpins={
            "Sky Admiral Rogers": 16.5,
            "Proud Privateer": 15.5,
            "Gunpowder Courier": 11.5,
            "Bigwig Bandit": 11.0,
            "Shipwrecked Rascal": 11.5,
        },
        scalers={
            "Blade Collector": 13.5,
            "Lovesick Balladist": 9.5,
            "Enterprising Escapee": 12.0,
            "Clever Castaway": 8.0,
            "Timecap'n Hooktail": 8.5,
            "Doubloon Grifter": 15.5,
            "Azsharan Cutlassier": 7.5,
        },
        multipliers={
            "Brann Bronzebeard": 13.0,
            "Titus Rivendare": 9.5,
            "Captain Sanders": 18.0,
            "Hooktusk, Master Marauder": 13.0,
        },
        support={
            "Southsea Busker": 6.5,
            "Bilgewater Breakout": 5.5,
            "Aureate Laureate": 6.5,
            "Elite Navigator": 10.5,
            "Silent Deliverer": 10.0,
        },
        spells={
            "Wealthy Bounty": 11.0,
            "Friendly Bounty": 10.0,
            "Hostile Bounty": 8.5,
            "Healthy Bounty": 8.5,
            "Selfish Bounty": 9.0,
            "Strike Oil": 7.5,
            "Careful Investment": 7.0,
        },
        trinkets={
            "Grifter Portrait": 15.0,
            "Booty Bay Brew": 16.0,
            "Miniature Ship": 14.0,
        },
        counter_signals={
            "gold_spent_this_turn": 0.4,
            "base_max_gold_bonus": 1.5,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=91.0,
        flex_slots=1,
        wants_spells=True,
        rules=frozenset({"keep_cycle_slot", "cleave_front", "balladist_late_in_turn", "selfish_bounty_leftmost"}),
    ),
    CompSpec(
        comp_id="pirate_golden_lockbox",
        name="Golden / Lockbox Pirates",
        required_tribes=("pirate",),
        lynchpins={
            "Hooktusk, Master Marauder": 15.5,
            "Enterprising Escapee": 14.0,
            "Ship Master Eudora": 14.0,
            "Elite Navigator": 13.0,
            "Silent Deliverer": 13.5,
        },
        scalers={
            "Maritime Extortionist": 11.0,
            "Greedy Conniver": 11.5,
            "Locked-up Mutineer": 8.0,
            "Treasure Parrot": 8.0,
            "Blade Collector": 12.5,
            "Surfing Sylvar": 7.0,
        },
        multipliers={
            "Captain Sanders": 20.0,
            "Brann Bronzebeard": 13.5,
            "Titus Rivendare": 13.0,
        },
        support={
            "Sky Admiral Rogers": 12.0,
            "Proud Privateer": 11.0,
            "Aureate Laureate": 7.5,
            "Clever Castaway": 8.0,
            "Southsea Busker": 5.5,
        },
        spells={
            "Wave of Gold": 10.0,
            "Eyes of the Earth Mother": 11.5,
            "Golden Touch": 9.5,
            "Boundless Potential": 7.5,
            "Planar Telescope": 7.5,
        },
        trinkets={
            "Escapee Portrait": 19.0,
            "Booty Bay Brew": 15.0,
        },
        counter_signals={
            "golden_minions_played": 2.2,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=89.0,
        flex_slots=1,
        rules=frozenset({"gold_conniver_t7", "eudora_early_die", "cleave_front"}),
    ),
    # =========================================================================
    # QUILBOAR
    # =========================================================================
    CompSpec(
        comp_id="quilboar_geomant_gems",
        name="Geomant / Surveyor Blood Gems",
        required_tribes=("quilboar",),
        lynchpins={
            "Victorious Geomant": 19.5,
            "Hot-Air Surveyor": 15.5,
            "Sanguine Champion": 16.0,
            "Sanguine Refiner": 13.5,
            "Geomagus Roogug": 12.0,
        },
        scalers={
            "Gem Rat": 10.0,
            "Mangled Bandit": 9.5,
            "Trench Fighter": 8.5,
            "Fearless Foodie": 8.5,
            "Bonker": 10.5,
            "Razorfen Vineweaver": 10.5,
            "Veteran Brigand": 13.5,
        },
        multipliers={
            "Balinda Stonehearth": 17.0,
            "Brann Bronzebeard": 14.0,
            "Titus Rivendare": 12.5,
            "Drakkari Enchanter": 13.0,
            "Young Murk-Eye": 15.0,
            "Jailbird Juggernaut": 19.0,
        },
        support={
            "Roadboar": 6.0,
            "Crater Miner": 6.0,
            "Sly Infiltrator": 6.5,
            "Prodigious Tusker": 6.5,
            "Razorfen Geomancer": 5.0,
            "Snare Trapper": 8.5,
        },
        spells={
            "Blood Gem": 8.0,
            "Gem Day": 10.5,
            "Gem Confiscation": 9.5,
            "Blood Gem Barrage": 8.0,
        },
        trinkets={
            "Surveyor Portrait": 18.0,
            "Great Boar Sticker": 17.0,
            "Aggem Sticker": 18.0,
        },
        counter_signals={
            "blood_gem_bonus_atk": 1.6,
            "blood_gem_bonus_hp": 1.6,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=96.0,
        flex_slots=0,
        rules=frozenset({"geomant_hand_or_overflow", "gems_on_roogug", "sanguine_champion_early"}),
    ),
    CompSpec(
        comp_id="quilboar_choose_one",
        name="Choose-One Quilboar (Turbo Hogrider)",
        required_tribes=("quilboar",),
        lynchpins={
            "Turbo Hogrider": 16.5,
            "Veteran Brigand": 15.5,
            "Thorned Trailblazer": 12.5,
        },
        scalers={
            "Sanguine Champion": 14.5,
            "Sanguine Refiner": 12.5,
            "Bramble Tunneler": 10.5,
            "Snare Trapper": 10.5,
            "Fearless Foodie": 9.5,
            "Gem Rat": 9.5,
            "Sly Infiltrator": 8.5,
            "Crater Miner": 7.5,
        },
        multipliers={
            "Jailbird Juggernaut": 18.0,
            "Drakkari Enchanter": 10.5,
            "Titus Rivendare": 10.0,
        },
        support={
            "Geomagus Roogug": 10.5,
            "Bonker": 10.0,
            "Victorious Geomant": 14.0,
            "Hot-Air Surveyor": 11.0,
        },
        spells={
            "Fandral's Fortune": 11.0,
            "Gem Day": 10.5,
            "Forest's Bounty": 7.0,
            "Boundless Potential": 8.0,
            "Alliance Flag": 6.5,
            "Time Management": 6.5,
        },
        trinkets={
            "Great Boar Sticker": 16.0,
        },
        counter_signals={
            "blood_gem_bonus_atk": 1.4,
            "blood_gem_bonus_hp": 1.4,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=91.0,
        flex_slots=1,
        rules=frozenset({"trailblazer_best_first", "sanguine_champion_early"}),
    ),
    CompSpec(
        comp_id="quilboar_rally",
        name="Combat Rally Quilboar",
        required_tribes=("quilboar",),
        bonus_tribes=("dragon",),
        lynchpins={
            "Bonker": 13.5,
            "Razorfen Vineweaver": 13.0,
            "Sanguine Refiner": 14.5,
            "Jailbird Juggernaut": 20.0,
        },
        scalers={
            "Sanguine Champion": 14.5,
            "Prodigious Tusker": 8.5,
            "Sky-hatch Runaway": 12.0,
            "Geomagus Roogug": 11.0,
            "Gem Rat": 9.0,
            "Bramble Tunneler": 9.0,
        },
        multipliers={
            "Titus Rivendare": 10.5,
            "Victorious Geomant": 14.0,
        },
        support={
            "Tusked Camper": 5.5,
            "Roadboar": 6.5,
            "Fearless Foodie": 8.0,
            "Veteran Brigand": 12.5,
        },
        spells={
            "Gem Day": 9.5,
            "Gem Confiscation": 9.0,
        },
        counter_signals={
            "blood_gem_bonus_atk": 1.3,
            "blood_gem_bonus_hp": 1.3,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=88.0,
        flex_slots=0,
        rules=frozenset({"refiner_attacks_first", "sanguine_champion_early"}),
    ),
    # =========================================================================
    # UNDEAD
    # =========================================================================
    CompSpec(
        comp_id="undead_reborn_butcher",
        name="Reborn Stats Undead (Snazzy + Butcher)",
        required_tribes=("undead",),
        lynchpins={
            "Snazzy Phantom": 17.5,
            "Barrier Banshee": 15.0,
            "Drustfallen Butcher": 14.5,
            "Dead Bellringer": 12.5,
        },
        scalers={
            "Plaguerunner": 11.5,
            "Forsaken Weaver": 12.5,
            "Eternal Summoner": 12.5,
            "Mummifier": 9.5,
            "Prosthetic Hand": 10.0,
            "Handless Forsaken": 8.5,
            "Lichling Hoarder": 11.5,
        },
        multipliers={
            "Stitched Salvager": 19.0,
            "Titus Rivendare": 13.5,
            "Humon'gozz": 9.5,
        },
        support={
            "Cadaver Caretaker": 8.0,
            "Risen Rider": 6.5,
            "Scarlet Skull": 6.5,
            "Maw Caster": 8.5,
            "Disguised Graverobber": 8.5,
            "Nerubian Deathswarmer": 6.0,
            "Deathly Striker": 10.5,
        },
        spells={
            "Butchering": 14.0,
            "Tomb Turning": 9.5,
        },
        trinkets={
            "Artisanal Urn": 14.0,
            "Eternal Portrait": 13.0,
        },
        counter_signals={
            "undead_bonus_attack": 0.9,
            "undead_bonus_health": 1.2,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=93.0,
        flex_slots=0,
        rules=frozenset({"tavern_reborn_rightmost", "plaguerunner_tavern_destroy", "reborn_left_carry_right"}),
    ),
    CompSpec(
        comp_id="undead_attack_tokens",
        name="Undead Attack & Token Spam",
        required_tribes=("undead",),
        lynchpins={
            "Forsaken Weaver": 15.5,
            "Deathly Striker": 15.0,
            "Drustfallen Butcher": 14.0,
            "Plaguerunner": 12.5,
            "Lichling Hoarder": 13.5,
        },
        scalers={
            "Dead Bellringer": 11.5,
            "Cadaver Caretaker": 10.0,
            "Handless Forsaken": 9.5,
            "Maw Caster": 9.0,
            "Disguised Graverobber": 8.5,
            "Friendly Geist": 8.5,
            "Nerubian Deathswarmer": 7.0,
        },
        multipliers={
            "Titus Rivendare": 14.5,
            "Stitched Salvager": 17.5,
        },
        support={
            "Mummifier": 8.5,
            "Eternal Summoner": 11.5,
            "Barrier Banshee": 11.0,
            "Snazzy Phantom": 12.5,
            "Risen Rider": 6.0,
            "Harmless Bonehead": 5.5,
            "Prosthetic Hand": 8.0,
        },
        spells={
            "Butchering": 13.5,
            "Tomb Turning": 9.0,
        },
        trinkets={
            "Artisanal Urn": 15.0,
            "Ghastly Sticker": 16.0,
        },
        counter_signals={
            "undead_bonus_attack": 1.1,
            "deathrattles_triggered": 0.5,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=88.0,
        flex_slots=0,
        wants_spells=True,
        rules=frozenset({"plaguerunner_tavern_destroy", "tokens_left_avenge_right", "leave_summon_space"}),
    ),
    CompSpec(
        comp_id="undead_eternal_knights",
        name="Eternal Knights Undead",
        required_tribes=("undead",),
        lynchpins={
            "Eternal Summoner": 16.5,
            "Eternal Tycoon": 15.5,
            "Eternal Knight": 11.5,
        },
        scalers={
            "Dead Bellringer": 11.0,
            "Mummifier": 10.0,
            "Prosthetic Hand": 10.0,
            "Lichling Hoarder": 12.0,
            "Maw Caster": 8.5,
            "Disguised Graverobber": 9.0,
        },
        multipliers={
            "Titus Rivendare": 15.5,
            "Stitched Salvager": 18.0,
        },
        support={
            "Cadaver Caretaker": 8.5,
            "Handless Forsaken": 8.0,
            "Barrier Banshee": 11.0,
            "Snazzy Phantom": 11.5,
            "Drustfallen Butcher": 10.0,
        },
        spells={
            "Butchering": 9.0,
            "Tomb Turning": 8.5,
        },
        trinkets={
            "Eternal Portrait": 20.0,
            "Rivendare Portrait": 15.0,
        },
        home_tier=5,
        cap_tier=5,
        cap_ceiling=86.0,
        flex_slots=0,
        rules=frozenset({"summoner_early_die", "leave_summon_space", "protect_backline"}),
    ),
    # =========================================================================
    # CROSS-TRIBE / NEUTRAL / SPELLS / MENAGERIE
    # =========================================================================
    CompSpec(
        comp_id="tavern_spells",
        name="Tavern Spell Power & Spam",
        required_tribes=(),
        bonus_tribes=("demon", "dragon", "aberration", "mech", "murloc"),
        lynchpins={
            "Felfire Conjurer": 15.5,
            "Cataclysmic Harbinger": 15.5,
            "Balinda Stonehearth": 16.0,
            "Dark Puppeteer": 14.0,
            "N'raqi Frostcaller": 13.0,
        },
        scalers={
            "Humon'gozz": 11.5,
            "Rodeo Performer": 11.5,
            "Gearfin": 11.0,
            "Enchanted Sentinel": 10.0,
            "Friendly Geist": 9.5,
            "Shoalfin Mystic": 9.0,
            "Azsharan Cutlassier": 9.0,
            "Blue Whelp": 9.0,
            "Intrepid Botanist": 7.5,
            "Fruit Vendor": 8.5,
            "Runic Arcanist": 11.0,
            "Firescale Hoarder": 9.5,
        },
        multipliers={
            "Drakkari Enchanter": 16.5,
            "Brann Bronzebeard": 13.0,
            "Titus Rivendare": 11.0,
            "Sha of Fear": 22.0,
            "Gatekeeper Amalgam": 16.5,
        },
        support={
            "Felboar": 11.5,
            "Living Azerite": 12.0,
            "Charging Czarina": 11.0,
            "Forsaken Weaver": 11.5,
            "Timecap'n Hooktail": 8.0,
            "Ominous Seer": 6.0,
            "Clever Castaway": 8.0,
            "Magicfin Mycologist": 12.5,
        },
        spells={
            "Azerite Empowerment": 14.0,
            "Menagerie Tableware": 15.0,
            "Mighty Dragonbreath": 11.5,
            "Wave of Gold": 10.5,
            "Shiny Ring": 9.0,
            "Misplaced Tea Set": 9.5,
            "Forest's Bounty": 9.0,
            "Saloon's Finest": 8.5,
            "Time Management": 8.0,
            "Seafood Stew": 8.5,
            "Butchering": 10.5,
        },
        trinkets={
            "Tableware Portrait": 24.0,
            "Book of Medivh": 12.0,
            "Magician's Top Hat": 14.0,
        },
        counter_signals={
            "spell_bonus_atk": 2.2,
            "spell_bonus_hp": 2.2,
            "tavern_spells_cast": 0.8,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=94.0,
        flex_slots=1,
        wants_spells=True,
        rules=frozenset({"harbinger_spell_last", "protect_backline"}),
    ),
    CompSpec(
        comp_id="menagerie_amalgam",
        name="Menagerie (Gatekeeper Amalgam / Tea Set)",
        required_tribes=(),
        lynchpins={
            "Gatekeeper Amalgam": 18.0,
            "The Last One Standing": 26.0,
            "Nightmare Par-tea Guest": 12.5,
        },
        scalers={
            "Balinda Stonehearth": 17.5,
            "Fruit Vendor": 10.0,
            "Gearfin": 10.0,
            "Victorious Geomant": 15.0,
            "Sky-hatch Runaway": 11.0,
            "Dark Paradox": 9.0,
            "Tyrael": 13.5,
        },
        multipliers={
            "Brann Bronzebeard": 13.0,
            "Titus Rivendare": 11.0,
            "Drakkari Enchanter": 10.0,
        },
        support={
            # Premium multi-tribe DS/Cleave carriers
            "Blade Collector": 11.0,
            "Wildfire Elemental": 10.0,
            "Crackling Cyclone": 9.0,
            "Bronze Warden": 8.5,
            "Geomagus Roogug": 10.0,
            "Costume Enthusiast": 9.5,
            "Barrier Banshee": 9.5,
            "Crimson Vindicator": 11.5,
            "Elemental of Surprise": 11.0,
            "Holy Vanguard": 9.0,
        },
        spells={
            "Misplaced Tea Set": 11.5,
            "Menagerie Tableware": 15.0,
            "Tavern Dish Banana": 8.5,
            "Blood Gem": 8.5,
            "Pointy Arrow": 8.0,
            "Fortify": 7.0,
            "Weapons Forge": 10.0,
            "Seafood Stew": 8.0,
        },
        trinkets={
            "Tableware Portrait": 26.0,
            "Aggem Sticker": 17.0,
            "Curator Sticker": 14.0,
        },
        home_tier=5,
        cap_tier=6,
        cap_ceiling=93.0,
        flex_slots=0,
        wants_spells=True,
        rules=frozenset({"spells_on_amalgam", "maximize_tribes", "cleave_front"}),
    ),
)


@dataclass(frozen=True)
class CompRegistry:
    comps: Tuple[CompSpec, ...]
    by_id: Dict[str, CompSpec]
    name_to_id: Dict[str, int]
    id_to_name: Dict[int, str]
    # Pre-resolved card_id -> weight maps per comp_id for fast hot-loop lookups
    lynchpin_ids: Dict[str, Dict[int, float]]
    scaler_ids: Dict[str, Dict[int, float]]
    multiplier_ids: Dict[str, Dict[int, float]]
    support_ids: Dict[str, Dict[int, float]]
    spell_ids: Dict[str, Dict[int, float]]
    trinket_name_weights: Dict[str, Mapping[str, float]]
    counter_idx_signals: Dict[str, Tuple[Tuple[int, float], ...]]
    # Universal scam/tech card IDs
    scam_ids: FrozenSet[int]
    cleave_ids: FrozenSet[int]
    counter_idx_by_name: Dict[str, int]


@lru_cache(maxsize=4)
def get_comp_registry(catalog_name: str = "full") -> CompRegistry:
    name_to_id: Dict[str, int] = {}
    id_to_name: Dict[int, str] = {}

    for tmpl in list(seaglass.catalog("duos")) + list(seaglass.catalog(catalog_name)):
        if tmpl.card_id not in id_to_name:
            id_to_name[tmpl.card_id] = tmpl.name
            name_to_id[tmpl.name] = tmpl.card_id

    # Trinket-exclusive minions (from engine cards::trinket_minion_catalog)
    trinket_exclusives = {
        10801: "Doubloon Grifter",
        10802: "Bassgill",
        10803: "Arcane Behemoth",
        10804: "Dr. Boom's Monster",
        10805: "Flourishing Frostling",
        10806: "Faceless Manipulator",
        10807: "Tide Oracle Morgl",
        10808: "Mishmash",
        10809: "Rusted Reggie",
        10810: "Magtheridon Prime",
        10811: "Baltharak",
        10812: "Shield of the Legion",
    }
    for cid, cname in trinket_exclusives.items():
        id_to_name.setdefault(cid, cname)
        name_to_id.setdefault(cname, cid)

    # Also resolve spells by querying card_tags / known spell names if needed;
    # note `obs.shop[i].name` / `obs.hand[i].name` gives exact names for spells too!
    counter_names = list(seaglass.player_counter_names())
    counter_idx_by_name = {n: i for i, n in enumerate(counter_names)}

    def _resolve(mapping: Mapping[str, float]) -> Dict[int, float]:
        out: Dict[int, float] = {}
        for cname, w in mapping.items():
            cid = name_to_id.get(cname)
            if cid is not None:
                out[cid] = w
        return out

    lynchpin_ids: Dict[str, Dict[int, float]] = {}
    scaler_ids: Dict[str, Dict[int, float]] = {}
    multiplier_ids: Dict[str, Dict[int, float]] = {}
    support_ids: Dict[str, Dict[int, float]] = {}
    spell_ids: Dict[str, Dict[int, float]] = {}
    trinket_name_weights: Dict[str, Mapping[str, float]] = {}
    counter_idx_signals: Dict[str, Tuple[Tuple[int, float], ...]] = {}

    for comp in ALL_COMPS:
        cid = comp.comp_id
        lynchpin_ids[cid] = _resolve(comp.lynchpins)
        scaler_ids[cid] = _resolve(comp.scalers)
        multiplier_ids[cid] = _resolve(comp.multipliers)
        support_ids[cid] = _resolve(comp.support)
        spell_ids[cid] = _resolve(comp.spells)
        trinket_name_weights[cid] = comp.trinkets
        sigs = []
        for cname, scale in comp.counter_signals.items():
            idx = counter_idx_by_name.get(cname)
            if idx is not None:
                sigs.append((idx, scale))
        counter_idx_signals[cid] = tuple(sigs)

    scam_names = (
        "Leeroy the Reckless",
        "Deadly Spore",
        "Bile Spitter",
        "Sin'dorei Straight Shot",
        "Boom-in-a-Box",
        "Heroic Underdog",
        "Tide Oracle Morgl",
    )
    scam_ids = frozenset(name_to_id[n] for n in scam_names if n in name_to_id)

    cleave_names = ("Blade Collector", "Wildfire Elemental", "Obsidian Ravager")
    cleave_ids = frozenset(name_to_id[n] for n in cleave_names if n in name_to_id)

    return CompRegistry(
        comps=ALL_COMPS,
        by_id={c.comp_id: c for c in ALL_COMPS},
        name_to_id=name_to_id,
        id_to_name=id_to_name,
        lynchpin_ids=lynchpin_ids,
        scaler_ids=scaler_ids,
        multiplier_ids=multiplier_ids,
        support_ids=support_ids,
        spell_ids=spell_ids,
        trinket_name_weights=trinket_name_weights,
        counter_idx_signals=counter_idx_signals,
        scam_ids=scam_ids,
        cleave_ids=cleave_ids,
        counter_idx_by_name=counter_idx_by_name,
    )
