//! Darkmoon Prizes (`docs/darkmoon_prizes.md`).
//!
//! The prizes are the generated [`PRIZES`] table (`table.rs`, written by
//! `tools/gen_darkmoon_prizes.py` from `CardDefs.xml`). Each implemented prize has its own file
//! (`src/darkmoon/<slug>.rs`) whose header names the prize, its Tier and its text (checked against
//! the table by the tests below), and which defines its [`CardHooks`]. A prize spell is a card with
//! hooks, cast like every spell: the Tavern and Combat engines never name a prize. The minion
//! prize is a minion in no pool (`cards::trinket_minion_catalog`), whose hooks are its template's.
//!
//! This module also holds the prize Discover (`docs/darkmoon_prizes.md` §3), which the hero and
//! trinkets that give prizes use (§7).

mod table;

pub use table::{CARD_DEFS_BUILD, PRIZES};

use crate::cards::{CardFlags, CardHooks};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// A Darkmoon Prize (`docs/darkmoon_prizes.md` §2.1): one row of the generated table.
#[derive(Clone, Copy, Debug)]
pub struct PrizeData {
    /// Its dbfId: the prize's engine `CardId` (the minion prize's minion has it too).
    pub id: CardId,
    pub card_id: &'static str,
    pub name: &'static str,
    /// Its prize Tier, 1-4.
    pub tier: u32,
    /// The minion prize (`All-Seeing Elder`), not a spell.
    pub minion: bool,
    pub text: &'static str,
}

/// Options in a prize Discover (confirmed).
pub const OFFER_COUNT: usize = 3;

/// The highest prize Tier.
pub const MAX_TIER: u32 = 4;

/// The flags of every prize spell (`docs/darkmoon_prizes.md` §2.2): a spell you cast, not a
/// Tavern spell, in no pool.
pub const PRIZE_SPELL: CardFlags = CardFlags::NOT_TAVERN_SPELL.union(CardFlags::NOT_IN_POOL);

/// An implemented prize: its id and its cards' hooks.
type Implemented = (CardId, fn() -> Vec<(CardId, CardHooks)>);

/// Every implemented prize, by file name.
const IMPLEMENTED: &[Implemented] = &[];

/// The hooks of every implemented prize's cards, for the card registry: a prize spell's own hooks
/// with [`PRIZE_SPELL`]'s flags.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    IMPLEMENTED
        .iter()
        .flat_map(|(_, cards)| cards())
        .map(|(id, hooks)| {
            if data(id).is_some_and(|p| !p.minion) {
                (id, hooks.with_flags(PRIZE_SPELL))
            } else {
                (id, hooks)
            }
        })
        .collect()
}

/// The prizes, sorted by Tier, then name.
pub fn table() -> &'static [PrizeData] {
    PRIZES
}

/// The prize `id`.
pub fn data(id: CardId) -> Option<&'static PrizeData> {
    PRIZES.iter().find(|p| p.id == id)
}

/// `true` if `id` is a Darkmoon Prize.
pub fn is_prize(id: CardId) -> bool {
    data(id).is_some()
}

/// The prize whose card ID is `card_id`.
pub fn by_card_id(card_id: &str) -> Option<&'static PrizeData> {
    PRIZES.iter().find(|p| p.card_id == card_id)
}

/// The file stem of `p`: its name's slug (`big_winner`).
pub fn slug(p: &PrizeData) -> String {
    crate::scenario::slug(p.name)
}

/// The prize named `name` (case-insensitive): its name, its slug, or its card ID.
pub fn by_name(name: &str) -> Option<&'static PrizeData> {
    let wanted = name.trim();
    PRIZES
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(wanted) || slug(p) == wanted)
        .or_else(|| by_card_id(wanted))
}

/// `true` if the engine implements the prize `id`.
pub fn is_implemented(id: CardId) -> bool {
    IMPLEMENTED.iter().any(|&(p, _)| p == id)
}

