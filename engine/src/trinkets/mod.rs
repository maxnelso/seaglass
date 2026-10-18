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
mod vocab;

pub mod accord_o_tron_portrait;
pub mod aggem_sticker;
pub mod all_purpose_kibble;
pub mod ancient_wishbone;
pub mod artisanal_urn;
pub mod azerite_portrait;
pub mod azeroth_model_globe;
pub mod baleful_incense;
pub mod baller_portrait;
pub mod baller_portrait_greater;
pub mod bartend_o_trons_oilcan;
pub mod bassgill_portrait;
pub mod bazaar_sticker;
pub mod beatboxer_portrait;
pub mod beetle_band;
pub mod beetle_band_greater;
pub mod behemoth_portrait;
pub mod bleeding_heart;
pub mod blood_golem_sticker;
pub mod bloodbound_earrings;
pub mod bloodbound_earrings_greater;
pub mod bloodfury_shield;
pub mod bluegill_flippers;
pub mod bob_blehead;
pub mod bobs_tip_jar;
pub mod book_of_medivh;
pub mod book_of_medivh_greater;
pub mod booms_monster_portrait;
pub mod booty_bay_brew;
pub mod booty_bay_brew_greater;
pub mod bottle_sticker;
pub mod bronze_timepiece;
pub mod bronzebeard_portrait;
pub mod butchers_sickle;
pub mod chillmere_mosaic;
pub mod chogall_sticker;
pub mod chromatic_tear;
pub mod chromatic_tear_greater;
pub mod colorful_compass;
pub mod colorful_compass_greater;
pub mod comfy_coffin;
pub mod comfy_coffin_greater;
pub mod common_thread;
pub mod conch_portrait;
pub mod consuming_claw;
pub mod converter_portrait;
pub mod cookies_stirring_rod;
pub mod corrupted_baton;
pub mod corrupted_baton_greater;
pub mod corrupted_coin_portrait;
pub mod corrupted_tome;
pub mod curator_sticker;
pub mod cursed_crystal;
pub mod czarina_portrait;
pub mod dalaran_cheese_wheel;
pub mod deathly_phylactery;
pub mod defensive_shell;
pub mod demonic_bloodletter;
pub mod demonic_tapestry;
pub mod designer_eyepatch;
pub mod dragon_skull;
pub mod dragonhead_staff;
pub mod dragons_eye;
pub mod dragonwing_glider;
pub mod dramaloc_sticker;
pub mod electrode_attractor;
pub mod emergency_gearblade;
pub mod empowered_mirror;
pub mod empowered_mirror_greater;
pub mod enigmatic_headstone;
pub mod escapee_portrait;
pub mod eternal_portrait;
pub mod eternal_portrait_greater;
pub mod evil_experiment;
pub mod explorers_binoculars;
pub mod faerie_dragon_scale;
pub mod fang_anklet;
pub mod felsteel_cleaver;
pub mod fish_portrait;
pub mod flaming_portrait;
pub mod flighty_portrait;
pub mod floating_candle_set;
pub mod fridge_magnet;
pub mod frigid_blossom;
pub mod funeral_wreath;
pub mod ghastly_sticker;
pub mod glass_of_perspective;
pub mod glass_of_perspective_greater;
pub mod glowing_crystal;
pub mod glowing_gauntlet;
pub mod goblin_wallet;
pub mod gold_mallet;
pub mod gold_mallet_greater;
pub mod gold_pendant;
pub mod gold_plated_compass;
pub mod goldenizer_supply;
pub mod great_boar_sticker;
pub mod great_boar_sticker_greater;
pub mod grifter_portrait;
pub mod guiding_candle;
pub mod hackerfin_portrait;
pub mod hammer_of_twilight;
pub mod hammer_of_twilight_greater;
pub mod heart_of_the_forest;
pub mod herald_sticker;
pub mod herding_horn;
pub mod hoggy_bank;
pub mod holy_mallet;
pub mod honeycomb_ring;
pub mod inductive_gyroblade;
pub mod inductive_gyroblade_greater;
pub mod innkeepers_hearth;
pub mod innkeepers_stein;
pub mod insurrectionists_blade;
pub mod ironforge_anvil;
pub mod jailer_sticker;
pub mod jailer_sticker_greater;
pub mod jar_o_gems;
pub mod jarred_frostling;
pub mod jewelry_box;
pub mod lava_lamp;
pub mod lens_case;
pub mod lightfeather_sticker;
pub mod lionfish_portrait;
pub mod lockbox_portrait;
pub mod lovely_locket;
pub mod lucky_tabby;
pub mod magic_road_sign;
pub mod magicfin_sticker;
pub mod magicians_top_hat;
pub mod makeshift_master;
pub mod maldraxxus_dagger;
pub mod mama_bear_sticker;
pub mod manipulator_portrait;
pub mod mask_of_ancient_ones;
pub mod maw_caster_portrait;
pub mod mecha_jaraxxus_sticker;
pub mod mindslasher_portrait;
pub mod miniature_ship;
pub mod minion_bait;
pub mod morgl_portrait;
pub mod moveout_box;
pub mod multilayered_shield;
pub mod murky_sticker;
pub mod mysterious_orb;
pub mod mystery_cube;
pub mod nerglish_phrasebook;
pub mod nerglish_phrasebook_greater;
pub mod nether_pendant;
pub mod nomi_sticker;
pub mod nomi_sticker_greater;
pub mod ominous_stone;
pub mod ophidian_staff;
pub mod orb_of_the_unknown;
pub mod orb_of_the_unknown_greater;
pub mod orc_estra_portrait;
pub mod ornate_clock;
pub mod pagles_fishing_rod;
pub mod pilgrimp_sticker;
pub mod plaguerunner_portrait;
pub mod pocket_cyclone;
pub mod pocket_cyclone_greater;
pub mod portable_factory;
pub mod portable_factory_greater;
pub mod privateer_portrait;
pub mod protective_ring;
pub mod quartermasters_hook;
pub mod quilligraphy_set;
pub mod quilligraphy_set_greater;
pub mod rascal_sticker;
pub mod recycling_sticker;
pub mod reflective_pendant;
pub mod reinforced_shield;
pub mod reinvigorating_light;
pub mod rendle_sticker;
pub mod replica_cathedral;
pub mod reusable_batteries;
pub mod rewinder_portrait;
pub mod rivendare_portrait;
pub mod rockin_music_box;
pub mod ruby_tusk;
pub mod safety_patch;
pub mod scraper_sticker;
pub mod secret_schematic;
pub mod sellemental_portrait;
pub mod shark_cannon;
pub mod shathyar_shrine;
pub mod shrine_of_evolution;
pub mod sinister_invitation;
pub mod sinstone_sticker;
pub mod sky_golem_portrait;
pub mod slamma_sticker;
pub mod sludge_portrait;
pub mod snarky_portrait;
pub mod soul_battery;
pub mod soul_fermenter;
pub mod sous_chef_sticker;
pub mod souvenir_stand;
pub mod spell_powered_wrench;
pub mod sphere_of_memory;
pub mod splinter_of_aurum;
pub mod staff_of_the_scourge;
pub mod stormcoil_sticker;
pub mod stuffed_coin_purse;
pub mod summoning_sphere;
pub mod sunken_anchor;
pub mod surveyor_portrait;
pub mod tableware_portrait;
pub mod the_eye_of_dalaran;
pub mod the_eye_of_sargeras;
pub mod tickatus_sticker;
pub mod token_of_the_old_gods;
pub mod tome_of_the_ancients;
pub mod trailblazer_sticker;
pub mod training_certificate;
pub mod transcribing_typewriter;
pub mod transcribing_typewriter_greater;
pub mod tricksters_sleeve;
pub mod trip_vouchers;
pub mod trusty_crowbar;
pub mod upstart_embers;
pub mod urzul_sticker;
pub mod vibrant_bubble;
pub mod volumizer_portrait;
pub mod volumizer_portrait_greater;
pub mod wand_of_divination;
pub mod war_drum;
pub mod warband_whistle;
pub mod warcry_totem;
pub mod water_wheel;
pub mod wax_lance;
pub mod weighted_gauntlet;
pub mod wicked_tome;
pub mod wildfeather_duster;
pub mod willbreaker_sticker;
pub mod wolfhead_flail;
pub mod worn_treasure_map;
pub mod writhing_tentacles;
pub mod young_murk_eye_sticker;

pub use table::{CARD_DEFS_BUILD, TRINKETS};
pub use vocab::vocabulary;

