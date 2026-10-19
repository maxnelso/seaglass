//! Re-simulating a replay into every intermediate state (`docs/replays.md`).

use std::panic::{self, AssertUnwindSafe};
use std::path::Path;

use crate::combat::BattleResult;
use crate::game::Match;
use crate::model::CardId;
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernAction, TavernState};

use super::{action_from_pb, engine_version, pb};
use pb::record::Kind;

/// The engine call that produced a [`Frame`].
#[derive(Clone, Debug, PartialEq)]
pub enum FrameKind {
    /// The match before any call.
    Start,
    /// `player` started their turn `turn`.
    StartTurn { player: usize, turn: u32 },
    /// `player` took `action`, which used card `card_id` ([`super::action_card`]).
    Act {
        player: usize,
        action: TavernAction,
        card_id: Option<CardId>,
        note: Option<pb::Note>,
    },
    /// Players `a` and `b` fought.
    Combat {
        a: usize,
        b: usize,
        seed: u64,
        result: Box<BattleResult>,
    },
    /// The lobby got the coming turn's pairings, and every player their view ([`Match::set_pairs`]).
    SetPairs { pairs: Vec<(usize, usize)> },
    /// The lobby delivered every player's lobby gifts ([`Match::deliver_gifts`]).
    DeliverGifts,
    /// The lobby recorded who is still alive after the combats ([`Match::record_deaths`]).
    RecordDeaths { alive: Vec<bool> },
    /// The match ended.
    End { winner: Option<usize> },
}

/// The match after one engine call.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub kind: FrameKind,
    /// The replay record the call re-simulates (`None` for [`FrameKind::Start`]).
    pub record: Option<usize>,
    /// The highest turn any player has reached.
    pub turn: u32,
    /// Every player's state after the call.
    pub players: Vec<TavernState>,
    /// The state of every RNG after the call ([`Match::rngs`], [`Rng::state`]).
    pub rng_states: Vec<u64>,
    /// The copies left of each card in every pool after the call ([`Match::pools`],
    /// [`CardPool::remaining`]), in [`Timeline::card_ids`] order.
    pub pool_remaining: Vec<Vec<u32>>,
}

/// A re-simulated call whose result differs from the one recorded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    pub record: usize,
    pub frame: usize,
    pub message: String,
}

/// A recorded call that could not be re-simulated. The timeline stops before it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayError {
    pub record: usize,
    pub message: String,
}

/// One action of a [`Timeline`], with the states around it.
#[derive(Clone, Copy, Debug)]
pub struct Decision<'a> {
    /// The index of the frame the action produced.
    pub frame: usize,
    pub player: usize,
    pub action: TavernAction,
    pub card_id: Option<CardId>,
    pub note: Option<&'a pb::Note>,
    /// Every player's state before the action.
    pub before: &'a [TavernState],
    /// Every player's state after the action.
    pub after: &'a [TavernState],
}

/// A replay re-simulated into the state after every call (`docs/replays.md`).
#[derive(Clone, Debug)]
pub struct Timeline {
    pub header: pb::Header,
    /// The match before any call, then after each re-simulated call.
    pub frames: Vec<Frame>,
    /// Calls whose re-simulated result differs from the recorded one.
    pub divergences: Vec<Divergence>,
    /// The call the re-simulation stopped at, if it could not re-simulate every call.
    pub error: Option<ReplayError>,
    /// The card of each [`Frame::pool_remaining`] entry.
    pub card_ids: Vec<CardId>,
}

