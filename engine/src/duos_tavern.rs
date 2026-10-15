//! A Duos team's Tavern phase (`docs/duos_tavern.md`): two players' Taverns with one Health
//! pool, simultaneous turns, Passes between teammates, and the team's combat.
//!
//! Each player acts through their own [`TavernState`], exactly as in Solo; this module adds only
//! what the team shares. It stays card-agnostic: a free Pass is a charge of a
//! [`CardFlags::FREE_PASS`] minion (`Wanderer Cho`), and the Pass hooks (`passed`, `after_pass`,
//! `after_team_pass`) are dispatched through [`crate::cards`].

use serde::{Deserialize, Serialize};

use crate::cards::{self, CardFlags};
use crate::combat::{
    resolve_duos_battle, DuosResult, DuosTeam, Warband, WarbandResult, MAX_BOARD_SIZE,
};
use crate::model::{
    BattleOutcome, CombatResult, EffectDuration, HeroId, PlayerEffect, Side, TeammateGift, Unit,
};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernAction, TavernState};

/// The team's starting Health (`docs/duos_tavern.md` §3).
pub const TEAM_HEALTH: i32 = 30;

/// The default chance that the first fighter switches each turn (`docs/duos_tavern.md` §4.3).
pub const DEFAULT_FIRST_FIGHTER_SWITCH_CHANCE: f64 = 0.75;

/// A player's hand limit (`docs/tavern.md` §3.2).
const MAX_HAND_SIZE: usize = 10;

/// Salts of the RNGs that resolve each player's hand cards that became ready in combat
/// (`docs/duos_tavern.md` §6.2), by team (side A, side B) and player.
const READY_SALTS: [[u64; 2]; 2] = [
    [0xD1B5_4A32_D192_ED03, 0xAEF1_7502_108E_F2D9],
    [0x8CB9_2BA7_2F3D_8DD7, 0xDB4F_0B91_75AE_2165],
];

/// Salts of the RNGs of the gifts the players' cards gave in combat that the team's Tavern
/// delivers (`docs/duos_tavern.md` §7.2), by team (side A, side B).
const GIFT_SALTS: [u64; 2] = [0x9FB2_1C65_1E98_DF25, 0xC2B2_AE3D_27D4_EB4F];

/// The most rounds of gifts delivered at once (§7.2): a gift's hooks may give more gifts, but
/// never endlessly.
const MAX_GIFT_ROUNDS: usize = 64;

/// One Duos team's Tavern phase (`docs/duos_tavern.md` §2).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DuosTavern {
    /// Each player's own Tavern: player 0, player 1.
    pub players: [TavernState; 2],
    /// The team's shared Health; each player's `health` mirrors it (§3).
    pub health: i32,
    /// The team's shared Armor; each player's `armor` mirrors it (§3).
    pub armor: i32,
    /// The team's Passes this turn (reset at the start of each turn) and this game (§7).
    pub passes_this_turn: u32,
    pub passes_this_game: u32,
    /// Each player's queue of passed cards waiting for room in their hand, first in first out
    /// (§5.4).
    pub incoming: [Vec<Unit>; 2],
    /// Which players have ended their turn (§4.2).
    pub ended: [bool; 2],
    /// The player whose warband fights first in the coming combat (§4.3).
    pub first_fighter: usize,
    /// The chance that the first fighter switches at the start of each turn after the first
    /// (§4.3).
    pub first_fighter_switch_chance: f64,
    /// Whose warband did not fight in the last combat: it keeps its next-combat effects through
    /// the next start of turn (§6.3).
    pub sat_out: [bool; 2],
}

impl Default for DuosTavern {
    fn default() -> Self {
        Self::new()
    }
}

