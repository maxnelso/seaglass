//! Dark Gifts (`docs/dark_gifts.md`).
//!
//! The gifts are the generated [`DARK_GIFTS`] table (`table.rs`, written by
//! `tools/gen_dark_gifts.py` from `CardDefs.xml`). Each implemented gift has its own file
//! (`src/dark_gifts/<slug>.rs`) whose header names the gift, its turn window, weight and text
//! (checked against the table by the tests below), and which defines its [`CardHooks`]. A gift is
//! a card with hooks (`docs/dark_gifts.md` §6) that acts for the minion it is on
//! (`Unit::dark_gifts`): the Tavern and Combat engines reach it only through the functions here,
//! which look its hooks up by its card id (its dbfId); they never name a gift.
//!
//! This module also holds the Dark Discovery button (`docs/dark_gifts.md` §3) and the offers
//! (§4, §5), which the heroes and trinkets that give gifts reuse (§12).

mod table;
mod vocab;

pub mod admiration;
pub mod amalgamation;
pub mod battle_scars_early;
pub mod battle_scars_late;
pub mod charisma;
pub mod consanguinity;
pub mod deaths_embrace_early;
pub mod deaths_embrace_late;
pub mod defensive_sacrifice;
pub mod demonology;
pub mod desecration;
pub mod doppelganger;
pub mod fortitude;
pub mod fresh_perspective;
pub mod furtiveness;
pub mod gilding;
pub mod golemancy;
pub mod harpys_talons;
pub mod hostility;
pub mod invulnerability;
pub mod jaws_of_death;
pub mod mystic_essence;
pub mod offensive_sacrifice;
pub mod persisting_horror;
pub mod resistance;
pub mod spell_siphon_early;
pub mod spell_siphon_late;
pub mod tarecgosas_blessing;
pub mod titanic_strength;
pub mod toreths_blessing;
pub mod toxicity;
pub mod transcendence;
pub mod worship;

pub use table::{CARD_DEFS_BUILD, DARK_GIFTS};
pub use vocab::vocabulary;

use crate::cards::{self, hooks, spells, CardFlags, CardHooks, CardTemplate};
use crate::model::{CardId, DarkGift, PlayerAuras, Tribe, Unit, UnitId, LOBBY_TRIBES};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// The two strengths of a gift name (`docs/dark_gifts.md` §2.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// The one with the earlier first turn.
    Early,
    /// The other.
    Late,
}

/// A Dark Gift (`docs/dark_gifts.md` §2.1): one row of the generated table.
#[derive(Clone, Copy, Debug)]
pub struct DarkGiftData {
    /// Its dbfId: the gift's engine `CardId`.
    pub id: CardId,
    pub card_id: &'static str,
    pub name: &'static str,
    /// `Early` / `Late` for the two strengths of a shared name.
    pub variant: Option<Variant>,
    /// The first game turn it can be offered on (0: from the first).
    pub min_turn: u32,
    /// The last game turn it can be offered on (`None`: no last turn).
    pub max_turn: Option<u32>,
    /// Its weight in a draw.
    pub weight: u32,
    /// `Tras'tath, Soul Parasite`'s Discover can give it.
    pub trastath: bool,
    pub text: &'static str,
    /// Its enchantments' card IDs (what a Power.log attaches to a minion with the gift).
    pub enchantments: &'static [&'static str],
}

/// The button's price in Gold (`docs/dark_gifts.md` §3.1, confirmed).
pub const PRICE: u32 = 3;
/// The turn the button unlocks on (confirmed).
pub const UNLOCK_TURN: u32 = 3;
/// The button's uses per game (confirmed).
pub const USES_PER_GAME: u32 = 3;
/// Options in an offer (confirmed).
pub const OFFER_COUNT: usize = 3;

/// The minions no offer has (`docs/dark_gifts.md` §4.1 rule 5, engine decision: the post's
/// examples, and `Dark Paradox`, which has a gift of its own).
pub const BANNED: [&str; 3] = ["Leeroy the Reckless", "Deadly Spore", "Dark Paradox"];

/// An implemented gift: its id and its cards' hooks.
type Implemented = (CardId, fn() -> Vec<(CardId, CardHooks)>);

/// Every implemented gift, by file name.
const IMPLEMENTED: &[Implemented] = &[
    (admiration::ID, admiration::behaviors),
    (amalgamation::ID, amalgamation::behaviors),
    (battle_scars_early::ID, battle_scars_early::behaviors),
    (battle_scars_late::ID, battle_scars_late::behaviors),
    (charisma::ID, charisma::behaviors),
    (consanguinity::ID, consanguinity::behaviors),
    (deaths_embrace_early::ID, deaths_embrace_early::behaviors),
    (deaths_embrace_late::ID, deaths_embrace_late::behaviors),
    (defensive_sacrifice::ID, defensive_sacrifice::behaviors),
    (demonology::ID, demonology::behaviors),
    (desecration::ID, desecration::behaviors),
    (doppelganger::ID, doppelganger::behaviors),
    (fortitude::ID, fortitude::behaviors),
    (fresh_perspective::ID, fresh_perspective::behaviors),
    (furtiveness::ID, furtiveness::behaviors),
    (gilding::ID, gilding::behaviors),
    (golemancy::ID, golemancy::behaviors),
    (harpys_talons::ID, harpys_talons::behaviors),
    (hostility::ID, hostility::behaviors),
    (invulnerability::ID, invulnerability::behaviors),
    (jaws_of_death::ID, jaws_of_death::behaviors),
    (mystic_essence::ID, mystic_essence::behaviors),
    (offensive_sacrifice::ID, offensive_sacrifice::behaviors),
    (persisting_horror::ID, persisting_horror::behaviors),
    (resistance::ID, resistance::behaviors),
    (spell_siphon_early::ID, spell_siphon_early::behaviors),
    (spell_siphon_late::ID, spell_siphon_late::behaviors),
    (tarecgosas_blessing::ID, tarecgosas_blessing::behaviors),
    (titanic_strength::ID, titanic_strength::behaviors),
    (toreths_blessing::ID, toreths_blessing::behaviors),
    (toxicity::ID, toxicity::behaviors),
    (transcendence::ID, transcendence::behaviors),
    (worship::ID, worship::behaviors),
];

