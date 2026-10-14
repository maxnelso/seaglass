//! Heroes and their Hero Powers (`docs/heroes.md`).
//!
//! The hero pool is the generated [`HEROES`] table (`table.rs`, written by
//! `tools/gen_heroes.py` from `CardDefs.xml`). Each implemented hero has its own file
//! (`src/heroes/<hero>.rs`) whose header names the hero, its Armor and its Hero Power (checked
//! against the table by the tests below), and which defines its Hero Power's cards and their
//! [`CardHooks`]. A Hero Power is a card with hooks (`docs/heroes.md` §4): the Tavern and Combat
//! engines reach it only through the dispatch functions here, which look its hooks up by its card
//! id (its dbfId); they never name a hero.

mod table;

pub mod alakir;
pub mod brukan;
pub mod deathwing;
pub mod drekthar;
pub mod flobbidinous_floop;
pub mod illidan_stormrage;
pub mod queen_wagtoggle;
pub mod rokara;
pub mod tamsin_roame;
pub mod teron_gorefiend;
pub mod vanndar_stormpike;
pub mod yshaarj;

pub use table::{CARD_DEFS_BUILD, HEROES};

use crate::cards::{hooks, ActivateTargetKind, BoardCtx, CardHooks, HeroCombatCtx, Played};
use crate::model::{CardId, HeroId, HeroPowerState, HeroState, PlayerAuras, Unit, UnitId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// A hero of the pool (`docs/heroes.md` §2.1): one row of the generated table.
#[derive(Clone, Copy, Debug)]
pub struct HeroData {
    /// Its `HeroId`: the hero card's dbfId.
    pub id: HeroId,
    pub card_id: &'static str,
    pub name: &'static str,
    /// Starting Health.
    pub health: i32,
    /// Starting Armor in Solo (0 for a Duos-only hero).
    pub armor: i32,
    /// Starting Armor in Duos.
    pub duos_armor: i32,
    /// The data has no Duos Armor for this hero: `duos_armor` is its Solo Armor (an engine
    /// decision, `docs/heroes.md` §2.1).
    pub duos_armor_from_solo: bool,
    /// Only in Duos.
    pub duos_only: bool,
    pub power: PowerData,
    /// The other forms of its Hero Power: `(card ID, dbfId)`.
    pub power_variants: &'static [(&'static str, CardId)],
    /// Its skins' card IDs.
    pub skins: &'static [&'static str],
}

/// A hero's Hero Power, as the data gives it.
#[derive(Clone, Copy, Debug)]
pub struct PowerData {
    /// Its dbfId: the Hero Power's engine `CardId`.
    pub id: CardId,
    pub card_id: &'static str,
    pub name: &'static str,
    /// Its starting Gold cost.
    pub cost: u32,
    pub text: &'static str,
}

/// Heroes offered to each player at the start of a game (`docs/heroes.md` §6, confirmed).
pub const OFFER_COUNT: usize = 2;

/// The most hero offers the RL action space has room for (`docs/heroes.md` §8).
pub const MAX_OFFER_COUNT: usize = 4;

/// The source of the damage a Hero Power deals in combat (it is not a minion).
pub const HERO_SOURCE: UnitId = UnitId::MAX;

/// The base Health a hero's `health` is counted from (`docs/heroes.md` §2.3).
const BASE_HEALTH: i32 = 30;

/// An implemented hero: its id and its Hero Power cards' hooks.
type Implemented = (HeroId, fn() -> Vec<(CardId, CardHooks)>);

/// Every implemented hero, by name.
const IMPLEMENTED: &[Implemented] = &[
    (alakir::ID, alakir::behaviors),
    (brukan::ID, brukan::behaviors),
    (deathwing::ID, deathwing::behaviors),
    (drekthar::ID, drekthar::behaviors),
    (flobbidinous_floop::ID, flobbidinous_floop::behaviors),
    (illidan_stormrage::ID, illidan_stormrage::behaviors),
    (queen_wagtoggle::ID, queen_wagtoggle::behaviors),
    (rokara::ID, rokara::behaviors),
    (tamsin_roame::ID, tamsin_roame::behaviors),
    (teron_gorefiend::ID, teron_gorefiend::behaviors),
    (vanndar_stormpike::ID, vanndar_stormpike::behaviors),
    (yshaarj::ID, yshaarj::behaviors),
];

/// The hooks of every implemented hero's Hero Power cards (and the cards they use: Choose One
/// options, Deathrattles they give), for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    IMPLEMENTED.iter().flat_map(|(_, cards)| cards()).collect()
}

