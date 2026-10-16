//! The lobby (`docs/tavern.md` §3.1): what a player's Tavern knows about the other players, built
//! the same way for every caller (the fuzzers, `sim_match`, Python).
//!
//! A [`Lobby`] records, for each player of a game (Solo: up to 8; Duos: the players of every team,
//! teammates side by side), whether they are alive, whether they died in the last combats, their
//! last warband (their board as it entered their last combat), their last and next opponents, and
//! the featured pair with its last outcome. Each turn, before the players' turns start, the caller
//! records the next pairings and [`Lobby::apply`]s it: every player's Tavern gets their own
//! [`LobbyView`]. After a player's turn starts, [`Lobby::deliver_gifts`] hands the cards their
//! Hero Power made for the others (`TavernState::lobby_gifts`: King Mukla's Bananas) to the other
//! living players. In Duos a player's teammate is not an opponent: their teammate's gifts reach them
//! through their team's Tavern (`docs/duos_tavern.md` §7.2), not through the lobby.

use crate::model::{BattleOutcome, LobbyView, OpponentView, Unit};
use crate::tavern::TavernState;

/// The players of a game, as the lobby sees them (`docs/tavern.md` §3.1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Lobby {
    /// A Duos game: players `2k` and `2k + 1` are teammates.
    pub duos: bool,
    /// Each player is still in the game.
    pub alive: Vec<bool>,
    /// Each player died in the last combats (a Duos team that is out: both its players).
    pub died_last_combat: Vec<bool>,
    /// Each player's board as it entered their last combat.
    pub last_warbands: Vec<Vec<Unit>>,
    /// Each player's last opponent (a player of the lobby), if they had one.
    pub last_opponent: Vec<Option<usize>>,
    /// Each player's next opponent, if they have one.
    pub next_opponent: Vec<Option<usize>>,
    /// The featured combat of the coming turn: two players of the lobby (`Lord Barov`'s wager).
    pub featured_pair: Option<(usize, usize)>,
    /// The outcome of the last turn's featured combat, for its first player.
    pub last_pair_outcome: Option<BattleOutcome>,
}

impl Lobby {
    /// A lobby of `players` players, all alive, before any combat.
    pub fn new(players: usize, duos: bool) -> Self {
        Self {
            duos,
            alive: vec![true; players],
            died_last_combat: vec![false; players],
            last_warbands: vec![Vec::new(); players],
            last_opponent: vec![None; players],
            next_opponent: vec![None; players],
            featured_pair: None,
            last_pair_outcome: None,
        }
    }

    /// The number of players.
    pub fn len(&self) -> usize {
        self.alive.len()
    }

    /// `true` if the lobby has no players.
    pub fn is_empty(&self) -> bool {
        self.alive.is_empty()
    }

    /// Player `p`'s teammate, in Duos.
    pub fn teammate(&self, p: usize) -> Option<usize> {
        (self.duos && (p ^ 1) < self.len()).then_some(p ^ 1)
    }

    /// Player `p`'s opponents: every other player but their Duos teammate, in lobby order. Their
    /// `LobbyView::opponents` follow this order.
    pub fn opponents(&self, p: usize) -> Vec<usize> {
        (0..self.len())
            .filter(|&q| q != p && Some(q) != self.teammate(p))
            .collect()
    }

    /// Player `p`'s view of the lobby: their opponents, and the indices into them of their last and
    /// next opponents and of the featured pair (none if `p` is in it: a player can't wager on
    /// their own combat).
    pub fn view(&self, p: usize) -> LobbyView {
        let opponents = self.opponents(p);
        let index = |q: Option<usize>| q.and_then(|q| opponents.iter().position(|&o| o == q));
        let featured_pair = self
            .featured_pair
            .and_then(|(a, b)| Some((index(Some(a))?, index(Some(b))?)));
        LobbyView {
            opponents: opponents
                .iter()
                .map(|&q| OpponentView {
                    alive: self.alive[q],
                    died_last_combat: self.died_last_combat[q],
                    last_warband: self.last_warbands[q].clone(),
                })
                .collect(),
            next_opponent: index(self.next_opponent.get(p).copied().flatten()),
            last_opponent: index(self.last_opponent.get(p).copied().flatten()),
            featured_pair,
            last_pair_outcome: self.last_pair_outcome,
        }
    }

    /// Record the coming turn's pairings (`pairs` of players; a player in none has no next
    /// opponent) and its featured pair: the first pair, if any.
    pub fn set_next_pairs(&mut self, pairs: &[(usize, usize)]) {
        self.next_opponent.iter_mut().for_each(|o| *o = None);
        for &(a, b) in pairs {
            self.next_opponent[a] = Some(b);
            self.next_opponent[b] = Some(a);
        }
        self.featured_pair = pairs.first().copied();
    }

