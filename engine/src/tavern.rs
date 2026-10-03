//! Card-agnostic Tavern Phase (Recruit Phase) state machine (`docs/tavern.md`).

use crate::cards::{self, tokens, CardTemplate};
use crate::model::{CardId, DeityKind, PlayerAuras, Tribe, Unit};
use crate::rng::Rng;

/// Base copies per unique minion in the shared pool by Tavern Tier (`docs/tavern.md` §2).
pub fn base_copies_for_tier(tier: u32) -> u32 {
    match tier {
        1 => 15,
        2 => 15,
        3 => 13,
        4 => 11,
        5 => 9,
        6 => 7,
        _ => panic!("invalid tavern tier {tier} (must be 1..=6)"),
    }
}

/// Shop minion capacity by Tavern Tier (`docs/tavern.md` §3.3).
pub fn shop_capacity(tier: u32) -> usize {
    match tier {
        1 => 3,
        2 | 3 => 4,
        4 | 5 => 5,
        6 => 6,
        _ => panic!("invalid tavern tier {tier} (must be 1..=6)"),
    }
}

/// Base cost to upgrade from `current_tier` to `current_tier + 1` (`docs/tavern.md` §3.3).
pub fn base_upgrade_cost(current_tier: u32) -> u32 {
    match current_tier {
        1 => 5,
        2 => 7,
        3 => 8,
        4 => 10,
        5 => 10,
        6 => 0,
        _ => panic!("invalid tavern tier {current_tier} (must be 1..=6)"),
    }
}

#[derive(Clone, Debug)]
struct PoolEntry {
    template: CardTemplate,
    remaining: u32,
}

/// Shared finite card pool with depletion (`docs/tavern.md` §2).
#[derive(Clone, Debug)]
pub struct CardPool {
    entries: Vec<PoolEntry>,
}

impl CardPool {
    /// Construct a shared card pool from a list of templates.
    pub fn new(templates: Vec<CardTemplate>) -> Self {
        let entries = templates
            .into_iter()
            .map(|t| {
                let remaining = base_copies_for_tier(t.tavern_tier);
                PoolEntry {
                    template: t,
                    remaining,
                }
            })
            .collect();
        Self { entries }
    }

    /// Remaining copies in the pool for `card_id`.
    pub fn remaining_copies(&self, card_id: CardId) -> u32 {
        self.entries
            .iter()
            .find(|e| e.template.card_id == card_id)
            .map(|e| e.remaining)
            .unwrap_or(0)
    }

    /// Draw one minion with `tavern_tier <= max_tier`, weighted by remaining copies.
    pub fn draw_from_pool(&mut self, max_tier: u32, rng: &mut Rng) -> Option<Unit> {
        let total_copies: u32 = self
            .entries
            .iter()
            .filter(|e| e.template.tavern_tier <= max_tier && e.remaining > 0)
            .map(|e| e.remaining)
            .sum();

        if total_copies == 0 {
            return None;
        }

        let mut pick = rng.below(total_copies as usize) as u32;
        for entry in &mut self.entries {
            if entry.template.tavern_tier <= max_tier && entry.remaining > 0 {
                if pick < entry.remaining {
                    entry.remaining -= 1;
                    return Some(entry.template.instantiate());
                }
                pick -= entry.remaining;
            }
        }
        unreachable!("draw_from_pool pick must land inside total_copies");
    }

    /// Draw up to `count` distinct `card_id`s of exact `tier` (used for Triple Reward Discover).
    pub fn draw_discover_options(
        &mut self,
        exact_tier: u32,
        count: usize,
        rng: &mut Rng,
    ) -> Vec<Unit> {
        let mut options = Vec::with_capacity(count);
        let mut chosen_ids = Vec::with_capacity(count);

        for _ in 0..count {
            let total_copies: u32 = self
                .entries
                .iter()
                .filter(|e| {
                    e.template.tavern_tier == exact_tier
                        && e.remaining > 0
                        && !chosen_ids.contains(&e.template.card_id)
                })
                .map(|e| e.remaining)
                .sum();

            if total_copies == 0 {
                break;
            }

            let mut pick = rng.below(total_copies as usize) as u32;
            for entry in &mut self.entries {
                if entry.template.tavern_tier == exact_tier
                    && entry.remaining > 0
                    && !chosen_ids.contains(&entry.template.card_id)
                {
                    if pick < entry.remaining {
                        entry.remaining -= 1;
                        chosen_ids.push(entry.template.card_id);
                        options.push(entry.template.instantiate());
                        break;
                    }
                    pick -= entry.remaining;
                }
            }
        }
        options
    }

