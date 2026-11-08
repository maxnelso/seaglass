//! The replay player's state and navigation (`match_replay tui`).

use std::time::Duration;

use seaglass::Side;
use seaglass_tools::render::Glyphs;

use super::combat::{CombatPlayback, Phase, SampleKind};
use super::show::{Kind, Source};

/// The playback speeds `+` and `-` step through.
const SPEEDS: [f32; 7] = [0.25, 0.5, 1.0, 1.5, 2.0, 4.0, 8.0];

/// Currently shown simulated combat from [`CombatPlayback::odds`].
#[derive(Clone, Debug)]
pub struct ActiveSample {
    /// Side from whose perspective `kind` was chosen.
    pub side: Side,
    pub kind: SampleKind,
    pub index: usize,
    pub count: usize,
    pub playback: CombatPlayback,
}

impl ActiveSample {
    pub fn kind_for(&self, side: Side) -> SampleKind {
        if side == self.side {
            self.kind
        } else {
            self.kind.flipped()
        }
    }
}

/// The replay player: a [`Source`] and where the player is in it.
pub struct App {
    pub source: Box<dyn Source>,
    /// What the header names (the file, the game).
    pub file: String,
    /// The frame shown.
    pub frame: usize,
    /// The step shown, when the frame is a combat.
    pub step: usize,
    /// The current frame's combat, ready to play back.
    pub combat: Option<CombatPlayback>,
    /// Simulated Win / Tie / Loss combat currently viewed in place of the recorded combat.
    pub sample: Option<ActiveSample>,
    /// Show the combat's other version (the engine's) instead.
    pub show_alt: bool,
    /// The player whose Tavern is shown (when not following the actor).
    pub perspective: usize,
    /// Show the Tavern of whoever acts in the frame.
    pub follow: bool,
    /// Only show the frames that concern these players (one player, or a Duos team).
    pub focus: Option<Vec<usize>>,
    /// Auto-play is on.
    pub playing: bool,
    /// Index into [`SPEEDS`].
    speed: usize,
    /// The card selected for inspection (an index into the screen's cards), if inspecting.
    pub inspect: Option<usize>,
    pub help: bool,
    pub glyphs: Glyphs,
}

impl App {
    pub fn new(source: Box<dyn Source>, file: String, frame: usize, glyphs: Glyphs) -> Self {
        let mut app = Self {
            source,
            file,
            frame: 0,
            step: 0,
            combat: None,
            sample: None,
            show_alt: false,
            perspective: 0,
            follow: true,
            focus: None,
            playing: false,
            speed: 2,
            inspect: None,
            help: false,
            glyphs,
        };
        app.go_to(frame);
        app
    }

    pub fn len(&self) -> usize {
        self.source.len()
    }

    pub fn names(&self) -> &[String] {
        self.source.players()
    }

    pub fn speed(&self) -> f32 {
        SPEEDS[self.speed]
    }

    pub fn faster(&mut self) {
        self.speed = (self.speed + 1).min(SPEEDS.len() - 1);
    }

    pub fn slower(&mut self) {
        self.speed = self.speed.saturating_sub(1);
    }

    /// Which side of the current combat is drawn at the bottom of the screen.
    pub fn bottom_side(&self) -> Side {
        match &self.combat {
            Some(c) if c.players[1].contains(&self.shown_player()) => Side::B,
            _ => Side::A,
        }
    }

    /// The combat version shown: a simulated sample if selected, else the frame's or its alternate.
    pub fn shown_combat(&self) -> Option<&CombatPlayback> {
        if let Some(sample) = &self.sample {
            return Some(&sample.playback);
        }
        let combat = self.combat.as_ref()?;
        match (&combat.alt, self.show_alt) {
            (Some(alt), true) => Some(alt),
            _ => Some(combat),
        }
    }

    /// Show the `index`-th simulated combat for `kind` from the bottom side's perspective.
    pub fn select_sample(&mut self, kind: SampleKind, index: usize) -> bool {
        let side = self.bottom_side();
        let Some(combat) = &self.combat else {
            return false;
        };
        let Some((idx, count, playback)) = combat.sample_combat(side, kind, index) else {
            return false;
        };
        self.playing = false;
        self.inspect = None;
        self.step = 0;
        self.sample = Some(ActiveSample {
            side,
            kind,
            index: idx,
            count,
            playback,
        });
        true
    }

