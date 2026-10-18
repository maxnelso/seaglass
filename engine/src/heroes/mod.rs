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

pub mod a_f_kay;
pub mod alakir;
pub mod alexstrasza;
pub mod ambassador_faelin;
pub mod aranna_starseeker;
pub mod arch_villain_rafaam;
pub mod brukan;
pub mod capn_hoggarr;
pub mod captain_eudora;
pub mod captain_hooktusk;
pub mod cariel_roame;
pub mod chenvaala;
pub mod cho;
pub mod cookie_the_cook;
pub mod cthun;
pub mod dancin_deryl;
pub mod death_speaker_blackthorn;
pub mod deathwing;
pub mod dinotamer_brann;
pub mod doctor_hollidae;
pub mod drekthar;
pub mod drestagath;
pub mod edwin_vancleef;
pub mod elise_starseeker;
pub mod enhance_o_mechano;
pub mod exarch_othaar;
pub mod farseer_nobundo;
pub mod flobbidinous_floop;
pub mod forest_lord_cenarius;
pub mod forest_warden_omu;
pub mod fungalmancer_flurgl;
pub mod galakrond;
pub mod galewing;
pub mod gall;
pub mod genn_worgen_king;
pub mod george_the_fallen;
pub mod greybough;
pub mod guff_runetotem;
pub mod heistbaron_togwaggle;
pub mod illidan_stormrage;
pub mod infinite_toki;
pub mod inge_the_iron_hymn;
pub mod ini_stormcoil;
pub mod jandice_barov;
pub mod kaelthas_sunstrider;
pub mod king_mukla;
pub mod kithix;
pub mod kurtrus_ashfallen;
pub mod lich_bazhial;
pub mod loh_the_living_legend;
pub mod lord_barov;
pub mod lord_jaraxxus;
pub mod madam_goya;
pub mod maiev_shadowsong;
pub mod malygos;
pub mod master_nguyen;
pub mod millhouse_manastorm;
pub mod millificent_manastorm;
pub mod mister_clocksworth;
pub mod mr_bigglesworth;
pub mod murloc_holmes;
pub mod mutanus_the_devourer;
pub mod nightmare_lord_xavius;
pub mod nozdormu;
pub mod nzoth;
pub mod onyxia;
pub mod overlord_saurfang;
pub mod ozumat;
pub mod patches_the_pirate;
pub mod patchwerk;
pub mod pyramad;
pub mod queen_wagtoggle;
pub mod ragnaros_the_firelord;
pub mod rakanishu;
pub mod reno_jackson;
pub mod rock_master_voone;
pub mod rokara;
pub mod scabbs_cutterbutter;
pub mod shudderwock;
pub mod silas_darkmoon;
pub mod sindragosa;
pub mod sir_finley_mrrgglton;
pub mod skycapn_kragg;
pub mod snake_eyes;
pub mod sneed;
pub mod sylvanas_windrunner;
pub mod taethelan_bloodwatcher;
pub mod tamsin_roame;
pub mod tavish_stormpike;
pub mod teron_gorefiend;
pub mod tess_greymane;
pub mod the_curator;
pub mod the_jailer;
pub mod the_lich_king;
pub mod the_nameless_one;
pub mod the_rat_king;
pub mod thorim_stormlord;
pub mod tickatus;
pub mod time_twister_chromie;
pub mod trade_prince_gallywix;
pub mod trastath_soul_parasite;
pub mod vanndar_stormpike;
pub mod varden_dawngrasp;
pub mod voljin;
pub mod xyrella;
pub mod yogg_saron_hopes_end;
pub mod ysera;
pub mod yshaarj;
pub mod zephrys_the_great;
pub mod zerek_master_cloner;

pub use table::{CARD_DEFS_BUILD, HEROES};

use crate::cards::{
    hooks, ActivateTargetKind, BoardCtx, CardFlags, CardHooks, HeroCombatCtx, Played,
};
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

/// The most cards a hand holds (`tavern.md` §2).
const MAX_HAND_SIZE: usize = 10;

/// The base Health a hero's `health` is counted from (`docs/heroes.md` §2.3).
const BASE_HEALTH: i32 = 30;

/// An implemented hero: its id and its Hero Power cards' hooks.
type Implemented = (HeroId, fn() -> Vec<(CardId, CardHooks)>);