    /// Return a unit's copy (or 3 copies if tripled Golden) back to the shared card pool.
    pub fn return_unit(&mut self, unit: &Unit) {
        let copies = if unit.is_golden && !unit.intrinsic_golden {
            3
        } else {
            1
        };
        for entry in &mut self.entries {
            if entry.template.card_id == unit.card_id {
                let cap = base_copies_for_tier(entry.template.tavern_tier);
                entry.remaining = (entry.remaining + copies).min(cap);
                break;
            }
        }
    }
}

/// Discrete player actions during the Tavern Phase (`docs/tavern.md` §6).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TavernAction {
    Buy {
        shop_index: usize,
    },
    Play {
        hand_index: usize,
        board_pos: usize,
    },
    Sell {
        board_pos: usize,
    },
    Reposition {
        from_pos: usize,
        to_pos: usize,
    },
    Activate {
        board_pos: usize,
        target_pos: Option<usize>,
    },
    Refresh,
    UpgradeTavern,
    ToggleFreeze,
    ChooseDiscover {
        option_index: usize,
    },
    EndTurn,
}

/// Complete state of a player during the Tavern Phase (`docs/tavern.md` §3).
#[derive(Clone, Debug)]
pub struct TavernState {
    pub turn: u32,
    pub health: i32,
    pub tavern_tier: u32,
    pub max_gold: u32,
    pub gold: u32,
    pub bonus_gold_next_turn: u32,
    pub upgrade_cost: u32,
    pub is_frozen: bool,
    pub board: Vec<Unit>,
    pub hand: Vec<Unit>,
    pub shop: Vec<Unit>,
    pub discover_pending: Option<Vec<Unit>>,
    pub auras: PlayerAuras,
}

impl Default for TavernState {
    fn default() -> Self {
        Self::new()
    }
}

impl TavernState {
    pub fn new() -> Self {
        Self {
            turn: 0,
            health: 30,
            tavern_tier: 1,
            max_gold: 0,
            gold: 0,
            bonus_gold_next_turn: 0,
            upgrade_cost: base_upgrade_cost(1),
            is_frozen: false,
            board: Vec::new(),
            hand: Vec::new(),
            shop: Vec::new(),
            discover_pending: None,
            auras: PlayerAuras::default(),
        }
    }

    /// Apply Tavern-wide shop buffs (e.g. `Dune Dweller` Elemental buff) to a newly drawn shop minion.
    fn apply_shop_auras(&self, unit: &mut Unit) {
        if unit.tribe.matches(Tribe::Elemental)
            && (self.auras.tavern_elemental_atk != 0 || self.auras.tavern_elemental_hp != 0)
        {
            unit.add_stats(
                self.auras.tavern_elemental_atk,
                self.auras.tavern_elemental_hp,
            );
        }
    }

    /// Deal `amount` damage to the friendly hero during the Tavern Phase.
    pub fn deal_hero_damage(&mut self, amount: i32) {
        if amount <= 0 {
            return;
        }
        self.health -= amount;
    }