/// The hooks of every implemented gift's cards, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    IMPLEMENTED.iter().flat_map(|(_, cards)| cards()).collect()
}

/// The gifts, sorted by name.
pub fn table() -> &'static [DarkGiftData] {
    DARK_GIFTS
}

/// The gift `id`.
pub fn data(id: CardId) -> Option<&'static DarkGiftData> {
    DARK_GIFTS.iter().find(|g| g.id == id)
}

/// The gift whose card ID is `card_id`.
pub fn by_card_id(card_id: &str) -> Option<&'static DarkGiftData> {
    DARK_GIFTS.iter().find(|g| g.card_id == card_id)
}

/// The gift whose enchantment is `card_id` (`BG36_MidGameEffect_000t9e2` -> `Admiration`).
pub fn by_enchantment(card_id: &str) -> Option<&'static DarkGiftData> {
    DARK_GIFTS
        .iter()
        .find(|g| g.enchantments.contains(&card_id))
}

/// The name scenarios and headers use for `g`: its name, with ` (early)` or ` (late)` for one of
/// two strengths (`docs/dark_gifts.md` §9).
pub fn display_name(g: &DarkGiftData) -> String {
    match g.variant {
        None => g.name.to_string(),
        Some(Variant::Early) => format!("{} (early)", g.name),
        Some(Variant::Late) => format!("{} (late)", g.name),
    }
}

/// The file stem of `g`: its display name's slug (`battle_scars_late`).
pub fn slug(g: &DarkGiftData) -> String {
    crate::scenario::slug(&display_name(g))
}

/// The gift named `name` (case-insensitive): its display name, its slug, or its card ID.
pub fn by_name(name: &str) -> Option<&'static DarkGiftData> {
    let wanted = name.trim();
    DARK_GIFTS
        .iter()
        .find(|g| display_name(g).eq_ignore_ascii_case(wanted) || slug(g) == wanted)
        .or_else(|| by_card_id(wanted))
}

/// `true` if the engine implements the gift `id`.
pub fn is_implemented(id: CardId) -> bool {
    IMPLEMENTED.iter().any(|&(g, _)| g == id)
}

// ---------------------------------------------------------------------------------------------
// A minion's gifts
// ---------------------------------------------------------------------------------------------

/// Give `unit` the gift `id` on game turn `turn` (`docs/dark_gifts.md` §2.2): it joins the
/// minion's gifts (its `stacks` the turn) and its `gift_given` text resolves.
pub fn give(unit: &mut Unit, id: CardId, turn: u32) {
    let mut gift = DarkGift {
        card_id: id,
        counter: 0,
        stacks: turn as i32,
    };
    if let Some(given) = hooks(id).gift_given {
        given(unit, &mut gift);
    }
    unit.dark_gifts.push(gift);
}

/// Attach the gift `id` to `unit` without its `gift_given` text (the replay: the log's minion
/// already has what it gave; `docs/dark_gifts.md` §10).
pub fn attach(unit: &mut Unit, id: CardId) {
    unit.dark_gifts.push(DarkGift {
        card_id: id,
        counter: 0,
        stacks: 0,
    });
}

/// `true` if `unit` has the gift `id`.
pub fn has(unit: &Unit, id: CardId) -> bool {
    unit.dark_gifts.iter().any(|g| g.card_id == id)
}

