//! What the replay player shows, whatever it comes from (`match_replay tui`).
//!
//! A [`Source`] is a list of frames: each has a turn, a kind, a title, a Tavern view for each
//! player it can show, and maybe a combat to play back. [`ReplaySource`] reads a seaglass replay
//! (a re-simulated [`Timeline`]); `powerlog::PowerLogSource` reads a game from a Hearthstone
//! `Power.log`. The screen only draws what a source gives it.

use seaglass::combat::MAX_BOARD_SIZE;
use seaglass::replay::{Frame, FrameKind};
use seaglass::{
    heroes, trinkets, CombatResult, DuosTavern, DuosTeam, PlayerAuras, TavernState, Timeline, Unit,
    Warband,
};
use seaglass_tools::replay_text::{frame_title, player_name, unit_players};

use super::combat::{
    duos_ghost_odds, duos_odds, solo_ghost_odds, tavern_label, trinkets_clause, CombatPlayback,
    DuosSide, SoloSide,
};

/// What kind of moment a frame is (for colours, auto-play pacing and navigation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Start,
    TurnStart,
    Act,
    Combat,
    /// Bookkeeping between the players (pairings, gifts, deaths).
    Lobby,
    End,
}

/// A frame's turn, kind and acting player.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameInfo {
    pub turn: u32,
    pub kind: Kind,
    /// The player who acts or starts their turn in it.
    pub actor: Option<usize>,
}

/// A row of cards in a Tavern view.
#[derive(Clone, Debug, Default)]
pub struct Row {
    pub label: String,
    pub units: Vec<Unit>,
    /// The cards the frame's action brought into the row (highlighted).
    pub fresh: Vec<bool>,
    /// The slots drawn even when empty (a board's 7).
    pub slots: usize,
}

/// One player's Tavern at a frame.
#[derive(Clone, Debug, Default)]
pub struct TavernView {
    /// Health, Tier, Gold, hero, trinkets.
    pub hero_line: String,
    /// The other players, or other context.
    pub detail_line: String,
    pub rows: Vec<Row>,
    /// Offers that are not cards (trinkets, heroes).
    pub offers: Vec<String>,
}

/// Something the replay player can show.
pub trait Source {
    /// The number of frames.
    fn len(&self) -> usize;
    fn info(&self, frame: usize) -> FrameInfo;
    /// One line describing the frame.
    fn title(&self, frame: usize) -> String;
    /// The players whose Taverns can be shown.
    fn players(&self) -> &[String];
    /// Player `player`'s Tavern at `frame`.
    fn tavern(&self, frame: usize, player: usize) -> TavernView;
    /// The frame's combat, ready to play back.
    fn combat(&self, frame: usize) -> Option<CombatPlayback>;
    /// The players a frame concerns (its actor, or a combat's fighters, with their teammates in
    /// Duos), for focusing on some players. `None`: everyone (the start and the end); empty:
    /// nobody (lobby bookkeeping). By default every frame concerns everyone.
    fn involves(&self, _frame: usize) -> Option<Vec<usize>> {
        None
    }
    /// `player` and the players on their team (Duos), in order.
    fn team_of(&self, player: usize) -> Vec<usize> {
        vec![player]
    }
    /// The player a replay marks as its agent (an RL `agent` or `learner` controller), if any.
    fn agent(&self) -> Option<usize> {
        None
    }
}

/// The units of `now` that were not in `before` (by id and card).
pub fn fresh(now: &[Unit], before: Option<&[Unit]>) -> Vec<bool> {
    match before {
        None => vec![false; now.len()],
        Some(before) => now
            .iter()
            .map(|u| {
                !before
                    .iter()
                    .any(|b| b.id == u.id && b.card_id == u.card_id)
            })
            .collect(),
    }
}

/// A seaglass replay, re-simulated.
pub struct ReplaySource {
    timeline: Timeline,
    names: Vec<String>,
}

impl ReplaySource {
    pub fn new(timeline: Timeline) -> Self {
        let names = (0..timeline.num_players())
            .map(|p| player_name(&timeline, p))
            .collect();
        Self { timeline, names }
    }
}