impl DuosTavern {
    /// A new team: two fresh Taverns, 30 Health, no Armor.
    pub fn new() -> Self {
        let mut team = Self {
            players: [TavernState::new(), TavernState::new()],
            health: TEAM_HEALTH,
            armor: 0,
            passes_this_turn: 0,
            passes_this_game: 0,
            incoming: [Vec::new(), Vec::new()],
            ended: [false; 2],
            first_fighter: 0,
            first_fighter_switch_chance: DEFAULT_FIRST_FIGHTER_SWITCH_CHANCE,
            sat_out: [false; 2],
        };
        team.mirror();
        team
    }

    /// The team starts with both heroes' Armor, `armor[0] + armor[1]` (§3.1).
    pub fn with_armor(mut self, armor: [i32; 2]) -> Self {
        self.armor = armor[0] + armor[1];
        self.mirror();
        self
    }

    /// Give the players their heroes (`docs/heroes.md` §2.3): each one's Hero Power, and the
    /// team's pool their Duos Armor and their Health beyond 30.
    pub fn with_heroes(mut self, heroes: [Option<HeroId>; 2]) -> Result<Self, String> {
        for (player, hero) in heroes.into_iter().enumerate() {
            if let Some(id) = hero {
                self.set_hero(player, id)?;
            }
        }
        Ok(self)
    }

    /// Give `player` the hero `id` (`docs/heroes.md` §2.3), the team's pool taking its Duos Armor
    /// and its Health beyond 30.
    pub fn set_hero(&mut self, player: usize, id: HeroId) -> Result<(), String> {
        self.mirror();
        self.players
            .get_mut(player)
            .ok_or_else(|| format!("a Duos team's players are 0 and 1, got {player}"))?
            .set_hero(id)?;
        self.settle();
        Ok(())
    }

    /// Enable or disable Tavern spells in both players' shops (`TavernState::with_shop_spells`).
    pub fn with_shop_spells(mut self, enabled: bool) -> Self {
        for state in &mut self.players {
            state.include_shop_spells = enabled;
        }
        self
    }

    /// Set the chance that the first fighter switches each turn (§4.3).
    pub fn with_first_fighter_switch_chance(mut self, chance: f64) -> Self {
        self.first_fighter_switch_chance = chance;
        self
    }

    /// `player`'s teammate.
    pub fn teammate(player: usize) -> usize {
        1 - player
    }

    /// `true` if the team is out: its Health is 0 or less (§3.6).
    pub fn is_out(&self) -> bool {
        self.health <= 0
    }

    /// `true` once both players have ended their turn: the team's turn is over (§4.2).
    pub fn turn_over(&self) -> bool {
        self.ended == [true, true]
    }

    /// The free Passes `player` has left this turn: their `FREE_PASS` minions' charges (§5.2).
    pub fn free_passes(&self, player: usize) -> u32 {
        self.players.get(player).map_or(0, |state| {
            cards::charges_left(&state.board, CardFlags::FREE_PASS)
        })
    }

    /// Start the team's turn (§4.1): reset the turn's counter, run both players' start of turn
    /// (player 0's first), deliver the gifts and the queues, then draw the first fighter.
    pub fn start_turn(&mut self, pool: &mut CardPool, rng: &mut Rng) {
        self.passes_this_turn = 0;
        self.ended = [false; 2];
        self.mirror();
        for player in 0..2 {
            // A warband that sat out the last combat keeps its unused next-combat effects (§6.3).
            let kept: Vec<PlayerEffect> = if std::mem::take(&mut self.sat_out[player]) {
                self.players[player]
                    .auras
                    .effects
                    .iter()
                    .filter(|e| e.duration == EffectDuration::Turn)
                    .filter(|e| cards::is_start_of_combat_effect(e))
                    .copied()
                    .collect()
            } else {
                Vec::new()
            };
            let state = &mut self.players[player];
            state.start_turn(pool, rng);
            for effect in kept {
                state
                    .auras
                    .add_effect(effect.card_id, effect.stacks, effect.duration);
            }
            self.settle();
        }
        self.deliver_gifts(0, Some(pool), rng);
        self.deliver();
        self.draw_first_fighter(rng);
    }