    /// Execute the start-of-turn sequence (`docs/tavern.md` §4).
    pub fn start_turn(&mut self, pool: &mut CardPool, rng: &mut Rng) {
        for u in &mut self.board {
            u.activated_this_turn = false;
        }

        self.turn += 1;
        self.max_gold = std::cmp::min(10, 2 + self.turn);
        self.gold = std::cmp::min(10, self.max_gold + self.bonus_gold_next_turn);
        self.bonus_gold_next_turn = 0;

        if self.turn > 1 && self.tavern_tier < 6 {
            self.upgrade_cost = self.upgrade_cost.saturating_sub(1);
        }

        let cap = shop_capacity(self.tavern_tier);
        if self.is_frozen {
            self.is_frozen = false;
            let needed = cap.saturating_sub(self.shop.len());
            for _ in 0..needed {
                if let Some(mut drawn) = pool.draw_from_pool(self.tavern_tier, rng) {
                    self.apply_shop_auras(&mut drawn);
                    self.shop.push(drawn);
                }
            }
        } else {
            for old in self.shop.drain(..) {
                pool.return_unit(&old);
            }
            for _ in 0..cap {
                if let Some(mut drawn) = pool.draw_from_pool(self.tavern_tier, rng) {
                    self.apply_shop_auras(&mut drawn);
                    self.shop.push(drawn);
                }
            }
        }
    }

    /// Build the player's board snapshot for Combat (including Start-of-Combat hand summons like `Flighty Scout`).
    pub fn combat_board(&self) -> Vec<Unit> {
        let mut b = self.board.clone();
        cards::tier1::flighty_scout::apply_start_of_combat_hand(&self.hand, &mut b);
        b
    }

    /// Check whether `action` is currently legal (`docs/tavern.md` §6).
    pub fn is_legal(&self, action: &TavernAction) -> bool {
        if let Some(ref opts) = self.discover_pending {
            return matches!(
                action,
                TavernAction::ChooseDiscover { option_index } if *option_index < opts.len()
            );
        }

        match *action {
            TavernAction::Buy { shop_index } => {
                self.gold >= 3 && shop_index < self.shop.len() && self.hand.len() < 10
            }
            TavernAction::Play {
                hand_index,
                board_pos,
            } => {
                let Some(card) = self.hand.get(hand_index) else {
                    return false;
                };
                if card.is_spell {
                    if card.card_id == tokens::SPELL_TAVERN_COIN {
                        board_pos == 0
                    } else {
                        board_pos < self.board.len()
                    }
                } else if card.magnetic
                    && board_pos < self.board.len()
                    && self.board[board_pos].tribe.matches(Tribe::Mech)
                {
                    true
                } else {
                    self.board.len() < 7 && board_pos <= self.board.len()
                }
            }
            TavernAction::Sell { board_pos } => board_pos < self.board.len(),
            TavernAction::Reposition { from_pos, to_pos } => {
                from_pos < self.board.len() && to_pos < self.board.len() && from_pos != to_pos
            }
            TavernAction::Activate {
                board_pos,
                target_pos,
            } => {
                let Some(unit) = self.board.get(board_pos) else {
                    return false;
                };
                if unit.activated_this_turn {
                    return false;
                }
                let Some(cost) = cards::activate_cost(unit.card_id) else {
                    return false;
                };
                if self.gold < cost {
                    return false;
                }
                if cards::activate_requires_other_target(unit.card_id) {
                    matches!(target_pos, Some(t) if t < self.board.len() && t != board_pos)
                } else {
                    target_pos.is_none()
                }
            }
            TavernAction::Refresh => self.gold >= 1,
            TavernAction::UpgradeTavern => self.tavern_tier < 6 && self.gold >= self.upgrade_cost,
            TavernAction::ToggleFreeze => true,
            TavernAction::ChooseDiscover { .. } => false,
            TavernAction::EndTurn => true,
        }
    }

