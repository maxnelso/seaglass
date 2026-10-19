//! Hearthstone card IDs (`BG36_108`, `BG36_311_G`, `TB_BaconUps_079`, ...) to engine cards.
//!
//! The minion table is generated from the engine's own module headers (`build.rs`): every
//! engine minion names its card ID(s) in its first doc line, so a minion added to the engine is
//! mapped as soon as it exists, and a renamed module fails the build. Tokens have no per-card
//! module, so their IDs are listed in [`TOKEN_HS_IDS`] (a test checks that every engine token
//! has one). Spells mostly have no ID in their headers and resolve by name.
//!
//! Lookup order ([`engine_card`]):
//! 1. the exact card ID (minion table, then [`TOKEN_HS_IDS`], then the spells with an ID);
//! 2. its Golden forms: `<id>_G` → `<id>`, `<id>_Gt` → `<id>t`;
//! 3. a variant of a minion whose header lists variants (`Dark Paradox`: `BG36_360t6` →
//!    `BG36_360`, because its header lists `BG36_360t5`; its Golden form `BG36_360_Gt6` too);
//! 4. the card's name (case-insensitive) among the engine's minions, tokens and spells: this
//!    covers the old-style Golden IDs (`TB_BaconUps_079` is "Wrath Weaver") and the spells.
//!
//! Anything else is not implemented by the engine.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use seaglass::cards::tokens;
use seaglass::model::CardId;
use seaglass::scenario::{card_index, CardKind as IndexKind};

include!(concat!(env!("OUT_DIR"), "/engine_hs_ids.rs"));

/// Card IDs of the engine's token minions (`seaglass::cards::tokens`), Golden IDs included.
pub static TOKEN_HS_IDS: &[(&str, CardId)] = &[
    ("BG36_098t", tokens::TOKEN_ABERRANT_TENTACLE),
    ("BGFYM_002t", tokens::TOKEN_ABERRANT_TENTACLE),
    ("BG28_603t", tokens::TOKEN_BEETLE),
    ("BG_BOT_312t", tokens::TOKEN_MICROBOT),
    ("TB_BaconUps_032t", tokens::TOKEN_MICROBOT),
    ("BG28_300t", tokens::TOKEN_SKELETON),
    ("BG36_200t", tokens::TOKEN_BAT),
    ("BGS_115t", tokens::TOKEN_WATER_DROPLET),
    ("TB_BaconUps_156t", tokens::TOKEN_WATER_DROPLET),
    ("BG35_150t", tokens::TOKEN_DEMON_FODDER),
    ("BG36_201t", tokens::TOKEN_FISHBAIT),
    ("BG36_205", tokens::TOKEN_FISHBAIT),
    ("BG25_010t", tokens::TOKEN_HELPING_HAND),
    ("BG31_171t", tokens::TOKEN_SATELLITE),
    ("BG31_171_Gt", tokens::TOKEN_SATELLITE),
    ("BG34_Giant_610t", tokens::TOKEN_SATELLITE),
    ("BG34_Giant_610_Gt", tokens::TOKEN_SATELLITE),
    ("BG19_010", tokens::TOKEN_SEWER_RAT),
    ("TB_BaconUps_259", tokens::TOKEN_SEWER_RAT),
    ("BG19_010t", tokens::TOKEN_HALF_SHELL),
    ("TB_BaconUps_259t", tokens::TOKEN_HALF_SHELL),
    ("BG33_891t", tokens::TOKEN_MAGICFIN_APPRENTICE),
    ("BG33_891_Gt", tokens::TOKEN_MAGICFIN_APPRENTICE),
    ("BG23_013t", tokens::TOKEN_BLOOD_GOLEM),
    ("BG23_013_Gt", tokens::TOKEN_BLOOD_GOLEM),
    // The Golem `Blood Golem Sticker` and `Jailbird Juggernaut` summon.
    ("BG30_MagicItem_442t", tokens::TOKEN_BLOOD_GOLEM),
    ("BG34_634t", tokens::TOKEN_BLUE_CHROMADRAKE),
    ("BG34_635t", tokens::TOKEN_BLACK_CHROMADRAKE),
    ("BG34_636t", tokens::TOKEN_GREEN_CHROMADRAKE),
    ("BG34_637t", tokens::TOKEN_BRONZE_CHROMADRAKE),
    ("BG34_638t", tokens::TOKEN_RED_CHROMADRAKE),
    ("TB_BaconShop_HP_105t", tokens::TOKEN_FISH_OF_NZOTH),
    ("BG21_HERO_030t", tokens::TOKEN_SNEEDS_SHREDDER),
    ("TB_BaconShop_HP_033t", tokens::TOKEN_AMALGAM),
    ("BG29_864t", tokens::TOKEN_DESERTED_GOLEM),
];

