//! The Wheel of Yogg-Saron (`docs/wheel_of_yogg.md`).
//!
//! The Wheel's six outcomes are the [`OUTCOMES`] table, written from `CardDefs.xml` and
//! HearthstoneJSON (the `Acolyte of Yogg-Saron` Buddy's token spells). Each outcome has its own
//! file (`src/yogg/<slug>.rs`) whose header names the outcome, its card ID, its odds and its text
//! (checked against the table by the tests below), and which defines its [`CardHooks`]: an
//! outcome is a spell card whose `cast` hook is its effect. The Tavern engine never names an
//! outcome; a spin ([`spin_wheel`]) draws one and resolves it ([`resolve`]). The cards that spin
//! the Wheel (`Yogg-Tastic Pastry`) call [`spin_wheel`].

pub mod curse_of_flesh;
pub mod devouring_hunger;
pub mod hand_of_fate;
pub mod mindflayer_goggles;
pub mod mysterybox;
pub mod rod_of_roasting;

use crate::cards::{hooks, CardFlags, CardHooks};
use crate::model::{CardId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// An outcome of the Wheel (`docs/wheel_of_yogg.md` §2): one row of [`OUTCOMES`].
#[derive(Clone, Copy, Debug)]
pub struct Outcome {
    /// Its dbfId: the outcome's engine `CardId`.
    pub id: CardId,
    pub card_id: &'static str,
    pub name: &'static str,
    /// Its odds, in percent (the weights add up to [`TOTAL_WEIGHT`]).
    pub weight: u32,
    /// Its text, as HearthstoneJSON renders it, without markup and line breaks.
    pub text: &'static str,
}

/// The Wheel's outcomes, in the order a spin reads them (`docs/wheel_of_yogg.md` §2, confirmed).
pub const OUTCOMES: &[Outcome] = &[
    Outcome {
        id: mysterybox::ID,
        card_id: "TB_BaconShop_HERO_35_Buddy_t7",
        name: "Mysterybox",
        weight: 19,
        text: "Make a random minion in the Tavern Golden.",
    },
    Outcome {
        id: hand_of_fate::ID,
        card_id: "TB_BaconShop_HERO_35_Buddy_t2",
        name: "Hand of Fate",
        weight: 19,
        text: "Get 2 random Darkmoon Prizes.",
    },
    Outcome {
        id: curse_of_flesh::ID,
        card_id: "TB_BaconShop_HERO_35_Buddy_t3",
        name: "Curse of Flesh",
        weight: 19,
        text: "A random friendly minion gains the stats of another friendly minion.",
    },
    Outcome {
        id: devouring_hunger::ID,
        card_id: "TB_BaconShop_HERO_35_Buddy_t5",
        name: "Devouring Hunger",
        weight: 19,
        text: "Consume all minions in the Tavern and give their stats to your minions. Then Refresh it.",
    },
    Outcome {
        id: mindflayer_goggles::ID,
        card_id: "TB_BaconShop_HERO_35_Buddy_t8",
        name: "Mindflayer Goggles",
        weight: 19,
        text: "Cast 4 random Tavern spells.",
    },
    Outcome {
        id: rod_of_roasting::ID,
        card_id: "TB_BaconShop_HERO_35_Buddy_t6",
        name: "Rod of Roasting",
        weight: 5,
        text: "Cast 'Pyrobuff' randomly to give minions +10/+10 until one hits your bartender or hero.",
    },
];

/// The sum of the weights: a spin's one draw is `rng.below(TOTAL_WEIGHT)`.
pub const TOTAL_WEIGHT: u32 = 100;

/// The flags of every outcome's spell: not a Tavern spell, in no pool.
const OUTCOME_SPELL: CardFlags = CardFlags::NOT_TAVERN_SPELL.union(CardFlags::NOT_IN_POOL);

/// An implemented outcome's cards' hooks.
type Behaviors = fn() -> Vec<(CardId, CardHooks)>;

/// Every outcome's hooks, by file name.
const BEHAVIORS: &[Behaviors] = &[
    curse_of_flesh::behaviors,
    devouring_hunger::behaviors,
    hand_of_fate::behaviors,
    mindflayer_goggles::behaviors,
    mysterybox::behaviors,
    rod_of_roasting::behaviors,
];

/// The hooks of every outcome's spell, for the card registry, with [`OUTCOME_SPELL`]'s flags.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    BEHAVIORS
        .iter()
        .flat_map(|cards| cards())
        .map(|(id, hooks)| (id, hooks.with_flags(OUTCOME_SPELL)))
        .collect()
}

