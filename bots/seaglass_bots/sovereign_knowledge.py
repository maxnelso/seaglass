"""Sovereign Knowledge Base: 5-Tribe Lobby Conditioning & Empirical Meta Priors."""

from __future__ import annotations

from dataclasses import dataclass
from functools import lru_cache
from typing import Dict, FrozenSet, Sequence, Tuple

from .apex_knowledge import (
    HERO_EQUITY_PRIORS,
    TRINKET_EQUITY_PRIORS,
    ApexKnowledge,
    get_apex_knowledge,
)

TRIBE_ORDER: Tuple[str, ...] = (
    "aberration",
    "beast",
    "demon",
    "dragon",
    "elemental",
    "mech",
    "murloc",
    "pirate",
    "quilboar",
    "undead",
)
TRIBE_TO_IDX: Dict[str, int] = {t: i for i, t in enumerate(TRIBE_ORDER)}

# Hero -> (favorable_tribes, bonus_per_present_tribe, penalty_when_zero_present)
HERO_TRIBE_CONDITIONING: Dict[str, Tuple[Tuple[str, ...], float, float]] = {
    # Cycle / APM / 2g-Refresh economy heroes thrive on Elemental/Pirate/Murloc cycle density
    "Millhouse Manastorm": (("elemental", "pirate", "murloc", "demon"), 1.6, -4.5),
    "Trade Prince Gallywix": (("elemental", "pirate", "murloc", "quilboar"), 1.5, -3.0),
    "Dancin' Deryl": (("elemental", "pirate", "mech"), 1.1, -2.0),
    "Edwin VanCleef": (("elemental", "pirate", "mech", "quilboar"), 1.0, -1.8),
    # Deathrattle / Reborn / Summon synergy heroes
    "Teron Gorefiend": (("undead", "beast", "mech"), 2.0, -5.0),
    "Greybough": (("beast", "undead", "mech"), 1.8, -4.5),
    "The Lich King": (("undead", "beast", "mech"), 1.7, -3.5),
    "N'Zoth": (("beast", "undead", "mech"), 1.8, -4.5),
    "Onyxia": (("undead", "beast"), 1.5, -2.5),
    "Ozumat": (("beast", "undead"), 1.2, -2.0),
    "Tamsin Roame": (("dragon", "undead", "demon"), 1.2, -2.0),
    # Battlecry / Bounce synergy heroes
    "Shudderwock": (("murloc", "elemental", "dragon", "demon", "pirate"), 1.4, -4.0),
    "Dinotamer Brann": (("murloc", "elemental", "dragon", "pirate"), 1.6, -4.5),
    "Jandice Barov": (("murloc", "elemental", "demon"), 1.4, -3.0),
    "Rock Master Voone": (("murloc", "dragon", "quilboar", "elemental"), 1.1, -1.5),
    # Cleave / First-attacker / Divine-Shield combat heroes
    "Al'Akir": (("beast", "mech", "pirate", "elemental", "quilboar"), 1.2, -1.5),
    "Illidan Stormrage": (("beast", "pirate", "quilboar", "dragon"), 1.3, -2.5),
    "Rokara": (("undead", "beast", "mech", "dragon"), 1.0, -1.5),
    # Stat-swap / Eat heroes
    "Vol'jin": (("demon", "elemental", "mech", "naga"), 1.2, -1.5),
    "Mutanus the Devourer": (("elemental", "demon", "mech"), 1.2, -1.5),
    # Spell / Tavern-buff heroes
    "Farseer Nobundo": (("demon", "dragon", "quilboar", "pirate"), 1.0, -1.0),
    "Tae'thelan Bloodwatcher": (("demon", "dragon", "pirate", "murloc"), 1.0, -1.0),
    "Doctor Holli'dae": (("demon", "dragon", "quilboar"), 0.9, -0.8),
}

