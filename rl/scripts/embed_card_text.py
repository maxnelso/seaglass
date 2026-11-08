"""Embed every card's rules text, for the models' static card features (`seaglass_rl/card_features.py`).

Reads the card sources' doc comments: every file under `engine/src/cards/{minions,spells}`,
`engine/src/trinkets`, `engine/src/dark_gifts`, `engine/src/darkmoon` and `engine/src/heroes` starts
with a title line (`` `Name` (`ID`) — Tier 2 Dragon (`3/4`). ``) and then the card's text. Its
`pub const ID` is the key. The text of a card is its title's description and its first paragraph,
with the Markdown stripped (the same text `tools/build.rs` gives the replay player).

Each text is embedded with a sentence-embedding model (`sentence-transformers`, default
`all-MiniLM-L6-v2`). Without the package it falls back to TF-IDF + SVD. The embeddings are reduced
to `--dim` dimensions with PCA and saved, with the texts, to
`seaglass_rl/data/card_text_embeddings.npz`. Run it again when cards change:

    cd rl && uvx --with sentence-transformers --with numpy python scripts/embed_card_text.py
"""

from __future__ import annotations

import argparse
import re
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
SOURCES = ["cards/minions", "cards/spells", "trinkets", "dark_gifts", "darkmoon", "heroes"]
OUT = Path(__file__).resolve().parents[1] / "seaglass_rl" / "data" / "card_text_embeddings.npz"

ID_RE = re.compile(r"pub const ID: \w+ = (\d+);")
# Any card id constant, with the `///` doc lines just above it.
CONST_RE = re.compile(r"((?:^[ \t]*///.*\n)*)^[ \t]*pub const (\w+): (?:CardId|HeroId) = (\d+);", re.M)
# `` `Name` (`ID`) — description `` or `` `Name` — description ``.
TITLE_RE = re.compile(r"^`([^`]+)`\s*(?:\([^)]*\))?\s*[—-]?\s*(.*)$")


def strip_markdown(text: str) -> str:
    return re.sub(r"[*`]", "", text).strip()


def card_text(source: str) -> str | None:
    """The card's title description and first paragraph, from its leading `//!` doc comment."""
    lines = []
    for line in source.splitlines():
        if not line.startswith("//!"):
            break
        lines.append(line[3:].strip())
    if not lines:
        return None
    title = TITLE_RE.match(lines[0])
    if not title:
        return None
    name, desc = title.group(1), title.group(2)
    paragraph: list[str] = []
    for line in lines[1:]:
        if not line:
            if paragraph:
                break
            continue
        paragraph.append(line)
    return strip_markdown(f"{name}. {desc} {' '.join(paragraph)}")


def const_name(name: str, doc: str) -> str:
    """A constant's card name: its doc's first `` `Name` ``, else the constant's name in words."""
    m = re.search(r"`([^`]+)`", doc)
    if m:
        return m.group(1)
    words = [w for w in name.split("_") if w not in {"TOKEN", "SPELL", "CARD", "CHOICE", "POWER"}]
    return " ".join(words).title() or name.title()


def hero_texts(table: str) -> list[tuple[int, str]]:
    """(id, text) for every hero, Hero Power and power variant in `heroes/table.rs`."""
    out = []
    for block in table.split("HeroData {")[1:]:
        hero = re.search(r"^\s*id: (\d+),", block)
        name = re.search(r'^\s*name: "([^"]*)"', block, re.M)
        power = re.search(r"PowerData \{(.*?)\}", block, re.S)
        if not (hero and name and power):
            continue
        field = {k: v for k, v in re.findall(r'(\w+): ("[^"]*"|\d+)', power.group(1))}
        p_name, p_text = field.get("name", '""').strip('"'), field.get("text", '""').strip('"')
        line = f"{p_name}. Hero Power of {name.group(1)} ({field.get('cost', '?')} Gold): {p_text}"
        out.append((int(hero.group(1)), f"{name.group(1)}. Hero Power: {line}"))
        ids = [field.get("id")] + re.findall(r"(\d+)\)", block.split("power_variants", 1)[-1].split("]", 1)[0])
        out.extend((int(i), line) for i in ids if i)
    return out


