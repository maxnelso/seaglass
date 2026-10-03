//! Card-agnostic Tavern Phase (Recruit Phase) state machine (`docs/tavern.md`).

use crate::cards::tokens::{
    self, CHOICE_ALLIANCE_ATK, CHOICE_ALLIANCE_HP, CHOICE_BOTANIST_ATK, CHOICE_BOTANIST_HP,
    CHOICE_CRATER_GEMS, CHOICE_CRATER_GEM_DAY, CHOICE_FOODIE_BUFF_GEMS, CHOICE_FOODIE_GET_GEMS,
    CHOICE_GEM_DAY_ATK, CHOICE_GEM_DAY_HP, CHOICE_SCARAB_REBORN, CHOICE_SCARAB_WINDFURY,
    CHOICE_SLY_GEMS, CHOICE_SLY_REFRESHES, CHOICE_TIME_MGMT_LATER, CHOICE_TIME_MGMT_NOW,
};
use crate::cards::{self, ActivateTargetKind, CardTemplate};
use crate::combat::{resolve_battle, BattleResult};
use crate::model::{BattleOutcome, CardId, DeityKind, GameState, PlayerAuras, Tribe, Unit};
use crate::rng::Rng;

/// Maximum number of minions allowed on a player's board.
pub const MAX_BOARD_SIZE: usize = 7;

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

    /// Draw one minion matching `tribe` with `tavern_tier <= max_tier` (excluding `exclude_card_id`),
    /// weighted by remaining copies (`Tad`, `Chef's Choice`).
    pub fn draw_by_tribe(
        &mut self,
        tribe: Tribe,
        exclude_card_id: Option<CardId>,
        max_tier: u32,
        rng: &mut Rng,
    ) -> Option<Unit> {
        if tribe == Tribe::None {
            return None;
        }
        let total_copies: u32 = self
            .entries
            .iter()
            .filter(|e| {
                e.template.tavern_tier <= max_tier
                    && e.remaining > 0
                    && e.template.tribe.matches(tribe)
                    && Some(e.template.card_id) != exclude_card_id
            })
            .map(|e| e.remaining)
            .sum();

        if total_copies == 0 {
            return None;
        }

        let mut pick = rng.below(total_copies as usize) as u32;
        for entry in &mut self.entries {
            if entry.template.tavern_tier <= max_tier
                && entry.remaining > 0
                && entry.template.tribe.matches(tribe)
                && Some(entry.template.card_id) != exclude_card_id
            {
                if pick < entry.remaining {
                    entry.remaining -= 1;
                    return Some(entry.template.instantiate());
                }
                pick -= entry.remaining;
            }
        }
        unreachable!("draw_by_tribe pick must land inside total_copies");
    }

    /// Draw up to `count` distinct `card_id`s of exact `tier` (used for Triple Reward Discover).
    /// Falls back to the highest available tier `<= exact_tier` if `exact_tier` has no entries in the pool.
    pub fn draw_discover_options(
        &mut self,
        exact_tier: u32,
        count: usize,
        rng: &mut Rng,
    ) -> Vec<Unit> {
        let target_tier = if self
            .entries
            .iter()
            .any(|e| e.template.tavern_tier == exact_tier && e.remaining > 0)
        {
            exact_tier
        } else {
            self.entries
                .iter()
                .filter(|e| e.template.tavern_tier <= exact_tier && e.remaining > 0)
                .map(|e| e.template.tavern_tier)
                .max()
                .unwrap_or(exact_tier)
        };

        let mut options = Vec::with_capacity(count);
        let mut chosen_ids = Vec::with_capacity(count);

        for _ in 0..count {
            let total_copies: u32 = self
                .entries
                .iter()
                .filter(|e| {
                    e.template.tavern_tier == target_tier
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
                if entry.template.tavern_tier == target_tier
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

    /// Draw up to `count` distinct `card_id`s matching `tribe` with `tavern_tier <= max_tier` (`Planar Telescope`).
    pub fn draw_discover_by_tribe(
        &mut self,
        tribe: Tribe,
        max_tier: u32,
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
                    e.template.tavern_tier <= max_tier
                        && e.remaining > 0
                        && e.template.tribe.matches(tribe)
                        && !chosen_ids.contains(&e.template.card_id)
                })
                .map(|e| e.remaining)
                .sum();

            if total_copies == 0 {
                break;
            }

            let mut pick = rng.below(total_copies as usize) as u32;
            for entry in &mut self.entries {
                if entry.template.tavern_tier <= max_tier
                    && entry.remaining > 0
                    && entry.template.tribe.matches(tribe)
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
        if unit.is_spell {
            return;
        }
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
    pub discover_queue: Vec<Vec<Unit>>,
    pub pending_choice_target: Option<usize>,
    pub next_willbreaker_group: u32,
    pub last_combat_won: bool,
    pub include_shop_spells: bool,
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
            discover_queue: Vec::new(),
            pending_choice_target: None,
            next_willbreaker_group: 1,
            last_combat_won: false,
            include_shop_spells: false,
            auras: PlayerAuras::default(),
        }
    }

    /// Enable or disable automatic rolling of 1 Tavern Spell into `shop` on `start_turn` / `Refresh`.
    pub fn with_shop_spells(mut self, enabled: bool) -> Self {
        self.include_shop_spells = enabled;
        self
    }

    /// Enqueue a Discover or Choose-One prompt (`discover_pending`).
    pub fn push_discover(&mut self, opts: Vec<Unit>) {
        if opts.is_empty() {
            return;
        }
        if self.discover_pending.is_none() {
            self.discover_pending = Some(opts);
        } else {
            self.discover_queue.push(opts);
        }
    }

    /// Apply global "wherever they are" auras to a single unit.
    pub fn apply_global_unit_auras(&self, unit: &mut Unit) {
        cards::sync_unit_auras(unit, &self.auras);
    }

    /// Apply Tavern-wide shop buffs (`Dune Dweller`, `Staff of Enrichment`, + global unit auras) to a newly drawn shop minion.
    pub fn apply_shop_auras(&self, unit: &mut Unit) {
        if unit.is_spell {
            return;
        }
        self.apply_global_unit_auras(unit);
        if unit.tribe.matches(Tribe::Elemental)
            && (self.auras.tavern_elemental_atk != 0 || self.auras.tavern_elemental_hp != 0)
        {
            unit.add_stats(
                self.auras.tavern_elemental_atk,
                self.auras.tavern_elemental_hp,
            );
        }
        if self.auras.tavern_all_atk != 0 || self.auras.tavern_all_hp != 0 {
            unit.add_stats(self.auras.tavern_all_atk, self.auras.tavern_all_hp);
        }
    }

    /// Synchronize all persistent "wherever they are" auras across `board`, `hand`, and `shop`.
    pub fn sync_all_auras(&mut self) {
        for u in &mut self.board {
            cards::sync_unit_auras(u, &self.auras);
        }
        for u in &mut self.hand {
            cards::sync_unit_auras(u, &self.auras);
        }
        for u in &mut self.shop {
            cards::sync_unit_auras(u, &self.auras);
        }
    }

    /// Add a card (minion or spell) to `hand` (if `hand.len() < 10`), syncing global auras and checking triples.
    pub fn add_to_hand(&mut self, mut card: Unit) {
        if self.hand.len() >= 10 {
            return;
        }
        self.apply_global_unit_auras(&mut card);
        let cid = card.card_id;
        let is_spell = card.is_spell;
        self.hand.push(card);
        if !is_spell {
            self.check_and_resolve_triple(cid);
        }
    }

    /// Discard the card at `hand[hand_idx]`, returning non-spells to `pool` and firing `cards::on_discard_hand_card`.
    pub fn discard_hand_card(
        &mut self,
        hand_idx: usize,
        pool: &mut CardPool,
        rng: &mut Rng,
    ) -> Option<Unit> {
        if hand_idx >= self.hand.len() {
            return None;
        }
        let discarded = self.hand.remove(hand_idx);
        pool.return_unit(&discarded);
        cards::on_discard_hand_card(self, &discarded, pool, rng);
        Some(discarded)
    }

    /// Resolve a Choose-One prompt (`opt0` vs `opt1`).
    /// If a friendly `Thorned Trailblazer` has charges remaining, consumes 1 charge and applies both options immediately.
    pub fn resolve_choose_one(
        &mut self,
        opt0: Unit,
        opt1: Unit,
        pool: &mut CardPool,
        rng: &mut Rng,
    ) {
        if let Some(tb) = self
            .board
            .iter_mut()
            .find(|u| u.trailblazer_charges_left > 0)
        {
            tb.trailblazer_charges_left -= 1;
            self.apply_choice_option(&opt0, pool, rng);
            self.apply_choice_option(&opt1, pool, rng);
            self.pending_choice_target = None;
        } else {
            self.push_discover(vec![opt0, opt1]);
        }
    }

    /// Apply a single `CHOICE_*` option unit.
    pub fn apply_choice_option(&mut self, chosen: &Unit, _pool: &mut CardPool, _rng: &mut Rng) {
        let mult = if chosen.is_golden { 2 } else { 1 };
        match chosen.card_id {
            CHOICE_CRATER_GEMS => {
                for _ in 0..(2 * mult) {
                    self.add_to_hand(tokens::make_blood_gem());
                }
            }
            CHOICE_CRATER_GEM_DAY => {
                for _ in 0..mult {
                    self.add_to_hand(tokens::make_gem_day());
                }
            }
            CHOICE_GEM_DAY_ATK => {
                self.auras.blood_gem_bonus_atk += 1;
            }
            CHOICE_GEM_DAY_HP => {
                self.auras.blood_gem_bonus_hp += 1;
            }
            CHOICE_BOTANIST_ATK => {
                self.auras.spell_bonus_atk += mult;
            }
            CHOICE_BOTANIST_HP => {
                self.auras.spell_bonus_hp += mult;
            }
            CHOICE_ALLIANCE_ATK => {
                if let Some(pos) = self.pending_choice_target {
                    if pos < self.board.len() {
                        let (atk, hp) = self.auras.spell_stat_buff(3, 1);
                        self.board[pos].add_stats(atk, hp);
                    }
                }
            }
            CHOICE_ALLIANCE_HP => {
                if let Some(pos) = self.pending_choice_target {
                    if pos < self.board.len() {
                        let (atk, hp) = self.auras.spell_stat_buff(1, 3);
                        self.board[pos].add_stats(atk, hp);
                    }
                }
            }
            CHOICE_FOODIE_BUFF_GEMS => {
                self.auras.blood_gem_bonus_atk += mult;
                self.auras.blood_gem_bonus_hp += mult;
            }
            CHOICE_FOODIE_GET_GEMS => {
                for _ in 0..(4 * mult) {
                    self.add_to_hand(tokens::make_blood_gem());
                }
            }
            CHOICE_SLY_REFRESHES => {
                self.auras.free_refreshes += (2 * mult) as u32;
            }
            CHOICE_SLY_GEMS => {
                for _ in 0..(3 * mult) {
                    self.add_to_hand(tokens::make_blood_gem());
                }
            }
            CHOICE_SCARAB_REBORN => {
                if let Some(pos) = self.pending_choice_target {
                    if pos < self.board.len() {
                        self.board[pos].add_stats(mult, mult);
                        self.board[pos].reborn = true;
                    }
                }
            }
            CHOICE_SCARAB_WINDFURY => {
                if let Some(pos) = self.pending_choice_target {
                    if pos < self.board.len() {
                        self.board[pos].add_stats(4 * mult, 0);
                        self.board[pos].windfury = true;
                    }
                }
            }
            CHOICE_TIME_MGMT_NOW => {
                let (atk, hp) = self.auras.spell_stat_buff(2, 2);
                for b in &mut self.board {
                    b.add_stats(atk, hp);
                }
                for h in &mut self.hand {
                    if !h.is_spell {
                        h.add_stats(atk, hp);
                    }
                }
            }
            CHOICE_TIME_MGMT_LATER => {
                self.auras.time_management_next_turn += 2;
            }
            _ => {}
        }
    }

    /// Open the `Lockbox` at `hand[hand_idx]`, replacing it with a random Golden minion with a type.
    pub fn open_lockbox_at(&mut self, hand_idx: usize, rng: &mut Rng) {
        if hand_idx >= self.hand.len() || self.hand[hand_idx].card_id != tokens::SPELL_LOCKBOX {
            return;
        }
        let typed_templates: Vec<CardTemplate> = cards::full_catalog()
            .into_iter()
            .filter(|t| t.tribe != Tribe::None && t.tavern_tier <= self.tavern_tier.max(2))
            .collect();
        if typed_templates.is_empty() {
            return;
        }
        let pick = rng.below(typed_templates.len());
        let tpl = &typed_templates[pick];
        let mut golden = tpl.instantiate();
        golden.name = format!("Golden {}", tpl.name);
        golden.attack = tpl.attack * 2;
        golden.health = tpl.health * 2;
        golden.max_attack = golden.attack;
        golden.max_health = golden.health;
        golden.base_attack = golden.attack;
        golden.base_health = golden.health;
        golden.is_golden = true;
        golden.intrinsic_golden = true;
        self.apply_global_unit_auras(&mut golden);
        cards::check_stat_thresholds(&mut golden);
        self.hand[hand_idx] = golden;
    }

    /// Open any `Lockbox` in `hand` that already has `lockbox_turns_left == 0` (`Hired Mount`).
    pub fn open_ready_lockboxes(&mut self, rng: &mut Rng) {
        for idx in 0..self.hand.len() {
            if self.hand[idx].card_id == tokens::SPELL_LOCKBOX
                && self.hand[idx].lockbox_turns_left == 0
            {
                self.open_lockbox_at(idx, rng);
            }
        }
    }

    /// Check whether a friendly minion (`Malchezaar, Prince of Dance`) can pay Health for a `Refresh`.
    pub fn has_health_refresh(&self) -> bool {
        self.board.iter().any(|u| u.malchezaar_refreshes_left > 0)
            && (self.health > 1 || cards::board_prevents_hero_damage(&self.board))
    }

    /// Effective Gold cost to buy `card` from `shop` (taking `next_spell_discount` into account).
    pub fn effective_spell_buy_cost(&self, card: &Unit) -> u32 {
        if card.costs_health {
            card.spell_cost
        } else {
            (card.spell_cost as i32 - self.auras.next_spell_discount).max(0) as u32
        }
    }

    /// Deal `amount` damage to the friendly hero during the Tavern Phase.
    /// Rewinds the damage if a friendly `Soul Rewinder` is on `board`.
    pub fn deal_hero_damage(&mut self, amount: i32) {
        if amount <= 0 {
            return;
        }
        self.health -= amount;
        if cards::on_hero_damage_taken(&mut self.board) {
            self.health += amount;
        }
    }

    /// Resolve `Waveling` buffs (`+4/+4` to a random minion in the Tavern per stack) on shop `Refresh`.
    fn resolve_refresh_waveling(&mut self, rng: &mut Rng) {
        if self.auras.waveling_stacks == 0 {
            return;
        }
        for _ in 0..self.auras.waveling_stacks {
            let minion_indices: Vec<usize> = self
                .shop
                .iter()
                .enumerate()
                .filter(|(_, u)| !u.is_spell)
                .map(|(i, _)| i)
                .collect();
            if !minion_indices.is_empty() {
                let idx = if minion_indices.len() == 1 {
                    minion_indices[0]
                } else {
                    minion_indices[rng.below(minion_indices.len())]
                };
                self.shop[idx].add_stats(4, 4);
            }
        }
    }

    /// Resolve `Laboratory Assistant` Fodder spawns on a shop `Refresh`.
    fn resolve_refresh_fodder(&mut self, rng: &mut Rng) {
        let fodder_count = self.auras.fodder_per_refresh[0];
        self.auras.fodder_per_refresh = [
            self.auras.fodder_per_refresh[1],
            self.auras.fodder_per_refresh[2],
            0,
        ];
        if fodder_count == 0 {
            return;
        }

        for _ in 0..fodder_count {
            let mut fodder = tokens::make_demon_fodder(false);
            self.apply_shop_auras(&mut fodder);

            let demon_indices: Vec<usize> = self
                .board
                .iter()
                .enumerate()
                .filter(|(_, u)| u.tribe.matches(Tribe::Demon))
                .map(|(i, _)| i)
                .collect();

            if !demon_indices.is_empty() {
                let idx = if demon_indices.len() == 1 {
                    demon_indices[0]
                } else {
                    demon_indices[rng.below(demon_indices.len())]
                };
                self.board[idx].add_stats(fodder.attack, fodder.health);
            } else {
                self.shop.push(fodder);
            }
        }
    }

    /// Execute the start-of-turn sequence (`docs/tavern.md` §4).
    pub fn start_turn(&mut self, pool: &mut CardPool, rng: &mut Rng) {
        let won_last = self.last_combat_won;
        self.last_combat_won = false;

        for u in &mut self.board {
            u.activated_this_turn = false;
            cards::on_start_turn(u);
            if won_last && u.winners_bread_stacks > 0 {
                let stacks = u.winners_bread_stacks;
                u.winners_bread_stacks = 0;
                u.play_blood_gems(stacks, &self.auras);
            } else {
                u.winners_bread_stacks = 0;
            }
        }

        if self.auras.time_management_next_turn > 0 {
            let stacks = self.auras.time_management_next_turn;
            self.auras.time_management_next_turn = 0;
            let (atk, hp) = self.auras.spell_stat_buff(2, 2);
            for _ in 0..stacks {
                for b in &mut self.board {
                    b.add_stats(atk, hp);
                }
                for h in &mut self.hand {
                    if !h.is_spell {
                        h.add_stats(atk, hp);
                    }
                }
            }
        }

        for idx in 0..self.hand.len() {
            if self.hand[idx].locked_turns > 0 {
                self.hand[idx].locked_turns -= 1;
            }
            if self.hand[idx].card_id == tokens::SPELL_LOCKBOX
                && self.hand[idx].lockbox_turns_left > 0
            {
                self.hand[idx].lockbox_turns_left -= 1;
                if self.hand[idx].lockbox_turns_left == 0 {
                    self.open_lockbox_at(idx, rng);
                }
            }
        }

        self.turn += 1;
        let base_cap = std::cmp::min(10, 2 + self.turn) + self.auras.base_max_gold_bonus;
        self.max_gold = base_cap;
        self.gold = (self.max_gold + self.bonus_gold_next_turn)
            .min(10 + self.auras.base_max_gold_bonus);
        self.bonus_gold_next_turn = 0;
        cards::on_start_turn_board(self);

        if self.turn > 1 && self.tavern_tier < 6 {
            self.upgrade_cost = self.upgrade_cost.saturating_sub(1);
        }

        let cap = shop_capacity(self.tavern_tier);
        if self.is_frozen {
            self.is_frozen = false;
            let minion_count = self.shop.iter().filter(|u| !u.is_spell).count();
            let needed = cap.saturating_sub(minion_count);
            for _ in 0..needed {
                if let Some(mut drawn) = pool.draw_from_pool(self.tavern_tier, rng) {
                    self.apply_shop_auras(&mut drawn);
                    self.shop.push(drawn);
                }
            }
            if self.include_shop_spells && !self.shop.iter().any(|u| u.is_spell) {
                self.shop
                    .push(cards::spells::draw_random_tavern_spell(self.tavern_tier, rng));
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
            if self.include_shop_spells {
                self.shop
                    .push(cards::spells::draw_random_tavern_spell(self.tavern_tier, rng));
            }
            self.resolve_refresh_waveling(rng);
        }
    }

    /// Build the player's board snapshot for Combat (including Start-of-Combat hand summons).
    pub fn combat_board(&self) -> Vec<Unit> {
        let mut b = self.board.clone();
        cards::on_start_of_combat_hand(&self.hand, &mut b);
        b
    }

    /// Run a combat phase against an opponent and apply all Combat-to-Tavern persistence.
    pub fn resolve_combat_against(
        &mut self,
        opponent_board: &[Unit],
        opponent_tier: u32,
        opponent_auras: &PlayerAuras,
        opponent_hand: &[Unit],
        seed: u64,
    ) -> BattleResult {
        let mut board_a = self.combat_board();
        for (i, u) in board_a.iter_mut().enumerate() {
            u.id = i as u32;
        }
        let pre_combat_snapshot = board_a.clone();

        let game_state = GameState {
            hero_tier_a: self.tavern_tier,
            hero_tier_b: opponent_tier,
            auras_a: self.auras.clone(),
            auras_b: opponent_auras.clone(),
            hand_a: self.hand.clone(),
            hand_b: opponent_hand.to_vec(),
        };

        let res = resolve_battle(&board_a, opponent_board, &game_state, seed);

        self.auras = res.auras_a.clone();
        let overconf = self.auras.overconfidence_stacks;
        self.auras.overconfidence_stacks = 0;
        if overconf > 0 {
            match res.outcome {
                BattleOutcome::AWin => self.bonus_gold_next_turn += 3 * overconf,
                BattleOutcome::Draw => self.bonus_gold_next_turn += overconf,
                BattleOutcome::BWin => {}
            }
        }
        self.hand = res.hand_a.clone();
        let mut lockbox_rng = Rng::new(seed ^ 0x9E37_79B9_7F4A_7C15);
        self.open_ready_lockboxes(&mut lockbox_rng);
        self.sync_all_auras();

        let mut post_units = res.survivors_a.clone();
        post_units.extend_from_slice(&res.dead_units_a);
        for (idx, tavern_unit) in self.board.iter_mut().enumerate() {
            if let Some(pre) = pre_combat_snapshot.get(idx) {
                cards::on_post_combat_unit(pre, &post_units, tavern_unit);
            }
        }

        self.last_combat_won = res.outcome == BattleOutcome::AWin;
        if res.outcome == BattleOutcome::BWin {
            self.health -= res.hero_damage as i32;
        }

        res
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
                let Some(card) = self.shop.get(shop_index) else {
                    return false;
                };
                if self.hand.len() >= 10 || card.card_id == tokens::TOKEN_FISHBAIT {
                    return false;
                }
                if card.is_spell {
                    if card.costs_health {
                        self.health > card.spell_cost as i32
                            || cards::board_prevents_hero_damage(&self.board)
                    } else {
                        self.gold >= self.effective_spell_buy_cost(card)
                    }
                } else {
                    self.gold >= 3
                }
            }
            TavernAction::Play {
                hand_index,
                board_pos,
            } => {
                let Some(card) = self.hand.get(hand_index) else {
                    return false;
                };
                if card.unplayable || card.lockbox_turns_left > 0 || card.locked_turns > 0 {
                    return false;
                }
                if card.is_spell {
                    if cards::spells::spell_requires_board_target(card.card_id) {
                        board_pos < self.board.len()
                    } else {
                        board_pos == 0
                    }
                } else if card.magnetic
                    && board_pos < self.board.len()
                    && self.board[board_pos].tribe.matches(card.tribe)
                {
                    true
                } else {
                    self.board.len() < MAX_BOARD_SIZE && board_pos <= self.board.len()
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
                match cards::activate_target_kind(unit.card_id) {
                    ActivateTargetKind::None => target_pos.is_none(),
                    ActivateTargetKind::BoardOther => {
                        matches!(target_pos, Some(t) if t < self.board.len() && t != board_pos)
                    }
                    ActivateTargetKind::HandCard => {
                        matches!(target_pos, Some(t) if t < self.hand.len())
                    }
                    ActivateTargetKind::ShopCard => {
                        matches!(target_pos, Some(t) if t < self.shop.len())
                    }
                }
            }
            TavernAction::Refresh => {
                self.gold >= 1 || self.auras.free_refreshes > 0 || self.has_health_refresh()
            }
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

        if self.hand.len() < 10 {
            for i in 0..self.shop.len() {
                let a = TavernAction::Buy { shop_index: i };
                if self.is_legal(&a) {
                    actions.push(a);
                }
            }
        }

        for (h_idx, card) in self.hand.iter().enumerate() {
            if card.unplayable || card.lockbox_turns_left > 0 || card.locked_turns > 0 {
                continue;
            }
            if card.is_spell {
                if cards::spells::spell_requires_board_target(card.card_id) {
                    for b_pos in 0..self.board.len() {
                        actions.push(TavernAction::Play {
                            hand_index: h_idx,
                            board_pos: b_pos,
                        });
                    }
                } else {
                    actions.push(TavernAction::Play {
                        hand_index: h_idx,
                        board_pos: 0,
                    });
                }
            } else if self.board.len() < MAX_BOARD_SIZE {
                for b_pos in 0..=self.board.len() {
                    actions.push(TavernAction::Play {
                        hand_index: h_idx,
                        board_pos: b_pos,
                    });
                }
            } else if card.magnetic {
                for (b_pos, target) in self.board.iter().enumerate() {
                    if target.tribe.matches(card.tribe) {
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
                        match cards::activate_target_kind(unit.card_id) {
                            ActivateTargetKind::None => {
                                actions.push(TavernAction::Activate {
                                    board_pos: b_pos,
                                    target_pos: None,
                                });
                            }
                            ActivateTargetKind::BoardOther => {
                                for t_pos in 0..self.board.len() {
                                    if t_pos != b_pos {
                                        actions.push(TavernAction::Activate {
                                            board_pos: b_pos,
                                            target_pos: Some(t_pos),
                                        });
                                    }
                                }
                            }
                            ActivateTargetKind::HandCard => {
                                for t_pos in 0..self.hand.len() {
                                    actions.push(TavernAction::Activate {
                                        board_pos: b_pos,
                                        target_pos: Some(t_pos),
                                    });
                                }
                            }
                            ActivateTargetKind::ShopCard => {
                                for t_pos in 0..self.shop.len() {
                                    actions.push(TavernAction::Activate {
                                        board_pos: b_pos,
                                        target_pos: Some(t_pos),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        if self.gold >= 1 || self.auras.free_refreshes > 0 || self.has_health_refresh() {
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
                let unit = self.shop.remove(shop_index);
                if unit.is_spell {
                    if unit.costs_health {
                        self.deal_hero_damage(unit.spell_cost as i32);
                    } else {
                        let cost = self.effective_spell_buy_cost(&unit);
                        self.gold -= cost;
                        self.auras.next_spell_discount = 0;
                    }
                    self.hand.push(unit);
                } else {
                    self.gold -= 3;
                    let cid = unit.card_id;
                    self.hand.push(unit);
                    self.check_and_resolve_triple(cid);
                }
                Ok(false)
            }
            TavernAction::Play {
                hand_index,
                board_pos,
            } => {
                let mut card = self.hand.remove(hand_index);
                if card.is_spell {
                    self.auras.spells_played += 1;
                    if card.willbreaker_group != 0 {
                        let grp = card.willbreaker_group;
                        for h in &mut self.hand {
                            if h.willbreaker_group == grp && h.willbreaker_remaining > 0 {
                                h.willbreaker_remaining -= 1;
                            }
                        }
                        let mut expired_indices: Vec<usize> = self
                            .hand
                            .iter()
                            .enumerate()
                            .filter(|(_, h)| {
                                h.willbreaker_group == grp && h.willbreaker_remaining == 0
                            })
                            .map(|(i, _)| i)
                            .collect();
                        expired_indices.reverse();
                        for exp_idx in expired_indices {
                            self.discard_hand_card(exp_idx, pool, rng);
                        }
                    }
                    cards::spells::cast_spell(self, card, board_pos, pool, rng);
                    return Ok(false);
                }

                let played_card_id = card.card_id;
                let played_tribe = card.tribe;
                let was_tripled_golden = card.is_golden && !card.intrinsic_golden;

                // Check for Magnetic fusion onto the compatible minion immediately to the right (`board[board_pos]`).
                if card.magnetic
                    && board_pos < self.board.len()
                    && self.board[board_pos].tribe.matches(card.tribe)
                {
                    cards::on_first_play_or_magnetize(self, &mut card);
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
                    cards::on_magnetize_transfer(&card, target);
                    pool.return_unit(&card);
                    cards::after_play_minion(self, played_card_id, played_tribe, board_pos, true);
                } else {
                    cards::on_first_play_or_magnetize(self, &mut card);
                    cards::on_play_battlecry(self, &mut card, board_pos, pool, rng);
                    let insert_idx = board_pos.min(self.board.len());
                    self.board.insert(insert_idx, card);
                    cards::after_play_minion(self, played_card_id, played_tribe, insert_idx, false);
                }

                if was_tripled_golden {
                    let reward_tier = std::cmp::min(6, self.tavern_tier + 1);
                    let mut opts = pool.draw_discover_options(reward_tier, 3, rng);
                    for opt in &mut opts {
                        self.apply_global_unit_auras(opt);
                    }
                    if !opts.is_empty() {
                        self.push_discover(opts);
                    }
                }
                Ok(false)
            }
            TavernAction::Sell { board_pos } => {
                let sold = self.board.remove(board_pos);
                pool.return_unit(&sold);
                self.gold =
                    std::cmp::min(10 + self.auras.base_max_gold_bonus, self.gold + 1);
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
                cards::on_activate(self, board_pos, target_pos, pool, rng);
                Ok(false)
            }
            TavernAction::Refresh => {
                if self.auras.free_refreshes > 0 {
                    self.auras.free_refreshes -= 1;
                } else if self.has_health_refresh() {
                    if let Some(m) = self
                        .board
                        .iter_mut()
                        .find(|u| u.malchezaar_refreshes_left > 0)
                    {
                        m.malchezaar_refreshes_left -= 1;
                    }
                    self.deal_hero_damage(1);
                } else {
                    self.gold -= 1;
                }
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
                if self.include_shop_spells {
                    self.shop
                        .push(cards::spells::draw_random_tavern_spell(self.tavern_tier, rng));
                }
                self.resolve_refresh_waveling(rng);
                self.resolve_refresh_fodder(rng);
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
                if !self.discover_queue.is_empty() {
                    self.discover_pending = Some(self.discover_queue.remove(0));
                }
                let chosen = opts.remove(option_index);

                if tokens::is_choice_option(chosen.card_id) {
                    self.apply_choice_option(&chosen, pool, rng);
                    self.pending_choice_target = None;
                    return Ok(false);
                }

                for unchosen in &opts {
                    pool.return_unit(unchosen);
                }
                self.add_to_hand(chosen);
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
        golden.scout_tier = copies.iter().map(|u| u.scout_tier).max().unwrap_or(1);
        let sum_sot_gold: u32 = copies.iter().map(|u| u.sot_gold_bonus).sum();
        golden.sot_gold_bonus = if card_id == cards::tier3::accord_o_tron::ID {
            sum_sot_gold.saturating_sub(1).max(2)
        } else {
            sum_sot_gold
        };
        if card_id == cards::tier3::malchezaar_prince_of_dance::ID {
            golden.malchezaar_refreshes_left = 4;
        }
        if card_id == cards::tier3::thorned_trailblazer::ID {
            golden.trailblazer_charges_left = 2;
        }
        golden.perm_atk_gained = copies.iter().map(|u| u.perm_atk_gained).sum();
        golden.perm_hp_gained = copies.iter().map(|u| u.perm_hp_gained).sum();
        golden.blood_gems_played = copies.iter().map(|u| u.blood_gems_played).sum();
        golden.blood_gem_stats_applied = (
            copies.iter().map(|u| u.blood_gem_stats_applied.0).sum(),
            copies.iter().map(|u| u.blood_gem_stats_applied.1).sum(),
        );
        golden.eternal_knight_stacks_applied = self.auras.eternal_knights_died;
        golden.volumizer_stacks_applied = (
            self.auras.volumizer_bonus_atk,
            self.auras.volumizer_bonus_hp,
        );
        golden.undead_attack_applied = self.auras.undead_bonus_attack;
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
    #[serde(default)]
    pub shop_spells: bool,
    #[serde(with = "serde_yaml::with::singleton_map_recursive")]
    pub steps: Vec<TavernStepSpec>,
    pub expect: TavernExpectSpec,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TavernStepSpec {
    StartTurn,
    SetGold(u32),
    SetTier(u32),
    SetDeity(DeityKind),
    GiveHand(String),
    GiveBoard(String),
    GiveShop(String),
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
    pub max_gold: Option<u32>,
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

fn instantiate_named_card(card_name: &str, templates: &[CardTemplate]) -> Result<Unit, String> {
    if let Some(spell) = cards::spells::spell_by_name(card_name) {
        return Ok(spell);
    }
    if let Some(tpl) = templates.iter().find(|t| t.name == card_name) {
        return Ok(tpl.instantiate());
    }
    if let Some(tpl) = cards::full_catalog()
        .into_iter()
        .find(|t| t.name == card_name)
    {
        return Ok(tpl.instantiate());
    }
    Err(format!("unknown card {card_name:?}"))
}

pub fn run_tavern_scenario(scenario: &TavernScenario) -> Result<(), String> {
    let catalog_name = scenario.catalog.as_deref().unwrap_or("tier1");
    let templates = cards::catalog_for(catalog_name)?;
    let mut pool = CardPool::new(templates.clone());
    let mut rng = Rng::new(scenario.seed);
    let mut state = TavernState::new().with_shop_spells(scenario.shop_spells);

    for (idx, step) in scenario.steps.iter().enumerate() {
        match step {
            TavernStepSpec::StartTurn => state.start_turn(&mut pool, &mut rng),
            TavernStepSpec::SetGold(g) => state.gold = *g,
            TavernStepSpec::SetTier(t) => state.tavern_tier = *t,
            TavernStepSpec::SetDeity(kind) => state.auras.deity.kind = *kind,
            TavernStepSpec::GiveHand(ref card_name) => {
                let unit = instantiate_named_card(card_name, &templates)
                    .map_err(|e| format!("[{}] {e}", scenario.name))?;
                state.add_to_hand(unit);
            }
            TavernStepSpec::GiveBoard(ref card_name) => {
                let mut unit = instantiate_named_card(card_name, &templates)
                    .map_err(|e| format!("[{}] {e}", scenario.name))?;
                state.apply_global_unit_auras(&mut unit);
                state.board.push(unit);
            }
            TavernStepSpec::GiveShop(ref card_name) => {
                let mut unit = instantiate_named_card(card_name, &templates)
                    .map_err(|e| format!("[{}] {e}", scenario.name))?;
                state.apply_shop_auras(&mut unit);
                state.shop.push(unit);
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
    if let Some(max_gold) = exp.max_gold {
        if state.max_gold != max_gold {
            return Err(format!(
                "[{}] expected max_gold {max_gold}, got {}",
                scenario.name, state.max_gold
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
