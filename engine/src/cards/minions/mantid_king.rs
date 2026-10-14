//! `Mantid King` (`BGDUO31_212`) — Tier 4 Neutral (`3/3`), Duos only.
//! After your team Passes, randomly gain Venomous, Taunt, or Divine Shield until next turn
//! (Golden `BGDUO31_212_G`, `6/6`: permanently).
//!
//! An `after_team_pass` observer: after each of the team's Passes, one of the three it doesn't
//! have, at random; nothing if it has them all (`docs/duos_tavern.md` §8.3). Until next turn: its
//! `counter` remembers which (one bit per keyword) and it loses them at the start of its player's
//! next turn, on the board or in the hand; a Divine Shield is a `temp_divine_shield`. Made
//! Golden, the keywords it has until next turn stay temporary.

use crate::cards::CardTemplate;
use crate::model::{CardId, Keyword, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10403;
pub const NAME: &str = "Mantid King";

/// The keywords it gains, by `counter` bit.
const KEYWORDS: [Keyword; 3] = [Keyword::Venomous, Keyword::Taunt, Keyword::DivineShield];

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 3, 4)
        .on_after_team_pass(after_team_pass)
        .on_turn_start_unit(expire_keywords)
        .on_turn_start_in_hand(|state, hand_idx, _| expire_keywords(&mut state.hand[hand_idx]))
        .on_merge_golden(merge_golden)
}

/// The team Passed: gain one of the keywords it lacks, until next turn unless it is Golden.
pub fn after_team_pass(state: &mut TavernState, self_idx: usize, _: &mut CardPool, rng: &mut Rng) {
    let king = &mut state.board[self_idx];
    let missing: Vec<usize> = (0..KEYWORDS.len())
        .filter(|&bit| !king.has_keyword(KEYWORDS[bit]))
        .collect();
    let bit = match missing.len() {
        0 => return,
        1 => missing[0],
        n => missing[rng.below(n)],
    };
    king.apply_keyword(KEYWORDS[bit], false);
    if !king.is_golden {
        king.counter |= 1 << bit;
        if KEYWORDS[bit] == Keyword::DivineShield {
            king.temp_divine_shield = true;
        }
    }
}

/// Start of turn: lose the keywords it had until then.
fn expire_keywords(king: &mut Unit) {
    for (bit, keyword) in KEYWORDS.into_iter().enumerate() {
        if king.counter & (1 << bit) == 0 {
            continue;
        }
        match keyword {
            Keyword::Venomous => king.venomous = false,
            Keyword::Taunt => king.taunt = false,
            _ => {
                king.divine_shield = false;
                king.temp_divine_shield = false;
            }
        }
    }
    king.counter = 0;
}

/// A triple keeps a keyword until next turn if a copy had it until next turn and none had it for
/// good.
fn merge_golden(copies: &[Unit], golden: &mut Unit) {
    golden.counter = 0;
    for (bit, keyword) in KEYWORDS.into_iter().enumerate() {
        let temporary = |c: &Unit| c.counter & (1 << bit) != 0;
        let kept = copies
            .iter()
            .any(|c| c.has_keyword(keyword) && !temporary(c));
        if !kept && copies.iter().any(temporary) {
            golden.counter |= 1 << bit;
        }
    }
}
