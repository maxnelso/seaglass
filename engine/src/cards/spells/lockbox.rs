//! `Lockbox` (`BG36_520t`) — unplayable token spell.
//!
//! Opens in 5 turns for a Golden minion with a type.

use crate::cards::tokens::SPELL_LOCKBOX;
use crate::cards::{
    all_templates, check_stat_thresholds, on_card_added_to_hand, CardFlags, CardHooks, CardTemplate,
};
use crate::model::Tribe;
use crate::rng::Rng;
use crate::tavern::TavernState;

/// Hooks registered for this spell in [`super::behaviors`].
pub fn hooks() -> CardHooks {
    CardHooks::EMPTY
        .with_flags(CardFlags::NOT_TAVERN_SPELL)
        .on_turn_start_in_hand(turn_start_in_hand)
        .on_ready_in_hand(ready_in_hand)
}

/// `Lockbox` in hand at the start of your turn: count down, and open once the countdown ends.
fn turn_start_in_hand(state: &mut TavernState, hand_idx: usize, rng: &mut Rng) {
    let lockbox = &mut state.hand[hand_idx];
    lockbox.lockbox_turns_left = lockbox.lockbox_turns_left.saturating_sub(1);
    if lockbox.lockbox_turns_left == 0 {
        open_lockbox(state, hand_idx, rng);
    }
}

/// `Lockbox` in hand: open it if its countdown has already ended (e.g. sped up in combat).
fn ready_in_hand(state: &mut TavernState, hand_idx: usize, rng: &mut Rng) {
    if state.hand[hand_idx].lockbox_turns_left == 0 {
        open_lockbox(state, hand_idx, rng);
    }
}

/// Open the `Lockbox` at `state.hand[hand_idx]`, replacing it with a random Golden minion of your
/// Tier with a type.
pub fn open_lockbox(state: &mut TavernState, hand_idx: usize, rng: &mut Rng) {
    if hand_idx >= state.hand.len() || state.hand[hand_idx].card_id != SPELL_LOCKBOX {
        return;
    }
    let target_tier = state.tavern_tier.max(1);
    let typed = |t: &&CardTemplate| t.tribe != Tribe::None && !t.intrinsic_golden;
    let mut typed_templates: Vec<&CardTemplate> = all_templates()
        .iter()
        .filter(|t| typed(t) && t.tavern_tier == target_tier)
        .collect();
    if typed_templates.is_empty() {
        typed_templates = all_templates()
            .iter()
            .filter(|t| typed(t) && t.tavern_tier <= target_tier)
            .collect();
    }
    if typed_templates.is_empty() {
        return;
    }
    let pick = rng.below(typed_templates.len());
    let mut golden = typed_templates[pick].instantiate();
    golden.make_golden();
    golden.intrinsic_golden = false;
    state.apply_global_unit_auras(&mut golden);
    check_stat_thresholds(&mut golden);
    state.hand[hand_idx] = golden;
    on_card_added_to_hand(&state.board, &mut state.auras);
}