/// Every implemented hero, by file name.
const IMPLEMENTED: &[Implemented] = &[
    (a_f_kay::ID, a_f_kay::behaviors),
    (alakir::ID, alakir::behaviors),
    (alexstrasza::ID, alexstrasza::behaviors),
    (ambassador_faelin::ID, ambassador_faelin::behaviors),
    (aranna_starseeker::ID, aranna_starseeker::behaviors),
    (arch_villain_rafaam::ID, arch_villain_rafaam::behaviors),
    (brukan::ID, brukan::behaviors),
    (capn_hoggarr::ID, capn_hoggarr::behaviors),
    (captain_eudora::ID, captain_eudora::behaviors),
    (captain_hooktusk::ID, captain_hooktusk::behaviors),
    (cariel_roame::ID, cariel_roame::behaviors),
    (chenvaala::ID, chenvaala::behaviors),
    (cho::ID, cho::behaviors),
    (cookie_the_cook::ID, cookie_the_cook::behaviors),
    (cthun::ID, cthun::behaviors),
    (dancin_deryl::ID, dancin_deryl::behaviors),
    (
        death_speaker_blackthorn::ID,
        death_speaker_blackthorn::behaviors,
    ),
    (deathwing::ID, deathwing::behaviors),
    (dinotamer_brann::ID, dinotamer_brann::behaviors),
    (doctor_hollidae::ID, doctor_hollidae::behaviors),
    (drekthar::ID, drekthar::behaviors),
    (drestagath::ID, drestagath::behaviors),
    (edwin_vancleef::ID, edwin_vancleef::behaviors),
    (elise_starseeker::ID, elise_starseeker::behaviors),
    (enhance_o_mechano::ID, enhance_o_mechano::behaviors),
    (exarch_othaar::ID, exarch_othaar::behaviors),
    (farseer_nobundo::ID, farseer_nobundo::behaviors),
    (flobbidinous_floop::ID, flobbidinous_floop::behaviors),
    (forest_lord_cenarius::ID, forest_lord_cenarius::behaviors),
    (forest_warden_omu::ID, forest_warden_omu::behaviors),
    (fungalmancer_flurgl::ID, fungalmancer_flurgl::behaviors),
    (galakrond::ID, galakrond::behaviors),
    (galewing::ID, galewing::behaviors),
    (gall::ID, gall::behaviors),
    (genn_worgen_king::ID, genn_worgen_king::behaviors),
    (george_the_fallen::ID, george_the_fallen::behaviors),
    (greybough::ID, greybough::behaviors),
    (guff_runetotem::ID, guff_runetotem::behaviors),
    (heistbaron_togwaggle::ID, heistbaron_togwaggle::behaviors),
    (illidan_stormrage::ID, illidan_stormrage::behaviors),
    (infinite_toki::ID, infinite_toki::behaviors),
    (inge_the_iron_hymn::ID, inge_the_iron_hymn::behaviors),
    (ini_stormcoil::ID, ini_stormcoil::behaviors),
    (jandice_barov::ID, jandice_barov::behaviors),
    (kaelthas_sunstrider::ID, kaelthas_sunstrider::behaviors),
    (king_mukla::ID, king_mukla::behaviors),
    (kithix::ID, kithix::behaviors),
    (kurtrus_ashfallen::ID, kurtrus_ashfallen::behaviors),
    (lich_bazhial::ID, lich_bazhial::behaviors),
    (loh_the_living_legend::ID, loh_the_living_legend::behaviors),
    (lord_barov::ID, lord_barov::behaviors),
    (lord_jaraxxus::ID, lord_jaraxxus::behaviors),
    (madam_goya::ID, madam_goya::behaviors),
    (maiev_shadowsong::ID, maiev_shadowsong::behaviors),
    (malygos::ID, malygos::behaviors),
    (master_nguyen::ID, master_nguyen::behaviors),
    (millhouse_manastorm::ID, millhouse_manastorm::behaviors),
    (millificent_manastorm::ID, millificent_manastorm::behaviors),
    (mister_clocksworth::ID, mister_clocksworth::behaviors),
    (mr_bigglesworth::ID, mr_bigglesworth::behaviors),
    (murloc_holmes::ID, murloc_holmes::behaviors),
    (mutanus_the_devourer::ID, mutanus_the_devourer::behaviors),
    (nightmare_lord_xavius::ID, nightmare_lord_xavius::behaviors),
    (nozdormu::ID, nozdormu::behaviors),
    (nzoth::ID, nzoth::behaviors),
    (onyxia::ID, onyxia::behaviors),
    (overlord_saurfang::ID, overlord_saurfang::behaviors),
    (ozumat::ID, ozumat::behaviors),
    (patches_the_pirate::ID, patches_the_pirate::behaviors),
    (patchwerk::ID, patchwerk::behaviors),
    (pyramad::ID, pyramad::behaviors),
    (queen_wagtoggle::ID, queen_wagtoggle::behaviors),
    (ragnaros_the_firelord::ID, ragnaros_the_firelord::behaviors),
    (rakanishu::ID, rakanishu::behaviors),
    (reno_jackson::ID, reno_jackson::behaviors),
    (rock_master_voone::ID, rock_master_voone::behaviors),
    (rokara::ID, rokara::behaviors),
    (scabbs_cutterbutter::ID, scabbs_cutterbutter::behaviors),
    (shudderwock::ID, shudderwock::behaviors),
    (silas_darkmoon::ID, silas_darkmoon::behaviors),
    (sindragosa::ID, sindragosa::behaviors),
    (sir_finley_mrrgglton::ID, sir_finley_mrrgglton::behaviors),
    (skycapn_kragg::ID, skycapn_kragg::behaviors),
    (snake_eyes::ID, snake_eyes::behaviors),
    (sneed::ID, sneed::behaviors),
    (sylvanas_windrunner::ID, sylvanas_windrunner::behaviors),
    (
        taethelan_bloodwatcher::ID,
        taethelan_bloodwatcher::behaviors,
    ),
    (tamsin_roame::ID, tamsin_roame::behaviors),
    (tavish_stormpike::ID, tavish_stormpike::behaviors),
    (teron_gorefiend::ID, teron_gorefiend::behaviors),
    (tess_greymane::ID, tess_greymane::behaviors),
    (the_curator::ID, the_curator::behaviors),
    (the_jailer::ID, the_jailer::behaviors),
    (the_lich_king::ID, the_lich_king::behaviors),
    (the_nameless_one::ID, the_nameless_one::behaviors),
    (the_rat_king::ID, the_rat_king::behaviors),
    (thorim_stormlord::ID, thorim_stormlord::behaviors),
    (tickatus::ID, tickatus::behaviors),
    (time_twister_chromie::ID, time_twister_chromie::behaviors),
    (trade_prince_gallywix::ID, trade_prince_gallywix::behaviors),
    (trastath_soul_parasite::ID, trastath_soul_parasite::behaviors),
    (vanndar_stormpike::ID, vanndar_stormpike::behaviors),
    (varden_dawngrasp::ID, varden_dawngrasp::behaviors),
    (voljin::ID, voljin::behaviors),
    (xyrella::ID, xyrella::behaviors),
    (yogg_saron_hopes_end::ID, yogg_saron_hopes_end::behaviors),
    (ysera::ID, ysera::behaviors),
    (yshaarj::ID, yshaarj::behaviors),
    (zephrys_the_great::ID, zephrys_the_great::behaviors),
    (zerek_master_cloner::ID, zerek_master_cloner::behaviors),
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
/// in Duos, the Dark Gift ones (`CardFlags::DARK_GIFTS`) only in a game with Dark Gifts
/// (`docs/dark_gifts.md` §12), in table order.
pub fn offerable(duos: bool, dark_gifts: bool) -> Vec<HeroId> {
    HEROES
        .iter()
        .filter(|h| is_implemented(h.id) && (duos || !h.duos_only))
        .filter(|h| dark_gifts || !hooks(h.power.id).has(CardFlags::DARK_GIFTS))
        .map(|h| h.id)
        .collect()
}

/// Draw `per_player` distinct heroes for each of `players` players (`docs/heroes.md` §6): for
/// each player in turn, uniformly among the offerable heroes not offered yet, one `rng.below(k)`
/// per hero. No hero is offered twice in a lobby, so a Duos team never gets two copies of one (The
/// Nameless One). A player gets fewer if the pool runs out.
pub fn draw_offers(
    players: usize,
    per_player: usize,
    duos: bool,
    dark_gifts: bool,
    rng: &mut Rng,
) -> Vec<Vec<HeroId>> {
    let mut left = offerable(duos, dark_gifts);
    (0..players)
        .map(|_| {
            let mut offered = Vec::with_capacity(per_player);
            while offered.len() < per_player && !left.is_empty() {
                offered.push(left.remove(rng.below(left.len())));
            }
            offered
        })
        .collect()
}

/// A fresh state for the Hero Power `p` (`docs/heroes.md` §3.2): at its starting cost, with a use
/// left if it is active.
pub fn new_power_state(p: PowerData) -> HeroPowerState {
    HeroPowerState {
        card_id: p.id,
        cost: p.cost,
        uses_left: u32::from(hooks(p.id).hero_power.is_some()),
        counter: 0,
        stacks: 0,
        armed: 0,
        stored: Vec::new(),
        picked: None,
    }
}

/// A fresh state for the hero `h` (`docs/heroes.md` §3.2): its Hero Power at its starting cost,
/// with a use left if it is active.
pub fn new_state(h: &HeroData) -> HeroState {
    HeroState {
        hero_id: h.id,
        power: new_power_state(h.power),
        second_power: None,
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

std::thread_local! {
    /// True while checking legality of `second_power` (`can_use_2`), so [`active_power`] reads
    /// `second_power` on `&TavernState`.
    static CHECKING_SECOND_POWER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The Hero Power currently being checked or used (`power`, or `second_power` during `can_use_2`).
pub fn active_power(state: &TavernState) -> Option<&HeroPowerState> {
    let hero = state.auras.hero.as_ref()?;
    if CHECKING_SECOND_POWER.with(std::cell::Cell::get) {
        hero.second_power.as_ref()
    } else {
        Some(&hero.power)
    }
}

/// Run `f` with `hero.power` and `hero.second_power` swapped if `second_power` is present.
fn with_second_power<R>(
    state: &mut TavernState,
    f: impl FnOnce(&mut TavernState) -> R,
) -> Option<R> {
    {
        let hero = state.auras.hero.as_mut()?;
        let second = hero.second_power.as_mut()?;
        std::mem::swap(&mut hero.power, second);
    }
    let out = f(state);
    if let Some(hero) = state.auras.hero.as_mut() {
        if let Some(second) = hero.second_power.as_mut() {
            std::mem::swap(&mut hero.power, second);
        }
    }
    Some(out)
}

/// Run `f` with `hero.power` and `hero.second_power` swapped on `auras` if `second_power` is present.
fn with_second_power_auras<R>(
    auras: &mut PlayerAuras,
    f: impl FnOnce(&mut PlayerAuras) -> R,
) -> Option<R> {
    {
        let hero = auras.hero.as_mut()?;
        let second = hero.second_power.as_mut()?;
        std::mem::swap(&mut hero.power, second);
    }
    let out = f(auras);
    if let Some(hero) = auras.hero.as_mut() {
        if let Some(second) = hero.second_power.as_mut() {
            std::mem::swap(&mut hero.power, second);
        }
    }
    Some(out)
}

/// The player's Hero Power's hooks, if they have a hero.
fn power_hooks(auras: &PlayerAuras) -> Option<&'static CardHooks> {
    auras.hero.as_ref().map(|h| hooks(h.power.card_id))
}

/// The player's second Hero Power's hooks, if they have one (`Genn, Worgen King`).
fn second_power_hooks(auras: &PlayerAuras) -> Option<&'static CardHooks> {
    auras
        .hero
        .as_ref()
        .and_then(|h| h.second_power.as_ref())
        .map(|p| hooks(p.card_id))
}

/// `true` if `target` is a valid target of kind `kind` in `state` (`docs/heroes.md` §3.3).
fn target_ok(state: &TavernState, kind: ActivateTargetKind, target: Option<usize>) -> bool {
    match (kind, target) {
        (ActivateTargetKind::None, None) => true,
        (ActivateTargetKind::BoardAny, Some(t)) => t < state.board.len(),
        (ActivateTargetKind::ShopCard, Some(t)) => t < state.shop.len(),
        (ActivateTargetKind::HandCard, Some(t)) => {
            state.hand.get(t).is_some_and(|c| c.locked_turns == 0)
        }
        (ActivateTargetKind::BoardOrShopMinion, Some(t)) if t < 7 => t < state.board.len(),
        (ActivateTargetKind::BoardOrShopMinion, Some(t)) if (7..14).contains(&t) => {
            state.shop.get(t - 7).is_some_and(|u| !u.is_spell)
        }
        (ActivateTargetKind::PairSide, Some(0 | 1)) => true,
        _ => false,
    }
}

/// The candidate targets of a Hero Power of target kind `kind` in `state`.
fn targets(state: &TavernState, kind: ActivateTargetKind) -> Vec<Option<usize>> {
    match kind {
        ActivateTargetKind::None => vec![None],
        ActivateTargetKind::BoardAny => (0..state.board.len()).map(Some).collect(),
        ActivateTargetKind::ShopCard => (0..state.shop.len()).map(Some).collect(),
        ActivateTargetKind::HandCard => (0..state.hand.len()).map(Some).collect(),
        ActivateTargetKind::BoardOrShopMinion => (0..state.board.len())
            .chain(
                (0..state.shop.len())
                    .filter(|&i| !state.shop[i].is_spell)
                    .map(|i| 7 + i),
            )
            .map(Some)
            .collect(),
        ActivateTargetKind::PairSide => vec![Some(0), Some(1)],
        _ => Vec::new(),
    }
}

fn can_use_power(state: &TavernState, power: &HeroPowerState, target: Option<usize>) -> bool {
    let card = hooks(power.card_id);
    card.hero_power.is_some()
        && power.uses_left > 0
        && state.gold >= power.cost
        && card.hero_power_usable.is_none_or(|usable| usable(state))
        && target_ok(state, card.activate_target, target)
        && match (target, card.hero_power_target) {
            (Some(t), Some(filter)) => filter(state, t),
            _ => true,
        }
}

/// `true` if the player can use their Hero Power on `target` now, apart from pending prompts
/// (`docs/heroes.md` §5): an active power with uses left, enough Gold, its own conditions, a
/// valid target.
pub fn can_use(state: &TavernState, target: Option<usize>) -> bool {
    let Some(hero) = state.auras.hero.as_ref() else {
        return false;
    };
    can_use_power(state, &hero.power, target)
}

/// `true` if the player can use their second Hero Power (`HeroPower2`) on `target` now.
pub fn can_use_2(state: &TavernState, target: Option<usize>) -> bool {
    let Some(second) = state
        .auras
        .hero
        .as_ref()
        .and_then(|h| h.second_power.as_ref())
    else {
        return false;
    };
    CHECKING_SECOND_POWER.with(|c| c.set(true));
    let ok = can_use_power(state, second, target);
    CHECKING_SECOND_POWER.with(|c| c.set(false));
    ok
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

/// The targets the player can use their second Hero Power (`HeroPower2`) on now.
pub fn valid_targets_2(state: &TavernState) -> Vec<Option<usize>> {
    let Some(card) = second_power_hooks(&state.auras) else {
        return Vec::new();
    };
    targets(state, card.activate_target)
        .into_iter()
        .filter(|&t| can_use_2(state, t))
        .collect()
}

/// Use the player's Hero Power on `target` (legal, `docs/heroes.md` §5): a use, its `hero_power`
/// hook, then its cost as it was when used (Gold spent).
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
    // As a Buy, what the power gives arrives before the Gold is paid (`docs/heroes.md` §5).
    let card = hooks(card_id);
    if let Some(hero_power) = card.hero_power {
        let targeted = target_card(state, card.activate_target, target);
        hero_power(state, target, pool, rng);
        // A trinket makes it trigger again (`docs/trinkets.md` §11.4): while its target is
        // still there (the same card in the same place) and a valid one, and it is usable.
        for _ in 0..crate::trinkets::extra_hero_power_triggers(&state.auras) {
            let target_still_ok = target_card(state, card.activate_target, target) == targeted
                && target_ok(state, card.activate_target, target)
                && card.hero_power_usable.is_none_or(|usable| usable(state))
                && match (target, card.hero_power_target) {
                    (Some(t), Some(filter)) => filter(state, t),
                    _ => true,
                };
            if target_still_ok {
                hero_power(state, target, pool, rng);
            }
        }
    }
    state.spend_gold(cost, pool, rng);
    crate::trinkets::after_hero_power(state, pool, rng);
}

/// The card a Hero Power's `target` names in its zone (`kind`): its card id and Golden flag, or
/// `None` for no card.
fn target_card(
    state: &TavernState,
    kind: ActivateTargetKind,
    target: Option<usize>,
) -> Option<(CardId, bool)> {
    let t = target?;
    let unit = match kind {
        ActivateTargetKind::BoardAny => state.board.get(t),
        ActivateTargetKind::ShopCard => state.shop.get(t),
        ActivateTargetKind::HandCard => state.hand.get(t),
        ActivateTargetKind::BoardOrShopMinion if t < 7 => state.board.get(t),
        ActivateTargetKind::BoardOrShopMinion => state.shop.get(t - 7),
        _ => None,
    }?;
    Some((unit.card_id, unit.is_golden))
}

/// Use the player's second Hero Power (`HeroPower2`) on `target`.
pub fn use_power_2(
    state: &mut TavernState,
    target: Option<usize>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    with_second_power(state, |state| use_power(state, target, pool, rng));
}

/// Start of turn (`docs/heroes.md` §3.3, §3.4): the board's marks clear, the hero's own Hero
/// Power's `hero_base_turn_reset` hook runs (whatever power the hero has now), and the Hero
/// Power's uses are reset (1 if it is active, 0 if passive) before its `hero_turn_reset` hook.
pub fn turn_reset(state: &mut TavernState, pool: &mut CardPool) {
    let Some(hero_id) = state.auras.hero.as_ref().map(|h| h.hero_id) else {
        return;
    };
    for unit in &mut state.board {
        unit.marked = false;
    }
    let own_power = data(hero_id).map(|h| h.power.id);
    if let Some(base_reset) = own_power.and_then(|id| hooks(id).hero_base_turn_reset) {
        base_reset(state, pool);
    }
    let extra_uses = crate::trinkets::extra_hero_power_uses(&state.auras);
    let Some(hero) = state.auras.hero.as_mut() else {
        return;
    };
    let card = hooks(hero.power.card_id);
    hero.power.uses_left = active_uses(card, extra_uses);
    if let Some(reset) = card.hero_turn_reset {
        reset(hero);
    }
    if let Some(second) = hero.second_power.as_mut() {
        std::mem::swap(&mut hero.power, second);
        let card = hooks(hero.power.card_id);
        hero.power.uses_left = active_uses(card, extra_uses);
        if let Some(reset) = card.hero_turn_reset {
            reset(hero);
        }
        if let Some(second) = hero.second_power.as_mut() {
            std::mem::swap(&mut hero.power, second);
        }
    }
}

/// `true` if the Hero Power `card_id` is active (it has a `hero_power` hook: the `HeroPower`
/// action uses it), `false` if it is passive.
pub fn is_active(card_id: CardId) -> bool {
    hooks(card_id).hero_power.is_some()
}

/// The uses a Hero Power with hooks `card` has at the start of a turn: 1 if it is active (plus
/// the trinkets' `extra` uses: `docs/trinkets.md` §11.4), 0 if passive.
fn active_uses(card: &CardHooks, extra: u32) -> u32 {
    if card.hero_power.is_some() {
        1 + extra
    } else {
        0
    }
}

/// The Tavern's prices and rules for a player (`docs/tavern.md` §3.3): as written there, unless
/// their Hero Power's `hero_tavern_rules` hook changes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TavernRules {
    /// A minion's price in Gold, before `next_minion_discount`.
    pub minion_cost: u32,
    /// A paid Refresh's price in Gold.
    pub refresh_cost: u32,
    /// Gold an Upgrade costs on top of `upgrade_cost`.
    pub upgrade_extra: u32,
    /// Minions fewer than `shop_capacity` the shop holds.
    pub fewer_shop_minions: usize,
    /// The non-Golden copies a Triple takes.
    pub triple_copies: usize,
    /// Whether a Golden played from the hand gives a `Triple Reward` card (otherwise a
    /// `Tavern Coin`).
    pub golden_gives_triple_reward: bool,
}

impl Default for TavernRules {
    fn default() -> Self {
        Self {
            minion_cost: 3,
            refresh_cost: 1,
            upgrade_extra: 0,
            fewer_shop_minions: 0,
            triple_copies: 3,
            golden_gives_triple_reward: true,
        }
    }
}

/// The player's Tavern rules: the defaults, changed by their Hero Power's `hero_tavern_rules`.
pub fn tavern_rules(auras: &PlayerAuras) -> TavernRules {
    let mut rules = TavernRules::default();
    if let Some(change) = power_hooks(auras).and_then(|c| c.hero_tavern_rules) {
        change(&mut rules);
    }
    if let Some(change) = second_power_hooks(auras).and_then(|c| c.hero_tavern_rules) {
        change(&mut rules);
    }
    rules
}

/// `true` if the player's Hero Power skips their current turn (`hero_skips_turn`).
pub fn skips_turn(state: &TavernState) -> bool {
    power_hooks(&state.auras)
        .and_then(|c| c.hero_skips_turn)
        .is_some_and(|skips| skips(state))
        || second_power_hooks(&state.auras)
            .and_then(|c| c.hero_skips_turn)
            .is_some_and(|skips| skips(state))
}

/// The player got their hero: its Hero Power's `hero_chosen` hook (`docs/heroes.md` §2.3).
pub fn on_chosen(state: &mut TavernState) {
    if let Some(chosen) = power_hooks(&state.auras).and_then(|c| c.hero_chosen) {
        chosen(state);
    }
    if let Some(chosen) = second_power_hooks(&state.auras).and_then(|c| c.hero_chosen) {
        with_second_power(state, chosen);
    }
}

/// Gold was spent: the Hero Power's `hero_gold_spent` hook.
pub fn on_gold_spent(state: &mut TavernState, amount: u32) {
    if let Some(gold_spent) = power_hooks(&state.auras).and_then(|c| c.hero_gold_spent) {
        gold_spent(state, amount);
    }
    if let Some(gold_spent) = second_power_hooks(&state.auras).and_then(|c| c.hero_gold_spent) {
        with_second_power(state, |s| gold_spent(s, amount));
    }
}

/// `true` if the player's Hero Power acts at Start of Combat (`docs/combat.md` §5.1.1 step 1).
pub fn has_start_of_combat(auras: &PlayerAuras) -> bool {
    power_hooks(auras).is_some_and(|c| c.hero_start_of_combat.is_some())
        || second_power_hooks(auras).is_some_and(|c| c.hero_start_of_combat.is_some())
}

/// Run the player's Hero Power(s) `hero_start_of_combat` hook (`docs/heroes.md` §7.2).
pub fn on_start_of_combat(ctx: &mut HeroCombatCtx<'_>) {
    if let Some(soc) = power_hooks(ctx.own.auras).and_then(|c| c.hero_start_of_combat) {
        soc(ctx);
    }
    if let Some(soc) = second_power_hooks(ctx.own.auras).and_then(|c| c.hero_start_of_combat) {
        if let Some(hero) = ctx.own.auras.hero.as_mut() {
            if let Some(second) = hero.second_power.as_mut() {
                std::mem::swap(&mut hero.power, second);
            }
        }
        soc(ctx);
        if let Some(hero) = ctx.own.auras.hero.as_mut() {
            if let Some(second) = hero.second_power.as_mut() {
                std::mem::swap(&mut hero.power, second);
            }
        }
    }
}

/// Room may have opened on the board in combat: the Hero Power's `hero_combat_space` hook.
pub fn combat_space(ctx: &mut BoardCtx<'_>) {
    if let Some(combat_space) = power_hooks(ctx.auras).and_then(|c| c.hero_combat_space) {
        combat_space(ctx);
    }
    if let Some(combat_space) = second_power_hooks(ctx.auras).and_then(|c| c.hero_combat_space) {
        if let Some(hero) = ctx.auras.hero.as_mut() {
            if let Some(second) = hero.second_power.as_mut() {
                std::mem::swap(&mut hero.power, second);
            }
        }
        combat_space(ctx);
        if let Some(hero) = ctx.auras.hero.as_mut() {
            if let Some(second) = hero.second_power.as_mut() {
                std::mem::swap(&mut hero.power, second);
            }
        }
    }
}

/// Start of turn, once the shop is filled: the Hero Power's `hero_turn_start` hook.
pub fn on_turn_start(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(turn_start) = power_hooks(&state.auras).and_then(|c| c.hero_turn_start) {
        turn_start(state, pool, rng);
    }
    if let Some(turn_start) = second_power_hooks(&state.auras).and_then(|c| c.hero_turn_start) {
        with_second_power(state, |s| turn_start(s, pool, rng));
    }
}

/// After a card was bought: the Hero Power's `hero_after_buy` hook.
pub fn after_buy(state: &mut TavernState, bought: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(after_buy) = power_hooks(&state.auras).and_then(|c| c.hero_after_buy) {
        after_buy(state, bought, pool, rng);
    }
    if let Some(after_buy) = second_power_hooks(&state.auras).and_then(|c| c.hero_after_buy) {
        with_second_power(state, |s| after_buy(s, bought, pool, rng));
    }
}

/// After a minion was sold: the Hero Power's `hero_after_sell` hook.
pub fn after_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(after_sell) = power_hooks(&state.auras).and_then(|c| c.hero_after_sell) {
        after_sell(state, sold, pool, rng);
    }
    if let Some(after_sell) = second_power_hooks(&state.auras).and_then(|c| c.hero_after_sell) {
        with_second_power(state, |s| after_sell(s, sold, pool, rng));
    }
}

