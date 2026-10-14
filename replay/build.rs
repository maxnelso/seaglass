//! Derives the Hearthstone-card-ID-to-engine mapping from the engine itself, so it can't go
//! stale: every engine minion module starts with a header naming its card ID(s),
//!
//! ```text
//! //! `Vicious Mindslasher` (`BG36_108`) — Tier 3 Aberration (`1/2`).
//! //! `Dark Paradox` (`BG36_360` / `BG36_360t5`) — Tier 4 All (`2/6`).
//! ```
//!
//! and this script turns each into `("BG36_108", seaglass::cards::minions::vicious_mindslasher::ID,
//! "Vicious Mindslasher")`. The generated code names the module's `ID` constant, so a renamed or
//! removed module fails the build instead of silently mapping to the wrong card. Spells whose
//! headers name a card ID are listed too (most spell headers only name the card; those resolve
//! by name).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// The card IDs and name in a module's first doc line, if it has the expected header.
fn header_ids(src: &str) -> Option<(String, Vec<String>)> {
    let first = src.lines().next()?.strip_prefix("//! `")?;
    let (name, rest) = first.split_once('`')?;
    let rest = rest.trim_start();
    let ids_part = rest.strip_prefix('(')?;
    let close = ids_part.find(')')?;
    let ids: Vec<String> = ids_part[..close]
        .split('/')
        .map(|s| s.trim().trim_matches('`').to_string())
        .filter(|s| !s.is_empty() && !s.contains(' '))
        .collect();
    if ids.is_empty() {
        return None;
    }
    Some((name.to_string(), ids))
}

/// `pub const <NAME>: CardId` constants a spell module could be registered under are not
/// declared in the module itself, so spells are emitted with their name only.
fn modules(dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        if stem == "mod" {
            continue;
        }
        if let Ok(src) = fs::read_to_string(&path) {
            out.push((stem, src));
        }
    }
    out.sort();
    out
}

fn main() {
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let cards = manifest.join("../engine/src/cards");
    println!("cargo:rerun-if-changed={}", cards.join("minions").display());
    println!("cargo:rerun-if-changed={}", cards.join("spells").display());

    let mut code = String::new();
    code.push_str("/// `(card ID, engine CardId, card name)` for every engine minion, from the module headers.\n");
    code.push_str(
        "pub static ENGINE_MINION_HS_IDS: &[(&str, seaglass::model::CardId, &str)] = &[\n",
    );
    for (module, src) in modules(&cards.join("minions")) {
        if !src.contains("pub const ID: CardId") {
            continue;
        }
        let Some((name, ids)) = header_ids(&src) else {
            panic!("engine minion module `{module}` has no `//! `Name` (`CARD_ID`)` header");
        };
        for id in ids {
            code.push_str(&format!(
                "    ({id:?}, seaglass::cards::minions::{module}::ID, {name:?}),\n"
            ));
        }
    }
    code.push_str("];\n\n");

    code.push_str(
        "/// `(card ID, card name)` for the engine spells whose module header names a card ID.\n",
    );
    code.push_str("pub static ENGINE_SPELL_HS_IDS: &[(&str, &str)] = &[\n");
    for (_module, src) in modules(&cards.join("spells")) {
        if let Some((name, ids)) = header_ids(&src) {
            for id in ids {
                code.push_str(&format!("    ({id:?}, {name:?}),\n"));
            }
        }
    }
    code.push_str("];\n");

    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("engine_hs_ids.rs");
    fs::write(out, code).unwrap();
}