    /// Return all currently legal `TavernAction`s.
    pub fn valid_actions(&self) -> Vec<TavernAction> {
        let mut actions = Vec::new();
        if let Some(ref opts) = self.discover_pending {
            for i in 0..opts.len() {
                actions.push(TavernAction::ChooseDiscover { option_index: i });
            }
            return actions;
        }

        if self.gold >= 3 && self.hand.len() < 10 {
            for i in 0..self.shop.len() {
                actions.push(TavernAction::Buy { shop_index: i });
            }
        }

        for (h_idx, card) in self.hand.iter().enumerate() {
            if card.is_spell {
                if card.card_id == tokens::SPELL_TAVERN_COIN {
                    actions.push(TavernAction::Play {
                        hand_index: h_idx,
                        board_pos: 0,
                    });
                } else {
                    for b_pos in 0..self.board.len() {
                        actions.push(TavernAction::Play {
                            hand_index: h_idx,
                            board_pos: b_pos,
                        });
                    }
                }
            } else if self.board.len() < 7 {
                for b_pos in 0..=self.board.len() {
                    actions.push(TavernAction::Play {
                        hand_index: h_idx,
                        board_pos: b_pos,
                    });
                }
            } else if card.magnetic {
                for (b_pos, target) in self.board.iter().enumerate() {
                    if target.tribe.matches(Tribe::Mech) {
                        actions.push(TavernAction::Play {
                            hand_index: h_idx,
                            board_pos: b_pos,
                        });
                    }
                }
            }
        }

        for b_pos in 0..self.board.len() {
            actions.push(TavernAction::Sell { board_pos: b_pos });
        }

        for from_pos in 0..self.board.len() {
            for to_pos in 0..self.board.len() {
                if from_pos != to_pos {
                    actions.push(TavernAction::Reposition { from_pos, to_pos });
                }
            }
        }

        for (b_pos, unit) in self.board.iter().enumerate() {
            if !unit.activated_this_turn {
                if let Some(cost) = cards::activate_cost(unit.card_id) {
                    if self.gold >= cost {
                        if cards::activate_requires_other_target(unit.card_id) {
                            for t_pos in 0..self.board.len() {
                                if t_pos != b_pos {
                                    actions.push(TavernAction::Activate {
                                        board_pos: b_pos,
                                        target_pos: Some(t_pos),
                                    });
                                }
                            }
                        } else {
                            actions.push(TavernAction::Activate {
                                board_pos: b_pos,
                                target_pos: None,
                            });
                        }
                    }
                }
            }
        }

        if self.gold >= 1 {
            actions.push(TavernAction::Refresh);
        }
        if self.tavern_tier < 6 && self.gold >= self.upgrade_cost {
            actions.push(TavernAction::UpgradeTavern);
        }
        actions.push(TavernAction::ToggleFreeze);
        actions.push(TavernAction::EndTurn);
        actions
    }

