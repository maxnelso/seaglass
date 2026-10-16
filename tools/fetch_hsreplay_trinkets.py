#!/usr/bin/env python3
"""Write the trinket pool override (`tools/data/trinkets_hsreplay.tsv`) from HSReplay's
Battlegrounds trinket stats, the source of truth for which trinkets the game offers
(`engine/docs/trinkets.md` §9).

    python3 tools/fetch_hsreplay_trinkets.py path/to/CardDefs.xml \\
        [--json saved.json] [-o tools/data/trinkets_hsreplay.tsv]

Without `--json`, it fetches

    https://hsreplay.net/api/v1/battlegrounds/trinkets/?BattlegroundsMMRPercentile=ALL&BattlegroundsTimeRange=LAST_7_DAYS

with browser-like headers (`curl --http2` when available, else `urllib`); Cloudflare may still
block the request, and the script then stops with the status and the start of the answer. The
answer is a JSON list of `{trinket_dbf_id, group: lesser|greater, pick_rate, ...}` (a trinket can
appear several times, once per `extra_data` variant). With `--json`, it reads a saved copy of it.

Each trinket's card ID, kind and price come from `CardDefs.xml` (spell school tag 1635 = 11 / 12,
`COST` 48); a kind that differs from HSReplay's `group` is an error. HSReplay only reports Solo
games, so the Duos-exclusive trinkets of the CardDefs pool (tag 3407 and `IS_BACON_DUOS_EXCLUSIVE`
3166) are added too (**engine decision**: HSReplay has no Duos list).

The output is what `tools/gen_trinkets.py` reads: one trinket per line,
`<card ID>\\t<Lesser|Greater>\\t<cost>`, the name in a trailing `#` comment, sorted by name, then
Lesser before Greater, then card ID. Only the Python standard library is used.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gen_heroes import clean_text, parse  # noqa: E402

URL = (
    "https://hsreplay.net/api/v1/battlegrounds/trinkets/"
    "?BattlegroundsMMRPercentile=ALL&BattlegroundsTimeRange=LAST_7_DAYS"
)
HEADERS = {
    "User-Agent": (
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) "
        "Chrome/124.0 Safari/537.36"
    ),
    "Accept": "application/json",
    "Referer": "https://hsreplay.net/battlegrounds/trinkets/lesser/",
}

TAG_COST = "48"
TAG_CARDNAME = "185"
TAG_SPELL_SCHOOL = "1635"
TAG_DUOS_EXCLUSIVE = "3166"
TAG_IN_POOL = "3407"
SCHOOLS = {"11": "Lesser", "12": "Greater"}

DEFAULT_OUTPUT = Path(__file__).resolve().parent / "data/trinkets_hsreplay.tsv"


def fetch() -> str:
    """HSReplay's answer, as text."""
    if shutil.which("curl"):
        cmd = ["curl", "--http2", "-sS", "-w", "\n%{http_code}", URL]
        for k, v in HEADERS.items():
            cmd += ["-H", f"{k}: {v}"]
        out = subprocess.run(cmd, capture_output=True, text=True, check=False)
        if out.returncode != 0:
            sys.exit(f"curl failed ({out.returncode}): {out.stderr.strip()}")
        body, _, status = out.stdout.rpartition("\n")
        if status != "200":
            sys.exit(f"HSReplay answered HTTP {status} (Cloudflare?): {body[:200]!r}")
        return body
    req = urllib.request.Request(URL, headers=HEADERS)
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return resp.read().decode()
    except urllib.error.HTTPError as e:
        sys.exit(f"HSReplay answered HTTP {e.code} (Cloudflare?): {e.read()[:200]!r}")


def rows(text: str) -> list[dict]:
    """The trinket rows of HSReplay's answer, one per trinket."""
    try:
        data = json.loads(text)
    except json.JSONDecodeError:
        sys.exit(f"HSReplay's answer is not JSON (Cloudflare?): {text[:200]!r}")
    if not isinstance(data, list) or not all("trinket_dbf_id" in r for r in data):
        sys.exit(f"unexpected answer: {text[:200]!r}")
    by_dbf: dict[int, dict] = {}
    for r in data:
        by_dbf.setdefault(r["trinket_dbf_id"], r)
    return list(by_dbf.values())


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("card_defs", type=Path)
    ap.add_argument("--json", type=Path, help="a saved copy of HSReplay's answer")
    ap.add_argument("-o", "--output", type=Path, default=DEFAULT_OUTPUT)
    args = ap.parse_args()
    text = args.json.read_text() if args.json else fetch()
    build, entities = parse(args.card_defs)
    by_dbf = {e["dbf"]: (cid, e) for cid, e in entities.items()}

    pool = {}
    for r in rows(text):
        found = by_dbf.get(r["trinket_dbf_id"])
        if not found:
            sys.exit(f"dbf {r['trinket_dbf_id']}: not in CardDefs build {build}")
        cid, e = found
        kind = SCHOOLS.get(e["tags"].get(TAG_SPELL_SCHOOL))
        if kind is None or kind.lower() != r.get("group"):
            sys.exit(f"{cid}: HSReplay says {r.get('group')}, CardDefs says {kind}")
        pool[cid] = (e, kind, "")
    solo = len(pool)
    for cid, e in entities.items():
        t = e["tags"]
        kind = SCHOOLS.get(t.get(TAG_SPELL_SCHOOL))
        if kind and t.get(TAG_IN_POOL) == "1" and t.get(TAG_DUOS_EXCLUSIVE) == "1":
            pool.setdefault(cid, (e, kind, ", Duos only"))

    lines = []
    for cid, (e, kind, note) in pool.items():
        name = clean_text(e["tags"].get(TAG_CARDNAME, cid))
        cost = int(e["tags"].get(TAG_COST, "0"))
        lines.append(((name.lower(), kind == "Greater", cid), f"{cid}\t{kind}\t{cost}\t# {name}{note}"))
    lines.sort()
    header = [
        "# The trinket pool: GENERATED by `tools/fetch_hsreplay_trinkets.py` from HSReplay's",
        f"# Battlegrounds trinket stats ({solo} Solo trinkets) and CardDefs build {build}",
        f"# (kinds, prices, and the {len(pool) - solo} Duos-exclusive trinkets). Read by",
        "# `tools/gen_trinkets.py`. Do not edit: rerun the script (`engine/docs/trinkets.md` §9).",
    ]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text("\n".join(header + [line for _, line in lines]) + "\n")
    print(f"wrote {args.output} ({len(pool)} trinkets: {solo} Solo, {len(pool) - solo} Duos only)")


if __name__ == "__main__":
    main()
