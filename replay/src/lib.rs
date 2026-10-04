//! Parser and structured replay model for Hearthstone Battlegrounds `Power.log` files.
//!
//! Reconstructs each Battlegrounds game in a `Power.log` into a turn-by-turn
//! [`GameReplay`] containing both the **Tavern (Recruit) Phase** (initial shop/board/hand,
//! chronological player actions, shop refreshes, and end-of-turn state) and the
//! **Combat Phase** (starting boards, strike-by-strike server event log, tag-ins,
//! hero damage, and conversion to [`Scenario`] / [`Unit`] for `seaglass` simulation).

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use seaglass::cards::{full_catalog, tier1, tokens};
use seaglass::model::{BattleOutcome, CardId, Keyword, Tribe, Unit};
use seaglass::scenario::{Batch, Defaults, Scenario};

/// A complete parsed Battlegrounds game from `Power.log`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameReplay {
    pub game_index: usize,
    pub start_time: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub game_seed: Option<i64>,
    pub is_duos: bool,
    pub player_name: String,
    pub hero_name: String,
    pub hero_card_id: String,
    pub turns: Vec<TurnReplay>,
}

/// One full Battlegrounds round (Tavern Phase + Combat Phase).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TurnReplay {
    pub turn: u32,
    pub tavern: TavernPhaseReplay,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combat: Option<CombatPhaseReplay>,
}

/// Structured record of a single Tavern (Recruit) phase.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TavernPhaseReplay {
    pub tavern_tier: u32,
    pub starting_gold: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upgrade_cost: Option<i32>,
    pub hero_health: i32,
    pub hero_armor: i32,
    pub deity_attack: i32,
    pub deity_health: i32,
    pub initial_board: Vec<ReplayUnit>,
    pub initial_hand: Vec<ReplayCard>,
    pub initial_shop: Vec<ReplayCard>,
    pub actions: Vec<TavernActionRecord>,
    pub final_board: Vec<ReplayUnit>,
    pub final_hand: Vec<ReplayCard>,
    pub final_shop: Vec<ReplayCard>,
    pub ending_gold: i32,
    pub is_frozen: bool,
}

/// A player action or notable event during the Tavern phase.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TavernActionRecord {
    BuyMinion {
        card: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        shop_pos: Option<usize>,
        gold_after: i32,
    },
    BuySpell {
        spell: String,
        gold_after: i32,
    },
    PlayMinion {
        card: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        board_pos: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        gold_after: i32,
    },
    CastSpell {
        spell: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        gold_after: i32,
    },
    SellMinion {
        card: String,
        gold_after: i32,
    },
    Refresh {
        gold_after: i32,
        new_shop: Vec<ReplayCard>,
    },
    Freeze {
        is_frozen: bool,
    },
    UpgradeTier {
        new_tier: u32,
        gold_after: i32,
    },
    Activate {
        card: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        gold_after: i32,
    },
    HeroPower {
        card: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        gold_after: i32,
    },
    Triple {
        golden_card: String,
    },
    PassCard {
        card: String,
        gold_after: i32,
    },
}

/// Structured record of a single Combat phase.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CombatPhaseReplay {
    pub friendly_hero: String,
    pub friendly_tier: u32,
    pub opponent_hero: String,
    pub opponent_tier: u32,
    pub team_a: Vec<ReplayUnit>,
    pub team_b: Vec<ReplayUnit>,
    pub events: Vec<CombatLogEvent>,
    pub outcome: BattleOutcome,
    /// Signed hero damage from the friendly team's perspective (`+` = dealt, `-` = taken).
    pub hero_damage: i32,
    pub survivors_a: Vec<ReplayUnit>,
    pub survivors_b: Vec<ReplayUnit>,
}

/// A combat event recorded from the server's `Power.log` stream.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CombatLogEvent {
    Attack {
        side: String,
        attacker: String,
        defender: String,
        attacker_after: String,
        defender_after: String,
    },
    Trigger {
        source: String,
        summary: String,
    },
    Deaths {
        units: Vec<String>,
    },
    TagIn {
        side: String,
        hero: String,
        board: Vec<ReplayUnit>,
    },
    HeroDamage {
        attacker_hero: String,
        defender_hero: String,
        damage: i32,
    },
}

/// A minion snapshot on a board or in shop.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayUnit {
    pub entity_id: u32,
    pub card_id: String,
    pub name: String,
    pub attack: i32,
    pub health: i32,
    pub tier: u32,
    pub is_golden: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
}

/// A card snapshot in hand or shop (minion or spell).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayCard {
    pub entity_id: u32,
    pub card_id: String,
    pub name: String,
    pub card_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stats: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<i32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_golden: bool,
}

impl ReplayUnit {
    /// Format as a concise display string, e.g. `"Suspicious Prisonguard (3/6) [Taunt]"`.
    pub fn display_short(&self) -> String {
        let gold = if self.is_golden { " ★" } else { "" };
        let kw = if self.keywords.is_empty() {
            String::new()
        } else {
            format!(" [{}]", self.keywords.join(", "))
        };
        format!(
            "{}{} ({}/{}){}",
            self.name, gold, self.attack, self.health, kw
        )
    }

    /// Format as a `seaglass` scenario unit specification string (`"3/6 card:suspicious_prisonguard"`).
    pub fn to_scenario_spec(&self) -> String {
        let mut tokens = vec![format!("{}/{}", self.attack, self.health)];
        if self.is_golden {
            tokens.push("golden".to_string());
        }
        if let Some((_, slug, _)) = map_hs_card_id(&self.card_id) {
            tokens.push(format!("card:{slug}"));
        } else if self.tier > 1 {
            tokens.push(format!("tier:{}", self.tier));
        }
        for kw in &self.keywords {
            tokens.push(kw.to_ascii_lowercase().replace(' ', "_"));
        }
        tokens.join(" ")
    }

    /// Convert to a `seaglass` [`Unit`] for combat simulation.
    /// Returns `(Unit, is_natively_supported)`.
    pub fn to_seaglass_unit(&self) -> (Unit, bool) {
        let mut u = Unit::new(self.name.clone(), self.attack, self.health);
        u.tavern_tier = self.tier.max(1);
        u.is_golden = self.is_golden;
        for kw in &self.keywords {
            match kw.as_str() {
                "Taunt" => u.apply_keyword(Keyword::Taunt, true),
                "Divine Shield" => u.apply_keyword(Keyword::DivineShield, true),
                "Windfury" => u.apply_keyword(Keyword::Windfury, true),
                "Reborn" => u.apply_keyword(Keyword::Reborn, true),
                "Venomous" => u.apply_keyword(Keyword::Venomous, true),
                "Stealth" => u.apply_keyword(Keyword::Stealth, true),
                "Magnetic" => u.apply_keyword(Keyword::Magnetic, true),
                _ => {}
            }
        }
        if let Some((cid, _, tribe)) = map_hs_card_id(&self.card_id) {
            u.card_id = cid;
            u.tribe = tribe;
            (u, true)
        } else {
            (u, false)
        }
    }
}

impl ReplayCard {
    pub fn display_short(&self) -> String {
        let gold = if self.is_golden { " ★" } else { "" };
        match (&self.stats, self.cost) {
            (Some(st), _) => format!("{}{} ({})", self.name, gold, st),
            (None, Some(c)) => format!("{}{} [Spell, {}g]", self.name, gold, c),
            (None, None) => format!("{}{}", self.name, gold),
        }
    }
}

impl CombatPhaseReplay {
    /// Convert this combat's starting boards into `seaglass` [`Unit`] vectors and return
    /// any card names whose custom combat triggers are not yet in the `seaglass` catalog.
    pub fn to_seaglass_boards(&self) -> (Vec<Unit>, Vec<Unit>, Vec<String>) {
        let mut unsupported = Vec::new();
        let mut board_a = Vec::with_capacity(self.team_a.len());
        let mut board_b = Vec::with_capacity(self.team_b.len());

        for ru in &self.team_a {
            let (u, ok) = ru.to_seaglass_unit();
            if !ok && !unsupported.contains(&ru.name) {
                unsupported.push(ru.name.clone());
            }
            board_a.push(u);
        }
        for ru in &self.team_b {
            let (u, ok) = ru.to_seaglass_unit();
            if !ok && !unsupported.contains(&ru.name) {
                unsupported.push(ru.name.clone());
            }
            board_b.push(u);
        }
        (board_a, board_b, unsupported)
    }

    /// Export this combat phase as a [`Scenario`] compatible with `combat_cli`.
    pub fn to_scenario(&self, turn: u32) -> Scenario {
        Scenario {
            name: format!("replay_turn_{turn}_combat"),
            seed: 42,
            hero_tier_a: self.friendly_tier.max(1),
            hero_tier_b: self.opponent_tier.max(1),
            deity_a: None,
            deity_b: None,
            deity_stats_a: None,
            deity_stats_b: None,
            defaults: Defaults::default(),
            team_a: self.team_a.iter().map(|u| u.to_scenario_spec()).collect(),
            team_b: self.team_b.iter().map(|u| u.to_scenario_spec()).collect(),
            expect: None,
            batch: Some(Batch {
                base_seed: 2026,
                n: 10000,
            }),
            expect_stats: None,
        }
    }
}

