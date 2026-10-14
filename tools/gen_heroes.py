#!/usr/bin/env python3
"""Rebuild the engine's hero table (`engine/src/heroes/table.rs`) from HearthstoneJSON's
`CardDefs.xml`, so it can be refreshed each patch (`engine/docs/heroes.md` §9).

    python3 tools/gen_heroes.py path/to/CardDefs.xml [-o engine/src/heroes/table.rs]

The hero pool is every hero with `BACON_HERO_CAN_BE_DRAFTED` (tag 1491). For each one:

- Health (`HEALTH`, 45) and Solo Armor (`ARMOR`, 292);
- Duos Armor: tag 3206 (unnamed in python-hearthstone). A Duos-only hero
  (`IS_BACON_DUOS_EXCLUSIVE`, 3166) has its Duos Armor as its `ARMOR` and no Solo Armor. Any
  other hero without tag 3206 uses its Solo Armor in Duos, flagged `duos_armor_from_solo`
  (an engine decision, `engine/docs/heroes.md` §2.3);
- its hero power (`HERO_POWER`, 380): card ID, dbfId, name, Gold cost (`COST`, 48) and text;
- the other hero powers whose `BACON_HEROPOWER_BASE_HERO_ID` (2376) is the hero (upgraded or
  chosen forms: Bru'kan's Elements), and its skins (`BACON_SKIN_PARENT_ID`, 2039), so Power.log
  card IDs map back to it.

Only the Python standard library is used. The output is deterministic (heroes sorted by name).
"""

from __future__ import annotations

import argparse
import html
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

TAG_SCRIPT_DATA_NUM_1 = "2"
TAG_SCRIPT_DATA_NUM_2 = "3"
TAG_SCRIPT_DATA_NUM_3 = "2889"
TAG_HEALTH = "45"
TAG_COST = "48"
TAG_CARDTEXT = "184"
TAG_CARDNAME = "185"
TAG_CARDTYPE = "202"
TAG_ARMOR = "292"
TAG_HERO_POWER = "380"
TAG_CAN_BE_DRAFTED = "1491"
TAG_SKIN_PARENT = "2039"
TAG_HEROPOWER_BASE_HERO = "2376"
TAG_DUOS_EXCLUSIVE = "3166"
TAG_DUOS_ARMOR = "3206"

CARDTYPE_HERO = "3"
CARDTYPE_HERO_POWER = "10"


def plural(before: str, m: re.Match) -> str:
    """`|4(a, b)`: `a` if the last number before it is 1, else `b`."""
    numbers = re.findall(r"\d+", before)
    return m[1] if numbers and numbers[-1] == "1" else m[2]


def clean_text(raw: str, numbers: tuple[str, str, str] = ("0", "0", "0")) -> str:
    """Card text as plain text, as HearthstoneJSON renders it: the numbers shown with their
    starting values (`{0}`, `{1}`, `{2}` and a lone `@` are `TAG_SCRIPT_DATA_NUM_1/2/3`), the
    first of the text's variants (an `@` followed by the text again separates them), no markup,
    `|4(a, b)` as `a` after the number 1 and `b` otherwise, `_` (a non-breaking space) and line
    breaks as spaces, whitespace collapsed."""
    text = html.unescape(raw)
    head, sep, tail = text.partition("@")
    start = head.replace("[x]", "").lstrip()[:10]
    if sep and ("{" in head or (start and tail.replace("[x]", "").lstrip().startswith(start))):
        text = head
    else:
        text = text.replace("@", numbers[0])
    for i, n in enumerate(numbers):
        text = text.replace("{%d}" % i, n)
    text = text.replace("[x]", "")
    text = re.sub(r"<[^>]+>", "", text)
    text = re.sub(r"\|4\(([^,]*),\s*([^)]*)\)", lambda m: plural(text[: m.start()], m), text)
    text = re.sub(r"[$#](\d)", r"\1", text)
    text = text.replace("_", " ")
    return " ".join(text.split())


def parse(path: Path) -> tuple[str, dict[str, dict]]:
    """`(build, entities)`: every entity's tags, by card ID."""
    entities: dict[str, dict] = {}
    build = "?"
    for event, elem in ET.iterparse(path, events=("start", "end")):
        if event == "start":
            if elem.tag == "CardDefs":
                build = elem.get("build", "?")
            continue
        if elem.tag != "Entity":
            continue
        card_id = elem.get("CardID")
        ent = {"dbf": int(elem.get("ID")), "tags": {}, "card_tags": {}}
        for tag in elem.findall("Tag"):
            enum = tag.get("enumID")
            if tag.get("type") == "LocString":
                en = tag.find("enUS")
                ent["tags"][enum] = en.text if en is not None and en.text else ""
            else:
                ent["tags"][enum] = tag.get("value")
                if tag.get("cardID"):
                    ent["card_tags"][enum] = tag.get("cardID")
        entities[card_id] = ent
        elem.clear()
    return build, entities