    /// Show a simulated `kind` combat (or advance to the next seed in that bucket if already shown).
    pub fn show_sample(&mut self, kind: SampleKind) -> bool {
        let side = self.bottom_side();
        let next = match &self.sample {
            Some(s) if s.kind_for(side) == kind => s.index + 1,
            _ => 0,
        };
        self.select_sample(kind, next)
    }

    /// Return from a simulated sample combat to the recorded combat.
    pub fn clear_sample(&mut self) {
        if self.sample.take().is_some() {
            self.playing = false;
            self.inspect = None;
            self.step = 0;
        }
    }

    /// Switch between a combat's two versions, at the matching step.
    pub fn toggle_alt(&mut self) {
        let Some(current) = self.shown_combat() else {
            return;
        };
        if self.combat.as_ref().is_none_or(|c| c.alt.is_none()) {
            return;
        }
        let step = current.steps[self.step.min(current.steps.len() - 1)].clone();
        self.sample = None;
        self.show_alt = !self.show_alt;
        let target = self.shown_combat().unwrap();
        self.step = target.matching_step(&step);
    }

    /// Whether player `p` is one the screen may show: any, or one of the focused players.
    fn in_focus(&self, p: usize) -> bool {
        self.focus.as_ref().is_none_or(|f| f.contains(&p))
    }

    /// The player whose Tavern the screen shows.
    pub fn shown_player(&self) -> usize {
        if self.follow {
            if let Some(actor) = self.source.info(self.frame).actor {
                if self.in_focus(actor) {
                    return actor;
                }
            }
        }
        match &self.focus {
            Some(f) if !f.contains(&self.perspective) => f[0],
            _ => self.perspective.min(self.names().len().saturating_sub(1)),
        }
    }

    /// Whether frame `i` is shown: every frame, or with a focus only those about the focused
    /// players (and the start and the end).
    pub fn visible(&self, i: usize) -> bool {
        let Some(focus) = &self.focus else {
            return true;
        };
        match self.source.involves(i) {
            None => true,
            Some(players) => players.iter().any(|p| focus.contains(p)),
        }
    }

    /// The first shown frame after `frame`, if any.
    fn next_visible(&self, frame: usize) -> Option<usize> {
        (frame + 1..self.len()).find(|&i| self.visible(i))
    }

    /// The last shown frame before `frame`, if any.
    fn prev_visible(&self, frame: usize) -> Option<usize> {
        (0..frame).rev().find(|&i| self.visible(i))
    }

    /// Only show the frames about `players` (`None`: every frame), moving to a shown frame.
    pub fn set_focus(&mut self, players: Option<Vec<usize>>) {
        self.focus = players.filter(|p| !p.is_empty());
        if let Some(f) = &self.focus {
            self.perspective = f[0];
        }
        if !self.visible(self.frame) {
            let target = self
                .next_visible(self.frame)
                .or_else(|| self.prev_visible(self.frame))
                .unwrap_or(self.frame);
            self.go_to(target);
        }
    }

    /// Focus on the shown player (and their Duos teammate), or show everyone again.
    pub fn toggle_focus(&mut self) {
        if self.focus.is_some() {
            self.set_focus(None);
        } else {
            let team = self.source.team_of(self.shown_player());
            self.set_focus(Some(team));
        }
    }

    /// Show `frame` (clamped), from its first step.
    pub fn go_to(&mut self, frame: usize) {
        self.frame = frame.min(self.len().saturating_sub(1));
        self.step = 0;
        self.sample = None;
        self.inspect = None;
        self.combat = self.source.combat(self.frame);
        if self.follow {
            if let Some(actor) = self.source.info(self.frame).actor {
                if self.in_focus(actor) {
                    self.perspective = actor;
                }
            }
        }
    }

    fn steps(&self) -> usize {
        self.shown_combat().map_or(1, |c| c.steps.len())
    }

