//! Trinkets (`docs/trinkets.md`).
//!
//! The trinket pool is the generated [`TRINKETS`] table (`table.rs`, written by
//! `tools/gen_trinkets.py` from `CardDefs.xml` and HearthstoneJSON). Each implemented trinket has
//! its own file (`src/trinkets/<slug>.rs`) whose header names the trinket, its kind, price, types
//! and text (checked against the table by the tests below), and which defines its [`CardHooks`].
//! A trinket is a card with hooks (`docs/trinkets.md` §6): the Tavern and Combat engines reach it
//! only through the functions here, which look its hooks up by its card id (its dbfId); they never
//! name a trinket.

mod table;

pub mod all_purpose_kibble;
pub mod artisanal_urn;
pub mod azeroth_model_globe;
pub mod baleful_incense;
pub mod baller_portrait;
pub mod baller_portrait_greater;
pub mod bartend_o_trons_oilcan;
pub mod beatboxer_portrait;
pub mod beetle_band;
pub mod beetle_band_greater;
pub mod bleeding_heart;
pub mod blood_golem_sticker;
pub mod bluegill_flippers;
pub mod bobs_tip_jar;
pub mod booty_bay_brew;
pub mod booty_bay_brew_greater;
pub mod bronze_timepiece;
pub mod butchers_sickle;
pub mod comfy_coffin;
pub mod comfy_coffin_greater;
pub mod conch_portrait;
pub mod converter_portrait;
pub mod cookies_stirring_rod;
pub mod corrupted_baton;
pub mod corrupted_baton_greater;
pub mod corrupted_coin_portrait;
pub mod dalaran_cheese_wheel;
pub mod defensive_shell;
pub mod demonic_bloodletter;
pub mod dramaloc_sticker;
pub mod emergency_gearblade;
pub mod enigmatic_headstone;
pub mod eternal_portrait;
pub mod evil_experiment;
pub mod explorers_binoculars;
pub mod faerie_dragon_scale;
pub mod fish_portrait;
pub mod flighty_portrait;
pub mod fridge_magnet;
pub mod frigid_blossom;
pub mod ghastly_sticker;
pub mod glass_of_perspective;
pub mod glass_of_perspective_greater;
pub mod goblin_wallet;
pub mod gold_pendant;
pub mod great_boar_sticker;
pub mod great_boar_sticker_greater;
pub mod herald_sticker;
pub mod herding_horn;
pub mod hoggy_bank;
pub mod holy_mallet;
pub mod innkeepers_hearth;
pub mod innkeepers_stein;
pub mod ironforge_anvil;
pub mod jar_o_gems;
pub mod lava_lamp;
pub mod lightfeather_sticker;
pub mod lionfish_portrait;
pub mod lucky_tabby;
pub mod magicians_top_hat;
pub mod mama_bear_sticker;
pub mod miniature_ship;
pub mod nerglish_phrasebook;
pub mod nerglish_phrasebook_greater;
pub mod nomi_sticker;
pub mod nomi_sticker_greater;
pub mod orc_estra_portrait;
pub mod pocket_cyclone;
pub mod pocket_cyclone_greater;
pub mod protective_ring;
pub mod quilligraphy_set;
pub mod quilligraphy_set_greater;
pub mod rascal_sticker;
pub mod recycling_sticker;
pub mod reflective_pendant;
pub mod rendle_sticker;
pub mod rivendare_portrait;
pub mod rockin_music_box;
pub mod scraper_sticker;
pub mod secret_schematic;
pub mod sellemental_portrait;
pub mod slamma_sticker;
pub mod spell_powered_wrench;
pub mod sphere_of_memory;
pub mod staff_of_the_scourge;
pub mod stormcoil_sticker;
pub mod stuffed_coin_purse;
pub mod summoning_sphere;
pub mod the_eye_of_dalaran;
pub mod training_certificate;
pub mod transcribing_typewriter;
pub mod transcribing_typewriter_greater;
pub mod tricksters_sleeve;
pub mod upstart_embers;
pub mod volumizer_portrait;
pub mod volumizer_portrait_greater;
pub mod water_wheel;
pub mod weighted_gauntlet;
pub mod wicked_tome;
pub mod willbreaker_sticker;
pub mod worn_treasure_map;

pub use table::{CARD_DEFS_BUILD, TRINKETS};

use crate::cards::{self, hooks, BoardCtx, CardHooks, HeroCombatCtx, Passive, Played};
use crate::model::{
    CardId, PlayerAuras, Tribe, TrinketOption, TrinketState, Unit, UnitId, LOBBY_TRIBES,
};
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, RefreshKind, TavernState};

/// A trinket of the pool (`docs/trinkets.md` §2.1): one row of the generated table.
#[derive(Clone, Copy, Debug)]
pub struct TrinketData {
    /// Its dbfId: the trinket's engine `CardId`.
    pub id: CardId,
    pub card_id: &'static str,
    pub name: &'static str,
    /// A Greater trinket (else Lesser).
    pub greater: bool,
    /// Its price in Gold.
    pub cost: u32,
    /// The minion types it is associated with (none, one or two).
    pub tribes: &'static [Tribe],
    /// Only in Duos.
    pub duos_only: bool,
    pub text: &'static str,
}

/// The turn of the Lesser offer (`docs/trinkets.md` §3.1, confirmed).
pub const LESSER_TURN: u32 = 6;
/// The turn of the Greater offer (`docs/trinkets.md` §3.1, confirmed).
pub const GREATER_TURN: u32 = 9;
/// Options in an offer (`docs/trinkets.md` §3.2, confirmed).
pub const OFFER_COUNT: usize = 4;
/// An option this cheap or cheaper is guaranteed (`docs/trinkets.md` §3.2, confirmed).
pub const CHEAP_COST: u32 = 2;
/// What an option of a type that isn't the player's most common one costs less (confirmed).
pub const OFF_TYPE_DISCOUNT: u32 = 2;

/// An implemented trinket: its id and its cards' hooks.
type Implemented = (CardId, fn() -> Vec<(CardId, CardHooks)>);