impl Timeline {
    /// Load and re-simulate a replay file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, String> {
        Self::new(&super::load(path)?)
    }

    /// Re-simulate a replay. Fails only if the header is unusable; a call that cannot be
    /// re-simulated ends the timeline early and is reported in [`Self::error`].
    pub fn new(replay: &pb::Replay) -> Result<Self, String> {
        let header = replay.header.clone().ok_or("the replay has no header")?;
        let mut game = Match::from_header(&header)?;
        let mut timeline = Self {
            header,
            frames: Vec::new(),
            divergences: Vec::new(),
            error: None,
            card_ids: game.pools()[0].card_ids(),
        };
        timeline.capture(&game, FrameKind::Start, None);
        for (i, record) in replay.records.iter().enumerate() {
            let kind = match panic::catch_unwind(AssertUnwindSafe(|| rerun(&mut game, record))) {
                Ok(Ok(kind)) => kind,
                Ok(Err(message)) => {
                    timeline.error = Some(ReplayError { record: i, message });
                    break;
                }
                Err(payload) => {
                    let message = format!("the engine panicked: {}", panic_message(&payload));
                    timeline.error = Some(ReplayError { record: i, message });
                    break;
                }
            };
            let produced = game.records().last().expect("every call records itself");
            let frame = timeline.frames.len();
            for message in compare(record, produced) {
                timeline.divergences.push(Divergence {
                    record: i,
                    frame,
                    message,
                });
            }
            timeline.capture(&game, kind, Some(i));
        }
        Ok(timeline)
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub fn num_players(&self) -> usize {
        self.header.players.len()
    }

    /// Whether every call re-simulated and matched its recording.
    pub fn is_faithful(&self) -> bool {
        self.divergences.is_empty() && self.error.is_none()
    }

    /// The engine that recorded the replay, if it is not this engine build.
    pub fn engine_mismatch(&self) -> Option<&str> {
        (self.header.engine != engine_version()).then_some(self.header.engine.as_str())
    }

    /// The highest turn reached.
    pub fn last_turn(&self) -> u32 {
        self.frames.last().map_or(0, |f| f.turn)
    }

    /// The index of the first frame on turn `turn` or later.
    pub fn turn_start(&self, turn: u32) -> Option<usize> {
        self.frames.iter().position(|f| f.turn >= turn)
    }

    /// Every action in order, with the states around it.
    pub fn decisions(&self) -> impl Iterator<Item = Decision<'_>> + '_ {
        self.frames
            .iter()
            .enumerate()
            .skip(1)
            .filter_map(move |(i, frame)| match &frame.kind {
                FrameKind::Act {
                    player,
                    action,
                    card_id,
                    note,
                } => Some(Decision {
                    frame: i,
                    player: *player,
                    action: *action,
                    card_id: *card_id,
                    note: note.as_ref(),
                    before: &self.frames[i - 1].players,
                    after: &frame.players,
                }),
                _ => None,
            })
    }

    fn capture(&mut self, game: &Match, kind: FrameKind, record: Option<usize>) {
        self.frames.push(Frame {
            kind,
            record,
            turn: game.players().iter().map(|p| p.turn).max().unwrap_or(0),
            players: game.players().to_vec(),
            rng_states: game.rngs().iter().map(Rng::state).collect(),
            pool_remaining: game.pools().iter().map(CardPool::remaining).collect(),
        });
    }
}

/// Re-run one recorded call on `game`.
fn rerun(game: &mut Match, record: &pb::Record) -> Result<FrameKind, String> {
    let players = game.num_players();
    let player = |p: u32| {
        let p = p as usize;
        if p < players {
            Ok(p)
        } else {
            Err(format!(
                "player {p} does not exist (the match has {players} players)"
            ))
        }
    };
    match record.kind.as_ref().ok_or("the record is empty")? {
        Kind::StartTurn(start) => {
            let p = player(start.player)?;
            game.start_turn(p);
            Ok(FrameKind::StartTurn {
                player: p,
                turn: game.player(p).turn,
            })
        }
        Kind::Act(act) => {
            let p = player(act.player)?;
            let action = action_from_pb(act.action.as_ref().ok_or("the act has no action")?)?;
            let card_id = super::action_card(game.player(p), &action);
            game.step_with_note(p, action, act.note.clone())?;
            Ok(FrameKind::Act {
                player: p,
                action,
                card_id,
                note: act.note.clone(),
            })
        }
        Kind::Combat(combat) => {
            let (a, b) = (player(combat.a)?, player(combat.b)?);
            if a == b {
                return Err(format!("player {a} cannot fight themselves"));
            }
            let result = if combat.external_seed {
                game.combat_with_seed(a, b, combat.seed)
            } else {
                game.combat(a, b)
            };
            let seed = match game.records().last().and_then(|r| r.kind.as_ref()) {
                Some(Kind::Combat(produced)) => produced.seed,
                _ => unreachable!("a combat records itself"),
            };
            Ok(FrameKind::Combat {
                a,
                b,
                seed,
                result: Box::new(result),
            })
        }
        Kind::SetPairs(set) => {
            let pairs = set
                .pairs
                .iter()
                .map(|pair| {
                    let (a, b) = (player(pair.a)?, player(pair.b)?);
                    if a == b {
                        return Err(format!("player {a} cannot be paired with themselves"));
                    }
                    Ok((a, b))
                })
                .collect::<Result<Vec<_>, String>>()?;
            game.set_pairs(&pairs);
            Ok(FrameKind::SetPairs { pairs })
        }
        Kind::DeliverGifts(_) => {
            game.deliver_gifts();
            Ok(FrameKind::DeliverGifts)
        }
        Kind::RecordDeaths(_) => Ok(FrameKind::RecordDeaths {
            alive: game.record_deaths(),
        }),
        Kind::End(_) => Ok(FrameKind::End {
            winner: game.finish(),
        }),
    }
}