/// One line about a player: Health, Tier, Gold, hero and trinkets.
fn player_line(name: &str, s: &TavernState) -> String {
    let mut parts = vec![format!("{name} ♥{}", s.health)];
    if s.armor > 0 {
        parts[0].push_str(&format!("+{}", s.armor));
    }
    let upgrade = if s.tavern_tier < 6 {
        format!(" (up {}g)", s.upgrade_cost)
    } else {
        String::new()
    };
    parts.push(format!("Tier {}{upgrade}", s.tavern_tier));
    parts.push(format!("Gold {}/{}", s.gold, s.max_gold));
    if let Some((state, data)) = s
        .hero()
        .and_then(|h| heroes::data(h.hero_id).map(|d| (h, d)))
    {
        let used = if state.power.uses_left == 0 {
            ", used"
        } else {
            ""
        };
        parts.push(format!(
            "{}: {} ({}g{used})",
            data.name, data.power.name, state.power.cost
        ));
    }
    if let Some(owned) = trinkets_clause(s) {
        parts.push(owned);
    }
    if s.is_frozen {
        parts.push("❄ frozen".to_string());
    }
    parts.join(" · ")
}

impl Source for ReplaySource {
    fn len(&self) -> usize {
        self.timeline.len()
    }

    fn info(&self, frame: usize) -> FrameInfo {
        let f = &self.timeline.frames[frame];
        let (kind, actor) = match &f.kind {
            FrameKind::Start => (Kind::Start, None),
            FrameKind::StartTurn { player, .. } => (
                Kind::TurnStart,
                Some(unit_players(&self.timeline, *player)[0]),
            ),
            FrameKind::Act { player, .. } => (Kind::Act, Some(*player)),
            FrameKind::Combat { .. }
            | FrameKind::DuosCombat { .. }
            | FrameKind::GhostCombat { .. } => (Kind::Combat, None),
            FrameKind::SetPairs { .. }
            | FrameKind::DeliverGifts
            | FrameKind::RecordDeaths { .. } => (Kind::Lobby, None),
            FrameKind::End { .. } => (Kind::End, None),
        };
        FrameInfo {
            turn: f.turn,
            kind,
            actor,
        }
    }

    fn title(&self, frame: usize) -> String {
        frame_title(&self.timeline, frame)
    }

    fn players(&self) -> &[String] {
        &self.names
    }

    fn involves(&self, frame: usize) -> Option<Vec<usize>> {
        let unit = |u: usize| unit_players(&self.timeline, u);
        Some(match &self.timeline.frames[frame].kind {
            FrameKind::Start | FrameKind::End { .. } => return None,
            FrameKind::Act { player, .. } => vec![*player],
            FrameKind::StartTurn { player, .. } => unit(*player),
            FrameKind::Combat { a, b, .. } => vec![*a, *b],
            FrameKind::DuosCombat { a, b, .. } => [unit(*a), unit(*b)].concat(),
            FrameKind::GhostCombat { unit: u, .. } => unit(*u),
            FrameKind::SetPairs { .. }
            | FrameKind::DeliverGifts
            | FrameKind::RecordDeaths { .. } => Vec::new(),
        })
    }

    fn team_of(&self, player: usize) -> Vec<usize> {
        if self.timeline.header.duos {
            unit_players(&self.timeline, player / 2)
        } else {
            vec![player]
        }
    }

    fn agent(&self) -> Option<usize> {
        // `seaglass_rl` labels each unit's controller (`meta.controllers`, one per unit).
        let controllers = self.timeline.header.meta.get("controllers")?;
        let u = controllers
            .split(',')
            .position(|c| c == "agent" || c == "learner")?;
        Some(unit_players(&self.timeline, u)[0])
    }

