"""Engine-Grounded Patch 36.6.3 Knowledge Base for ApexBot."""

from __future__ import annotations

from dataclasses import dataclass
from functools import lru_cache
from typing import Dict, FrozenSet, Set, Tuple

import seaglass

from .catalog_meta import CardMetaDB, get_meta_db


def _engine_slug(name: str) -> str:
    return name.lower().replace(" ", "_").replace("-", "_").replace("'", "_")


# Calibrated compounding engine weights:
#   - ONLY true Tier 5-7 endgame scaling carries have weight >= 12.0 (protecting them from cycle-sell),
#   - Tier 2-4 midgame helpers stay strictly < 12.0 so full boards smoothly transition to Tier 6 carries!
APEX_COMPOUNDING_ENGINES: Dict[str, float] = {
    # Universal Multipliers & S-Tier Tier 5-7 Carry Engines
    "Brann Bronzebeard": 15.0,
    "Drakkari Enchanter": 13.5,
    "Titus Rivendare": 13.5,
    "Tyrael": 18.0,
    "Victorious Geomant": 18.5,
    "Young Murk-Eye": 16.5,
    "Polarizing Beatboxer": 19.0,
    "Stone Age Slab": 24.0,
    "Champion of Sargeras": 22.0,
    "Highkeeper Ra": 17.5,
    "Captain Sanders": 18.0,
    "Stalwart Kodo": 18.5,
    "Stitched Salvager": 16.0,
    "Sha of Fear": 23.0,
    "The Last One Standing": 32.0,
    "Jailbird Juggernaut": 17.0,
    "Obsidian Ravager": 18.0,
    "Futurefin": 16.5,
    # Elementals
    "Living Azerite": 15.0,
    "Ultraviolet Ascendant": 14.0,
    "Unbound Tempest": 11.5,
    "Living Prison": 11.5,
    "Air Revenant": 9.5,
    "Elemental of Surprise": 11.0,
    "Ichoron the Protector": 7.5,
    # Quilboar
    "Sanguine Champion": 15.0,
    "Hot-Air Surveyor": 12.5,
    "Turbo Hogrider": 13.5,
    "Veteran Brigand": 12.0,
    "Sanguine Refiner": 10.5,
    "Geomagus Roogug": 9.5,
    "Razorfen Vineweaver": 8.5,
    "Bonker": 8.0,
    "Gem Rat": 6.5,
    "Thorned Trailblazer": 7.5,
    # Mechs
    "Utility Drone": 15.0,
    "Auto Reveille": 14.0,
    "Resourceful Robot": 18.0,
    "Spark Snapper": 12.5,
    "Charging Czarina": 10.5,
    "Falling Sky Golem": 10.5,
    "Drone Duplicator": 8.5,
    "Glambot": 8.5,
    "Mechagnome Interpreter": 5.5,
    "Accord-o-Tron": 7.0,
    # Pirates
    "Sky Admiral Rogers": 13.5,
    "Proud Privateer": 12.5,
    "Enterprising Escapee": 11.0,
    "Hooktusk, Master Marauder": 11.5,
    "Gunpowder Courier": 7.5,
    "Ship Master Eudora": 9.5,
    "Blade Collector": 9.5,
    # Demons
    "Deft Deserter": 9.5,
    "Insatiable Ur'zul": 12.5,
    "Flaming Enforcer": 12.5,
    "Felboar": 11.5,
    "Tichondrius": 10.5,
    "Felfire Conjurer": 10.5,
    "Twisted Wrathguard": 10.5,
    "Soul Rewinder": 8.5,
    "Ashen Corruptor": 8.5,
    "Malchezaar, Prince of Dance": 7.5,
    "Soulkeeping Jailer": 8.0,
    # Dragons
    "Kalecgos, Arcane Aspect": 14.5,
    "Persistent Poet": 12.5,
    "Crimson Vindicator": 13.0,
    "Heroic Broodmother": 10.5,
    "Hopebringer": 9.5,
    "Nadina the Red": 11.5,
    "Tarecgosa": 6.5,
    "Amber Guardian": 6.5,
    # Murlocs
    "Choral Mrrrglr": 16.0,
    "Magicfin Mycologist": 12.5,
    "Shamanic Tidecaller": 11.5,
    "Bream Counter": 10.5,
    "Hackerfin": 10.0,
    "Bile Spitter": 12.5,
    "Sewer Escapee": 11.0,
    "Twilight Tidehunter": 8.5,
    "Costume Enthusiast": 8.5,
    "Diremuck Forager": 7.5,
    # Beasts
    "Goldrinn, the Great Wolf": 14.5,
    "Banana Slamma": 13.5,
    "Lurking Leviathan": 12.5,
    "Deathstrider": 12.5,
    "Ravaging Scorpid": 12.5,
    "Turquoise Skitterer": 10.5,
    "Cage Gnawer": 7.5,
    # Undead
    "Eternal Summoner": 13.0,
    "Eternal Tycoon": 11.5,
    "Forsaken Weaver": 12.5,
    "Deathly Striker": 12.0,
    "Snazzy Phantom": 10.5,
    "Barrier Banshee": 10.0,
    "Lichling Hoarder": 11.5,
    "Drustfallen Butcher": 9.5,
    "Plaguerunner": 7.5,
    # Aberrations & Neutral Spell/Tech
    "Balinda Stonehearth": 14.5,
    "Gatekeeper Amalgam": 12.5,
    "The Shadow of Doubt": 11.5,
    "Mindbender Ghur'sha": 11.5,
    "Harbinger Aph'lass": 10.5,
    "Dark Puppeteer": 10.0,
    "Cataclysmic Harbinger": 10.0,
    "Mysterious K'Thir": 9.5,
    "Cutthroat K'Thir": 7.5,
    "Nightmare Corroder": 7.5,
    "Parasitic Fleshling": 7.5,
    "Holy Vanguard": 9.0,
    "Leeroy the Reckless": 9.5,
    "Sin'dorei Straight Shot": 7.5,
    # Duos standouts
    "Orc-estra Conductor": 9.5,
    "Loyal Mobster": 12.5,
    "Support System": 11.5,
    "Sandy": 15.0,
    "Selfless Sightseer": 11.5,
    "Man'ari Messenger": 11.0,
    "Transport Reactor": 11.0,
    "Passenger": 7.5,
    "Wanderer Cho": 8.5,
    # Trinket-exclusive carries (`cards::trinket_minion_catalog`, IDs 10801..10812)
    "Doubloon Grifter": 13.5,
    "Bassgill": 11.5,
    "Arcane Behemoth": 18.5,
    "Dr. Boom's Monster": 15.5,
    "Flourishing Frostling": 12.5,
    "Tide Oracle Morgl": 24.0,
    "Mishmash": 11.5,
}

