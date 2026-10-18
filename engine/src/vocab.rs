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
    // The Darkmoon Prizes (`docs/darkmoon_prizes.md` §8.2), in the table's order when they were
    // added (CardDefs build 255026): the 31 spells and the minion `All-Seeing Elder`.
    92337,  // Banana Bunch
    129551, // Crystallization
    65236,  // Gacha Gift
    66463,  // New Recruit
    66427,  // Rocking and Rolling
    65254,  // The Good Stuff
    129192, // Unfurled Codex
    66458,  // Big Brann Play
    65238,  // Evolving Tavern
    65246,  // Gruul Rules
    65262,  // I'm Still Just a Rat in a Cage
    129193, // Mageroyal Blossom
    65250,  // On the House
    65380,  // The Bouncer
    65256,  // The Unlimited Coin
    65248,  // Time Thief
    66947,  // All That Glitters
    65264,  // B.A.N.A.N.A.S.
    65259,  // Buy the Holy Light
    92336,  // Mindflayer Goggles
    66495,  // Repeat Customer
    129198, // Reserve Prices
    65265,  // Top Shelf
    65249,  // Training Session
    122104, // All-Seeing Elder
    66460,  // Big Winner!
    65274,  // Fresh Tab
    65270,  // Friends and Family Discount
    66424,  // Give a Dog a Bone
    129197, // Gorgeous Goblet
    65277,  // Open Bar
    65260,  // Raise the Stakes
    // `Corrupted Tome`'s `Triple Prize` (`docs/darkmoon_prizes.md` §7.3).
    trinkets::corrupted_tome::SPELL,
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
