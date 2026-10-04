//! Card-agnostic Tavern Phase (Recruit Phase) state machine (`docs/tavern.md`).

use crate::cards::{self, ActivateTargetKind, BoardCtx, CardFlags, CardTemplate, Passive, Played};
use crate::combat::{resolve_battle, BattleResult};
use crate::model::{
    BattleOutcome, CardId, DeityKind, GameState, PlayerAuras, Side, Tribe, Unit,
};
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
        7 => 5,
        _ => panic!("invalid tavern tier {tier} (must be 1..=7)"),
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

    /// Draw up to `count` distinct `card_id`s matching `predicate` with `tavern_tier <= max_tier` (`Contracted Corpse`, `Hired Headhunter`).
    pub fn draw_discover_filtered<F>(
        &mut self,
        max_tier: u32,
        count: usize,
        predicate: F,
        rng: &mut Rng,
    ) -> Vec<Unit>
    where
        F: Fn(CardId) -> bool,
    {
        let mut options = Vec::with_capacity(count);
        let mut chosen_ids = Vec::with_capacity(count);

        for _ in 0..count {
            let total_copies: u32 = self
                .entries
                .iter()
                .filter(|e| {
                    e.template.tavern_tier <= max_tier
                        && e.remaining > 0
                        && predicate(e.template.card_id)
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
                    && predicate(entry.template.card_id)
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

    /// Take 1 copy of `card_id` from the shared card pool if available (`Disguised Graverobber`).
    pub fn take_copy(&mut self, card_id: CardId) {
        for entry in &mut self.entries {
            if entry.template.card_id == card_id {
                entry.remaining = entry.remaining.saturating_sub(1);
                break;
            }
        }
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
    pub armor: i32,
    pub tavern_tier: u32,
    pub max_gold: u32,
    pub gold: u32,
    pub bonus_gold_next_turn: u32,
    pub gold_spent_this_turn: u32,
    pub elementals_played_this_turn: u32,
    pub upgrade_cost: u32,
    pub is_frozen: bool,
    pub board: Vec<Unit>,
    pub hand: Vec<Unit>,
    pub shop: Vec<Unit>,
    pub discover_pending: Option<Vec<Unit>>,
    pub discover_queue: Vec<Vec<Unit>>,
    pub pending_choice_target: Option<usize>,
    pub fandral_combined_active: bool,
    pub defer_combined_choose_one: bool,
    pub pending_combined_choices: Vec<(Unit, Unit)>,
    pub next_willbreaker_group: u32,
    pub last_combat_won: bool,
    pub last_combat_lost: bool,
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
            armor: 0,
            tavern_tier: 1,
            max_gold: 0,
            gold: 0,
            bonus_gold_next_turn: 0,
            gold_spent_this_turn: 0,
            elementals_played_this_turn: 0,
            upgrade_cost: base_upgrade_cost(1),
            is_frozen: false,
            board: Vec::new(),
            hand: Vec::new(),
            shop: Vec::new(),
            discover_pending: None,
            discover_queue: Vec::new(),
            pending_choice_target: None,
            fandral_combined_active: false,
            defer_combined_choose_one: false,
            pending_combined_choices: Vec::new(),
            next_willbreaker_group: 1,
            last_combat_won: false,
            last_combat_lost: false,
            include_shop_spells: false,
            auras: PlayerAuras::default(),
        }
    }

    /// Enable or disable automatic rolling of 1 Tavern Spell into `shop` on `start_turn` / `Refresh`.
    pub fn with_shop_spells(mut self, enabled: bool) -> Self {
        self.include_shop_spells = enabled;
        self
    }

    /// Spend `amount` Gold, updating `gold_spent_this_turn` and firing `cards::on_gold_spent`.
    pub fn spend_gold(&mut self, amount: u32, pool: &mut CardPool, rng: &mut Rng) {
        if amount == 0 {
            return;
        }
        self.gold -= amount;
        self.gold_spent_this_turn += amount;
        cards::on_gold_spent(self, amount, pool, rng);
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

    /// Apply Tavern-wide shop buffs (`Dune Dweller`, `Staff of Enrichment`, `Eonar's Favor`, + global unit auras) to a newly drawn shop minion.
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
        for &(tribe, atk, hp) in &self.auras.tavern_tribe_buffs {
            if unit.tribe.matches(tribe) {
                unit.add_stats(atk, hp);
            }
        }
        if self.auras.tavern_all_atk != 0 || self.auras.tavern_all_hp != 0 {
            unit.add_stats(self.auras.tavern_all_atk, self.auras.tavern_all_hp);
        }
        if self.auras.ashen_corruptor_turn_buff != 0 {
            let b = self.auras.ashen_corruptor_turn_buff;
            unit.add_stats(b, b);
        }
    }

    /// Synchronize all persistent "wherever they are" auras across `board`, `hand`, and `shop`.
    pub fn sync_all_auras(&mut self) {
        self.auras.hero_low_health = self.health <= 15;
        cards::sync_board_spell_auras(&self.board, &mut self.auras);
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

    /// Destroy `self.board[board_pos]` during the Tavern Phase (`Disguised Graverobber`, `Maw Caster`, `Dead Bellringer`, `Tomb Turning`),
    /// resolving its Deathrattle (with `in_combat = false`) and Reborn resummon.
    pub fn destroy_board_unit(&mut self, board_pos: usize, pool: &mut CardPool, rng: &mut Rng) {
        if board_pos >= self.board.len() {
            return;
        }
        let dying = self.board.remove(board_pos);
        pool.return_unit(&dying);

        let old_tavern_all = (self.auras.tavern_all_atk, self.auras.tavern_all_hp);
        let mut hand_summoned = vec![false; self.hand.len()];
        let mut beast_bonus_atk = 0;
        let mut pending_attacks = Vec::new();
        let mut next_id = 1000u32;
        let mut events = Vec::new();
        let mut dr_ctx = BoardCtx {
            side: Side::A,
            in_combat: false,
            board: &mut self.board,
            cursor: board_pos,
            auras: &mut self.auras,
            hand: &mut self.hand,
            hand_summoned: &mut hand_summoned,
            dead_aberrations: &[],
            beast_bonus_atk: &mut beast_bonus_atk,
            hero_tier: self.tavern_tier,
            pending_attacks: &mut pending_attacks,
            enemy_destroys: Vec::new(),
            next_id: &mut next_id,
            rng,
            events: &mut events,
        };
        cards::on_friendly_death(&mut dr_ctx, &dying);
        let dr_repeats = 1 + cards::board_passive(dr_ctx.board, Passive::ExtraDeathrattles);
        for _ in 0..dr_repeats {
            cards::on_deathrattle(&dying, &mut dr_ctx);
        }
        let reborn = dying.reborn && cards::reborn(&mut dr_ctx, &dying);

        let d_tavern_atk = self.auras.tavern_all_atk - old_tavern_all.0;
        let d_tavern_hp = self.auras.tavern_all_hp - old_tavern_all.1;
        if d_tavern_atk != 0 || d_tavern_hp != 0 {
            for s in &mut self.shop {
                if !s.is_spell {
                    s.add_stats(d_tavern_atk, d_tavern_hp);
                }
            }
        }

        if reborn {
            self.check_and_resolve_triple(dying.card_id);
        }

        cards::resolve_ready_hand_cards(self, rng);
        self.sync_all_auras();
        let hand_cids: Vec<CardId> = self
            .hand
            .iter()
            .filter(|u| !u.is_spell)
            .map(|u| u.card_id)
            .collect();
        for cid in hand_cids {
            self.check_and_resolve_triple(cid);
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
        cards::on_card_added_to_hand(&self.board, &mut self.auras);
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
        if hand_idx >= self.hand.len() || self.hand[hand_idx].locked_turns > 0 {
            return None;
        }
        let discarded = self.hand.remove(hand_idx);
        pool.return_unit(&discarded);
        cards::on_discard_hand_card(self, &discarded, pool, rng);
        Some(discarded)
    }

    /// Resolve a Choose-One prompt (`opt0` vs `opt1`).
    /// If `fandral_combined` is active or a friendly `Thorned Trailblazer` has charges remaining, applies both options immediately.
    pub fn resolve_choose_one(
        &mut self,
        opt0: Unit,
        opt1: Unit,
        pool: &mut CardPool,
        rng: &mut Rng,
    ) {
        let combined = if self.fandral_combined_active || opt0.fandral_combined {
            true
        } else if let Some(tb) = self
            .board
            .iter_mut()
            .find(|u| u.trailblazer_charges_left > 0)
        {
            tb.trailblazer_charges_left -= 1;
            true
        } else {
            false
        };

        if combined {
            if self.defer_combined_choose_one {
                self.pending_combined_choices.push((opt0, opt1));
            } else {
                cards::apply_chosen_option(self, &opt0, pool, rng);
                cards::apply_chosen_option(self, &opt1, pool, rng);
                cards::after_choose_one(self, &opt0, pool, rng);
                if self.discover_pending.is_none() {
                    self.pending_choice_target = None;
                }
            }
        } else {
            self.push_discover(vec![opt0, opt1]);
        }
    }

    /// Flush any deferred combined Choose-One options after the played minion has entered `self.board`.
    pub fn flush_combined_choices(&mut self, pool: &mut CardPool, rng: &mut Rng) {
        let pending = std::mem::take(&mut self.pending_combined_choices);
        for (opt0, opt1) in pending {
            cards::apply_chosen_option(self, &opt0, pool, rng);
            cards::apply_chosen_option(self, &opt1, pool, rng);
            cards::after_choose_one(self, &opt0, pool, rng);
            if self.discover_pending.is_none() {
                self.pending_choice_target = None;
            }
        }
    }

    /// The board slot targeted by the Choose One being resolved (`pending_choice_target`), if it
    /// still holds a minion.
    pub fn choice_target(&self) -> Option<usize> {
        self.pending_choice_target
            .filter(|&pos| pos < self.board.len())
    }

    /// Check whether a friendly minion (`Malchezaar, Prince of Dance`) can pay Health for a `Refresh`.
    pub fn has_health_refresh(&self) -> bool {
        self.board.iter().any(|u| u.malchezaar_refreshes_left > 0)
            && (self.health + self.armor > 1 || cards::board_prevents_hero_damage(&self.board))
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
    /// Rewinds the damage if a friendly minion rewinds hero damage (`cards::on_hero_damage`);
    /// otherwise absorbs into `armor` before reducing `health`.
    pub fn deal_hero_damage(&mut self, amount: i32) {
        if amount <= 0 {
            return;
        }
        if !cards::on_hero_damage(self, amount) {
            let absorbed = amount.min(self.armor.max(0));
            self.armor -= absorbed;
            self.health -= amount - absorbed;
        }
        cards::after_hero_damage(self, amount);
        self.sync_all_auras();
    }

    /// Perform a free shop Refresh (`Snarky Shark`).
    pub fn refresh_shop_free(&mut self, pool: &mut CardPool, rng: &mut Rng) {
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
        cards::after_shop_refresh(self, rng);
    }

    /// Refresh the Tavern with minions of `tribe` (`Lost Staff of Hamuul`).
    pub fn refresh_shop_with_tribe(&mut self, tribe: Tribe, pool: &mut CardPool, rng: &mut Rng) {
        self.is_frozen = false;
        for old in self.shop.drain(..) {
            pool.return_unit(&old);
        }
        let cap = shop_capacity(self.tavern_tier);
        for _ in 0..cap {
            if let Some(mut drawn) = pool.draw_by_tribe(tribe, None, self.tavern_tier, rng) {
                self.apply_shop_auras(&mut drawn);
                self.shop.push(drawn);
            }
        }
        if self.include_shop_spells {
            self.shop
                .push(cards::spells::draw_random_tavern_spell(self.tavern_tier, rng));
        }
        cards::after_shop_refresh(self, rng);
    }

    /// Execute the start-of-turn sequence (`docs/tavern.md` §4).
    pub fn start_turn(&mut self, pool: &mut CardPool, rng: &mut Rng) {
        let won_last = self.last_combat_won;
        self.last_combat_won = false;
        self.gold_spent_this_turn = 0;
        self.elementals_played_this_turn = 0;
        self.auras.goldrinn_bonus = 0;
        self.auras.brood_of_nozdormu_stacks = 0;
        self.auras.upper_hand_stacks = 0;
        let prev_ashen = self.auras.ashen_corruptor_turn_buff;
        self.auras.ashen_corruptor_turn_buff = 0;
        if prev_ashen != 0 && self.is_frozen {
            for s in &mut self.shop {
                if !s.is_spell {
                    s.attack = (s.attack - prev_ashen).max(0);
                    s.health = (s.health - prev_ashen).max(1);
                    s.max_attack = s.attack;
                    s.max_health = s.health;
                }
            }
        }

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
        cards::resolve_pending_effects(&mut self.board, &self.auras, rng);

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
            self.hand[idx].dies_on_play_this_turn = false;
            if self.hand[idx].locked_turns > 0 {
                self.hand[idx].locked_turns -= 1;
            }
            cards::on_turn_start_in_hand(self, idx, rng);
        }

        self.turn += 1;
        let base_cap = std::cmp::min(10, 2 + self.turn) + self.auras.base_max_gold_bonus;
        self.max_gold = base_cap;
        self.gold = self.max_gold + self.bonus_gold_next_turn;
        self.bonus_gold_next_turn = 0;
        cards::on_start_turn_board(self, pool, rng);

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
            cards::after_shop_refresh(self, rng);
        }
        self.sync_all_auras();
    }

    /// Build the player's board snapshot for Combat (including Start-of-Combat hand summons).
    pub fn combat_board(&self) -> Vec<Unit> {
        let mut b = self.board.clone();
        cards::on_start_of_combat_hand(&self.hand, &mut b);
        b
    }

    fn apply_post_combat_side(
        &mut self,
        side: Side,
        pre_combat_snapshot: &[Unit],
        res: &BattleResult,
        seed: u64,
    ) {
        let (new_auras, new_hand, survivors, dead_units, won, lost) = match side {
            Side::A => (
                res.auras_a.clone(),
                res.hand_a.clone(),
                &res.survivors_a,
                &res.dead_units_a,
                res.outcome == BattleOutcome::AWin,
                res.outcome == BattleOutcome::BWin,
            ),
            Side::B => (
                res.auras_b.clone(),
                res.hand_b.clone(),
                &res.survivors_b,
                &res.dead_units_b,
                res.outcome == BattleOutcome::BWin,
                res.outcome == BattleOutcome::AWin,
            ),
        };

        let d_tavern_atk = new_auras.tavern_all_atk - self.auras.tavern_all_atk;
        let d_tavern_hp = new_auras.tavern_all_hp - self.auras.tavern_all_hp;
        self.auras = new_auras;
        if d_tavern_atk != 0 || d_tavern_hp != 0 {
            for s in &mut self.shop {
                if !s.is_spell {
                    s.add_stats(d_tavern_atk, d_tavern_hp);
                }
            }
        }

        let overconf = self.auras.overconfidence_stacks;
        self.auras.overconfidence_stacks = 0;
        if overconf > 0 {
            if won {
                self.bonus_gold_next_turn += 3 * overconf;
            } else if res.outcome == BattleOutcome::Draw {
                self.bonus_gold_next_turn += overconf;
            }
        }
        self.hand = new_hand;
        // Hand cards that became ready during combat (e.g. a sped-up `Lockbox`) resolve with a
        // per-side RNG derived from the battle seed.
        let ready_salt = match side {
            Side::A => 0x9E37_79B9_7F4A_7C15,
            Side::B => 0xBF58_476D_1CE4_E5B9,
        };
        let mut ready_rng = Rng::new(seed ^ ready_salt);
        cards::resolve_ready_hand_cards(self, &mut ready_rng);

        let mut post_units = survivors.clone();
        post_units.extend_from_slice(dead_units);
        for (idx, tavern_unit) in self.board.iter_mut().enumerate() {
            cards::on_post_combat_unit(pre_combat_snapshot, idx, &post_units, tavern_unit);
        }

        let hand_cids: Vec<CardId> = self
            .hand
            .iter()
            .filter(|u| !u.is_spell && !u.is_golden)
            .map(|u| u.card_id)
            .collect();
        for cid in hand_cids {
            self.check_and_resolve_triple(cid);
        }

        self.last_combat_won = won;
        self.last_combat_lost = lost;
        if lost {
            let dmg = res.hero_damage as i32;
            let absorbed = dmg.min(self.armor.max(0));
            self.armor -= absorbed;
            let net_dmg = dmg - absorbed;
            self.health -= net_dmg;
            if net_dmg > 0 {
                cards::after_hero_damage(self, net_dmg);
            }
        }
        self.sync_all_auras();
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
        self.sync_all_auras();
        let mut board_a = self.board.clone();
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
        self.apply_post_combat_side(Side::A, &pre_combat_snapshot, &res, seed);
        res
    }

    /// Run a symmetric head-to-head combat phase between `player_a` and `player_b`,
    /// applying all Combat-to-Tavern persistence and hero damage to both players.
    pub fn resolve_combat_pair(
        player_a: &mut TavernState,
        player_b: &mut TavernState,
        seed: u64,
    ) -> BattleResult {
        player_a.sync_all_auras();
        player_b.sync_all_auras();

        let mut board_a = player_a.board.clone();
        board_a.truncate(MAX_BOARD_SIZE);
        for (i, u) in board_a.iter_mut().enumerate() {
            u.id = i as u32;
        }
        let pre_a = board_a.clone();

        let mut board_b = player_b.board.clone();
        board_b.truncate(MAX_BOARD_SIZE);
        let offset = board_a.len() as u32;
        for (i, u) in board_b.iter_mut().enumerate() {
            u.id = offset + i as u32;
        }
        let pre_b = board_b.clone();

        let game_state = GameState {
            hero_tier_a: player_a.tavern_tier,
            hero_tier_b: player_b.tavern_tier,
            auras_a: player_a.auras.clone(),
            auras_b: player_b.auras.clone(),
            hand_a: player_a.hand.clone(),
            hand_b: player_b.hand.clone(),
        };

        let res = resolve_battle(&board_a, &board_b, &game_state, seed);
        player_a.apply_post_combat_side(Side::A, &pre_a, &res, seed);
        player_b.apply_post_combat_side(Side::B, &pre_b, &res, seed);
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
                if self.hand.len() >= 10 || cards::hooks(card.card_id).has(CardFlags::UNBUYABLE) {
                    return false;
                }
                if card.is_spell {
                    if card.costs_health {
                        self.health + self.armor > card.spell_cost as i32
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
                            && cards::spells::can_target(card.card_id, &self.board[board_pos])
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
                    ActivateTargetKind::BoardOtherUndead => {
                        matches!(
                            target_pos,
                            Some(t) if t < self.board.len() && t != board_pos && self.board[t].tribe.matches(Tribe::Undead)
                        )
                    }
                    ActivateTargetKind::BoardOtherMurloc => {
                        matches!(
                            target_pos,
                            Some(t) if t < self.board.len() && t != board_pos && self.board[t].tribe.matches(Tribe::Murloc)
                        )
                    }
                    ActivateTargetKind::BoardBattlecry => {
                        matches!(
                            target_pos,
                            Some(t) if t < self.board.len() && cards::is_battlecry_minion(self.board[t].card_id)
                        )
                    }
                    ActivateTargetKind::BoardRally => {
                        matches!(
                            target_pos,
                            Some(t) if t < self.board.len() && cards::is_rally_minion(self.board[t].card_id)
                        )
                    }
                    ActivateTargetKind::BoardAny => {
                        matches!(target_pos, Some(t) if t < self.board.len())
                    }
                    ActivateTargetKind::HandCard => {
                        matches!(
                            target_pos,
                            Some(t) if t < self.hand.len() && self.hand[t].locked_turns == 0
                        )
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
                    for (b_pos, unit) in self.board.iter().enumerate() {
                        if cards::spells::can_target(card.card_id, unit) {
                            actions.push(TavernAction::Play {
                                hand_index: h_idx,
                                board_pos: b_pos,
                            });
                        }
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
                            ActivateTargetKind::BoardOtherUndead => {
                                for (t_pos, t_unit) in self.board.iter().enumerate() {
                                    if t_pos != b_pos && t_unit.tribe.matches(Tribe::Undead) {
                                        actions.push(TavernAction::Activate {
                                            board_pos: b_pos,
                                            target_pos: Some(t_pos),
                                        });
                                    }
                                }
                            }
                            ActivateTargetKind::BoardOtherMurloc => {
                                for (t_pos, t_unit) in self.board.iter().enumerate() {
                                    if t_pos != b_pos && t_unit.tribe.matches(Tribe::Murloc) {
                                        actions.push(TavernAction::Activate {
                                            board_pos: b_pos,
                                            target_pos: Some(t_pos),
                                        });
                                    }
                                }
                            }
                            ActivateTargetKind::BoardBattlecry => {
                                for (t_pos, t_unit) in self.board.iter().enumerate() {
                                    if cards::is_battlecry_minion(t_unit.card_id) {
                                        actions.push(TavernAction::Activate {
                                            board_pos: b_pos,
                                            target_pos: Some(t_pos),
                                        });
                                    }
                                }
                            }
                            ActivateTargetKind::BoardRally => {
                                for (t_pos, t_unit) in self.board.iter().enumerate() {
                                    if cards::is_rally_minion(t_unit.card_id) {
                                        actions.push(TavernAction::Activate {
                                            board_pos: b_pos,
                                            target_pos: Some(t_pos),
                                        });
                                    }
                                }
                            }
                            ActivateTargetKind::BoardAny => {
                                for t_pos in 0..self.board.len() {
                                    actions.push(TavernAction::Activate {
                                        board_pos: b_pos,
                                        target_pos: Some(t_pos),
                                    });
                                }
                            }
                            ActivateTargetKind::HandCard => {
                                for (t_pos, h_card) in self.hand.iter().enumerate() {
                                    if h_card.locked_turns == 0 {
                                        actions.push(TavernAction::Activate {
                                            board_pos: b_pos,
                                            target_pos: Some(t_pos),
                                        });
                                    }
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
                let mut unit = self.shop.remove(shop_index);
                let bought_snapshot = unit.clone();
                if unit.is_spell {
                    if unit.costs_health {
                        self.deal_hero_damage(unit.spell_cost as i32);
                    } else {
                        let cost = self.effective_spell_buy_cost(&unit);
                        self.auras.next_spell_discount = 0;
                        self.spend_gold(cost, pool, rng);
                    }
                    self.hand.push(unit);
                    cards::on_card_added_to_hand(&self.board, &mut self.auras);
                } else {
                    self.spend_gold(3, pool, rng);
                    cards::on_minion_bought(self, &mut unit);
                    let cid = unit.card_id;
                    self.hand.push(unit);
                    cards::on_card_added_to_hand(&self.board, &mut self.auras);
                    self.check_and_resolve_triple(cid);
                }
                cards::after_buy_card(self, &bought_snapshot, pool, rng);
                Ok(false)
            }
            TavernAction::Play {
                hand_index,
                board_pos,
            } => {
                let mut card = self.hand.remove(hand_index);
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
                if card.is_spell {
                    self.auras.spells_played += 1;
                    cards::spells::cast_spell(self, card, board_pos, pool, rng);
                    return Ok(false);
                }

                let played_card_id = card.card_id;
                let played_tribe = card.tribe;
                let was_golden_minion = card.is_golden;
                let was_tripled_golden = card.is_golden && !card.intrinsic_golden;
                let should_die_on_play = card.dies_on_play_this_turn;

                if played_tribe.matches(Tribe::Elemental) {
                    self.elementals_played_this_turn += 1;
                }

                if was_golden_minion {
                    self.auras.golden_minions_played += 1;
                    self.sync_all_auras();
                }

                // Check for Magnetic fusion onto the compatible minion immediately to the right (`board[board_pos]`).
                if card.magnetic
                    && board_pos < self.board.len()
                    && self.board[board_pos].tribe.matches(card.tribe)
                {
                    cards::magnetize(self, &mut card, board_pos, pool, rng);
                    pool.return_unit(&card);
                } else {
                    cards::on_first_play_or_magnetize(self, &mut card);
                    self.fandral_combined_active = card.fandral_combined;
                    self.defer_combined_choose_one = true;
                    cards::on_play_battlecry(self, &mut card, board_pos, pool, rng);
                    self.defer_combined_choose_one = false;
                    self.fandral_combined_active = false;
                    let insert_idx = board_pos.min(self.board.len());
                    self.board.insert(insert_idx, card);
                    self.flush_combined_choices(pool, rng);
                    let played = Played {
                        card_id: played_card_id,
                        tribe: played_tribe,
                        board_pos: insert_idx,
                        magnetized: false,
                    };
                    cards::after_play_minion(self, &played, pool, rng);
                    if should_die_on_play
                        && insert_idx < self.board.len()
                        && self.board[insert_idx].dies_on_play_this_turn
                    {
                        self.destroy_board_unit(insert_idx, pool, rng);
                    }
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
                self.gold += 1;
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
                self.board[board_pos].activated_this_turn = true;
                self.spend_gold(cost, pool, rng);
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
                    self.spend_gold(1, pool, rng);
                }
                self.refresh_shop_free(pool, rng);
                Ok(false)
            }
            TavernAction::UpgradeTavern => {
                let cost = self.upgrade_cost;
                self.spend_gold(cost, pool, rng);
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
                let mut chosen = opts.remove(option_index);

                if cards::is_option_card(chosen.card_id) {
                    cards::apply_chosen_option(self, &chosen, pool, rng);
                    cards::after_choose_one(self, &chosen, pool, rng);
                    if self.discover_pending.is_none() {
                        self.pending_choice_target = None;
                    }
                    return Ok(false);
                }

                for unchosen in &opts {
                    pool.return_unit(unchosen);
                }
                if chosen.discover_deals_tier_damage {
                    chosen.discover_deals_tier_damage = false;
                    let dmg = chosen.tavern_tier as i32;
                    self.deal_hero_damage(dmg);
                }
                self.add_to_hand(chosen);
                cards::on_card_discovered(self, pool, rng);
                Ok(false)
            }
            TavernAction::EndTurn => {
                cards::on_end_turn(self, pool, rng);
                Ok(true)
            }
        }
    }

    /// Combine 3 non-golden copies of `card_id` across `board` and `hand` into 1 Golden copy in `hand` (`docs/tavern.md` §5.1).
    /// Cards flagged [`CardFlags::TRIPLE_WILDCARD_ELEMENTAL`] (`Elemental of Surprise`) can triple
    /// with any Elemental; cards flagged [`CardFlags::NO_TRIPLE`] never triple.
    pub fn check_and_resolve_triple(&mut self, card_id: CardId) {
        let no_triple = |cid: CardId| cards::hooks(cid).has(CardFlags::NO_TRIPLE);
        let is_wildcard = |cid: CardId| cards::hooks(cid).has(CardFlags::TRIPLE_WILDCARD_ELEMENTAL);
        if card_id == 0 || no_triple(card_id) {
            return;
        }
        let is_eligible = |u: &Unit| -> bool {
            !u.is_golden
                && !u.intrinsic_golden
                && !u.is_spell
                && u.card_id != 0
                && !no_triple(u.card_id)
        };

        let exact_count = self
            .board
            .iter()
            .chain(self.hand.iter())
            .filter(|u| is_eligible(u) && u.card_id == card_id)
            .count();

        let (target_cid, use_wildcard) = if exact_count >= 3 {
            (card_id, false)
        } else {
            let wildcard_count = self
                .board
                .iter()
                .chain(self.hand.iter())
                .filter(|u| is_eligible(u) && is_wildcard(u.card_id))
                .count();
            if wildcard_count == 0 {
                return;
            }
            if !is_wildcard(card_id) {
                let elem_count = self
                    .board
                    .iter()
                    .chain(self.hand.iter())
                    .filter(|u| {
                        is_eligible(u) && u.card_id == card_id && u.tribe.matches(Tribe::Elemental)
                    })
                    .count();
                if elem_count >= 1 && elem_count + wildcard_count >= 3 {
                    (card_id, true)
                } else {
                    return;
                }
            } else {
                let mut candidate_cids: Vec<CardId> = Vec::new();
                for u in self.board.iter().chain(self.hand.iter()) {
                    if is_eligible(u)
                        && !is_wildcard(u.card_id)
                        && u.tribe.matches(Tribe::Elemental)
                        && !candidate_cids.contains(&u.card_id)
                    {
                        candidate_cids.push(u.card_id);
                    }
                }
                let mut found = None;
                for &cid in &candidate_cids {
                    let cnt = self
                        .board
                        .iter()
                        .chain(self.hand.iter())
                        .filter(|u| is_eligible(u) && u.card_id == cid)
                        .count();
                    if cnt == 2 && cnt + wildcard_count >= 3 {
                        found = Some(cid);
                        break;
                    }
                }
                if found.is_none() {
                    for &cid in &candidate_cids {
                        let cnt = self
                            .board
                            .iter()
                            .chain(self.hand.iter())
                            .filter(|u| is_eligible(u) && u.card_id == cid)
                            .count();
                        if cnt >= 1 && cnt + wildcard_count >= 3 {
                            found = Some(cid);
                            break;
                        }
                    }
                }
                if let Some(cid) = found {
                    (cid, true)
                } else {
                    return;
                }
            }
        };

        let mut copies = Vec::with_capacity(3);
        self.board.retain(|u| {
            if copies.len() < 3 && is_eligible(u) && u.card_id == target_cid {
                copies.push(u.clone());
                false
            } else {
                true
            }
        });
        self.hand.retain(|u| {
            if copies.len() < 3 && is_eligible(u) && u.card_id == target_cid {
                copies.push(u.clone());
                false
            } else {
                true
            }
        });
        if use_wildcard && copies.len() < 3 {
            self.board.retain(|u| {
                if copies.len() < 3 && is_eligible(u) && is_wildcard(u.card_id) {
                    copies.push(u.clone());
                    false
                } else {
                    true
                }
            });
            self.hand.retain(|u| {
                if copies.len() < 3 && is_eligible(u) && is_wildcard(u.card_id) {
                    copies.push(u.clone());
                    false
                } else {
                    true
                }
            });
        }

        debug_assert_eq!(copies.len(), 3);
        let primary = copies
            .iter()
            .find(|u| u.card_id == target_cid)
            .unwrap_or(&copies[0]);
        let base_atk = primary.base_attack;
        let base_hp = primary.base_health;
        let tier = primary.tavern_tier;
        let tribe = primary.tribe;
        let name = format!("Golden {}", primary.name);

        let aura_atk_on = |u: &Unit| -> i32 {
            u.undead_attack_applied
                + u.volumizer_stacks_applied.0
                + u.holy_vanguard_buff_applied.0
                + (u.eternal_knight_stacks_applied as i32) * 4
                + (u.sky_golem_stacks_applied as i32) * 4
                + (u.maritime_stacks_applied as i32) * 3
        };
        let aura_hp_on = |u: &Unit| -> i32 {
            u.volumizer_stacks_applied.1
                + u.holy_vanguard_buff_applied.1
                + (u.eternal_knight_stacks_applied as i32) * 2
                + (u.sky_golem_stacks_applied as i32) * 2
                + (u.maritime_stacks_applied as i32) * 3
        };

        let atk_buffs: i32 = copies
            .iter()
            .map(|u| (u.attack - u.base_attack) - aura_atk_on(u))
            .sum();
        let hp_buffs: i32 = copies
            .iter()
            .map(|u| (u.health - u.base_health) - aura_hp_on(u))
            .sum();

        let golden_atk = 2 * base_atk + atk_buffs;
        let golden_hp = 2 * base_hp + hp_buffs;

        let mut golden = Unit::new(name, golden_atk, golden_hp)
            .with_card_id(target_cid)
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
        golden.eot_health_bonus = copies.iter().map(|u| u.eot_health_bonus).sum();
        golden.sot_gold_bonus = copies.iter().map(|u| u.sot_gold_bonus).sum();
        cards::init_unit_turn_charges(&mut golden);
        // The Golden's own spell aura comes from its card; what the copies gained on top of
        // theirs carries over.
        cards::init_spell_aura(&mut golden);
        for copy in &copies {
            let (atk, hp) = cards::gained_spell_aura(copy);
            golden.spell_atk_aura += atk;
            golden.spell_hp_aura += hp;
        }
        golden.wrathguard_bonus = copies.iter().map(|u| u.wrathguard_bonus).sum();
        golden.perm_atk_gained = copies.iter().map(|u| u.perm_atk_gained).sum();
        golden.perm_hp_gained = copies.iter().map(|u| u.perm_hp_gained).sum();
        golden.perm_blood_gems_gained = copies.iter().map(|u| u.perm_blood_gems_gained).sum();
        golden.hopebringer_stacks = copies.iter().map(|u| u.hopebringer_stacks).sum();
        golden.leviathan_stacks = copies.iter().map(|u| u.leviathan_stacks).sum();
        golden.spark_snapper_stacks = copies.iter().map(|u| u.spark_snapper_stacks).sum();
        golden.auto_reveille_buys = copies
            .iter()
            .map(|u| u.auto_reveille_buys)
            .max()
            .unwrap_or(0);
        golden.eredar_damage_progress = copies
            .iter()
            .map(|u| u.eredar_damage_progress)
            .max()
            .unwrap_or(0);
        golden.aphlass_stacks = copies.iter().map(|u| u.aphlass_stacks).sum();
        golden.ultraviolet_stacks = copies.iter().map(|u| u.ultraviolet_stacks).sum();
        golden.unbound_tempest_progress = copies
            .iter()
            .map(|u| u.unbound_tempest_progress)
            .max()
            .unwrap_or(0);
        golden.magnetizations_count = copies.iter().map(|u| u.magnetizations_count).sum();
        golden.blood_gems_played = copies.iter().map(|u| u.blood_gems_played).sum();
        golden.blood_gem_stats_applied = (
            copies.iter().map(|u| u.blood_gem_stats_applied.0).sum(),
            copies.iter().map(|u| u.blood_gem_stats_applied.1).sum(),
        );
        cards::on_merge_golden(&copies, &mut golden);
        cards::check_stat_thresholds(&mut golden);

        self.hand.push(golden);
        cards::on_card_added_to_hand(&self.board, &mut self.auras);
        self.sync_all_auras();
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