/// Maps Hearthstone Battlegrounds `cardId` strings to `(CardId, slug, Tribe)` in `seaglass`.
pub fn map_hs_card_id(hs_id: &str) -> Option<(CardId, &'static str, Tribe)> {
    let base = hs_id.strip_suffix("_G").unwrap_or(hs_id);
    match base {
        "BG36_110" => Some((tier1::joyous::ID, "joyous", Tribe::Aberration)),
        "BG36_098" => Some((tier1::zoatroid::ID, "zoatroid", Tribe::Aberration)),
        "BG31_803" => Some((tier1::buzzing_vermin::ID, "buzzing_vermin", Tribe::Beast)),
        "BG36_200" => Some((tier1::flittering_bat::ID, "flittering_bat", Tribe::Beast)),
        "BGS_004" | "TB_BaconUps_079" => {
            Some((tier1::wrath_weaver::ID, "wrath_weaver", Tribe::Demon))
        }
        "BG31_330" => Some((tier1::ominous_seer::ID, "ominous_seer", Tribe::Demon)),
        "BG29_888" => Some((tier1::glim_guardian::ID, "glim_guardian", Tribe::Dragon)),
        "BG35_814" => Some((
            tier1::scarlet_survivor::ID,
            "scarlet_survivor",
            Tribe::Dragon,
        )),
        "BGS_119" | "TB_BaconUps_159" => Some((
            tier1::crackling_cyclone::ID,
            "crackling_cyclone",
            Tribe::Elemental,
        )),
        "BG31_815" => Some((tier1::dune_dweller::ID, "dune_dweller", Tribe::Elemental)),
        "BG29_611" => Some((tier1::cord_puller::ID, "cord_puller", Tribe::Mech)),
        "BG26_146" => Some((tier1::lullabot::ID, "lullabot", Tribe::Mech)),
        "BG31_149" => Some((tier1::bubble_gunner::ID, "bubble_gunner", Tribe::Murloc)),
        "BG32_330" => Some((tier1::flighty_scout::ID, "flighty_scout", Tribe::Murloc)),
        "BG32_236" => Some((
            tier1::aureate_laureate::ID,
            "aureate_laureate",
            Tribe::Pirate,
        )),
        "BG26_135" => Some((tier1::southsea_busker::ID, "southsea_busker", Tribe::Pirate)),
        "BG20_100" => Some((
            tier1::razorfen_geomancer::ID,
            "razorfen_geomancer",
            Tribe::Quilboar,
        )),
        "BG33_886" => Some((tier1::tusked_camper::ID, "tusked_camper", Tribe::Quilboar)),
        "BG28_300" => Some((
            tier1::harmless_bonehead::ID,
            "harmless_bonehead",
            Tribe::Undead,
        )),
        "BG25_001" => Some((tier1::risen_rider::ID, "risen_rider", Tribe::Undead)),
        "BG36_345" => Some((
            tier1::suspicious_prisonguard::ID,
            "suspicious_prisonguard",
            Tribe::None,
        )),
        // Tier 2 Minions
        "BG36_520" => Some((
            seaglass::cards::tier2::bilgewater_breakout::ID,
            "bilgewater_breakout",
            Tribe::Pirate,
        )),
        "BG34_170t2" => Some((
            seaglass::cards::tier2::blue_volumizer::ID,
            "blue_volumizer",
            Tribe::Mech,
        )),
        "BG36_099" => Some((
            seaglass::cards::tier2::brain_rotter::ID,
            "brain_rotter",
            Tribe::Aberration,
        )),
        "BGS_034" | "TB_BaconUps_149" => Some((
            seaglass::cards::tier2::bronze_warden::ID,
            "bronze_warden",
            Tribe::Dragon,
        )),
        "BG36_342" => Some((
            seaglass::cards::tier2::clever_castaway::ID,
            "clever_castaway",
            Tribe::Pirate,
        )),
        "BG31_320" => Some((
            seaglass::cards::tier2::crater_miner::ID,
            "crater_miner",
            Tribe::Quilboar,
        )),
        "BG36_354" => Some((
            seaglass::cards::tier2::decoy_conjurer::ID,
            "decoy_conjurer",
            Tribe::None,
        )),
        "BG26_963" => Some((
            seaglass::cards::tier2::electric_synthesizer::ID,
            "electric_synthesizer",
            Tribe::Dragon,
        )),
        "BG25_008" => Some((
            seaglass::cards::tier2::eternal_knight::ID,
            "eternal_knight",
            Tribe::Undead,
        )),
        "BG34_140" => Some((
            seaglass::cards::tier2::expert_aviator::ID,
            "expert_aviator",
            Tribe::Murloc,
        )),
        "BG31_816" => Some((
            seaglass::cards::tier2::fire_baller::ID,
            "fire_baller",
            Tribe::Elemental,
        )),
        "BG31_801" => Some((
            seaglass::cards::tier2::forest_rover::ID,
            "forest_rover",
            Tribe::Beast,
        )),
        "BG34_170t3" => Some((
            seaglass::cards::tier2::green_volumizer::ID,
            "green_volumizer",
            Tribe::Mech,
        )),
        "BG26_805" => Some((
            seaglass::cards::tier2::humming_bird::ID,
            "humming_bird",
            Tribe::Beast,
        )),
        "BG32_237" => Some((
            seaglass::cards::tier2::intrepid_botanist::ID,
            "intrepid_botanist",
            Tribe::None,
        )),
        "BG35_150" => Some((
            seaglass::cards::tier2::laboratory_assistant::ID,
            "laboratory_assistant",
            Tribe::Demon,
        )),
        "BG36_201" => Some((
            seaglass::cards::tier2::lurking_lionfish::ID,
            "lurking_lionfish",
            Tribe::Beast,
        )),
        "BG31_177" => Some((
            seaglass::cards::tier2::mechagnome_interpreter::ID,
            "mechagnome_interpreter",
            Tribe::Mech,
        )),
        "BG23_357" => Some((
            seaglass::cards::tier2::mind_muck::ID,
            "mind_muck",
            Tribe::Demon,
        )),
        "BG25_011" => Some((
            seaglass::cards::tier2::nerubian_deathswarmer::ID,
            "nerubian_deathswarmer",
            Tribe::Undead,
        )),
        "BG24_715" => Some((
            seaglass::cards::tier2::patient_scout::ID,
            "patient_scout",
            Tribe::None,
        )),
        "BG33_430" => Some((
            seaglass::cards::tier2::prodigious_tusker::ID,
            "prodigious_tusker",
            Tribe::Quilboar,
        )),
        "BG34_170t" => Some((
            seaglass::cards::tier2::red_volumizer::ID,
            "red_volumizer",
            Tribe::Mech,
        )),
        "BG20_101" => Some((
            seaglass::cards::tier2::roadboar::ID,
            "roadboar",
            Tribe::Quilboar,
        )),
        "BG25_022" => Some((
            seaglass::cards::tier2::scarlet_skull::ID,
            "scarlet_skull",
            Tribe::Undead,
        )),
        "BGS_115" | "TB_BaconUps_156" => Some((
            seaglass::cards::tier2::sellemental::ID,
            "sellemental",
            Tribe::Elemental,
        )),
        "BG31_818" => Some((
            seaglass::cards::tier2::snow_baller::ID,
            "snow_baller",
            Tribe::Elemental,
        )),
        "BG26_174" => Some((
            seaglass::cards::tier2::soul_rewinder::ID,
            "soul_rewinder",
            Tribe::Demon,
        )),
        "BG32_235" => Some((
            seaglass::cards::tier2::surfing_sylvar::ID,
            "surfing_sylvar",
            Tribe::Pirate,
        )),
        "BG22_202" => Some((seaglass::cards::tier2::tad::ID, "tad", Tribe::Murloc)),
        "BG21_015" => Some((
            seaglass::cards::tier2::tarecgosa::ID,
            "tarecgosa",
            Tribe::Dragon,
        )),
        "BG36_116" => Some((
            seaglass::cards::tier2::underrot_spawn::ID,
            "underrot_spawn",
            Tribe::Aberration,
        )),
        "BG29_300" => Some((
            seaglass::cards::tier2::very_hungry_winterfinner::ID,
            "very_hungry_winterfinner",
            Tribe::Murloc,
        )),
        "BG36_100" => Some((
            seaglass::cards::tier2::wandering_willbreaker::ID,
            "wandering_willbreaker",
            Tribe::Aberration,
        )),
        // Tier 3 Minions
        "BG36_102" => Some((
            seaglass::cards::tier3::abyssal_envoy::ID,
            "abyssal_envoy",
            Tribe::Aberration,
        )),
        "BG26_147" => Some((
            seaglass::cards::tier3::accord_o_tron::ID,
            "accord_o_tron",
            Tribe::Mech,
        )),
        "BG24_500" => Some((
            seaglass::cards::tier3::amber_guardian::ID,
            "amber_guardian",
            Tribe::Dragon,
        )),
        "BG_BOT_911" | "TB_BaconUps_099" => Some((
            seaglass::cards::tier3::annoy_o_module::ID,
            "annoy_o_module",
            Tribe::Mech,
        )),
        "BG34_170" => Some((
            seaglass::cards::tier3::auto_accelerator::ID,
            "auto_accelerator",
            Tribe::Mech,
        )),
        "BG35_890" => Some((
            seaglass::cards::tier3::azsharan_cutlassier::ID,
            "azsharan_cutlassier",
            Tribe::Pirate,
        )),
        "BG33_926" => Some((
            seaglass::cards::tier3::blue_whelp::ID,
            "blue_whelp",
            Tribe::Dragon,
        )),
        "BG31_890" => Some((
            seaglass::cards::tier3::cadaver_caretaker::ID,
            "cadaver_caretaker",
            Tribe::Undead,
        )),
        "BGS_131" | "TB_BaconUps_251" => Some((
            seaglass::cards::tier3::deadly_spore::ID,
            "deadly_spore",
            Tribe::None,
        )),
        "BG33_156" => Some((
            seaglass::cards::tier3::devout_hellcaller::ID,
            "devout_hellcaller",
            Tribe::Demon,
        )),
        "BG27_556" => Some((
            seaglass::cards::tier3::diremuck_forager::ID,
            "diremuck_forager",
            Tribe::Murloc,
        )),
        "BG28_303" => Some((
            seaglass::cards::tier3::disguised_graverobber::ID,
            "disguised_graverobber",
            Tribe::None,
        )),
        "BG36_103" => Some((
            seaglass::cards::tier3::drifting_sacrifice::ID,
            "drifting_sacrifice",
            Tribe::Aberration,
        )),
        "BG28_555" => Some((
            seaglass::cards::tier3::fearless_foodie::ID,
            "fearless_foodie",
            Tribe::Quilboar,
        )),
        "BG36_101" => Some((
            seaglass::cards::tier3::fetid_corroder::ID,
            "fetid_corroder",
            Tribe::Aberration,
        )),
        "BG36_346" => Some((
            seaglass::cards::tier3::fruit_vendor::ID,
            "fruit_vendor",
            Tribe::Elemental,
        )),
        "BG31_325" => Some((
            seaglass::cards::tier3::gem_rat::ID,
            "gem_rat",
            Tribe::Quilboar,
        )),
        "BG36_349" => Some((
            seaglass::cards::tier3::greedy_conniver::ID,
            "greedy_conniver",
            Tribe::Pirate,
        )),
        "BG25_010" => Some((
            seaglass::cards::tier3::handless_forsaken::ID,
            "handless_forsaken",
            Tribe::Undead,
        )),
        "BG36_521" => Some((
            seaglass::cards::tier3::hired_mount::ID,
            "hired_mount",
            Tribe::BeastPirate,
        )),
        "BG27_000" => Some((
            seaglass::cards::tier3::iron_groundskeeper::ID,
            "iron_groundskeeper",
            Tribe::None,
        )),
        "BG36_522" => Some((
            seaglass::cards::tier3::locked_up_mutineer::ID,
            "locked_up_mutineer",
            Tribe::Pirate,
        )),
        "BG26_524" => Some((
            seaglass::cards::tier3::malchezaar_prince_of_dance::ID,
            "malchezaar_prince_of_dance",
            Tribe::Demon,
        )),
        "BG28_582" => Some((
            seaglass::cards::tier3::mangled_bandit::ID,
            "mangled_bandit",
            Tribe::Quilboar,
        )),
        "BG28_309" => Some((
            seaglass::cards::tier3::mummifier::ID,
            "mummifier",
            Tribe::Undead,
        )),
        "BG29_860" => Some((
            seaglass::cards::tier3::prosthetic_hand::ID,
            "prosthetic_hand",
            Tribe::UndeadMech,
        )),
        "BG34_405" => Some((
            seaglass::cards::tier3::relentless_deflector::ID,
            "relentless_deflector",
            Tribe::Mech,
        )),
        "BG36_184" => Some((
            seaglass::cards::tier3::rescue_bot::ID,
            "rescue_bot",
            Tribe::MechMurloc,
        )),
        "BG36_130" => Some((
            seaglass::cards::tier3::roaring_recruiter::ID,
            "roaring_recruiter",
            Tribe::Dragon,
        )),
        "BG32_331" => Some((
            seaglass::cards::tier3::shoalfin_mystic::ID,
            "shoalfin_mystic",
            Tribe::Murloc,
        )),
        "BG36_190" => Some((
            seaglass::cards::tier3::sly_infiltrator::ID,
            "sly_infiltrator",
            Tribe::Murloc,
        )),
        "BG27_084" => Some((
            seaglass::cards::tier3::sprightly_scarab::ID,
            "sprightly_scarab",
            Tribe::Beast,
        )),
        "BG36_113" => Some((
            seaglass::cards::tier3::tasty_lobster::ID,
            "tasty_lobster",
            Tribe::Aberration,
        )),
        "BG32_202" => Some((
            seaglass::cards::tier3::thorned_trailblazer::ID,
            "thorned_trailblazer",
            Tribe::Quilboar,
        )),
        "BG34_928" => Some((
            seaglass::cards::tier3::timecapn_hooktail::ID,
            "timecapn_hooktail",
            Tribe::DragonPirate,
        )),
        "BG36_350" => Some((
            seaglass::cards::tier3::trapped_clapper::ID,
            "trapped_clapper",
            Tribe::Mech,
        )),
        "BG36_160" => Some((
            seaglass::cards::tier3::treasure_parrot::ID,
            "treasure_parrot",
            Tribe::Beast,
        )),
        "BG35_891" => Some((
            seaglass::cards::tier3::trench_fighter::ID,
            "trench_fighter",
            Tribe::Pirate,
        )),
        "BG36_174" => Some((
            seaglass::cards::tier3::unwilling_slacker::ID,
            "unwilling_slacker",
            Tribe::Undead,
        )),
        "BG36_340" => Some((
            seaglass::cards::tier3::vicious_mindslasher::ID,
            "vicious_mindslasher",
            Tribe::None,
        )),
        "BG35_844" => Some((
            seaglass::cards::tier3::waveling::ID,
            "waveling",
            Tribe::Elemental,
        )),
        "BGS_126" | "TB_BaconUps_166" => Some((
            seaglass::cards::tier3::wildfire_elemental::ID,
            "wildfire_elemental",
            Tribe::Elemental,
        )),
        "BG36_161" => Some((
            seaglass::cards::tier3::wolf_pup::ID,
            "wolf_pup",
            Tribe::Beast,
        )),
        // Tier 4 Minions
        "BG36_181" => Some((
            seaglass::cards::tier4::air_baller::ID,
            "air_baller",
            Tribe::Elemental,
        )),
        "BG32_873" => Some((
            seaglass::cards::tier4::ashen_corruptor::ID,
            "ashen_corruptor",
            Tribe::Demon,
        )),
        "BG26_802" => Some((
            seaglass::cards::tier4::banana_slamma::ID,
            "banana_slamma",
            Tribe::Beast,
        )),
        "BG33_822" => Some((
            seaglass::cards::tier4::bigwig_bandit::ID,
            "bigwig_bandit",
            Tribe::Pirate,
        )),
        "BG26_817" => Some((
            seaglass::cards::tier4::blade_collector::ID,
            "blade_collector",
            Tribe::Pirate,
        )),
        "BG20_104" => Some((
            seaglass::cards::tier4::bonker::ID,
            "bonker",
            Tribe::Quilboar,
        )),
        "BG36_620" => Some((
            seaglass::cards::tier4::boom_in_a_box::ID,
            "boom_in_a_box",
            Tribe::None,
        )),
        "BG36_331" => Some((
            seaglass::cards::tier4::bramble_tunneler::ID,
            "bramble_tunneler",
            Tribe::Quilboar,
        )),
        "BG26_137" => Some((
            seaglass::cards::tier4::bream_counter::ID,
            "bream_counter",
            Tribe::Murloc,
        )),
        "BG36_242" => Some((
            seaglass::cards::tier4::bronze_timewalker::ID,
            "bronze_timewalker",
            Tribe::Dragon,
        )),
        "BG36_211" => Some((
            seaglass::cards::tier4::cage_gnawer::ID,
            "cage_gnawer",
            Tribe::Beast,
        )),
        "BG34_171" => Some((
            seaglass::cards::tier4::conveyor_construct::ID,
            "conveyor_construct",
            Tribe::Mech,
        )),
        "BG36_106" => Some((
            seaglass::cards::tier4::cutthroat_kthir::ID,
            "cutthroat_kthir",
            Tribe::Aberration,
        )),
        "BG36_360"
        | "BG36_360t3"
        | "BG36_360t4"
        | "BG36_360t5"
        | "BG36_360t6"
        | "BG36_360t9"
        | "BG36_360_Gt3"
        | "BG36_360_Gt4"
        | "BG36_360_Gt5"
        | "BG36_360_Gt6"
        | "BG36_360_Gt9" => Some((
            seaglass::cards::tier4::dark_paradox::ID,
            "dark_paradox",
            Tribe::All,
        )),
        "BG36_511" => Some((
            seaglass::cards::tier4::dead_bellringer::ID,
            "dead_bellringer",
            Tribe::Undead,
        )),
        "BG36_506" => Some((
            seaglass::cards::tier4::drone_duplicator::ID,
            "drone_duplicator",
            Tribe::Mech,
        )),
        "BG34_865" => Some((
            seaglass::cards::tier4::en_djinn_blazer::ID,
            "en_djinn_blazer",
            Tribe::Elemental,
        )),
        "BG35_341" => Some((
            seaglass::cards::tier4::enchanted_sentinel::ID,
            "enchanted_sentinel",
            Tribe::Mech,
        )),
        "BG36_308" => Some((
            seaglass::cards::tier4::faceless_operative::ID,
            "faceless_operative",
            Tribe::Aberration,
        )),
        "BG34_500" => Some((
            seaglass::cards::tier4::flaming_enforcer::ID,
            "flaming_enforcer",
            Tribe::ElementalDemon,
        )),
        "BG32_880" => Some((
            seaglass::cards::tier4::friendly_geist::ID,
            "friendly_geist",
            Tribe::Undead,
        )),
        "BG36_764" => Some((
            seaglass::cards::tier4::gearfin::ID,
            "gearfin",
            Tribe::MechMurloc,
        )),
        "BG28_583" => Some((
            seaglass::cards::tier4::geomagus_roogug::ID,
            "geomagus_roogug",
            Tribe::Quilboar,
        )),
        "BG36_853" => Some((
            seaglass::cards::tier4::glambot::ID,
            "glambot",
            Tribe::Mech,
        )),
        "BG32_336" => Some((
            seaglass::cards::tier4::gormling_gourmet::ID,
            "gormling_gourmet",
            Tribe::Murloc,
        )),
        "BG26_810" => Some((
            seaglass::cards::tier4::gunpowder_courier::ID,
            "gunpowder_courier",
            Tribe::Pirate,
        )),
        "BG36_204" => Some((
            seaglass::cards::tier4::headhunter_gryphon::ID,
            "headhunter_gryphon",
            Tribe::Beast,
        )),
        "BG34_604" => Some((
            seaglass::cards::tier4::heroic_underdog::ID,
            "heroic_underdog",
            Tribe::None,
        )),
        "BG36_210" => Some((
            seaglass::cards::tier4::hoarding_hyena::ID,
            "hoarding_hyena",
            Tribe::Beast,
        )),
        "BG36_372" => Some((
            seaglass::cards::tier4::holy_vanguard::ID,
            "holy_vanguard",
            Tribe::None,
        )),
        "BG30_121" => Some((
            seaglass::cards::tier4::hot_air_surveyor::ID,
            "hot_air_surveyor",
            Tribe::Quilboar,
        )),
        "BG32_341" => Some((
            seaglass::cards::tier4::humongozz::ID,
            "humongozz",
            Tribe::None,
        )),
        "BG31_812" => Some((
            seaglass::cards::tier4::ichoron_the_protector::ID,
            "ichoron_the_protector",
            Tribe::Elemental,
        )),
        "BG36_731" => Some((
            seaglass::cards::tier4::imp_lusionist::ID,
            "imp_lusionist",
            Tribe::Demon,
        )),
        "BG26_525" => Some((
            seaglass::cards::tier4::imposing_percussionist::ID,
            "imposing_percussionist",
            Tribe::Demon,
        )),
        "BG36_701" => Some((
            seaglass::cards::tier4::kelp_keeper::ID,
            "kelp_keeper",
            Tribe::Murloc,
        )),
        "BG35_881" => Some((
            seaglass::cards::tier4::leyline_surfacer::ID,
            "leyline_surfacer",
            Tribe::Elemental,
        )),
        "BG36_180" => Some((
            seaglass::cards::tier4::living_prison::ID,
            "living_prison",
            Tribe::Elemental,
        )),
        "BG26_814" => Some((
            seaglass::cards::tier4::lovesick_balladist::ID,
            "lovesick_balladist",
            Tribe::Pirate,
        )),
        "BG36_524" => Some((
            seaglass::cards::tier4::maritime_extortionist::ID,
            "maritime_extortionist",
            Tribe::Pirate,
        )),
        "BG32_340" => Some((
            seaglass::cards::tier4::maw_caster::ID,
            "maw_caster",
            Tribe::Undead,
        )),
        "BG36_312" => Some((
            seaglass::cards::tier4::mindbending_recruiter::ID,
            "mindbending_recruiter",
            Tribe::Aberration,
        )),
        "BG36_115" => Some((
            seaglass::cards::tier4::nightmare_corroder::ID,
            "nightmare_corroder",
            Tribe::Aberration,
        )),
        "BG36_114" => Some((
            seaglass::cards::tier4::parasitic_fleshling::ID,
            "parasitic_fleshling",
            Tribe::Aberration,
        )),
        "BG29_813" => Some((
            seaglass::cards::tier4::persistent_poet::ID,
            "persistent_poet",
            Tribe::Dragon,
        )),
        "BG34_690" => Some((
            seaglass::cards::tier4::plaguerunner::ID,
            "plaguerunner",
            Tribe::Undead,
        )),
        "BG34_682" => Some((
            seaglass::cards::tier4::razorfen_flapper::ID,
            "razorfen_flapper",
            Tribe::Quilboar,
        )),
        "BGS_116" | "TB_BaconUps_167" => Some((
            seaglass::cards::tier4::refreshing_anomaly::ID,
            "refreshing_anomaly",
            Tribe::Elemental,
        )),
        "BG36_245" => Some((
            seaglass::cards::tier4::runic_arcanist::ID,
            "runic_arcanist",
            Tribe::Dragon,
        )),
        "BG36_362" => Some((
            seaglass::cards::tier4::sacrificial_wrathguard::ID,
            "sacrificial_wrathguard",
            Tribe::Demon,
        )),
        "BG25_016" => Some((
            seaglass::cards::tier4::sindorei_straight_shot::ID,
            "sindorei_straight_shot",
            Tribe::None,
        )),
        "BG36_243" => Some((
            seaglass::cards::tier4::sky_hatch_runaway::ID,
            "sky_hatch_runaway",
            Tribe::Dragon,
        )),
        "BG36_332" => Some((
            seaglass::cards::tier4::snare_trapper::ID,
            "snare_trapper",
            Tribe::Quilboar,
        )),
        "BG36_206" => Some((
            seaglass::cards::tier4::snarky_shark::ID,
            "snarky_shark",
            Tribe::Beast,
        )),
        "BG36_503" => Some((
            seaglass::cards::tier4::soulkeeping_jailer::ID,
            "soulkeeping_jailer",
            Tribe::Demon,
        )),
        "BGS_123" | "TB_BaconUps_162" => Some((
            seaglass::cards::tier4::tavern_tempest::ID,
            "tavern_tempest",
            Tribe::Elemental,
        )),
        "BG24_018" => Some((
            seaglass::cards::tier4::tortollan_blue_shell::ID,
            "tortollan_blue_shell",
            Tribe::None,
        )),
        "BG36_703" => Some((
            seaglass::cards::tier4::twilight_tidehunter::ID,
            "twilight_tidehunter",
            Tribe::Murloc,
        )),
        // Tokens
        "BG28_603t" => Some((tokens::TOKEN_BEETLE, "beetle", Tribe::Beast)),
        "BG36_200t" => Some((tokens::TOKEN_BAT, "bat", Tribe::Beast)),
        "BG_BOT_312t" | "TB_BaconUps_032t" => {
            Some((tokens::TOKEN_MICROBOT, "microbot", Tribe::Mech))
        }
        "BG28_300t" => Some((tokens::TOKEN_SKELETON, "skeleton", Tribe::Undead)),
        "BG36_098t" => Some((
            tokens::TOKEN_ABERRANT_TENTACLE,
            "aberrant_tentacle",
            Tribe::Aberration,
        )),
        "BGS_115t" | "TB_BaconUps_156t" => Some((
            tokens::TOKEN_WATER_DROPLET,
            "water_droplet",
            Tribe::Elemental,
        )),
        "BG35_150t" => Some((tokens::TOKEN_DEMON_FODDER, "demon_fodder", Tribe::Demon)),
        "BG36_201t" => Some((tokens::TOKEN_FISHBAIT, "fishbait", Tribe::Beast)),
        "BG25_010t" => Some((tokens::TOKEN_HELPING_HAND, "helping_hand", Tribe::Undead)),
        "BG31_171t" | "BG31_171_Gt" | "BG34_Giant_610t" | "BG34_Giant_610_Gt" => {
            Some((tokens::TOKEN_SATELLITE, "satellite", Tribe::Mech))
        }
        "BG34_634t" => Some((
            tokens::TOKEN_BLUE_CHROMADRAKE,
            "blue_chromadrake",
            Tribe::Dragon,
        )),
        "BG34_635t" => Some((
            tokens::TOKEN_BLACK_CHROMADRAKE,
            "black_chromadrake",
            Tribe::Dragon,
        )),
        "BG34_636t" => Some((
            tokens::TOKEN_GREEN_CHROMADRAKE,
            "green_chromadrake",
            Tribe::Dragon,
        )),
        "BG34_637t" => Some((
            tokens::TOKEN_BRONZE_CHROMADRAKE,
            "bronze_chromadrake",
            Tribe::Dragon,
        )),
        "BG34_638t" => Some((
            tokens::TOKEN_RED_CHROMADRAKE,
            "red_chromadrake",
            Tribe::Dragon,
        )),
        _ => None,
    }
}

