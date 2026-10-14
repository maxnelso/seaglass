//! `Shifty Snake` (`BGDUO31_203`) — Tier 4 Beast (`6/1`), Duos only.
//! **Deathrattle:** Your teammate gets a random **Deathrattle** minion (Golden `BGDUO31_203_G`,
//! `12/2`: `2` random Deathrattle minions).
//!
//! The minions are of this player's Tavern Tier or lower, from the game's catalog (the Duos one
//! in a Duos game: `cards::game_templates`), and are gifts: in combat they go into the teammate's
//! combat hand at once (`docs/duos.md` §2, `docs/duos_tavern.md` §8.2).

use crate::cards::{self, CardTemplate, DeathrattleContext};
use crate::model::{CardId, TeammateGift, Tribe, Unit};

pub const ID: CardId = 10407;
pub const NAME: &str = "Shifty Snake";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 6, 1, 4)
        .with_tribe(Tribe::Beast)
        .on_deathrattle(on_deathrattle)
}

pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    let max_tier = ctx.hero_tier.max(1);
    let candidates: Vec<&CardTemplate> = cards::game_templates(ctx.auras)
        .iter()
        .filter(|t| cards::is_deathrattle_minion(t.card_id) && t.tavern_tier <= max_tier)
        .collect();
    if candidates.is_empty() {
        return;
    }
    for _ in 0..dying.golden_mult() {
        let pick = ctx.rng.below(candidates.len());
        ctx.auras
            .give_teammate(TeammateGift::Card(candidates[pick].instantiate()));
    }
}