use crate::cards::{self, hooks, BoardCtx, CardFlags, CardHooks, HeroCombatCtx, Passive, Played};
use crate::model::{
    CardId, PlayerAuras, Tribe, TrinketOption, TrinketState, Unit, UnitId, LOBBY_TRIBES,
};
use crate::rng::Rng;
use crate::tavern::{BuyPrice, CardPool, MinionSlot, RefreshKind, TavernState};

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
    (
        accord_o_tron_portrait::ID,
        accord_o_tron_portrait::behaviors,
    ),
    (aggem_sticker::ID, aggem_sticker::behaviors),
    (all_purpose_kibble::ID, all_purpose_kibble::behaviors),
    (ancient_wishbone::ID, ancient_wishbone::behaviors),
    (artisanal_urn::ID, artisanal_urn::behaviors),
    (azerite_portrait::ID, azerite_portrait::behaviors),
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
    (bassgill_portrait::ID, bassgill_portrait::behaviors),
    (bazaar_sticker::ID, bazaar_sticker::behaviors),
    (beatboxer_portrait::ID, beatboxer_portrait::behaviors),
    (beetle_band::ID, beetle_band::behaviors),
    (beetle_band_greater::ID, beetle_band_greater::behaviors),
    (behemoth_portrait::ID, behemoth_portrait::behaviors),
    (bleeding_heart::ID, bleeding_heart::behaviors),
    (blood_golem_sticker::ID, blood_golem_sticker::behaviors),
    (bloodbound_earrings::ID, bloodbound_earrings::behaviors),
    (
        bloodbound_earrings_greater::ID,
        bloodbound_earrings_greater::behaviors,
    ),
    (bloodfury_shield::ID, bloodfury_shield::behaviors),
    (bluegill_flippers::ID, bluegill_flippers::behaviors),
    (bob_blehead::ID, bob_blehead::behaviors),
    (bobs_tip_jar::ID, bobs_tip_jar::behaviors),
    (book_of_medivh::ID, book_of_medivh::behaviors),
    (
        book_of_medivh_greater::ID,
        book_of_medivh_greater::behaviors,
    ),
    (
        booms_monster_portrait::ID,
        booms_monster_portrait::behaviors,
    ),
    (booty_bay_brew::ID, booty_bay_brew::behaviors),
    (
        booty_bay_brew_greater::ID,
        booty_bay_brew_greater::behaviors,
    ),
    (bottle_sticker::ID, bottle_sticker::behaviors),
    (bronze_timepiece::ID, bronze_timepiece::behaviors),
    (bronzebeard_portrait::ID, bronzebeard_portrait::behaviors),
    (butchers_sickle::ID, butchers_sickle::behaviors),
    (chillmere_mosaic::ID, chillmere_mosaic::behaviors),
    (chogall_sticker::ID, chogall_sticker::behaviors),
    (chromatic_tear::ID, chromatic_tear::behaviors),
    (
        chromatic_tear_greater::ID,
        chromatic_tear_greater::behaviors,
    ),
    (colorful_compass::ID, colorful_compass::behaviors),
    (
        colorful_compass_greater::ID,
        colorful_compass_greater::behaviors,
    ),
    (comfy_coffin::ID, comfy_coffin::behaviors),
    (comfy_coffin_greater::ID, comfy_coffin_greater::behaviors),
    (common_thread::ID, common_thread::behaviors),
    (conch_portrait::ID, conch_portrait::behaviors),
    (consuming_claw::ID, consuming_claw::behaviors),
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
    (corrupted_tome::ID, corrupted_tome::behaviors),
    (curator_sticker::ID, curator_sticker::behaviors),
    (cursed_crystal::ID, cursed_crystal::behaviors),
    (czarina_portrait::ID, czarina_portrait::behaviors),
    (dalaran_cheese_wheel::ID, dalaran_cheese_wheel::behaviors),
    (deathly_phylactery::ID, deathly_phylactery::behaviors),
    (defensive_shell::ID, defensive_shell::behaviors),
    (demonic_bloodletter::ID, demonic_bloodletter::behaviors),
    (demonic_tapestry::ID, demonic_tapestry::behaviors),
    (designer_eyepatch::ID, designer_eyepatch::behaviors),
    (dragon_skull::ID, dragon_skull::behaviors),
    (dragonhead_staff::ID, dragonhead_staff::behaviors),
    (dragons_eye::ID, dragons_eye::behaviors),
    (dragonwing_glider::ID, dragonwing_glider::behaviors),
    (dramaloc_sticker::ID, dramaloc_sticker::behaviors),
    (electrode_attractor::ID, electrode_attractor::behaviors),
    (emergency_gearblade::ID, emergency_gearblade::behaviors),
    (empowered_mirror::ID, empowered_mirror::behaviors),
    (
        empowered_mirror_greater::ID,
        empowered_mirror_greater::behaviors,
    ),
    (enigmatic_headstone::ID, enigmatic_headstone::behaviors),
    (escapee_portrait::ID, escapee_portrait::behaviors),
    (eternal_portrait::ID, eternal_portrait::behaviors),
    (
        eternal_portrait_greater::ID,
        eternal_portrait_greater::behaviors,
    ),
    (evil_experiment::ID, evil_experiment::behaviors),
    (explorers_binoculars::ID, explorers_binoculars::behaviors),
    (faerie_dragon_scale::ID, faerie_dragon_scale::behaviors),
    (fang_anklet::ID, fang_anklet::behaviors),
    (felsteel_cleaver::ID, felsteel_cleaver::behaviors),
    (fish_portrait::ID, fish_portrait::behaviors),
    (flaming_portrait::ID, flaming_portrait::behaviors),
    (flighty_portrait::ID, flighty_portrait::behaviors),
    (floating_candle_set::ID, floating_candle_set::behaviors),
    (fridge_magnet::ID, fridge_magnet::behaviors),
    (frigid_blossom::ID, frigid_blossom::behaviors),
    (funeral_wreath::ID, funeral_wreath::behaviors),
    (ghastly_sticker::ID, ghastly_sticker::behaviors),
    (glass_of_perspective::ID, glass_of_perspective::behaviors),
    (
        glass_of_perspective_greater::ID,
        glass_of_perspective_greater::behaviors,
    ),
    (glowing_crystal::ID, glowing_crystal::behaviors),
    (glowing_gauntlet::ID, glowing_gauntlet::behaviors),
    (goblin_wallet::ID, goblin_wallet::behaviors),
    (gold_mallet::ID, gold_mallet::behaviors),
    (gold_mallet_greater::ID, gold_mallet_greater::behaviors),
    (gold_pendant::ID, gold_pendant::behaviors),
    (gold_plated_compass::ID, gold_plated_compass::behaviors),
    (goldenizer_supply::ID, goldenizer_supply::behaviors),
    (great_boar_sticker::ID, great_boar_sticker::behaviors),
    (
        great_boar_sticker_greater::ID,
        great_boar_sticker_greater::behaviors,
    ),
    (grifter_portrait::ID, grifter_portrait::behaviors),
    (guiding_candle::ID, guiding_candle::behaviors),
    (hackerfin_portrait::ID, hackerfin_portrait::behaviors),
    (hammer_of_twilight::ID, hammer_of_twilight::behaviors),
    (
        hammer_of_twilight_greater::ID,
        hammer_of_twilight_greater::behaviors,
    ),
    (heart_of_the_forest::ID, heart_of_the_forest::behaviors),
    (herald_sticker::ID, herald_sticker::behaviors),
    (herding_horn::ID, herding_horn::behaviors),
    (hoggy_bank::ID, hoggy_bank::behaviors),
    (holy_mallet::ID, holy_mallet::behaviors),
    (honeycomb_ring::ID, honeycomb_ring::behaviors),
    (inductive_gyroblade::ID, inductive_gyroblade::behaviors),
    (
        inductive_gyroblade_greater::ID,
        inductive_gyroblade_greater::behaviors,
    ),
    (innkeepers_hearth::ID, innkeepers_hearth::behaviors),
    (innkeepers_stein::ID, innkeepers_stein::behaviors),
    (
        insurrectionists_blade::ID,
        insurrectionists_blade::behaviors,
    ),
    (ironforge_anvil::ID, ironforge_anvil::behaviors),
    (jailer_sticker::ID, jailer_sticker::behaviors),
    (
        jailer_sticker_greater::ID,
        jailer_sticker_greater::behaviors,
    ),
    (jar_o_gems::ID, jar_o_gems::behaviors),
    (jarred_frostling::ID, jarred_frostling::behaviors),
    (jewelry_box::ID, jewelry_box::behaviors),
    (lava_lamp::ID, lava_lamp::behaviors),
    (lens_case::ID, lens_case::behaviors),
    (lightfeather_sticker::ID, lightfeather_sticker::behaviors),
    (lionfish_portrait::ID, lionfish_portrait::behaviors),
    (lockbox_portrait::ID, lockbox_portrait::behaviors),
    (lovely_locket::ID, lovely_locket::behaviors),
    (lucky_tabby::ID, lucky_tabby::behaviors),
    (magic_road_sign::ID, magic_road_sign::behaviors),
    (magicfin_sticker::ID, magicfin_sticker::behaviors),
    (magicians_top_hat::ID, magicians_top_hat::behaviors),
    (makeshift_master::ID, makeshift_master::behaviors),
    (maldraxxus_dagger::ID, maldraxxus_dagger::behaviors),
    (mama_bear_sticker::ID, mama_bear_sticker::behaviors),
    (manipulator_portrait::ID, manipulator_portrait::behaviors),
    (mask_of_ancient_ones::ID, mask_of_ancient_ones::behaviors),
    (maw_caster_portrait::ID, maw_caster_portrait::behaviors),
    (
        mecha_jaraxxus_sticker::ID,
        mecha_jaraxxus_sticker::behaviors,
    ),
    (mindslasher_portrait::ID, mindslasher_portrait::behaviors),
    (miniature_ship::ID, miniature_ship::behaviors),
    (minion_bait::ID, minion_bait::behaviors),
    (morgl_portrait::ID, morgl_portrait::behaviors),
    (moveout_box::ID, moveout_box::behaviors),
    (multilayered_shield::ID, multilayered_shield::behaviors),
    (murky_sticker::ID, murky_sticker::behaviors),
    (mysterious_orb::ID, mysterious_orb::behaviors),
    (mystery_cube::ID, mystery_cube::behaviors),
    (nerglish_phrasebook::ID, nerglish_phrasebook::behaviors),
    (
        nerglish_phrasebook_greater::ID,
        nerglish_phrasebook_greater::behaviors,
    ),
    (nether_pendant::ID, nether_pendant::behaviors),
    (nomi_sticker::ID, nomi_sticker::behaviors),
    (nomi_sticker_greater::ID, nomi_sticker_greater::behaviors),
    (ominous_stone::ID, ominous_stone::behaviors),
    (ophidian_staff::ID, ophidian_staff::behaviors),
    (orb_of_the_unknown::ID, orb_of_the_unknown::behaviors),
    (
        orb_of_the_unknown_greater::ID,
        orb_of_the_unknown_greater::behaviors,
    ),
    (orc_estra_portrait::ID, orc_estra_portrait::behaviors),
    (ornate_clock::ID, ornate_clock::behaviors),
    (pagles_fishing_rod::ID, pagles_fishing_rod::behaviors),
    (pilgrimp_sticker::ID, pilgrimp_sticker::behaviors),
    (plaguerunner_portrait::ID, plaguerunner_portrait::behaviors),
    (pocket_cyclone::ID, pocket_cyclone::behaviors),
    (
        pocket_cyclone_greater::ID,
        pocket_cyclone_greater::behaviors,
    ),
    (portable_factory::ID, portable_factory::behaviors),
    (
        portable_factory_greater::ID,
        portable_factory_greater::behaviors,
    ),
    (privateer_portrait::ID, privateer_portrait::behaviors),
    (protective_ring::ID, protective_ring::behaviors),
    (quartermasters_hook::ID, quartermasters_hook::behaviors),
    (quilligraphy_set::ID, quilligraphy_set::behaviors),
    (
        quilligraphy_set_greater::ID,
        quilligraphy_set_greater::behaviors,
    ),
    (rascal_sticker::ID, rascal_sticker::behaviors),
    (recycling_sticker::ID, recycling_sticker::behaviors),
    (reflective_pendant::ID, reflective_pendant::behaviors),
    (reinforced_shield::ID, reinforced_shield::behaviors),
    (reinvigorating_light::ID, reinvigorating_light::behaviors),
    (rendle_sticker::ID, rendle_sticker::behaviors),
    (replica_cathedral::ID, replica_cathedral::behaviors),
    (reusable_batteries::ID, reusable_batteries::behaviors),
    (rewinder_portrait::ID, rewinder_portrait::behaviors),
    (rivendare_portrait::ID, rivendare_portrait::behaviors),
    (rockin_music_box::ID, rockin_music_box::behaviors),
    (ruby_tusk::ID, ruby_tusk::behaviors),
    (safety_patch::ID, safety_patch::behaviors),
    (scraper_sticker::ID, scraper_sticker::behaviors),
    (secret_schematic::ID, secret_schematic::behaviors),
    (sellemental_portrait::ID, sellemental_portrait::behaviors),
    (shark_cannon::ID, shark_cannon::behaviors),
    (shathyar_shrine::ID, shathyar_shrine::behaviors),
    (shrine_of_evolution::ID, shrine_of_evolution::behaviors),
    (sinister_invitation::ID, sinister_invitation::behaviors),
    (sinstone_sticker::ID, sinstone_sticker::behaviors),
    (sky_golem_portrait::ID, sky_golem_portrait::behaviors),
    (slamma_sticker::ID, slamma_sticker::behaviors),
    (sludge_portrait::ID, sludge_portrait::behaviors),
    (snarky_portrait::ID, snarky_portrait::behaviors),
    (soul_battery::ID, soul_battery::behaviors),
    (soul_fermenter::ID, soul_fermenter::behaviors),
    (sous_chef_sticker::ID, sous_chef_sticker::behaviors),
    (souvenir_stand::ID, souvenir_stand::behaviors),
    (spell_powered_wrench::ID, spell_powered_wrench::behaviors),
    (sphere_of_memory::ID, sphere_of_memory::behaviors),
    (splinter_of_aurum::ID, splinter_of_aurum::behaviors),
    (staff_of_the_scourge::ID, staff_of_the_scourge::behaviors),
    (stormcoil_sticker::ID, stormcoil_sticker::behaviors),
    (stuffed_coin_purse::ID, stuffed_coin_purse::behaviors),
    (summoning_sphere::ID, summoning_sphere::behaviors),
    (sunken_anchor::ID, sunken_anchor::behaviors),
    (surveyor_portrait::ID, surveyor_portrait::behaviors),
    (tableware_portrait::ID, tableware_portrait::behaviors),
    (the_eye_of_dalaran::ID, the_eye_of_dalaran::behaviors),
    (the_eye_of_sargeras::ID, the_eye_of_sargeras::behaviors),
    (tickatus_sticker::ID, tickatus_sticker::behaviors),
    (token_of_the_old_gods::ID, token_of_the_old_gods::behaviors),
    (tome_of_the_ancients::ID, tome_of_the_ancients::behaviors),
    (trailblazer_sticker::ID, trailblazer_sticker::behaviors),
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
    (trip_vouchers::ID, trip_vouchers::behaviors),
    (trusty_crowbar::ID, trusty_crowbar::behaviors),
    (upstart_embers::ID, upstart_embers::behaviors),
    (urzul_sticker::ID, urzul_sticker::behaviors),
    (vibrant_bubble::ID, vibrant_bubble::behaviors),
    (volumizer_portrait::ID, volumizer_portrait::behaviors),
    (
        volumizer_portrait_greater::ID,
        volumizer_portrait_greater::behaviors,
    ),
    (wand_of_divination::ID, wand_of_divination::behaviors),
    (war_drum::ID, war_drum::behaviors),
    (warband_whistle::ID, warband_whistle::behaviors),
    (warcry_totem::ID, warcry_totem::behaviors),
    (water_wheel::ID, water_wheel::behaviors),
    (wax_lance::ID, wax_lance::behaviors),
    (weighted_gauntlet::ID, weighted_gauntlet::behaviors),
    (wicked_tome::ID, wicked_tome::behaviors),
    (wildfeather_duster::ID, wildfeather_duster::behaviors),
    (willbreaker_sticker::ID, willbreaker_sticker::behaviors),
    (wolfhead_flail::ID, wolfhead_flail::behaviors),
    (worn_treasure_map::ID, worn_treasure_map::behaviors),
    (writhing_tentacles::ID, writhing_tentacles::behaviors),
    (
        young_murk_eye_sticker::ID,
        young_murk_eye_sticker::behaviors,
    ),
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