/// Every implemented trinket, by file name.
const IMPLEMENTED: &[Implemented] = &[
    (all_purpose_kibble::ID, all_purpose_kibble::behaviors),
    (artisanal_urn::ID, artisanal_urn::behaviors),
    (azeroth_model_globe::ID, azeroth_model_globe::behaviors),
    (baleful_incense::ID, baleful_incense::behaviors),
    (baller_portrait::ID, baller_portrait::behaviors),
    (
        baller_portrait_greater::ID,
        baller_portrait_greater::behaviors,
    ),
    (
        bartend_o_trons_oilcan::ID,
        bartend_o_trons_oilcan::behaviors,
    ),
    (beatboxer_portrait::ID, beatboxer_portrait::behaviors),
    (beetle_band::ID, beetle_band::behaviors),
    (beetle_band_greater::ID, beetle_band_greater::behaviors),
    (bleeding_heart::ID, bleeding_heart::behaviors),
    (blood_golem_sticker::ID, blood_golem_sticker::behaviors),
    (bluegill_flippers::ID, bluegill_flippers::behaviors),
    (bobs_tip_jar::ID, bobs_tip_jar::behaviors),
    (booty_bay_brew::ID, booty_bay_brew::behaviors),
    (
        booty_bay_brew_greater::ID,
        booty_bay_brew_greater::behaviors,
    ),
    (bronze_timepiece::ID, bronze_timepiece::behaviors),
    (butchers_sickle::ID, butchers_sickle::behaviors),
    (comfy_coffin::ID, comfy_coffin::behaviors),
    (comfy_coffin_greater::ID, comfy_coffin_greater::behaviors),
    (conch_portrait::ID, conch_portrait::behaviors),
    (converter_portrait::ID, converter_portrait::behaviors),
    (cookies_stirring_rod::ID, cookies_stirring_rod::behaviors),
    (corrupted_baton::ID, corrupted_baton::behaviors),
    (
        corrupted_baton_greater::ID,
        corrupted_baton_greater::behaviors,
    ),
    (
        corrupted_coin_portrait::ID,
        corrupted_coin_portrait::behaviors,
    ),
    (dalaran_cheese_wheel::ID, dalaran_cheese_wheel::behaviors),
    (defensive_shell::ID, defensive_shell::behaviors),
    (demonic_bloodletter::ID, demonic_bloodletter::behaviors),
    (dramaloc_sticker::ID, dramaloc_sticker::behaviors),
    (emergency_gearblade::ID, emergency_gearblade::behaviors),
    (enigmatic_headstone::ID, enigmatic_headstone::behaviors),
    (eternal_portrait::ID, eternal_portrait::behaviors),
    (evil_experiment::ID, evil_experiment::behaviors),
    (explorers_binoculars::ID, explorers_binoculars::behaviors),
    (faerie_dragon_scale::ID, faerie_dragon_scale::behaviors),
    (fish_portrait::ID, fish_portrait::behaviors),
    (flighty_portrait::ID, flighty_portrait::behaviors),
    (fridge_magnet::ID, fridge_magnet::behaviors),
    (frigid_blossom::ID, frigid_blossom::behaviors),
    (ghastly_sticker::ID, ghastly_sticker::behaviors),
    (glass_of_perspective::ID, glass_of_perspective::behaviors),
    (
        glass_of_perspective_greater::ID,
        glass_of_perspective_greater::behaviors,
    ),
    (goblin_wallet::ID, goblin_wallet::behaviors),
    (gold_pendant::ID, gold_pendant::behaviors),
    (great_boar_sticker::ID, great_boar_sticker::behaviors),
    (
        great_boar_sticker_greater::ID,
        great_boar_sticker_greater::behaviors,
    ),
    (herald_sticker::ID, herald_sticker::behaviors),
    (herding_horn::ID, herding_horn::behaviors),
    (hoggy_bank::ID, hoggy_bank::behaviors),
    (holy_mallet::ID, holy_mallet::behaviors),
    (innkeepers_hearth::ID, innkeepers_hearth::behaviors),
    (innkeepers_stein::ID, innkeepers_stein::behaviors),
    (ironforge_anvil::ID, ironforge_anvil::behaviors),
    (jar_o_gems::ID, jar_o_gems::behaviors),
    (lava_lamp::ID, lava_lamp::behaviors),
    (lightfeather_sticker::ID, lightfeather_sticker::behaviors),
    (lionfish_portrait::ID, lionfish_portrait::behaviors),
    (lucky_tabby::ID, lucky_tabby::behaviors),
    (magicians_top_hat::ID, magicians_top_hat::behaviors),
    (mama_bear_sticker::ID, mama_bear_sticker::behaviors),
    (miniature_ship::ID, miniature_ship::behaviors),
    (nerglish_phrasebook::ID, nerglish_phrasebook::behaviors),
    (
        nerglish_phrasebook_greater::ID,
        nerglish_phrasebook_greater::behaviors,
    ),
    (nomi_sticker::ID, nomi_sticker::behaviors),
    (nomi_sticker_greater::ID, nomi_sticker_greater::behaviors),
    (orc_estra_portrait::ID, orc_estra_portrait::behaviors),
    (pocket_cyclone::ID, pocket_cyclone::behaviors),
    (
        pocket_cyclone_greater::ID,
        pocket_cyclone_greater::behaviors,
    ),
    (protective_ring::ID, protective_ring::behaviors),
    (quilligraphy_set::ID, quilligraphy_set::behaviors),
    (
        quilligraphy_set_greater::ID,
        quilligraphy_set_greater::behaviors,
    ),
    (rascal_sticker::ID, rascal_sticker::behaviors),
    (recycling_sticker::ID, recycling_sticker::behaviors),
    (reflective_pendant::ID, reflective_pendant::behaviors),
    (rendle_sticker::ID, rendle_sticker::behaviors),
    (rivendare_portrait::ID, rivendare_portrait::behaviors),
    (rockin_music_box::ID, rockin_music_box::behaviors),
    (scraper_sticker::ID, scraper_sticker::behaviors),
    (secret_schematic::ID, secret_schematic::behaviors),
    (sellemental_portrait::ID, sellemental_portrait::behaviors),
    (slamma_sticker::ID, slamma_sticker::behaviors),
    (spell_powered_wrench::ID, spell_powered_wrench::behaviors),
    (sphere_of_memory::ID, sphere_of_memory::behaviors),
    (staff_of_the_scourge::ID, staff_of_the_scourge::behaviors),
    (stormcoil_sticker::ID, stormcoil_sticker::behaviors),
    (stuffed_coin_purse::ID, stuffed_coin_purse::behaviors),
    (summoning_sphere::ID, summoning_sphere::behaviors),
    (the_eye_of_dalaran::ID, the_eye_of_dalaran::behaviors),
    (training_certificate::ID, training_certificate::behaviors),
    (
        transcribing_typewriter::ID,
        transcribing_typewriter::behaviors,
    ),
    (
        transcribing_typewriter_greater::ID,
        transcribing_typewriter_greater::behaviors,
    ),
    (tricksters_sleeve::ID, tricksters_sleeve::behaviors),
    (upstart_embers::ID, upstart_embers::behaviors),
    (volumizer_portrait::ID, volumizer_portrait::behaviors),
    (
        volumizer_portrait_greater::ID,
        volumizer_portrait_greater::behaviors,
    ),
    (water_wheel::ID, water_wheel::behaviors),
    (weighted_gauntlet::ID, weighted_gauntlet::behaviors),
    (wicked_tome::ID, wicked_tome::behaviors),
    (willbreaker_sticker::ID, willbreaker_sticker::behaviors),
    (worn_treasure_map::ID, worn_treasure_map::behaviors),
];