/// The hero pool, sorted by name.
pub fn table() -> &'static [HeroData] {
    HEROES
}

/// The hero `id`.
pub fn data(id: HeroId) -> Option<&'static HeroData> {
    HEROES.iter().find(|h| h.id == id)
}

/// The hero named `name` (case-insensitive; its slug works too: `alakir`).
pub fn by_name(name: &str) -> Option<&'static HeroData> {
    let wanted = name.trim();
    HEROES
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(wanted) || crate::scenario::slug(h.name) == wanted)
}

/// The hero whose card (or one of whose skins) is `card_id`.
pub fn by_card_id(card_id: &str) -> Option<&'static HeroData> {
    HEROES
        .iter()
        .find(|h| h.card_id == card_id || h.skins.contains(&card_id))
}

/// The engine `CardId` of the Hero Power card `card_id`: a hero's power or one of its forms.
pub fn power_card(card_id: &str) -> Option<CardId> {
    HEROES.iter().find_map(|h| {
        if h.power.card_id == card_id {
            Some(h.power.id)
        } else {
            h.power_variants
                .iter()
                .find(|(id, _)| *id == card_id)
                .map(|&(_, dbf)| dbf)
        }
    })
}

/// `true` if the engine implements the hero `id`'s Hero Power.
pub fn is_implemented(id: HeroId) -> bool {
    IMPLEMENTED.iter().any(|&(hero, _)| hero == id)
}

/// The heroes a game can offer (`docs/heroes.md` §6): the implemented ones, Duos-only heroes only
/// in Duos, in table order.
pub fn offerable(duos: bool) -> Vec<HeroId> {
    HEROES
        .iter()
        .filter(|h| is_implemented(h.id) && (duos || !h.duos_only))
        .map(|h| h.id)
        .collect()
}

/// Draw `per_player` distinct heroes for each of `players` players (`docs/heroes.md` §6): for
/// each player in turn, uniformly among the offerable heroes not offered yet, one `rng.below(k)`
/// per hero. A player gets fewer if the pool runs out.
pub fn draw_offers(
    players: usize,
    per_player: usize,
    duos: bool,
    rng: &mut Rng,
) -> Vec<Vec<HeroId>> {
    let mut left = offerable(duos);
    (0..players)
        .map(|_| {
            (0..per_player)
                .filter_map(|_| match left.len() {
                    0 => None,
                    k => Some(left.remove(rng.below(k))),
                })
                .collect()
        })
        .collect()
}

/// A fresh state for the hero `h` (`docs/heroes.md` §3.2): its Hero Power at its starting cost,
/// with a use left if it is active.
pub fn new_state(h: &HeroData) -> HeroState {
    HeroState {
        hero_id: h.id,
        power: HeroPowerState {
            card_id: h.power.id,
            cost: h.power.cost,
            uses_left: u32::from(hooks(h.power.id).hero_power.is_some()),
            counter: 0,
            stacks: 0,
            armed: 0,
            stored: Vec::new(),
        },
    }
}

/// `(extra Health, Armor)` the hero `h` starts with (`docs/heroes.md` §2.3): its Health beyond
/// 30, and its Duos or Solo Armor.
pub fn starting_bonus(h: &HeroData, duos: bool) -> (i32, i32) {
    let armor = if duos { h.duos_armor } else { h.armor };
    (h.health - BASE_HEALTH, armor)
}

// ---------------------------------------------------------------------------------------------
// Dispatch: the player's Hero Power's hooks
// ---------------------------------------------------------------------------------------------

/// The player's Hero Power's hooks, if they have a hero.
fn power_hooks(auras: &PlayerAuras) -> Option<&'static CardHooks> {
    auras.hero.as_ref().map(|h| hooks(h.power.card_id))
}

/// `true` if `target` is a valid target of kind `kind` in `state` (`docs/heroes.md` §3.3).
fn target_ok(state: &TavernState, kind: ActivateTargetKind, target: Option<usize>) -> bool {
    match (kind, target) {
        (ActivateTargetKind::None, None) => true,
        (ActivateTargetKind::BoardAny, Some(t)) => t < state.board.len(),
        (ActivateTargetKind::ShopCard, Some(t)) => state.shop.get(t).is_some_and(|u| !u.is_spell),
        (ActivateTargetKind::HandCard, Some(t)) => {
            state.hand.get(t).is_some_and(|c| c.locked_turns == 0)
        }
        _ => false,
    }
}