/// The Golden a Triple made from `copies` gets every copy's gifts, in copy order, and what they
/// keep giving it (`gift_reapply`; `docs/dark_gifts.md` §2.2).
pub fn merge_into_golden(copies: &[Unit], golden: &mut Unit) {
    golden.dark_gifts = copies
        .iter()
        .flat_map(|u| u.dark_gifts.iter().copied())
        .collect();
    for gift in golden.dark_gifts.clone() {
        if let Some(reapply) = hooks(gift.card_id).gift_reapply {
            reapply(golden);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Dispatch: a minion's gifts' hooks, in the order it got them
// ---------------------------------------------------------------------------------------------

/// Where a minion is, for `gift_turn_start`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zone {
    Board,
    Hand,
}

/// The stats `unit`'s gifts give it wherever it is (their `gift_aura`s; none without gifts).
pub fn aura_bonus(unit: &Unit, auras: &PlayerAuras) -> (i32, i32) {
    let (mut atk, mut hp) = (0, 0);
    for gift in &unit.dark_gifts {
        if let Some(f) = hooks(gift.card_id).gift_aura {
            let (a, h) = f(unit, gift, auras);
            atk += a;
            hp += h;
        }
    }
    (atk, hp)
}

/// `card` was Discovered: each of its gifts' `gift_obtained` hook.
pub fn on_obtained(state: &mut TavernState, card: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    for gift in &card.dark_gifts {
        if let Some(obtained) = hooks(gift.card_id).gift_obtained {
            obtained(state, card, pool, rng);
        }
    }
}

/// The board's gifts with the hook `get`, left to right then in gift order: `(pos, card_id,
/// gift_idx, gift_id, hook)`.
fn board_gifts<F>(
    board: &[Unit],
    get: impl Fn(&CardHooks) -> Option<F>,
) -> Vec<(usize, CardId, usize, CardId, F)> {
    let mut out = Vec::new();
    for (pos, unit) in board.iter().enumerate() {
        for (i, gift) in unit.dark_gifts.iter().enumerate() {
            if let Some(f) = get(hooks(gift.card_id)) {
                out.push((pos, unit.card_id, i, gift.card_id, f));
            }
        }
    }
    out
}

/// `true` if `board[pos]` is still the minion `card_id` with the gift `gift_id` at `gift_idx`.
fn still_there(
    board: &[Unit],
    pos: usize,
    card_id: CardId,
    gift_idx: usize,
    gift_id: CardId,
) -> bool {
    board.get(pos).is_some_and(|u| {
        u.card_id == card_id
            && u.dark_gifts
                .get(gift_idx)
                .is_some_and(|g| g.card_id == gift_id)
    })
}

/// The player plays a card from the hand: each board minion's gifts' `gift_card_played` hooks.
pub fn on_card_played(state: &mut TavernState) {
    for (pos, card_id, i, gift_id, f) in board_gifts(&state.board, |h| h.gift_card_played) {
        if still_there(&state.board, pos, card_id, i, gift_id) {
            f(state, pos, i);
        }
    }
}

/// The player ends their turn: each board minion's gifts' `gift_end_of_turn` hooks, left to
/// right, with the end-of-turn repeat count. Nothing happens (not even a Triple check) without a
/// gift on the board.
pub fn on_end_of_turn(state: &mut TavernState, repeats: u32, pool: &mut CardPool, rng: &mut Rng) {
    let listed = board_gifts(&state.board, |h| h.gift_end_of_turn);
    if listed.is_empty() {
        return;
    }
    state.with_deferred_triples(|state| {
        for (pos, card_id, i, gift_id, f) in listed {
            if still_there(&state.board, pos, card_id, i, gift_id) {
                f(state, pos, i, repeats, pool, rng);
            }
        }
    });
}

/// The `gift_end_of_turn` hooks of the board minion `pos`'s gifts but `skip` (`Time Turning`:
/// its end of turn effects at the start of turn), once each.
pub fn end_of_turn_of(
    state: &mut TavernState,
    pos: usize,
    skip: usize,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(unit) = state.board.get(pos) else {
        return;
    };
    let card_id = unit.card_id;
    let listed: Vec<(usize, CardId, _)> = unit
        .dark_gifts
        .iter()
        .enumerate()
        .filter(|&(i, _)| i != skip)
        .filter_map(|(i, g)| hooks(g.card_id).gift_end_of_turn.map(|f| (i, g.card_id, f)))
        .collect();
    for (i, gift_id, f) in listed {
        if still_there(&state.board, pos, card_id, i, gift_id) {
            f(state, pos, i, 1, pool, rng);
        }
    }
}

/// Start of turn: the `gift_turn_start` hooks of the gifts of the minions on the board (left to
/// right), then in the hand. Nothing happens without such a gift.
pub fn on_turn_start(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    let board = board_gifts(&state.board, |h| h.gift_turn_start);
    let hand = board_gifts(&state.hand, |h| h.gift_turn_start);
    if board.is_empty() && hand.is_empty() {
        return;
    }
    state.with_deferred_triples(|state| {
        for (pos, card_id, i, gift_id, f) in board {
            if still_there(&state.board, pos, card_id, i, gift_id) {
                f(state, Zone::Board, pos, i, pool, rng);
            }
        }
        for (pos, card_id, i, gift_id, f) in hand {
            if still_there(&state.hand, pos, card_id, i, gift_id) {
                f(state, Zone::Hand, pos, i, pool, rng);
            }
        }
    });
}

/// `true` if one of `unit`'s gifts acts at Start of Combat.
pub fn has_start_of_combat(unit: &Unit) -> bool {
    unit.dark_gifts
        .iter()
        .any(|g| hooks(g.card_id).gift_start_of_combat.is_some())
}

/// A gift's `gift_start_of_combat` hook: `(ctx, unit_id, gift_idx)`.
pub type GiftSocFn = fn(&mut cards::BoardCtx<'_>, UnitId, usize);

/// The `gift_start_of_combat` hooks of `unit`'s gifts, in gift order, with their gift index.
pub fn start_of_combat_hooks(unit: &Unit) -> Vec<(usize, GiftSocFn)> {
    unit.dark_gifts
        .iter()
        .enumerate()
        .filter_map(|(i, g)| hooks(g.card_id).gift_start_of_combat.map(|f| (i, f)))
        .collect()
}

/// `true` if one of `unit`'s gifts has a Deathrattle.
pub fn has_deathrattle(unit: &Unit) -> bool {
    unit.dark_gifts
        .iter()
        .any(|g| hooks(g.card_id).gift_deathrattle.is_some())
}

/// `true` if one of `unit`'s gifts has a Rally.
pub fn has_rally(unit: &Unit) -> bool {
    unit.dark_gifts
        .iter()
        .any(|g| hooks(g.card_id).gift_rally.is_some())
}

/// `dying`'s Reborn copy, as its gifts make it (`gift_reborn`: `Persisting Horror`).
pub fn on_reborn(dying: &Unit, copy: &mut Unit) {
    for gift in &dying.dark_gifts {
        if let Some(reborn) = hooks(gift.card_id).gift_reborn {
            reborn(dying, copy);
        }
    }
}

/// Extra times `unit`'s `Activate` triggers (its gifts' `gift_extra_activations`).
pub fn extra_activations(unit: &Unit) -> u32 {
    unit.dark_gifts
        .iter()
        .filter_map(|g| hooks(g.card_id).gift_extra_activations)
        .map(|f| f(unit))
        .sum()
}

/// A combat starts for `unit` (its combat copy): each gift's `gift_combat_start`.
pub fn combat_start(unit: &mut Unit) {
    for gift in &mut unit.dark_gifts {
        if let Some(start) = hooks(gift.card_id).gift_combat_start {
            start(gift);
        }
    }
}

/// `unit`'s Divine Shield is hit: `true` if one of its gifts absorbs the hit (`gift_absorb_shield_hit`).
pub fn absorb_shield_hit(unit: &mut Unit) -> bool {
    unit.dark_gifts.iter_mut().any(|gift| {
        hooks(gift.card_id)
            .gift_absorb_shield_hit
            .is_some_and(|f| f(gift))
    })
}

/// `true` if `unit` takes no damage while attacking (a gift with
/// [`CardFlags::GIFT_IMMUNE_ATTACKING`]).
pub fn immune_attacking(unit: &Unit) -> bool {
    unit.dark_gifts
        .iter()
        .any(|g| hooks(g.card_id).has(CardFlags::GIFT_IMMUNE_ATTACKING))
}

/// `true` if `unit`'s Spellcraft spell is not temporary (a gift with
/// [`CardFlags::GIFT_KEEPS_SPELLCRAFT`]).
pub fn keeps_spellcraft(unit: &Unit) -> bool {
    unit.dark_gifts
        .iter()
        .any(|g| hooks(g.card_id).has(CardFlags::GIFT_KEEPS_SPELLCRAFT))
}

/// `spell`, the Spellcraft spell `unit` gives, carries `unit`'s gifts that keep it
/// ([`CardFlags::GIFT_KEEPS_SPELLCRAFT`]: `Sunken Persistence`), so it is not temporary.
pub fn mark_spellcraft(unit: &Unit, spell: &mut Unit) {
    let kept = unit
        .dark_gifts
        .iter()
        .filter(|g| hooks(g.card_id).has(CardFlags::GIFT_KEEPS_SPELLCRAFT));
    spell.dark_gifts.extend(kept.copied());
}

/// A gift's "every 2 turns" count (`Replication`, `Affinity`): its `counter` goes up by 1, and
/// `true` (the count starting again) every 2nd time.
pub fn every_two_turns(gift: &mut DarkGift) -> bool {
    gift.counter += 1;
    if gift.counter < 2 {
        return false;
    }
    gift.counter -= 2;
    true
}

/// The multiplier with which `unit` keeps what it gained in combat, from its gifts (0: none).
pub fn keep_combat_mult(unit: &Unit) -> i32 {
    unit.dark_gifts
        .iter()
        .filter_map(|g| hooks(g.card_id).gift_keep_combat_mult)
        .map(|f| f(unit))
        .max()
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------------------------
// Offers (`docs/dark_gifts.md` §4, §5)
// ---------------------------------------------------------------------------------------------

/// The minion Tiers the button offers on turn `turn` (`docs/dark_gifts.md` §4.1, confirmed).
pub fn button_tiers(turn: u32) -> &'static [u32] {
    match turn {
        0..=3 => &[2],
        4 => &[2, 3],
        5 => &[3],
        6 => &[3, 4],
        7 => &[4],
        8 => &[4, 5],
        9 => &[4, 5, 6],
        _ => &[5, 6],
    }
}

/// Which gifts an offer gives (`docs/dark_gifts.md` §4, §12.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GiftPool {
    /// The gifts whose turn window holds the turn.
    Turn,
    /// `Tras'tath`'s pool (`trastath`), whatever their windows.
    Trastath,
}

/// Which options must be of the player's most common type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeRule {
    None,
    /// The first option (the button from turn 6).
    First,
    /// Every option (`Ominous Stone`, `Sinister Invitation`).
    All,
}

/// How an offer is drawn (`docs/dark_gifts.md` §4.3): the button's rules, or a hero's or a
/// trinket's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfferRules {
    /// The turn whose rules apply (the minion filters, the gifts' windows).
    pub turn: u32,
    /// The minion Tiers it offers.
    pub tiers: Vec<u32>,
    pub most_common: TypeRule,
    pub gifts: GiftPool,
    /// Each option is made Golden, without a Triple Reward (`Sinister Invitation`).
    pub golden: bool,
}