/// The hooks of every implemented trinket's cards, for the card registry.
pub fn behaviors() -> Vec<(CardId, CardHooks)> {
    IMPLEMENTED.iter().flat_map(|(_, cards)| cards()).collect()
}

/// The trinket pool, sorted by name.
pub fn table() -> &'static [TrinketData] {
    TRINKETS
}

/// The trinket `id`.
pub fn data(id: CardId) -> Option<&'static TrinketData> {
    TRINKETS.iter().find(|t| t.id == id)
}

/// The trinket whose card ID is `card_id`.
pub fn by_card_id(card_id: &str) -> Option<&'static TrinketData> {
    TRINKETS.iter().find(|t| t.card_id == card_id)
}

/// `true` if another trinket of the table has `t`'s name (the Lesser and Greater `Eternal
/// Portrait`).
fn name_is_shared(t: &TrinketData) -> bool {
    TRINKETS.iter().any(|o| o.id != t.id && o.name == t.name)
}

/// The name scenarios and headers use for `t`: its name, with ` (Greater)` for a Greater trinket
/// that shares its name with another (`docs/trinkets.md` §8.3).
pub fn display_name(t: &TrinketData) -> String {
    if t.greater && name_is_shared(t) {
        format!("{} (Greater)", t.name)
    } else {
        t.name.to_string()
    }
}

/// The file stem of `t` (`docs/trinkets.md` §9): its name's slug, with `_greater` for a Greater
/// trinket that shares its name with another.
pub fn slug(t: &TrinketData) -> String {
    let base = crate::scenario::slug(t.name);
    if t.greater && name_is_shared(t) {
        format!("{base}_greater")
    } else {
        base
    }
}

/// The trinket named `name` (case-insensitive): its display name, its name (the Lesser one of a
/// shared name first), its slug, or its card ID.
pub fn by_name(name: &str) -> Option<&'static TrinketData> {
    let wanted = name.trim();
    TRINKETS
        .iter()
        .find(|t| display_name(t).eq_ignore_ascii_case(wanted) || slug(t) == wanted)
        .or_else(|| {
            TRINKETS
                .iter()
                .filter(|t| t.name.eq_ignore_ascii_case(wanted))
                .min_by_key(|t| t.greater)
        })
        .or_else(|| by_card_id(wanted))
}

/// `true` if the engine implements the trinket `id`.
pub fn is_implemented(id: CardId) -> bool {
    IMPLEMENTED.iter().any(|&(t, _)| t == id)
}

/// The trinkets a game can offer (`docs/trinkets.md` §3.1): the implemented ones, not Naga,
/// Duos-only ones only in Duos, in table order.
pub fn offerable(duos: bool) -> impl Iterator<Item = &'static TrinketData> {
    TRINKETS.iter().filter(move |t| {
        is_implemented(t.id) && (duos || !t.duos_only) && !t.tribes.contains(&Tribe::Naga)
    })
}

/// A fresh state for the trinket `id`.
pub fn new_state(id: CardId) -> TrinketState {
    TrinketState {
        card_id: id,
        ..TrinketState::default()
    }
}

/// `true` if the player owns the trinket `id`.
pub fn owns(auras: &PlayerAuras, id: CardId) -> bool {
    auras.trinkets.iter().any(|t| t.card_id == id)
}

/// The player's state of the trinket `id` (a player never owns two copies of a trinket, so a
/// trinket's hooks find their state by its id: `docs/trinkets.md` §2.2).
pub fn state_mut(auras: &mut PlayerAuras, id: CardId) -> Option<&mut TrinketState> {
    auras.trinkets.iter_mut().find(|t| t.card_id == id)
}

/// The player's state of the trinket `id`.
pub fn state(auras: &PlayerAuras, id: CardId) -> Option<&TrinketState> {
    auras.trinkets.iter().find(|t| t.card_id == id)
}

// ---------------------------------------------------------------------------------------------
// Offers
// ---------------------------------------------------------------------------------------------

/// For each lobby type (`LOBBY_TRIBES` order), the minions on the board and in the hand that have
/// it (`docs/trinkets.md` §3.2 rule 3): a two-type minion counts for both, an `All` minion for
/// every type, a spell for none.
pub fn type_counts(state: &TavernState) -> [u32; LOBBY_TRIBES.len()] {
    let mut counts = [0; LOBBY_TRIBES.len()];
    for unit in state
        .board
        .iter()
        .chain(&state.hand)
        .filter(|u| !u.is_spell)
    {
        for (i, &t) in LOBBY_TRIBES.iter().enumerate() {
            if unit.tribe.has_single(t) {
                counts[i] += 1;
            }
        }
    }
    counts
}