/// `true` if the trinket's only type is Naga (`docs/trinkets.md` §1: never offered; one with a
/// second type is offered for it).
pub fn naga_only(t: &TrinketData) -> bool {
    !t.tribes.is_empty() && t.tribes.iter().all(|&tribe| tribe == Tribe::Naga)
}

/// The trinkets a game can offer (`docs/trinkets.md` §3.1): the implemented ones, not of the Naga
/// type only, Duos-only ones only in Duos, in table order.
pub fn offerable(duos: bool) -> impl Iterator<Item = &'static TrinketData> {
    TRINKETS
        .iter()
        .filter(move |t| is_implemented(t.id) && (duos || !t.duos_only) && !naga_only(t))
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
    let slot = slot_of(auras, id)?;
    auras.trinkets.get_mut(slot)
}

/// The player's state of the trinket `id`.
pub fn state(auras: &PlayerAuras, id: CardId) -> Option<&TrinketState> {
    auras.trinkets.get(slot_of(auras, id)?)
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
        .filter(|t| state.dark_gifts_enabled || !hooks(t.id).has(CardFlags::DARK_GIFTS))
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
    let mut options = draw_from(&candidates(state, greater), rng);
    for option in &mut options {
        option.tribe = assigned_type(state, option.card_id, rng);
    }
    options
}