    /// Execute one `TavernAction`. Returns `Ok(true)` if the turn ended (`EndTurn`),
    /// `Ok(false)` otherwise, or `Err` if the action is illegal.
    pub fn step(
        &mut self,
        action: TavernAction,
        pool: &mut CardPool,
        rng: &mut Rng,
    ) -> Result<bool, String> {
        if !self.is_legal(&action) {
            return Err(format!("illegal tavern action: {action:?}"));
        }

        match action {
            TavernAction::Buy { shop_index } => {
                self.gold -= 3;
                let unit = self.shop.remove(shop_index);
                let cid = unit.card_id;
                self.hand.push(unit);
                self.check_and_resolve_triple(cid);
                Ok(false)
            }
            TavernAction::Play {
                hand_index,
                board_pos,
            } => {
                let mut card = self.hand.remove(hand_index);
                if card.is_spell {
                    self.auras.spells_played += 1;
                    if card.card_id == tokens::SPELL_TAVERN_COIN {
                        self.gold = (self.gold + 1).min(10);
                    } else {
                        self.board[board_pos].add_stats(card.attack, card.health);
                    }
                    return Ok(false);
                }

                let played_card_id = card.card_id;
                let played_tribe = card.tribe;
                let was_tripled_golden = card.is_golden && !card.intrinsic_golden;

                // Check for Magnetic fusion onto the Mech immediately to the right (`board[board_pos]`).
                if card.magnetic
                    && board_pos < self.board.len()
                    && self.board[board_pos].tribe.matches(Tribe::Mech)
                {
                    let target = &mut self.board[board_pos];
                    target.add_stats(card.attack, card.health);
                    target.taunt |= card.taunt;
                    target.divine_shield |= card.divine_shield;
                    target.inherent_divine_shield |= card.inherent_divine_shield;
                    target.windfury |= card.windfury;
                    target.reborn |= card.reborn;
                    target.venomous |= card.venomous;
                    target.stealth |= card.stealth;
                    target.magnetic |= card.magnetic;
                    target.eot_health_bonus += card.eot_health_bonus;
                    if card.card_id == cards::tier1::lullabot::ID {
                        target.eot_health_bonus += if card.is_golden { 2 } else { 1 };
                    }
                    pool.return_unit(&card);
                    cards::after_play_minion(self, played_card_id, played_tribe, board_pos);
                } else {
                    cards::on_play_battlecry(self, &mut card, rng);
                    self.board.insert(board_pos, card);
                    cards::after_play_minion(self, played_card_id, played_tribe, board_pos);
                }

                if was_tripled_golden {
                    let reward_tier = std::cmp::min(6, self.tavern_tier + 1);
                    let opts = pool.draw_discover_options(reward_tier, 3, rng);
                    if !opts.is_empty() {
                        self.discover_pending = Some(opts);
                    }
                }
                Ok(false)
            }
            TavernAction::Sell { board_pos } => {
                let sold = self.board.remove(board_pos);
                pool.return_unit(&sold);
                self.gold = std::cmp::min(10, self.gold + 1);
                cards::on_sell(self, &sold, pool, rng);
                Ok(false)
            }
            TavernAction::Reposition { from_pos, to_pos } => {
                let unit = self.board.remove(from_pos);
                self.board.insert(to_pos, unit);
                Ok(false)
            }
            TavernAction::Activate {
                board_pos,
                target_pos,
            } => {
                let cid = self.board[board_pos].card_id;
                let cost = cards::activate_cost(cid).unwrap_or(0);
                self.gold -= cost;
                self.board[board_pos].activated_this_turn = true;
                cards::on_activate(self, board_pos, target_pos);
                Ok(false)
            }
            TavernAction::Refresh => {
                self.gold -= 1;
                self.is_frozen = false;
                for old in self.shop.drain(..) {
                    pool.return_unit(&old);
                }
                let cap = shop_capacity(self.tavern_tier);
                for _ in 0..cap {
                    if let Some(mut drawn) = pool.draw_from_pool(self.tavern_tier, rng) {
                        self.apply_shop_auras(&mut drawn);
                        self.shop.push(drawn);
                    }
                }
                Ok(false)
            }
            TavernAction::UpgradeTavern => {
                self.gold -= self.upgrade_cost;
                self.tavern_tier += 1;
                self.upgrade_cost = base_upgrade_cost(self.tavern_tier);
                Ok(false)
            }
            TavernAction::ToggleFreeze => {
                self.is_frozen = !self.is_frozen;
                Ok(false)
            }
            TavernAction::ChooseDiscover { option_index } => {
                let mut opts = self
                    .discover_pending
                    .take()
                    .expect("discover_pending verified by is_legal");
                let chosen = opts.remove(option_index);
                for unchosen in &opts {
                    pool.return_unit(unchosen);
                }
                let cid = chosen.card_id;
                self.hand.push(chosen);
                self.check_and_resolve_triple(cid);
                Ok(false)
            }
            TavernAction::EndTurn => {
                cards::on_end_turn(self);
                Ok(true)
            }
        }
    }