    /// Draw the coming combat's first fighter (§4.3): a fair coin on the first turn, then a
    /// switch with `first_fighter_switch_chance`. Always exactly one draw.
    fn draw_first_fighter(&mut self, rng: &mut Rng) {
        let v = rng.next_u64();
        if self.players[0].turn <= 1 {
            self.first_fighter = (v % 2) as usize;
        } else {
            let roll = (v >> 11) as f64 / (1u64 << 53) as f64;
            if roll < self.first_fighter_switch_chance {
                self.first_fighter = 1 - self.first_fighter;
            }
        }
    }

    /// Check whether `player` may take `action` now (§4.2, §5.1).
    pub fn is_legal(&self, player: usize, action: &TavernAction) -> bool {
        let Some(state) = self.players.get(player) else {
            return false;
        };
        if self.ended[player] {
            return false;
        }
        match *action {
            TavernAction::Pass { hand_index } => self.can_pass(player, hand_index),
            _ => state.is_legal(action),
        }
    }

    /// Every action `player` may take now: their Tavern's, then their Passes.
    pub fn valid_actions(&self, player: usize) -> Vec<TavernAction> {
        let Some(state) = self.players.get(player) else {
            return Vec::new();
        };
        if self.ended[player] {
            return Vec::new();
        }
        let mut actions = state.valid_actions();
        for hand_index in 0..state.hand.len() {
            if self.can_pass(player, hand_index) {
                actions.push(TavernAction::Pass { hand_index });
            }
        }
        actions
    }

    /// `true` if `player` may Pass `hand[hand_index]` (§5.1): no Discover pending, a card that
    /// isn't locked, and a free Pass or 1 Gold.
    fn can_pass(&self, player: usize, hand_index: usize) -> bool {
        let state = &self.players[player];
        if state.discover_pending.is_some() || !state.hero_options.is_empty() {
            return false;
        }
        let Some(card) = state.hand.get(hand_index) else {
            return false;
        };
        !Self::is_locked(card) && (self.free_passes(player) > 0 || state.gold >= 1)
    }

    /// A locked card can't be passed (§5.3): one locked until next turn (`Search Through Time`)
    /// or still in a `Lockbox`.
    fn is_locked(card: &Unit) -> bool {
        card.locked_turns > 0 || card.lockbox_turns_left > 0
    }

    /// `player` takes `action` (§4.2): a Pass, or an action of their own Tavern. Returns
    /// `Ok(true)` if it ended their turn, or `Err` if it is illegal. Afterwards the teammate's
    /// observers see a sold minion (§7.3), the team's pool takes the Health and Armor the action
    /// changed, and the gifts (§7.2) and the queues deliver.
    pub fn step(
        &mut self,
        player: usize,
        action: TavernAction,
        pool: &mut CardPool,
        rng: &mut Rng,
    ) -> Result<bool, String> {
        if player > 1 {
            return Err(format!("a Duos team's players are 0 and 1, got {player}"));
        }
        if self.ended[player] {
            return Err(format!(
                "illegal tavern action: {action:?} (player {player} has ended their turn)"
            ));
        }
        if !self.is_legal(player, &action) {
            return Err(format!("illegal tavern action: {action:?}"));
        }
        self.mirror();
        let ended = match action {
            TavernAction::Pass { hand_index } => {
                self.pass(player, hand_index, pool, rng);
                false
            }
            _ => self.players[player].step(action, pool, rng)?,
        };
        if ended {
            self.ended[player] = true;
        }
        // Every sale of the step, whatever made it (docs/duos_tavern.md §7.3).
        let mate = Self::teammate(player);
        for sold in std::mem::take(&mut self.players[player].sold_this_step) {
            cards::after_teammate_sell(&mut self.players[mate], &sold, pool, rng);
        }
        self.settle();
        self.deliver_gifts(player, Some(pool), rng);
        self.deliver();
        Ok(ended)
    }

