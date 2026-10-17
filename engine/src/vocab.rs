//! The cards added after the RL card vocabulary was fixed (`docs/trinkets.md` §8.2): append-only.
//!
//! `python::all_card_ids` sorts the cards it knows by id, then appends the Hero Powers, the
//! trinkets and the Dark Gifts in their fixed orders. A card added later would move every index
//! after it, so the cards listed here are left out of the sorted part and appended last, in this
//! order (do not reorder or remove; add new cards at the end).

use crate::cards::minions;
use crate::model::CardId;
use crate::trinkets;

/// The cards added since, in the order they were added.
pub const APPENDED: &[CardId] = &[
    // The trinkets' Spellcraft spells (`docs/trinkets.md` §11.6).
    trinkets::chillmere_mosaic::SPELL,
    trinkets::floating_candle_set::SPELL,
    trinkets::jailer_sticker::SPELL,
    trinkets::jailer_sticker_greater::SPELL,
    trinkets::makeshift_master::SPELL,
    trinkets::ophidian_staff::SPELL,
    trinkets::token_of_the_old_gods::SPELL,
    trinkets::vibrant_bubble::SPELL,
    // The minions only trinkets give (`docs/trinkets.md` §11.9).
    minions::faceless_manipulator::ID,
    minions::rusted_reggie::ID,
    minions::magtheridon_prime::ID,
    minions::baltharak::ID,
    minions::shield_of_the_legion::ID,
    minions::bassgill::ID,
    minions::mishmash::ID,
    minions::doubloon_grifter::ID,
    minions::flourishing_frostling::ID,
    minions::dr_booms_monster::ID,
    minions::arcane_behemoth::ID,
    minions::tide_oracle_morgl::ID,
    // The trinkets' token spells (`docs/trinkets.md` §11.9).
    trinkets::bottle_sticker::SPELL,
    trinkets::goldenizer_supply::SPELL,
    trinkets::lens_case::SPELL,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appended_cards_are_distinct() {
        for (i, id) in APPENDED.iter().enumerate() {
            assert!(!APPENDED[..i].contains(id), "{id} is listed twice");
        }
    }
}