/// Card IDs of the Deities (`C'Thun`, `Y'Shaarj`) once awakened.
pub static DEITY_HS_IDS: &[(&str, CardId)] = &[
    ("BGFYM_000", seaglass::cards::deities::CARD_CTHUN),
    ("BGFYM_011", seaglass::cards::deities::CARD_YSHAARJ),
];

/// What kind of engine card a Hearthstone card maps to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CardKind {
    /// A catalog (shop) minion.
    Minion,
    /// A token minion.
    Token,
    /// A Tavern or token spell.
    Spell,
    /// An awakened Deity (`C'Thun`, `Y'Shaarj`).
    Deity,
}

/// How a Hearthstone card was matched to its engine card.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchedBy {
    CardId,
    /// A Golden card ID (`_G`, `_Gt`) of a mapped card ID.
    GoldenCardId,
    /// A variant ID of a minion whose header lists variants (`Dark Paradox`).
    Variant,
    /// The card's name (old-style Golden IDs, spells).
    Name,
}

/// The engine card a Hearthstone card maps to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineCard {
    pub card_id: CardId,
    pub kind: CardKind,
    /// The engine's name for the card.
    pub name: String,
    pub matched_by: MatchedBy,
}

struct Tables {
    /// Card ID → (engine id, kind).
    by_hs_id: HashMap<String, (CardId, CardKind)>,
    /// Lowercase name → (engine id, kind, engine name); ambiguous names are dropped.
    by_name: HashMap<String, (CardId, CardKind, String)>,
    /// Base IDs of minions whose header lists variants (`BG36_360` for `BG36_360t5`).
    variant_bases: HashMap<String, CardId>,
}

fn kind_of(kind: IndexKind) -> CardKind {
    match kind {
        IndexKind::Minion => CardKind::Minion,
        IndexKind::Token => CardKind::Token,
        IndexKind::Spell => CardKind::Spell,
        IndexKind::Deity => CardKind::Deity,
    }
}

fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let index = card_index();
        let mut by_name: HashMap<String, (CardId, CardKind, String)> = HashMap::new();
        let mut ambiguous = Vec::new();
        for card in index.cards() {
            let key = card.name.to_ascii_lowercase();
            let entry = (card.card_id, kind_of(card.kind), card.name.clone());
            match by_name.get(&key) {
                Some(existing) if existing.0 != card.card_id => ambiguous.push(key),
                Some(_) => {}
                None => {
                    by_name.insert(key, entry);
                }
            }
        }
        for key in ambiguous {
            by_name.remove(&key);
        }

        let mut by_hs_id = HashMap::new();
        for &(hs_id, card_id, _) in ENGINE_MINION_HS_IDS {
            by_hs_id.insert(hs_id.to_string(), (card_id, CardKind::Minion));
        }
        for &(hs_id, card_id) in TOKEN_HS_IDS {
            by_hs_id.insert(hs_id.to_string(), (card_id, CardKind::Token));
        }
        for &(hs_id, card_id) in DEITY_HS_IDS {
            by_hs_id.insert(hs_id.to_string(), (card_id, CardKind::Deity));
        }
        for &(hs_id, name) in ENGINE_SPELL_HS_IDS {
            if let Some(&(card_id, kind, _)) = by_name.get(&name.to_ascii_lowercase()) {
                by_hs_id.entry(hs_id.to_string()).or_insert((card_id, kind));
            }
        }

        let mut variant_bases = HashMap::new();
        for &(hs_id, card_id, _) in ENGINE_MINION_HS_IDS {
            if let Some(base) = variant_base(hs_id) {
                if by_hs_id.get(base).map(|e| e.0) == Some(card_id) {
                    variant_bases.insert(base.to_string(), card_id);
                }
            }
        }
        Tables {
            by_hs_id,
            by_name,
            variant_bases,
        }
    })
}

/// `BG36_360` for `BG36_360t5` (an ID ending in `t<digits>`).
fn variant_base(hs_id: &str) -> Option<&str> {
    let t = hs_id.rfind('t')?;
    let digits = &hs_id[t + 1..];
    (!digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())).then(|| &hs_id[..t])
}

/// The engine name of `card_id`.
fn engine_name(card_id: CardId) -> String {
    card_index()
        .by_id(card_id)
        .map(|c| c.name.clone())
        .unwrap_or_default()
}