/// `true` if the trinket `id`'s text names a minion type, assigned when it is offered
/// (`CardFlags::TRINKET_MINION_TYPE`, `docs/trinkets.md` §3.4).
pub fn is_typed(id: CardId) -> bool {
    hooks(id).has(CardFlags::TRINKET_MINION_TYPE)
}

/// The minion type a trinket offered to the player now names (`docs/trinkets.md` §3.4,
/// confirmed): their most common type, counted as for the offer rules ([`type_counts`]); a tie
/// (no minion at all too) is broken at random among the tied types, one `rng.below(k)`
/// (**engine decision**).
pub fn most_common_type(state: &TavernState, rng: &mut Rng) -> Tribe {
    let counts = type_counts(state);
    let best = counts.iter().copied().max().unwrap_or(0);
    let tied: Vec<usize> = (0..counts.len()).filter(|&i| counts[i] == best).collect();
    LOBBY_TRIBES[pick_tied(&tied, rng)]
}

/// The type the trinket `id` gets as it is offered: [`most_common_type`] for a typed trinket
/// (nothing drawn for the others).
fn assigned_type(state: &TavernState, id: CardId, rng: &mut Rng) -> Tribe {
    if is_typed(id) {
        most_common_type(state, rng)
    } else {
        Tribe::None
    }
}

/// The typed options of the offer that have no type yet get theirs (`docs/trinkets.md` §3.4):
/// an offer made without a draw (`TavernState::offer_trinkets`).
pub fn assign_offer_types(state: &mut TavernState, rng: &mut Rng) {
    for i in 0..state.trinket_options.len() {
        if state.trinket_options[i].tribe == Tribe::None {
            let tribe = assigned_type(state, state.trinket_options[i].card_id, rng);
            state.trinket_options[i].tribe = tribe;
        }
    }
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
            tribe: Tribe::None,
            replaces: None,
        })
        .collect()
}

/// The start of turn's offer (`docs/trinkets.md` §3.1): on turn 6 a Lesser one, on turn 9 a
/// Greater one, for a player with offers enabled. Nothing else happens, and nothing is drawn.
pub fn offer_at_turn_start(state: &mut TavernState, rng: &mut Rng) {
    if !state.trinkets_enabled {
        return;
    }
    let scheduled = match state.turn {
        LESSER_TURN => Some(false),
        GREATER_TURN => Some(true),
        _ => None,
    };
    let mut kind = scheduled;
    for t in &state.auras.trinkets {
        if let Some(offer_kind) = hooks(t.card_id).trinket_offer_kind {
            kind = offer_kind(t, state, kind);
        }
    }
    if let Some(greater) = kind {
        let options = draw_offer(state, greater, rng);
        push_offer(state, options);
    }
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
    let option = state.trinket_options[option_index];
    state.trinket_options.clear();
    match option.replaces {
        // The very trinket it would replace: kept, for nothing (`docs/trinkets.md` §3.5).
        Some(old) if old == option.card_id => {}
        Some(old) => {
            state.spend_gold(price, pool, rng);
            replace(state, old, option.card_id, option.tribe, pool, rng);
            after_pick(state, option.card_id, pool, rng);
        }
        None => {
            state.spend_gold(price, pool, rng);
            give_typed(state, option.card_id, option.tribe, pool, rng);
            after_pick(state, option.card_id, pool, rng);
        }
    }
    prune_offers(state);
}

/// The offers leave out the options that no longer apply (`docs/trinkets.md` §3.5): a trinket
/// the player owns by now (picked from an offer before, or one of theirs became it), or one
/// replacing a trinket they no longer own **(engine decision)**. An offer left with no option is
/// dropped, and the next waiting one (`trinket_offer_queue`) comes up.
fn prune_offers(state: &mut TavernState) {
    let auras = &state.auras;
    let applies = |o: &TrinketOption| {
        let kept = o.replaces == Some(o.card_id);
        (kept || !owns(auras, o.card_id)) && o.replaces.is_none_or(|r| owns(auras, r))
    };
    state.trinket_options.retain(applies);
    for offer in &mut state.trinket_offer_queue {
        offer.retain(applies);
    }
    state.trinket_offer_queue.retain(|offer| !offer.is_empty());
    if state.trinket_options.is_empty() && !state.trinket_offer_queue.is_empty() {
        state.trinket_options = state.trinket_offer_queue.remove(0);
    }
}

/// The player picked the trinket `picked` from an offer: each trinket's `trinket_after_pick`
/// hook.
fn after_pick(state: &mut TavernState, picked: CardId, pool: &mut CardPool, rng: &mut Rng) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(after) = hooks(id).trinket_after_pick {
            in_slot(slot, || after(state, picked, pool, rng));
        }
    }
}

/// Give the player the trinket `id` (`docs/trinkets.md` §4): it joins their trinkets and its
/// `trinket_bought` hook resolves. No price, no offer.
pub fn give(state: &mut TavernState, id: CardId, pool: &mut CardPool, rng: &mut Rng) {
    give_typed(state, id, Tribe::None, pool, rng);
}

/// [`give`], the trinket naming the minion type `tribe` (`docs/trinkets.md` §3.4): a typed
/// trinket given without one gets the player's most common type now.
pub fn give_typed(
    state: &mut TavernState,
    id: CardId,
    tribe: Tribe,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let tribe = if tribe == Tribe::None {
        assigned_type(state, id, rng)
    } else {
        tribe
    };
    state.auras.trinkets.push(TrinketState {
        tribe,
        ..new_state(id)
    });
    let slot = state.auras.trinkets.len() - 1;
    if let Some(bought) = hooks(id).trinket_bought {
        in_slot(slot, || bought(state, pool, rng));
    }
    state.sync_all_auras();
    prune_offers(state);
}

/// Replace the player's trinket `old` with `new` (`docs/trinkets.md` §3.5): `new` takes its place
/// in pick order, fresh (naming `tribe`, or the player's most common type now if it is typed and
/// `tribe` is `None`), and its `trinket_bought` text resolves. Nothing if the player doesn't own
/// `old`.
pub fn replace(
    state: &mut TavernState,
    old: CardId,
    new: CardId,
    tribe: Tribe,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let Some(slot) = slot_of(&state.auras, old) else {
        return;
    };
    let tribe = if tribe == Tribe::None {
        assigned_type(state, new, rng)
    } else {
        tribe
    };
    state.auras.trinkets[slot] = TrinketState {
        tribe,
        ..new_state(new)
    };
    if let Some(bought) = hooks(new).trinket_bought {
        in_slot(slot, || bought(state, pool, rng));
    }
    state.sync_all_auras();
    prune_offers(state);
}

/// `count` different random trinkets (`docs/trinkets.md` §3.5): each uniform, one
/// `rng.below(k)`, among the offerable trinkets of the kind (`greater`) the player doesn't own
/// and not drawn yet, of `tribe` if it isn't `None`. Fewer if there aren't enough.
pub fn random_trinkets(
    state: &TavernState,
    greater: bool,
    tribe: Tribe,
    count: usize,
    rng: &mut Rng,
) -> Vec<CardId> {
    let mut left: Vec<CardId> = offerable(state.auras.duos_game)
        .filter(|t| t.greater == greater && !owns(&state.auras, t.id))
        .filter(|t| state.dark_gifts_enabled || !hooks(t.id).has(CardFlags::DARK_GIFTS))
        .filter(|t| tribe == Tribe::None || t.tribes.contains(&tribe))
        .map(|t| t.id)
        .collect();
    let mut drawn = Vec::with_capacity(count);
    while drawn.len() < count && !left.is_empty() {
        drawn.push(left.remove(rng.below(left.len())));
    }
    drawn
}