    /// Combine 3 non-golden copies of `card_id` across `board` and `hand` into 1 Golden copy in `hand` (`docs/tavern.md` §5.1).
    pub fn check_and_resolve_triple(&mut self, card_id: CardId) {
        if card_id == 0 {
            return;
        }
        let count_board = self
            .board
            .iter()
            .filter(|u| !u.is_golden && !u.intrinsic_golden && !u.is_spell && u.card_id == card_id)
            .count();
        let count_hand = self
            .hand
            .iter()
            .filter(|u| !u.is_golden && !u.intrinsic_golden && !u.is_spell && u.card_id == card_id)
            .count();

        if count_board + count_hand < 3 {
            return;
        }

        let mut copies = Vec::with_capacity(3);
        self.board.retain(|u| {
            if copies.len() < 3
                && !u.is_golden
                && !u.intrinsic_golden
                && !u.is_spell
                && u.card_id == card_id
            {
                copies.push(u.clone());
                false
            } else {
                true
            }
        });
        self.hand.retain(|u| {
            if copies.len() < 3
                && !u.is_golden
                && !u.intrinsic_golden
                && !u.is_spell
                && u.card_id == card_id
            {
                copies.push(u.clone());
                false
            } else {
                true
            }
        });

        debug_assert_eq!(copies.len(), 3);
        let base_atk = copies[0].base_attack;
        let base_hp = copies[0].base_health;
        let tier = copies[0].tavern_tier;
        let tribe = copies[0].tribe;
        let name = format!("Golden {}", copies[0].name);

        let atk_buffs: i32 = copies.iter().map(|u| u.attack - base_atk).sum();
        let hp_buffs: i32 = copies.iter().map(|u| u.health - base_hp).sum();

        let golden_atk = 2 * base_atk + atk_buffs;
        let golden_hp = 2 * base_hp + hp_buffs;

        let mut golden = Unit::new(name, golden_atk, golden_hp)
            .with_card_id(card_id)
            .with_tavern_tier(tier)
            .with_tribe(tribe)
            .with_golden(true);
        golden.base_attack = 2 * base_atk;
        golden.base_health = 2 * base_hp;
        golden.taunt = copies.iter().any(|u| u.taunt);
        golden.divine_shield = copies.iter().any(|u| u.divine_shield);
        golden.inherent_divine_shield = copies.iter().any(|u| u.inherent_divine_shield);
        golden.windfury = copies.iter().any(|u| u.windfury);
        golden.reborn = copies.iter().any(|u| u.reborn);
        golden.venomous = copies.iter().any(|u| u.venomous);
        golden.stealth = copies.iter().any(|u| u.stealth);
        golden.magnetic = copies.iter().any(|u| u.magnetic);
        golden.threshold_triggered = copies.iter().any(|u| u.threshold_triggered);
        cards::check_stat_thresholds(&mut golden);

        self.hand.push(golden);
    }
}