def collect() -> tuple[list[int], list[str]]:
    """Every card id with a text, in three passes (the first text found for an id wins):
    1. each card file's own id (`pub const ID`, or for a spell file its `SPELL_<FILE>` constant in
       `cards/spells/mod.rs`) gets the file's text;
    2. each hero, its Hero Power and the power's variants (`heroes/table.rs`) get the hero's name
       and the power's text;
    3. the other card ids a card file defines (a hero's Hero Power, a trinket's spell, a minion's
       token) get their name and the file's text;
    4. every other card id constant in the engine (the shared tokens, Choose One options, ...)
       gets its doc comment, or just its name.
    """
    src = ROOT / "engine" / "src"
    spell_consts = {name: int(cid) for _, name, cid in CONST_RE.findall((src / "cards/spells/mod.rs").read_text())}
    texts: dict[int, str] = {}
    secondary: list[tuple[int, str]] = []
    for sub in SOURCES:
        for path in sorted((src / sub).glob("*.rs")):
            source = path.read_text()
            text = card_text(source)
            if not text:
                continue
            m = ID_RE.search(source)
            primary = int(m.group(1)) if m else spell_consts.get(f"SPELL_{path.stem.upper()}")
            if primary is not None:
                texts.setdefault(primary, text)
            for doc, name, cid in CONST_RE.findall(source):
                if int(cid) != primary:
                    secondary.append((int(cid), f"{const_name(name, doc)}. From {text}"))
    for cid, text in hero_texts((src / "heroes/table.rs").read_text()):
        texts.setdefault(cid, text)
    for cid, text in secondary:
        texts.setdefault(cid, text)
    for path in sorted(src.rglob("*.rs")):
        for doc, name, cid in CONST_RE.findall(path.read_text()):
            doc_text = strip_markdown(" ".join(line.strip().lstrip("/").strip() for line in doc.splitlines()))
            texts.setdefault(int(cid), f"{const_name(name, doc)}. {doc_text}".strip())
    ids = sorted(texts)
    return ids, [texts[i] for i in ids]


def embed(texts: list[str], model_name: str) -> np.ndarray:
    try:
        from sentence_transformers import SentenceTransformer
    except ImportError:
        print("sentence-transformers is not installed: TF-IDF + SVD")
        return tfidf(texts)
    model = SentenceTransformer(model_name)
    return np.asarray(model.encode(texts, normalize_embeddings=True, show_progress_bar=False), dtype=np.float32)


def tfidf(texts: list[str]) -> np.ndarray:
    docs = [re.findall(r"[a-z0-9+/]+", t.lower()) for t in texts]
    vocab = sorted({w for d in docs for w in d})
    index = {w: i for i, w in enumerate(vocab)}
    tf = np.zeros((len(docs), len(vocab)), dtype=np.float32)
    for r, d in enumerate(docs):
        for w in d:
            tf[r, index[w]] += 1
    idf = np.log((1 + len(docs)) / (1 + (tf > 0).sum(0))) + 1
    x = tf * idf
    x /= np.linalg.norm(x, axis=1, keepdims=True) + 1e-8
    return x


def pca(x: np.ndarray, dim: int) -> np.ndarray:
    x = x - x.mean(0)
    u, s, _ = np.linalg.svd(x, full_matrices=False)
    out = u[:, :dim] * s[:dim]
    return (out / (out.std() + 1e-8)).astype(np.float32)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--dim", type=int, default=32)
    parser.add_argument("--model", default="sentence-transformers/all-MiniLM-L6-v2")
    args = parser.parse_args()
    ids, texts = collect()
    raw = embed(texts, args.model)
    emb = pca(raw, args.dim)
    OUT.parent.mkdir(parents=True, exist_ok=True)
    np.savez_compressed(OUT, ids=np.asarray(ids, dtype=np.int64), emb=emb, texts=np.asarray(texts))
    print(f"{len(ids)} cards, {raw.shape[1]} -> {emb.shape[1]} dims: {OUT}")


if __name__ == "__main__":
    main()