impl OfferRules {
    /// The button's offer on turn `turn` (`docs/dark_gifts.md` §4.1).
    pub fn button(turn: u32) -> Self {
        Self {
            turn,
            tiers: button_tiers(turn).to_vec(),
            most_common: if turn >= 6 {
                TypeRule::First
            } else {
                TypeRule::None
            },
            gifts: GiftPool::Turn,
            golden: false,
        }
    }
}

/// What a gift's offer rules see (`gift_offerable`; `docs/dark_gifts.md` §5).
#[derive(Clone, Debug)]
pub struct OfferCtx<'a> {
    /// The game turn whose rules apply.
    pub turn: u32,
    pub auras: &'a PlayerAuras,
    /// The player's Tavern Tier.
    pub tavern_tier: u32,
    /// For each lobby type (`LOBBY_TRIBES` order), whether the lobby has it.
    pub lobby: [bool; LOBBY_TRIBES.len()],
    /// The lowest Tier among the offer's minions (`u32::MAX` while they are being drawn).
    pub lowest_tier: u32,
}

impl OfferCtx<'_> {
    /// `true` if the lobby has the type `tribe` (`docs/dark_gifts.md` §5.1 rule 5).
    pub fn lobby_has(&self, tribe: Tribe) -> bool {
        LOBBY_TRIBES
            .iter()
            .position(|&t| t == tribe)
            .is_some_and(|i| self.lobby[i])
    }
}

