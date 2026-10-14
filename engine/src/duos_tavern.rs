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
use crate::model::{BattleOutcome, CombatResult, EffectDuration, PlayerEffect, Side, Unit};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernAction, TavernState};

/// The team's starting Health (`docs/duos_tavern.md` §3).
pub const TEAM_HEALTH: i32 = 30;

/// The default chance that the first fighter switches each turn (`docs/duos_tavern.md` §4.3).
pub const DEFAULT_FIRST_FIGHTER_SWITCH_CHANCE: f64 = 0.75;

/// A player's hand limit (`docs/tavern.md` §3.2).
const MAX_HAND_SIZE: usize = 10;

/// Whether the team's combat damage counts for the players' "after your hero takes damage"
/// minions (`Eredar Escapist`): not yet, pending the user's answer for Solo
/// (`docs/duos_tavern.md` §3.5).
const COMBAT_DAMAGE_TRIGGERS_AFTER_HERO_DAMAGE: bool = false;

/// Salts of the RNGs that resolve each player's hand cards that became ready in combat
/// (`docs/duos_tavern.md` §6.2), by team (side A, side B) and player.
const READY_SALTS: [[u64; 2]; 2] = [
    [0xD1B5_4A32_D192_ED03, 0xAEF1_7502_108E_F2D9],
    [0x8CB9_2BA7_2F3D_8DD7, 0xDB4F_0B91_75AE_2165],
];

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
    /// (player 0's first), deliver the queues, then draw the first fighter.
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
        if state.discover_pending.is_some() {
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
    /// `Ok(true)` if it ended their turn, or `Err` if it is illegal. Afterwards the team's pool
    /// takes the Health and Armor the action changed, and the queues deliver.
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
        self.settle();
        self.deliver();
        Ok(ended)
    }

    /// `player` Passes `hand[hand_index]` to their teammate (§5.6).
    fn pass(&mut self, player: usize, hand_index: usize, pool: &mut CardPool, rng: &mut Rng) {
        let mate = Self::teammate(player);
        let passer = &mut self.players[player];
        let mut card = passer.hand.remove(hand_index);
        // A free Pass first (a charge of the left-most FREE_PASS minion), else 1 Gold spent.
        if !cards::use_charge(&mut passer.board, CardFlags::FREE_PASS) {
            passer.spend_gold(1, pool, rng);
        }
        cards::on_passed(&mut card);
        // A card of a group leaves its group: a group of its own in the receiver's hand.
        if card.willbreaker_group != 0 {
            let receiver = &mut self.players[mate];
            card.willbreaker_group = receiver.next_willbreaker_group;
            receiver.next_willbreaker_group += 1;
        }
        self.passes_this_turn += 1;
        self.passes_this_game += 1;
        self.mirror();
        cards::after_pass(&mut self.players[player], &card, pool, rng);
        cards::after_team_pass(&mut self.players[player], pool, rng);
        cards::after_team_pass(&mut self.players[mate], pool, rng);
        self.incoming[mate].push(card);
    }

    /// Move queued passed cards into their receivers' hands while they have room (§5.4): player
    /// 0's queue, then player 1's. Each card goes through the receiver's `add_to_hand` (their
    /// auras, observers and triple check).
    fn deliver(&mut self) {
        for player in 0..2 {
            while self.players[player].hand.len() < MAX_HAND_SIZE
                && !self.incoming[player].is_empty()
            {
                let card = self.incoming[player].remove(0);
                self.players[player].add_to_hand(card);
            }
        }
    }

    /// Copy the team's Health, Armor and counters into both players (§3.3, §7), re-syncing the
    /// auras of a player whose values changed.
    pub fn mirror(&mut self) {
        let (health, armor) = (self.health, self.armor);
        let (this_turn, this_game) = (self.passes_this_turn, self.passes_this_game);
        for state in &mut self.players {
            let same = state.health == health
                && state.armor == armor
                && state.auras.team_passes_this_turn == this_turn
                && state.auras.team_passes_this_game == this_game;
            if !same {
                state.health = health;
                state.armor = armor;
                state.auras.team_passes_this_turn = this_turn;
                state.auras.team_passes_this_game = this_game;
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
        let salts = match side {
            Side::A => READY_SALTS[0],
            Side::B => READY_SALTS[1],
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
        if result == CombatResult::Lost {
            self.take_combat_damage(res.hero_damage);
        }
        for state in &mut self.players {
            state.sync_all_auras();
        }
    }

    /// The team lost a combat: its pool takes `damage`, Armor first (§3.5). No "your hero takes
    /// damage" minion reacts to combat damage (`Soul Rewinder` doesn't rewind it, `Ashen
    /// Corruptor` and `Tichondrius` don't trigger), and whether the "after your hero takes
    /// damage" ones count it (`Eredar Escapist`) is decided here, once:
    /// [`COMBAT_DAMAGE_TRIGGERS_AFTER_HERO_DAMAGE`].
    fn take_combat_damage(&mut self, damage: u32) {
        let dmg = damage as i32;
        let absorbed = dmg.min(self.armor.max(0));
        self.armor -= absorbed;
        let net_dmg = dmg - absorbed;
        self.health -= net_dmg;
        self.mirror();
        if COMBAT_DAMAGE_TRIGGERS_AFTER_HERO_DAMAGE && net_dmg > 0 {
            for state in &mut self.players {
                cards::after_hero_damage(state, net_dmg);
            }
            self.settle();
        }
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
}
