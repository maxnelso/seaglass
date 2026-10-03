//! Card templates, catalogs, and modular per-card hook dispatch.
//!
//! Each card in the catalog lives in its own module under `src/cards/tier1/`,
//! while Deities live in `src/cards/deities.rs` and generated tokens/spells live
//! in `src/cards/tokens.rs`.

pub mod deities;
pub mod tier1;
pub mod tokens;

use crate::model::{CardId, Keyword, PlayerAuras, Tribe, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Static definition of a purchasable minion in the shared card pool.
#[derive(Clone, Debug)]
pub struct CardTemplate {
    pub card_id: CardId,
    pub name: String,
    pub attack: i32,
    pub health: i32,
    pub tavern_tier: u32,
    pub tribe: Tribe,
    pub taunt: bool,
    pub divine_shield: bool,
    pub windfury: bool,
    pub reborn: bool,
    pub venomous: bool,
    pub stealth: bool,
    pub magnetic: bool,
    pub intrinsic_golden: bool,
    pub activate_cost: Option<u32>,
}

impl CardTemplate {
    pub fn new(
        card_id: CardId,
        name: impl Into<String>,
        attack: i32,
        health: i32,
        tavern_tier: u32,
    ) -> Self {
        Self {
            card_id,
            name: name.into(),
            attack,
            health,
            tavern_tier,
            tribe: Tribe::None,
            taunt: false,
            divine_shield: false,
            windfury: false,
            reborn: false,
            venomous: false,
            stealth: false,
            magnetic: false,
            intrinsic_golden: false,
            activate_cost: None,
        }
    }

    pub fn with_tribe(mut self, tribe: Tribe) -> Self {
        self.tribe = tribe;
        self
    }

    pub fn with_keyword(mut self, kw: Keyword) -> Self {
        match kw {
            Keyword::Taunt => self.taunt = true,
            Keyword::DivineShield => self.divine_shield = true,
            Keyword::Windfury => self.windfury = true,
            Keyword::Reborn => self.reborn = true,
            Keyword::Venomous => self.venomous = true,
            Keyword::Stealth => self.stealth = true,
            Keyword::Magnetic => self.magnetic = true,
        }
        self
    }

    pub fn with_intrinsic_golden(mut self) -> Self {
        self.intrinsic_golden = true;
        self
    }

    pub fn with_activate_cost(mut self, cost: u32) -> Self {
        self.activate_cost = Some(cost);
        self
    }

    /// Return the printed keywords on this template.
    pub fn keywords(&self) -> Vec<Keyword> {
        let mut kws = Vec::new();
        if self.taunt {
            kws.push(Keyword::Taunt);
        }
        if self.divine_shield {
            kws.push(Keyword::DivineShield);
        }
        if self.windfury {
            kws.push(Keyword::Windfury);
        }
        if self.reborn {
            kws.push(Keyword::Reborn);
        }
        if self.venomous {
            kws.push(Keyword::Venomous);
        }
        if self.stealth {
            kws.push(Keyword::Stealth);
        }
        if self.magnetic {
            kws.push(Keyword::Magnetic);
        }
        kws
    }

    /// Instantiate a fresh [`Unit`] from this template.
    pub fn instantiate(&self) -> Unit {
        let mut u = Unit::new(self.name.clone(), self.attack, self.health)
            .with_card_id(self.card_id)
            .with_tavern_tier(self.tavern_tier)
            .with_tribe(self.tribe);
        u.taunt = self.taunt;
        u.divine_shield = self.divine_shield;
        u.inherent_divine_shield = self.divine_shield;
        u.windfury = self.windfury;
        u.reborn = self.reborn;
        u.venomous = self.venomous;
        u.stealth = self.stealth;
        u.magnetic = self.magnetic;
        if self.intrinsic_golden {
            u.is_golden = true;
            u.intrinsic_golden = true;
        }
        u
    }
}

/// All 21 active Solo Tier 1 minions (Patch 36.6.3, excluding rotated Naga & Dark Paradox).
pub fn tier1_catalog() -> Vec<CardTemplate> {
    tier1::catalog()
}

/// Alias for [`tier1_catalog`].
pub fn solo_tier_1_catalog() -> Vec<CardTemplate> {
    tier1_catalog()
}

/// Full active catalog (currently Solo Tier 1).
pub fn full_catalog() -> Vec<CardTemplate> {
    tier1_catalog()
}

/// Alias for [`full_catalog`].
pub fn solo_full_catalog() -> Vec<CardTemplate> {
    full_catalog()
}

/// Minimal test catalog for generic economy scenarios.
pub fn default_test_catalog() -> Vec<CardTemplate> {
    tier1_catalog()
}

/// Resolve a catalog name to its templates.
pub fn catalog_for(name: &str) -> Result<Vec<CardTemplate>, String> {
    match name {
        "tier1" | "solo_tier_1" | "full" | "solo_full" | "test" => Ok(tier1_catalog()),
        other => Err(format!(
            "unknown catalog {other:?}; expected one of: tier1, solo_tier_1, full, solo_full, test"
        )),
    }
}

// ---------------------------------------------------------------------------
// Tavern & Combat Hook Dispatch
// ---------------------------------------------------------------------------

/// Run stat-threshold checks whenever a unit's stats change (in Tavern or Combat).
pub fn check_stat_thresholds(unit: &mut Unit) {
    tier1::check_stat_thresholds(unit);
}

/// Apply on-play Battlecry when `unit` is played from `hand` onto `board`.
pub fn on_play_battlecry(state: &mut TavernState, unit: &mut Unit, rng: &mut Rng) {
    tier1::on_play_battlecry(state, unit, rng);
}

/// Apply board-wide observers after a minion of `(played_card_id, played_tribe)` is placed on `board`.
pub fn after_play_minion(
    state: &mut TavernState,
    played_card_id: CardId,
    played_tribe: Tribe,
    board_pos: usize,
) {
    tier1::after_play_minion(state, played_card_id, played_tribe, board_pos);
}

/// Apply On-Sell triggers when `sold` is sold from `board`.
pub fn on_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    tier1::on_sell(state, sold, pool, rng);
}