/// The candidate targets of a Hero Power of target kind `kind` in `state`.
fn targets(state: &TavernState, kind: ActivateTargetKind) -> Vec<Option<usize>> {
    let n = match kind {
        ActivateTargetKind::None => return vec![None],
        ActivateTargetKind::BoardAny => state.board.len(),
        ActivateTargetKind::ShopCard => state.shop.len(),
        ActivateTargetKind::HandCard => state.hand.len(),
        _ => 0,
    };
    (0..n).map(Some).collect()
}

/// `true` if the player can use their Hero Power on `target` now, apart from pending prompts
/// (`docs/heroes.md` §5): an active power with uses left, enough Gold, its own conditions, a
/// valid target.
pub fn can_use(state: &TavernState, target: Option<usize>) -> bool {
    let Some(hero) = state.auras.hero.as_ref() else {
        return false;
    };
    let card = hooks(hero.power.card_id);
    card.hero_power.is_some()
        && hero.power.uses_left > 0
        && state.gold >= hero.power.cost
        && card.hero_power_usable.is_none_or(|usable| usable(state))
        && target_ok(state, card.activate_target, target)
}

/// The targets the player can use their Hero Power on now (empty if they can't use it).
pub fn valid_targets(state: &TavernState) -> Vec<Option<usize>> {
    let Some(card) = power_hooks(&state.auras) else {
        return Vec::new();
    };
    targets(state, card.activate_target)
        .into_iter()
        .filter(|&t| can_use(state, t))
        .collect()
}

/// Use the player's Hero Power on `target` (legal, `docs/heroes.md` §5): a use, its cost (Gold
/// spent), then its `hero_power` hook.
pub fn use_power(
    state: &mut TavernState,
    target: Option<usize>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(hero) = state.auras.hero.as_mut() else {
        return;
    };
    hero.power.uses_left -= 1;
    let (cost, card_id) = (hero.power.cost, hero.power.card_id);
    state.spend_gold(cost, pool, rng);
    if let Some(hero_power) = hooks(card_id).hero_power {
        hero_power(state, target, pool, rng);
    }
}

/// Start of turn (`docs/heroes.md` §3.3, §3.4): the board's marks clear, and the Hero Power's
/// uses are reset (1 if it is active, 0 if passive) before its `hero_turn_reset` hook.
pub fn turn_reset(state: &mut TavernState) {
    let Some(hero) = state.auras.hero.as_mut() else {
        return;
    };
    for unit in &mut state.board {
        unit.marked = false;
    }
    let card = hooks(hero.power.card_id);
    hero.power.uses_left = u32::from(card.hero_power.is_some());
    if let Some(reset) = card.hero_turn_reset {
        reset(hero);
    }
}

/// The player got their hero: its Hero Power's `hero_chosen` hook (`docs/heroes.md` §2.3).
pub fn on_chosen(state: &mut TavernState) {
    if let Some(chosen) = power_hooks(&state.auras).and_then(|c| c.hero_chosen) {
        chosen(state);
    }
}

/// Gold was spent: the Hero Power's `hero_gold_spent` hook.
pub fn on_gold_spent(state: &mut TavernState, amount: u32) {
    if let Some(gold_spent) = power_hooks(&state.auras).and_then(|c| c.hero_gold_spent) {
        gold_spent(state, amount);
    }
}

/// `true` if the player's Hero Power acts at Start of Combat (`docs/combat.md` §5.1.1 step 1).
pub fn has_start_of_combat(auras: &PlayerAuras) -> bool {
    power_hooks(auras).is_some_and(|c| c.hero_start_of_combat.is_some())
}

/// The player's Hero Power's `hero_start_of_combat` hook (`docs/heroes.md` §7.2).
pub fn start_of_combat_hook(auras: &PlayerAuras) -> Option<fn(&mut HeroCombatCtx<'_>)> {
    power_hooks(auras).and_then(|c| c.hero_start_of_combat)
}

/// Room may have opened on the board in combat: the Hero Power's `hero_combat_space` hook.
pub fn combat_space(ctx: &mut BoardCtx<'_>) {
    if let Some(combat_space) = power_hooks(ctx.auras).and_then(|c| c.hero_combat_space) {
        combat_space(ctx);
    }
}

/// Start of turn, once the shop is filled: the Hero Power's `hero_turn_start` hook.
pub fn on_turn_start(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(turn_start) = power_hooks(&state.auras).and_then(|c| c.hero_turn_start) {
        turn_start(state, pool, rng);
    }
}

/// After a card was bought: the Hero Power's `hero_after_buy` hook.
pub fn after_buy(state: &mut TavernState, bought: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(after_buy) = power_hooks(&state.auras).and_then(|c| c.hero_after_buy) {
        after_buy(state, bought, pool, rng);
    }
}