    /// `player` Passes `hand[hand_index]` to their teammate (§5.6). A card of a group counts down
    /// its group, whose rest is discarded once the Pass has resolved if it is used up, as after a
    /// play (§5.3).
    fn pass(&mut self, player: usize, hand_index: usize, pool: &mut CardPool, rng: &mut Rng) {
        let passer = &mut self.players[player];
        let (card, group) = passer.take_card_to_pass(hand_index);
        // A free Pass first (a charge of the left-most FREE_PASS minion), else 1 Gold spent.
        if !cards::use_charge(&mut passer.board, CardFlags::FREE_PASS) {
            passer.spend_gold(1, pool, rng);
        }
        self.send_pass(player, card, pool, rng);
        self.players[player].discard_used_group(group, pool, rng);
    }

    /// `player` Passes `card`, which has left their hand or wherever it was, to their teammate:
    /// a Pass from its `passed` hook on (§5.6 steps 3-6), the whole of a Pass by a card effect
    /// (§7.2). It arrives as a plain card, in no group (§5.3). The team's pool first takes the
    /// Health and Armor the players' observers changed (those of the Pass's Gold, or of an
    /// earlier gift), which mirroring the counters would otherwise undo.
    fn send_pass(&mut self, player: usize, mut card: Unit, pool: &mut CardPool, rng: &mut Rng) {
        let mate = Self::teammate(player);
        cards::on_passed(&mut card);
        card.willbreaker_group = 0;
        card.willbreaker_remaining = 0;
        self.passes_this_turn += 1;
        self.passes_this_game += 1;
        self.settle();
        cards::after_pass(&mut self.players[player], &card, pool, rng);
        cards::after_team_pass(&mut self.players[player], pool, rng);
        cards::after_team_pass(&mut self.players[mate], pool, rng);
        self.incoming[mate].push(card);
    }

    /// Deliver the gifts the players' cards gave (§7.2): each player's in order, `first`'s
    /// first, until none is left. A gift card joins the receiver's incoming queue, as a passed
    /// card does (the caller delivers the queues); a Pass is sent (§5.6); an `Effect` runs its
    /// card's `teammate_gift` hook on the teammate's Tavern, or on both players' if it is the
    /// team's. Without a `pool` (after a combat) the Passes wait for the next delivery. The team's
    /// pool takes the Health and Armor the deliveries' observers change (`settle`) as it goes.
    fn deliver_gifts(&mut self, first: usize, mut pool: Option<&mut CardPool>, rng: &mut Rng) {
        for _ in 0..MAX_GIFT_ROUNDS {
            let mut delivered = false;
            for player in [first, Self::teammate(first)] {
                let gifts = std::mem::take(&mut self.players[player].auras.to_teammate);
                let mut waiting = Vec::new();
                for gift in gifts {
                    match gift {
                        TeammateGift::Pass(card) => match pool.as_deref_mut() {
                            Some(pool) => self.send_pass(player, card, pool, rng),
                            None => {
                                waiting.push(TeammateGift::Pass(card));
                                continue;
                            }
                        },
                        TeammateGift::Card(card) => {
                            self.incoming[Self::teammate(player)].push(card)
                        }
                        TeammateGift::Effect {
                            card_id,
                            amount,
                            team,
                        } => {
                            let mate = Self::teammate(player);
                            let receivers: &[usize] = if team { &[player, mate] } else { &[mate] };
                            for &receiver in receivers {
                                self.settle();
                                let state = &mut self.players[receiver];
                                let cards = cards::apply_teammate_gift(state, card_id, amount, rng);
                                state.sync_all_auras();
                                self.incoming[receiver].extend(cards);
                                self.settle();
                            }
                        }
                    }
                    delivered = true;
                }
                // Keep the waiting Passes ahead of any gift their delivery added.
                let added = std::mem::take(&mut self.players[player].auras.to_teammate);
                waiting.extend(added);
                self.players[player].auras.to_teammate = waiting;
            }
            if !delivered {
                break;
            }
        }
        self.settle();
    }