def rust_str(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def generate(build: str, entities: dict[str, dict]) -> str:
    heroes = []
    for cid, e in entities.items():
        t = e["tags"]
        if t.get(TAG_CARDTYPE) != CARDTYPE_HERO or t.get(TAG_CAN_BE_DRAFTED) != "1":
            continue
        if t.get(TAG_SKIN_PARENT):
            continue
        power_cid = e["card_tags"].get(TAG_HERO_POWER)
        if power_cid is None:
            sys.exit(f"{cid} has no hero power")
        power = entities[power_cid]
        armor = int(t.get(TAG_ARMOR, "0"))
        duos_only = t.get(TAG_DUOS_EXCLUSIVE) == "1"
        if TAG_DUOS_ARMOR in t:
            duos_armor, from_solo = int(t[TAG_DUOS_ARMOR]), False
        elif duos_only:
            duos_armor, from_solo = armor, False
        else:
            duos_armor, from_solo = armor, True
        variants = sorted(
            other
            for other, oe in entities.items()
            if other != power_cid
            and oe["tags"].get(TAG_CARDTYPE) == CARDTYPE_HERO_POWER
            and oe["tags"].get(TAG_HEROPOWER_BASE_HERO) == str(e["dbf"])
        )
        skins = sorted(
            other
            for other, oe in entities.items()
            if oe["tags"].get(TAG_SKIN_PARENT) == str(e["dbf"])
        )
        heroes.append(
            {
                "card_id": cid,
                "dbf": e["dbf"],
                "name": clean_text(t.get(TAG_CARDNAME, cid)),
                "health": int(t.get(TAG_HEALTH, "30")),
                "armor": 0 if duos_only else armor,
                "duos_armor": duos_armor,
                "from_solo": from_solo,
                "duos_only": duos_only,
                "power_cid": power_cid,
                "power_dbf": power["dbf"],
                "power_name": clean_text(power["tags"].get(TAG_CARDNAME, power_cid)),
                "cost": int(power["tags"].get(TAG_COST, "0")),
                "text": clean_text(
                    power["tags"].get(TAG_CARDTEXT, ""),
                    (
                        power["tags"].get(TAG_SCRIPT_DATA_NUM_1, "0"),
                        power["tags"].get(TAG_SCRIPT_DATA_NUM_2, "0"),
                        power["tags"].get(TAG_SCRIPT_DATA_NUM_3, "0"),
                    ),
                ),
                "variants": [(v, entities[v]["dbf"]) for v in variants],
                "skins": skins,
            }
        )
    heroes.sort(key=lambda h: (h["name"].lower(), h["card_id"]))

    out = [
        f"//! The hero pool: GENERATED by `tools/gen_heroes.py` from HearthstoneJSON's `CardDefs.xml`",
        f"//! (build {build}). Do not edit: rerun the generator (`docs/heroes.md` §9).",
        "",
        "use super::{HeroData, PowerData};",
        "",
        "/// The CardDefs build this table was generated from.",
        f"pub const CARD_DEFS_BUILD: u32 = {build if build.isdigit() else 0};",
        "",
        f"/// Every hero with `BACON_HERO_CAN_BE_DRAFTED`, sorted by name ({len(heroes)} heroes).",
        "pub static HEROES: &[HeroData] = &[",
    ]
    for h in heroes:
        variants = ", ".join(f"({rust_str(v)}, {d})" for v, d in h["variants"])
        skins = ", ".join(rust_str(s) for s in h["skins"])
        out += [
            "    HeroData {",
            f"        id: {h['dbf']},",
            f"        card_id: {rust_str(h['card_id'])},",
            f"        name: {rust_str(h['name'])},",
            f"        health: {h['health']},",
            f"        armor: {h['armor']},",
            f"        duos_armor: {h['duos_armor']},",
            f"        duos_armor_from_solo: {str(h['from_solo']).lower()},",
            f"        duos_only: {str(h['duos_only']).lower()},",
            "        power: PowerData {",
            f"            id: {h['power_dbf']},",
            f"            card_id: {rust_str(h['power_cid'])},",
            f"            name: {rust_str(h['power_name'])},",
            f"            cost: {h['cost']},",
            f"            text: {rust_str(h['text'])},",
            "        },",
            f"        power_variants: &[{variants}],",
            f"        skins: &[{skins}],",
            "    },",
        ]
    out.append("];")
    return "\n".join(out) + "\n"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("card_defs", type=Path)
    ap.add_argument(
        "-o",
        "--output",
        type=Path,
        default=Path(__file__).resolve().parent.parent / "engine/src/heroes/table.rs",
    )
    args = ap.parse_args()
    build, entities = parse(args.card_defs)
    args.output.write_text(generate(build, entities))
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