/// After a minion was played or Magnetized: the Hero Power's `hero_after_play` hook.
pub fn after_play(state: &mut TavernState, played: &Played, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(after_play) = power_hooks(&state.auras).and_then(|c| c.hero_after_play) {
        after_play(state, played, pool, rng);
    }
    if let Some(after_play) = second_power_hooks(&state.auras).and_then(|c| c.hero_after_play) {
        with_second_power(state, |s| after_play(s, played, pool, rng));
    }
}

/// After the Tavern was upgraded: the Hero Power's `hero_after_upgrade` hook.
pub fn after_upgrade(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(after_upgrade) = power_hooks(&state.auras).and_then(|c| c.hero_after_upgrade) {
        after_upgrade(state, pool, rng);
    }
    if let Some(after_upgrade) = second_power_hooks(&state.auras).and_then(|c| c.hero_after_upgrade)
    {
        with_second_power(state, |s| after_upgrade(s, pool, rng));
    }
}

/// The player chose an option of a Discover their Hero Power keeps (`Unit::to_hero_power`,
/// `docs/tavern.md` §5.2): its `hero_option_chosen` hook (or installs `second_power` for Genn's
/// second Discover, marked by `counter == 1` on a Hero Power option), or else the chosen card goes
/// to the Hero Power's `stored` cards and the others are discarded.
pub fn keep_chosen(
    state: &mut TavernState,
    chosen: Unit,
    unchosen: Vec<Unit>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let is_power = HEROES.iter().any(|h| h.power.id == chosen.card_id);
    if chosen.counter == SECOND_POWER_OPTION && is_power {
        install_power(state, chosen.card_id, true, pool, rng);
        return;
    }
    if chosen.counter == REPLACING_POWER_OPTION && is_power {
        install_power(state, chosen.card_id, false, pool, rng);
        return;
    }
    let for_second = state
        .auras
        .hero
        .as_ref()
        .and_then(|h| h.second_power.as_ref())
        .is_some_and(|sp| chosen.taught_spell_id == Some(sp.card_id));
    if for_second {
        if let Some(option_chosen) =
            second_power_hooks(&state.auras).and_then(|c| c.hero_option_chosen)
        {
            with_second_power(state, |s| option_chosen(s, chosen, unchosen, pool, rng));
            return;
        }
    }
    if let Some(option_chosen) = power_hooks(&state.auras).and_then(|c| c.hero_option_chosen) {
        option_chosen(state, chosen, unchosen, pool, rng);
    } else if let Some(option_chosen) =
        second_power_hooks(&state.auras).and_then(|c| c.hero_option_chosen)
    {
        with_second_power(state, |s| option_chosen(s, chosen, unchosen, pool, rng));
    } else if let Some(power) = power_mut(&mut state.auras) {
        power.stored.push(chosen);
    }
}