#[derive(Clone, Debug, Default)]
struct RawEntity {
    id: u32,
    card_id: String,
    name: String,
    tags: HashMap<String, String>,
}

impl RawEntity {
    fn tag_int(&self, key: &str) -> i32 {
        self.tags
            .get(key)
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(0)
    }

    fn tag_str(&self, key: &str) -> &str {
        self.tags.get(key).map(|s| s.as_str()).unwrap_or("")
    }

    fn cur_health(&self) -> i32 {
        self.tag_int("HEALTH") - self.tag_int("DAMAGE")
    }

    fn display_name(&self, card_names: &HashMap<String, String>) -> String {
        if !self.name.is_empty() && !self.name.starts_with("UNKNOWN ENTITY") {
            return self.name.clone();
        }
        if let Some(n) = card_names.get(&self.card_id) {
            return n.clone();
        }
        if !self.card_id.is_empty() {
            self.card_id.clone()
        } else {
            format!("Entity#{}", self.id)
        }
    }

    fn to_replay_unit(&self, card_names: &HashMap<String, String>) -> ReplayUnit {
        let mut keywords = Vec::new();
        if self.tag_int("TAUNT") > 0 {
            keywords.push("Taunt".to_string());
        }
        if self.tag_int("DIVINE_SHIELD") > 0 {
            keywords.push("Divine Shield".to_string());
        }
        if self.tag_int("WINDFURY") > 0 {
            keywords.push("Windfury".to_string());
        }
        if self.tag_int("REBORN") > 0 {
            keywords.push("Reborn".to_string());
        }
        if self.tag_int("VENOMOUS") > 0 || self.tag_int("POISONOUS") > 0 {
            keywords.push("Venomous".to_string());
        }
        if self.tag_int("STEALTH") > 0 {
            keywords.push("Stealth".to_string());
        }
        if self.tag_int("MAGNETIC") > 0 || self.tag_int("MODULAR") > 0 {
            keywords.push("Magnetic".to_string());
        }
        ReplayUnit {
            entity_id: self.id,
            card_id: self.card_id.clone(),
            name: self.display_name(card_names),
            attack: self.tag_int("ATK"),
            health: self.cur_health(),
            tier: (self.tag_int("TECH_LEVEL").max(1)) as u32,
            is_golden: self.tag_int("PREMIUM") > 0,
            keywords,
        }
    }