/// The engine card for the Hearthstone card `hs_id` named `name` (see the module docs for the
/// lookup order), or `None` if the engine doesn't implement it.
pub fn engine_card(hs_id: &str, name: &str) -> Option<EngineCard> {
    let t = tables();
    let found = |card_id: CardId, kind: CardKind, matched_by: MatchedBy| EngineCard {
        card_id,
        kind,
        name: engine_name(card_id),
        matched_by,
    };
    if let Some(&(card_id, kind)) = t.by_hs_id.get(hs_id) {
        return Some(found(card_id, kind, MatchedBy::CardId));
    }
    let golden_forms = [
        hs_id.strip_suffix("_G").map(str::to_string),
        hs_id.strip_suffix("_Gt").map(|base| format!("{base}t")),
    ];
    for form in golden_forms.into_iter().flatten() {
        if let Some(&(card_id, kind)) = t.by_hs_id.get(&form) {
            return Some(found(card_id, kind, MatchedBy::GoldenCardId));
        }
    }
    // A Golden variant (`BG36_360_Gt6`) is the Golden form of the variant `BG36_360t6`.
    let plain = hs_id
        .strip_suffix("_G")
        .map(str::to_string)
        .or_else(|| {
            let (base, n) = hs_id.rsplit_once("_Gt")?;
            (!n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())).then(|| format!("{base}t{n}"))
        })
        .unwrap_or_else(|| hs_id.to_string());
    if let Some(&card_id) = variant_base(&plain).and_then(|base| t.variant_bases.get(base)) {
        return Some(found(card_id, CardKind::Minion, MatchedBy::Variant));
    }
    let key = name.trim().to_ascii_lowercase();
    if !key.is_empty() {
        if let Some((card_id, kind, _)) = t.by_name.get(&key) {
            return Some(found(*card_id, *kind, MatchedBy::Name));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use seaglass::cards::full_catalog;

    #[test]
    fn every_engine_minion_has_a_card_id() {
        let mapped: std::collections::HashSet<CardId> =
            ENGINE_MINION_HS_IDS.iter().map(|e| e.1).collect();
        let missing: Vec<String> = full_catalog()
            .into_iter()
            .filter(|t| !mapped.contains(&t.card_id))
            .map(|t| t.name)
            .collect();
        assert!(
            missing.is_empty(),
            "engine minions without a card ID: {missing:?}"
        );
    }

    #[test]
    fn minion_table_names_match_the_engine() {
        for &(hs_id, card_id, name) in ENGINE_MINION_HS_IDS {
            let tpl = seaglass::cards::template(card_id)
                .unwrap_or_else(|| panic!("{hs_id} maps to {card_id}, not a catalog minion"));
            assert_eq!(tpl.name, name, "{hs_id}");
        }
    }

    #[test]
    fn card_ids_are_unique() {
        let mut seen = HashMap::new();
        for &(hs_id, card_id, _) in ENGINE_MINION_HS_IDS {
            if let Some(other) = seen.insert(hs_id, card_id) {
                panic!("{hs_id} maps to both {other} and {card_id}");
            }
        }
        for &(hs_id, _) in TOKEN_HS_IDS {
            assert!(
                !seen.contains_key(hs_id),
                "{hs_id} is both a minion and a token"
            );
        }
    }

    #[test]
    fn every_engine_token_has_a_card_id() {
        for id in tokens::TOKEN_MINION_IDS {
            assert!(
                TOKEN_HS_IDS.iter().any(|e| e.1 == id),
                "engine token {} ({id}) has no card ID",
                engine_name(id)
            );
        }
    }

    #[test]
    fn lookups() {
        let card = |id: &str, name: &str| engine_card(id, name).map(|c| (c.name, c.matched_by));
        assert_eq!(
            card("BG36_108", ""),
            Some(("Vicious Mindslasher".into(), MatchedBy::CardId))
        );
        assert_eq!(
            card("BG36_311_G", "Abyssal Envoy"),
            Some(("Abyssal Envoy".into(), MatchedBy::GoldenCardId))
        );
        assert_eq!(
            card("TB_BaconUps_079", "Wrath Weaver"),
            Some(("Wrath Weaver".into(), MatchedBy::Name))
        );
        assert_eq!(
            card("BG36_360t6", "Dark Paradox"),
            Some(("Dark Paradox".into(), MatchedBy::Variant))
        );
        assert_eq!(
            card("BG28_603_Gt", ""),
            Some(("Beetle".into(), MatchedBy::GoldenCardId))
        );
        assert_eq!(card("BGDUO_124", "Recruitment Program"), None);
    }
}