/// The outcomes, in table order.
pub fn table() -> &'static [Outcome] {
    OUTCOMES
}

/// The outcome `id`.
pub fn data(id: CardId) -> Option<&'static Outcome> {
    OUTCOMES.iter().find(|o| o.id == id)
}

/// The file stem of `o`: its name's slug (`curse_of_flesh`).
pub fn slug(o: &Outcome) -> String {
    crate::scenario::slug(o.name)
}

/// The outcome named `name` (case-insensitive): its name, its slug, or its card ID.
pub fn by_name(name: &str) -> Option<&'static Outcome> {
    let wanted = name.trim();
    OUTCOMES
        .iter()
        .find(|o| o.name.eq_ignore_ascii_case(wanted) || slug(o) == wanted || o.card_id == wanted)
}

/// The outcome a draw `r` (`0..TOTAL_WEIGHT`) lands on: the first whose cumulative weight
/// exceeds it.
pub fn outcome_at(r: u32) -> &'static Outcome {
    let mut acc = 0;
    for o in OUTCOMES {
        acc += o.weight;
        if r < acc {
            return o;
        }
    }
    OUTCOMES.last().expect("the Wheel has outcomes")
}

/// Spin the Wheel for the player of `state` (`docs/wheel_of_yogg.md` §3): one
/// `rng.below(TOTAL_WEIGHT)` over the weights in table order, then the outcome resolves at once
/// ([`resolve`]). Returns the outcome's id.
pub fn spin_wheel(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) -> CardId {
    let outcome = outcome_at(rng.below(TOTAL_WEIGHT as usize) as u32);
    resolve(state, outcome.id, pool, rng);
    outcome.id
}

/// Resolve the outcome `id` for the player of `state` (`docs/wheel_of_yogg.md` §3): its spell's
/// `cast` hook. Not a spell the player casts: no spell counter or spell-cast observer sees it.
pub fn resolve(state: &mut TavernState, id: CardId, pool: &mut CardPool, rng: &mut Rng) {
    let Some(o) = data(id) else {
        return;
    };
    if let Some(cast) = hooks(id).cast {
        let card = outcome_card(o);
        cast(state, &card, 0, pool, rng);
    }
}