/// Replace the player's trinket `old` with a random trinket of the kind `greater` (and of
/// `tribe`, unless it is `None`): [`random_trinkets`], then [`replace`]. Nothing if there is
/// none.
pub fn replace_with_random(
    state: &mut TavernState,
    old: CardId,
    greater: bool,
    tribe: Tribe,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if let Some(&new) = random_trinkets(state, greater, tribe, 1, rng).first() {
        replace(state, old, new, Tribe::None, pool, rng);
    }
}

/// "After N ..., replace this with a random Greater `tribe` Trinket": once the trinket `id`'s
/// `counter` has reached `count`, [`replace_with_random`].
pub fn replace_at_count(
    state: &mut TavernState,
    id: CardId,
    count: i32,
    tribe: Tribe,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if self::state(&state.auras, id).is_some_and(|t| t.counter >= count) {
        replace_with_random(state, id, true, tribe, pool, rng);
    }
}

/// An offer option of the trinket `id` at `cost` that replaces the player's trinket `old`
/// (`docs/trinkets.md` §3.5), its minion type assigned.
pub fn option_replacing(
    state: &TavernState,
    id: CardId,
    cost: u32,
    old: CardId,
    rng: &mut Rng,
) -> TrinketOption {
    TrinketOption {
        card_id: id,
        cost,
        tribe: assigned_type(state, id, rng),
        replaces: Some(old),
    }
}

/// Offer the player `options` (`docs/trinkets.md` §3.5): at once, or, if an offer is pending or
/// waiting, after them (`trinket_offer_queue`). Nothing for no options.
pub fn push_offer(state: &mut TavernState, options: Vec<TrinketOption>) {
    if options.is_empty() {
        return;
    }
    if state.trinket_options.is_empty() && state.trinket_offer_queue.is_empty() {
        state.trinket_options = options;
    } else {
        state.trinket_offer_queue.push(options);
    }
}

/// What a trinket sees of the player's teammate in Duos (`trinket_teammate`, `docs/trinkets.md`
/// §8.1): their Tavern, and whether the team's turn is over (both players ended it).
pub struct TeammateView<'a> {
    pub state: &'a TavernState,
    pub turn_over: bool,
}

/// Duos: each of the player's trinkets' `trinket_teammate` hook sees the teammate
/// (`docs/trinkets.md` §8.1).
pub fn teammate(
    state: &mut TavernState,
    mate: &TeammateView<'_>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(teammate) = hooks(id).trinket_teammate {
            in_slot(slot, || teammate(state, mate, pool, rng));
        }
    }
}

/// `Empowered Mirror`: the trinket `id` becomes a copy of the teammate's first trinket of the
/// kind `greater` that isn't a Mirror, naming the same minion type ([`replace`]).
pub fn become_teammates(
    state: &mut TavernState,
    id: CardId,
    greater: bool,
    mate: &TeammateView<'_>,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let mirrors = [empowered_mirror::ID, empowered_mirror_greater::ID];
    let copied = mate.state.auras.trinkets.iter().find(|t| {
        !mirrors.contains(&t.card_id) && data(t.card_id).is_some_and(|d| d.greater == greater)
    });
    if let Some(t) = copied {
        let (copy, tribe) = (t.card_id, t.tribe);
        replace(state, id, copy, tribe, pool, rng);
    }
}

/// `true` if one of the player's trinkets skips their current turn (`trinket_skips_turn`).
pub fn skips_turn(state: &TavernState) -> bool {
    state.auras.trinkets.iter().any(|t| {
        hooks(t.card_id)
            .trinket_skips_turn
            .is_some_and(|f| f(t, state))
    })
}

// ---------------------------------------------------------------------------------------------
// Dispatch: the player's trinkets' hooks, in the order they were picked
// ---------------------------------------------------------------------------------------------

/// The slots (pick-order indexes) and ids of the player's trinkets, in the order they were picked
/// (a snapshot: a hook may change the list).
fn owned_slots(auras: &PlayerAuras) -> Vec<(usize, CardId)> {
    auras
        .trinkets
        .iter()
        .map(|t| t.card_id)
        .enumerate()
        .collect()
}

std::thread_local! {
    /// The slot of the trinket whose hook is running (`docs/trinkets.md` §2.3): a player can own
    /// two copies of a trinket, and each copy's hooks find their own state ([`state_mut`]).
    static SLOT: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// Run `f`, a hook of the player's trinket in `slot` (its index in pick order), so that
/// [`state_mut`] finds that copy's state.
pub fn in_slot<R>(slot: usize, f: impl FnOnce() -> R) -> R {
    let previous = SLOT.with(|s| s.replace(Some(slot)));
    let out = f();
    SLOT.with(|s| s.set(previous));
    out
}

/// The slot of the player's trinket `id`: the trinket whose hook is running if it is that one,
/// else its first copy.
pub fn slot_of(auras: &PlayerAuras, id: CardId) -> Option<usize> {
    SLOT.with(std::cell::Cell::get)
        .filter(|&i| auras.trinkets.get(i).is_some_and(|t| t.card_id == id))
        .or_else(|| auras.trinkets.iter().position(|t| t.card_id == id))
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
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(turn_start) = hooks(id).trinket_turn_start {
            in_slot(slot, || turn_start(state, pool, rng));
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
    for (slot, id) in owned_slots(ctx.auras) {
        if let Some(friendly_death) = hooks(id).trinket_friendly_death {
            in_slot(slot, || friendly_death(ctx, dying));
        }
    }
}

/// A friendly minion attacks: each trinket's `trinket_friendly_attack` hook.
pub fn on_friendly_attack(ctx: &mut BoardCtx<'_>, attacker_id: UnitId) {
    for (slot, id) in owned_slots(ctx.auras) {
        if let Some(friendly_attack) = hooks(id).trinket_friendly_attack {
            in_slot(slot, || friendly_attack(ctx, attacker_id));
        }
    }
}

/// Extra times the effect of a `HeroPower` action resolves, summed over the trinkets'
/// `trinket_hero_power_triggers` (0 without trinkets).
pub fn extra_hero_power_triggers(auras: &PlayerAuras) -> u32 {
    auras
        .trinkets
        .iter()
        .filter_map(|t| hooks(t.card_id).trinket_hero_power_triggers.map(|f| f(t)))
        .sum()
}

/// Extra uses an active Hero Power gets each turn, summed over the trinkets'
/// `trinket_hero_power_uses` (0 without trinkets).
pub fn extra_hero_power_uses(auras: &PlayerAuras) -> u32 {
    auras
        .trinkets
        .iter()
        .filter_map(|t| hooks(t.card_id).trinket_hero_power_uses.map(|f| f(t)))
        .sum()
}

/// `true` if one of the player's trinkets changes the card `card_id`'s text
/// (`trinket_modifies`, `docs/trinkets.md` §11.8): the card's hooks read it.
pub fn modifies(auras: &PlayerAuras, card_id: CardId) -> bool {
    auras.trinkets.iter().any(|t| {
        hooks(t.card_id)
            .trinket_modifies
            .is_some_and(|f| f(card_id))
    })
}

/// A friendly minion was destroyed in the Tavern: each trinket's `trinket_tavern_destroy` hook.
pub fn tavern_destroy(
    state: &mut TavernState,
    destroyed: &Unit,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(destroy) = hooks(id).trinket_tavern_destroy {
            in_slot(slot, || destroy(state, destroyed, pool, rng));
        }
    }
}

/// A minion in the Tavern died: each trinket's `trinket_shop_minion_died` hook.
pub fn shop_minion_died(state: &mut TavernState, dead: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(died) = hooks(id).trinket_shop_minion_died {
            in_slot(slot, || died(state, dead, pool, rng));
        }
    }
}

/// After the player used a Hero Power: each trinket's `trinket_after_hero_power` hook.
pub fn after_hero_power(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(after) = hooks(id).trinket_after_hero_power {
            in_slot(slot, || after(state, pool, rng));
        }
    }
}

/// A minion is being bought (before it reaches the hand): each trinket's `trinket_minion_bought`
/// hook.
pub fn minion_bought(state: &mut TavernState, bought: &mut Unit) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(minion_bought) = hooks(id).trinket_minion_bought {
            in_slot(slot, || minion_bought(state, bought));
        }
    }
}

/// After a card was bought: each trinket's `trinket_after_buy` hook.
pub fn after_buy(state: &mut TavernState, bought: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(after_buy) = hooks(id).trinket_after_buy {
            in_slot(slot, || after_buy(state, bought, pool, rng));
        }
    }
}