    /// Move queued passed cards into their receivers' hands while they have room (§5.4): player
    /// 0's queue, then player 1's. Each card goes through the receiver's `add_to_hand` (their
    /// auras, observers and triple check). A player's own cards waiting for room (a Spellcraft
    /// spell, §8.3) go first.
    fn deliver(&mut self) {
        for player in 0..2 {
            self.players[player].deliver_waiting_for_hand();
            while self.players[player].hand.len() < MAX_HAND_SIZE
                && !self.incoming[player].is_empty()
            {
                let card = self.incoming[player].remove(0);
                self.players[player].add_to_hand(card);
            }
        }
    }

    /// Copy the team's Health, Armor and counters into both players, and each player's card
    /// counters into their teammate's `teammate_counters` (§3.3, §7.1), re-syncing the auras of a
    /// player whose values changed. Both players' auras say they are in a Duos game (§8).
    pub fn mirror(&mut self) {
        let (health, armor) = (self.health, self.armor);
        let (this_turn, this_game) = (self.passes_this_turn, self.passes_this_game);
        let counters = [
            self.players[1].auras.card_counters.clone(),
            self.players[0].auras.card_counters.clone(),
        ];
        for (state, teammate_counters) in self.players.iter_mut().zip(counters) {
            state.auras.duos_game = true;
            let same = state.health == health
                && state.armor == armor
                && state.auras.team_passes_this_turn == this_turn
                && state.auras.team_passes_this_game == this_game
                && state.auras.teammate_counters == teammate_counters;
            if !same {
                state.health = health;
                state.armor = armor;
                state.auras.team_passes_this_turn = this_turn;
                state.auras.team_passes_this_game = this_game;
                state.auras.teammate_counters = teammate_counters;
                state.sync_all_auras();
            }
        }
    }

    /// Gather the Health and Armor changes the players' own Taverns made into the team's pool,
    /// then mirror the pool back into both (§3.3). [`step`](Self::step) does it after every
    /// action; call it after changing a player's `TavernState` directly.
    pub fn settle(&mut self) {
        let (health, armor) = (self.health, self.armor);
        for state in &self.players {
            self.health += state.health - health;
            self.armor += state.armor - armor;
        }
        self.mirror();
    }

    /// `player`'s warband for the team's combat (§6.1): their board (at most 7 minions), hand,
    /// auras and Tavern Tier. Their queued cards are not in it.
    pub fn warband(&self, player: usize) -> Warband {
        let state = &self.players[player];
        let mut board = state.board.clone();
        board.truncate(MAX_BOARD_SIZE);
        Warband {
            board,
            hand: state.hand.clone(),
            auras: state.auras.clone(),
            hero_tier: state.tavern_tier,
        }
    }

    /// The team for its combat: both players' warbands, and its first fighter (§6.1).
    pub fn team(&self) -> DuosTeam {
        DuosTeam {
            warbands: [self.warband(0), self.warband(1)],
            first: self.first_fighter,
        }
    }

    /// Sync both players' auras before the team's combat, as Solo does.
    fn prepare_combat(&mut self) {
        self.mirror();
        for state in &mut self.players {
            state.sync_all_auras();
        }
    }

    /// This team (side A) fights a fixed `opponent` team (side B) (§6.1); only this team takes
    /// its results and the damage (§6.2).
    pub fn resolve_combat_against(&mut self, opponent: &DuosTeam, seed: u64) -> DuosResult {
        self.prepare_combat();
        let mut team_a = self.team();
        let mut team_b = opponent.clone();
        number_units(&mut team_a, &mut team_b);
        let res = resolve_duos_battle(&team_a, &team_b, seed);
        self.apply_results(&team_a, &res.team_a, &res, Side::A, seed);
        res
    }

