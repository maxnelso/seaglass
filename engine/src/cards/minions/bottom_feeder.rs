//! `Bottom Feeder` (`BGDUO33_140`) — Tier 3 Murloc (`3/4`), Duos only.
//! At the end of your turn, you and your teammate each get a random Tier 1 card (Golden
//! `BGDUO33_140_G`, `6/8`: two random Tier 1 cards).
//!
//! Each card is drawn on its own, uniformly among the game's Tier 1 minions (the Duos catalog in
//! Duos: `cards::game_templates`) and the Tier 1 Tavern spells the shop can offer; generated, not
//! from the pool. The teammate's are `Card` gifts (`docs/duos_tavern.md` §8.4).

use crate::cards::{self, spells, CardTemplate};
use crate::model::{CardId, PlayerAuras, TeammateGift, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: CardId = 10301;
pub const NAME: &str = "Bottom Feeder";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 3, 4, 3)
        .with_tribe(Tribe::Murloc)
        .on_end_of_turn(end_of_turn)
}

/// End of turn: its player and their teammate each get a random Tier 1 card (2 if Golden).
pub fn end_of_turn(state: &mut TavernState, self_idx: usize, _: &mut CardPool, rng: &mut Rng) {
    for _ in 0..state.board[self_idx].golden_mult() {
        let card = random_tier_1_card(&state.auras, rng);
        state.add_to_hand(card);
        let card = random_tier_1_card(&state.auras, rng);
        state.auras.give_teammate(TeammateGift::Card(card));
    }
}

/// A random Tier 1 card: one of the game's Tier 1 minions or the shop's Tier 1 Tavern spells.
fn random_tier_1_card(auras: &PlayerAuras, rng: &mut Rng) -> Unit {
    let mut cards: Vec<Unit> = cards::game_templates(auras)
        .iter()
        .filter(|t| t.tavern_tier == 1)
        .map(|t| t.instantiate())
        .collect();
    cards.extend(
        spells::tier1_spells()
            .into_iter()
            .filter(|s| spells::is_pool_tavern_spell(s.card_id)),
    );
    let pick = rng.below(cards.len());
    cards.swap_remove(pick)
}