/// Apply End-of-Turn triggers across `board` and `hand` when `EndTurn` is taken.
pub fn on_end_turn(state: &mut TavernState) {
    tier1::on_end_turn(state);
}

/// Returns the Gold cost of a minion's `Activate` ability, if it has one.
pub fn activate_cost(card_id: CardId) -> Option<u32> {
    tier1::activate_cost(card_id)
}

/// Returns `true` if the `Activate` ability of `card_id` requires a target friendly minion.
pub fn activate_requires_other_target(card_id: CardId) -> bool {
    tier1::activate_requires_other_target(card_id)
}

/// Execute a minion's `Activate` ability on `board`.
pub fn on_activate(state: &mut TavernState, source_pos: usize, target_pos: Option<usize>) {
    tier1::on_activate(state, source_pos, target_pos);
}

/// Resolve a minion's On-Attack (`Rally`) effect during combat.
/// Returns any token(s) to be summoned immediately to the attacker's right.
pub fn on_rally(attacker: &mut Unit) -> Vec<Unit> {
    tier1::on_rally(attacker)
}

/// Resolve a dying minion's `Deathrattle` during combat.
/// Returns any token(s) to be summoned at the dying unit's slot.
pub fn on_deathrattle(
    dying: &Unit,
    auras: &PlayerAuras,
    dead_aberrations: &[Unit],
) -> Vec<Unit> {
    if dying.card_id == deities::CARD_YSHAARJ {
        return deities::yshaarj_deathrattle_summons(dying.is_golden, dead_aberrations);
    }
    tier1::on_deathrattle(dying, auras)
}

/// Update dynamic combat auras that depend on total friendly minion deaths this combat (`Rot Hide Gnoll`).
pub fn sync_friendly_death_auras(board: &mut [Unit], friendly_deaths_this_combat: u32) {
    tier1::sync_friendly_death_auras(board, friendly_deaths_this_combat);
}
