//! Card-agnostic Tavern Phase (Recruit Phase) state machine (`docs/tavern.md`).

use crate::cards::{self, ActivateTargetKind, BoardCtx, CardFlags, CardTemplate, Passive, Played};
use crate::combat::{resolve_battle, BattleResult};
use crate::heroes;
use crate::model::{
    BattleOutcome, CardId, CombatResult, GameState, HeroId, HeroState, PlayerAuras, Side,
    TeammateGift, Tribe, Unit,
};
use crate::rng::Rng;

/// Maximum number of minions allowed on a player's board.
pub const MAX_BOARD_SIZE: usize = 7;

/// Maximum number of cards in Bob's Tavern: a card an effect would add beyond it isn't added
/// (`docs/tavern.md` §3.2).
pub const MAX_SHOP_SIZE: usize = 7;

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
        self.draw_discover_by_tribe_except(tribe, None, max_tier, count, rng)
    }

    /// [`draw_discover_by_tribe`](Self::draw_discover_by_tribe), never offering
    /// `exclude_card_id` (`Primalfin Lookout` doesn't Discover itself).
    pub fn draw_discover_by_tribe_except(
        &mut self,
        tribe: Tribe,
        exclude_card_id: Option<CardId>,
        max_tier: u32,
        count: usize,
        rng: &mut Rng,
    ) -> Vec<Unit> {
        let mut options = Vec::with_capacity(count);
        let mut chosen_ids: Vec<CardId> = exclude_card_id.into_iter().collect();

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

    /// Return a unit's copy back to the shared card pool (3 copies for a Golden formed by a Triple;
    /// 1 for one an effect made Golden, `golden_from_effect`, or that is `intrinsic_golden`).
    pub fn return_unit(&mut self, unit: &Unit) {
        if unit.is_spell {
            return;
        }
        let copies = Self::copies_of(unit);
        for entry in &mut self.entries {
            if entry.template.card_id == unit.card_id {
                let cap = base_copies_for_tier(entry.template.tavern_tier);
                entry.remaining = (entry.remaining + copies).min(cap);
                break;
            }
        }
    }

    /// Take a unit's copies back out of the shared card pool, as many as
    /// [`return_unit`](Self::return_unit) returns (never below 0): the Duos `Jumping Jack`, Passed
    /// once it is sold instead of going back to the pool.
    pub fn take_unit(&mut self, unit: &Unit) {
        if unit.is_spell {
            return;
        }
        let copies = Self::copies_of(unit);
        for entry in &mut self.entries {
            if entry.template.card_id == unit.card_id {
                entry.remaining = entry.remaining.saturating_sub(copies);
                break;
            }
        }
    }

    /// The pool copies `unit` stands for (see [`return_unit`](Self::return_unit)).
    fn copies_of(unit: &Unit) -> u32 {
        if unit.is_golden && !unit.intrinsic_golden && !unit.golden_from_effect {
            3
        } else {
            1
        }
    }
}

/// A minion's place in the Tavern Phase: `board[i]`, or `shop[i]` (a minion in Bob's Tavern).
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum MinionSlot {
    Board(usize),
    Shop(usize),
}

/// Discrete player actions during the Tavern Phase (`docs/tavern.md` §6). New variants go at the
/// end, so the earlier ones keep their places.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TavernAction {
    Buy {
        shop_index: usize,
    },
    Play {
        hand_index: usize,
        board_pos: usize,
    },
    /// Cast the spell `hand[hand_index]` on the minion `shop[shop_index]` in the Tavern (a spell
    /// whose text says "a minion", `docs/tavern.md` §7.4).
    PlayOnShop {
        hand_index: usize,
        shop_index: usize,
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
    /// Duos: Pass `hand[hand_index]` to the teammate (`docs/duos_tavern.md` §5). Only a team's
    /// Tavern ([`crate::duos_tavern::DuosTavern`]) allows it: a Solo `TavernState` never does.
    Pass {
        hand_index: usize,
    },
    /// Use the player's Hero Power, on `target` if it needs one (`docs/heroes.md` §5).
    HeroPower {
        target: Option<usize>,
    },
    /// Choose the hero `hero_options[option_index]` (`docs/heroes.md` §6).
    ChooseHero {
        option_index: usize,
    },
}