/// After a minion was played or Magnetized: each trinket's `trinket_after_play` hook.
pub fn after_play(state: &mut TavernState, played: &Played, pool: &mut CardPool, rng: &mut Rng) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(after_play) = hooks(id).trinket_after_play {
            in_slot(slot, || after_play(state, played, pool, rng));
        }
    }
}

/// After a minion was sold: each trinket's `trinket_after_sell` hook.
pub fn after_sell(state: &mut TavernState, sold: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(after_sell) = hooks(id).trinket_after_sell {
            in_slot(slot, || after_sell(state, sold, pool, rng));
        }
    }
}

/// Gold was spent: each trinket's `trinket_gold_spent` hook.
pub fn gold_spent(state: &mut TavernState, amount: u32, pool: &mut CardPool, rng: &mut Rng) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(gold_spent) = hooks(id).trinket_gold_spent {
            in_slot(slot, || gold_spent(state, amount, pool, rng));
        }
    }
}

/// A spell the player cast in the Tavern, as the trinkets see it once all its casts are done
/// (`trinket_after_spell`, `docs/trinkets.md` §6).
#[derive(Clone, Debug)]
pub struct SpellCast {
    /// The spell card.
    pub card: Unit,
    /// It is cast on a minion.
    pub targeted: bool,
    /// Where its target now is (on the board or in the Tavern), if it is still there.
    pub target: Option<MinionSlot>,
    /// Played from the hand (else cast by an effect).
    pub from_hand: bool,
    /// The casts that resolved (extra casts included; none once the target was gone).
    pub casts: u32,
}

/// A spell was cast: each trinket's `trinket_after_spell` hook.
pub fn after_spell(state: &mut TavernState, spell: &SpellCast, pool: &mut CardPool, rng: &mut Rng) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(after_spell) = hooks(id).trinket_after_spell {
            in_slot(slot, || after_spell(state, spell, pool, rng));
        }
    }
}

/// A spell is about to be cast: each trinket's `trinket_before_spell` hook.
pub fn before_spell(state: &mut TavernState) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(before_spell) = hooks(id).trinket_before_spell {
            in_slot(slot, || before_spell(state));
        }
    }
}

/// The hero took damage in the Tavern: each trinket's `trinket_hero_damage` hook.
pub fn hero_damage(state: &mut TavernState, amount: i32) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(hero_damage) = hooks(id).trinket_hero_damage {
            in_slot(slot, || hero_damage(state, amount));
        }
    }
}

/// A card entered the hand: each trinket's `trinket_card_added` hook.
pub fn card_added(state: &mut TavernState, card: &Unit) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(card_added) = hooks(id).trinket_card_added {
            in_slot(slot, || card_added(state, card));
        }
    }
}

/// The card `card` becomes as it is about to enter the hand: each trinket's
/// `trinket_replace_gained` hook in pick order, each seeing the card as the earlier ones left it
/// (`docs/darkmoon_prizes.md` §4.4: `Corrupted Tome`).
pub fn replace_gained(auras: &PlayerAuras, mut card: Unit) -> Unit {
    for trinket in &auras.trinkets {
        if let Some(replace) = hooks(trinket.card_id).trinket_replace_gained {
            if let Some(new) = replace(&card) {
                card = new;
            }
        }
    }
    card
}

/// After each Tavern action: each trinket's `trinket_after_action` hook.
pub fn after_action(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(after_action) = hooks(id).trinket_after_action {
            in_slot(slot, || after_action(state, pool, rng));
        }
    }
}

/// The extra triggers the trinkets give the friendly Battlecry about to resolve
/// (`trinket_extra_battlecries`, which may use up their charges).
pub fn extra_battlecries(auras: &mut PlayerAuras, unit: &Unit) -> u32 {
    let mut extra = 0;
    for trinket in &mut auras.trinkets {
        if let Some(extra_battlecries) = hooks(trinket.card_id).trinket_extra_battlecries {
            extra += extra_battlecries(trinket, unit);
        }
    }
    extra
}

/// A friendly Battlecry triggered: each trinket's `trinket_battlecry_triggered` hook.
pub fn battlecry_triggered(state: &mut TavernState, unit: &Unit) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(triggered) = hooks(id).trinket_battlecry_triggered {
            in_slot(slot, || triggered(state, unit));
        }
    }
}

/// A card was discarded from the hand: each trinket's `trinket_after_discard` hook.
pub fn after_discard(state: &mut TavernState, card: &Unit, pool: &mut CardPool, rng: &mut Rng) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(after_discard) = hooks(id).trinket_after_discard {
            in_slot(slot, || after_discard(state, card, pool, rng));
        }
    }
}

/// The extra casts the trinkets give the spell about to be cast (`trinket_extra_spell_casts`,
/// which uses up their charges).
pub fn extra_spell_casts(auras: &mut PlayerAuras) -> u32 {
    let mut extra = 0;
    for trinket in &mut auras.trinkets {
        if let Some(extra_casts) = hooks(trinket.card_id).trinket_extra_spell_casts {
            extra += extra_casts(trinket);
        }
    }
    extra
}

/// What buying `card` costs, after each trinket's `trinket_buy_price` (in pick order).
pub fn buy_price(state: &TavernState, card: &Unit, mut price: BuyPrice) -> BuyPrice {
    for trinket in &state.auras.trinkets {
        if let Some(buy_price) = hooks(trinket.card_id).trinket_buy_price {
            price = buy_price(trinket, state, card, price);
        }
    }
    price
}

/// The fewest copies of `card_id` a trinket makes a Triple take (`trinket_triple_copies`).
pub fn triple_copies(auras: &PlayerAuras, card_id: CardId) -> Option<usize> {
    auras
        .trinkets
        .iter()
        .filter_map(|t| hooks(t.card_id).trinket_triple_copies)
        .filter_map(|f| f(card_id))
        .min()
}

/// The Tiers a shop draw offers for a Refresh of `kind`, given `tiers` (1 to the player's Tier),
/// folded over the trinkets' `trinket_shop_tiers` in pick order.
pub fn shop_tiers(auras: &PlayerAuras, kind: RefreshKind, mut tiers: (u32, u32)) -> (u32, u32) {
    for trinket in &auras.trinkets {
        if let Some(shop_tiers) = hooks(trinket.card_id).trinket_shop_tiers {
            tiers = shop_tiers(trinket, kind, tiers);
        }
    }
    tiers
}

/// The minions a rolled shop holds, given `slots`, folded over the trinkets'
/// `trinket_shop_slots` in pick order.
pub fn shop_minion_slots(state: &TavernState, mut slots: usize) -> usize {
    for trinket in &state.auras.trinkets {
        if let Some(shop_slots) = hooks(trinket.card_id).trinket_shop_slots {
            slots = shop_slots(state, slots);
        }
    }
    slots
}

/// The `Refresh` action's new shop, if a trinket makes it (`trinket_refresh`, the first in pick
/// order that does): whether one did.
pub fn refresh(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) -> bool {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(refresh) = hooks(id).trinket_refresh {
            if in_slot(slot, || refresh(state, pool, rng)) {
                return true;
            }
        }
    }
    false
}

/// `consumer` consumed `consumed` (a minion that left the Tavern): each trinket's
/// `trinket_after_consume` hook.
pub fn after_consume(state: &mut TavernState, consumer: MinionSlot, consumed: &Unit) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(after_consume) = hooks(id).trinket_after_consume {
            in_slot(slot, || after_consume(state, consumer, consumed));
        }
    }
}

/// A friendly minion came back with Reborn at `ctx.board[pos]`: each trinket's
/// `trinket_after_reborn` hook.
pub fn after_reborn(ctx: &mut BoardCtx<'_>, pos: usize) {
    for (slot, id) in owned_slots(ctx.auras) {
        if let Some(after_reborn) = hooks(id).trinket_after_reborn {
            in_slot(slot, || after_reborn(ctx, pos));
        }
    }
}

/// A Discover was resolved: each trinket's `trinket_after_discover` hook.
pub fn after_discover(
    state: &mut TavernState,
    chosen: &Unit,
    offered: &[CardId],
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(after_discover) = hooks(id).trinket_after_discover {
            in_slot(slot, || after_discover(state, chosen, offered, pool, rng));
        }
    }
}

/// The extra triggers the trinkets give the Deathrattles of `unit`, about to trigger in combat
/// (`trinket_extra_deathrattles`, which may use up their charges).
pub fn extra_deathrattles(auras: &mut PlayerAuras, unit: &Unit) -> u32 {
    let mut extra = 0;
    for trinket in &mut auras.trinkets {
        if let Some(f) = hooks(trinket.card_id).trinket_extra_deathrattles {
            extra += f(trinket, unit);
        }
    }
    extra
}