    /// Record a player's combat: `opponent` (none for a ghost of no player) and their board as it
    /// entered it (at most 7 minions).
    pub fn record_combat(&mut self, p: usize, opponent: Option<usize>, board: &[Unit]) {
        self.last_opponent[p] = opponent;
        self.last_warbands[p] = board
            .iter()
            .take(crate::combat::MAX_BOARD_SIZE)
            .cloned()
            .collect();
    }

    /// Record the outcome of the combat of `a` (side A) against `b`: if it is the featured pair's
    /// combat, it is the outcome Lord Barov's wager reads next turn.
    pub fn record_outcome(&mut self, a: usize, b: usize, outcome: BattleOutcome) {
        match self.featured_pair {
            Some(pair) if pair == (a, b) => self.last_pair_outcome = Some(outcome),
            Some(pair) if pair == (b, a) => {
                self.last_pair_outcome = Some(match outcome {
                    BattleOutcome::AWin => BattleOutcome::BWin,
                    BattleOutcome::BWin => BattleOutcome::AWin,
                    BattleOutcome::Draw => BattleOutcome::Draw,
                })
            }
            _ => {}
        }
    }

    /// The combats are over: `alive` says who is still in the game; those who were alive and
    /// aren't died in them.
    pub fn record_deaths(&mut self, alive: &[bool]) {
        for (p, &now) in alive.iter().enumerate().take(self.len()) {
            self.died_last_combat[p] = self.alive[p] && !now;
            self.alive[p] = now;
        }
    }

    /// Give every player's Tavern their view (`players[p]` is player `p`).
    pub fn apply(&self, players: &mut [&mut TavernState]) {
        for (p, state) in players.iter_mut().enumerate() {
            state.set_lobby_view(self.view(p));
        }
    }

    /// Hand every player's lobby gifts (`TavernState::take_lobby_gifts`) to their living opponents,
    /// one card each, in the order of their view's living opponents, under the full-hand rule
    /// (`TavernState::add_to_hand`). A gift for no one is lost.
    pub fn deliver_gifts(&self, players: &mut [&mut TavernState]) {
        for giver in 0..players.len().min(self.len()) {
            let gifts = players[giver].take_lobby_gifts();
            if gifts.is_empty() {
                continue;
            }
            let recipients = self.opponents(giver).into_iter().filter(|&q| self.alive[q]);
            for (q, card) in recipients.zip(gifts) {
                if let Some(state) = players.get_mut(q) {
                    state.add_to_hand(card);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::tokens;

    fn minion(attack: i32) -> Unit {
        Unit::new("m", attack, 1)
    }

    #[test]
    fn a_view_indexes_the_players_opponents() {
        let mut lobby = Lobby::new(4, false);
        lobby.record_combat(2, Some(0), &[minion(5)]);
        lobby.set_next_pairs(&[(1, 2), (0, 3)]);
        let view = lobby.view(0);
        assert_eq!(view.opponents.len(), 3);
        assert_eq!(view.next_opponent, Some(2)); // player 3
        assert_eq!(view.featured_pair, Some((0, 1))); // players 1 and 2
        assert_eq!(view.opponents[1].last_warband, vec![minion(5)]);
        assert_eq!(lobby.view(1).featured_pair, None); // in the featured pair
        assert_eq!(lobby.view(2).last_opponent, Some(0));
    }

    #[test]
    fn a_duos_teammate_is_not_an_opponent() {
        let lobby = Lobby::new(4, true);
        assert_eq!(lobby.opponents(0), vec![2, 3]);
        assert_eq!(lobby.opponents(3), vec![0, 1]);
        assert_eq!(lobby.teammate(2), Some(3));
    }

    #[test]
    fn the_featured_outcome_follows_the_pairs_order_and_deaths_are_recorded() {
        let mut lobby = Lobby::new(3, false);
        lobby.set_next_pairs(&[(0, 2)]);
        lobby.record_outcome(2, 0, BattleOutcome::AWin);
        assert_eq!(lobby.last_pair_outcome, Some(BattleOutcome::BWin));
        lobby.record_deaths(&[true, false, true]);
        assert_eq!(lobby.died_last_combat, vec![false, true, false]);
        lobby.record_deaths(&[true, false, true]);
        assert_eq!(lobby.died_last_combat, vec![false, false, false]);
    }

    #[test]
    fn gifts_reach_the_living_opponents_under_the_full_hand_rule() {
        let mut lobby = Lobby::new(4, false);
        lobby.record_deaths(&[true, true, false, true]);
        let mut players: Vec<TavernState> = (0..4).map(|_| TavernState::new()).collect();
        players[0].lobby_gifts = vec![tokens::make_bananas(), tokens::make_big_banana()];
        players[3].hand = (0..10).map(minion).collect();
        let mut refs: Vec<&mut TavernState> = players.iter_mut().collect();
        lobby.deliver_gifts(&mut refs);
        assert!(players[0].lobby_gifts.is_empty());
        assert_eq!(players[1].hand.len(), 1);
        assert!(players[2].hand.is_empty()); // out of the game
        assert_eq!(players[3].hand.len(), 10); // full: lost
    }
}