    /// One step forward: the next combat step, else the next frame. Returns `false` at the end.
    pub fn next(&mut self) -> bool {
        if self.step + 1 < self.steps() {
            self.step += 1;
            true
        } else if let Some(next) = self.next_visible(self.frame) {
            self.go_to(next);
            true
        } else {
            false
        }
    }

    /// One step back: the previous combat step, else the previous frame's last step.
    pub fn prev(&mut self) {
        if self.step > 0 {
            self.step -= 1;
        } else if let Some(prev) = self.prev_visible(self.frame) {
            self.go_to(prev);
            self.step = self.steps() - 1;
        }
    }

    /// The next frame, skipping the rest of a combat.
    pub fn next_frame(&mut self) {
        if let Some(next) = self.next_visible(self.frame) {
            self.go_to(next);
        }
    }

    pub fn prev_frame(&mut self) {
        if let Some(prev) = self.prev_visible(self.frame) {
            self.go_to(prev);
        }
    }

    /// The first shown frame of the next turn (or the last frame).
    pub fn next_turn(&mut self) {
        let turn = self.source.info(self.frame).turn;
        let next = (self.frame + 1..self.len())
            .find(|&i| self.source.info(i).turn > turn && self.visible(i))
            .unwrap_or(self.len() - 1);
        self.go_to(next);
    }

    /// The first shown frame of this turn, or of the previous turn (with shown frames) if already
    /// there.
    pub fn prev_turn(&mut self) {
        let turn = |i: usize| self.source.info(i).turn;
        // The first shown frame of `frame`'s turn, up to `frame`.
        let first_shown = |frame: usize| {
            (0..=frame)
                .rev()
                .take_while(|&j| turn(j) == turn(frame))
                .filter(|&j| self.visible(j))
                .last()
        };
        let target = match first_shown(self.frame) {
            Some(first) if first < self.frame => first,
            // Already at this turn's first shown frame: the previous turn's (with a focus, frames
            // about other players are hidden, so a turn's own first frame may not be shown).
            _ => match self.prev_visible(self.frame) {
                Some(p) => first_shown(p).unwrap_or(p),
                None => self.frame,
            },
        };
        self.go_to(target);
    }

    fn is_combat(&self, i: usize) -> bool {
        self.source.info(i).kind == Kind::Combat && self.visible(i)
    }

    /// The next combat frame.
    pub fn next_combat(&mut self) {
        if let Some(i) = (self.frame + 1..self.len()).find(|&i| self.is_combat(i)) {
            self.go_to(i);
        }
    }

    /// The previous combat frame.
    pub fn prev_combat(&mut self) {
        if let Some(i) = (0..self.frame).rev().find(|&i| self.is_combat(i)) {
            self.go_to(i);
        }
    }

    /// Show the next player's Tavern (the next focused one, with a focus), and stop following the
    /// actor.
    pub fn cycle_perspective(&mut self) {
        self.follow = false;
        let shown = self.shown_player();
        self.perspective = match &self.focus {
            Some(f) => {
                let i = f.iter().position(|&p| p == shown).unwrap_or(0);
                f[(i + 1) % f.len()]
            }
            None => (shown + 1) % self.names().len().max(1),
        };
    }

    pub fn toggle_follow(&mut self) {
        self.follow = !self.follow;
        if self.follow {
            if let Some(actor) = self.source.info(self.frame).actor {
                self.perspective = actor;
            }
        }
    }

    /// How long auto-play shows the current step.
    pub fn delay(&self) -> Duration {
        let base = match (self.source.info(self.frame).kind, self.shown_combat()) {
            (Kind::Combat, Some(combat)) => {
                match combat.steps[self.step.min(combat.steps.len() - 1)].phase {
                    Phase::Start | Phase::End => 1400,
                    Phase::StartOfCombat | Phase::TagIn => 1200,
                    Phase::Strike => 900,
                    Phase::Settle => 500,
                }
            }
            (Kind::TurnStart, _) => 900,
            (Kind::Act, _) => 700,
            (Kind::End, _) => 2000,
            _ => 250,
        };
        Duration::from_millis((base as f32 / self.speed()) as u64)
    }
}
