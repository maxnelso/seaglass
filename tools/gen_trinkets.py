#!/usr/bin/env python3
"""Rebuild the engine's trinket table (`engine/src/trinkets/table.rs`) from HearthstoneJSON's
`CardDefs.xml` and `cards.json`, so it can be refreshed each patch (`engine/docs/trinkets.md` §9).

    python3 tools/gen_trinkets.py path/to/CardDefs.xml path/to/cards.json \\
        [--hsreplay tools/data/trinkets_hsreplay.tsv] [-o engine/src/trinkets/table.rs]

The pool is every `BATTLEGROUND_TRINKET` with CardDefs tag 3407 set to 1. For each one:

- its kind: Lesser or Greater (`spellSchool` `LESSER_TRINKET` / `GREATER_TRINKET`, CardDefs tag
  1635 = 11 / 12);
- its price in Gold (`COST`, 48);
- its minion types (`battlegroundsAssociatedRaces` in `cards.json`);
- whether it is only in Duos (`IS_BACON_DUOS_EXCLUSIVE`, 3166);
- its text, rendered as for Hero Powers (`tools/gen_heroes.py`).

HSReplay's trinket pages are the source of truth for the current pool and prices. When the
override file exists (by default `tools/data/trinkets_hsreplay.tsv`, or `--hsreplay`), it defines
the pool and the prices instead of tag 3407 and `COST`: one trinket per line,
`<name or card ID>\\t<Lesser|Greater>\\t<cost>`, `#` starting a comment. A line that matches no
trinket, or more than one, is an error.

Only the Python standard library is used. The output is deterministic (trinkets sorted by name,
Lesser before Greater, then by card ID).
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_heroes import clean_text, parse, rust_str  # noqa: E402

TAG_SCRIPT_DATA_NUM_1 = "2"
TAG_SCRIPT_DATA_NUM_2 = "3"
TAG_SCRIPT_DATA_NUM_3 = "2889"
TAG_COST = "48"
TAG_CARDTEXT = "184"
TAG_CARDNAME = "185"
TAG_SPELL_SCHOOL = "1635"
TAG_DUOS_EXCLUSIVE = "3166"
TAG_IN_POOL = "3407"

SCHOOL_LESSER = "11"
SCHOOL_GREATER = "12"

TRIBES = {
    "BEAST": "Beast",
    "DEMON": "Demon",
    "DRAGON": "Dragon",
    "ELEMENTAL": "Elemental",
    "MECHANICAL": "Mech",
    "MURLOC": "Murloc",
    "NAGA": "Naga",
    "PIRATE": "Pirate",
    "QUILBOAR": "Quilboar",
    "UNDEAD": "Undead",
    "ABERRATION": "Aberration",
}

DEFAULT_OVERRIDE = Path(__file__).resolve().parent / "data/trinkets_hsreplay.tsv"


def trinkets(entities: dict[str, dict], hsjson: dict[str, dict]) -> dict[str, dict]:
    """Every trinket of the data (in the pool or not), by card ID."""
    out = {}
    for cid, e in entities.items():
        h = hsjson.get(cid)
        t = e["tags"]
        if not h or h.get("type") != "BATTLEGROUND_TRINKET":
            continue
        school = t.get(TAG_SPELL_SCHOOL)
        if school not in (SCHOOL_LESSER, SCHOOL_GREATER):
            continue
        races = h.get("battlegroundsAssociatedRaces", [])
        for race in races:
            if race not in TRIBES:
                sys.exit(f"{cid}: unknown minion type {race}")
        out[cid] = {
            "card_id": cid,
            "dbf": e["dbf"],
            "name": clean_text(t.get(TAG_CARDNAME, cid)),
            "greater": school == SCHOOL_GREATER,
            "cost": int(t.get(TAG_COST, "0")),
            "tribes": [TRIBES[r] for r in races],
            "duos_only": t.get(TAG_DUOS_EXCLUSIVE) == "1",
            "in_pool": t.get(TAG_IN_POOL) == "1",
            "text": clean_text(
                t.get(TAG_CARDTEXT, ""),
                (
                    t.get(TAG_SCRIPT_DATA_NUM_1, "0"),
                    t.get(TAG_SCRIPT_DATA_NUM_2, "0"),
                    t.get(TAG_SCRIPT_DATA_NUM_3, "0"),
                ),
            ),
        }
    return out


def apply_override(all_trinkets: dict[str, dict], path: Path) -> list[dict]:
    """The pool the HSReplay file lists, with its prices."""
    pool = []
    seen = set()
    for n, line in enumerate(path.read_text().splitlines(), 1):
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        fields = [f.strip() for f in line.split("\t")]
        if len(fields) != 3 or fields[1] not in ("Lesser", "Greater") or not fields[2].isdigit():
            sys.exit(f"{path}:{n}: expected `<name or card ID>\\t<Lesser|Greater>\\t<cost>`")
        key, kind, cost = fields[0], fields[1] == "Greater", int(fields[2])
        if key in all_trinkets:
            matches = [all_trinkets[key]]
        else:
            matches = [
                t
                for t in all_trinkets.values()
                if t["name"].lower() == key.lower() and t["greater"] == kind
            ]
            # Several cards of one name and kind: the one in the CardDefs pool, if only one is.
            if len(matches) > 1:
                matches = [t for t in matches if t["in_pool"]] or matches
        if len(matches) != 1:
            what = "no trinket" if not matches else f"{len(matches)} trinkets"
            sys.exit(f"{path}:{n}: {key!r} ({fields[1]}) matches {what}")
        t = dict(matches[0])
        if t["greater"] != kind:
            sys.exit(f"{path}:{n}: {key!r} is not a {fields[1]} trinket")
        if t["card_id"] in seen:
            sys.exit(f"{path}:{n}: {key!r} is listed twice")
        seen.add(t["card_id"])
        t["cost"] = cost
        pool.append(t)
    return pool


def generate(build: str, pool: list[dict], source: str) -> str:
    pool = sorted(pool, key=lambda t: (t["name"].lower(), t["greater"], t["card_id"]))
    lesser = sum(1 for t in pool if not t["greater"])
    out = [
        "//! The trinket pool: GENERATED by `tools/gen_trinkets.py` from HearthstoneJSON's",
        f"//! `CardDefs.xml` (build {build}) and `cards.json`{source}. Do not edit: rerun the",
        "//! generator (`docs/trinkets.md` §9).",
        "",
        "use super::TrinketData;",
        "use crate::model::Tribe;",
        "",
        "/// The CardDefs build this table was generated from.",
        f"pub const CARD_DEFS_BUILD: u32 = {build if build.isdigit() else 0};",
        "",
        f"/// The trinket pool, sorted by name ({len(pool)} trinkets: {lesser} Lesser,"
        f" {len(pool) - lesser} Greater).",
        "pub static TRINKETS: &[TrinketData] = &[",
    ]
    for t in pool:
        tribes = ", ".join(f"Tribe::{x}" for x in t["tribes"])
        out += [
            "    TrinketData {",
            f"        id: {t['dbf']},",
            f"        card_id: {rust_str(t['card_id'])},",
            f"        name: {rust_str(t['name'])},",
            f"        greater: {str(t['greater']).lower()},",
            f"        cost: {t['cost']},",
            f"        tribes: &[{tribes}],",
            f"        duos_only: {str(t['duos_only']).lower()},",
            f"        text: {rust_str(t['text'])},",
            "    },",
        ]
    out.append("];")
    return "\n".join(out) + "\n"


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("card_defs", type=Path)
    ap.add_argument("cards_json", type=Path)
    ap.add_argument("--hsreplay", type=Path, help="HSReplay pool and prices (TSV)")
    ap.add_argument(
        "-o",
        "--output",
        type=Path,
        default=Path(__file__).resolve().parent.parent / "engine/src/trinkets/table.rs",
    )
    args = ap.parse_args()
    build, entities = parse(args.card_defs)
    hsjson = {c["id"]: c for c in json.loads(args.cards_json.read_text())}
    all_trinkets = trinkets(entities, hsjson)
    override = args.hsreplay or (DEFAULT_OVERRIDE if DEFAULT_OVERRIDE.exists() else None)
    if override:
        pool = apply_override(all_trinkets, override)
        source = f", with the pool and prices of `{override.name}`"
    else:
        pool = [t for t in all_trinkets.values() if t["in_pool"]]
        source = ""
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(generate(build, pool, source))
    print(f"wrote {args.output} ({len(pool)} trinkets)")


if __name__ == "__main__":
    main()