/// A friendly minion was summoned in combat: each trinket's `trinket_after_combat_summon` hook.
pub fn after_combat_summon(ctx: &mut BoardCtx<'_>) {
    for (slot, id) in owned_slots(ctx.auras) {
        if let Some(after_summon) = hooks(id).trinket_after_combat_summon {
            in_slot(slot, || after_summon(ctx));
        }
    }
}

/// The stats the player's trinkets give `unit`, a minion on the board, as an aura (each one's
/// `trinket_board_aura`; none without trinkets).
pub fn board_aura(unit: &Unit, auras: &PlayerAuras) -> (i32, i32) {
    let (mut atk, mut hp) = (0, 0);
    for trinket in &auras.trinkets {
        if let Some(f) = hooks(trinket.card_id).trinket_board_aura {
            let (a, h) = f(trinket, unit, auras);
            atk += a;
            hp += h;
        }
    }
    (atk, hp)
}

/// `Hammer of Twilight`: the trinket `id`'s hooks, the board aura `aura`, brought up to date as
/// picked and after each discard.
pub fn hammer_of_twilight(
    id: CardId,
    aura: fn(&TrinketState, &Unit, &PlayerAuras) -> (i32, i32),
) -> Vec<(CardId, CardHooks)> {
    vec![(
        id,
        CardHooks::EMPTY
            .on_trinket_board_aura(aura)
            .on_trinket_bought(|s, _, _| s.sync_all_auras())
            .on_trinket_after_discard(|s, _, _, _| s.sync_all_auras()),
    )]
}

/// `Hammer of Twilight`'s bonus: `+attack/+health` × (1 + the cards discarded this game).
pub fn hammer_bonus(auras: &PlayerAuras, attack: i32, health: i32) -> (i32, i32) {
    let times = 1 + auras.cards_discarded.min(i32::MAX as u32 / 4) as i32;
    (attack.saturating_mul(times), health.saturating_mul(times))
}

/// `unit` has a Deathrattle (its card's, one an effect gave it, or a Dark Gift's).
pub fn has_deathrattle(unit: &Unit) -> bool {
    cards::has_deathrattle(unit)
}

/// `times` Discovers of 3 Tavern spells of the player's Tier or lower (`Book of Medivh`; as
/// `Clever Castaway`'s).
pub fn discover_tavern_spells(state: &mut TavernState, times: u32, rng: &mut Rng) {
    for _ in 0..times {
        let opts = cards::spells::draw_discover_tavern_spells(state.tavern_tier, 3, rng);
        state.push_discover(opts);
    }
}

/// `Portable Factory`: a Discover of 3 minions of exactly `tier` with a type, from the pool; the
/// trinket `id` keeps the options (`stored`, `armed` 1) until it is resolved.
pub fn factory_discover(
    state: &mut TavernState,
    id: CardId,
    tier: u32,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    let typed = |cid: CardId| {
        cards::template(cid).is_some_and(|t| t.tavern_tier == tier && t.tribe != Tribe::None)
    };
    let mut opts = pool.draw_discover_filtered(tier, 3, typed, rng);
    for opt in &mut opts {
        state.apply_global_unit_auras(opt);
    }
    if let Some(t) = state_mut(&mut state.auras, id) {
        t.stored = opts.clone();
        t.armed = u32::from(!opts.is_empty());
    }
    state.push_discover(opts);
}

/// `Portable Factory`: a resolved Discover whose options are the ones the trinket `id` kept is
/// its own; it keeps a plain copy of the chosen minion instead.
pub fn factory_chosen(state: &mut TavernState, id: CardId, chosen: &Unit, offered: &[CardId]) {
    let Some(t) = state_mut(&mut state.auras, id) else {
        return;
    };
    if t.armed == 0 {
        return;
    }
    let mut kept: Vec<CardId> = t.stored.iter().map(|u| u.card_id).collect();
    let mut seen = offered.to_vec();
    kept.sort_unstable();
    seen.sort_unstable();
    if kept != seen {
        return;
    }
    t.armed = 0;
    t.stored = vec![cards::instantiate_plain_copy(chosen)];
}

/// `Portable Factory`: at the turn start, a plain copy of the minion the trinket `id` keeps.
pub fn factory_copy(state: &mut TavernState, id: CardId) {
    let Some(t) = self::state(&state.auras, id) else {
        return;
    };
    if t.armed != 0 {
        return;
    }
    if let Some(copy) = t.stored.first().map(cards::instantiate_plain_copy) {
        state.add_to_hand_or_wait(copy);
    }
}

/// `state.board[consumer]` consumes `state.shop[shop_idx]` (a trinket's consume): the minion
/// leaves the Tavern (its copy returns to the pool), the consumer gains its stats, then the
/// consume observers ([`after_consume`]).
pub fn consume_shop_minion(
    state: &mut TavernState,
    consumer: usize,
    shop_idx: usize,
    pool: &mut CardPool,
) {
    let consumed = state.shop.remove(shop_idx);
    pool.return_unit(&consumed);
    state.board[consumer].add_stats(consumed.attack, consumed.health);
    after_consume(state, MinionSlot::Board(consumer), &consumed);
}

/// The player ends their turn: each trinket's `trinket_end_turn` hook, with the end-of-turn
/// repeat count (`docs/tavern.md` §7.5).
pub fn end_turn(state: &mut TavernState, repeats: u32, pool: &mut CardPool, rng: &mut Rng) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(end_turn) = hooks(id).trinket_end_turn {
            in_slot(slot, || end_turn(state, repeats, pool, rng));
        }
    }
}