/// The card of the outcome `o`: a 0-Gold spell.
fn outcome_card(o: &Outcome) -> Unit {
    let mut spell = Unit::new(o.name, 0, 0).with_card_id(o.id);
    spell.is_spell = true;
    spell.spell_cost = 0;
    spell
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// `(file stem, source)` of every outcome file.
    fn outcome_files() -> Vec<(String, String)> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/yogg");
        let mut files: Vec<(String, String)> = fs::read_dir(&dir)
            .expect("src/yogg")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|e| e == "rs"))
            .map(|p| {
                let stem = p.file_stem().unwrap().to_string_lossy().into_owned();
                (stem, fs::read_to_string(&p).expect("readable"))
            })
            .filter(|(stem, _)| stem != "mod")
            .collect();
        files.sort();
        files
    }

    /// The header an outcome file must start with (`docs/wheel_of_yogg.md` §6).
    fn expected_header(o: &Outcome) -> String {
        format!(
            "//! `{}` (`{}`) — Wheel of Yogg-Saron, {}%.\n//!\n//! {}\n",
            o.name, o.card_id, o.weight, o.text
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
    fn every_outcome_file_matches_the_table() {
        let files = outcome_files();
        assert_eq!(files.len(), OUTCOMES.len(), "one file per outcome");
        assert_eq!(
            BEHAVIORS.len(),
            OUTCOMES.len(),
            "every outcome is in BEHAVIORS"
        );
        let scenarios = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/yogg");
        for (stem, src) in &files {
            let id = constant(src, "ID").unwrap_or_else(|| panic!("{stem}.rs: no `pub const ID`"));
            let o = data(id).unwrap_or_else(|| panic!("{stem}.rs: ID {id} is not in the table"));
            assert_eq!(slug(o), *stem, "{stem}.rs: file name is the outcome's slug");
            let header = expected_header(o);
            assert!(
                src.starts_with(&header),
                "{stem}.rs: the header doesn't match the table; expected:\n{header}"
            );
            assert!(
                behaviors()
                    .iter()
                    .any(|(card, h)| *card == id && h.cast.is_some()),
                "{stem}.rs: the outcome has no cast hook"
            );
            let scenario = fs::read_to_string(scenarios.join(format!("{stem}.yaml")))
                .unwrap_or_else(|_| panic!("{stem}.rs: no tests/scenarios/yogg/{stem}.yaml"));
            assert!(
                scenario.starts_with(&format!("wheel: {}\n", o.name)),
                "{stem}.yaml: starts with `wheel: {}`",
                o.name
            );
        }
    }

    #[test]
    fn the_table_has_the_six_outcomes_and_their_odds() {
        assert_eq!(OUTCOMES.len(), 6);
        assert_eq!(OUTCOMES.iter().map(|o| o.weight).sum::<u32>(), TOTAL_WEIGHT);
        let weights: Vec<u32> = OUTCOMES.iter().map(|o| o.weight).collect();
        assert_eq!(weights, [19, 19, 19, 19, 19, 5]);
        for (i, o) in OUTCOMES.iter().enumerate() {
            assert!(o.id > 11_000, "{}: outcome ids don't collide", o.name);
            assert!(OUTCOMES[..i]
                .iter()
                .all(|p| p.id != o.id && p.name != o.name));
            assert!(crate::trinkets::data(o.id).is_none());
            assert!(crate::darkmoon::data(o.id).is_none());
        }
        assert_eq!(
            by_name("curse_of_flesh").map(|o| o.id),
            Some(curse_of_flesh::ID)
        );
        assert_eq!(
            by_name("Rod of Roasting").map(|o| o.id),
            Some(rod_of_roasting::ID)
        );
        assert_eq!(
            by_name("TB_BaconShop_HERO_35_Buddy_t8").map(|o| o.id),
            Some(mindflayer_goggles::ID)
        );
    }

    /// A draw lands on each outcome for exactly its weight's share of `0..100` (§3).
    #[test]
    fn a_draw_lands_on_each_outcome_by_its_weight() {
        for o in OUTCOMES {
            let hits = (0..TOTAL_WEIGHT)
                .filter(|&r| outcome_at(r).id == o.id)
                .count();
            assert_eq!(hits as u32, o.weight, "{}", o.name);
        }
        assert_eq!(outcome_at(0).id, mysterybox::ID);
        assert_eq!(outcome_at(94).id, mindflayer_goggles::ID);
        assert_eq!(outcome_at(95).id, rod_of_roasting::ID);
        assert_eq!(outcome_at(99).id, rod_of_roasting::ID);
    }

    /// A spin is one draw of the RNG, then the outcome (§3): its odds over many seeds.
    #[test]
    fn spins_land_with_the_odds() {
        let mut counts = [0u32; 6];
        let n = 20_000;
        for seed in 0..n {
            let mut rng = Rng::new(seed);
            let mut expected = rng.clone();
            let r = expected.below(TOTAL_WEIGHT as usize) as u32;
            let mut state = TavernState::new();
            let mut pool = CardPool::new(crate::cards::full_catalog());
            let id = spin_wheel(&mut state, &mut pool, &mut rng);
            assert_eq!(id, outcome_at(r).id);
            counts[OUTCOMES.iter().position(|o| o.id == id).unwrap()] += 1;
        }
        for (o, &c) in OUTCOMES.iter().zip(&counts) {
            let share = f64::from(c) / n as f64 * 100.0;
            assert!(
                (share - f64::from(o.weight)).abs() < 1.5,
                "{}: {share:.2}% for {}%",
                o.name,
                o.weight
            );
        }
    }
}