/// How a re-simulated call's record differs from the recorded one (notes are not compared).
fn compare(recorded: &pb::Record, produced: &pb::Record) -> Vec<String> {
    let mut diffs = Vec::new();
    let mut check = |what: &str, recorded: String, produced: String| {
        if recorded != produced {
            diffs.push(format!(
                "{what}: recorded {recorded}, re-simulated {produced}"
            ));
        }
    };
    match (&recorded.kind, &produced.kind) {
        (Some(Kind::StartTurn(r)), Some(Kind::StartTurn(p))) => {
            check("turn", r.turn.to_string(), p.turn.to_string());
        }
        (Some(Kind::Act(r)), Some(Kind::Act(p))) => {
            check("card", card(r.card_id), card(p.card_id));
        }
        (Some(Kind::Combat(r)), Some(Kind::Combat(p))) => {
            check("combat seed", r.seed.to_string(), p.seed.to_string());
            // An unset outcome means the recorded combat never finished.
            if r.outcome() != pb::Outcome::Unspecified {
                check(
                    "outcome",
                    r.outcome().as_str_name().to_string(),
                    p.outcome().as_str_name().to_string(),
                );
                check(
                    "hero damage",
                    r.hero_damage.to_string(),
                    p.hero_damage.to_string(),
                );
                check(
                    &format!("player {} after combat", r.a),
                    checkpoint(&r.check_a),
                    checkpoint(&p.check_a),
                );
                check(
                    &format!("player {} after combat", r.b),
                    checkpoint(&r.check_b),
                    checkpoint(&p.check_b),
                );
            }
        }
        (Some(Kind::SetPairs(_)), Some(Kind::SetPairs(_)))
        | (Some(Kind::DeliverGifts(_)), Some(Kind::DeliverGifts(_))) => {}
        (Some(Kind::RecordDeaths(r)), Some(Kind::RecordDeaths(p))) => {
            check("alive", format!("{:?}", r.alive), format!("{:?}", p.alive));
        }
        (Some(Kind::End(r)), Some(Kind::End(p))) => {
            check(
                "winner",
                format!("{:?}", r.winner),
                format!("{:?}", p.winner),
            );
            check("final turn", r.turn.to_string(), p.turn.to_string());
            check(
                "final health",
                format!("{:?}", r.health),
                format!("{:?}", p.health),
            );
        }
        _ => unreachable!("a re-simulated call records the same kind of call"),
    }
    diffs
}

fn card(card_id: Option<u32>) -> String {
    card_id.map_or("none".to_string(), |id| format!("#{id}"))
}

fn checkpoint(check: &Option<pb::Checkpoint>) -> String {
    match check {
        Some(c) => format!(
            "{} health, {} armor, tier {}, board+hand {:016x}",
            c.health, c.armor, c.tier, c.fingerprint
        ),
        None => "nothing".to_string(),
    }
}

fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}
