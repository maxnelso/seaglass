//! The Dark Gifts' place in the RL card vocabulary (`docs/dark_gifts.md` §8.2): append-only.
//!
//! `python::all_card_ids` appends the gifts after the trinkets. Their order was fixed when Dark
//! Gifts were added: the 47 gifts of CardDefs build 255026, in table order ([`VOCAB_IDS`]). A
//! gift that later leaves the table keeps its index; a new one is appended ([`vocabulary`]).

use super::DARK_GIFTS;
use crate::model::CardId;

/// The gifts' vocabulary order when Dark Gifts were added (do not reorder or remove).
pub const VOCAB_IDS: [CardId; 47] = [
    132207, 133860, 132790, 133476, 132553, 132201, 133474, 133478, 132554, 132200, 133351, 134731,
    133361, 133457, 134733, 132485, 132208, 133470, 133421, 132734, 133472, 132441, 132835, 132279,
    133359, 132202, 132833, 132443, 132203, 132192, 132276, 133344, 132445, 132205, 133423, 133480,
    132555, 132733, 133310, 132732, 132448, 133363, 132442, 133424, 133353, 133482, 134732,
];

/// The gifts' vocabulary: [`VOCAB_IDS`], then the gifts of the table not in it, in table order.
pub fn vocabulary() -> Vec<CardId> {
    let mut ids = VOCAB_IDS.to_vec();
    for g in DARK_GIFTS {
        if !ids.contains(&g.id) {
            ids.push(g.id);
        }
    }
    ids
}