    fn tavern(&self, frame: usize, p: usize) -> TavernView {
        let frames = &self.timeline.frames;
        let f = &frames[frame];
        let state = &f.players[p];
        // What an action (or the lobby's gifts) changed is highlighted.
        let before = match &f.kind {
            FrameKind::Act { player, .. } if *player == p => Some(&frames[frame - 1].players[p]),
            FrameKind::DeliverGifts => Some(&frames[frame - 1].players[p]),
            _ => None,
        };
        let others: Vec<String> = (0..self.names.len())
            .filter(|&q| q != p)
            .map(|q| {
                let s = &f.players[q];
                format!(
                    "{} ♥{} T{} board {}",
                    self.names[q],
                    s.health,
                    s.tavern_tier,
                    s.board.len()
                )
            })
            .collect();
        let shop = match &state.discover_pending {
            Some(options) => Row {
                label: "Discover".to_string(),
                units: options.clone(),
                fresh: vec![false; options.len()],
                slots: 0,
            },
            None => Row {
                label: if state.is_frozen {
                    "Shop ❄ frozen".to_string()
                } else {
                    "Shop".to_string()
                },
                fresh: fresh(&state.shop, before.map(|b| b.shop.as_slice())),
                units: state.shop.clone(),
                slots: 0,
            },
        };
        let board = Row {
            label: "Board".to_string(),
            fresh: fresh(&state.board, before.map(|b| b.board.as_slice())),
            units: state.board.clone(),
            slots: 7,
        };
        let hand = Row {
            label: format!("Hand {}/10", state.hand.len()),
            fresh: fresh(&state.hand, before.map(|b| b.hand.as_slice())),
            units: state.hand.clone(),
            slots: 0,
        };
        let mut offers = Vec::new();
        if !state.trinket_options.is_empty() {
            let names: Vec<String> = state
                .trinket_options
                .iter()
                .map(|o| {
                    let name =
                        trinkets::data(o.card_id).map_or_else(|| "?".to_string(), trinkets::display_name);
                    format!("{name} ({}g)", o.cost)
                })
                .collect();
            offers.push(format!("Trinket offer: {}", names.join(" · ")));
        }
        if !state.hero_options.is_empty() {
            let names: Vec<&str> = state
                .hero_options
                .iter()
                .filter_map(|&id| heroes::data(id).map(|h| h.name))
                .collect();
            offers.push(format!("Hero choice: {}", names.join(" · ")));
        }
        TavernView {
            hero_line: player_line(&self.names[p], state),
            detail_line: others.join(" · "),
            rows: vec![shop, board, hand],
            offers,
        }
    }

    fn combat(&self, frame: usize) -> Option<CombatPlayback> {
        let frames = &self.timeline.frames;
        let (before, after) = (frames.get(frame.checked_sub(1)?)?, &frames[frame]);
        match &after.kind {
            FrameKind::Combat { a, b, seed, result } => Some(CombatPlayback::from_replay(
                [*a, *b],
                [&before.players[*a], &before.players[*b]],
                [&after.players[*a], &after.players[*b]],
                *seed,
                result,
                [&self.names[*a], &self.names[*b]],
            )),
            FrameKind::DuosCombat { a, b, seed, result } => {
                let (pre_a, pre_b) = (team_tavern(before, *a), team_tavern(before, *b));
                let (side_a, side_b) = (self.team_side(frame, *a), self.team_side(frame, *b));
                let odds = duos_odds([&side_a.team, &side_b.team], &pre_a, &pre_b, *seed);
                let (mut team_a, mut team_b) = (pre_a, pre_b);
                Some(
                    CombatPlayback::from_duos(
                        || DuosTavern::resolve_combat_pair(&mut team_a, &mut team_b, *seed),
                        result,
                        before.players[2 * a].lobby.damage_cap,
                        [side_a, side_b],
                    )
                    .with_odds(odds),
                )
            }
            FrameKind::GhostCombat {
                unit,
                ghost,
                seed,
                result,
            } => Some(match &**result {
                CombatResult::Solo(recorded) => {
                    let mut state = before.players[*unit].clone();
                    let board = self.last_warband(frame, *ghost);
                    let tier = before.players[*ghost].tavern_tier;
                    let name = &self.names[*unit];
                    let ghost_name = format!("ghost of {}", self.names[*ghost]);
                    let odds = solo_ghost_odds([name, &ghost_name], &state, &board, tier, *seed);
                    let ghost_trinkets = trinkets_clause(&before.players[*ghost])
                        .map(|t| format!(" · {t}"))
                        .unwrap_or_default();
                    let ghost_label = format!("{ghost_name} · Tier {tier}{ghost_trinkets}");
                    let sides = [
                        SoloSide {
                            name: name.clone(),
                            label: tavern_label(name, &state, &after.players[*unit], false),
                            end_label: tavern_label(name, &state, &after.players[*unit], true),
                            players: vec![*unit],
                        },
                        SoloSide {
                            name: ghost_name,
                            label: ghost_label.clone(),
                            end_label: ghost_label,
                            players: Vec::new(),
                        },
                    ];
                    let damage_cap = state.lobby.damage_cap;
                    CombatPlayback::from_battle(
                        || {
                            state.resolve_combat_against(
                                &board,
                                tier,
                                &PlayerAuras::default(),
                                &[],
                                *seed,
                            )
                        },
                        recorded,
                        damage_cap,
                        sides,
                    )
                    .with_odds(odds)
                }
                CombatResult::Duos(recorded) => {
                    let ghosts = unit_players(&self.timeline, *ghost);
                    let warband = |g: usize| Warband {
                        board: self.last_warband(frame, g),
                        hand: Vec::new(),
                        auras: PlayerAuras::default(),
                        hero_tier: before.players[g].tavern_tier,
                    };
                    let opponent = DuosTeam {
                        warbands: [warband(ghosts[0]), warband(ghosts[1])],
                        first: 0,
                    };
                    let team = format!("ghost of team {}", ghost + 1);
                    let labels = [0, 1].map(|i| {
                        let g = ghosts[i];
                        let trinkets = trinkets_clause(&before.players[g])
                            .map(|t| format!(" · {t}"))
                            .unwrap_or_default();
                        format!(
                            "{team} / {} · Tier {}{trinkets}",
                            self.names[g], before.players[g].tavern_tier
                        )
                    });
                    let ghost_side = DuosSide {
                        names: [0, 1].map(|i| self.names[ghosts[i]].clone()),
                        end_labels: labels.clone(),
                        labels,
                        team,
                        players: Vec::new(),
                    };
                    let pre_a = team_tavern(before, *unit);
                    let side_a = self.team_side(frame, *unit);
                    let odds =
                        duos_ghost_odds([&side_a.team, &ghost_side.team], &pre_a, &opponent, *seed);
                    let mut tavern = pre_a;
                    CombatPlayback::from_duos(
                        || tavern.resolve_combat_against(&opponent, *seed),
                        recorded,
                        before.players[2 * unit].lobby.damage_cap,
                        [side_a, ghost_side],
                    )
                    .with_odds(odds)
                }
            }),
            _ => None,
        }
    }
}