/// After a minion was sold: the Hero Power's `hero_after_sell` hook.
pub fn after_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(after_sell) = power_hooks(&state.auras).and_then(|c| c.hero_after_sell) {
        after_sell(state, sold, pool, rng);
    }
}

/// After a minion was played or Magnetized: the Hero Power's `hero_after_play` hook.
pub fn after_play(state: &mut TavernState, played: &Played, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(after_play) = power_hooks(&state.auras).and_then(|c| c.hero_after_play) {
        after_play(state, played, pool, rng);
    }
}

/// After the Tavern was upgraded: the Hero Power's `hero_after_upgrade` hook.
pub fn after_upgrade(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(after_upgrade) = power_hooks(&state.auras).and_then(|c| c.hero_after_upgrade) {
        after_upgrade(state, pool, rng);
    }
}

/// A friendly minion is being summoned in combat: the Hero Power's `hero_combat_summon` hook.
pub fn combat_summon(auras: &PlayerAuras, summoned: &mut Unit) {
    let Some(hero) = auras.hero.as_ref() else {
        return;
    };
    if let Some(combat_summon) = hooks(hero.power.card_id).hero_combat_summon {
        combat_summon(hero, summoned);
    }
}

/// `true` if the player's Hero Power reacts to their minions' deaths in combat.
pub fn has_friendly_death(auras: &PlayerAuras) -> bool {
    power_hooks(auras).is_some_and(|c| c.hero_friendly_death.is_some())
}

/// A friendly minion died in combat: the Hero Power's `hero_friendly_death` hook.
pub fn on_friendly_death(ctx: &mut BoardCtx<'_>, dying: &Unit) {
    if let Some(friendly_death) = power_hooks(ctx.auras).and_then(|c| c.hero_friendly_death) {
        friendly_death(ctx, dying);
    }
}

/// `true` if the player's Hero Power reacts to their minions' kills (`hero_enemy_killed`).
pub fn has_enemy_killed(auras: &PlayerAuras) -> bool {
    power_hooks(auras).is_some_and(|c| c.hero_enemy_killed.is_some())
}

/// The player's minion `killer` killed the enemy minion `victim` (`docs/heroes.md` §7.3).
pub fn on_enemy_killed(ctx: &mut BoardCtx<'_>, killer: UnitId, victim: &Unit) {
    if let Some(enemy_killed) = power_hooks(ctx.auras).and_then(|c| c.hero_enemy_killed) {
        enemy_killed(ctx, killer, victim);
    }
}

/// The `hero_power` hook of the powers that choose a friendly minion to act on at the next Start
/// of Combat (`docs/heroes.md` §3.3): mark `board[target]`, unmarking the others.
pub fn mark_minion(state: &mut TavernState, target: Option<usize>, _: &mut CardPool, _: &mut Rng) {
    for unit in &mut state.board {
        unit.marked = false;
    }
    if let Some(unit) = target.and_then(|t| state.board.get_mut(t)) {
        unit.marked = true;
    }
}

/// The turn the Hero Powers that "unlock on Turn N" unlock (`docs/heroes.md` §11).
pub const UNLOCK_TURN: i32 = 7;

/// `hero_turn_reset` of the Hero Powers that act once per combat once unlocked (Vanndar,
/// Drek'Thar): count the turn (`counter`), and from `UNLOCK_TURN` on arm the coming combat.
pub fn count_turn_and_arm(hero: &mut HeroState) {
    hero.power.counter += 1;
    hero.power.armed = u32::from(hero.power.counter >= UNLOCK_TURN);
}

/// Once per combat, if armed and there is room: summon a copy of the friendly minion with the
/// most `key` (a tie at random, one draw) at the right end of the board, using up the arm
/// (Vanndar's highest-Health, Drek'Thar's highest-Attack).
pub fn summon_copy_of_best(ctx: &mut BoardCtx<'_>, key: fn(&Unit) -> i32, reason: &'static str) {
    if power_mut(ctx.auras).is_none_or(|p| p.armed == 0)
        || ctx.board.len() >= crate::combat::MAX_BOARD_SIZE
    {
        return;
    }
    let Some(best) = ctx.board.iter().filter(|u| u.health > 0).map(key).max() else {
        return;
    };
    let tied: Vec<usize> = (0..ctx.board.len())
        .filter(|&i| ctx.board[i].health > 0 && key(&ctx.board[i]) == best)
        .collect();
    let pick = match tied.len() {
        1 => tied[0],
        k => tied[ctx.rng.below(k)],
    };
    let mut copy = ctx.board[pick].clone();
    copy.marked = false;
    if let Some(power) = power_mut(ctx.auras) {
        power.armed = 0;
    }
    ctx.cursor = ctx.board.len();
    ctx.summon_as(HERO_SOURCE, copy, reason);
}