/// A Tavern spell was cast in the Tavern: each trinket's `trinket_spell_cast` hook.
pub fn spell_cast(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng) {
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(spell_cast) = hooks(id).trinket_spell_cast {
            in_slot(slot, || spell_cast(state, pool, rng));
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
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(after_refresh) = hooks(id).trinket_after_refresh {
            in_slot(slot, || after_refresh(state, pool, rng));
        }
    }
    if kind != RefreshKind::Player {
        return;
    }
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(after_refresh) = hooks(id).trinket_after_player_refresh {
            in_slot(slot, || after_refresh(state, pool, rng));
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
    for (slot, id) in owned_slots(&state.auras) {
        if let Some(after_upgrade) = hooks(id).trinket_after_upgrade {
            in_slot(slot, || after_upgrade(state, pool, rng));
        }
    }
}

/// The hero is about to take fatal combat damage: whether a trinket prevents it (the first in
/// pick order whose `trinket_prevent_fatal` does; `Safety Patch`'s Ice Block).
pub fn prevent_fatal(auras: &mut PlayerAuras) -> bool {
    auras.trinkets.iter_mut().any(|trinket| {
        hooks(trinket.card_id)
            .trinket_prevent_fatal
            .is_some_and(|prevent| prevent(trinket))
    })
}

/// A friendly minion is being summoned in combat: each trinket's `trinket_combat_summon` hook.
pub fn combat_summon(auras: &mut PlayerAuras, summoned: &mut Unit) {
    for trinket in &mut auras.trinkets {
        if let Some(combat_summon) = hooks(trinket.card_id).trinket_combat_summon {
            combat_summon(trinket, summoned);
        }
    }
}

/// A friendly minion was summoned in the Tavern: each trinket's `trinket_tavern_summon` hook.
pub fn tavern_summon(auras: &mut PlayerAuras, summoned: &mut Unit) {
    for trinket in &mut auras.trinkets {
        if let Some(tavern_summon) = hooks(trinket.card_id).trinket_tavern_summon {
            tavern_summon(trinket, summoned);
        }
    }
}

/// The player's Deity awakened in combat at `ctx.board[idx]`: each trinket's
/// `trinket_deity_awakened` hook.
pub fn on_deity_awakened(ctx: &mut BoardCtx<'_>, idx: usize) {
    for (slot, id) in owned_slots(ctx.auras) {
        if let Some(awakened) = hooks(id).trinket_deity_awakened {
            in_slot(slot, || awakened(ctx, idx));
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Helpers for the trinkets' hooks
// ---------------------------------------------------------------------------------------------

/// The minion type the player's trinket `id` names (`TrinketState::tribe`, `docs/trinkets.md`
/// §3.4), or `None` if it has none.
pub fn named_type(auras: &PlayerAuras, id: CardId) -> Option<Tribe> {
    state(auras, id)
        .map(|t| t.tribe)
        .filter(|&t| t != Tribe::None)
}

/// A trinket's Spellcraft spell `id`, named `name` (`docs/trinkets.md` §11.6): a token spell for
/// 0 Gold whose hooks (in the trinket's file) carry the Spellcraft flags (temporary, not a Tavern
/// spell, never in a pool).
pub fn spellcraft_spell(id: CardId, name: &str) -> Unit {
    let mut spell = Unit::new(name, 0, 0).with_card_id(id);
    spell.is_spell = true;
    spell
}

/// A token spell a trinket gives, `id` named `name`: 0 Gold to cast, its hooks (in the trinket's
/// file) saying what it is (`Portal in a Bottle`, `The Goldenizer`, `Duplicating Lens`).
pub fn token_spell(id: CardId, name: &str) -> Unit {
    let mut spell = Unit::new(name, 0, 0).with_card_id(id);
    spell.is_spell = true;
    spell
}

/// The board position of the minion the trinket `id` marked (`Unit::trinket_marks`), if any.
pub fn marked(state: &TavernState, id: CardId) -> Option<usize> {
    state
        .board
        .iter()
        .position(|u| u.trinket_marks.contains(&id))
}

/// "Destroy a friendly Undead to get `count` random Undead" (`Jailer Sticker`'s spell, as `The
/// Jailer`'s Rune of Damnation): `state.board[pos]` is destroyed, then each Undead, of the player's
/// Tier or lower from the pool, goes to the hand.
pub fn destroy_for_undead(
    state: &mut TavernState,
    pos: usize,
    count: u32,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    if pos >= state.board.len() {
        return;
    }
    state.destroy_board_unit(pos, pool, rng);
    for _ in 0..count {
        if let Some(undead) = pool.draw_by_tribe(Tribe::Undead, None, state.tavern_tier, rng) {
            state.add_pool_card_to_hand(undead, pool);
        }
    }
}

/// `true` if the resolved Discover whose options were `offered` is the one the trinket `id` keeps
/// (`stored`, `armed` 1: `Token of the Old Gods`); it then stops keeping it.
pub fn own_discover(state: &mut TavernState, id: CardId, offered: &[CardId]) -> bool {
    let Some(t) = state_mut(&mut state.auras, id) else {
        return false;
    };
    if t.armed == 0 {
        return false;
    }
    let mut kept: Vec<CardId> = t.stored.iter().map(|u| u.card_id).collect();
    let mut seen = offered.to_vec();
    kept.sort_unstable();
    seen.sort_unstable();
    if kept != seen {
        return false;
    }
    t.armed = 0;
    t.stored.clear();
    true
}

/// The trinket `id` marks `state.board[pos]` and no other card.
pub fn mark(state: &mut TavernState, pos: usize, id: CardId) {
    unmark(state, id);
    if let Some(unit) = state.board.get_mut(pos) {
        unit.trinket_marks.push(id);
    }
}

/// The trinket `id`'s marks leave every card of the board, the hand and the Tavern.
pub fn unmark(state: &mut TavernState, id: CardId) {
    for unit in state
        .board
        .iter_mut()
        .chain(&mut state.hand)
        .chain(&mut state.shop)
    {
        unit.trinket_marks.retain(|&m| m != id);
    }
}

/// `count` random minions of the type the trinket `id` names ([`give_random_minion`]; nothing
/// without a type).
pub fn give_random_of_named_type(state: &mut TavernState, id: CardId, count: u32, rng: &mut Rng) {
    let Some(tribe) = named_type(&state.auras, id) else {
        return;
    };
    for _ in 0..count {
        give_random_minion(state, rng, |t| t.tribe.matches(tribe));
    }
}

/// `Inductive Gyroblade`'s end of turn: `repeats` times, a `Satellite` of `stats/stats` plus
/// `+stats/+stats` per Tavern spell cast this turn (the trinket `id`'s `counter`).
pub fn gyroblade_satellites(state: &mut TavernState, id: CardId, stats: i32, repeats: u32) {
    let spells = self::state(&state.auras, id).map_or(0, |t| t.counter);
    let size = stats.saturating_mul(spells.saturating_add(1));
    for _ in 0..repeats {
        let mut satellite = cards::tokens::make_satellite(false);
        satellite.attack = size;
        satellite.health = size;
        satellite.base_attack = size;
        satellite.base_health = size;
        satellite.sync_max_stats();
        state.add_to_hand_or_wait(satellite);
    }
}

/// One more Tavern spell cast this turn for the trinket `id` (`counter`).
pub fn count_spell_this_turn(state: &mut TavernState, id: CardId) {
    if let Some(t) = state_mut(&mut state.auras, id) {
        t.counter = t.counter.saturating_add(1);
    }
}

/// A trinket's "Get a ..." (`docs/trinkets.md` §5): a plain copy of the minion `card_id` from the
/// catalog (not the pool) goes to the hand, or waits for room in a full hand.
pub fn give_minion(state: &mut TavernState, card_id: CardId) {
    if let Some(template) = cards::template(card_id) {
        state.add_to_hand_or_wait(template.instantiate());
    }
}

/// A trinket's "Get a Golden ...": a plain copy of the minion `card_id` from the catalog, made
/// Golden (`Unit::make_golden`, as `Golden Touch`'s), goes to the hand, or waits for room.
pub fn give_golden_minion(state: &mut TavernState, card_id: CardId) {
    if let Some(template) = cards::template(card_id) {
        let mut golden = template.instantiate();
        golden.make_golden();
        state.add_to_hand_or_wait(golden);
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

/// A Golden minion played from the hand (or Magnetized from it) adds one to the trinket `id`'s
/// `counter` (`Gold Mallet`).
pub fn count_golden_play(state: &mut TavernState, id: CardId, played: &Played) {
    if !(played.from_hand && played.golden) {
        return;
    }
    if let Some(t) = state_mut(&mut state.auras, id) {
        t.counter += 1;
    }
}

/// `count` Blood Gems are played (not cast: `Unit::play_blood_gems`) on every minion on the board.
pub fn play_blood_gems_on_board(state: &mut TavernState, count: u32) {
    let auras = state.auras.clone();
    for unit in &mut state.board {
        unit.play_blood_gems(count, &auras);
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
            .filter(|(stem, _)| !["mod", "table", "vocab"].contains(&stem.as_str()))
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
        // HSReplay's 237 Solo trinkets and the 9 Duos-exclusive ones (`docs/trinkets.md` §9).
        assert_eq!(TRINKETS.len(), 246);
        assert_eq!(TRINKETS.iter().filter(|t| !t.greater).count(), 120);
        assert_eq!(TRINKETS.iter().filter(|t| t.duos_only).count(), 9);
        assert!(!TRINKETS.iter().any(naga_only));
        let naga = |t: &&TrinketData| t.tribes.contains(&Tribe::Naga);
        let dual: Vec<_> = TRINKETS.iter().filter(naga).map(|t| t.name).collect();
        assert_eq!(dual, ["Heart of the Forest", "Wand of Divination"]);
        let solo = TRINKETS.iter().filter(|t| !t.duos_only).collect::<Vec<_>>();
        assert_eq!(solo.len(), 237);
        assert_eq!(solo.iter().filter(|t| !t.greater).count(), 115);
        assert!(by_name("Kiri's Double Eclipse").is_none());
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

    /// The table is exactly the committed pool file (`tools/data/trinkets_hsreplay.tsv`): its
    /// trinkets, kinds and prices (`docs/trinkets.md` §9).
    #[test]
    fn the_table_is_the_hsreplay_pool_file() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../tools/data/trinkets_hsreplay.tsv");
        let text = fs::read_to_string(&path).expect("tools/data/trinkets_hsreplay.tsv");
        let mut listed = 0;
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').map(str::trim).collect();
            let t =
                by_card_id(fields[0]).unwrap_or_else(|| panic!("{} is in the table", fields[0]));
            assert_eq!(t.greater, fields[1] == "Greater", "{}: kind", t.name);
            assert_eq!(t.cost.to_string(), fields[2], "{}: price", t.name);
            listed += 1;
        }
        assert_eq!(listed, TRINKETS.len());
    }

    /// The RL vocabulary keeps the first table's order and every trinket of the table
    /// (`docs/trinkets.md` §8.2).
    #[test]
    fn the_vocabulary_is_append_only() {
        let ids = vocabulary();
        assert_eq!(ids[..vocab::VOCAB_IDS.len()], vocab::VOCAB_IDS);
        assert!(TRINKETS.iter().all(|t| ids.contains(&t.id)));
        assert_eq!(ids.len(), 264);
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
        let pool = || TRINKETS.iter().filter(|t| !t.duos_only && !naga_only(t));
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
