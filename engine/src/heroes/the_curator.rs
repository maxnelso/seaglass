//! `The Curator` (`TB_BaconShop_HERO_33`) — Armor 16 (Duos 14).
//!
//! Hero Power: `Menagerist` (`TB_BaconShop_HP_033`), 0 Gold: Start the game with a 2/2 Amalgam with Venomous and all minion types.
//!
//! Passive. When the player gets the hero (`hero_chosen`), an `Amalgam` (`TB_BaconShop_HP_033t`, a
//! Tier 1 `2/2` token of every type with Venomous) joins the board if it has room.
//!
//! Master Nguyen never offers it for a single turn (`CardFlags::NOT_A_TURN_POWER`): its Amalgam
//! would come back every turn.

use crate::cards::{tokens, CardFlags, CardHooks};
use crate::combat::MAX_BOARD_SIZE;
use crate::model::{CardId, HeroId};
use crate::tavern::TavernState;

pub const ID: HeroId = 59203;
pub const POWER: CardId = 59201;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .with_flags(CardFlags::NOT_A_TURN_POWER)
            .on_hero_chosen(start_with_an_amalgam),
    )]
}

/// The start of the game: the Amalgam joins the board.
fn start_with_an_amalgam(state: &mut TavernState) {
    if state.board.len() < MAX_BOARD_SIZE {
        state.board.push(tokens::make_amalgam());
        state.sync_all_auras();
    }
}