/// The player's Hero Power state (for hooks that know the player has a hero).
pub fn power_mut(auras: &mut PlayerAuras) -> Option<&mut HeroPowerState> {
    auras.hero.as_mut().map(|h| &mut h.power)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// `(file stem, header lines)` of every hero file.
    fn hero_files() -> Vec<(String, String)> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/heroes");
        let mut files: Vec<(String, String)> = fs::read_dir(&dir)
            .expect("src/heroes")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|e| e == "rs"))
            .map(|p| {
                let stem = p.file_stem().unwrap().to_string_lossy().into_owned();
                (stem, fs::read_to_string(&p).expect("readable"))
            })
            .filter(|(stem, _)| stem != "mod" && stem != "table")
            .collect();
        files.sort();
        files
    }

    /// The header a hero file must start with (`docs/heroes.md` §9).
    fn expected_header(h: &HeroData) -> String {
        let armor = if h.duos_only {
            format!("Duos Armor {}", h.duos_armor)
        } else {
            format!("Armor {} (Duos {})", h.armor, h.duos_armor)
        };
        format!(
            "//! `{}` (`{}`) — {armor}.\n//!\n//! Hero Power: `{}` (`{}`), {} Gold: {}\n",
            h.name, h.card_id, h.power.name, h.power.card_id, h.power.cost, h.power.text
        )
    }

    /// The value of `pub const <name>: <ty> = <value>;` in `src`.
    fn constant(src: &str, name: &str) -> Option<u32> {
        let line = src
            .lines()
            .find(|l| l.starts_with(&format!("pub const {name}: ")))?;
        line.split('=')
            .nth(1)?
            .trim()
            .trim_end_matches(';')
            .replace('_', "")
            .parse()
            .ok()
    }

    #[test]
    fn every_hero_file_matches_the_table() {
        let files = hero_files();
        assert_eq!(
            files.len(),
            IMPLEMENTED.len(),
            "every hero file is in IMPLEMENTED"
        );
        for (stem, src) in &files {
            let id = constant(src, "ID").unwrap_or_else(|| panic!("{stem}.rs: no `pub const ID`"));
            let h = data(id).unwrap_or_else(|| panic!("{stem}.rs: ID {id} is not in the table"));
            assert_eq!(
                crate::scenario::slug(h.name),
                *stem,
                "{stem}.rs: file name is the hero's slug"
            );
            let header = expected_header(h);
            assert!(
                src.starts_with(&header),
                "{stem}.rs: the header doesn't match the table; expected:\n{header}"
            );
            assert_eq!(
                constant(src, "POWER"),
                Some(h.power.id),
                "{stem}.rs: POWER is the table's"
            );
            assert!(is_implemented(id), "{stem}.rs: not in IMPLEMENTED");
            assert!(
                behaviors().iter().any(|&(card, _)| card == h.power.id),
                "{stem}.rs: its Hero Power has no hooks"
            );
        }
    }

    #[test]
    fn the_table_has_the_whole_pool() {
        assert_eq!(HEROES.len(), 123);
        let patchwerk = by_name("Patchwerk").expect("Patchwerk");
        assert_eq!(
            (patchwerk.health, patchwerk.armor, patchwerk.duos_armor),
            (60, 0, 0)
        );
        let reno = by_name("Reno Jackson").expect("Reno");
        assert_eq!((reno.armor, reno.duos_armor), (12, 10));
        assert_eq!(
            by_card_id("TB_BaconShop_HERO_34_SKIN_E").map(|h| h.name),
            Some("Patchwerk")
        );
        assert_eq!(power_card("BG22_HERO_001p_t3"), Some(79723));
        for h in HEROES {
            assert!(
                h.power.id > 11_000,
                "{}: power ids don't collide with card ids",
                h.name
            );
        }
    }

    #[test]
    fn offers_are_distinct_across_the_lobby() {
        let mut rng = Rng::new(7);
        let offers = draw_offers(4, OFFER_COUNT, false, &mut rng);
        let all: Vec<HeroId> = offers.iter().flatten().copied().collect();
        let mut unique = all.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(all.len(), unique.len());
        assert!(offers.iter().all(|o| o.len() == OFFER_COUNT));
        assert!(all.iter().all(|&id| !data(id).unwrap().duos_only));
    }
}