/// The context of an offer to the player on `turn`.
pub fn offer_ctx<'a>(state: &'a TavernState, pool: &CardPool, turn: u32) -> OfferCtx<'a> {
    OfferCtx {
        turn,
        auras: &state.auras,
        tavern_tier: state.tavern_tier,
        lobby: LOBBY_TRIBES.map(|t| pool.lobby_has(t)),
        lowest_tier: u32::MAX,
    }
}

// --- What a minion is (`docs/dark_gifts.md` §5.2) ------------------------------------------

/// The minion has a type (an `All` minion has every type).
pub fn has_type(t: &CardTemplate) -> bool {
    t.tribe != Tribe::None
}

/// The minion has a Battlecry.
pub fn is_battlecry(t: &CardTemplate) -> bool {
    t.hooks.battlecry.is_some()
}

/// The minion has a Choose One.
pub fn is_choose_one(t: &CardTemplate) -> bool {
    t.hooks.choose_one.is_some()
}

/// The minion has a Deathrattle.
pub fn is_deathrattle(t: &CardTemplate) -> bool {
    t.hooks.deathrattle.is_some()
}

/// The minion has a Rally.
pub fn is_rally(t: &CardTemplate) -> bool {
    t.hooks.rally.is_some()
}

/// The minion has an `Activate`.
pub fn is_activate(t: &CardTemplate) -> bool {
    t.hooks.activate.is_some()
}

/// The minion has an Avenge.
pub fn is_avenge(t: &CardTemplate) -> bool {
    t.hooks.has(CardFlags::AVENGE)
}

/// The minion has an end of turn effect.
pub fn is_end_of_turn(t: &CardTemplate) -> bool {
    t.hooks.end_of_turn.is_some()
}

/// The minion has a Spellcraft.
pub fn is_spellcraft(t: &CardTemplate) -> bool {
    t.hooks.spellcraft.is_some()
}

/// The minion is meant to stay in the hand (`docs/dark_gifts.md` §4.1 rule 4, engine decision).
fn stays_in_hand(t: &CardTemplate) -> bool {
    let h = &t.hooks;
    h.turn_start_in_hand.is_some()
        || h.after_friendly_play_in_hand.is_some()
        || h.ready_in_hand.is_some()
        || h.combat_copies_from_hand.is_some()
}

/// The minion filters of `docs/dark_gifts.md` §4.1 (rules 1-5) on turn `turn`.
pub fn minion_allowed(turn: u32, t: &CardTemplate) -> bool {
    !(turn < 5 && (is_battlecry(t) || is_choose_one(t)))
        && !t.magnetic
        && t.hooks.sell.is_none()
        && !stays_in_hand(t)
        && !BANNED.contains(&t.name.as_str())
}

/// The highest "most triggered" count of the player (`docs/dark_gifts.md` §5.1 rule 4).
fn most_triggered(auras: &PlayerAuras) -> u32 {
    IMPLEMENTED
        .iter()
        .filter_map(|&(id, _)| hooks(id).gift_trigger_count)
        .map(|f| f(auras))
        .max()
        .unwrap_or(0)
}

/// `true` if the gift `g` can be offered on the minion `t` (`docs/dark_gifts.md` §5).
pub fn offerable(ctx: &OfferCtx<'_>, g: &DarkGiftData, t: &CardTemplate, pool: GiftPool) -> bool {
    if !is_implemented(g.id) {
        return false;
    }
    let in_pool = match pool {
        GiftPool::Turn => g.min_turn <= ctx.turn && g.max_turn.is_none_or(|m| ctx.turn <= m),
        GiftPool::Trastath => g.trastath,
    };
    if !in_pool {
        return false;
    }
    let gift = hooks(g.id);
    if (is_battlecry(t) || is_choose_one(t)) && !gift.has(CardFlags::GIFT_FOR_BATTLECRY) {
        return false;
    }
    if is_deathrattle(t)
        && gift.has(CardFlags::GIFT_GIVES_STATS)
        && !gift.has(CardFlags::GIFT_FOR_DEATHRATTLE)
    {
        return false;
    }
    if let Some(count) = gift.gift_trigger_count {
        let c = count(ctx.auras);
        if c == 0 || c < most_triggered(ctx.auras) {
            return false;
        }
    }
    gift.gift_offerable.is_none_or(|f| f(ctx, t))
}