/// Complete state of a player during the Tavern Phase (`docs/tavern.md` §3).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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
    /// The minion targeted by the Choose One being resolved (see `choice_target`).
    pub pending_choice_target: Option<MinionSlot>,
    /// Extra times the next chosen Choose-One option applies: a Choose One spell cast several
    /// times is chosen once (`docs/tavern.md` §7.4).
    pub pending_choice_repeats: u32,
    /// While set, the spell being cast repeats its first cast (`spells::cast_spell_on`): a Choose
    /// One repeats the first cast's choice (see `resolve_choose_one`).
    pub repeating_cast: bool,
    /// While set, Choose Ones resolve with both effects combined (see `resolve_choose_one`).
    pub combine_choose_one: bool,
    pub defer_combined_choose_one: bool,
    pub pending_combined_choices: Vec<(Unit, Unit)>,
    /// While a played minion's Battlecry resolves (before it joins the board), the board slot it
    /// will take: kept between the same neighbours when the Battlecry destroys a minion
    /// (`docs/tavern.md` §7.1).
    pub play_slot: Option<usize>,
    /// While a Tavern trigger pass resolves ([`with_deferred_triples`](Self::with_deferred_triples)):
    /// its Triples wait for it to end (`docs/tavern.md` §5).
    #[serde(skip)]
    pub triples_deferred: bool,
    pub next_willbreaker_group: u32,
    pub last_combat_won: bool,
    pub last_combat_lost: bool,
    pub include_shop_spells: bool,
    pub auras: PlayerAuras,
    /// The heroes offered to the player (`docs/heroes.md` §6): while there are any, the only
    /// legal action is `ChooseHero`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hero_options: Vec<HeroId>,
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
            pending_choice_repeats: 0,
            repeating_cast: false,
            combine_choose_one: false,
            defer_combined_choose_one: false,
            pending_combined_choices: Vec::new(),
            play_slot: None,
            triples_deferred: false,
            next_willbreaker_group: 1,
            last_combat_won: false,
            last_combat_lost: false,
            include_shop_spells: false,
            auras: PlayerAuras::default(),
            hero_options: Vec::new(),
        }
    }

    /// The player's hero, if they have one (`docs/heroes.md` §2.2).
    pub fn hero(&self) -> Option<&HeroState> {
        self.auras.hero.as_ref()
    }

    /// Give the player the hero `id` (`docs/heroes.md` §2.3): its Hero Power, its Health beyond
    /// 30 and its Armor (its Duos Armor in a Duos game, where the team's pool takes them), then
    /// its start-of-game text. A previous hero's Health and Armor are taken back first.
    pub fn set_hero(&mut self, id: HeroId) -> Result<(), String> {
        let hero = heroes::data(id).ok_or_else(|| format!("unknown hero id {id}"))?;
        let duos = self.auras.duos_game;
        if let Some(old) = self.auras.hero.take().and_then(|h| heroes::data(h.hero_id)) {
            let (health, armor) = heroes::starting_bonus(old, duos);
            self.health -= health;
            self.armor -= armor;
        }
        let (health, armor) = heroes::starting_bonus(hero, duos);
        self.health += health;
        self.armor += armor;
        self.auras.hero = Some(heroes::new_state(hero));
        self.hero_options.clear();
        heroes::on_chosen(self);
        self.sync_all_auras();
        Ok(())
    }

    /// Offer the player the heroes `options` to choose from (`docs/heroes.md` §6).
    pub fn offer_heroes(&mut self, options: Vec<HeroId>) {
        self.hero_options = options;
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
        heroes::on_gold_spent(self, amount);
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
        if self.auras.tavern_turn_atk != 0 || self.auras.tavern_turn_hp != 0 {
            unit.add_stats(self.auras.tavern_turn_atk, self.auras.tavern_turn_hp);
        }
    }

    /// Synchronize all persistent "wherever they are" auras across `board`, `hand`, and `shop`.
    pub fn sync_all_auras(&mut self) {
        self.auras.hero_health = Some(self.health);
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

    /// Remove the board minions `remove` picks (left to right), keeping `play_slot` between the
    /// same neighbours.
    fn remove_board_minions(&mut self, mut remove: impl FnMut(&Unit) -> bool) {
        let slot = self.play_slot;
        let (mut idx, mut removed_before_slot) = (0, 0);
        self.board.retain(|u| {
            let removed = remove(u);
            if removed && slot.is_some_and(|s| idx < s) {
                removed_before_slot += 1;
            }
            idx += 1;
            !removed
        });
        if let Some(slot) = &mut self.play_slot {
            *slot -= removed_before_slot;
        }
    }

    /// Destroy `self.board[board_pos]` during the Tavern Phase (`Disguised Graverobber`, `Maw Caster`, `Dead Bellringer`, `Tomb Turning`),
    /// resolving its Deathrattle (with `in_combat = false`) and Reborn resummon. While a minion is
    /// being played (`play_slot`), the summons leave room for it, and those in the destroyed
    /// minion's place are on its side of the played minion.
    pub fn destroy_board_unit(&mut self, board_pos: usize, pool: &mut CardPool, rng: &mut Rng) {
        if board_pos >= self.board.len() {
            return;
        }
        let dying = self.board.remove(board_pos);
        pool.return_unit(&dying);

        let old_tavern_all = (self.auras.tavern_all_atk, self.auras.tavern_all_hp);
        let mut hand_summoned = vec![false; self.hand.len()];
        let mut hand_given_by = vec![None; self.hand.len()];
        let mut beast_bonus_atk = 0;
        let mut pending_attacks = Vec::new();
        let mut next_id = 1000u32;
        let mut events = Vec::new();
        let mut dr_ctx = BoardCtx {
            side: Side::A,
            in_combat: false,
            board: &mut self.board,
            start_board: &[],
            teammate_board: &[],
            cursor: board_pos,
            reserved_slots: usize::from(self.play_slot.is_some()),
            auras: &mut self.auras,
            hand: &mut self.hand,
            hand_summoned: &mut hand_summoned,
            hand_given_by: &mut hand_given_by,
            dead_aberrations: &[],
            beast_bonus_atk: &mut beast_bonus_atk,
            hero_tier: self.tavern_tier,
            pending_attacks: &mut pending_attacks,
            destroys: Vec::new(),
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
        let summoned_in_place = dr_ctx.cursor.saturating_sub(board_pos);
        if let Some(slot) = &mut self.play_slot {
            if board_pos < *slot {
                *slot = *slot - 1 + summoned_in_place;
            }
        }

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

    /// Add `card`, drawn from `pool`, to `hand`, and return whether it got there. A full hand can't
    /// take it: it is lost, and its copy returns to `pool` (`docs/tavern.md` §2).
    pub fn add_pool_card_to_hand(&mut self, card: Unit, pool: &mut CardPool) -> bool {
        if self.hand.len() >= 10 {
            pool.return_unit(&card);
            false
        } else {
            self.add_to_hand(card);
            true
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

    /// Remove the card at `hand[hand_idx]` to play it (`Play`, `PlayOnShop`). A card of a
    /// `Wandering Willbreaker` / `Faceless Operative` group counts the play down for the rest of
    /// its group, which [`discard_used_group`](Self::discard_used_group) discards once the play
    /// has resolved (`docs/tavern.md` §6, card groups).
    fn take_card_to_play(&mut self, hand_idx: usize) -> Unit {
        let card = self.hand.remove(hand_idx);
        let grp = card.willbreaker_group;
        if grp != 0 {
            for h in &mut self.hand {
                if h.willbreaker_group == grp && h.willbreaker_remaining > 0 {
                    h.willbreaker_remaining -= 1;
                }
            }
        }
        card
    }

    /// Duos: take `hand[hand_idx]` to Pass it (`docs/duos_tavern.md` §5.3). A card of a group
    /// counts down its group as a card taken to be played does, and leaves as a plain card, in
    /// no group. Returns the card and its group, whose rest the caller discards once the Pass
    /// has resolved ([`discard_used_group`](Self::discard_used_group)).
    pub fn take_card_to_pass(&mut self, hand_idx: usize) -> (Unit, u32) {
        let mut card = self.take_card_to_play(hand_idx);
        let group = std::mem::take(&mut card.willbreaker_group);
        card.willbreaker_remaining = 0;
        (card, group)
    }

    /// Once the play of a card of the group `grp` has resolved, discard the rest of the group if
    /// it is used up (right to left; nothing for `grp == 0`, a card of no group). A Duos Pass of
    /// a group card resolves the same way (`docs/duos_tavern.md` §5.3).
    pub fn discard_used_group(&mut self, grp: u32, pool: &mut CardPool, rng: &mut Rng) {
        if grp == 0 {
            return;
        }
        while let Some(idx) = self.hand.iter().rposition(|h| {
            h.willbreaker_group == grp && h.willbreaker_remaining == 0 && h.locked_turns == 0
        }) {
            self.discard_hand_card(idx, pool, rng);
        }
    }

    /// Play the minion `card` (taken from the hand) at `board_pos` (`docs/tavern.md` §7.1):
    /// Magnetize it onto `board[board_pos]` if it can, else resolve its Battlecry and insert it,
    /// then run the play observers; a Golden that gives one offers its Triple Reward.
    fn play_minion(
        &mut self,
        mut card: Unit,
        board_pos: usize,
        pool: &mut CardPool,
        rng: &mut Rng,
    ) {
        let played_card_id = card.card_id;
        let played_tribe = card.tribe;
        let was_golden_minion = card.is_golden;
        let gives_triple_reward = card.is_golden && !card.intrinsic_golden;
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
            cards::magnetize_from_hand(self, &mut card, board_pos, pool, rng);
            pool.return_unit(&card);
        } else {
            cards::on_first_play_or_magnetize(self, &mut card);
            let spellcraft = cards::spellcraft_spell(&card);
            self.combine_choose_one = card.combine_choose_one;
            self.defer_combined_choose_one = true;
            self.play_slot = Some(board_pos.min(self.board.len()));
            cards::on_play_battlecry(self, &mut card, board_pos, pool, rng);
            let insert_idx = self
                .play_slot
                .take()
                .unwrap_or(board_pos)
                .min(self.board.len());
            self.defer_combined_choose_one = false;
            self.combine_choose_one = false;
            // The Battlecry may have changed the auras (a Deathrattle it triggered).
            self.apply_global_unit_auras(&mut card);
            self.board.insert(insert_idx, card);
            self.flush_combined_choices(pool, rng);
            let played = Played {
                card_id: played_card_id,
                tribe: played_tribe,
                board_pos: insert_idx,
                magnetized: false,
                from_hand: true,
            };
            cards::after_play_minion(self, &played, pool, rng);
            if let Some(spell) = spellcraft {
                self.add_to_hand(spell);
            }
            if should_die_on_play
                && insert_idx < self.board.len()
                && self.board[insert_idx].dies_on_play_this_turn
            {
                self.destroy_board_unit(insert_idx, pool, rng);
            }
        }

        if gives_triple_reward {
            let reward_tier = std::cmp::min(6, self.tavern_tier + 1);
            let mut opts = pool.draw_discover_options(reward_tier, 3, rng);
            for opt in &mut opts {
                self.apply_global_unit_auras(opt);
            }
            if !opts.is_empty() {
                self.push_discover(opts);
            }
        }
    }

    /// Resolve a Choose-One prompt (`opt0` vs `opt1`).
    /// If `combine_choose_one` is active or a friendly minion with [`CardFlags::COMBINES_CHOOSE_ONE`]
    /// has charges left (using one), applies both options immediately.
    /// In a repeated cast of a spell (`repeating_cast`), the first cast's choice repeats: the
    /// option chosen from its still-open prompt applies once more (`pending_choice_repeats`), or
    /// if it had both effects combined, both apply again (`docs/tavern.md` §7.4).
    pub fn resolve_choose_one(
        &mut self,
        opt0: Unit,
        opt1: Unit,
        pool: &mut CardPool,
        rng: &mut Rng,
    ) {
        if self.repeating_cast {
            let same_choice = |opts: &Vec<Unit>| {
                opts.len() == 2
                    && opts[0].card_id == opt0.card_id
                    && opts[1].card_id == opt1.card_id
            };
            if self
                .discover_pending
                .iter()
                .chain(&self.discover_queue)
                .any(same_choice)
            {
                self.pending_choice_repeats += 1;
            } else {
                cards::apply_chosen_option(self, &opt0, pool, rng);
                cards::apply_chosen_option(self, &opt1, pool, rng);
                if self.discover_pending.is_none() {
                    self.pending_choice_target = None;
                }
            }
            return;
        }

        let combined = self.combine_choose_one
            || opt0.combine_choose_one
            || cards::use_charge(&mut self.board, CardFlags::COMBINES_CHOOSE_ONE);

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

    /// Whether a Choose One (an option card prompt) is still to be chosen, pending or queued: one
    /// that may need `pending_choice_target`.
    fn choice_prompt_left(&self) -> bool {
        self.discover_pending
            .iter()
            .chain(&self.discover_queue)
            .any(|opts| opts.iter().any(|o| cards::is_option_card(o.card_id)))
    }

    /// The board slot targeted by the Choose One being resolved (`pending_choice_target`), if it
    /// still holds a minion.
    pub fn choice_target(&self) -> Option<usize> {
        match self.pending_choice_target {
            Some(MinionSlot::Board(pos)) if pos < self.board.len() => Some(pos),
            _ => None,
        }
    }

    /// The minion targeted by the Choose One being resolved (`pending_choice_target`), on the
    /// board or in the Tavern.
    pub fn choice_target_mut(&mut self) -> Option<&mut Unit> {
        let slot = self.pending_choice_target?;
        self.minion_mut(slot)
    }

    /// The minion at `slot`, if there is one (a spell in the shop is not a minion).
    pub fn minion(&self, slot: MinionSlot) -> Option<&Unit> {
        match slot {
            MinionSlot::Board(pos) => self.board.get(pos),
            MinionSlot::Shop(idx) => self.shop.get(idx).filter(|u| !u.is_spell),
        }
    }

    /// The minion at `slot`, mutably (see [`minion`](Self::minion)).
    pub fn minion_mut(&mut self, slot: MinionSlot) -> Option<&mut Unit> {
        match slot {
            MinionSlot::Board(pos) => self.board.get_mut(pos),
            MinionSlot::Shop(idx) => self.shop.get_mut(idx).filter(|u| !u.is_spell),
        }
    }

    /// Check whether the next `Refresh` can cost Health instead of Gold: a friendly minion with
    /// [`CardFlags::HEALTH_REFRESHES`] has charges left and the hero can pay.
    pub fn has_health_refresh(&self) -> bool {
        cards::has_charge(&self.board, CardFlags::HEALTH_REFRESHES)
            && (self.health + self.armor > 1 || cards::board_prevents_hero_damage(&self.board))
    }

    /// Effective cost to buy spell `card` from `shop`, in Gold or, if it `costs_health`, in Health
    /// (taking `next_spell_discount` into account).
    pub fn effective_spell_buy_cost(&self, card: &Unit) -> u32 {
        (card.spell_cost as i32 - self.auras.next_spell_discount).max(0) as u32
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

    /// Draw the spell the shop offers: a random Tavern spell of the Tavern's Tier or lower, or in a
    /// Duos game a Duos spell too (`docs/duos_tavern.md` §8.3).
    fn draw_shop_spell(&self, rng: &mut Rng) -> Unit {
        cards::spells::draw_shop_spell(self.tavern_tier, self.auras.duos_game, rng)
    }

    /// Discard the temporary cards still in the hand ([`CardFlags::TEMPORARY`]: Spellcraft
    /// spells), without discard effects (`docs/tavern.md` §7.4).
    fn discard_temporary_cards(&mut self) {
        self.hand.retain(|card| !cards::is_temporary(card));
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
            let spell = self.draw_shop_spell(rng);
            self.shop.push(spell);
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
            let spell = self.draw_shop_spell(rng);
            self.shop.push(spell);
        }
        cards::after_shop_refresh(self, rng);
    }

    /// Execute the start-of-turn sequence (`docs/tavern.md` §4).
    pub fn start_turn(&mut self, pool: &mut CardPool, rng: &mut Rng) {
        self.discard_temporary_cards();
        let won_last = self.last_combat_won;
        self.last_combat_won = false;
        self.gold_spent_this_turn = 0;
        self.elementals_played_this_turn = 0;
        self.auras.expire_turn_effects();
        heroes::turn_reset(self);
        let turn_atk = std::mem::take(&mut self.auras.tavern_turn_atk);
        let turn_hp = std::mem::take(&mut self.auras.tavern_turn_hp);
        if (turn_atk != 0 || turn_hp != 0) && self.is_frozen {
            for s in &mut self.shop {
                if !s.is_spell {
                    s.attack = (s.attack - turn_atk).max(0);
                    s.health = (s.health - turn_hp).max(1);
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
        cards::on_player_turn_start(self);

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
            // Every kept card fills a slot (`cap`, plus the spell slot); the spell slot refills
            // with a spell only if no spell is left, the other free slots with minions.
            let slots = cap + usize::from(self.include_shop_spells);
            let new_spell = self.include_shop_spells && !self.shop.iter().any(|u| u.is_spell);
            let needed = slots.saturating_sub(self.shop.len() + usize::from(new_spell));
            for _ in 0..needed {
                if let Some(mut drawn) = pool.draw_from_pool(self.tavern_tier, rng) {
                    self.apply_shop_auras(&mut drawn);
                    self.shop.push(drawn);
                }
            }
            if new_spell {
                let spell = self.draw_shop_spell(rng);
                self.shop.push(spell);
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
                let spell = self.draw_shop_spell(rng);
                self.shop.push(spell);
            }
            cards::after_shop_refresh(self, rng);
        }
        heroes::on_turn_start(self, pool, rng);
        cards::on_start_turn_spellcrafts(self);
        self.sync_all_auras();
        self.resolve_triples();
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
        let result = if won {
            CombatResult::Won
        } else if lost {
            CombatResult::Lost
        } else {
            CombatResult::Tied
        };
        // Hand cards that became ready during combat (e.g. a sped-up `Lockbox`) resolve with a
        // per-side RNG derived from the battle seed.
        let ready_salt = match side {
            Side::A => 0x9E37_79B9_7F4A_7C15,
            Side::B => 0xBF58_476D_1CE4_E5B9,
        };
        let mut ready_rng = Rng::new(seed ^ ready_salt);
        self.apply_combat_return(
            pre_combat_snapshot,
            new_auras,
            new_hand,
            survivors,
            dead_units,
            result,
            &mut ready_rng,
        );

        if lost {
            // Combat damage triggers no hero-damage minion (`docs/tavern.md` §7.2).
            let dmg = res.hero_damage as i32;
            let absorbed = dmg.min(self.armor.max(0));
            self.armor -= absorbed;
            self.health -= dmg - absorbed;
        }
        self.sync_all_auras();
    }

    /// Bring back what a combat gives this player's Tavern (Combat-to-Tavern Persistence,
    /// `docs/combat.md` §1), but not the hero damage: their auras after the combat (Tavern auras
    /// gained in combat also buff the shop), their after-combat player effects with `result`,
    /// their hand after the combat, the hand cards that became ready (resolved with `ready_rng`),
    /// what their minions keep from combat (`survivors` and `dead_units`, matched to
    /// `pre_combat_snapshot` by unit id), Triples, and the result. A Solo side's result
    /// and a Duos player's (`docs/duos_tavern.md` §6.2) both come back through here.
    #[allow(clippy::too_many_arguments)]
    pub fn apply_combat_return(
        &mut self,
        pre_combat_snapshot: &[Unit],
        new_auras: PlayerAuras,
        new_hand: Vec<Unit>,
        survivors: &[Unit],
        dead_units: &[Unit],
        result: CombatResult,
        ready_rng: &mut Rng,
    ) {
        // Tavern auras gained in combat also buff the minions already in the Tavern
        // (`docs/combat.md` §1, Combat-to-Tavern Persistence).
        let d_tavern_atk = new_auras.tavern_all_atk - self.auras.tavern_all_atk;
        let d_tavern_hp = new_auras.tavern_all_hp - self.auras.tavern_all_hp;
        let d_elemental_atk = new_auras.tavern_elemental_atk - self.auras.tavern_elemental_atk;
        let d_elemental_hp = new_auras.tavern_elemental_hp - self.auras.tavern_elemental_hp;
        self.auras = new_auras;
        if d_tavern_atk != 0 || d_tavern_hp != 0 {
            for s in &mut self.shop {
                if !s.is_spell {
                    s.add_stats(d_tavern_atk, d_tavern_hp);
                }
            }
        }
        if d_elemental_atk != 0 || d_elemental_hp != 0 {
            for s in &mut self.shop {
                if !s.is_spell && s.tribe.matches(Tribe::Elemental) {
                    s.add_stats(d_elemental_atk, d_elemental_hp);
                }
            }
        }

        cards::on_player_after_combat(self, result);
        self.hand = new_hand;
        cards::resolve_ready_hand_cards(self, ready_rng);

        let mut post_units = survivors.to_vec();
        post_units.extend_from_slice(dead_units);
        for (idx, tavern_unit) in self.board.iter_mut().enumerate() {
            cards::on_post_combat_unit(pre_combat_snapshot, idx, &post_units, tavern_unit);
        }

        self.resolve_triples();

        self.last_combat_won = result == CombatResult::Won;
        self.last_combat_lost = result == CombatResult::Lost;
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
        if !self.hero_options.is_empty() {
            return matches!(
                action,
                TavernAction::ChooseHero { option_index } if *option_index < self.hero_options.len()
            );
        }
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
                    let cost = self.effective_spell_buy_cost(card);
                    if card.costs_health {
                        self.health + self.armor > cost as i32
                            || cards::board_prevents_hero_damage(&self.board)
                    } else {
                        self.gold >= cost
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
            TavernAction::PlayOnShop {
                hand_index,
                shop_index,
            } => {
                let Some(card) = self.hand.get(hand_index) else {
                    return false;
                };
                if card.unplayable || card.lockbox_turns_left > 0 || card.locked_turns > 0 {
                    return false;
                }
                card.is_spell
                    && self
                        .shop
                        .get(shop_index)
                        .is_some_and(|unit| cards::spells::can_target_in_shop(card.card_id, unit))
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
            // Solo has no teammate to Pass to.
            TavernAction::Pass { .. } => false,
            TavernAction::HeroPower { target } => heroes::can_use(self, target),
            TavernAction::ChooseHero { .. } => false,
        }
    }

    /// Return all currently legal `TavernAction`s.
    pub fn valid_actions(&self) -> Vec<TavernAction> {
        let mut actions = Vec::new();
        if !self.hero_options.is_empty() {
            return (0..self.hero_options.len())
                .map(|option_index| TavernAction::ChooseHero { option_index })
                .collect();
        }
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
                    for (s_idx, unit) in self.shop.iter().enumerate() {
                        if cards::spells::can_target_in_shop(card.card_id, unit) {
                            actions.push(TavernAction::PlayOnShop {
                                hand_index: h_idx,
                                shop_index: s_idx,
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
        actions.extend(
            heroes::valid_targets(self)
                .into_iter()
                .map(|target| TavernAction::HeroPower { target }),
        );
        actions
    }

    /// Execute one `TavernAction`. Returns `Ok(true)` if the turn ended (`EndTurn`),
    /// `Ok(false)` otherwise, or `Err` if the action is illegal. The copies that came together
    /// during the action form their Triples once it has resolved (`docs/tavern.md` §5).
    pub fn step(
        &mut self,
        action: TavernAction,
        pool: &mut CardPool,
        rng: &mut Rng,
    ) -> Result<bool, String> {
        if !self.is_legal(&action) {
            return Err(format!("illegal tavern action: {action:?}"));
        }
        let ended = self.apply_action(action, pool, rng)?;
        self.resolve_triples();
        // The minions' "wherever this is" auras and conditional keywords are up to date between
        // actions (a Divine Shield a spell gave a `Relentless Deflector` gives it Taunt at once).
        self.sync_all_auras();
        Ok(ended)
    }

    /// The effect of the legal `action` (see [`step`](Self::step)).
    fn apply_action(
        &mut self,
        action: TavernAction,
        pool: &mut CardPool,
        rng: &mut Rng,
    ) -> Result<bool, String> {
        match action {
            TavernAction::Buy { shop_index } => {
                // The bought card takes its hand slot before its price is paid, so the triggers
                // of the payment see it there (`docs/tavern.md` §6).
                let mut unit = self.shop.remove(shop_index);
                let bought_snapshot = unit.clone();
                let obtained;
                if unit.is_spell {
                    let cost = self.effective_spell_buy_cost(&unit);
                    self.auras.next_spell_discount = 0;
                    let costs_health = unit.costs_health;
                    obtained = unit.clone();
                    self.hand.push(unit);
                    cards::on_card_added_to_hand(&self.board, &mut self.auras);
                    if costs_health {
                        self.deal_hero_damage(cost as i32);
                    } else {
                        self.spend_gold(cost, pool, rng);
                    }
                } else {
                    cards::on_minion_bought(self, &mut unit);
                    let cid = unit.card_id;
                    obtained = unit.clone();
                    self.hand.push(unit);
                    cards::on_card_added_to_hand(&self.board, &mut self.auras);
                    self.check_and_resolve_triple(cid);
                    self.spend_gold(3, pool, rng);
                }
                cards::on_obtained(self, &obtained, pool, rng);
                cards::after_buy_card(self, &bought_snapshot, pool, rng);
                heroes::after_buy(self, &bought_snapshot, pool, rng);
                Ok(false)
            }
            TavernAction::Play {
                hand_index,
                board_pos,
            } => {
                let card = self.take_card_to_play(hand_index);
                let group = card.willbreaker_group;
                if card.is_spell {
                    cards::spells::cast_spell(self, card, board_pos, pool, rng);
                } else {
                    self.play_minion(card, board_pos, pool, rng);
                }
                self.discard_used_group(group, pool, rng);
                Ok(false)
            }
            TavernAction::PlayOnShop {
                hand_index,
                shop_index,
            } => {
                let card = self.take_card_to_play(hand_index);
                let group = card.willbreaker_group;
                let target = MinionSlot::Shop(shop_index);
                cards::spells::cast_spell_on(self, card, target, pool, rng);
                self.discard_used_group(group, pool, rng);
                Ok(false)
            }
            TavernAction::Sell { board_pos } => {
                let sold = self.board.remove(board_pos);
                pool.return_unit(&sold);
                self.gold += 1;
                cards::on_sell(self, &sold, pool, rng);
                heroes::after_sell(self, &sold, pool, rng);
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
                    // Still a Health Refresh if a charge is left: it uses the charge, for 0 Health.
                    self.auras.free_refreshes -= 1;
                    cards::use_charge(&mut self.board, CardFlags::HEALTH_REFRESHES);
                } else if self.has_health_refresh() {
                    cards::use_charge(&mut self.board, CardFlags::HEALTH_REFRESHES);
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
                heroes::after_upgrade(self, pool, rng);
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
                    // A Choose One spell cast several times was chosen once: once per cast.
                    let mut repeats = 0;
                    if cards::hooks(chosen.card_id).has(CardFlags::CHOOSE_ONE_OPTION) {
                        repeats = std::mem::take(&mut self.pending_choice_repeats);
                    }
                    for _ in 0..=repeats {
                        cards::apply_chosen_option(self, &chosen, pool, rng);
                    }
                    cards::after_choose_one(self, &chosen, pool, rng);
                    if !self.choice_prompt_left() {
                        self.pending_choice_target = None;
                    }
                    return Ok(false);
                }
                // Another prompt after a Choose One (a Triple Reward): its target is done with,
                // unless a Choose One is still to come.
                if !self.choice_prompt_left() {
                    self.pending_choice_target = None;
                }

                for mut unchosen in opts {
                    if std::mem::take(&mut unchosen.unchosen_to_teammate) {
                        // Duos `Bargain Bundle`: the teammate gets the other options.
                        self.auras.give_teammate(TeammateGift::Card(unchosen));
                    } else {
                        pool.return_unit(&unchosen);
                    }
                }
                chosen.unchosen_to_teammate = false;
                // The card reaches the hand before its Tier damage (`docs/tavern.md` §7.2).
                let tier_damage = std::mem::take(&mut chosen.discover_deals_tier_damage)
                    .then_some(chosen.tavern_tier as i32);
                let obtained = chosen.clone();
                if self.add_pool_card_to_hand(chosen, pool) {
                    cards::on_obtained(self, &obtained, pool, rng);
                }
                if let Some(dmg) = tier_damage {
                    self.deal_hero_damage(dmg);
                }
                cards::on_card_discovered(self, pool, rng);
                Ok(false)
            }
            TavernAction::EndTurn => {
                cards::on_end_turn(self, pool, rng);
                self.discard_temporary_cards();
                Ok(true)
            }
            TavernAction::Pass { .. } => Err(format!(
                "illegal tavern action: {action:?} (a Pass needs a teammate)"
            )),
            TavernAction::HeroPower { target } => {
                heroes::use_power(self, target, pool, rng);
                Ok(false)
            }
            TavernAction::ChooseHero { option_index } => {
                let id = self.hero_options[option_index];
                self.set_hero(id)?;
                Ok(false)
            }
        }
    }

    /// Run `f`, a Tavern trigger pass, with its Triples deferred until it returns, then resolve
    /// them (`docs/tavern.md` §5): no minion of the pass moves under it. A pass inside a pass
    /// defers to the outer one.
    pub fn with_deferred_triples<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        if self.triples_deferred {
            return f(self);
        }
        self.triples_deferred = true;
        let out = f(self);
        self.triples_deferred = false;
        self.resolve_triples();
        out
    }

    /// Form a Triple from every 3 non-Golden copies that have come together across `board` and
    /// `hand` (`docs/tavern.md` §5): checked after each action, start of turn and combat, and once
    /// each Tavern trigger pass is over.
    pub fn resolve_triples(&mut self) {
        loop {
            let cards = self.board.len() + self.hand.len();
            let mut cids: Vec<CardId> = Vec::new();
            for u in self.board.iter().chain(&self.hand) {
                if !u.is_spell && !u.is_golden && !cids.contains(&u.card_id) {
                    cids.push(u.card_id);
                }
            }
            for cid in cids {
                self.check_and_resolve_triple(cid);
            }
            // A Triple takes 3 cards for 1; stop once a round forms none.
            if self.board.len() + self.hand.len() == cards {
                return;
            }
        }
    }

    /// Combine 3 non-golden copies of `card_id` across `board` and `hand` into 1 Golden copy in `hand` (`docs/tavern.md` §5.1).
    /// Cards flagged [`CardFlags::TRIPLE_WILDCARD_ELEMENTAL`] (`Elemental of Surprise`) can triple
    /// with any Elemental; cards flagged [`CardFlags::NO_TRIPLE`] never triple. During a Tavern
    /// trigger pass, the check waits for the pass to end
    /// ([`with_deferred_triples`](Self::with_deferred_triples)), and a Triple whose Golden has no
    /// room in the hand waits for room.
    pub fn check_and_resolve_triple(&mut self, card_id: CardId) {
        let no_triple = |cid: CardId| cards::hooks(cid).has(CardFlags::NO_TRIPLE);
        let is_wildcard = |cid: CardId| cards::hooks(cid).has(CardFlags::TRIPLE_WILDCARD_ELEMENTAL);
        if card_id == 0 || no_triple(card_id) {
            return;
        }
        if self.triples_deferred {
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

        // The Golden goes to the hand: unless enough copies leave the hand, a full hand makes the
        // Triple wait for room (`docs/tavern.md` §5). The copies are taken from the board first,
        // then from the hand, exact copies before wildcards.
        let exact = |u: &&Unit| is_eligible(u) && u.card_id == target_cid;
        let board_exact = self.board.iter().filter(exact).count().min(3);
        let mut from_hand = self.hand.iter().filter(exact).count().min(3 - board_exact);
        if use_wildcard {
            let wildcard = |u: &&Unit| is_eligible(u) && is_wildcard(u.card_id);
            let left = 3 - board_exact - from_hand;
            from_hand += left - self.board.iter().filter(wildcard).count().min(left);
        }
        if self.hand.len() - from_hand >= 10 {
            return;
        }

        let mut copies = Vec::with_capacity(3);
        self.remove_board_minions(|u| {
            let take = copies.len() < 3 && is_eligible(u) && u.card_id == target_cid;
            if take {
                copies.push(u.clone());
            }
            take
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
            self.remove_board_minions(|u| {
                let take = copies.len() < 3 && is_eligible(u) && is_wildcard(u.card_id);
                if take {
                    copies.push(u.clone());
                }
                take
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

        // Buffs carry over; aura bonuses are re-applied to the Golden by the next aura sync.
        let atk_buffs: i32 = copies
            .iter()
            .map(|u| (u.attack - u.base_attack) - u.aura_applied.0)
            .sum();
        let hp_buffs: i32 = copies
            .iter()
            .map(|u| (u.health - u.base_health) - u.aura_applied.1)
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
        golden.granted_taunt = copies.iter().any(|u| u.granted_taunt);
        golden.divine_shield = copies.iter().any(|u| u.divine_shield);
        // A Divine Shield the copies only have until next turn stays temporary.
        golden.temp_divine_shield = golden.divine_shield
            && copies
                .iter()
                .all(|u| !u.divine_shield || u.temp_divine_shield);
        golden.inherent_divine_shield = copies.iter().any(|u| u.inherent_divine_shield);
        golden.windfury = copies.iter().any(|u| u.windfury);
        golden.reborn = copies.iter().any(|u| u.reborn);
        golden.venomous = copies.iter().any(|u| u.venomous);
        golden.stealth = copies.iter().any(|u| u.stealth);
        golden.magnetic = copies.iter().any(|u| u.magnetic);
        golden.threshold_triggered = copies.iter().any(|u| u.threshold_triggered);
        golden.counter = copies.iter().map(|u| u.counter).max().unwrap_or(0);
        golden.stacks = copies.iter().map(|u| u.stacks).sum();
        golden.eot_health_bonus = copies.iter().map(|u| u.eot_health_bonus).sum();
        golden.sot_gold_bonus = copies.iter().map(|u| u.sot_gold_bonus).sum();
        golden.pass_aura = copies.iter().map(|u| u.pass_aura).sum();
        cards::init_unit_turn_charges(&mut golden);
        // The Golden's own spell aura comes from its card; what the copies gained on top of
        // theirs carries over.
        cards::init_spell_aura(&mut golden);
        for copy in &copies {
            let (atk, hp) = cards::gained_spell_aura(copy);
            golden.spell_atk_aura += atk;
            golden.spell_hp_aura += hp;
        }
        golden.perm_atk_gained = copies.iter().map(|u| u.perm_atk_gained).sum();
        golden.perm_hp_gained = copies.iter().map(|u| u.perm_hp_gained).sum();
        golden.perm_blood_gems_gained = copies.iter().map(|u| u.perm_blood_gems_gained).sum();
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