// ---------------------------------------------------------------------------
// Declarative YAML Tavern Scenario Runner (`docs/scenarios.md` Part 2)
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TavernScenario {
    pub name: String,
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub catalog: Option<String>,
    #[serde(with = "serde_yaml::with::singleton_map_recursive")]
    pub steps: Vec<TavernStepSpec>,
    pub expect: TavernExpectSpec,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TavernStepSpec {
    StartTurn,
    SetGold(u32),
    SetDeity(DeityKind),
    GiveHand(String),
    GiveBoard(String),
    Buy(usize),
    Play { hand: usize, pos: usize },
    Sell(usize),
    Reposition { from: usize, to: usize },
    Activate { pos: usize, target: Option<usize> },
    Refresh,
    UpgradeTavern,
    ToggleFreeze,
    ChooseDiscover(usize),
    EndTurn,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TavernExpectSpec {
    pub turn: Option<u32>,
    pub tavern_tier: Option<u32>,
    pub gold: Option<u32>,
    pub health: Option<i32>,
    pub bonus_gold_next_turn: Option<u32>,
    pub upgrade_cost: Option<u32>,
    pub is_frozen: Option<bool>,
    pub board_count: Option<usize>,
    pub hand_count: Option<usize>,
    pub shop_count: Option<usize>,
    pub board_has_golden: Option<bool>,
    pub discover_pending: Option<bool>,
    pub board_first_stats: Option<[i32; 2]>,
    pub board_contains_name: Option<String>,
    pub deity_stats: Option<[i32; 2]>,
}

pub fn run_tavern_scenario(scenario: &TavernScenario) -> Result<(), String> {
    let catalog_name = scenario.catalog.as_deref().unwrap_or("tier1");
    let templates = cards::catalog_for(catalog_name)?;
    let mut pool = CardPool::new(templates.clone());
    let mut rng = Rng::new(scenario.seed);
    let mut state = TavernState::new();

    for (idx, step) in scenario.steps.iter().enumerate() {
        match step {
            TavernStepSpec::StartTurn => state.start_turn(&mut pool, &mut rng),
            TavernStepSpec::SetGold(g) => state.gold = *g,
            TavernStepSpec::SetDeity(kind) => state.auras.deity.kind = *kind,
            TavernStepSpec::GiveHand(ref card_name) => {
                let tpl = templates
                    .iter()
                    .find(|t| t.name == *card_name)
                    .ok_or_else(|| format!("[{}] unknown card {card_name:?}", scenario.name))?;
                let unit = tpl.instantiate();
                let cid = unit.card_id;
                state.hand.push(unit);
                state.check_and_resolve_triple(cid);
            }
            TavernStepSpec::GiveBoard(ref card_name) => {
                let tpl = templates
                    .iter()
                    .find(|t| t.name == *card_name)
                    .ok_or_else(|| format!("[{}] unknown card {card_name:?}", scenario.name))?;
                state.board.push(tpl.instantiate());
            }
            TavernStepSpec::Buy(shop_index) => {
                state
                    .step(
                        TavernAction::Buy {
                            shop_index: *shop_index,
                        },
                        &mut pool,
                        &mut rng,
                    )
                    .map_err(|e| format!("[{}] step {idx}: {e}", scenario.name))?;
            }
            TavernStepSpec::Play { hand, pos } => {
                state
                    .step(
                        TavernAction::Play {
                            hand_index: *hand,
                            board_pos: *pos,
                        },
                        &mut pool,
                        &mut rng,
                    )
                    .map_err(|e| format!("[{}] step {idx}: {e}", scenario.name))?;
            }
            TavernStepSpec::Sell(board_pos) => {
                state
                    .step(
                        TavernAction::Sell {
                            board_pos: *board_pos,
                        },
                        &mut pool,
                        &mut rng,
                    )
                    .map_err(|e| format!("[{}] step {idx}: {e}", scenario.name))?;
            }
            TavernStepSpec::Reposition { from, to } => {
                state
                    .step(
                        TavernAction::Reposition {
                            from_pos: *from,
                            to_pos: *to,
                        },
                        &mut pool,
                        &mut rng,
                    )
                    .map_err(|e| format!("[{}] step {idx}: {e}", scenario.name))?;
            }
            TavernStepSpec::Activate { pos, target } => {
                state
                    .step(
                        TavernAction::Activate {
                            board_pos: *pos,
                            target_pos: *target,
                        },
                        &mut pool,
                        &mut rng,
                    )
                    .map_err(|e| format!("[{}] step {idx}: {e}", scenario.name))?;
            }
            TavernStepSpec::Refresh => {
                state
                    .step(TavernAction::Refresh, &mut pool, &mut rng)
                    .map_err(|e| format!("[{}] step {idx}: {e}", scenario.name))?;
            }
            TavernStepSpec::UpgradeTavern => {
                state
                    .step(TavernAction::UpgradeTavern, &mut pool, &mut rng)
                    .map_err(|e| format!("[{}] step {idx}: {e}", scenario.name))?;
            }
            TavernStepSpec::ToggleFreeze => {
                state
                    .step(TavernAction::ToggleFreeze, &mut pool, &mut rng)
                    .map_err(|e| format!("[{}] step {idx}: {e}", scenario.name))?;
            }
            TavernStepSpec::ChooseDiscover(option_index) => {
                state
                    .step(
                        TavernAction::ChooseDiscover {
                            option_index: *option_index,
                        },
                        &mut pool,
                        &mut rng,
                    )
                    .map_err(|e| format!("[{}] step {idx}: {e}", scenario.name))?;
            }
            TavernStepSpec::EndTurn => {
                state
                    .step(TavernAction::EndTurn, &mut pool, &mut rng)
                    .map_err(|e| format!("[{}] step {idx}: {e}", scenario.name))?;
            }
        }
    }

    let exp = &scenario.expect;
    if let Some(turn) = exp.turn {
        if state.turn != turn {
            return Err(format!(
                "[{}] expected turn {turn}, got {}",
                scenario.name, state.turn
            ));
        }
    }
    if let Some(tier) = exp.tavern_tier {
        if state.tavern_tier != tier {
            return Err(format!(
                "[{}] expected tavern_tier {tier}, got {}",
                scenario.name, state.tavern_tier
            ));
        }
    }
    if let Some(gold) = exp.gold {
        if state.gold != gold {
            return Err(format!(
                "[{}] expected gold {gold}, got {}",
                scenario.name, state.gold
            ));
        }
    }
    if let Some(hp) = exp.health {
        if state.health != hp {
            return Err(format!(
                "[{}] expected health {hp}, got {}",
                scenario.name, state.health
            ));
        }
    }
    if let Some(bonus) = exp.bonus_gold_next_turn {
        if state.bonus_gold_next_turn != bonus {
            return Err(format!(
                "[{}] expected bonus_gold_next_turn {bonus}, got {}",
                scenario.name, state.bonus_gold_next_turn
            ));
        }
    }
    if let Some(cost) = exp.upgrade_cost {
        if state.upgrade_cost != cost {
            return Err(format!(
                "[{}] expected upgrade_cost {cost}, got {}",
                scenario.name, state.upgrade_cost
            ));
        }
    }
    if let Some(frozen) = exp.is_frozen {
        if state.is_frozen != frozen {
            return Err(format!(
                "[{}] expected is_frozen {frozen}, got {}",
                scenario.name, state.is_frozen
            ));
        }
    }
    if let Some(bc) = exp.board_count {
        if state.board.len() != bc {
            return Err(format!(
                "[{}] expected board_count {bc}, got {}",
                scenario.name,
                state.board.len()
            ));
        }
    }
    if let Some(hc) = exp.hand_count {
        if state.hand.len() != hc {
            return Err(format!(
                "[{}] expected hand_count {hc}, got {}",
                scenario.name,
                state.hand.len()
            ));
        }
    }
    if let Some(sc) = exp.shop_count {
        if state.shop.len() != sc {
            return Err(format!(
                "[{}] expected shop_count {sc}, got {}",
                scenario.name,
                state.shop.len()
            ));
        }
    }
    if let Some(has_golden) = exp.board_has_golden {
        let actual = state.board.iter().any(|u| u.is_golden);
        if actual != has_golden {
            return Err(format!(
                "[{}] expected board_has_golden {has_golden}, got {actual}",
                scenario.name
            ));
        }
    }
    if let Some(dp) = exp.discover_pending {
        if state.discover_pending.is_some() != dp {
            return Err(format!(
                "[{}] expected discover_pending {dp}, got {}",
                scenario.name,
                state.discover_pending.is_some()
            ));
        }
    }
    if let Some([atk, hp]) = exp.board_first_stats {
        let first = state
            .board
            .first()
            .ok_or_else(|| format!("[{}] expected board[0], board is empty", scenario.name))?;
        if first.attack != atk || first.health != hp {
            return Err(format!(
                "[{}] expected board[0] stats {atk}/{hp}, got {}/{}",
                scenario.name, first.attack, first.health
            ));
        }
    }
    if let Some(ref want_name) = exp.board_contains_name {
        if !state.board.iter().any(|u| u.name == *want_name) {
            return Err(format!(
                "[{}] expected board to contain {want_name:?}",
                scenario.name
            ));
        }
    }
    if let Some([d_atk, d_hp]) = exp.deity_stats {
        let actual = (state.auras.deity.attack, state.auras.deity.health);
        if actual != (d_atk, d_hp) {
            return Err(format!(
                "[{}] expected deity_stats {d_atk}/{d_hp}, got {}/{}",
                scenario.name, actual.0, actual.1
            ));
        }
    }

    Ok(())
}