# Empirical 8-player FFA Hero adjustments calibrated from 1,312 competitive seats (512 8-ApexBot + 800 1v7 replay seats)
EMPIRICAL_HERO_DELTAS: Dict[str, float] = {
    # Proven 8p-FFA Lobby Winners (avg_place <= 4.00 across 1,312 seats)
    "King Mukla": 5.5,
    "Galewing": 5.2,
    "Dancin' Deryl": 4.6,
    "The Rat King": 4.0,
    "Tavish Stormpike": 3.8,
    "C'Thun": 3.8,
    "Enhance-o Mechano": 3.6,
    "Thorim, Stormlord": 3.5,
    "The Jailer": 3.2,
    "George the Fallen": 3.0,
    "Scabbs Cutterbutter": 2.8,
    "Nozdormu": 2.8,
    "Zerek, Master Cloner": 2.8,
    "Ragnaros the Firelord": 2.6,
    "Ozumat": 2.5,
    "Time Twister Chromie": 2.4,
    "Patchwerk": 2.2,
    "Millhouse Manastorm": 2.0,
    "Doctor Holli'dae": 1.8,
    "Farseer Nobundo": 1.8,
    "Sneed": 1.8,
    "Trade Prince Gallywix": 1.5,
    "Galakrond": 1.4,
    "Shudderwock": 1.4,
    "Death Speaker Blackthorn": 1.2,
    "Greybough": 1.2,
    "Cap'n Hoggarr": 0.6,
    # Proven 8p-FFA Trap Heroes in ApexBot priors (avg_place >= 5.10 across 1,312 seats)
    "Mister Clocksworth": -6.5,
    "Forest Lord Cenarius": -5.5,
    "Sir Finley Mrrgglton": -5.5,
    "Millificent Manastorm": -5.0,
    "Zephrys, the Great": -5.0,
    "Kurtrus Ashfallen": -4.8,
    "Loh, the Living Legend": -4.5,
    "Snake Eyes": -4.5,
    "Skycap'n Kragg": -4.5,
    "Captain Eudora": -4.5,
    "Overlord Saurfang": -4.2,
    "Xyrella": -4.2,
    "Patches the Pirate": -4.0,
    "Ini Stormcoil": -4.0,
    "Teron Gorefiend": -3.8,
    "Jandice Barov": -3.5,
    "Captain Hooktusk": -3.5,
    "Guff Runetotem": -3.2,
    "Varden Dawngrasp": -2.8,
    "Al'Akir": -2.5,
}

# Empirical 8-player FFA Trinket adjustments calibrated from 1,312 competitive seats (512 8-ApexBot + 800 1v7 replay seats)
EMPIRICAL_TRINKET_DELTAS: Dict[str, float] = {
    # Proven 8p-FFA Lobby-Winning Trinkets
    "Weighted Gauntlet": 18.0,
    "Yogg-Tastic Pastry": 14.0,
    "Wax Lance": 9.5,
    "Booty Bay Brew": 9.0,
    "Tableware Portrait": 8.5,
    "Mindslasher Portrait": 8.0,
    "Inductive Gyroblade": 6.0,
    "Ornate Clock": 5.8,
    "Boom's Monster Portrait": 5.5,
    "Rewinder Portrait": 5.5,
    "Trip Vouchers": 5.5,
    "Magician's Top Hat": 5.5,
    "Dragonwing Glider": 5.0,
    "Sky Golem Portrait": 5.0,
    "Morgl Portrait": 5.0,
    "Trusty Crowbar": 4.5,
    "Azeroth Model Globe": 4.5,
    "Miniature Ship": 4.0,
    "Reinvigorating Light": 3.5,
    "Tickatus Sticker": 3.5,
    "Bronzebeard Portrait": 2.5,
    "Drakkari Portrait": 2.5,
    "Titus Portrait": 2.0,
    "Ghastly Sticker": 1.5,
    # Proven 8p-FFA Trap Trinkets in ApexBot priors (offsetting inflated +14..+24 base priors so net prior is negative!)
    "Hammer of Twilight": -20.0,
    "Bartend-o-Tron's Oilcan": -26.0,
    "Gold-plated Compass": -26.5,
    "Bob's Tip Jar": -25.0,
    "Goblin Wallet": -24.5,
    "Portable Factory": -24.0,
    "Transcribing Typewriter": -24.0,
    "Grifter Portrait": -23.5,
    "Book of Medivh": -23.0,
    "Worn Treasure Map": -22.5,
    "Mecha-Jaraxxus Sticker": -16.0,
    "Innkeeper's Hearth": -14.0,
    "Bob-blehead": -14.5,
    "Flaming Portrait": -14.0,
    "Sphere of Memory": -12.5,
    "Orb of the Unknown": -12.0,
    "Honeycomb Ring": -11.5,
    "Sludge Portrait": -11.0,
    "Innkeeper's Stein": -12.5,
    "Makeshift Master": -10.5,
    "Shrine of Evolution": -8.0,
    "Stuffed Coin Purse": -10.5,
    "Warband Whistle": -10.5,
    "Explorer's Binoculars": -8.5,
    "Manipulator Portrait": -6.5,
    "Guiding Candle": -3.5,
}