/// The implemented prizes of `tier`, in table order: what a prize Discover of that Tier offers.
pub fn offerable(tier: u32) -> Vec<&'static PrizeData> {
    PRIZES
        .iter()
        .filter(|p| p.tier == tier && is_implemented(p.id))
        .collect()
}

/// A new card of the prize `id` (`docs/darkmoon_prizes.md` §2.2): a 0-Gold spell, or the minion
/// prize's minion. `None` for a card that isn't a prize (or a minion prize without its minion).
pub fn make_prize(id: CardId) -> Option<Unit> {
    let p = data(id)?;
    if p.minion {
        return crate::cards::template(id).map(|t| t.instantiate());
    }
    let mut spell = Unit::new(p.name, 0, 0)
        .with_card_id(p.id)
        .with_tavern_tier(p.tier);
    spell.is_spell = true;
    spell.spell_cost = 0;
    Some(spell)
}

/// A new card of every prize spell, in table order (the card index's prize cards).
pub fn prize_spells() -> Vec<Unit> {
    PRIZES
        .iter()
        .filter(|p| !p.minion)
        .filter_map(|p| make_prize(p.id))
        .collect()
}

/// Offer a Discover of prizes of `tier` (`docs/darkmoon_prizes.md` §3): 3 different implemented
/// prizes of that Tier, equally likely (one `rng.below(k)` per option over those left, in table
/// order; none for the last one), new cards with the player's "wherever this is" auras. Nothing if
/// the Tier has none.
pub fn discover_prize(state: &mut TavernState, tier: u32, _: &mut CardPool, rng: &mut Rng) {
    let mut candidates = offerable(tier);
    let mut options = Vec::with_capacity(OFFER_COUNT);
    while options.len() < OFFER_COUNT && !candidates.is_empty() {
        let pick = match candidates.len() {
            1 => 0,
            k => rng.below(k),
        };
        let p = candidates.remove(pick);
        if let Some(mut card) = make_prize(p.id) {
            state.apply_global_unit_auras(&mut card);
            options.push(card);
        }
    }
    state.push_discover(options);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// `(file stem, source)` of every prize file.
    fn prize_files() -> Vec<(String, String)> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/darkmoon");
        let mut files: Vec<(String, String)> = fs::read_dir(&dir)
            .expect("src/darkmoon")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|e| e == "rs"))
            .map(|p| {
                let stem = p.file_stem().unwrap().to_string_lossy().into_owned();
                (stem, fs::read_to_string(&p).expect("readable"))
            })
            .filter(|(stem, _)| !["mod", "table"].contains(&stem.as_str()))
            .collect();
        files.sort();
        files
    }

    /// The header a prize file must start with (`docs/darkmoon_prizes.md` §9).
    fn expected_header(p: &PrizeData) -> String {
        format!(
            "//! `{}` (`{}`) — Tier {} Darkmoon Prize.\n//!\n//! {}\n",
            p.name, p.card_id, p.tier, p.text
        )
    }

    /// The value of `pub const <name>: <ty> = <value>;` in `src`.
    fn constant(src: &str, name: &str) -> Option<u32> {
        let line = src
            .lines()
            .find(|l| l.starts_with(&format!("pub const {name}: ")))?;
        line.split('=')
            .nth(1)?
            .trim()
            .trim_end_matches(';')
            .replace('_', "")
            .parse()
            .ok()
    }

    #[test]
    fn every_prize_file_matches_the_table() {
        let files = prize_files();
        assert_eq!(
            files.len(),
            IMPLEMENTED.len(),
            "every prize file is in IMPLEMENTED"
        );
        let scenarios = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/darkmoon");
        for (stem, src) in &files {
            let id = constant(src, "ID").unwrap_or_else(|| panic!("{stem}.rs: no `pub const ID`"));
            let p = data(id).unwrap_or_else(|| panic!("{stem}.rs: ID {id} is not in the table"));
            assert_eq!(slug(p), *stem, "{stem}.rs: file name is the prize's slug");
            let header = expected_header(p);
            assert!(
                src.starts_with(&header),
                "{stem}.rs: the header doesn't match the table; expected:\n{header}"
            );
            assert!(is_implemented(id), "{stem}.rs: not in IMPLEMENTED");
            if p.minion {
                assert!(
                    crate::cards::template(id).is_some(),
                    "{stem}.rs: the minion prize has no minion"
                );
            } else {
                assert!(
                    behaviors().iter().any(|&(card, _)| card == id),
                    "{stem}.rs: the prize has no hooks"
                );
            }
            let scenario = fs::read_to_string(scenarios.join(format!("{stem}.yaml")))
                .unwrap_or_else(|_| panic!("{stem}.rs: no tests/scenarios/darkmoon/{stem}.yaml"));
            assert!(
                scenario.starts_with(&format!("prize: {}\n", p.name)),
                "{stem}.yaml: starts with `prize: {}`",
                p.name
            );
        }
    }

    #[test]
    fn the_table_has_every_prize() {
        assert_eq!(PRIZES.len(), 32);
        for tier in 1..=MAX_TIER {
            let n = PRIZES.iter().filter(|p| p.tier == tier).count();
            assert_eq!(n, [7, 9, 8, 8][tier as usize - 1], "Tier {tier}");
        }
        let minions: Vec<&str> = PRIZES.iter().filter(|p| p.minion).map(|p| p.name).collect();
        assert_eq!(minions, ["All-Seeing Elder"]);
        for (i, p) in PRIZES.iter().enumerate() {
            assert!(p.id > 11_000, "{}: prize ids don't collide", p.name);
            assert!(
                PRIZES[..i].iter().all(|q| q.id != p.id && q.name != p.name),
                "{}: unique",
                p.name
            );
        }
    }

    /// Every prize is in the RL vocabulary's appended cards (`docs/darkmoon_prizes.md` §8.2).
    #[test]
    fn every_prize_is_in_the_vocabulary() {
        for p in PRIZES {
            assert!(
                crate::vocab::APPENDED.contains(&p.id),
                "{} is not in vocab::APPENDED",
                p.name
            );
        }
    }

    #[test]
    fn the_catalog_finds_prizes() {
        let gacha = by_name("Gacha Gift").expect("Gacha Gift");
        assert_eq!(gacha.card_id, "BGS_Treasures_004");
        assert_eq!(by_name("gacha_gift").map(|p| p.id), Some(gacha.id));
        assert_eq!(by_name("BGS_Treasures_004").map(|p| p.id), Some(gacha.id));
        assert_eq!(
            slug(by_name("B.A.N.A.N.A.S.").expect("bananas")),
            "b_a_n_a_n_a_s"
        );
        let spell = make_prize(gacha.id).expect("a card");
        assert!(spell.is_spell);
        assert_eq!(spell.spell_cost, 0);
        assert_eq!(spell.name, "Gacha Gift");
    }

    /// A prize Discover offers 3 different implemented prizes of its Tier (§3).
    #[test]
    fn a_prize_discover_offers_three_different_prizes_of_its_tier() {
        let mut pool = CardPool::new(crate::cards::full_catalog());
        for tier in 1..=MAX_TIER {
            for seed in 0..50 {
                let mut state = TavernState::new();
                let mut rng = Rng::new(seed);
                discover_prize(&mut state, tier, &mut pool, &mut rng);
                let offered = offerable(tier).len().min(OFFER_COUNT);
                let options = state.discover_pending.clone().unwrap_or_default();
                assert_eq!(options.len(), offered, "Tier {tier}");
                for (i, o) in options.iter().enumerate() {
                    let p = data(o.card_id).expect("a prize");
                    assert_eq!(p.tier, tier);
                    assert!(options[..i].iter().all(|q| q.card_id != o.card_id));
                }
            }
        }
    }
}