/// A friendly minion attacks: the Hero Power's `hero_friendly_attack` hook (`docs/heroes.md` §4).
pub fn on_friendly_attack(ctx: &mut BoardCtx<'_>, attacker_id: UnitId) {
    if let Some(friendly_attack) = power_hooks(ctx.auras).and_then(|c| c.hero_friendly_attack) {
        friendly_attack(ctx, attacker_id);
    }
    if let Some(friendly_attack) =
        second_power_hooks(ctx.auras).and_then(|c| c.hero_friendly_attack)
    {
        if let Some(hero) = ctx.auras.hero.as_mut() {
            if let Some(second) = hero.second_power.as_mut() {
                std::mem::swap(&mut hero.power, second);
            }
        }
        friendly_attack(ctx, attacker_id);
        if let Some(hero) = ctx.auras.hero.as_mut() {
            if let Some(second) = hero.second_power.as_mut() {
                std::mem::swap(&mut hero.power, second);
            }
        }
    }
}

/// A friendly minion dealt damage: the Hero Power's `hero_friendly_damage_dealt` hook
/// (`docs/heroes.md` §4).
pub fn on_friendly_damage_dealt(ctx: &mut BoardCtx<'_>, dealer_id: UnitId, amount: i32) {
    if let Some(dmg_dealt) = power_hooks(ctx.auras).and_then(|c| c.hero_friendly_damage_dealt) {
        dmg_dealt(ctx, dealer_id, amount);
    }
    if let Some(dmg_dealt) =
        second_power_hooks(ctx.auras).and_then(|c| c.hero_friendly_damage_dealt)
    {
        if let Some(hero) = ctx.auras.hero.as_mut() {
            if let Some(second) = hero.second_power.as_mut() {
                std::mem::swap(&mut hero.power, second);
            }
        }
        dmg_dealt(ctx, dealer_id, amount);
        if let Some(hero) = ctx.auras.hero.as_mut() {
            if let Some(second) = hero.second_power.as_mut() {
                std::mem::swap(&mut hero.power, second);
            }
        }
    }
}