/// How an offer candidate is drawn (`docs/trinkets.md` §3.3 step 2).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OfferClass {
    /// No type.
    Neutral,
    /// A type that is one of the player's most common types.
    Main,
    /// No most common type, but an eligible one: 2 Gold cheaper.
    Off,
}

/// The offer candidates of a Lesser (`greater == false`) or Greater offer for the player
/// (`docs/trinkets.md` §3.3 steps 1-2), in table order: each offerable trinket of that kind they
/// don't own, its class and its price.
pub fn candidates(state: &TavernState, greater: bool) -> Vec<(CardId, OfferClass, u32)> {
    candidates_in(state, greater, offerable(state.auras.duos_game))
}

/// [`candidates`] among the trinkets of `pool` (in its order).
fn candidates_in<'a>(
    state: &TavernState,
    greater: bool,
    pool: impl Iterator<Item = &'a TrinketData>,
) -> Vec<(CardId, OfferClass, u32)> {
    let counts = type_counts(state);
    let threshold = if greater { 3 } else { 2 };
    let best = counts.iter().copied().max().unwrap_or(0);
    let count_of = |t: Tribe| {
        LOBBY_TRIBES
            .iter()
            .position(|&l| l == t)
            .map_or(0, |i| counts[i])
    };
    pool.filter(|t| t.greater == greater && !owns(&state.auras, t.id))
        .filter_map(|t| {
            if t.tribes.is_empty() {
                return Some((t.id, OfferClass::Neutral, t.cost));
            }
            let eligible = |&&tribe: &&Tribe| count_of(tribe) >= threshold;
            if !t.tribes.iter().any(|tribe| eligible(&tribe)) {
                return None;
            }
            if t.tribes.iter().any(|&tribe| count_of(tribe) == best) {
                Some((t.id, OfferClass::Main, t.cost))
            } else {
                Some((
                    t.id,
                    OfferClass::Off,
                    t.cost.saturating_sub(OFF_TYPE_DISCOUNT),
                ))
            }
        })
        .collect()
}

/// Draw an offer for the player (`docs/trinkets.md` §3.3): a Neutral option, then a cheap one
/// if none is yet, then the rest up to [`OFFER_COUNT`] (at most one Off option), each a uniform
/// `rng.below(k)` over what is left; then a Fisher-Yates shuffle.
pub fn draw_offer(state: &TavernState, greater: bool, rng: &mut Rng) -> Vec<TrinketOption> {
    draw_from(&candidates(state, greater), rng)
}

/// The draw of [`draw_offer`] among `candidates`.
fn draw_from(candidates: &[(CardId, OfferClass, u32)], rng: &mut Rng) -> Vec<TrinketOption> {
    let mut drawn: Vec<usize> = Vec::with_capacity(OFFER_COUNT);
    let mut draw = |drawn: &mut Vec<usize>, keep: &dyn Fn(&(CardId, OfferClass, u32)) -> bool| {
        let list: Vec<usize> = (0..candidates.len())
            .filter(|i| !drawn.contains(i) && keep(&candidates[*i]))
            .collect();
        if !list.is_empty() {
            drawn.push(list[rng.below(list.len())]);
        }
    };
    draw(&mut drawn, &|c| c.1 == OfferClass::Neutral);
    if !drawn.iter().any(|&i| candidates[i].2 <= CHEAP_COST) {
        draw(&mut drawn, &|c| c.2 <= CHEAP_COST);
    }
    while drawn.len() < OFFER_COUNT {
        let before = drawn.len();
        let off_drawn = drawn.iter().any(|&i| candidates[i].1 == OfferClass::Off);
        draw(&mut drawn, &|c| !(off_drawn && c.1 == OfferClass::Off));
        if drawn.len() == before {
            break;
        }
    }
    for i in (1..drawn.len()).rev() {
        let j = rng.below(i + 1);
        drawn.swap(i, j);
    }
    drawn
        .into_iter()
        .map(|i| TrinketOption {
            card_id: candidates[i].0,
            cost: candidates[i].2,
        })
        .collect()
}

/// The start of turn's offer (`docs/trinkets.md` §3.1): on turn 6 a Lesser one, on turn 9 a
/// Greater one, for a player with offers enabled. Nothing else happens, and nothing is drawn.
pub fn offer_at_turn_start(state: &mut TavernState, rng: &mut Rng) {
    if !state.trinkets_enabled {
        return;
    }
    let greater = match state.turn {
        LESSER_TURN => false,
        GREATER_TURN => true,
        _ => return,
    };
    state.trinket_options = draw_offer(state, greater, rng);
}

/// The Gold the offered option `option_index` costs the player now (`docs/trinkets.md` §4), or
/// `None` if they can't pick it: its price if they have it; if no option is affordable, nothing
/// for the cheapest ones.
pub fn option_price(state: &TavernState, option_index: usize) -> Option<u32> {
    let option = state.trinket_options.get(option_index)?;
    if state.gold >= option.cost {
        return Some(option.cost);
    }
    let affordable = state.trinket_options.iter().any(|o| state.gold >= o.cost);
    let cheapest = state.trinket_options.iter().map(|o| o.cost).min();
    (!affordable && cheapest == Some(option.cost)).then_some(0)
}

/// Pick the offered option `option_index` (legal, `docs/trinkets.md` §4): the offer clears, the
/// price is paid (Gold spent), then the trinket joins and its `trinket_bought` text resolves.
pub fn choose(state: &mut TavernState, option_index: usize, pool: &mut CardPool, rng: &mut Rng) {
    let Some(price) = option_price(state, option_index) else {
        return;
    };
    let id = state.trinket_options[option_index].card_id;
    state.trinket_options.clear();
    state.spend_gold(price, pool, rng);
    give(state, id, pool, rng);
}

/// Give the player the trinket `id` (`docs/trinkets.md` §4): it joins their trinkets and its
/// `trinket_bought` hook resolves. No price, no offer.
pub fn give(state: &mut TavernState, id: CardId, pool: &mut CardPool, rng: &mut Rng) {
    state.auras.trinkets.push(new_state(id));
    if let Some(bought) = hooks(id).trinket_bought {
        bought(state, pool, rng);
    }
    state.sync_all_auras();
}