    /// Team `a` (side A) fights team `b` (side B) (§6.1); both take their results and the
    /// losing team the damage (§6.2).
    pub fn resolve_combat_pair(a: &mut DuosTavern, b: &mut DuosTavern, seed: u64) -> DuosResult {
        a.prepare_combat();
        b.prepare_combat();
        let mut team_a = a.team();
        let mut team_b = b.team();
        number_units(&mut team_a, &mut team_b);
        let res = resolve_duos_battle(&team_a, &team_b, seed);
        a.apply_results(&team_a, &res.team_a, &res, Side::A, seed);
        b.apply_results(&team_b, &res.team_b, &res, Side::B, seed);
        res
    }

    /// Bring each player's result back to their own Tavern (§6.2), then the damage if the team
    /// (on `side`) lost. `team` is the team as it fought, with its units' combat ids.
    fn apply_results(
        &mut self,
        team: &DuosTeam,
        results: &[WarbandResult; 2],
        res: &DuosResult,
        side: Side,
        seed: u64,
    ) {
        let result = match (res.outcome, side) {
            (BattleOutcome::Draw, _) => CombatResult::Tied,
            (BattleOutcome::AWin, Side::A) | (BattleOutcome::BWin, Side::B) => CombatResult::Won,
            _ => CombatResult::Lost,
        };
        let (salts, gift_salt) = match side {
            Side::A => (READY_SALTS[0], GIFT_SALTS[0]),
            Side::B => (READY_SALTS[1], GIFT_SALTS[1]),
        };
        for (player, warband_result) in results.iter().enumerate() {
            // A warband that sat out comes back unchanged: its minions keep nothing from a
            // combat they didn't fight (§6.3).
            let (survivors, dead_units): (&[Unit], &[Unit]) = if warband_result.fought {
                (&warband_result.survivors, &warband_result.dead_units)
            } else {
                (&[], &[])
            };
            let mut ready_rng = Rng::new(seed ^ salts[player]);
            self.players[player].apply_combat_return(
                &team.warbands[player].board,
                warband_result.auras.clone(),
                warband_result.hand.clone(),
                survivors,
                dead_units,
                result,
                &mut ready_rng,
            );
            self.sat_out[player] = !warband_result.fought;
        }
        self.settle();
        // The gifts the players' cards gave in combat that still wait reach the teammate's
        // Tavern (§7.2): those that need it, the team's, the Passes and the cards a full combat
        // hand didn't take. The others reached the teammate in the combat (`docs/duos.md` §2).
        let mut gift_rng = Rng::new(seed ^ gift_salt);
        self.deliver_gifts(0, None, &mut gift_rng);
        self.deliver();
        if result == CombatResult::Lost {
            self.take_combat_damage(res.hero_damage);
        }
        for state in &mut self.players {
            state.sync_all_auras();
        }
    }

    /// The team lost a combat: its pool takes `damage`, Armor first (§3.5). As in Solo
    /// (`docs/tavern.md` §7.2), combat damage triggers no hero-damage minion, for either player:
    /// `Soul Rewinder` doesn't rewind it, `Ashen Corruptor` and `Tichondrius` don't trigger,
    /// `Eredar Escapist` doesn't count it.
    fn take_combat_damage(&mut self, damage: u32) {
        let dmg = damage as i32;
        let absorbed = dmg.min(self.armor.max(0));
        self.armor -= absorbed;
        self.health -= dmg - absorbed;
        self.mirror();
    }
}