/// The combat's results are back in the Tavern: the Hero Power's `hero_after_combat` hook.
pub fn after_combat(state: &mut TavernState, dead: &[Unit]) {
    if let Some(after_combat) = power_hooks(&state.auras).and_then(|c| c.hero_after_combat) {
        after_combat(state, dead);
    }
    if let Some(after_combat) = second_power_hooks(&state.auras).and_then(|c| c.hero_after_combat) {
        with_second_power(state, |s| after_combat(s, dead));
    }
}

/// The player ends their turn: the Hero Power's `hero_end_turn` hook (`docs/heroes.md` §4).
pub fn end_turn(state: &mut TavernState, repeats: u32, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(end_turn) = power_hooks(&state.auras).and_then(|c| c.hero_end_turn) {
        end_turn(state, repeats, pool, rng);
    }
    if let Some(end_turn) = second_power_hooks(&state.auras).and_then(|c| c.hero_end_turn) {
        with_second_power(state, |s| end_turn(s, repeats, pool, rng));
    }
}

/// The Tavern was Refreshed: the Hero Power's `hero_after_refresh` hook (`docs/heroes.md` §4).
pub fn after_refresh(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    if let Some(after_refresh) = power_hooks(&state.auras).and_then(|c| c.hero_after_refresh) {
        after_refresh(state, pool, rng);
    }
    if let Some(after_refresh) = second_power_hooks(&state.auras).and_then(|c| c.hero_after_refresh)
    {
        with_second_power(state, |s| after_refresh(s, pool, rng));
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
    if let Some(second) = hero.second_power.as_ref() {
        if let Some(combat_summon) = hooks(second.card_id).hero_combat_summon {
            let swapped = HeroState {
                hero_id: hero.hero_id,
                power: second.clone(),
                second_power: None,
            };
            combat_summon(&swapped, summoned);
        }
    }
}

/// `true` if the player's Hero Power reacts to their minions' deaths in combat.
pub fn has_friendly_death(auras: &PlayerAuras) -> bool {
    power_hooks(auras).is_some_and(|c| c.hero_friendly_death.is_some())
        || second_power_hooks(auras).is_some_and(|c| c.hero_friendly_death.is_some())
}

/// A friendly minion died in combat: the Hero Power's `hero_friendly_death` hook.
pub fn on_friendly_death(ctx: &mut BoardCtx<'_>, dying: &Unit) {
    if let Some(friendly_death) = power_hooks(ctx.auras).and_then(|c| c.hero_friendly_death) {
        friendly_death(ctx, dying);
    }
    if let Some(friendly_death) = second_power_hooks(ctx.auras).and_then(|c| c.hero_friendly_death)
    {
        if let Some(hero) = ctx.auras.hero.as_mut() {
            if let Some(second) = hero.second_power.as_mut() {
                std::mem::swap(&mut hero.power, second);
            }
        }
        friendly_death(ctx, dying);
        if let Some(hero) = ctx.auras.hero.as_mut() {
            if let Some(second) = hero.second_power.as_mut() {
                std::mem::swap(&mut hero.power, second);
            }
        }
    }
}

/// Duos: the player's warband never stepped in (`docs/heroes.md` §7.1): the card their Hero
/// Power's `hero_sat_out` hook gives, if any.
pub fn on_sat_out(auras: &mut PlayerAuras, hero_tier: u32, rng: &mut Rng) -> Option<Unit> {
    if let Some(card) = power_hooks(auras)
        .and_then(|c| c.hero_sat_out)
        .and_then(|sat_out| sat_out(auras, hero_tier, rng))
    {
        return Some(card);
    }
    if let Some(sat_out) = second_power_hooks(auras).and_then(|c| c.hero_sat_out) {
        return with_second_power_auras(auras, |a| sat_out(a, hero_tier, rng)).flatten();
    }
    None
}

/// `true` if the player's Hero Power reacts to their minions' kills (`hero_enemy_killed`).
pub fn has_enemy_killed(auras: &PlayerAuras) -> bool {
    power_hooks(auras).is_some_and(|c| c.hero_enemy_killed.is_some())
        || second_power_hooks(auras).is_some_and(|c| c.hero_enemy_killed.is_some())
}

/// The player's minion `killer` killed the enemy minion `victim` (`docs/heroes.md` §7.3).
pub fn on_enemy_killed(ctx: &mut BoardCtx<'_>, killer: UnitId, victim: &Unit) {
    if let Some(enemy_killed) = power_hooks(ctx.auras).and_then(|c| c.hero_enemy_killed) {
        enemy_killed(ctx, killer, victim);
    }
    if let Some(enemy_killed) = second_power_hooks(ctx.auras).and_then(|c| c.hero_enemy_killed) {
        if let Some(hero) = ctx.auras.hero.as_mut() {
            if let Some(second) = hero.second_power.as_mut() {
                std::mem::swap(&mut hero.power, second);
            }
        }
        enemy_killed(ctx, killer, victim);
        if let Some(hero) = ctx.auras.hero.as_mut() {
            if let Some(second) = hero.second_power.as_mut() {
                std::mem::swap(&mut hero.power, second);
            }
        }
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

/// `hero_turn_start` of the Hero Powers that act once per combat once unlocked (Vanndar,
/// Drek'Thar): the game's turn (`counter`), and from `UNLOCK_TURN` on the coming combat is armed.
pub fn arm_from_turn(state: &mut TavernState, _: &mut CardPool, _: &mut Rng) {
    let turn = state.turn as i32;
    if let Some(power) = power_mut(&mut state.auras) {
        power.counter = turn;
        power.armed = u32::from(turn >= UNLOCK_TURN);
    }
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

/// A `hero_power_usable` hook: the hand has room for a card (`docs/heroes.md` §11.2).
pub fn hand_has_room(state: &TavernState) -> bool {
    state.hand.len() < MAX_HAND_SIZE
}

/// A `hero_power_usable` hook for once-per-game powers: not used yet (`stacks` is set to 1 by the
/// use).
pub fn unused_this_game(state: &TavernState) -> bool {
    active_power(state).is_some_and(|p| p.stacks == 0)
}

/// Ids [`identify_minion`] gives stay below this, clear of the temporary ids the engine uses while
/// it resolves an effect (`cards::with_tavern_ctx`, a Battlecry's target).
const PICK_ID_LIMIT: UnitId = 1 << 29;

/// Give the minion at `BoardOrShopMinion` target `target` (`0..7` board, `7..14` shop) an id no
/// other card on the board, in the shop or in the hand has, and return it (`docs/heroes.md` §3.2):
/// Tavern cards have no distinct ids otherwise, and a Hero Power that picks a minion
/// (`HeroPowerState::picked`) finds it again by that id after the board or the shop changed.
pub fn identify_minion(state: &mut TavernState, target: usize) -> Option<UnitId> {
    let id = minion_at(state, target)?.id;
    let cards = || state.board.iter().chain(&state.hand).chain(&state.shop);
    let distinct = id != 0 && id < PICK_ID_LIMIT && cards().filter(|u| u.id == id).count() == 1;
    let id = if distinct {
        id
    } else {
        1 + cards()
            .map(|u| u.id)
            .filter(|&id| id < PICK_ID_LIMIT)
            .max()
            .unwrap_or(0)
    };
    let unit = if target < 7 {
        state.board.get_mut(target)
    } else {
        state.shop.get_mut(target - 7)
    };
    unit?.id = id;
    Some(id)
}

/// The minion at `BoardOrShopMinion` target `target` (a shop spell is none).
fn minion_at(state: &TavernState, target: usize) -> Option<&Unit> {
    if target < 7 {
        state.board.get(target)
    } else {
        state.shop.get(target - 7).filter(|u| !u.is_spell)
    }
}

/// The `BoardOrShopMinion` target of the minion whose id is `id`, if one is on the board or in
/// the shop (a minion [`identify_minion`] gave that id).
pub fn minion_target(state: &TavernState, id: UnitId) -> Option<usize> {
    state.board.iter().position(|u| u.id == id).or_else(|| {
        state
            .shop
            .iter()
            .position(|u| u.id == id && !u.is_spell)
            .map(|i| 7 + i)
    })
}

/// The observation's `hero_power_armed` for `power` (`docs/heroes.md` §8): its picked minion's
/// current target + 1 (0 once the pick dropped), or else its `armed`.
pub fn armed_feature(state: &TavernState, power: &HeroPowerState) -> u32 {
    match power.picked {
        Some(id) => minion_target(state, id).map_or(0, |t| t as u32 + 1),
        None => power.armed,
    }
}

/// The player's Hero Power state (for hooks that know the player has a hero).
pub fn power_mut(auras: &mut PlayerAuras) -> Option<&mut HeroPowerState> {
    auras.hero.as_mut().map(|h| &mut h.power)
}

/// The player's Hero Power state matching `card_id` (`power` or `second_power`), falling back to
/// `power` (`docs/heroes.md` §2.2).
pub fn power_mut_for(auras: &mut PlayerAuras, card_id: CardId) -> Option<&mut HeroPowerState> {
    let hero = auras.hero.as_mut()?;
    if hero.power.card_id == card_id {
        return Some(&mut hero.power);
    }
    if hero
        .second_power
        .as_ref()
        .is_some_and(|sp| sp.card_id == card_id)
    {
        return hero.second_power.as_mut();
    }
    Some(&mut hero.power)
}

/// The Hero Powers a Discover of Hero Powers can offer (`Sir Finley Mrrgglton`, `Master Nguyen`,
/// `Genn, Worgen King`; `docs/heroes.md` §11.2): the implemented ones (Duos-only ones only in
/// Duos; the Dark Gift ones only with `dark_gifts_enabled`) but `exclude`, the player's current
/// ones and those flagged [`CardFlags::NOT_DISCOVERABLE_POWER`]. For a single turn (`temporary`: Master Nguyen), not those
/// flagged [`CardFlags::NOT_A_TURN_POWER`] either, nor an active one its `hero_power_usable` hook
/// says can't be used now.
pub fn offerable_powers(
    state: &TavernState,
    exclude: Option<CardId>,
    temporary: bool,
) -> Vec<&'static HeroData> {
    let duos = state.auras.duos_game;
    let current: Vec<CardId> = state.hero().map_or_else(Vec::new, |h| {
        std::iter::once(h.power.card_id)
            .chain(h.second_power.iter().map(|p| p.card_id))
            .collect()
    });
    HEROES
        .iter()
        .filter(|h| is_implemented(h.id) && (duos || !h.duos_only))
        .filter(|h| exclude != Some(h.power.id) && !current.contains(&h.power.id))
        .filter(|h| {
            let card = hooks(h.power.id);
            !card.has(CardFlags::NOT_DISCOVERABLE_POWER)
                && (state.dark_gifts_enabled || !card.has(CardFlags::DARK_GIFTS))
                && (!temporary
                    || (!card.has(CardFlags::NOT_A_TURN_POWER)
                        && card.hero_power_usable.is_none_or(|usable| usable(state))))
        })
        .collect()
}

/// Offer a Discover of `count` distinct Hero Powers (`Sir Finley`, `Master Nguyen`, `Genn`).
pub fn offer_power_discover(
    state: &mut TavernState,
    count: usize,
    exclude: Option<CardId>,
    temporary: bool,
    for_second_power: bool,
    rng: &mut Rng,
) {
    let mut pool = offerable_powers(state, exclude, temporary);
    let mut options = Vec::with_capacity(count);
    for _ in 0..count {
        if pool.is_empty() {
            break;
        }
        let h = pool.remove(rng.below(pool.len()));
        let mut opt = Unit::new(h.power.name, 0, 1).with_card_id(h.power.id);
        opt.is_spell = true;
        opt.spell_cost = h.power.cost;
        opt.to_hero_power = true;
        if for_second_power {
            opt.counter = SECOND_POWER_OPTION;
        }
        options.push(opt);
    }
    state.push_discover(options);
}

/// The `counter` of a Hero Power Discover option whose chosen power becomes the second Hero Power
/// (Genn's second Discover).
pub const SECOND_POWER_OPTION: i32 = 1;
/// The `counter` of a Hero Power Discover option whose chosen power replaces the Hero Power,
/// whatever it is (`Training Session`; `docs/darkmoon_prizes.md` §4.3).
pub const REPLACING_POWER_OPTION: i32 = 2;

/// Offer a Discover of `count` distinct Hero Powers ([`offer_power_discover`]) whose chosen one
/// replaces the player's Hero Power ([`install_power`]), whatever their Hero Power is
/// (`Training Session`).
pub fn offer_replacing_power_discover(state: &mut TavernState, count: usize, rng: &mut Rng) {
    let pending_before = state.discover_pending.is_some();
    let queued_before = state.discover_queue.len();
    offer_power_discover(state, count, None, false, false, rng);
    let options = if !pending_before {
        state.discover_pending.as_mut()
    } else if state.discover_queue.len() > queued_before {
        state.discover_queue.last_mut()
    } else {
        None
    };
    for option in options.into_iter().flatten() {
        option.counter = REPLACING_POWER_OPTION;
    }
}

/// Replace the player's Hero Power (`hero.power`, or `hero.second_power` if `into_second`) with
/// `new` (`docs/heroes.md` §4): the outgoing power's `hero_power_removed` hook cleans up what it
/// keeps.
pub fn replace_power(
    state: &mut TavernState,
    new: HeroPowerState,
    into_second: bool,
    pool: &mut CardPool,
) {
    let Some(hero) = state.auras.hero.as_mut() else {
        return;
    };
    let old = if into_second {
        hero.second_power.replace(new)
    } else {
        Some(std::mem::replace(&mut hero.power, new))
    };
    if let Some(mut old) = old {
        if let Some(removed) = hooks(old.card_id).hero_power_removed {
            removed(state, &mut old, pool);
        }
    }
}

/// Install the Hero Power `power_card_id` into `hero.power` (or `hero.second_power` if
/// `into_second`, [`replace_power`]), running its `hero_chosen`, `hero_turn_reset`, and
/// `hero_turn_start` hooks.
pub fn install_power(
    state: &mut TavernState,
    power_card_id: CardId,
    into_second: bool,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(h) = HEROES.iter().find(|h| h.power.id == power_card_id) else {
        return;
    };
    replace_power(state, new_power_state(h.power), into_second, pool);
    if into_second {
        with_second_power(state, |state| {
            let card = hooks(power_card_id);
            if let Some(chosen) = card.hero_chosen {
                chosen(state);
            }
            if let Some(reset) = card.hero_turn_reset {
                if let Some(hero) = state.auras.hero.as_mut() {
                    reset(hero);
                }
            }
            if let Some(turn_start) = card.hero_turn_start {
                turn_start(state, pool, rng);
            }
        });
    } else {
        let card = hooks(power_card_id);
        if let Some(chosen) = card.hero_chosen {
            chosen(state);
        }
        if let Some(reset) = card.hero_turn_reset {
            if let Some(hero) = state.auras.hero.as_mut() {
                reset(hero);
            }
        }
        if let Some(turn_start) = card.hero_turn_start {
            turn_start(state, pool, rng);
        }
    }
    state.sync_all_auras();
}

/// Duos: the team's turn is about to start, before either player's start of turn
/// (`docs/heroes.md` §4): each player's Hero Powers' `hero_before_team_turn` hook, with their
/// teammate's Tavern, player 0's first.
pub fn before_team_turn(players: &mut [TavernState; 2]) {
    for player in 0..2 {
        let (left, right) = players.split_at_mut(1);
        let (own, mate) = if player == 0 {
            (&mut left[0], &right[0])
        } else {
            (&mut right[0], &left[0])
        };
        if let Some(f) = power_hooks(&own.auras).and_then(|c| c.hero_before_team_turn) {
            f(own, mate);
        }
        if let Some(f) = second_power_hooks(&own.auras).and_then(|c| c.hero_before_team_turn) {
            with_second_power(own, |s| f(s, mate));
        }
    }
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

    /// A Tavern whose player has the hero `name`, on turn `turn`.
    fn tavern_with(name: &str, turn: u32) -> TavernState {
        let mut state = TavernState::new();
        state
            .set_hero(by_name(name).expect("a hero").id)
            .expect("an implemented hero");
        state.turn = turn;
        state
    }

    fn offered(state: &TavernState, temporary: bool) -> Vec<&'static str> {
        offerable_powers(state, None, temporary)
            .iter()
            .map(|h| h.name)
            .collect()
    }

    #[test]
    fn hero_power_discovers_never_offer_the_choosers_skippers_cho_or_gall() {
        let never = [
            "Sir Finley Mrrgglton",
            "Master Nguyen",
            "Genn, Worgen King",
            "The Nameless One",
            "A. F. Kay",
            "Ambassador Faelin",
            "Cho",
            "Gall",
        ];
        let mut duos = tavern_with("Sir Finley Mrrgglton", 1);
        duos.auras.duos_game = true;
        for (state, temporary) in [
            (tavern_with("Sir Finley Mrrgglton", 1), false),
            (tavern_with("Genn, Worgen King", 4), false),
            (tavern_with("Master Nguyen", 5), true),
            (duos, false),
        ] {
            let offered = offered(&state, temporary);
            for name in never {
                assert!(!offered.contains(&name), "{name} is offered");
            }
        }
    }

    #[test]
    fn finley_and_genn_may_get_the_start_of_game_powers_nguyen_never() {
        let start_of_game = [
            "Patchwerk",
            "N'Zoth",
            "Sneed",
            "The Curator",
            "Thorim, Stormlord",
        ];
        for (name, turn) in [("Sir Finley Mrrgglton", 1), ("Genn, Worgen King", 4)] {
            let offered = offered(&tavern_with(name, turn), false);
            assert!(start_of_game.iter().all(|p| offered.contains(p)), "{name}");
        }
        let nguyen = offered(&tavern_with("Master Nguyen", 5), true);
        assert!(start_of_game.iter().all(|p| !nguyen.contains(p)));
        assert!(!nguyen.contains(&"Cariel Roame"));
        for simple in [
            "Millhouse Manastorm",
            "Sindragosa",
            "Ysera",
            "Cookie the Cook",
        ] {
            assert!(nguyen.contains(&simple), "{simple}");
        }
    }

    #[test]
    fn offers_are_distinct_across_the_lobby() {
        let mut rng = Rng::new(7);
        let offers = draw_offers(4, OFFER_COUNT, false, false, &mut rng);
        let all: Vec<HeroId> = offers.iter().flatten().copied().collect();
        let mut unique = all.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(all.len(), unique.len());
        assert!(offers.iter().all(|o| o.len() == OFFER_COUNT));
        assert!(all.iter().all(|&id| !data(id).unwrap().duos_only));
    }
}