// ---------------------------------------------------------------------------------------------
// Dispatch: the player's trinkets' hooks, in the order they were picked
// ---------------------------------------------------------------------------------------------

/// The ids of the player's trinkets, in the order they were picked (a snapshot: a hook may change
/// the list).
fn owned_ids(auras: &PlayerAuras) -> Vec<CardId> {
    auras.trinkets.iter().map(|t| t.card_id).collect()
}

/// `true` if one of the player's trinkets acts at Start of Combat (`docs/trinkets.md` §7.2).
pub fn has_start_of_combat(auras: &PlayerAuras) -> bool {
    auras
        .trinkets
        .iter()
        .any(|t| hooks(t.card_id).trinket_start_of_combat.is_some())
}

/// The `trinket_start_of_combat` hooks of the player's trinkets, in pick order: the engine runs
/// each with its own context, then the damage, destroys and deaths it caused
/// (`docs/trinkets.md` §7.2).
pub fn start_of_combat_hooks(auras: &PlayerAuras) -> Vec<fn(&mut HeroCombatCtx<'_>)> {
    auras
        .trinkets
        .iter()
        .filter_map(|t| hooks(t.card_id).trinket_start_of_combat)
        .collect()
}

/// Start of turn, after the Hero Power's: each trinket's `trinket_turn_start` hook.
pub fn on_turn_start(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    for id in owned_ids(&state.auras) {
        if let Some(turn_start) = hooks(id).trinket_turn_start {
            turn_start(state, pool, rng);
        }
    }
}

/// `true` if one of the player's trinkets reacts to their minions' deaths in combat.
pub fn has_friendly_death(auras: &PlayerAuras) -> bool {
    auras
        .trinkets
        .iter()
        .any(|t| hooks(t.card_id).trinket_friendly_death.is_some())
}

/// A friendly minion died in combat: each trinket's `trinket_friendly_death` hook.
pub fn on_friendly_death(ctx: &mut BoardCtx<'_>, dying: &Unit) {
    for id in owned_ids(ctx.auras) {
        if let Some(friendly_death) = hooks(id).trinket_friendly_death {
            friendly_death(ctx, dying);
        }
    }
}

/// A friendly minion attacks: each trinket's `trinket_friendly_attack` hook.
pub fn on_friendly_attack(ctx: &mut BoardCtx<'_>, attacker_id: UnitId) {
    for id in owned_ids(ctx.auras) {
        if let Some(friendly_attack) = hooks(id).trinket_friendly_attack {
            friendly_attack(ctx, attacker_id);
        }
    }
}

/// After a card was bought: each trinket's `trinket_after_buy` hook.
pub fn after_buy(state: &mut TavernState, bought: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    for id in owned_ids(&state.auras) {
        if let Some(after_buy) = hooks(id).trinket_after_buy {
            after_buy(state, bought, pool, rng);
        }
    }
}

/// After a minion was played or Magnetized: each trinket's `trinket_after_play` hook.
pub fn after_play(state: &mut TavernState, played: &Played, pool: &mut CardPool, rng: &mut Rng) {
    for id in owned_ids(&state.auras) {
        if let Some(after_play) = hooks(id).trinket_after_play {
            after_play(state, played, pool, rng);
        }
    }
}

/// After a minion was sold: each trinket's `trinket_after_sell` hook.
pub fn after_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    for id in owned_ids(&state.auras) {
        if let Some(after_sell) = hooks(id).trinket_after_sell {
            after_sell(state, sold, pool, rng);
        }
    }
}

/// Gold was spent: each trinket's `trinket_gold_spent` hook.
pub fn gold_spent(state: &mut TavernState, amount: u32, pool: &mut CardPool, rng: &mut Rng) {
    for id in owned_ids(&state.auras) {
        if let Some(gold_spent) = hooks(id).trinket_gold_spent {
            gold_spent(state, amount, pool, rng);
        }
    }
}

/// The player ends their turn: each trinket's `trinket_end_turn` hook, with the end-of-turn
/// repeat count (`docs/tavern.md` §7.5).
pub fn end_turn(state: &mut TavernState, repeats: u32, pool: &mut CardPool, rng: &mut Rng) {
    for id in owned_ids(&state.auras) {
        if let Some(end_turn) = hooks(id).trinket_end_turn {
            end_turn(state, repeats, pool, rng);
        }
    }
}

/// A Tavern spell was cast in the Tavern: each trinket's `trinket_spell_cast` hook.
pub fn spell_cast(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    for id in owned_ids(&state.auras) {
        if let Some(spell_cast) = hooks(id).trinket_spell_cast {
            spell_cast(state, pool, rng);
        }
    }
}

/// The Tavern was Refreshed (`docs/tavern.md` §4.1): each trinket's `trinket_after_refresh` hook,
/// then, for a Refresh the player does, each trinket's `trinket_after_player_refresh` hook.
pub fn after_refresh(
    state: &mut TavernState,
    kind: RefreshKind,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    for id in owned_ids(&state.auras) {
        if let Some(after_refresh) = hooks(id).trinket_after_refresh {
            after_refresh(state, pool, rng);
        }
    }
    if kind != RefreshKind::Player {
        return;
    }
    for id in owned_ids(&state.auras) {
        if let Some(after_refresh) = hooks(id).trinket_after_player_refresh {
            after_refresh(state, pool, rng);
        }
    }
}

/// Extra times the effects counted by `passive` trigger, summed over the trinkets'
/// `trinket_extra_triggers` (0 without trinkets).
pub fn extra_triggers(auras: &PlayerAuras, passive: Passive) -> u32 {
    auras
        .trinkets
        .iter()
        .filter_map(|t| hooks(t.card_id).trinket_extra_triggers)
        .map(|f| f(passive))
        .sum()
}

/// A combat starts for the player: each trinket's `trinket_combat_start` hook.
pub fn combat_start(auras: &mut PlayerAuras) {
    for trinket in &mut auras.trinkets {
        if let Some(combat_start) = hooks(trinket.card_id).trinket_combat_start {
            combat_start(trinket);
        }
    }
}