/// Give the warbands' units the combat ids `resolve_duos_battle` gives them (`docs/duos.md` §3.1
/// step 1: the first warbands, team A's then team B's, then the second warbands), so that each
/// player's results can be matched to their board.
fn number_units(team_a: &mut DuosTeam, team_b: &mut DuosTeam) {
    let mut next_id = 0;
    for nth in 0..2 {
        for team in [&mut *team_a, &mut *team_b] {
            let first = team.first.min(1);
            let player = if nth == 0 { first } else { 1 - first };
            for unit in &mut team.warbands[player].board {
                unit.id = next_id;
                next_id += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(attack: i32, health: i32) -> Unit {
        Unit::new(format!("{attack}/{health}"), attack, health)
    }

    #[test]
    fn the_team_starts_with_30_health_and_both_heroes_armor() {
        let team = DuosTavern::new().with_armor([4, 2]);
        assert_eq!((team.health, team.armor), (30, 6));
        for state in &team.players {
            assert_eq!((state.health, state.armor), (30, 6));
        }
    }

    #[test]
    fn a_pair_combat_damages_only_the_losing_team_and_both_take_their_results() {
        let mut a = DuosTavern::new();
        let mut b = DuosTavern::new().with_armor([1, 0]);
        a.players[0].board = vec![unit(10, 10)];
        a.players[1].board = vec![unit(1, 1)];
        b.players[0].board = vec![unit(1, 1)];
        b.players[1].board = vec![unit(1, 1)];
        let res = DuosTavern::resolve_combat_pair(&mut a, &mut b, 1);
        assert_eq!(res.outcome, BattleOutcome::AWin);
        // A0's Tier 1, its survivor's Tier 1, and its teammate's (who never fought) Tier 1.
        assert_eq!(res.hero_damage, 3);
        assert_eq!((a.health, a.armor), (30, 0));
        assert_eq!((b.health, b.armor), (28, 0));
        for state in &a.players {
            assert!(state.last_combat_won);
        }
        for state in &b.players {
            assert!(state.last_combat_lost);
            assert_eq!((state.health, state.armor), (28, 0));
        }
        // A0 beat both of B's warbands alone: A1's never fought.
        assert_eq!(a.sat_out, [false, true]);
        assert_eq!(b.sat_out, [false, false]);
    }

    fn empty_pool() -> CardPool {
        CardPool::new(Vec::new())
    }

    #[test]
    fn a_gift_card_arrives_like_a_passed_card_and_waits_if_the_hand_is_full() {
        let mut team = DuosTavern::new();
        let (mut pool, mut rng) = (empty_pool(), Rng::new(1));
        team.players[1].hand = (0..9).map(|_| unit(1, 1)).collect();
        team.players[0]
            .auras
            .give_teammate(TeammateGift::Card(unit(2, 2)));
        team.players[0]
            .auras
            .give_teammate(TeammateGift::Card(unit(3, 3)));
        team.step(0, TavernAction::ToggleFreeze, &mut pool, &mut rng)
            .unwrap();
        assert!(team.players[0].auras.to_teammate.is_empty());
        assert_eq!(team.players[1].hand.len(), 10);
        assert_eq!(team.players[1].hand[9], unit(2, 2));
        assert_eq!(team.incoming[1], vec![unit(3, 3)]);
        // A gift is not a Pass.
        assert_eq!(team.passes_this_game, 0);
    }

    #[test]
    fn a_pass_by_a_card_effect_is_free_and_counts_as_a_pass() {
        let mut team = DuosTavern::new();
        let (mut pool, mut rng) = (empty_pool(), Rng::new(1));
        team.players[1].gold = 2;
        team.players[1]
            .auras
            .give_teammate(TeammateGift::Pass(unit(2, 2)));
        team.step(1, TavernAction::ToggleFreeze, &mut pool, &mut rng)
            .unwrap();
        assert_eq!(team.players[1].gold, 2);
        assert_eq!(team.players[0].hand, vec![unit(2, 2)]);
        assert_eq!((team.passes_this_turn, team.passes_this_game), (1, 1));
        for state in &team.players {
            assert_eq!(state.auras.team_passes_this_game, 1);
        }
    }

    #[test]
    fn a_duos_players_random_minions_come_from_the_duos_catalog() {
        let team = DuosTavern::new();
        let ids = |templates: &[crate::cards::CardTemplate]| -> Vec<u32> {
            templates.iter().map(|t| t.card_id).collect()
        };
        let duos = ids(cards::game_templates(&team.players[1].auras));
        let solo = ids(cards::game_templates(&crate::model::PlayerAuras::default()));
        assert_eq!(solo, ids(cards::all_templates()));
        // The Solo catalog, then the Duos pool's exclusive minions.
        assert_eq!(duos[..solo.len()], solo[..]);
        assert_eq!(
            duos[solo.len()..],
            ids(cards::duos_exclusive_templates())[..]
        );
        assert!(duos.contains(&crate::cards::minions::wanderer_cho::ID));
    }

    #[test]
    fn card_counters_are_mirrored_into_the_teammates_auras() {
        let mut team = DuosTavern::new();
        team.players[0].auras.add_counter(7, 2);
        team.players[1].auras.add_counter(7, 3);
        team.mirror();
        assert_eq!(team.players[0].auras.team_counter(7), 5);
        assert_eq!(team.players[1].auras.team_counter(7), 5);
        assert_eq!(team.players[1].auras.counter(7), 3);
    }

    #[test]
    fn a_gift_card_reaches_the_teammate_in_the_combat_and_a_pass_waits_for_a_pool() {
        let mut team = DuosTavern::new();
        team.players[0].board = vec![unit(10, 10)];
        team.players[0]
            .auras
            .give_teammate(TeammateGift::Card(unit(2, 2)));
        team.players[0]
            .auras
            .give_teammate(TeammateGift::Pass(unit(3, 3)));
        let opponent = DuosTeam::default();
        let res = team.resolve_combat_against(&opponent, 1);
        // Player 1's warband never fought: the card comes back with its hand, once.
        assert_eq!(res.team_a[1].hand, vec![unit(2, 2)]);
        assert_eq!(team.players[1].hand, vec![unit(2, 2)]);
        assert!(team.incoming[1].is_empty());
        assert_eq!(
            team.players[0].auras.to_teammate,
            vec![TeammateGift::Pass(unit(3, 3))]
        );
        let (mut pool, mut rng) = (empty_pool(), Rng::new(1));
        team.start_turn(&mut pool, &mut rng);
        assert!(team.players[0].auras.to_teammate.is_empty());
        assert_eq!(team.players[1].hand, vec![unit(2, 2), unit(3, 3)]);
        assert_eq!(team.passes_this_game, 1);
    }

    #[test]
    fn a_players_health_and_armor_changes_survive_the_passes_and_gifts_after_them() {
        // No Gold-spent, `after_pass` or `after_team_pass` observer changes a hero's Health or
        // Armor yet: each case changes the players' Taverns itself, as such an observer would,
        // before the team's pool took the change (`settle`).
        let cases = [
            // A Pass sent after the Gold-spent observers of its Gold ran.
            ("a Pass", None),
            // A gift delivered after them (an `Effect` of a card with no `teammate_gift` hook,
            // which does nothing), or before the last delivery round ends (a gift card).
            (
                "an Effect gift",
                Some(TeammateGift::Effect {
                    card_id: 0,
                    amount: 0,
                    team: false,
                }),
            ),
            ("a gift card", Some(TeammateGift::Card(unit(2, 2)))),
        ];
        for (case, gift) in cases {
            let mut team = DuosTavern::new().with_armor([2, 0]);
            let (mut pool, mut rng) = (empty_pool(), Rng::new(1));
            team.players[0].health -= 3;
            team.players[1].armor -= 1;
            match gift {
                None => team.send_pass(0, unit(1, 1), &mut pool, &mut rng),
                Some(gift) => {
                    team.players[0].auras.give_teammate(gift);
                    team.deliver_gifts(0, Some(&mut pool), &mut rng);
                }
            }
            assert_eq!((team.health, team.armor), (27, 1), "{case}");
            for state in &team.players {
                assert_eq!((state.health, state.armor), (27, 1), "{case}");
            }
        }
    }
}