TRINKET_EXCLUSIVE_MINIONS: Dict[int, str] = {
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

# Pure support minions that do nothing without at least 1 other ally of that tribe on board.
REQUIRED_ALLY_SUPPORTS: Dict[str, str] = {
    "Nadina the Red": "dragon",
    "Amber Guardian": "dragon",
    "Persistent Poet": "dragon",
    "Soulkeeping Jailer": "demon",
    "Cage Gnawer": "beast",
    "Ichoron the Protector": "elemental",
    "Gunpowder Courier": "pirate",
}

# Tribe-dependent scaling/combat engines:
#   - Full value (plus per-ally bonus) when >= 1 other ally of that tribe is on board,
#   - Allowed as early/mid pivot (`turn <= 8`) with 0 allies,
#   - Discounted to 0.0 on `turn >= 10` when 0 allies exist so ApexBot NEVER sells active
#     comp units to buy off-tribe carries (`Goldrinn` w/ 0 Beasts, `Kalecgos` w/ 0 Dragons, etc.)!
TRIBAL_SYNERGY_ENGINES: Dict[str, str] = {
    # Beasts
    "Goldrinn, the Great Wolf": "beast",
    "Banana Slamma": "beast",
    "Lurking Leviathan": "beast",
    "Deathstrider": "beast",
    "Ravaging Scorpid": "beast",
    "Turquoise Skitterer": "beast",
    # Dragons
    "Kalecgos, Arcane Aspect": "dragon",
    "Crimson Vindicator": "dragon",
    "Heroic Broodmother": "dragon",
    "Hopebringer": "dragon",
    # Mechs
    "Polarizing Beatboxer": "mech",
    "Utility Drone": "mech",
    "Auto Reveille": "mech",
    "Spark Snapper": "mech",
    "Charging Czarina": "mech",
    "Falling Sky Golem": "mech",
    "Drone Duplicator": "mech",
    # Murlocs
    "Choral Mrrrglr": "murloc",
    "Magicfin Mycologist": "murloc",
    "Shamanic Tidecaller": "murloc",
    "Bream Counter": "murloc",
    "Hackerfin": "murloc",
    "Futurefin": "murloc",
    # Undead
    "Eternal Summoner": "undead",
    "Eternal Tycoon": "undead",
    "Forsaken Weaver": "undead",
    "Deathly Striker": "undead",
    "Lichling Hoarder": "undead",
    "Snazzy Phantom": "undead",
    "Barrier Banshee": "undead",
    # Demons
    "Insatiable Ur'zul": "demon",
    "Tichondrius": "demon",
    "Felfire Conjurer": "demon",
    "Twisted Wrathguard": "demon",
    # Quilboar
    "Hot-Air Surveyor": "quilboar",
    "Turbo Hogrider": "quilboar",
    "Veteran Brigand": "quilboar",
    "Geomagus Roogug": "quilboar",
    # Pirates
    "Proud Privateer": "pirate",
    "Hooktusk, Master Marauder": "pirate",
    "Enterprising Escapee": "pirate",
    # Elementals
    "Ultraviolet Ascendant": "elemental",
    "Unbound Tempest": "elemental",
    "Air Revenant": "elemental",
}


HERO_EQUITY_PRIORS: Dict[str, float] = {
    "Cap'n Hoggarr": 9.5,
    "Millhouse Manastorm": 9.0,
    "Forest Warden Omu": 8.5,
    "Mister Clocksworth": 9.5,
    "Trade Prince Gallywix": 8.5,
    "Forest Lord Cenarius": 8.8,
    "Snake Eyes": 8.2,
    "Nozdormu": 7.8,
    "Xyrella": 7.5,
    "Kurtrus Ashfallen": 7.5,
    "Kael'thas Sunstrider": 7.2,
    "Heistbaron Togwaggle": 7.8,
    "Farseer Nobundo": 7.8,
    "Master Nguyen": 7.5,
    "Sir Finley Mrrgglton": 7.2,
    "Shudderwock": 7.8,
    "Galakrond": 7.5,
    "Reno Jackson": 7.2,
    "Skycap'n Kragg": 6.8,
    "Galewing": 7.5,
    "Teron Gorefiend": 7.2,
    "The Lich King": 6.8,
    "Al'Akir": 7.5,
    "Greybough": 6.5,
    "Deathwing": 6.8,
    "Overlord Saurfang": 6.5,
    "Varden Dawngrasp": 7.5,
    "Tae'thelan Bloodwatcher": 7.2,
    "Doctor Holli'dae": 6.8,
    "Time Twister Chromie": 6.8,
    "Thorim, Stormlord": 8.0,
    "Tras'tath, Soul Parasite": 7.5,
    "Chenvaala": 6.8,
    "Death Speaker Blackthorn": 6.8,
    "Dinotamer Brann": 7.5,
    "Silas Darkmoon": 6.5,
    "Tickatus": 6.5,
    "Rock Master Voone": 7.0,
    "Zephrys, the Great": 7.0,
    "Yogg-Saron, Hope's End": 6.5,
    "Enhance-o Mechano": 6.2,
    "Arch-Villain Rafaam": 6.2,
    "Scabbs Cutterbutter": 6.8,
    "Tess Greymane": 6.5,
    "Mutanus the Devourer": 6.5,
    "The Curator": 6.2,
    "N'Zoth": 6.2,
    "Loh, the Living Legend": 7.0,
    "Cho": 8.5,
    "Gall": 8.5,
    "Madam Goya": 8.0,
    "Flobbidinous Floop": 8.0,
    "The Nameless One": 7.5,
    "A. F. Kay": -6.0,
    "Ambassador Faelin": -3.5,
    "Lich Baz'hial": -3.5,
    "Mr. Bigglesworth": -4.5,
    "Alexstrasza": -3.0,
    "Lord Barov": -2.5,
    "Murloc Holmes": -2.0,
}


TRINKET_EQUITY_PRIORS: Dict[str, float] = {
    # S+ Scaling, Tier-7 & Exponential Trinkets
    "Weighted Gauntlet": 85.0,
    "Tableware Portrait": 35.0,
    "Guiding Candle": 32.0,
    "Wax Lance": 31.0,
    "Ghastly Sticker": 31.0,
    "Morgl Portrait": 30.0,
    "Pagle's Fishing Rod": 26.0,
    "Innkeeper's Hearth": 24.0,
    "Trip Vouchers": 23.0,
    "Behemoth Portrait": 22.0,
    "Boom's Monster Portrait": 21.0,
    "Bob's Tip Jar": 21.5,
    "Ornate Clock": 21.0,
    "Souvenir Stand": 20.5,
    "Bronzebeard Portrait": 20.5,
    "Beatboxer Portrait": 20.5,
    "Bartend-o-Tron's Oilcan": 19.5,
    "Azeroth Model Globe": 19.5,
    "Sinister Invitation": 19.5,
    "Escapee Portrait": 19.0,
    "Azerite Portrait": 19.0,
    "Aggem Sticker": 18.5,
    "Gold-plated Compass": 18.5,
    "Goblin Wallet": 18.5,
    "Hackerfin Portrait": 18.0,
    "Czarina Portrait": 18.0,
    "Booty Bay Brew": 17.5,
    "Bassgill Portrait": 17.5,
    "Flaming Portrait": 17.5,
    "Rivendare Portrait": 17.0,
    "Sky Golem Portrait": 17.0,
    "Great Boar Sticker": 16.5,
    "Surveyor Portrait": 16.5,
    "Nomi Sticker": 16.5,
    "Portable Factory": 16.5,
    "Slamma Sticker": 16.5,
    "Gold Pendant": 16.0,
    "Mecha-Jaraxxus Sticker": 16.0,
    "Manipulator Portrait": 6.5,
    "Accord-o-Tron Portrait": 15.5,
    "Volumizer Portrait": 15.5,
    "Gold Mallet": 15.5,
    "Grifter Portrait": 15.5,
    "Trusty Crowbar": 15.5,
    "Inductive Gyroblade": 15.0,
    "Reinforced Shield": 15.0,
    "Stuffed Coin Purse": 15.0,
    "Upstart Embers": 15.0,
    "Eternal Portrait": 15.0,
    "Sinstone Sticker": 14.5,
    "Innkeeper's Stein": 14.5,
    "Artisanal Urn": 14.0,
    "Book of Medivh": 14.0,
    "Dalaran Cheese Wheel": 14.0,
    "Pocket Cyclone": 14.0,
    "Reinvigorating Light": 14.0,
    "Rendle Sticker": 13.5,
    "Glowing Gauntlet": 13.5,
    "Explorer's Binoculars": 13.5,
    "Transcribing Typewriter": 13.5,
    "Worn Treasure Map": 13.5,
    "Curator Sticker": 14.5,
    "Cho'gall Sticker": 17.5,
    "Bottle Sticker": 13.5,
    "Common Thread": 15.5,
    "Summoning Sphere": 15.0,
    "Orc-estra Portrait": 13.5,
    # Conditional / Trap / Variance / Anti-Scaling Trinkets
    "Maldraxxus Dagger": 2.0,
    "Mysterious Orb": -22.0,  # Forfeits Turn 9 Greater Trinket!
    "Safety Patch": 1.5,      # Zero scaling unless at lethal HP
    "Corrupted Tome": -8.0,   # Loses Tier+1 Triple Discover consistency
    "Orb of the Unknown": -4.0,
    "Mystery Cube": -3.0,
    "Yogg-Tastic Pastry": -4.0,
    "Shrine of Evolution": -16.0,
    "Moveout Box": -14.0,
    "Soul Fermenter": -6.0,
}


DARK_GIFT_PRIORS_BY_NAME: Dict[str, float] = {
    "Transcendence": 16.0,
    "Titanic Strength": 18.0,
    "Invulnerability": 14.0,
    "Echoing Voice": 12.5,
    "Persisting Horror": 12.0,
    "Toxicity": 12.5,
    "Tarecgosa's Blessing": 10.5,
    "Golemancy": 10.5,
    "Polarization": 9.5,
    "Time Turning": 9.5,
    "Amalgamation": 8.5,
    "Double Vision": 8.5,
    "Gilding": 8.5,
    "Harpy's Talons": 7.5,
    "Worship": 9.0,
    "Efficiency": 7.5,
}


@dataclass(frozen=True)
class ApexKnowledge:
    meta: CardMetaDB
    card_id_to_slug: Dict[int, str]
    name_to_id: Dict[str, int]
    id_to_name: Dict[int, str]
    hero_names: Dict[int, str]
    hero_implemented: FrozenSet[int]
    trinket_info: Dict[int, Tuple[str, bool, int, bool]]
    dark_gift_weights: Dict[int, float]
    engine_bonus_by_id: Dict[int, float]
    required_ally_by_id: Dict[int, str]
    tribal_synergy_by_id: Dict[int, str]
    targeted_battlecry_ids: FrozenSet[int]
    hand_synergy_board_ids: FrozenSet[int]
    hand_buff_receiver_ids: FrozenSet[int]
    true_on_sell_plain_ids: FrozenSet[int]
    golden_only_sell_ids: FrozenSet[int]
    young_murk_eye_id: int
    persistent_poet_id: int
    stitched_salvager_id: int
    banana_slamma_id: int
    lurking_leviathan_id: int
    futurefin_id: int
    parasitic_fleshling_id: int
    sindorei_id: int
    leeroy_id: int
    deadly_spore_id: int
    bile_spitter_id: int
    gatekeeper_amalgam_id: int
    morgl_id: int
    faceless_manipulator_id: int
    tyrael_id: int
    captain_sanders_id: int
    last_standing_id: int
    victorious_geomant_id: int
    boom_in_a_box_id: int
    guiding_candle_id: int
    robust_evolution_id: int
    upper_hand_id: int
    weighted_gauntlet_ids: FrozenSet[int]
    eot_trinket_ids: FrozenSet[int]
    deft_deserter_id: int
    soul_rewinder_id: int
    wandering_willbreaker_id: int
    souvenir_stand_id: int
    mysterious_orb_id: int
    willbreaker_sticker_id: int
    lich_bazhial_id: int
    mutanus_id: int
    jailer_id: int


@lru_cache(maxsize=4)
def get_apex_knowledge(catalog_name: str = "full") -> ApexKnowledge:
    meta = get_meta_db(catalog_name)
    tag_names, tag_table = seaglass.card_tags()
    t_idx = {n: i for i, n in enumerate(tag_names)}

    card_id_to_slug: Dict[int, str] = {}
    name_to_id: Dict[str, int] = {}
    id_to_name: Dict[int, str] = {}

    all_tmpls = list(seaglass.catalog("duos")) + list(seaglass.catalog(catalog_name))
    seen_cids: Set[int] = set()
    for tmpl in all_tmpls:
        cid = tmpl.card_id
        if cid in seen_cids:
            continue
        seen_cids.add(cid)
        name_to_id[tmpl.name] = cid
        id_to_name[cid] = tmpl.name
        slug = _engine_slug(tmpl.name)
        try:
            u = seaglass.parse_unit(f"2/2 card:{slug}")
            if u.card_id == cid:
                card_id_to_slug[cid] = slug
        except ValueError:
            pass

    for ex_cid, ex_name in TRINKET_EXCLUSIVE_MINIONS.items():
        name_to_id[ex_name] = ex_cid
        id_to_name[ex_cid] = ex_name

    hero_names: Dict[int, str] = {}
    hero_impl: Set[int] = set()
    for hid, hname, impl in seaglass.heroes_table():
        hero_names[hid] = hname
        if impl:
            hero_impl.add(hid)

    trinket_info: Dict[int, Tuple[str, bool, int, bool]] = {}
    guiding_candle_id = -1
    souvenir_stand_id = -1
    mysterious_orb_id = -1
    willbreaker_sticker_id = -1
    weighted_gauntlet_set: Set[int] = set()
    eot_trinket_set: Set[int] = set()
    eot_trinket_names = {
        "Weighted Gauntlet",
        "Gold Mallet",
        "Murky Sticker",
        "Aggem Sticker",
        "Sphere of Memory",
        "Inductive Gyroblade",
        "Emergency Gearblade",
        "Accord-o-Tron Portrait",
        "Wolfhead Flail",
        "Ghastly Sticker",
    }
    for tid, tname, is_greater, cost, impl in seaglass.trinkets_table():
        trinket_info[tid] = (tname, bool(is_greater), int(cost), bool(impl))
        if tname == "Guiding Candle":
            guiding_candle_id = tid
        elif tname == "Weighted Gauntlet":
            weighted_gauntlet_set.add(tid)
        elif tname == "Souvenir Stand":
            souvenir_stand_id = tid
        elif tname == "Mysterious Orb":
            mysterious_orb_id = tid
        elif tname == "Willbreaker Sticker":
            willbreaker_sticker_id = tid
        if tname in eot_trinket_names:
            eot_trinket_set.add(tid)

    dark_gift_weights: Dict[int, float] = {}
    for dgid, dgname, _min_t, _max_t, _w, impl in seaglass.dark_gifts_table():
        if impl:
            dark_gift_weights[dgid] = DARK_GIFT_PRIORS_BY_NAME.get(dgname, 4.5)

    engine_bonus_by_id: Dict[int, float] = {}
    for cname, w in APEX_COMPOUNDING_ENGINES.items():
        cid = name_to_id.get(cname)
        if cid is not None:
            engine_bonus_by_id[cid] = w

    required_ally_by_id: Dict[int, str] = {}
    for cname, req_tr in REQUIRED_ALLY_SUPPORTS.items():
        cid = name_to_id.get(cname)
        if cid is not None:
            required_ally_by_id[cid] = req_tr

    tribal_synergy_by_id: Dict[int, str] = {}
    for cname, syn_tr in TRIBAL_SYNERGY_ENGINES.items():
        cid = name_to_id.get(cname)
        if cid is not None:
            tribal_synergy_by_id[cid] = syn_tr

    bc_target_col = t_idx.get("battlecry_target")
    targeted_bc: Set[int] = set()
    if bc_target_col is not None:
        for cid, row in tag_table.items():
            if row[bc_target_col] > 0.5:
                targeted_bc.add(cid)

    greedy_conniver_id = name_to_id.get("Greedy Conniver", -1)
    golden_only_sell: Set[int] = {greedy_conniver_id} if greedy_conniver_id > 0 else set()

    sell_col = t_idx.get("sell")
    true_on_sell_plain: Set[int] = set()
    if sell_col is not None:
        for cid, row in tag_table.items():
            if row[sell_col] > 0.5 and cid not in golden_only_sell:
                true_on_sell_plain.add(cid)

    hand_synergy_names = (
        "Choral Mrrrglr",
        "Costume Enthusiast",
        "Diremuck Forager",
        "Futurefin",
        "Twilight Tidehunter",
        "Shamanic Tidecaller",
        "Bassgill",
        "Tide Oracle Morgl",
    )
    hand_synergy_ids = frozenset(
        name_to_id[n] for n in hand_synergy_names if n in name_to_id
    )
    hand_receiver_ids = frozenset(
        name_to_id[n] for n in ("Bream Counter", "Flighty Scout") if n in name_to_id
    )

    return ApexKnowledge(
        meta=meta,
        card_id_to_slug=card_id_to_slug,
        name_to_id=name_to_id,
        id_to_name=id_to_name,
        hero_names=hero_names,
        hero_implemented=frozenset(hero_impl),
        trinket_info=trinket_info,
        dark_gift_weights=dark_gift_weights,
        engine_bonus_by_id=engine_bonus_by_id,
        required_ally_by_id=required_ally_by_id,
        tribal_synergy_by_id=tribal_synergy_by_id,
        targeted_battlecry_ids=frozenset(targeted_bc),
        hand_synergy_board_ids=hand_synergy_ids,
        hand_buff_receiver_ids=hand_receiver_ids,
        true_on_sell_plain_ids=frozenset(true_on_sell_plain),
        golden_only_sell_ids=frozenset(golden_only_sell),
        young_murk_eye_id=name_to_id.get("Young Murk-Eye", -1),
        persistent_poet_id=name_to_id.get("Persistent Poet", -1),
        stitched_salvager_id=name_to_id.get("Stitched Salvager", -1),
        banana_slamma_id=name_to_id.get("Banana Slamma", -1),
        lurking_leviathan_id=name_to_id.get("Lurking Leviathan", -1),
        futurefin_id=name_to_id.get("Futurefin", -1),
        parasitic_fleshling_id=name_to_id.get("Parasitic Fleshling", -1),
        sindorei_id=name_to_id.get("Sin'dorei Straight Shot", -1),
        leeroy_id=name_to_id.get("Leeroy the Reckless", -1),
        deadly_spore_id=name_to_id.get("Deadly Spore", -1),
        bile_spitter_id=name_to_id.get("Bile Spitter", -1),
        gatekeeper_amalgam_id=name_to_id.get("Gatekeeper Amalgam", -1),
        morgl_id=name_to_id.get("Tide Oracle Morgl", 10807),
        faceless_manipulator_id=name_to_id.get("Faceless Manipulator", 10806),
        tyrael_id=name_to_id.get("Tyrael", -1),
        captain_sanders_id=name_to_id.get("Captain Sanders", 701),
        last_standing_id=name_to_id.get("The Last One Standing", 712),
        victorious_geomant_id=name_to_id.get("Victorious Geomant", -1),
        boom_in_a_box_id=name_to_id.get("Boom-in-a-Box", -1),
        guiding_candle_id=guiding_candle_id,
        robust_evolution_id=823,
        upper_hand_id=857,
        weighted_gauntlet_ids=frozenset(weighted_gauntlet_set),
        eot_trinket_ids=frozenset(eot_trinket_set),
        deft_deserter_id=name_to_id.get("Deft Deserter", 509),
        soul_rewinder_id=name_to_id.get("Soul Rewinder", 228),
        wandering_willbreaker_id=name_to_id.get("Wandering Willbreaker", 234),
        souvenir_stand_id=souvenir_stand_id,
        mysterious_orb_id=mysterious_orb_id,
        willbreaker_sticker_id=willbreaker_sticker_id,
        lich_bazhial_id=58044,
        mutanus_id=71916,
        jailer_id=97186,
    )