    fn to_replay_card(&self, card_names: &HashMap<String, String>) -> ReplayCard {
        let ctype = self.tag_str("CARDTYPE").to_string();
        let stats = if ctype == "MINION" {
            Some(format!("{}/{}", self.tag_int("ATK"), self.cur_health()))
        } else {
            None
        };
        let tier = if self.tag_int("TECH_LEVEL") > 0 {
            Some(self.tag_int("TECH_LEVEL") as u32)
        } else {
            None
        };
        let cost = if ctype == "BATTLEGROUND_SPELL" || ctype == "SPELL" {
            Some(self.tag_int("COST"))
        } else {
            None
        };
        ReplayCard {
            entity_id: self.id,
            card_id: self.card_id.clone(),
            name: self.display_name(card_names),
            card_type: ctype,
            stats,
            tier,
            cost,
            is_golden: self.tag_int("PREMIUM") > 0,
        }
    }
}

/// Parse a `Power.log` file from disk into a list of [`GameReplay`]s.
pub fn parse_power_log_file(path: &Path) -> Result<Vec<GameReplay>, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    parse_power_log_str(&content)
}

/// Parse the text of a `Power.log` into a list of [`GameReplay`]s.
pub fn parse_power_log_str(content: &str) -> Result<Vec<GameReplay>, String> {
    // Pass 1: Build a global CardID -> human-readable name dictionary from bracketed entities.
    let mut card_names: HashMap<String, String> = HashMap::new();
    for card in full_catalog() {
        card_names.insert(card.name.clone(), card.name);
    }
    for line in content.lines() {
        if !line.contains("GameState.DebugPrintPower() - ") {
            continue;
        }
        let mut rest = line;
        while let Some(pos) = rest.find("[entityName=") {
            let sub = &rest[pos + "[entityName=".len()..];
            let Some(id_pos) = sub.find(" id=") else {
                break;
            };
            let Some(rel_end) = sub[id_pos..].find(']') else {
                break;
            };
            let end_bracket = id_pos + rel_end;
            let inner = &sub[..end_bracket];
            if let Some(cid_pos) = inner.find(" cardId=") {
                let name = &inner[..id_pos];
                let after_cid = &inner[cid_pos + " cardId=".len()..];
                let cid = after_cid.split_whitespace().next().unwrap_or("");
                if !cid.is_empty() && !name.starts_with("UNKNOWN ENTITY") {
                    card_names
                        .entry(cid.to_string())
                        .or_insert_with(|| name.to_string());
                }
            }
            rest = &sub[end_bracket + 1..];
        }
    }

    // Split into games by `GameState.DebugPrintPower() - CREATE_GAME`.
    let mut game_chunks: Vec<(String, Vec<&str>)> = Vec::new();
    for line in content.lines() {
        let Some((prefix, body)) = line.split_once("GameState.DebugPrintPower() - ") else {
            continue;
        };
        let body = body.trim_end_matches('\r');
        if body.trim() == "CREATE_GAME" {
            let ts = prefix
                .split_whitespace()
                .nth(1)
                .unwrap_or("00:00:00")
                .split('.')
                .next()
                .unwrap_or("00:00:00")
                .to_string();
            game_chunks.push((ts, Vec::new()));
        }
        if let Some((_, lines)) = game_chunks.last_mut() {
            lines.push(body);
        }
    }

    let mut replays = Vec::new();
    for (idx, (start_time, lines)) in game_chunks.into_iter().enumerate() {
        if let Some(replay) = parse_single_game(idx + 1, start_time, &lines, &card_names) {
            replays.push(replay);
        }
    }
    Ok(replays)
}

