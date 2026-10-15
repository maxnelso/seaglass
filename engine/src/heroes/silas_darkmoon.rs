//! `Silas Darkmoon` (`TB_BaconShop_HERO_90`) — Armor 16 (Duos 14).
//!
//! Hero Power: `Come One, Come All!` (`TB_BaconShop_HP_101`), 0 Gold: Darkmoon Tickets are in the Tavern! Get 3 to Discover a minion of your Tier.
//!
//! Passive. Each time the Tavern is rolled (a Refresh, an unfrozen turn start: `hero_after_refresh`)
//! each of its minions without a ticket gets one with chance [`DARKMOON_TICKET_CHANCE`]
//! (`Unit::marked`; one `rng.below(den)` per minion; engine decision). Buying a minion with a
//! ticket gives the ticket (`counter`); every 3rd gives a Discover among 3 minions of exactly the
//! player's Tier from the pool, and the count starts again (confirmed).

use crate::cards::CardHooks;
use crate::heroes::power_mut;
use crate::model::{CardId, HeroId, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

pub const ID: HeroId = 64480;
pub const POWER: CardId = 64481;

/// A rolled minion's chance to carry a ticket: `(numerator, denominator)`.
pub const DARKMOON_TICKET_CHANCE: (usize, usize) = (1, 3);

/// Tickets per Discover.
const TICKETS: i32 = 3;

/// Discover options.
const OPTIONS: usize = 3;

/// The Hero Power's hooks, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    vec![(
        POWER,
        CardHooks::EMPTY
            .on_hero_after_refresh(place_tickets)
            .on_hero_after_buy(after_buy),
    )]
}

/// The Tavern was rolled: each minion without a ticket may get one.
fn place_tickets(state: &mut TavernState, _: &mut CardPool, rng: &mut Rng) {
    let (num, den) = DARKMOON_TICKET_CHANCE;
    for minion in state.shop.iter_mut().filter(|u| !u.is_spell && !u.marked) {
        minion.marked = rng.below(den) < num;
    }
}

/// After a card is bought: a ticket counts; every 3rd, a Discover.
fn after_buy(state: &mut TavernState, bought: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    if !bought.marked {
        return;
    }
    for card in &mut state.hand {
        card.marked = false;
    }
    let Some(power) = power_mut(&mut state.auras) else {
        return;
    };
    power.counter += 1;
    if power.counter < TICKETS {
        return;
    }
    power.counter -= TICKETS;
    let options = pool.draw_discover_options(state.tavern_tier, OPTIONS, rng);
    state.push_discover(options);
}
