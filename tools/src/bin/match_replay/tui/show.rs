//! What the replay player shows, whatever it comes from (`match_replay tui`).
//!
//! A [`Source`] is a list of frames: each has a turn, a kind, a title, a Tavern view for each
//! player it can show, and maybe a combat to play back. [`ReplaySource`] reads a seaglass replay
//! (a re-simulated [`Timeline`]); `powerlog::PowerLogSource` reads a game from a Hearthstone
//! `Power.log`. The screen only draws what a source gives it.

use seaglass::replay::FrameKind;
use seaglass::{heroes, trinkets, TavernState, Timeline, Unit};
use seaglass_tools::replay_text::{frame_title, player_name};

use super::combat::CombatPlayback;

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
    let owned: Vec<&str> = s
        .auras
        .trinkets
        .iter()
        .filter_map(|t| trinkets::data(t.card_id).map(|d| d.name))
        .collect();
    if !owned.is_empty() {
        parts.push(format!("Trinkets: {}", owned.join(", ")));
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
            FrameKind::StartTurn { player, .. } => (Kind::TurnStart, Some(*player)),
            FrameKind::Act { player, .. } => (Kind::Act, Some(*player)),
            FrameKind::Combat { .. } => (Kind::Combat, None),
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
                    let name = trinkets::data(o.card_id).map_or("?", |t| t.name);
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
        let FrameKind::Combat { a, b, seed, result } = &self.timeline.frames[frame].kind else {
            return None;
        };
        let before = &self.timeline.frames[frame - 1];
        let after = &self.timeline.frames[frame];
        Some(CombatPlayback::from_replay(
            [*a, *b],
            [&before.players[*a], &before.players[*b]],
            [&after.players[*a], &after.players[*b]],
            *seed,
            result,
            [&self.names[*a], &self.names[*b]],
        ))
    }
}