#[derive(Clone, Debug)]
struct ActiveBlock {
    indent: usize,
    block_type: String,
    entity_id: u32,
    source_zone: String,
    target_id: u32,
    target_pos: usize,
    attacker_id: u32,
    defender_id: u32,
    died_units: Vec<String>,
    summoned_units: Vec<String>,
    tripled_card: Option<String>,
}

fn parse_bracket_entity(s: &str) -> Option<(u32, Option<String>, Option<String>)> {
    let s = s.trim();
    if !s.starts_with('[') {
        return None;
    }
    let id_pos_outer = s.find(" id=")?;
    let end = id_pos_outer + s[id_pos_outer..].find(']')?;
    let inner = &s[1..end];
    let id_pos = inner.find(" id=")?;
    let name = inner
        .strip_prefix("entityName=")
        .map(|rest| rest[..id_pos - "entityName=".len()].to_string());
    let after_id = &inner[id_pos + 4..];
    let id_str = after_id.split_whitespace().next()?;
    let id: u32 = id_str.parse().ok()?;
    let cid = inner.find(" cardId=").map(|p| {
        inner[p + 8..]
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string()
    });
    Some((id, name, cid))
}

fn resolve_entity_ref(
    s: &str,
    game_entity_id: u32,
    local_player_entity_id: u32,
    local_player_name: &str,
    dummy_player_entity_id: u32,
    entities: &mut HashMap<u32, RawEntity>,
) -> Option<u32> {
    let s = s.trim();
    if s == "0" || s.is_empty() {
        return None;
    }
    if let Some((id, name, cid)) = parse_bracket_entity(s) {
        let ent = entities.entry(id).or_insert_with(|| RawEntity {
            id,
            ..RawEntity::default()
        });
        if let Some(n) = name {
            if !n.starts_with("UNKNOWN ENTITY") {
                ent.name = n;
            }
        }
        if let Some(c) = cid {
            if !c.is_empty() {
                ent.card_id = c;
            }
        }
        return Some(id);
    }
    if let Ok(id) = s.parse::<u32>() {
        return Some(id);
    }
    if s == "GameEntity" {
        return Some(game_entity_id);
    }
    if !local_player_name.is_empty() && s == local_player_name {
        return Some(local_player_entity_id);
    }
    if s.contains('#') && local_player_entity_id != 0 {
        return Some(local_player_entity_id);
    }
    if dummy_player_entity_id != 0 {
        return Some(dummy_player_entity_id);
    }
    None
}

fn snapshot_board(
    entities: &HashMap<u32, RawEntity>,
    controller_id: i32,
    card_names: &HashMap<String, String>,
) -> Vec<ReplayUnit> {
    let mut units: Vec<(i32, ReplayUnit)> = entities
        .values()
        .filter(|e| {
            e.tag_str("ZONE") == "PLAY"
                && e.tag_str("CARDTYPE") == "MINION"
                && e.tag_int("CONTROLLER") == controller_id
        })
        .map(|e| (e.tag_int("ZONE_POSITION"), e.to_replay_unit(card_names)))
        .collect();
    units.sort_by_key(|(pos, u)| (*pos, u.entity_id));
    units.into_iter().map(|(_, u)| u).collect()
}

fn snapshot_shop(
    entities: &HashMap<u32, RawEntity>,
    dummy_controller_id: i32,
    card_names: &HashMap<String, String>,
) -> Vec<ReplayCard> {
    let mut minions: Vec<(i32, ReplayCard)> = entities
        .values()
        .filter(|e| {
            e.tag_str("ZONE") == "PLAY"
                && e.tag_str("CARDTYPE") == "MINION"
                && e.tag_int("CONTROLLER") == dummy_controller_id
        })
        .map(|e| (e.tag_int("ZONE_POSITION"), e.to_replay_card(card_names)))
        .collect();
    minions.sort_by_key(|(pos, c)| (*pos, c.entity_id));

    let mut spells: Vec<(i32, ReplayCard)> = entities
        .values()
        .filter(|e| {
            e.tag_str("ZONE") == "PLAY"
                && e.tag_str("CARDTYPE") == "BATTLEGROUND_SPELL"
                && e.tag_int("CONTROLLER") == dummy_controller_id
        })
        .map(|e| (e.tag_int("ZONE_POSITION"), e.to_replay_card(card_names)))
        .collect();
    spells.sort_by_key(|(pos, c)| (*pos, c.entity_id));

    minions
        .into_iter()
        .chain(spells)
        .map(|(_, c)| c)
        .collect()
}

fn snapshot_hand(
    entities: &HashMap<u32, RawEntity>,
    controller_id: i32,
    card_names: &HashMap<String, String>,
) -> Vec<ReplayCard> {
    let mut cards: Vec<(i32, ReplayCard)> = entities
        .values()
        .filter(|e| {
            e.tag_str("ZONE") == "HAND"
                && e.tag_int("CONTROLLER") == controller_id
                && matches!(
                    e.tag_str("CARDTYPE"),
                    "MINION" | "SPELL" | "BATTLEGROUND_SPELL"
                )
        })
        .map(|e| (e.tag_int("ZONE_POSITION"), e.to_replay_card(card_names)))
        .collect();
    cards.sort_by_key(|(pos, c)| (*pos, c.entity_id));
    cards.into_iter().map(|(_, c)| c).collect()
}

fn current_gold(entities: &HashMap<u32, RawEntity>, player_eid: u32) -> i32 {
    let Some(p) = entities.get(&player_eid) else {
        return 0;
    };
    (p.tag_int("RESOURCES") + p.tag_int("TEMP_RESOURCES") - p.tag_int("RESOURCES_USED")).max(0)
}

fn find_upgrade_cost(entities: &HashMap<u32, RawEntity>, local_controller: i32) -> Option<i32> {
    entities
        .values()
        .find(|e| {
            e.tag_str("ZONE") == "PLAY"
                && e.tag_int("CONTROLLER") == local_controller
                && e.card_id.starts_with("TB_BaconShopTechUp")
        })
        .map(|e| e.tag_int("COST"))
}

fn find_active_hero(
    entities: &HashMap<u32, RawEntity>,
    controller_id: i32,
    player_eid: u32,
    card_names: &HashMap<String, String>,
) -> (String, String, u32, i32, i32) {
    let hero_eid = entities
        .get(&player_eid)
        .map(|p| p.tag_int("HERO_ENTITY") as u32)
        .unwrap_or(0);
    let hero = entities.get(&hero_eid).or_else(|| {
        entities.values().find(|e| {
            e.tag_str("ZONE") == "PLAY"
                && e.tag_str("CARDTYPE") == "HERO"
                && e.tag_int("CONTROLLER") == controller_id
                && e.card_id != "TB_BaconShopBob"
        })
    });
    if let Some(h) = hero {
        let name = h.display_name(card_names);
        let cid = h.card_id.clone();
        let tier = (h.tag_int("PLAYER_TECH_LEVEL").max(1)) as u32;
        let hp = h.cur_health();
        let armor = h.tag_int("ARMOR");
        (name, cid, tier, hp, armor)
    } else {
        ("Unknown Hero".to_string(), String::new(), 1, 30, 0)
    }
}