impl ReplaySource {
    /// Duos team `u` in the combat at `frame`, as its playback shows it: each player's board
    /// labelled with their name, team, Tier and the team's Health (before, and at the end after,
    /// the combat).
    fn team_side(&self, frame: usize, u: usize) -> DuosSide {
        let (before, after) = (
            &self.timeline.frames[frame - 1],
            &self.timeline.frames[frame],
        );
        let players = unit_players(&self.timeline, u);
        let label = |i: usize, end: bool| {
            let p = players[i];
            let name = format!("{} · team {}", self.names[p], u + 1);
            tavern_label(&name, &before.players[p], &after.players[p], end)
        };
        DuosSide {
            team: format!("team {}", u + 1),
            names: [0, 1].map(|i| self.names[players[i]].clone()),
            labels: [0, 1].map(|i| label(i, false)),
            end_labels: [0, 1].map(|i| label(i, true)),
            players,
        }
    }

    /// The board player `p` entered their last combat before `frame` with (at most 7 minions):
    /// the board their ghost fights with ([`seaglass::lobby::Lobby::last_warbands`], which the
    /// lobby updates at every combat of `p`'s unit, but not when `p` is the ghost).
    fn last_warband(&self, frame: usize, p: usize) -> Vec<Unit> {
        let duos = self.timeline.header.duos;
        let unit = if duos { p / 2 } else { p };
        let frames = &self.timeline.frames;
        (1..frame)
            .rev()
            .find(|&k| match &frames[k].kind {
                FrameKind::Combat { a, b, .. } | FrameKind::DuosCombat { a, b, .. } => {
                    *a == unit || *b == unit
                }
                FrameKind::GhostCombat { unit: u, .. } => *u == unit,
                _ => false,
            })
            .map(|k| {
                frames[k - 1].players[p]
                    .board
                    .iter()
                    .take(MAX_BOARD_SIZE)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Duos team `u`'s Tavern at `frame` (its players' states and the team's own), to re-run its
/// combat from.
fn team_tavern(frame: &Frame, u: usize) -> DuosTavern {
    let team = &frame.teams[u];
    DuosTavern {
        players: [
            frame.players[2 * u].clone(),
            frame.players[2 * u + 1].clone(),
        ],
        health: team.health,
        armor: team.armor,
        passes_this_turn: team.passes_this_turn,
        passes_this_game: team.passes_this_game,
        incoming: team.incoming.clone(),
        ended: team.ended,
        first_fighter: team.first_fighter,
        ..DuosTavern::new()
    }
}
