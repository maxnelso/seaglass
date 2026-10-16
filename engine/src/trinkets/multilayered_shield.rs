//! `Multilayered Shield` (`BG36_MagicItem_373`) — Greater, 5 Gold.
//!
//! Your Tavern spells give an extra +1/+2. (Improved by each different friendly minion type!)
//!
//! The Tavern spells' bonus (`auras.spell_bonus_atk` / `spell_bonus_hp`) holds +1/+2 × (1 + the
//! types the board covers by the Menagerie rule: `types::menagerie_type_count`, confirmed). It is
//! brought up to date as picked and before each spell the player casts in the Tavern
//! (`trinket_before_spell`; `stacks` holds the multiplier applied).

use crate::cards::CardHooks;
use crate::model::CardId;
use crate::tavern::TavernState;
use crate::types;

pub const ID: CardId = 133715;

/// The bonus per step.
const BONUS: (i32, i32) = (1, 2);

/// The trinket's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        ID,
        CardHooks::EMPTY
            .on_trinket_bought(|s, _, _| update(s))
            .on_trinket_before_spell(update),
    )]
}

fn update(state: &mut TavernState) {
    let wanted = 1 + types::menagerie_type_count(&state.board) as i32;
    let Some(t) = super::state_mut(&mut state.auras, ID) else {
        return;
    };
    let delta = wanted - std::mem::replace(&mut t.stacks, wanted);
    state.auras.spell_bonus_atk += BONUS.0 * delta;
    state.auras.spell_bonus_hp += BONUS.1 * delta;
}