fn parse_single_game(
    game_index: usize,
    start_time: String,
    lines: &[&str],
    card_names: &HashMap<String, String>,
) -> Option<GameReplay> {
    let mut entities: HashMap<u32, RawEntity> = HashMap::new();
    let mut game_entity_id: u32 = 22;
    let mut local_player_id: i32 = 0;
    let mut local_player_entity_id: u32 = 0;
    let mut local_player_name = String::new();
    let mut dummy_player_id: i32 = 0;
    let mut dummy_player_entity_id: u32 = 0;

    let mut current_init_entity: Option<(u32, usize)> = None;
    let mut block_stack: Vec<ActiveBlock> = Vec::new();
    let mut turns: Vec<TurnReplay> = Vec::new();
    let mut hero_name = String::new();
    let mut hero_card_id = String::new();
    let mut combat_boards_captured_this_turn = false;
    let mut last_combat_fighter_a: i32 = 0;
    let mut last_combat_fighter_b: i32 = 0;

    for &raw in lines {
        let indent = raw.len() - raw.trim_start().len();
        let s = raw.trim();

        // Close any entity tag initialization block if indentation dropped.
        if let Some((eid, init_indent)) = current_init_entity {
            if let Some(rest) = s.strip_prefix("tag=") {
                if indent > init_indent {
                    if let Some((t, v)) = rest.split_once(" value=") {
                        if let Some(ent) = entities.get_mut(&eid) {
                            ent.tags.insert(t.trim().to_string(), v.trim().to_string());
                        }
                    }
                    continue;
                }
            }
            if let Some(ent) = entities.get(&eid) {
                let in_combat = entities
                    .get(&game_entity_id)
                    .map(|g| g.tag_int("BACON_IN_COMBAT_PHASE") == 1)
                    .unwrap_or(false);
                if !in_combat
                    && ent.tag_int("CONTROLLER") == local_player_id
                    && ent.tag_str("CARDTYPE") == "MINION"
                    && ent.tag_str("ZONE") == "HAND"
                    && ent.tag_int("BACON_TRIPLED_BASE_MINION_ID") > 0
                {
                    let gc = ent.to_replay_unit(card_names).display_short();
                    if let Some(play_blk) = block_stack
                        .iter_mut()
                        .find(|b| b.indent == 0 && b.block_type == "PLAY")
                    {
                        play_blk.tripled_card = Some(gc);
                    } else if let Some(cur_turn) = turns.last_mut() {
                        cur_turn
                            .tavern
                            .actions
                            .push(TavernActionRecord::Triple { golden_card: gc });
                    }
                }
            }
            current_init_entity = None;
        }

        if let Some(rest) = s.strip_prefix("GameEntity EntityID=") {
            if let Ok(eid) = rest.trim().parse::<u32>() {
                game_entity_id = eid;
                entities.insert(
                    eid,
                    RawEntity {
                        id: eid,
                        name: "GameEntity".to_string(),
                        ..RawEntity::default()
                    },
                );
                current_init_entity = Some((eid, indent));
            }
            continue;
        }

        if let Some(rest) = s.strip_prefix("Player EntityID=") {
            // Format: Player EntityID=23 PlayerID=8 GameAccountId=[hi=... lo=...]
            let parts: Vec<&str> = rest.split_whitespace().collect();
            if parts.len() >= 3 {
                let eid = parts[0].parse::<u32>().unwrap_or(0);
                let pid = parts[1]
                    .strip_prefix("PlayerID=")
                    .and_then(|v| v.parse::<i32>().ok())
                    .unwrap_or(0);
                let is_local = !raw.contains("hi=0 lo=0");
                let mut tags = HashMap::new();
                tags.insert("PLAYER_ID".to_string(), pid.to_string());
                entities.insert(
                    eid,
                    RawEntity {
                        id: eid,
                        tags,
                        ..RawEntity::default()
                    },
                );
                if is_local {
                    local_player_id = pid;
                    local_player_entity_id = eid;
                } else {
                    dummy_player_id = pid;
                    dummy_player_entity_id = eid;
                }
                current_init_entity = Some((eid, indent));
            }
            continue;
        }

        if let Some(rest) = s.strip_prefix("FULL_ENTITY - Creating ID=") {
            if let Some((id_str, cid_str)) = rest.split_once(" CardID=") {
                if let Ok(eid) = id_str.trim().parse::<u32>() {
                    let cid = cid_str.trim().to_string();
                    let name = card_names.get(&cid).cloned().unwrap_or_default();
                    entities.insert(
                        eid,
                        RawEntity {
                            id: eid,
                            card_id: cid,
                            name,
                            tags: HashMap::new(),
                        },
                    );
                    current_init_entity = Some((eid, indent));
                }
            }
            continue;
        }

        if let Some(rest) = s
            .strip_prefix("SHOW_ENTITY - Updating Entity=")
            .or_else(|| s.strip_prefix("CHANGE_ENTITY - Updating Entity="))
        {
            if let Some((ent_str, cid_str)) = rest.split_once(" CardID=") {
                if let Some(eid) = resolve_entity_ref(
                    ent_str,
                    game_entity_id,
                    local_player_entity_id,
                    &local_player_name,
                    dummy_player_entity_id,
                    &mut entities,
                ) {
                    let cid = cid_str.trim().to_string();
                    let name = card_names.get(&cid).cloned();
                    let ent = entities.entry(eid).or_insert_with(|| RawEntity {
                        id: eid,
                        ..RawEntity::default()
                    });
                    if !cid.is_empty() {
                        ent.card_id = cid;
                    }
                    if let Some(n) = name {
                        ent.name = n;
                    }
                    current_init_entity = Some((eid, indent));
                }
            }
            continue;
        }

        if let Some(rest) = s.strip_prefix("BLOCK_START BlockType=") {
            let btype = rest.split_whitespace().next().unwrap_or("").to_string();
            let ent_id = rest
                .find(" Entity=")
                .and_then(|p| {
                    let sub = &rest[p + 8..];
                    let end = sub.find(" EffectCardId=")?;
                    resolve_entity_ref(
                        &sub[..end],
                        game_entity_id,
                        local_player_entity_id,
                        &local_player_name,
                        dummy_player_entity_id,
                        &mut entities,
                    )
                })
                .unwrap_or(0);
            let target_id = rest
                .find(" Target=")
                .and_then(|p| {
                    let sub = &rest[p + 8..];
                    let end = sub.find(" SubOption=").unwrap_or(sub.len());
                    resolve_entity_ref(
                        &sub[..end],
                        game_entity_id,
                        local_player_entity_id,
                        &local_player_name,
                        dummy_player_entity_id,
                        &mut entities,
                    )
                })
                .unwrap_or(0);
            let effect_index = rest
                .find(" EffectIndex=")
                .and_then(|p| {
                    let sub = &rest[p + 13..];
                    sub.split_whitespace().next()?.parse::<i32>().ok()
                })
                .unwrap_or(0);

            // Check if this is the combat-start marker (`TB_BaconShop_8P_PlayerE` with `EffectIndex=40`)
            let in_combat = entities
                .get(&game_entity_id)
                .map(|g| g.tag_int("BACON_IN_COMBAT_PHASE") == 1)
                .unwrap_or(false);
            let ent_cid = entities
                .get(&ent_id)
                .map(|e| e.card_id.as_str())
                .unwrap_or("");
            if in_combat
                && ent_cid == "TB_BaconShop_8P_PlayerE"
                && effect_index == 40
                && !combat_boards_captured_this_turn
            {
                combat_boards_captured_this_turn = true;
                let (f_hero, _, f_tier, f_hp, f_armor) = find_active_hero(
                    &entities,
                    local_player_id,
                    local_player_entity_id,
                    card_names,
                );
                let (o_hero, _, o_tier, o_hp, o_armor) = find_active_hero(
                    &entities,
                    dummy_player_id,
                    dummy_player_entity_id,
                    card_names,
                );
                last_combat_fighter_a = entities
                    .get(&local_player_entity_id)
                    .map(|p| p.tag_int("BACON_CURRENT_COMBAT_PLAYER_ID"))
                    .unwrap_or(local_player_id);
                last_combat_fighter_b = entities
                    .get(&dummy_player_entity_id)
                    .map(|p| p.tag_int("BACON_CURRENT_COMBAT_PLAYER_ID"))
                    .unwrap_or(dummy_player_id);

                if let Some(cur_turn) = turns.last_mut() {
                    cur_turn.combat = Some(CombatPhaseReplay {
                        friendly_hero: format!("{f_hero} ({f_hp}+{f_armor} HP)"),
                        friendly_tier: f_tier,
                        opponent_hero: format!("{o_hero} ({o_hp}+{o_armor} HP)"),
                        opponent_tier: o_tier,
                        team_a: snapshot_board(&entities, local_player_id, card_names),
                        team_b: snapshot_board(&entities, dummy_player_id, card_names),
                        events: Vec::new(),
                        outcome: BattleOutcome::Draw,
                        hero_damage: 0,
                        survivors_a: Vec::new(),
                        survivors_b: Vec::new(),
                    });
                }
            }

            if indent == 0
                && (btype == "PLAY"
                    || btype == "MOVE_MINION"
                    || btype == "DECK_ACTION"
                    || btype == "ATTACK")
            {
                while let Some(stale) = block_stack.pop() {
                    handle_block_end(
                        stale,
                        &entities,
                        game_entity_id,
                        local_player_id,
                        local_player_entity_id,
                        dummy_player_id,
                        dummy_player_entity_id,
                        card_names,
                        &mut turns,
                        &mut last_combat_fighter_a,
                        &mut last_combat_fighter_b,
                    );
                }
            }

            if in_combat && combat_boards_captured_this_turn && btype == "ATTACK" {
                check_combat_tag_in(
                    &entities,
                    local_player_id,
                    local_player_entity_id,
                    dummy_player_id,
                    dummy_player_entity_id,
                    card_names,
                    &mut turns,
                    &mut last_combat_fighter_a,
                    &mut last_combat_fighter_b,
                );
            }

            let source_zone = entities
                .get(&ent_id)
                .map(|e| e.tag_str("ZONE").to_string())
                .unwrap_or_default();
            let target_pos = entities
                .get(&target_id)
                .map(|e| (e.tag_int("ZONE_POSITION") - 1).max(0) as usize)
                .unwrap_or(0);

            block_stack.push(ActiveBlock {
                indent,
                block_type: btype,
                entity_id: ent_id,
                source_zone,
                target_id,
                target_pos,
                attacker_id: 0,
                defender_id: 0,
                died_units: Vec::new(),
                summoned_units: Vec::new(),
                tripled_card: None,
            });
            continue;
        }

        if s == "BLOCK_END" {
            if let Some(blk) = block_stack.pop() {
                handle_block_end(
                    blk,
                    &entities,
                    game_entity_id,
                    local_player_id,
                    local_player_entity_id,
                    dummy_player_id,
                    dummy_player_entity_id,
                    card_names,
                    &mut turns,
                    &mut last_combat_fighter_a,
                    &mut last_combat_fighter_b,
                );
            }
            continue;
        }

        if let Some(rest) = s
            .strip_prefix("TAG_CHANGE Entity=")
            .or_else(|| s.strip_prefix("HIDE_ENTITY - Entity="))
        {
            let Some(tag_pos) = rest.find(" tag=") else {
                continue;
            };
            let ent_str = &rest[..tag_pos];
            let after_tag = &rest[tag_pos + 5..];
            let Some((tag_name, tag_val)) = after_tag.split_once(" value=") else {
                continue;
            };
            let tag_name = tag_name.trim();
            let tag_val = tag_val.trim();

            if local_player_name.is_empty() && tag_name == "MULLIGAN_STATE" && !ent_str.starts_with('[')
            {
                local_player_name = ent_str.trim().to_string();
            }

            let Some(eid) = resolve_entity_ref(
                ent_str,
                game_entity_id,
                local_player_entity_id,
                &local_player_name,
                dummy_player_entity_id,
                &mut entities,
            ) else {
                continue;
            };

            // Track combat attack / death / summon metadata inside the active block
            let prev_zone = entities
                .get(&eid)
                .map(|e| e.tag_str("ZONE").to_string())
                .unwrap_or_default();

            entities
                .entry(eid)
                .or_insert_with(|| RawEntity {
                    id: eid,
                    ..RawEntity::default()
                })
                .tags
                .insert(tag_name.to_string(), tag_val.to_string());

            let in_combat = entities
                .get(&game_entity_id)
                .map(|g| g.tag_int("BACON_IN_COMBAT_PHASE") == 1)
                .unwrap_or(false);

            if eid == game_entity_id && tag_name == "PROPOSED_ATTACKER" {
                if let Ok(aid) = tag_val.parse::<u32>() {
                    if aid > 0 {
                        if let Some(blk) = block_stack.iter_mut().rev().find(|b| b.block_type == "ATTACK") {
                            blk.attacker_id = aid;
                        }
                    }
                }
            }
            if eid == game_entity_id && tag_name == "PROPOSED_DEFENDER" {
                if let Ok(did) = tag_val.parse::<u32>() {
                    if did > 0 {
                        if let Some(blk) = block_stack.iter_mut().rev().find(|b| b.block_type == "ATTACK") {
                            blk.defender_id = did;
                        }
                    }
                }
            }

            // Detect minion deaths and summons during combat
            if in_combat && tag_name == "ZONE" {
                if let Some(ent) = entities.get(&eid) {
                    if ent.tag_str("CARDTYPE") == "MINION" {
                        let side_tag = if ent.tag_int("CONTROLLER") == local_player_id {
                            "A"
                        } else {
                            "B"
                        };
                        if tag_val == "GRAVEYARD" && prev_zone == "PLAY" {
                            let label = format!(
                                "[{side_tag}#{}] {}",
                                ent.id,
                                ent.display_name(card_names)
                            );
                            if let Some(blk) = block_stack
                                .iter_mut()
                                .rev()
                                .find(|b| b.block_type == "DEATHS" || b.block_type == "TRIGGER")
                            {
                                blk.died_units.push(label);
                            }
                        } else if tag_val == "PLAY" && prev_zone != "PLAY" && combat_boards_captured_this_turn {
                            let label = format!(
                                "[{side_tag}#{}] {} ({}/{})",
                                ent.id,
                                ent.display_name(card_names),
                                ent.tag_int("ATK"),
                                ent.cur_health()
                            );
                            if let Some(blk) = block_stack.last_mut() {
                                blk.summoned_units.push(label);
                            }
                        }
                    }
                }
            }

            // Detect Triple formation in Tavern phase
            if !in_combat && tag_name == "ZONE" && tag_val == "HAND" {
                if let Some(ent) = entities.get(&eid) {
                    if ent.tag_int("CONTROLLER") == local_player_id
                        && ent.tag_str("CARDTYPE") == "MINION"
                        && ent.tag_int("PREMIUM") > 0
                        && block_stack.iter().any(|b| {
                            entities
                                .get(&b.entity_id)
                                .map(|be| be.card_id == "TB_BaconShop_3ofKindChecke")
                                .unwrap_or(false)
                        })
                    {
                        if let Some(cur_turn) = turns.last_mut() {
                            cur_turn.tavern.actions.push(TavernActionRecord::Triple {
                                golden_card: ent.to_replay_unit(card_names).display_short(),
                            });
                        }
                    }
                }
            }

            // Detect phase transitions on GameEntity
            if eid == game_entity_id && tag_name == "TURN" {
                block_stack.clear();
                let g_turn = tag_val.parse::<u32>().unwrap_or(0);
                if g_turn % 2 == 1 {
                    let round = g_turn.div_ceil(2);
                    turns.push(TurnReplay {
                        turn: round,
                        tavern: TavernPhaseReplay::default(),
                        combat: None,
                    });
                } else {
                    combat_boards_captured_this_turn = false;
                }
            }

            if eid == game_entity_id && tag_name == "STEP" {
                if tag_val == "MAIN_ACTION" && !in_combat {
                    block_stack.clear();
                    let (h_name, h_cid, h_tier, h_hp, h_armor) = find_active_hero(
                        &entities,
                        local_player_id,
                        local_player_entity_id,
                        card_names,
                    );
                    if hero_name.is_empty() && h_name != "Unknown Hero" {
                        hero_name = h_name;
                        hero_card_id = h_cid;
                    }
                    let deity_atk = entities
                        .get(&local_player_entity_id)
                        .map(|p| p.tag_int("BACON_OLD_GOD_ATTACK"))
                        .unwrap_or(0);
                    let deity_hp = entities
                        .get(&local_player_entity_id)
                        .map(|p| p.tag_int("BACON_OLD_GOD_HEALTH"))
                        .unwrap_or(0);
                    let start_gold = current_gold(&entities, local_player_entity_id);
                    let upg_cost = find_upgrade_cost(&entities, local_player_id);
                    let init_board = snapshot_board(&entities, local_player_id, card_names);
                    let init_hand = snapshot_hand(&entities, local_player_id, card_names);
                    let init_shop = snapshot_shop(&entities, dummy_player_id, card_names);

                    if let Some(cur_turn) = turns.last_mut() {
                        cur_turn.tavern.tavern_tier = h_tier;
                        cur_turn.tavern.starting_gold = start_gold;
                        cur_turn.tavern.upgrade_cost = upg_cost;
                        cur_turn.tavern.hero_health = h_hp;
                        cur_turn.tavern.hero_armor = h_armor;
                        cur_turn.tavern.deity_attack = deity_atk;
                        cur_turn.tavern.deity_health = deity_hp;
                        cur_turn.tavern.initial_board = init_board;
                        cur_turn.tavern.initial_hand = init_hand;
                        cur_turn.tavern.initial_shop = init_shop;
                    }
                } else if tag_val == "MAIN_END" && !in_combat {
                    while let Some(stale) = block_stack.pop() {
                        handle_block_end(
                            stale,
                            &entities,
                            game_entity_id,
                            local_player_id,
                            local_player_entity_id,
                            dummy_player_id,
                            dummy_player_entity_id,
                            card_names,
                            &mut turns,
                            &mut last_combat_fighter_a,
                            &mut last_combat_fighter_b,
                        );
                    }
                } else if tag_val == "MAIN_CLEANUP" && !in_combat {
                    let end_board = snapshot_board(&entities, local_player_id, card_names);
                    let end_hand = snapshot_hand(&entities, local_player_id, card_names);
                    let end_shop = snapshot_shop(&entities, dummy_player_id, card_names);
                    let end_gold = current_gold(&entities, local_player_entity_id);
                    let is_frozen = end_shop.iter().any(|c| {
                        entities
                            .get(&c.entity_id)
                            .map(|e| e.tag_int("FROZEN") > 0)
                            .unwrap_or(false)
                    });
                    if let Some(cur_turn) = turns.last_mut() {
                        cur_turn.tavern.final_board = end_board;
                        cur_turn.tavern.final_hand = end_hand;
                        cur_turn.tavern.final_shop = end_shop;
                        cur_turn.tavern.ending_gold = end_gold;
                        cur_turn.tavern.is_frozen = is_frozen;
                    }
                }
            }
        }
    }

    if turns.is_empty() {
        return None;
    }

    let game_seed = entities
        .get(&game_entity_id)
        .and_then(|g| g.tags.get("GAME_SEED"))
        .and_then(|v| v.parse::<i64>().ok());
    let is_duos = entities
        .get(&local_player_entity_id)
        .map(|p| p.tag_int("BACON_DUO_TEAMMATE_PLAYER_ID") > 0)
        .unwrap_or(false);

    Some(GameReplay {
        game_index,
        start_time,
        game_seed,
        is_duos,
        player_name: if local_player_name.is_empty() {
            "Player".to_string()
        } else {
            local_player_name
        },
        hero_name,
        hero_card_id,
        turns,
    })
}