/// The weight of the gift `g` on the minion `t` (its data weight, times its
/// `gift_weight_factor`).
fn weight(g: &DarkGiftData, t: &CardTemplate) -> u32 {
    let factor = hooks(g.id).gift_weight_factor.map_or(1, |f| f(t));
    g.weight * factor
}

/// `true` if some gift but those in `used` can be offered on `t` (`docs/dark_gifts.md` §4.1
/// rule 6).
fn has_any_gift(ctx: &OfferCtx<'_>, t: &CardTemplate, pool: GiftPool, used: &[CardId]) -> bool {
    DARK_GIFTS
        .iter()
        .any(|g| !used.contains(&g.id) && offerable(ctx, g, t, pool))
}

/// Draw one option's minion (`docs/dark_gifts.md` §4.3 step 2): its Tier (one `rng.below(k)`
/// among the rules' Tiers, then the others from the lowest), of the type `typed` if the pool has
/// one (else like the others), not in `offered`, passing the filters, with a gift not in `used`,
/// of Tier `min_tier` or higher (a replacement doesn't lower the offer's lowest Tier, which
/// `Gilding` reads).
#[allow(clippy::too_many_arguments)]
fn draw_minion(
    ctx: &OfferCtx<'_>,
    pool: &mut CardPool,
    rng: &mut Rng,
    rules: &OfferRules,
    typed: Option<Tribe>,
    offered: &[CardId],
    used: &[CardId],
    min_tier: u32,
) -> Option<Unit> {
    let mut tiers = rules.tiers.clone();
    if tiers.len() > 1 {
        let first = tiers.remove(rng.below(tiers.len()));
        tiers.sort_unstable();
        tiers.insert(0, first);
    }
    let candidate = |t: &CardTemplate, tier: u32, typed: Option<Tribe>| {
        t.tavern_tier == tier
            && tier >= min_tier
            && !offered.contains(&t.card_id)
            && typed.is_none_or(|tribe| t.tribe.matches(tribe))
            && minion_allowed(rules.turn, t)
            && has_any_gift(ctx, t, rules.gifts, used)
    };
    for want in [typed, None] {
        for &tier in &tiers {
            if let Some(unit) = pool.draw_template_where(|t| candidate(t, tier, want), rng) {
                return Some(unit);
            }
        }
        if want.is_none() {
            break;
        }
    }
    None
}

/// Draw one gift for the minion `t` by weight among those it can be offered but `used`
/// (`docs/dark_gifts.md` §4.3 step 3): one `rng.below(total weight)`, or `None` without any.
fn draw_gift(
    ctx: &OfferCtx<'_>,
    t: &CardTemplate,
    pool: GiftPool,
    used: &[CardId],
    rng: &mut Rng,
) -> Option<CardId> {
    let gifts: Vec<(CardId, u32)> = DARK_GIFTS
        .iter()
        .filter(|g| !used.contains(&g.id) && offerable(ctx, g, t, pool))
        .map(|g| (g.id, weight(g, t)))
        .collect();
    let total: u32 = gifts.iter().map(|&(_, w)| w).sum();
    if total == 0 {
        return None;
    }
    let mut pick = rng.below(total as usize) as u32;
    for &(id, w) in &gifts {
        if pick < w {
            return Some(id);
        }
        pick -= w;
    }
    None
}

/// Draw an offer for the player (`docs/dark_gifts.md` §4.3): up to 3 minions from the pool, each
/// with a different gift. The minions are taken out of the pool (the unchosen ones go back when
/// the Discover resolves); each option has the player's "wherever this is" auras.
pub fn draw_offer(
    state: &TavernState,
    pool: &mut CardPool,
    rng: &mut Rng,
    rules: &OfferRules,
) -> Vec<Unit> {
    let mut ctx = offer_ctx(state, pool, rules.turn);
    let tribe = match rules.most_common {
        TypeRule::None => None,
        _ => Some(spells::most_common_tribe(&state.board, rng)),
    };
    let typed_for = |k: usize| match rules.most_common {
        TypeRule::None => None,
        TypeRule::First => tribe.filter(|_| k == 0),
        TypeRule::All => tribe,
    };
    // 1. The minions.
    let mut picked: Vec<Unit> = Vec::with_capacity(OFFER_COUNT);
    for k in 0..OFFER_COUNT {
        let offered: Vec<CardId> = picked.iter().map(|u| u.card_id).collect();
        match draw_minion(&ctx, pool, rng, rules, typed_for(k), &offered, &[], 0) {
            Some(unit) => picked.push(unit),
            None => break,
        }
    }
    // 2. A gift for each, in option order; an option no gift is left for is replaced.
    ctx.lowest_tier = picked.iter().map(|u| u.tavern_tier).min().unwrap_or(0);
    let mut used: Vec<CardId> = Vec::with_capacity(OFFER_COUNT);
    let mut options: Vec<Unit> = Vec::with_capacity(picked.len());
    let mut tried: Vec<CardId> = picked.iter().map(|u| u.card_id).collect();
    for (k, first) in picked.into_iter().enumerate() {
        let mut unit = first;
        let chosen = loop {
            let gift = cards::template(unit.card_id)
                .and_then(|t| draw_gift(&ctx, t, rules.gifts, &used, rng));
            if let Some(gift) = gift {
                break Some(gift);
            }
            pool.return_unit(&unit);
            let lowest = ctx.lowest_tier;
            match draw_minion(&ctx, pool, rng, rules, typed_for(k), &tried, &used, lowest) {
                Some(replacement) => {
                    tried.push(replacement.card_id);
                    unit = replacement;
                }
                None => break None,
            }
        };
        let Some(chosen) = chosen else {
            continue;
        };
        used.push(chosen);
        if rules.golden {
            make_intrinsic_golden(&mut unit);
        }
        give(&mut unit, chosen, rules.turn);
        state.apply_global_unit_auras(&mut unit);
        options.push(unit);
    }
    options
}

