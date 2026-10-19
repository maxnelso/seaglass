//! The replay player's state and navigation (`match_replay tui`).

use std::time::Duration;

use seaglass_tools::render::Glyphs;

use super::combat::{CombatPlayback, Phase};
use super::show::{Kind, Source};

/// The playback speeds `+` and `-` step through.
const SPEEDS: [f32; 7] = [0.25, 0.5, 1.0, 1.5, 2.0, 4.0, 8.0];

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
    /// Show the combat's other version (the engine's) instead.
    pub show_alt: bool,
    /// The player whose Tavern is shown (when not following the actor).
    pub perspective: usize,
    /// Show the Tavern of whoever acts in the frame.
    pub follow: bool,
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
            show_alt: false,
            perspective: 0,
            follow: true,
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

    /// The combat version shown: the frame's, or its other version when toggled.
    pub fn shown_combat(&self) -> Option<&CombatPlayback> {
        let combat = self.combat.as_ref()?;
        match (&combat.alt, self.show_alt) {
            (Some(alt), true) => Some(alt),
            _ => Some(combat),
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
        self.show_alt = !self.show_alt;
        let target = self.shown_combat().unwrap();
        self.step = target.matching_step(&step);
    }

    /// The player whose Tavern the screen shows.
    pub fn shown_player(&self) -> usize {
        if self.follow {
            if let Some(actor) = self.source.info(self.frame).actor {
                return actor;
            }
        }
        self.perspective.min(self.names().len().saturating_sub(1))
    }

    /// Show `frame` (clamped), from its first step.
    pub fn go_to(&mut self, frame: usize) {
        self.frame = frame.min(self.len().saturating_sub(1));
        self.step = 0;
        self.inspect = None;
        self.combat = self.source.combat(self.frame);
        if self.follow {
            if let Some(actor) = self.source.info(self.frame).actor {
                self.perspective = actor;
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
        } else if self.frame + 1 < self.len() {
            self.go_to(self.frame + 1);
            true
        } else {
            false
        }
    }

    /// One step back: the previous combat step, else the previous frame's last step.
    pub fn prev(&mut self) {
        if self.step > 0 {
            self.step -= 1;
        } else if self.frame > 0 {
            self.go_to(self.frame - 1);
            self.step = self.steps() - 1;
        }
    }

    /// The next frame, skipping the rest of a combat.
    pub fn next_frame(&mut self) {
        self.go_to(self.frame + 1);
    }

    pub fn prev_frame(&mut self) {
        self.go_to(self.frame.saturating_sub(1));
    }

    /// The first frame of the next turn (or the last frame).
    pub fn next_turn(&mut self) {
        let turn = self.source.info(self.frame).turn;
        let next = (self.frame + 1..self.len())
            .find(|&i| self.source.info(i).turn > turn)
            .unwrap_or(self.len() - 1);
        self.go_to(next);
    }

    /// The first frame of this turn, or of the previous turn if already there.
    pub fn prev_turn(&mut self) {
        let turn_start = |i: usize| {
            let t = self.source.info(i).turn;
            (0..=i)
                .rev()
                .take_while(|&j| self.source.info(j).turn == t)
                .last()
                .unwrap_or(0)
        };
        let start = turn_start(self.frame);
        let target = if start == self.frame && start > 0 {
            turn_start(start - 1)
        } else {
            start
        };
        self.go_to(target);
    }

    fn is_combat(&self, i: usize) -> bool {
        self.source.info(i).kind == Kind::Combat
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

    /// Show the next player's Tavern (and stop following the actor).
    pub fn cycle_perspective(&mut self) {
        self.follow = false;
        self.perspective = (self.shown_player() + 1) % self.names().len().max(1);
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