#[allow(clippy::too_many_arguments)]
fn handle_block_end(
    blk: ActiveBlock,
    entities: &HashMap<u32, RawEntity>,
    game_entity_id: u32,
    local_player_id: i32,
    local_player_entity_id: u32,
    dummy_player_id: i32,
    dummy_player_entity_id: u32,
    card_names: &HashMap<String, String>,
    turns: &mut [TurnReplay],
    last_combat_fighter_a: &mut i32,
    last_combat_fighter_b: &mut i32,
) {
    let Some(cur_turn) = turns.last_mut() else {
        return;
    };
    let in_combat = entities
        .get(&game_entity_id)
        .map(|g| g.tag_int("BACON_IN_COMBAT_PHASE") == 1)
        .unwrap_or(false);

    if !in_combat {
        // Tavern Phase: record top-level PLAY blocks as player actions
        if blk.indent == 0 && blk.block_type == "PLAY" {
            let Some(ent) = entities.get(&blk.entity_id) else {
                return;
            };
            let gold_after = current_gold(entities, local_player_entity_id);
            let cid = ent.card_id.as_str();

            if cid == "TB_BaconShop_8p_Reroll_Button" {
                let new_shop = snapshot_shop(entities, dummy_player_id, card_names);
                cur_turn.tavern.actions.push(TavernActionRecord::Refresh {
                    gold_after,
                    new_shop,
                });
            } else if cid == "TB_BaconShopLockAll_Button" {
                let shop = snapshot_shop(entities, dummy_player_id, card_names);
                let is_frozen = shop.iter().any(|c| {
                    entities
                        .get(&c.entity_id)
                        .map(|e| e.tag_int("FROZEN") > 0)
                        .unwrap_or(false)
                });
                cur_turn
                    .tavern
                    .actions
                    .push(TavernActionRecord::Freeze { is_frozen });
            } else if cid.starts_with("TB_BaconShopTechUp") {
                let (_, _, new_tier, _, _) = find_active_hero(
                    entities,
                    local_player_id,
                    local_player_entity_id,
                    card_names,
                );
                cur_turn
                    .tavern
                    .actions
                    .push(TavernActionRecord::UpgradeTier {
                        new_tier,
                        gold_after,
                    });
            } else if cid == "TB_BaconShop_DragBuy" {
                if let Some(target) = entities.get(&blk.target_id) {
                    cur_turn.tavern.actions.push(TavernActionRecord::BuyMinion {
                        card: target.to_replay_unit(card_names).display_short(),
                        shop_pos: Some(blk.target_pos),
                        gold_after,
                    });
                }
            } else if cid == "TB_BaconShop_DragBuy_Spell" {
                if let Some(target) = entities.get(&blk.target_id) {
                    cur_turn.tavern.actions.push(TavernActionRecord::BuySpell {
                        spell: target.display_name(card_names),
                        gold_after,
                    });
                }
            } else if cid == "TB_BaconShop_DragSell" {
                if let Some(target) = entities.get(&blk.target_id) {
                    cur_turn
                        .tavern
                        .actions
                        .push(TavernActionRecord::SellMinion {
                            card: format!(
                                "{} ({}/{})",
                                target.display_name(card_names),
                                target.tag_int("ATK"),
                                target.tag_int("HEALTH")
                            ),
                            gold_after,
                        });
                }
            } else {
                let ctype = ent.tag_str("CARDTYPE");
                let target_desc = entities
                    .get(&blk.target_id)
                    .map(|t| t.display_name(card_names));
                if ctype == "MINION" {
                    // Distinguish playing from hand vs activating an on-board ability
                    if blk.source_zone == "PLAY" {
                        cur_turn.tavern.actions.push(TavernActionRecord::Activate {
                            card: ent.to_replay_unit(card_names).display_short(),
                            target: target_desc,
                            gold_after,
                        });
                    } else {
                        let bpos = (ent.tag_int("ZONE_POSITION") - 1).max(0) as usize;
                        cur_turn
                            .tavern
                            .actions
                            .push(TavernActionRecord::PlayMinion {
                                card: ent.to_replay_unit(card_names).display_short(),
                                board_pos: Some(bpos),
                                target: target_desc,
                                gold_after,
                            });
                    }
                } else if ctype == "SPELL" || ctype == "BATTLEGROUND_SPELL" {
                    cur_turn.tavern.actions.push(TavernActionRecord::CastSpell {
                        spell: ent.display_name(card_names),
                        target: target_desc,
                        gold_after,
                    });
                } else if ctype == "HERO_POWER" {
                    cur_turn.tavern.actions.push(TavernActionRecord::HeroPower {
                        card: ent.display_name(card_names),
                        target: target_desc,
                        gold_after,
                    });
                } else if blk.source_zone == "PLAY" {
                    cur_turn.tavern.actions.push(TavernActionRecord::Activate {
                        card: ent.display_name(card_names),
                        target: target_desc,
                        gold_after,
                    });
                }
            }
            if let Some(gc) = blk.tripled_card {
                cur_turn
                    .tavern
                    .actions
                    .push(TavernActionRecord::Triple { golden_card: gc });
            }
        } else if blk.indent == 0 && blk.block_type == "DECK_ACTION" {
            if let Some(ent) = entities.get(&blk.entity_id) {
                let gold_after = current_gold(entities, local_player_entity_id);
                let card = if ent.tag_str("CARDTYPE") == "MINION" {
                    ent.to_replay_unit(card_names).display_short()
                } else {
                    ent.display_name(card_names)
                };
                cur_turn
                    .tavern
                    .actions
                    .push(TavernActionRecord::PassCard { card, gold_after });
            }
        }
        return;
    }

    // Combat Phase
    check_combat_tag_in(
        entities,
        local_player_id,
        local_player_entity_id,
        dummy_player_id,
        dummy_player_entity_id,
        card_names,
        turns,
        last_combat_fighter_a,
        last_combat_fighter_b,
    );

    let Some(cur_turn) = turns.last_mut() else {
        return;
    };
    let Some(combat) = cur_turn.combat.as_mut() else {
        return;
    };

    if blk.block_type == "ATTACK" && blk.attacker_id > 0 && blk.defender_id > 0 {
        let Some(att) = entities.get(&blk.attacker_id) else {
            return;
        };
        let Some(def) = entities.get(&blk.defender_id) else {
            return;
        };

        if att.tag_str("CARDTYPE") == "HERO" || def.tag_str("CARDTYPE") == "HERO" {
            // End-of-combat Hero strike!
            let survivors_a = snapshot_board(entities, local_player_id, card_names);
            let survivors_b = snapshot_board(entities, dummy_player_id, card_names);
            let dmg = att.tag_int("ATK").max(0);
            let friendly_won = att.tag_int("CONTROLLER") == local_player_id;
            combat.outcome = if friendly_won {
                BattleOutcome::AWin
            } else {
                BattleOutcome::BWin
            };
            combat.hero_damage = if friendly_won { dmg } else { -dmg };
            combat.survivors_a = survivors_a;
            combat.survivors_b = survivors_b;
            combat.events.push(CombatLogEvent::HeroDamage {
                attacker_hero: att.display_name(card_names),
                defender_hero: def.display_name(card_names),
                damage: dmg,
            });
        } else {
            let side = if att.tag_int("CONTROLLER") == local_player_id {
                "Side A"
            } else {
                "Side B"
            };
            let att_side = if att.tag_int("CONTROLLER") == local_player_id {
                "A"
            } else {
                "B"
            };
            let def_side = if def.tag_int("CONTROLLER") == local_player_id {
                "A"
            } else {
                "B"
            };
            combat.events.push(CombatLogEvent::Attack {
                side: side.to_string(),
                attacker: format!("[{att_side}#{}] {}", att.id, att.display_name(card_names)),
                defender: format!("[{def_side}#{}] {}", def.id, def.display_name(card_names)),
                attacker_after: format!("{}/{}", att.tag_int("ATK"), att.cur_health()),
                defender_after: format!("{}/{}", def.tag_int("ATK"), def.cur_health()),
            });
        }
    } else if blk.block_type == "DEATHS" && !blk.died_units.is_empty() {
        combat.events.push(CombatLogEvent::Deaths {
            units: blk.died_units,
        });
    } else if blk.block_type == "TRIGGER" && !blk.summoned_units.is_empty() {
        if let Some(src) = entities.get(&blk.entity_id) {
            if src.tag_str("CARDTYPE") == "MINION" {
                let s_tag = if src.tag_int("CONTROLLER") == local_player_id {
                    "A"
                } else {
                    "B"
                };
                combat.events.push(CombatLogEvent::Trigger {
                    source: format!("[{s_tag}#{}] {}", src.id, src.display_name(card_names)),
                    summary: format!("Summons {}", blk.summoned_units.join(", ")),
                });
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_combat_tag_in(
    entities: &HashMap<u32, RawEntity>,
    local_player_id: i32,
    local_player_entity_id: u32,
    dummy_player_id: i32,
    dummy_player_entity_id: u32,
    card_names: &HashMap<String, String>,
    turns: &mut [TurnReplay],
    last_combat_fighter_a: &mut i32,
    last_combat_fighter_b: &mut i32,
) {
    let Some(cur_turn) = turns.last_mut() else {
        return;
    };
    let Some(combat) = cur_turn.combat.as_mut() else {
        return;
    };

    let cur_fighter_a = entities
        .get(&local_player_entity_id)
        .map(|p| p.tag_int("BACON_CURRENT_COMBAT_PLAYER_ID"))
        .unwrap_or(local_player_id);
    let cur_fighter_b = entities
        .get(&dummy_player_entity_id)
        .map(|p| p.tag_int("BACON_CURRENT_COMBAT_PLAYER_ID"))
        .unwrap_or(dummy_player_id);

    if cur_fighter_a != 0 && cur_fighter_a != *last_combat_fighter_a {
        *last_combat_fighter_a = cur_fighter_a;
        let (h_name, _, _, _, _) = find_active_hero(
            entities,
            local_player_id,
            local_player_entity_id,
            card_names,
        );
        let board = snapshot_board(entities, local_player_id, card_names);
        if !board.is_empty() {
            combat.events.push(CombatLogEvent::TagIn {
                side: "Side A".to_string(),
                hero: h_name,
                board,
            });
        }
    }
    if cur_fighter_b != 0 && cur_fighter_b != *last_combat_fighter_b {
        *last_combat_fighter_b = cur_fighter_b;
        let (h_name, _, _, _, _) = find_active_hero(
            entities,
            dummy_player_id,
            dummy_player_entity_id,
            card_names,
        );
        let board = snapshot_board(entities, dummy_player_id, card_names);
        if !board.is_empty() {
            combat.events.push(CombatLogEvent::TagIn {
                side: "Side B".to_string(),
                hero: h_name,
                board,
            });
        }
    }
}

