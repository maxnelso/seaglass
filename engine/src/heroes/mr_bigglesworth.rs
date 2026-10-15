//! `Mr. Bigglesworth` (`TB_BaconShop_HERO_70`) — Armor 19 (Duos 17).
//!
//! Hero Power: `Kel'Thuzad's Kitty` (`TB_BaconShop_HP_080`), 0 Gold: After another hero dies, Discover a minion from their warband. It keeps enchantments.
//!
//! Passive (`hero_turn_start`): for each opponent in `state.lobby.opponents` with
//! `died_last_combat == true` (in slice order), if their `last_warband` is non-empty, samples up
//! to 3 random minions without replacement via `rng.below`, keeping their stats, Golden status,
//! keywords, and permanent enchantments (with opponent-specific `aura_applied` cleared and
//! Bigglesworth's global auras applied), and offers a Discover. If `last_warband` is empty, adds
//! a `Consolation Coin` (`TB_BaconShop_HP_080t`, 0-Cost token spell: Gain 1 Gold) to hand instead.

use crate::cards::{self, tokens, CardFlags, CardHooks};
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 63319;
pub const POWER: CardId = 63320;

/// Discover options per dead opponent.
const OPTIONS: usize = 3;

/// The Hero Power's hooks (and `Consolation Coin`'s), for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![
        (
            POWER,
            CardHooks::EMPTY
                .on_hero_turn_start(on_turn_start)
                .on_hero_option_chosen(option_chosen),
        ),
        (
            tokens::SPELL_CONSOLATION_COIN,
            CardHooks::EMPTY
                .with_flags(CardFlags::NOT_IN_POOL | CardFlags::NOT_TAVERN_SPELL)
                .on_cast(|state, _, _, _, _| {
                    state.gold += 1;
                }),
        ),
    ]
}

/// Start of turn: for each opponent that died last combat, Discover from their warband (or get a
/// `Consolation Coin` if their warband was empty).
fn on_turn_start(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    let dead_warbands: Vec<Vec<Unit>> = state
        .lobby
        .opponents
        .iter()
        .filter(|opp| opp.died_last_combat)
        .map(|opp| opp.last_warband.clone())
        .collect();
    for mut candidates in dead_warbands {
        if candidates.is_empty() {
            state.add_to_hand(tokens::make_consolation_coin());
            continue;
        }
        let mut options = Vec::new();
        while options.len() < OPTIONS && !candidates.is_empty() {
            let pick = match candidates.len() {
                1 => 0,
                k => rng.below(k),
            };
            let mut opt = candidates.remove(pick);
            opt.id = 0;
            opt.marked = false;
            opt.frozen = false;
            opt.activated_this_turn = false;
            cards::on_start_turn(&mut opt);
            opt.dies_on_play_this_turn = false;
            opt.willbreaker_group = 0;
            opt.willbreaker_remaining = 0;
            opt.aura_applied = (0, 0);
            opt.sync_max_stats();
            state.apply_global_unit_auras(&mut opt);
            opt.to_hero_power = true;
            opt.taught_spell_id = Some(POWER);
            options.push(opt);
        }
        state.push_discover(options);
    }
}

/// Choosing an option: add the enchanted minion to hand (without returning unchosen options to
/// `pool`).
fn option_chosen(
    state: &mut TavernState,
    mut chosen: Unit,
    _: Vec<Unit>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if chosen.taught_spell_id == Some(POWER) {
        chosen.taught_spell_id = None;
    }
    if super::hand_has_room(state) {
        let obtained = chosen.clone();
        state.add_to_hand(chosen);
        cards::on_obtained(state, &obtained, pool, rng);
    }
    cards::on_card_discovered(state, pool, rng);
}