/// After the Tavern was upgraded: each trinket's `trinket_after_upgrade` hook.
pub fn after_upgrade(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    for id in owned_ids(&state.auras) {
        if let Some(after_upgrade) = hooks(id).trinket_after_upgrade {
            after_upgrade(state, pool, rng);
        }
    }
}

/// A friendly minion is being summoned in combat: each trinket's `trinket_combat_summon` hook.
pub fn combat_summon(auras: &PlayerAuras, summoned: &mut Unit) {
    for trinket in &auras.trinkets {
        if let Some(combat_summon) = hooks(trinket.card_id).trinket_combat_summon {
            combat_summon(trinket, summoned);
        }
    }
}

/// A friendly minion was summoned in the Tavern: each trinket's `trinket_tavern_summon` hook.
pub fn tavern_summon(auras: &PlayerAuras, summoned: &mut Unit) {
    for trinket in &auras.trinkets {
        if let Some(tavern_summon) = hooks(trinket.card_id).trinket_tavern_summon {
            tavern_summon(trinket, summoned);
        }
    }
}

/// The player's Deity awakened in combat at `ctx.board[idx]`: each trinket's
/// `trinket_deity_awakened` hook.
pub fn on_deity_awakened(ctx: &mut BoardCtx<'_>, idx: usize) {
    for id in owned_ids(ctx.auras) {
        if let Some(awakened) = hooks(id).trinket_deity_awakened {
            awakened(ctx, idx);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Helpers for the trinkets' hooks
// ---------------------------------------------------------------------------------------------

/// A trinket's "Get a ..." (`docs/trinkets.md` §5): a plain copy of the minion `card_id` from the
/// catalog (not the pool) goes to the hand, or waits for room in a full hand.
pub fn give_minion(state: &mut TavernState, card_id: CardId) {
    if let Some(template) = cards::template(card_id) {
        state.add_to_hand_or_wait(template.instantiate());
    }
}

/// A random minion of the player's Tier or lower from the game's catalog (Duos-exclusive ones in
/// Duos) that `keep` accepts, one `rng.below(k)` (none when there is a single one), or `None`:
/// the cards trinkets give in combat ("get a random Beast"; **engine decision**, as Ini
/// Stormcoil's Mech: `docs/heroes.md` §11.1).
pub fn random_minion(
    ctx: &mut BoardCtx<'_>,
    keep: impl Fn(&cards::CardTemplate) -> bool,
) -> Option<Unit> {
    let tier = ctx.hero_tier;
    let pool: Vec<_> = cards::game_templates(ctx.auras)
        .iter()
        .filter(|t| t.tavern_tier <= tier && keep(t))
        .collect();
    match pool.len() {
        0 => None,
        k => Some(pool[ctx.rng.below(k)].instantiate()),
    }
}

/// An Avenge or "after N friendly minions die" count: the trinket `id`'s `counter` goes up by 1,
/// and `true` (the count starting again) every `every`th time.
pub fn avenge(ctx: &mut BoardCtx<'_>, id: CardId, every: i32) -> bool {
    let Some(t) = state_mut(ctx.auras, id) else {
        return false;
    };
    t.counter += 1;
    if t.counter < every {
        return false;
    }
    t.counter -= every;
    true
}

/// The `trinket_combat_start` of a trinket whose count or charges are per combat (Avenge, "3
/// times per combat"): its `counter` and `armed` start again (confirmed: `docs/trinkets.md`
/// §11.1).
pub fn reset_per_combat(trinket: &mut TrinketState) {
    trinket.counter = 0;
    trinket.armed = 0;
}

/// The start of turn reset of a trinket whose charges are per turn ("twice per turn", "the
/// first ... each turn"): its `counter` and `armed` start again.
pub fn reset_per_turn(state: &mut TavernState, id: CardId) {
    if let Some(t) = state_mut(&mut state.auras, id) {
        t.counter = 0;
        t.armed = 0;
    }
}

/// A Tavern count: the trinket `id`'s `counter` goes up by 1, and `true` (the count starting
/// again) every `every`th time.
pub fn count_to(auras: &mut PlayerAuras, id: CardId, every: i32) -> bool {
    let Some(t) = state_mut(auras, id) else {
        return false;
    };
    t.counter += 1;
    if t.counter < every {
        return false;
    }
    t.counter -= every;
    true
}

/// The Tavern's Elemental bonus goes up by `(attack, health)` for the rest of the game, and the
/// Elementals in the Tavern now gain it (as `Dune Dweller`'s).
pub fn buff_tavern_elementals(state: &mut TavernState, attack: i32, health: i32) {
    state.auras.tavern_elemental_atk += attack;
    state.auras.tavern_elemental_hp += health;
    for unit in &mut state.shop {
        if !unit.is_spell && unit.tribe.matches(Tribe::Elemental) {
            unit.add_stats(attack, health);
        }
    }
}

/// A trinket's "Get a random ..." in the Tavern: a random minion of the player's Tier or lower
/// from the game's catalog that `keep` accepts (one `rng.below(k)`, none when there is a single
/// one) goes to the hand, or waits for room in a full hand (`docs/trinkets.md` §5).
pub fn give_random_minion(
    state: &mut TavernState,
    rng: &mut Rng,
    keep: impl Fn(&cards::CardTemplate) -> bool,
) {
    let tier = state.tavern_tier;
    let pool: Vec<_> = cards::game_templates(&state.auras)
        .iter()
        .filter(|t| t.tavern_tier <= tier && keep(t))
        .collect();
    if !pool.is_empty() {
        let card = pool[rng.below(pool.len())].instantiate();
        state.add_to_hand_or_wait(card);
    }
}

/// A trinket's "Get a random Tier N minion" in the Tavern: a random minion of exactly Tier `tier`
/// from the game's catalog (one `rng.below(k)`) goes to the hand, or waits for room in a full
/// hand (`docs/trinkets.md` §5). Nothing if the catalog has none.
pub fn give_random_minion_of_tier(state: &mut TavernState, rng: &mut Rng, tier: u32) {
    let pool: Vec<_> = cards::game_templates(&state.auras)
        .iter()
        .filter(|t| t.tavern_tier == tier)
        .collect();
    if !pool.is_empty() {
        let card = pool[rng.below(pool.len())].instantiate();
        state.add_to_hand_or_wait(card);
    }
}

/// The left-most minion in the hand (spells skipped) gains `+attack/+health`, if there is one.
pub fn buff_left_most_in_hand(state: &mut TavernState, attack: i32, health: i32) {
    if let Some(unit) = state.hand.iter_mut().find(|u| !u.is_spell) {
        unit.add_stats(attack, health);
    }
}

/// The Undead Attack bonus (`auras.undead_bonus_attack`, "your Undead have +N Attack this game
/// (wherever they are)", as `Nerubian Deathswarmer`'s) goes up by `attack`, and the auras resync.
pub fn buff_undead_attack(state: &mut TavernState, attack: i32) {
    state.auras.undead_bonus_attack += attack;
    state.sync_all_auras();
}

/// The Tavern's bonus (`auras.tavern_all_atk` / `tavern_all_hp`: "minions in the Tavern have
/// +N/+N", as `Staff of Enrichment`'s) goes up by `(attack, health)` for the rest of the game, and
/// the minions in the Tavern now gain it.
pub fn buff_tavern_minions(state: &mut TavernState, attack: i32, health: i32) {
    state.auras.tavern_all_atk += attack;
    state.auras.tavern_all_hp += health;
    for unit in &mut state.shop {
        if !unit.is_spell {
            unit.add_stats(attack, health);
        }
    }
}

/// Cast `Easterly Winds` `times` times by an effect (`spells::cast_spell_by_effect`: Tavern spell
/// casts, which the spell-cast observers see).
pub fn cast_easterly_winds(
    state: &mut TavernState,
    times: u32,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    for _ in 0..times {
        if let Some(spell) = cards::spells::spell_by_id(cards::spells::SPELL_EASTERLY_WINDS) {
            cards::spells::cast_spell_by_effect(state, spell, MinionSlot::Board(0), pool, rng);
        }
    }
}

/// Cast `Repair Job` by an effect (`spells::cast_spell_by_effect`: a Tavern spell cast, which the
/// spell-cast observers see) on `board[pos]`.
pub fn cast_repair_job(state: &mut TavernState, pos: usize, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(spell) = cards::spells::spell_by_id(cards::spells::SPELL_REPAIR_JOB) {
        cards::spells::cast_spell_by_effect(state, spell, MinionSlot::Board(pos), pool, rng);
    }
}

/// `count` different random Pirates on the board (every one if there are `count` or fewer; one
/// `rng.below(k)` per pick otherwise) gain `+attack/+health`.
pub fn buff_random_pirates(
    state: &mut TavernState,
    count: usize,
    attack: i32,
    health: i32,
    rng: &mut Rng,
) {
    let mut pirates: Vec<usize> = (0..state.board.len())
        .filter(|&i| state.board[i].tribe.matches(Tribe::Pirate))
        .collect();
    while pirates.len() > count {
        pirates.remove(rng.below(pirates.len()));
    }
    for i in pirates {
        state.board[i].add_stats(attack, health);
    }
}

/// One of `tied` (non-empty) at random: one `rng.below(k)`, none when there is a single one (ties
/// for "highest" / "lowest" are broken at random: confirmed, `docs/trinkets.md` §12).
pub fn pick_tied(tied: &[usize], rng: &mut Rng) -> usize {
    match tied {
        [only] => *only,
        _ => tied[rng.below(tied.len())],
    }
}

/// A `trinket_end_turn` without a turn counter: `payoff` resolves `repeats` times (the end-of-turn
/// repeat count: `docs/tavern.md` §7.5).
pub fn repeat_end_turn(
    state: &mut TavernState,
    repeats: u32,
    pool: &mut CardPool,
    rng: &mut Rng,
    payoff: fn(&mut TavernState, &mut CardPool, &mut Rng),
) {
    for _ in 0..repeats {
        payoff(state, pool, rng);
    }
}

/// The left- and right-most of `indexes` (in board order), once if there is only one.
pub fn ends(indexes: &[usize]) -> Vec<usize> {
    match indexes {
        [] => Vec::new(),
        [only] => vec![*only],
        [first, .., last] => vec![*first, *last],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// `(file stem, source)` of every trinket file.
    fn trinket_files() -> Vec<(String, String)> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/trinkets");
        let mut files: Vec<(String, String)> = fs::read_dir(&dir)
            .expect("src/trinkets")
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

    /// The header a trinket file must start with (`docs/trinkets.md` §9).
    fn expected_header(t: &TrinketData) -> String {
        let kind = if t.greater { "Greater" } else { "Lesser" };
        let mut tribes: Vec<&str> = t.tribes.iter().map(|&x| tribe_name(x)).collect();
        if t.duos_only {
            tribes.push("Duos only");
        }
        let extra = if tribes.is_empty() {
            String::new()
        } else {
            format!(", {}", tribes.join(", "))
        };
        format!(
            "//! `{}` (`{}`) — {kind}, {} Gold{extra}.\n//!\n//! {}\n",
            display_name(t),
            t.card_id,
            t.cost,
            t.text
        )
    }

    fn tribe_name(t: Tribe) -> &'static str {
        match t {
            Tribe::Aberration => "Aberration",
            Tribe::Beast => "Beast",
            Tribe::Demon => "Demon",
            Tribe::Dragon => "Dragon",
            Tribe::Elemental => "Elemental",
            Tribe::Mech => "Mech",
            Tribe::Murloc => "Murloc",
            Tribe::Pirate => "Pirate",
            Tribe::Quilboar => "Quilboar",
            Tribe::Undead => "Undead",
            Tribe::Naga => "Naga",
            _ => "?",
        }
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
    fn every_trinket_file_matches_the_table() {
        let files = trinket_files();
        assert_eq!(
            files.len(),
            IMPLEMENTED.len(),
            "every trinket file is in IMPLEMENTED"
        );
        let scenarios = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios/trinkets");
        for (stem, src) in &files {
            let id = constant(src, "ID").unwrap_or_else(|| panic!("{stem}.rs: no `pub const ID`"));
            let t = data(id).unwrap_or_else(|| panic!("{stem}.rs: ID {id} is not in the table"));
            assert_eq!(slug(t), *stem, "{stem}.rs: file name is the trinket's slug");
            let header = expected_header(t);
            assert!(
                src.starts_with(&header),
                "{stem}.rs: the header doesn't match the table; expected:\n{header}"
            );
            assert!(is_implemented(id), "{stem}.rs: not in IMPLEMENTED");
            assert!(
                behaviors().iter().any(|&(card, _)| card == id),
                "{stem}.rs: the trinket has no hooks"
            );
            let scenario = fs::read_to_string(scenarios.join(format!("{stem}.yaml")))
                .unwrap_or_else(|_| panic!("{stem}.rs: no tests/scenarios/trinkets/{stem}.yaml"));
            assert!(
                scenario.starts_with(&format!("trinket: {}\n", display_name(t))),
                "{stem}.yaml: starts with `trinket: {}`",
                display_name(t)
            );
        }
    }

    #[test]
    fn the_table_has_the_whole_pool() {
        assert_eq!(TRINKETS.len(), 264);
        assert_eq!(TRINKETS.iter().filter(|t| !t.greater).count(), 127);
        let naga = |t: &&TrinketData| t.tribes.contains(&Tribe::Naga);
        assert_eq!(TRINKETS.iter().filter(naga).count(), 18);
        assert_eq!(TRINKETS.iter().filter(|t| t.duos_only).count(), 9);
        let solo = TRINKETS
            .iter()
            .filter(|t| !naga(t) && !t.duos_only)
            .collect::<Vec<_>>();
        assert_eq!(solo.len(), 237);
        assert_eq!(solo.iter().filter(|t| !t.greater).count(), 115);
        for t in TRINKETS {
            assert!(t.id > 11_000, "{}: trinket ids don't collide", t.name);
            assert!(crate::heroes::table().iter().all(|h| h.power.id != t.id));
        }
        let lesser = by_name("Eternal Portrait").expect("Eternal Portrait");
        let greater = by_name("Eternal Portrait (Greater)").expect("the Greater one");
        assert!(!lesser.greater && greater.greater);
        assert_eq!(slug(greater), "eternal_portrait_greater");
        assert_eq!(
            by_card_id("BG30_MagicItem_902").map(|t| t.name),
            Some("Holy Mallet")
        );
    }

    fn minion(tribe: Tribe) -> Unit {
        let mut u = Unit::new("m", 1, 1);
        u.tribe = tribe;
        u
    }

    /// Over the whole table (every Solo trinket, as if all were implemented), random boards and
    /// hands get offers that follow every rule (`docs/trinkets.md` §3.2).
    #[test]
    fn offers_from_the_whole_pool_follow_the_rules() {
        let tribes = [
            Tribe::None,
            Tribe::Beast,
            Tribe::Demon,
            Tribe::Dragon,
            Tribe::Elemental,
            Tribe::Mech,
            Tribe::Murloc,
            Tribe::Pirate,
            Tribe::Quilboar,
            Tribe::Undead,
            Tribe::BeastPirate,
            Tribe::MechMurloc,
            Tribe::All,
        ];
        let pool = || {
            TRINKETS
                .iter()
                .filter(|t| !t.duos_only && !t.tribes.contains(&Tribe::Naga))
        };
        let mut rng = Rng::new(11);
        for _ in 0..2000 {
            let mut state = TavernState::new();
            for _ in 0..rng.below(8) {
                state.board.push(minion(tribes[rng.below(tribes.len())]));
            }
            for _ in 0..rng.below(6) {
                state.hand.push(minion(tribes[rng.below(tribes.len())]));
            }
            let greater = rng.below(2) == 0;
            if rng.below(3) == 0 {
                let kind: Vec<_> = pool().filter(|t| t.greater == greater).collect();
                state
                    .auras
                    .trinkets
                    .push(new_state(kind[rng.below(kind.len())].id));
            }
            let candidates = candidates_in(&state, greater, pool());
            let offer = draw_from(&candidates, &mut rng);
            let class = |id: CardId| candidates.iter().find(|c| c.0 == id).map(|c| c.1);
            let counts = type_counts(&state);
            let threshold = if greater { 3 } else { 2 };
            assert_eq!(offer.len(), OFFER_COUNT, "{offer:?}");
            for (i, o) in offer.iter().enumerate() {
                let t = data(o.card_id).unwrap();
                assert_eq!(t.greater, greater);
                assert!(
                    offer[..i].iter().all(|p| p.card_id != o.card_id),
                    "distinct"
                );
                assert!(!owns(&state.auras, o.card_id), "never one the player owns");
                let eligible = t.tribes.is_empty()
                    || t.tribes.iter().any(|&x| {
                        LOBBY_TRIBES
                            .iter()
                            .position(|&l| l == x)
                            .is_some_and(|k| counts[k] >= threshold)
                    });
                assert!(eligible, "{} needs {threshold} minions of its type", t.name);
                let price = match class(o.card_id) {
                    Some(OfferClass::Off) => t.cost.saturating_sub(OFF_TYPE_DISCOUNT),
                    _ => t.cost,
                };
                assert_eq!(o.cost, price);
            }
            assert!(offer.iter().any(|o| o.cost <= CHEAP_COST), "a cheap option");
            assert!(offer
                .iter()
                .any(|o| class(o.card_id) == Some(OfferClass::Neutral)));
            let off = offer
                .iter()
                .filter(|o| class(o.card_id) == Some(OfferClass::Off))
                .count();
            assert!(off <= 1, "at most one option of another type");
        }
    }

    #[test]
    fn type_counts_count_two_types_and_all() {
        let mut state = TavernState::new();
        state.board = vec![minion(Tribe::BeastPirate), minion(Tribe::All)];
        state.hand = vec![minion(Tribe::Beast)];
        let mut spell = minion(Tribe::Beast);
        spell.is_spell = true;
        state.hand.push(spell);
        let counts = type_counts(&state);
        let at = |t: Tribe| counts[LOBBY_TRIBES.iter().position(|&l| l == t).unwrap()];
        assert_eq!(
            (at(Tribe::Beast), at(Tribe::Pirate), at(Tribe::Undead)),
            (3, 2, 1)
        );
    }
}