/// Make `unit` Golden without a Triple Reward (`Gilding`, `Sinister Invitation`): an intrinsic
/// Golden (`Unit::make_golden`, `intrinsic_golden`).
pub fn make_intrinsic_golden(unit: &mut Unit) {
    unit.make_golden();
    unit.intrinsic_golden = true;
}

// ---------------------------------------------------------------------------------------------
// The button (`docs/dark_gifts.md` §3)
// ---------------------------------------------------------------------------------------------

/// `true` if the button can be used now, prompts aside (`docs/dark_gifts.md` §3.2).
pub fn can_use_button(state: &TavernState) -> bool {
    state.dark_gifts_enabled
        && state.turn >= UNLOCK_TURN
        && !state.dark_discovery_used_this_turn
        && state.dark_discoveries_used < USES_PER_GAME
        && state.gold >= PRICE
}

/// The button's uses left this game (0 without Dark Gifts).
pub fn uses_left(state: &TavernState) -> u32 {
    if state.dark_gifts_enabled {
        USES_PER_GAME.saturating_sub(state.dark_discoveries_used)
    } else {
        0
    }
}

/// Use the button (legal; `docs/dark_gifts.md` §3.2): pay 3 Gold, count the use, and Discover
/// one of the offer's minions.
pub fn use_button(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    state.spend_gold(PRICE, pool, rng);
    state.dark_discovery_used_this_turn = true;
    state.dark_discoveries_used += 1;
    let options = draw_offer(state, pool, rng, &OfferRules::button(state.turn));
    state.push_discover(options);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// `(file stem, source)` of every gift file.
    fn gift_files() -> Vec<(String, String)> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/dark_gifts");
        let mut files: Vec<(String, String)> = fs::read_dir(&dir)
            .expect("src/dark_gifts")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.extension().is_some_and(|e| e == "rs"))
            .map(|p| {
                let stem = p.file_stem().unwrap().to_string_lossy().into_owned();
                (stem, fs::read_to_string(&p).expect("readable"))
            })
            .filter(|(stem, _)| !["mod", "table", "vocab"].contains(&stem.as_str()))
            .collect();
        files.sort();
        files
    }

    /// The header a gift file must start with (`docs/dark_gifts.md` §9).
    fn expected_header(g: &DarkGiftData) -> String {
        let min = if g.min_turn == 0 {
            String::new()
        } else {
            g.min_turn.to_string()
        };
        let max = g.max_turn.map_or(String::new(), |m| m.to_string());
        format!(
            "//! `{}` (`{}`) — turns {min}–{max}, weight {}.\n//!\n//! {}\n",
            display_name(g),
            g.card_id,
            g.weight,
            g.text
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
    fn every_gift_file_matches_the_table() {
        let files = gift_files();
        assert_eq!(
            files.len(),
            IMPLEMENTED.len(),
            "every gift file is in IMPLEMENTED"
        );
        let scenarios = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/dark_gifts");
        for (stem, src) in &files {
            let id = constant(src, "ID").unwrap_or_else(|| panic!("{stem}.rs: no `pub const ID`"));
            let g = data(id).unwrap_or_else(|| panic!("{stem}.rs: ID {id} is not in the table"));
            assert_eq!(slug(g), *stem, "{stem}.rs: file name is the gift's slug");
            let header = expected_header(g);
            assert!(
                src.starts_with(&header),
                "{stem}.rs: the header doesn't match the table; expected:\n{header}"
            );
            assert!(is_implemented(id), "{stem}.rs: not in IMPLEMENTED");
            assert!(
                behaviors().iter().any(|&(card, _)| card == id),
                "{stem}.rs: the gift has no hooks"
            );
            let scenario = fs::read_to_string(scenarios.join(format!("{stem}.yaml")))
                .unwrap_or_else(|_| panic!("{stem}.rs: no tests/scenarios/dark_gifts/{stem}.yaml"));
            assert!(
                scenario.starts_with(&format!("dark_gift: {}\n", display_name(g))),
                "{stem}.yaml: starts with `dark_gift: {}`",
                display_name(g)
            );
        }
    }

    #[test]
    fn the_table_has_every_gift() {
        assert_eq!(DARK_GIFTS.len(), 47);
        let mut names: Vec<&str> = DARK_GIFTS.iter().map(|g| g.name).collect();
        names.dedup();
        assert_eq!(names.len(), 43);
        for g in DARK_GIFTS {
            assert!(g.id > 11_000, "{}: gift ids don't collide", g.name);
            assert!(crate::trinkets::data(g.id).is_none());
            assert!(!g.enchantments.is_empty(), "{}: has an enchantment", g.name);
            assert_eq!(by_name(&display_name(g)).map(|x| x.id), Some(g.id));
            assert_eq!(by_name(&slug(g)).map(|x| x.id), Some(g.id));
        }
        let late = by_name("Battle Scars (late)").expect("Battle Scars (late)");
        assert_eq!((late.min_turn, late.max_turn), (7, Some(10)));
        assert_eq!(slug(late), "battle_scars_late");
        assert_eq!(
            by_enchantment("BG36_MidGameEffect_000t9e2").map(|g| g.name),
            Some("Admiration")
        );
        let fortitude = by_card_id("BG36_MidGameEffect_000t73").expect("Fortitude");
        assert_eq!((fortitude.min_turn, fortitude.max_turn), (3, Some(3)));
    }

    /// The RL vocabulary keeps the first table's order and every gift of the table
    /// (`docs/dark_gifts.md` §8.2).
    #[test]
    fn the_vocabulary_is_append_only() {
        let ids = vocabulary();
        assert_eq!(ids[..vocab::VOCAB_IDS.len()], vocab::VOCAB_IDS);
        assert!(DARK_GIFTS.iter().all(|g| ids.contains(&g.id)));
        assert_eq!(ids.len(), 47);
    }

    /// Over random boards, turns and seeds, every offer follows the rules
    /// (`docs/dark_gifts.md` §4, §5): at most 3 options, different minions and different gifts,
    /// the turn's Tiers, the minion filters, each gift offerable on its minion, `Gilding` on the
    /// lowest Tier, and from turn 6 an option of the player's most common type.
    #[test]
    fn offers_follow_the_rules() {
        let tribes = [
            Tribe::Beast,
            Tribe::Demon,
            Tribe::Mech,
            Tribe::Murloc,
            Tribe::Quilboar,
            Tribe::Undead,
        ];
        let mut rng = Rng::new(36);
        for game in 0..600 {
            let mut pool = CardPool::new(cards::full_catalog());
            let mut state = TavernState::new();
            state.turn = 3 + game % 10;
            state.tavern_tier = 1 + rng.below(6) as u32;
            let main = tribes[rng.below(tribes.len())];
            for i in 0..rng.below(8) {
                let tribe = if i % 2 == 0 {
                    main
                } else {
                    tribes[rng.below(tribes.len())]
                };
                state.board.push(Unit::new("m", 1, 1).with_tribe(tribe));
            }
            state.auras.battlecries_triggered = rng.below(3) as u32;
            state.auras.deathrattles_triggered = rng.below(3) as u32;
            state.auras.tavern_spells_cast = rng.below(3) as u32;
            let rules = OfferRules::button(state.turn);
            let offer = draw_offer(&state, &mut pool, &mut rng, &rules);
            assert!(offer.len() <= OFFER_COUNT);
            let mut ctx = offer_ctx(&state, &pool, state.turn);
            ctx.lowest_tier = offer.iter().map(|u| u.tavern_tier).min().unwrap_or(0);
            for (i, unit) in offer.iter().enumerate() {
                let t = cards::template(unit.card_id).expect("a catalog minion");
                assert!(
                    rules.tiers.contains(&unit.tavern_tier),
                    "turn {}: Tier {}",
                    state.turn,
                    unit.tavern_tier
                );
                assert!(minion_allowed(state.turn, t), "{} can't be offered", t.name);
                assert_eq!(unit.dark_gifts.len(), 1, "{}: one gift", t.name);
                let gift = data(unit.dark_gifts[0].card_id).expect("a gift");
                assert!(
                    offerable(&ctx, gift, t, GiftPool::Turn),
                    "{} on {}",
                    gift.name,
                    t.name
                );
                for other in &offer[..i] {
                    assert_ne!(other.card_id, unit.card_id, "different minions");
                    assert_ne!(other.dark_gifts[0].card_id, gift.id, "different gifts");
                }
            }
            let distinct_main =
                state.board.iter().filter(|u| u.tribe == main).count() > state.board.len() / 2;
            if state.turn >= 6 && distinct_main && !offer.is_empty() {
                assert!(
                    offer.iter().any(|u| u.tribe.matches(main)),
                    "turn {}: an option of the most common type {main:?}",
                    state.turn
                );
            }
        }
    }

    #[test]
    fn the_minion_filters_hold() {
        let by_name = |name: &str| {
            cards::full_catalog()
                .into_iter()
                .find(|t| t.name == name)
                .unwrap_or_else(|| panic!("{name}"))
        };
        // No Battlecry before turn 5; no "when you sell this"; banned minions.
        assert!(!minion_allowed(4, &by_name("Forest Rover")));
        assert!(minion_allowed(5, &by_name("Forest Rover")));
        assert!(!minion_allowed(9, &by_name("Patient Scout")));
        assert!(!minion_allowed(9, &by_name("Deadly Spore")));
        assert!(!minion_allowed(9, &by_name("Bream Counter")));
        assert!(minion_allowed(3, &by_name("Harmless Bonehead")));
        // What the gifts' rules read.
        assert!(is_avenge(&by_name("Relentless Deflector")));
        assert!(!is_avenge(&by_name("Harmless Bonehead")));
        assert!(is_deathrattle(&by_name("Harmless Bonehead")));
        assert!(is_battlecry(&by_name("Forest Rover")));
    }

    #[test]
    fn button_tiers_follow_the_table() {
        let expected: [(u32, &[u32]); 9] = [
            (3, &[2]),
            (4, &[2, 3]),
            (5, &[3]),
            (6, &[3, 4]),
            (7, &[4]),
            (8, &[4, 5]),
            (9, &[4, 5, 6]),
            (10, &[5, 6]),
            (15, &[5, 6]),
        ];
        for (turn, tiers) in expected {
            assert_eq!(button_tiers(turn), tiers, "turn {turn}");
        }
    }
}
