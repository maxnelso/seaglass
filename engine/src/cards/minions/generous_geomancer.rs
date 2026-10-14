//! `Generous Geomancer` (`BGDUO_111`) — Tier 2 Quilboar (`1/1`), Duos only.
//! **Deathrattle:** You and your teammate each get a **Blood Gem** (Golden `BGDUO_111_G`, `2/2`:
//! `2` Blood Gems each).
//!
//! The teammate's Blood Gems are gifts (`docs/duos_tavern.md` §7.2): in combat they reach the
//! teammate's hand after the combat.

use crate::cards::{tokens, CardTemplate, DeathrattleContext};
use crate::model::{CardId, TeammateGift, Tribe, Unit};

pub const ID: CardId = 10204;
pub const NAME: &str = "Generous Geomancer";

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, NAME, 1, 1, 2)
        .with_tribe(Tribe::Quilboar)
        .on_deathrattle(on_deathrattle)
}

/// A Blood Gem (`2` if Golden) for the player, and as many for the teammate.
pub fn on_deathrattle(dying: &Unit, ctx: &mut DeathrattleContext<'_>) {
    for _ in 0..dying.golden_mult() {
        ctx.add_to_hand(tokens::make_blood_gem());
        ctx.auras
            .give_teammate(TeammateGift::Card(tokens::make_blood_gem()));
    }
}