BATTLECRY_HEAVY_TRIBES: FrozenSet[str] = frozenset(
    {"murloc", "dragon", "elemental", "pirate", "demon"}
)
DEATHRATTLE_HEAVY_TRIBES: FrozenSet[str] = frozenset(
    {"beast", "undead", "mech"}
)
EOT_HEAVY_TRIBES: FrozenSet[str] = frozenset(
    {"quilboar", "dragon", "mech", "elemental", "murloc"}
)


@dataclass(frozen=True)
class SovereignKnowledge:
    base: ApexKnowledge
    catalog_name: str

    def hero_lobby_score(self, hero_id: int, lobby_tribes: Sequence[str]) -> float:
        hname = self.base.hero_names.get(hero_id, "")
        score = HERO_EQUITY_PRIORS.get(hname, 3.5) + EMPIRICAL_HERO_DELTAS.get(hname, 0.0)
        if hero_id in self.base.hero_implemented:
            score += 3.0

        if lobby_tribes:
            t_set = set(lobby_tribes)
            cond = HERO_TRIBE_CONDITIONING.get(hname)
            if cond is not None:
                fav_tribes, bonus_each, zero_pen = cond
                matches = sum(1 for t in fav_tribes if t in t_set)
                if matches == 0:
                    score += zero_pen
                else:
                    score += bonus_each * min(3, matches)
            # George / Enhance-o gain extra edge when native Divine Shield tribes (Mech/Dragon) are scarce
            if hname in ("George the Fallen", "Enhance-o Mechano"):
                ds_tribes = sum(1 for t in ("mech", "dragon") if t in t_set)
                if ds_tribes == 0:
                    score += 2.2
        return score

    def trinket_prior_score(self, trinket_name: str) -> float:
        return TRINKET_EQUITY_PRIORS.get(trinket_name, 3.5) + EMPIRICAL_TRINKET_DELTAS.get(
            trinket_name, 0.0
        )

    def neutral_engine_tribe_multiplier(
        self, card_id: int, lobby_tribes: Sequence[str]
    ) -> float:
        """Scale speculative neutral cross-tribe multipliers by active 5-tribe lobby support."""
        if not lobby_tribes:
            return 1.0
        t_set = set(lobby_tribes)
        cname = self.base.id_to_name.get(card_id, "")
        if cname in ("Brann Bronzebeard", "Young Murk-Eye"):
            cnt = len(t_set & BATTLECRY_HEAVY_TRIBES)
            if cnt == 0:
                return 0.45
            if cnt == 1:
                return 0.78
            return min(1.18, 0.90 + 0.09 * cnt)
        if cname == "Titus Rivendare":
            cnt = len(t_set & DEATHRATTLE_HEAVY_TRIBES)
            if cnt == 0:
                return 0.35
            if cnt == 1:
                return 0.85
            return min(1.18, 0.92 + 0.10 * cnt)
        if cname == "Drakkari Enchanter":
            cnt = len(t_set & EOT_HEAVY_TRIBES)
            if cnt == 0:
                return 0.55
            if cnt == 1:
                return 0.85
            return min(1.15, 0.92 + 0.08 * cnt)
        return 1.0


@lru_cache(maxsize=4)
def get_sovereign_knowledge(catalog_name: str = "full") -> SovereignKnowledge:
    return SovereignKnowledge(
        base=get_apex_knowledge(catalog_name),
        catalog_name=catalog_name,
    )
