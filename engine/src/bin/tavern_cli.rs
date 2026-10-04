//! Interactive CLI sandbox for inspecting and testing the Seaglass Tavern (Recruit) Phase.
//!
//! Usage:
//!   cargo run --bin tavern_cli              # Interactive mode (default seed: 42)
//!   cargo run --bin tavern_cli -- 123       # Interactive mode with custom seed
//!   cargo run --bin tavern_cli -- --demo    # Scripted multi-turn walkthrough

use std::env;
use std::io::{self, Write};

use seaglass::cards::{self, tokens, CardTemplate};
use seaglass::{
    base_copies_for_tier, full_catalog, CardId, CardPool, DeityKind, Rng, TavernAction,
    TavernState, Tribe, Unit,
};

fn card_description(card_id: CardId, is_golden: bool) -> &'static str {
    use cards::spells::*;
    use cards::tier1::*;
    use cards::tier2::*;
    use cards::tier3::*;
    use cards::tier4::*;
    match (card_id, is_golden) {
        (joyous::ID, false) => "Battlecry: Give your Deity +2/+1.",
        (joyous::ID, true) => "Battlecry: Give your Deity +4/+2.",
        (zoatroid::ID, false) => "When you sell this, get a 0/2 Tentacle with Taunt.",
        (zoatroid::ID, true) => "When you sell this, get two 0/2 Tentacles with Taunt.",
        (buzzing_vermin::ID, false) => "Taunt. Deathrattle: Summon a 2/2 Beetle.",
        (buzzing_vermin::ID, true) => "Taunt. Deathrattle: Summon a 4/4 Beetle.",
        (flittering_bat::ID, false) => "Rally: Summon a 1/1 Beast.",
        (flittering_bat::ID, true) => "Rally: Summon a 2/2 Beast.",
        (wrath_weaver::ID, false) => {
            "After you play a Demon, deal 1 dmg to your hero and gain +2/+2."
        }
        (wrath_weaver::ID, true) => {
            "After you play a Demon, deal 1 dmg to your hero and gain +2/+2 twice."
        }
        (glim_guardian::ID, false) => "Rally: Gain +2 Attack.",
        (glim_guardian::ID, true) => "Rally: Gain +4 Attack.",
        (scarlet_survivor::ID, _) => "Once this reaches 6 Attack, gain Divine Shield.",
        (crackling_cyclone::ID, _) => "Divine Shield, Windfury.",
        (dune_dweller::ID, false) => "Battlecry: Give Elementals in the Tavern +1/+1 this game.",
        (dune_dweller::ID, true) => "Battlecry: Give Elementals in the Tavern +2/+2 this game.",
        (cord_puller::ID, false) => "Divine Shield. Deathrattle: Summon a 1/1 Microbot.",
        (cord_puller::ID, true) => "Divine Shield. Deathrattle: Summon a 2/2 Microbot.",
        (lullabot::ID, false) => "Magnetic. At the end of your turn, gain +1 Health.",
        (lullabot::ID, true) => "Magnetic. At the end of your turn, gain +2 Health.",
        (bubble_gunner::ID, false) => "Battlecry: Gain a random Bonus Keyword.",
        (bubble_gunner::ID, true) => "Battlecry: Gain 2 random Bonus Keywords.",
        (flighty_scout::ID, false) => {
            "Start of Combat: If this is in your hand, summon a copy of it."
        }
        (flighty_scout::ID, true) => {
            "Start of Combat: If this is in your hand, summon 2 copies of it."
        }
        (aureate_laureate::ID, _) => "Divine Shield. Always Golden (no Triple Reward).",
        (southsea_busker::ID, false) => "Battlecry: Gain 1 Gold next turn.",
        (southsea_busker::ID, true) => "Battlecry: Gain 2 Gold next turn.",
        (razorfen_geomancer::ID, false) => "Battlecry: Get 2 Blood Gems.",
        (razorfen_geomancer::ID, true) => "Battlecry: Get 4 Blood Gems.",
        (tusked_camper::ID, false) => "Rally: Plays a Blood Gem on itself.",
        (tusked_camper::ID, true) => "Rally: Plays 2 Blood Gems on itself.",
        (harmless_bonehead::ID, false) => "Deathrattle: Summon two 1/1 Skeletons.",
        (harmless_bonehead::ID, true) => "Deathrattle: Summon two 2/2 Skeletons.",
        (risen_rider::ID, _) => "Taunt, Reborn.",
        (ominous_seer::ID, false) => "Battlecry: The next Tavern spell you buy costs (1) less.",
        (ominous_seer::ID, true) => "Battlecry: The next Tavern spell you buy costs (2) less.",
        (suspicious_prisonguard::ID, false) => "Activate (1g): Give another minion +3/+3.",
        (suspicious_prisonguard::ID, true) => "Activate (1g): Give another minion +6/+6.",
        // Tier 2 Minions
        (bilgewater_breakout::ID, false) => {
            "Battlecry: Get a Lockbox, or accelerate yours by 1 turn."
        }
        (bilgewater_breakout::ID, true) => {
            "Battlecry: Get a Lockbox, or accelerate yours by 2 turns."
        }
        (blue_volumizer::ID, false) => {
            "Magnetic. First play/Magnetize gives Volumizers +3 Health this game."
        }
        (blue_volumizer::ID, true) => {
            "Magnetic. First play/Magnetize gives Volumizers +6 Health this game."
        }
        (brain_rotter::ID, false) => "Activate (0g): Discard a hand card to give Deity +2/+2.",
        (brain_rotter::ID, true) => "Activate (0g): Discard a hand card to give Deity +4/+4.",
        (bronze_warden::ID, _) => "Divine Shield, Reborn.",
        (clever_castaway::ID, false) => "Activate (2g): Discover a Tavern spell.",
        (clever_castaway::ID, true) => "Activate (2g): Discover 2 Tavern spells.",
        (crater_miner::ID, false) => "Choose One: Get 2 Blood Gems; or Get a Gem Day.",
        (crater_miner::ID, true) => "Choose One: Get 4 Blood Gems; or Get 2 Gem Days.",
        (decoy_conjurer::ID, false) => "Activate (2g): Steal the highest-Attack minion in shop.",
        (decoy_conjurer::ID, true) => "Activate (2g): Steal the 2 highest-Attack minions in shop.",
        (electric_synthesizer::ID, false) => {
            "Battlecry & Start of Combat: Give other Dragons +1/+1."
        }
        (electric_synthesizer::ID, true) => {
            "Battlecry & Start of Combat: Give other Dragons +2/+2."
        }
        (eternal_knight::ID, false) => {
            "Has +4/+2 for each friendly Eternal Knight that died this game."
        }
        (eternal_knight::ID, true) => {
            "Has +8/+4 for each friendly Eternal Knight that died this game."
        }
        (expert_aviator::ID, false) => "Rally: Summon highest-Attack Murloc from hand for combat.",
        (expert_aviator::ID, true) => {
            "Rally: Summon 2 highest-Attack Murlocs from hand for combat."
        }
        (fire_baller::ID, false) => {
            "On sell: Give your minions +1 Attack (upgrades future Ballers)."
        }
        (fire_baller::ID, true) => {
            "On sell: Give your minions +2 Attack (upgrades future Ballers by 2)."
        }
        (forest_rover::ID, false) => {
            "Battlecry: Beetles have +2/+1. Deathrattle: Summon a 2/2 Beetle."
        }
        (forest_rover::ID, true) => {
            "Battlecry: Beetles have +4/+2. Deathrattle: Summon two 2/2 Beetles."
        }
        (green_volumizer::ID, false) => {
            "Magnetic. First play/Magnetize gives Volumizers +1/+1 this game."
        }
        (green_volumizer::ID, true) => {
            "Magnetic. First play/Magnetize gives Volumizers +2/+2 this game."
        }
        (humming_bird::ID, false) => "Start of Combat: Your Beasts have +1 Attack this combat.",
        (humming_bird::ID, true) => "Start of Combat: Your Beasts have +2 Attack this combat.",
        (intrepid_botanist::ID, false) => {
            "Choose One: Tavern spells give an extra +1 Attack; or +1 Health."
        }
        (intrepid_botanist::ID, true) => {
            "Choose One: Tavern spells give an extra +2 Attack; or +2 Health."
        }
        (laboratory_assistant::ID, false) => "Battlecry: Add a Fodder to your next 3 Refreshes.",
        (laboratory_assistant::ID, true) => "Battlecry: Add 2 Fodders to your next 3 Refreshes.",
        (lurking_lionfish::ID, _) => {
            "Activate (2g): Replace a shop card with Fishbait for left-most Beast to attack."
        }
        (mechagnome_interpreter::ID, false) => {
            "Whenever you play or Magnetize a Mech, give it +3/+1."
        }
        (mechagnome_interpreter::ID, true) => {
            "Whenever you play or Magnetize a Mech, give it +6/+2."
        }
        (mind_muck::ID, false) => {
            "Battlecry: Friendly Demon consumes a shop minion to gain its stats."
        }
        (mind_muck::ID, true) => {
            "Battlecry: Friendly Demon consumes a shop minion to gain double its stats."
        }
        (nerubian_deathswarmer::ID, false) => "Battlecry: Your Undead have +1 Attack this game.",
        (nerubian_deathswarmer::ID, true) => "Battlecry: Your Undead have +2 Attack this game.",
        (patient_scout::ID, false) => "On sell: Discover a minion (tier upgrades each turn).",
        (patient_scout::ID, true) => "On sell: Discover 2 minions (tier upgrades each turn).",
        (prodigious_tusker::ID, false) => {
            "Whenever another friendly minion attacks, play a Blood Gem on it."
        }
        (prodigious_tusker::ID, true) => {
            "Whenever another friendly minion attacks, play 2 Blood Gems on it."
        }
        (red_volumizer::ID, false) => {
            "Magnetic. First play/Magnetize gives Volumizers +3 Attack this game."
        }
        (red_volumizer::ID, true) => {
            "Magnetic. First play/Magnetize gives Volumizers +6 Attack this game."
        }
        (roadboar::ID, false) => "Rally: Get a Blood Gem.",
        (roadboar::ID, true) => "Rally: Get 2 Blood Gems.",
        (scarlet_skull::ID, false) => "Reborn. Deathrattle: Give a friendly Undead +1/+2.",
        (scarlet_skull::ID, true) => "Reborn. Deathrattle: Give a friendly Undead +2/+4.",
        (sellemental::ID, false) => "On sell: Get a 3/3 Water Droplet.",
        (sellemental::ID, true) => "On sell: Get two 3/3 Water Droplets.",
        (snow_baller::ID, false) => {
            "On sell: Give your minions +1 Health (upgrades future Ballers)."
        }
        (snow_baller::ID, true) => {
            "On sell: Give your minions +2 Health (upgrades future Ballers by 2)."
        }
        (soul_rewinder::ID, false) => "After hero takes damage, rewind it and gain +2 Health.",
        (soul_rewinder::ID, true) => "After hero takes damage, rewind it and gain +4 Health.",
        (surfing_sylvar::ID, false) => {
            "End of Turn: Give adjacent minions +1 Attack (repeats per Golden minion)."
        }
        (surfing_sylvar::ID, true) => {
            "End of Turn: Give adjacent minions +2 Attack (repeats per Golden minion)."
        }
        (tad::ID, false) => "On sell: Get another random Murloc.",
        (tad::ID, true) => "On sell: Get 2 other random Murlocs.",
        (tarecgosa::ID, false) => "Permanently keeps Bonus Keywords and stats gained in combat.",
        (tarecgosa::ID, true) => {
            "Permanently keeps Bonus Keywords and double stats gained in combat."
        }
        (underrot_spawn::ID, false) => {
            "Deathrattle: Summon a 0/2 Tentacle with Taunt; give minions +1 Attack."
        }
        (underrot_spawn::ID, true) => {
            "Deathrattle: Summon two 0/2 Tentacles with Taunt; give minions +2 Attack."
        }
        (very_hungry_winterfinner::ID, false) => {
            "Taunt. Whenever this takes damage, give a random minion in hand +2/+1."
        }
        (very_hungry_winterfinner::ID, true) => {
            "Taunt. Whenever this takes damage, give a random minion in hand +4/+2."
        }
        (wandering_willbreaker::ID, false) => {
            "On sell: Get 2 random Tavern spells; after casting 1, discard the other."
        }
        (wandering_willbreaker::ID, true) => {
            "On sell: Get 4 random Tavern spells; after casting 2, discard the rest."
        }
        // Tier 3 Minions
        (abyssal_envoy::ID, false) => {
            "Activate (0g): Discard a card in hand to get a random Tavern spell."
        }
        (abyssal_envoy::ID, true) => {
            "Activate (0g): Discard a card in hand to get 2 random Tavern spells."
        }
        (accord_o_tron::ID, false) => "Magnetic. At the start of your turn, gain 1 Gold.",
        (accord_o_tron::ID, true) => "Magnetic. At the start of your turn, gain 2 Gold.",
        (amber_guardian::ID, false) => {
            "Start of Combat: Give another friendly Dragon +2/+2 and Divine Shield."
        }
        (amber_guardian::ID, true) => {
            "Start of Combat: Give 2 other friendly Dragons +4/+4 and Divine Shield."
        }
        (annoy_o_module::ID, _) => "Magnetic, Divine Shield, Taunt.",
        (auto_accelerator::ID, false) => "Battlecry: Get a random Magnetic Volumizer.",
        (auto_accelerator::ID, true) => "Battlecry: Get 2 random Magnetic Volumizers.",
        (azsharan_cutlassier::ID, false) => {
            "Battlecry: Your Tavern spells give an extra +1 Attack this game."
        }
        (azsharan_cutlassier::ID, true) => {
            "Battlecry: Your Tavern spells give an extra +2 Attack this game."
        }
        (blue_whelp::ID, false) => "Rally: Your Tavern spells give an extra +1 Health this game.",
        (blue_whelp::ID, true) => "Rally: Your Tavern spells give an extra +2 Health this game.",
        (cadaver_caretaker::ID, false) => "Deathrattle: Summon three 1/1 Skeletons.",
        (cadaver_caretaker::ID, true) => "Deathrattle: Summon six 1/1 Skeletons.",
        (deadly_spore::ID, _) => "Venomous.",
        (devout_hellcaller::ID, false) => {
            "After another friendly Demon deals damage, gain +2/+2 permanently."
        }
        (devout_hellcaller::ID, true) => {
            "After another friendly Demon deals damage, gain +4/+4 permanently."
        }
        (diremuck_forager::ID, false) => {
            "Start of Combat: Summon the highest-Attack Murloc from your hand for combat."
        }
        (diremuck_forager::ID, true) => {
            "Start of Combat: Summon the 2 highest-Attack Murlocs from your hand for combat."
        }
        (disguised_graverobber::ID, false) => {
            "Battlecry: Destroy a friendly Undead to get a plain copy of it."
        }
        (disguised_graverobber::ID, true) => {
            "Battlecry: Destroy a friendly Undead to get 2 plain copies of it."
        }
        (drifting_sacrifice::ID, false) => "Reborn. Deathrattle: Give your Deity +2/+1.",
        (drifting_sacrifice::ID, true) => "Reborn. Deathrattle: Give your Deity +4/+2.",
        (fearless_foodie::ID, false) => {
            "Choose One: Blood Gems give an extra +1/+1 this game; or Get 4 Blood Gems."
        }
        (fearless_foodie::ID, true) => {
            "Choose One: Blood Gems give an extra +2/+2 this game; or Get 8 Blood Gems."
        }
        (fetid_corroder::ID, false) => "Battlecry: Get a Sludge Corrosion.",
        (fetid_corroder::ID, true) => "Battlecry: Get 2 Sludge Corrosions.",
        (fruit_vendor::ID, false) => "Activate (1g): Get 2 Tavern Dish Bananas.",
        (fruit_vendor::ID, true) => "Activate (1g): Get 4 Tavern Dish Bananas.",
        (gem_rat::ID, false) => "At the end of your turn, get a Gem Day.",
        (gem_rat::ID, true) => "At the end of your turn, get 2 Gem Days.",
        (greedy_conniver::ID, _) => {
            "If this is Golden when you sell it, Discover a Tier 7 minion."
        }
        (handless_forsaken::ID, false) => "Deathrattle: Summon a 2/1 Hand with Reborn.",
        (handless_forsaken::ID, true) => "Deathrattle: Summon two 2/1 Hands with Reborn.",
        (hired_mount::ID, false) => "Activate (2g): Get a random Chromadrake.",
        (hired_mount::ID, true) => "Activate (2g): Get 2 random Chromadrakes.",
        (iron_groundskeeper::ID, false) => "Battlecry: Get 2 copies of Fortify.",
        (iron_groundskeeper::ID, true) => "Battlecry: Get 4 copies of Fortify.",
        (locked_up_mutineer::ID, false) => {
            "Deathrattle: Get a Lockbox, or accelerate yours by 1 turn."
        }
        (locked_up_mutineer::ID, true) => {
            "Deathrattle: Get a Lockbox, or accelerate yours by 2 turns."
        }
        (malchezaar_prince_of_dance::ID, false) => {
            "2 Refreshes each turn cost Health instead of Gold."
        }
        (malchezaar_prince_of_dance::ID, true) => {
            "4 Refreshes each turn cost Health instead of Gold."
        }
        (mangled_bandit::ID, false) => "Activate (0g): Discard a hand card to get 3 Blood Gems.",
        (mangled_bandit::ID, true) => "Activate (0g): Discard a hand card to get 6 Blood Gems.",
        (mummifier::ID, false) => "Deathrattle: Give a different friendly Undead Reborn.",
        (mummifier::ID, true) => "Deathrattle: Give 2 different friendly Undead Reborn.",
        (prosthetic_hand::ID, _) => "Magnetic, Reborn. Can Magnetize to Mechs or Undead.",
        (relentless_deflector::ID, _) => {
            "Has Taunt while this has Divine Shield. Avenge (3): Gain Divine Shield."
        }
        (rescue_bot::ID, false) => "Taunt. Deathrattle: Get a Repair Job.",
        (rescue_bot::ID, true) => "Taunt. Deathrattle: Get 2 Repair Jobs.",
        (roaring_recruiter::ID, false) => {
            "Whenever another friendly Dragon attacks, give it +3/+1."
        }
        (roaring_recruiter::ID, true) => {
            "Whenever another friendly Dragon attacks, give it +6/+2."
        }
        (shoalfin_mystic::ID, false) => {
            "When you sell this, your Tavern spells give an extra +1/+1 this game."
        }
        (shoalfin_mystic::ID, true) => {
            "When you sell this, your Tavern spells give an extra +2/+2 this game."
        }
        (sly_infiltrator::ID, false) => {
            "Choose One: Gain 2 free Refreshes; or Get 3 Blood Gems."
        }
        (sly_infiltrator::ID, true) => {
            "Choose One: Gain 4 free Refreshes; or Get 6 Blood Gems."
        }
        (sprightly_scarab::ID, false) => {
            "Choose One: Give a Beast +1/+1 & Reborn; or +4 Attack & Windfury."
        }
        (sprightly_scarab::ID, true) => {
            "Choose One: Give a Beast +2/+2 & Reborn; or +8 Attack & Windfury."
        }
        (tasty_lobster::ID, false) => {
            "Deathrattle: Give a random friendly Beast +2/+1. Improve your future Tasty Lobsters."
        }
        (tasty_lobster::ID, true) => {
            "Deathrattle: Give a random friendly Beast +4/+2. Improve your future Tasty Lobsters."
        }
        (thorned_trailblazer::ID, false) => {
            "1 Choose One card each turn has both effects combined."
        }
        (thorned_trailblazer::ID, true) => {
            "2 Choose One cards each turn have both effects combined."
        }
        (timecapn_hooktail::ID, false) => {
            "Whenever you cast a Tavern spell, give your minions +1 Attack."
        }
        (timecapn_hooktail::ID, true) => {
            "Whenever you cast a Tavern spell, give your minions +1 Attack twice."
        }
        (trapped_clapper::ID, false) => "Deathrattle: Add a Fodder to your next 3 Refreshes.",
        (trapped_clapper::ID, true) => "Deathrattle: Add 2 Fodders to your next 3 Refreshes.",
        (treasure_parrot::ID, false) => "Once this deals 35 damage, get a Golden Touch.",
        (treasure_parrot::ID, true) => "Once this deals 35 damage, get 2 Golden Touches.",
        (trench_fighter::ID, false) => "At the end of your turn, get a Gem Confiscation.",
        (trench_fighter::ID, true) => "At the end of your turn, get 2 Gem Confiscations.",
        (unwilling_slacker::ID, false) => "Deathrattle: Get a random 1-Cost Tavern spell.",
        (unwilling_slacker::ID, true) => "Deathrattle: Get 2 random 1-Cost Tavern spells.",
        (vicious_mindslasher::ID, false) => {
            "Whenever you cast a Tavern spell, give this and your Deity +1/+2."
        }
        (vicious_mindslasher::ID, true) => {
            "Whenever you cast a Tavern spell, give this and your Deity +2/+4."
        }
        (waveling::ID, false) => {
            "Deathrattle: After Tavern is Refreshed this game, give a minion in it +4/+4."
        }
        (waveling::ID, true) => {
            "Deathrattle: After Tavern is Refreshed this game, give a minion in it +4/+4 twice."
        }
        (wildfire_elemental::ID, false) => {
            "After this attacks and kills a minion, deal excess damage to an adjacent enemy."
        }
        (wildfire_elemental::ID, true) => {
            "After this attacks and kills a minion, deal excess damage to both adjacent enemies."
        }
        (wolf_pup::ID, false) => "Rally: Give your other minions +4/+1.",
        (wolf_pup::ID, true) => "Rally: Give your other minions +8/+2.",
        // Tier 4 Minions
        (air_baller::ID, false) => {
            "On sell: Give your minions +2/+2 (upgrades future Ballers)."
        }
        (air_baller::ID, true) => {
            "On sell: Give your minions +4/+4 (upgrades future Ballers by 2)."
        }
        (ashen_corruptor::ID, false) => {
            "After hero takes damage, rewind it and give shop minions +2/+2 this turn."
        }
        (ashen_corruptor::ID, true) => {
            "After hero takes damage, rewind it and give shop minions +4/+4 this turn."
        }
        (banana_slamma::ID, false) => {
            "After you summon a Beast in combat, double its Attack."
        }
        (banana_slamma::ID, true) => {
            "After you summon a Beast in combat, triple its Attack."
        }
        (bigwig_bandit::ID, false) => "Rally: Get a random Bounty.",
        (bigwig_bandit::ID, true) => "Rally: Get 2 random Bounties.",
        (blade_collector::ID, _) => "Also damages the enemies next to whomever this attacks.",
        (bonker::ID, false) => "Windfury. Rally: Play a Blood Gem on all your other minions.",
        (bonker::ID, true) => "Windfury. Rally: Play 2 Blood Gems on all your other minions.",
        (boom_in_a_box::ID, false) => {
            "Taunt. Start of Combat: Deal 3 damage to all other minions."
        }
        (boom_in_a_box::ID, true) => {
            "Taunt. Start of Combat: Deal 3 damage to all other minions twice."
        }
        (bramble_tunneler::ID, false) => "Rally: Get a random Choose One card.",
        (bramble_tunneler::ID, true) => "Rally: Get 2 random Choose One cards.",
        (bream_counter::ID, false) => {
            "While in hand, after you play a Murloc, gain +6/+6."
        }
        (bream_counter::ID, true) => {
            "While in hand, after you play a Murloc, gain +12/+12."
        }
        (bronze_timewalker::ID, false) => "Rally: Get a random Chromadrake.",
        (bronze_timewalker::ID, true) => "Rally: Get 2 random Chromadrakes.",
        (cage_gnawer::ID, false) => {
            "Whenever a friendly Beast attacks, give your Beasts +2/+1."
        }
        (cage_gnawer::ID, true) => {
            "Whenever a friendly Beast attacks, give your Beasts +4/+2."
        }
        (conveyor_construct::ID, false) => {
            "Deathrattle: Get a random Magnetic Volumizer."
        }
        (conveyor_construct::ID, true) => {
            "Deathrattle: Get 2 random Magnetic Volumizers."
        }
        (cutthroat_kthir::ID, false) => {
            "Whenever you discard a card, give this and your Deity +4/+4."
        }
        (cutthroat_kthir::ID, true) => {
            "Whenever you discard a card, give this and your Deity +8/+8."
        }
        (dark_paradox::ID, false) => {
            "Rally: Get a random minion of your most common type."
        }
        (dark_paradox::ID, true) => {
            "Rally: Get 2 random minions of your most common type."
        }
        (dead_bellringer::ID, false) => {
            "Activate (1g): Give another Undead Reborn, then destroy it to gain +4/+4."
        }
        (dead_bellringer::ID, true) => {
            "Activate (1g): Give another Undead Reborn, then destroy it to gain +8/+8."
        }
        (drone_duplicator::ID, false) => {
            "Divine Shield. Activate (1g): Next Magnetize to this happens an extra time."
        }
        (drone_duplicator::ID, true) => {
            "Divine Shield. Activate (1g): Next Magnetize to this happens 2 extra times."
        }
        (en_djinn_blazer::ID, false) => {
            "Battlecry: After Tavern is Refreshed this game, give a shop minion +10/+10."
        }
        (en_djinn_blazer::ID, true) => {
            "Battlecry: After Tavern is Refreshed this game, give a shop minion +10/+10 twice."
        }
        (enchanted_sentinel::ID, false) => {
            "Magnetic. Your Tavern spells give an extra +1/+1."
        }
        (enchanted_sentinel::ID, true) => {
            "Magnetic. Your Tavern spells give an extra +2/+2."
        }
        (faceless_operative::ID, false) => {
            "On sell: Get 2 random Aberrations; when you play 1, discard the other."
        }
        (faceless_operative::ID, true) => {
            "On sell: Get 4 random Aberrations; when you play 2, discard the rest."
        }
        (flaming_enforcer::ID, false) => {
            "End of Turn: Consume highest-Health shop minion to gain its stats."
        }
        (flaming_enforcer::ID, true) => {
            "End of Turn: Consume highest-Health shop minion to gain double its stats."
        }
        (friendly_geist::ID, false) => {
            "Deathrattle: Your Tavern spells give an extra +1 Attack this game."
        }
        (friendly_geist::ID, true) => {
            "Deathrattle: Your Tavern spells give an extra +2 Attack this game."
        }
        (gearfin::ID, false) => {
            "At the end of your turn, get two 1-Cost Tavern spells."
        }
        (gearfin::ID, true) => {
            "At the end of your turn, get four 1-Cost Tavern spells."
        }
        (geomagus_roogug::ID, false) => {
            "Divine Shield. Whenever a Blood Gem is played on this, play 1 on another minion."
        }
        (geomagus_roogug::ID, true) => {
            "Divine Shield. Whenever a Blood Gem is played on this, play 2 on another minion."
        }
        (glambot::ID, false) => {
            "Whenever you cast a spell on a Mech, Magnetize a 4/4 Satellite to it."
        }
        (glambot::ID, true) => {
            "Whenever you cast a spell on a Mech, Magnetize two 4/4 Satellites to it."
        }
        (gormling_gourmet::ID, false) => {
            "Taunt. Battlecry & Deathrattle: Get a Seafood Stew."
        }
        (gormling_gourmet::ID, true) => {
            "Taunt. Battlecry & Deathrattle: Get 2 Seafood Stews."
        }
        (gunpowder_courier::ID, false) => {
            "Whenever you spend 5 Gold, give your Pirates +3/+1."
        }
        (gunpowder_courier::ID, true) => {
            "Whenever you spend 5 Gold, give your Pirates +3/+1 twice."
        }
        (headhunter_gryphon::ID, false) => "Rally: Get a random Beast.",
        (headhunter_gryphon::ID, true) => "Rally: Get 2 random Beasts.",
        (heroic_underdog::ID, false) => {
            "Stealth. Rally: Gain the target's Attack."
        }
        (heroic_underdog::ID, true) => {
            "Stealth. Rally: Gain double the target's Attack."
        }
        (hoarding_hyena::ID, false) => "Rally: Summon a Tasty Lobster.",
        (hoarding_hyena::ID, true) => "Rally: Summon a Golden Tasty Lobster.",
        (holy_vanguard::ID, false) => {
            "Divine Shield. Has +30/+30 if you have 15 or less Health."
        }
        (holy_vanguard::ID, true) => {
            "Divine Shield. Has +60/+60 if you have 15 or less Health."
        }
        (hot_air_surveyor::ID, false) => {
            "Blood Gems played from your hand cast an extra time."
        }
        (hot_air_surveyor::ID, true) => {
            "Blood Gems played from your hand cast 2 extra times."
        }
        (humongozz::ID, false) => {
            "Divine Shield. Your Tavern spells give an extra +1/+2."
        }
        (humongozz::ID, true) => {
            "Divine Shield. Your Tavern spells give an extra +2/+4."
        }
        (ichoron_the_protector::ID, false) => {
            "Divine Shield. Whenever you play an Elemental, give it Divine Shield until next turn."
        }
        (ichoron_the_protector::ID, true) => {
            "Divine Shield. Whenever you play an Elemental, give it Divine Shield permanently."
        }
        (imp_lusionist::ID, false) => "Deathrattle: Get a Methodical Madness.",
        (imp_lusionist::ID, true) => "Deathrattle: Get 2 Methodical Madnesses.",
        (imposing_percussionist::ID, false) => {
            "Battlecry: Discover a Demon. Deal damage to your hero equal to its Tier."
        }
        (imposing_percussionist::ID, true) => {
            "Battlecry: Discover 2 Demons. Deal damage to your hero equal to their Tiers."
        }
        (kelp_keeper::ID, false) => {
            "Activate (1g): Trigger a friendly minion's Battlecry."
        }
        (kelp_keeper::ID, true) => {
            "Activate (1g): Trigger a friendly minion's Battlecry twice."
        }
        (leyline_surfacer::ID, false) => {
            "Battlecry & Deathrattle: Get an Arcane Absorption."
        }
        (leyline_surfacer::ID, true) => {
            "Battlecry & Deathrattle: Get 2 Arcane Absorptions."
        }
        (living_prison::ID, false) => {
            "Activate (1g): Gain the stats of the next minion you buy this turn."
        }
        (living_prison::ID, true) => {
            "Activate (1g): Gain double the stats of the next minion you buy this turn."
        }
        (lovesick_balladist::ID, false) => {
            "Battlecry: Give a Pirate +2 Health (improved by Gold spent this turn)."
        }
        (lovesick_balladist::ID, true) => {
            "Battlecry: Give a Pirate +2 Health twice (improved by Gold spent this turn)."
        }
        (maritime_extortionist::ID, false) => {
            "Has +7/+7 for each Golden minion you've played this game."
        }
        (maritime_extortionist::ID, true) => {
            "Has +14/+14 for each Golden minion you've played this game."
        }
        (maw_caster::ID, false) => {
            "Battlecry: Destroy a friendly Undead to Discover an Undead."
        }
        (maw_caster::ID, true) => {
            "Battlecry: Destroy a friendly Undead to Discover 2 Undead."
        }
        (mindbending_recruiter::ID, false) => {
            "Activate (0g): Discard a card to get a random Aberration."
        }
        (mindbending_recruiter::ID, true) => {
            "Activate (0g): Discard a card to get 2 random Aberrations."
        }
        (nightmare_corroder::ID, false) => {
            "At the end of your turn, get a Sludge Corrosion."
        }
        (nightmare_corroder::ID, true) => {
            "At the end of your turn, get 2 Sludge Corrosions."
        }
        (parasitic_fleshling::ID, false) => {
            "End of Turn: Give left-most minion +2/+2 (+1/+1 per card discarded this game)."
        }
        (parasitic_fleshling::ID, true) => {
            "End of Turn: Give left-most minion +4/+4 (+2/+2 per card discarded this game)."
        }
        (persistent_poet::ID, false) => {
            "Divine Shield. Adjacent Dragons permanently keep Bonus Keywords and stats gained in combat."
        }
        (persistent_poet::ID, true) => {
            "Divine Shield. Adjacent Dragons permanently keep Bonus Keywords and double stats gained in combat."
        }
        (plaguerunner::ID, false) => {
            "Deathrattle: Undead have +2 Attack this game (+4 outside combat)."
        }
        (plaguerunner::ID, true) => {
            "Deathrattle: Undead have +4 Attack this game (+8 outside combat)."
        }
        (razorfen_flapper::ID, false) => {
            "Battlecry & Deathrattle: Get a Blood Gem Barrage."
        }
        (razorfen_flapper::ID, true) => {
            "Battlecry & Deathrattle: Get 2 Blood Gem Barrages."
        }
        (refreshing_anomaly::ID, false) => "Battlecry: Gain 2 free Refreshes.",
        (refreshing_anomaly::ID, true) => "Battlecry: Gain 4 free Refreshes.",
        (runic_arcanist::ID, false) => "Start of Combat: Cast Shiny Ring twice.",
        (runic_arcanist::ID, true) => "Start of Combat: Cast Shiny Ring 4 times.",
        (sacrificial_wrathguard::ID, false) => {
            "Deathrattle: Give shop minions +2/+2 this game. Activate (1g): Improve this."
        }
        (sacrificial_wrathguard::ID, true) => {
            "Deathrattle: Give shop minions +4/+4 this game. Activate (1g): Improve this."
        }
        (sindorei_straight_shot::ID, _) => {
            "Divine Shield, Windfury. Rally: Remove Reborn and Taunt from the target."
        }
        (sky_hatch_runaway::ID, false) => {
            "Activate (1g): Trigger a friendly minion's Rally."
        }
        (sky_hatch_runaway::ID, true) => {
            "Activate (1g): Trigger a friendly minion's Rally twice."
        }
        (snare_trapper::ID, false) => {
            "Choose One: Get a random Quilboar; or Increase your maximum Gold by 1."
        }
        (snare_trapper::ID, true) => {
            "Choose One: Get 2 random Quilboars; or Increase your maximum Gold by 2."
        }
        (snarky_shark::ID, false) => {
            "On sell: Refresh Tavern with a Fishbait; left-most Beast attacks it."
        }
        (snarky_shark::ID, true) => {
            "On sell: Refresh Tavern with a Golden Fishbait; left-most Beast attacks it."
        }
        (soulkeeping_jailer::ID, false) => {
            "Activate (2g): Your Demons each consume a random shop minion to gain its stats."
        }
        (soulkeeping_jailer::ID, true) => {
            "Activate (2g): Your Demons each consume a random shop minion to gain double its stats."
        }
        (tavern_tempest::ID, false) => "Battlecry: Get a random Elemental.",
        (tavern_tempest::ID, true) => "Battlecry: Get 2 random Elementals.",
        (tortollan_blue_shell::ID, false) => {
            "If you lost your last combat, this sells for 5 Gold."
        }
        (tortollan_blue_shell::ID, true) => {
            "If you lost your last combat, this sells for 10 Gold."
        }
        (twilight_tidehunter::ID, false) => {
            "Whenever you cast a spell on this, give left-most minion in hand +8/+8."
        }
        (twilight_tidehunter::ID, true) => {
            "Whenever you cast a spell on this, give left-most minion in hand +16/+16."
        }
        // Tokens & Spells
        (tokens::TOKEN_ABERRANT_TENTACLE, _) => "Taunt.",
        (tokens::TOKEN_WATER_DROPLET, _) => "Token Elemental.",
        (tokens::TOKEN_DEMON_FODDER, _) => "Feeds itself to a friendly Demon on Refresh.",
        (tokens::TOKEN_FISHBAIT, _) => "Cannot gain stats. Deathrattle: Give killer +5/+5.",
        (tokens::TOKEN_HELPING_HAND, _) => "Reborn.",
        (tokens::TOKEN_SATELLITE, _) => "Token Mech.",
        (tokens::TOKEN_BLUE_CHROMADRAKE, _) => "Battlecry: Get a random 2-Cost Tavern spell.",
        (tokens::TOKEN_BLACK_CHROMADRAKE, _) => {
            "Battlecry: Your Tavern spells give an extra +1 Health this game."
        }
        (tokens::TOKEN_GREEN_CHROMADRAKE, _) => "Battlecry: Give your other Dragons +1/+3.",
        (tokens::TOKEN_BRONZE_CHROMADRAKE, _) => "Battlecry: Give your other Dragons +3/+1.",
        (tokens::TOKEN_RED_CHROMADRAKE, _) => {
            "Battlecry: Your Tavern spells give an extra +1 Attack this game."
        }
        (tokens::SPELL_BLOOD_GEM, _) => "Spell: Give a friendly minion +1/+1 (plus Gem bonuses).",
        (tokens::SPELL_TAVERN_COIN, _) => "Spell: Gain 1 Gold.",
        (tokens::SPELL_LOCKBOX, _) => "Unplayable. Opens in 5 turns for a Golden typed minion.",
        (tokens::SPELL_GEM_DAY, _) => "Choose One: Blood Gems give +1 Attack; or +1 Health.",
        (tokens::SPELL_SLUDGE_CORROSION, _) => {
            "Spell: Give your minions +1/+1. If you discard this, cast it twice."
        }
        (tokens::SPELL_GEM_CONFISCATION, _) => {
            "Spell: Play 3 Blood Gems on a minion and steal all Blood Gems from its neighbors."
        }
        (tokens::SPELL_GOLDEN_TOUCH, _) => "Spell: Make a random minion in the Tavern Golden.",
        (tokens::SPELL_POINTY_ARROW, _) => "Spell (1g): Give a minion +4 Attack.",
        (tokens::SPELL_ARCANE_ABSORPTION, _) => {
            "Spell (0g): Consume a minion in the Tavern to give a Demon its stats."
        }
        (SPELL_A_NEW_SPROUT, _) => "Spell (3g): Discover a Tier 1 minion.",
        (SPELL_ALLIANCE_FLAG, _) => "Spell (1g): Choose One — Give a minion +3/+1 or +1/+3.",
        (SPELL_ENCHANTED_LASSO, _) => "Spell (2g): Steal a random minion from the Tavern.",
        (SPELL_FORTIFY, _) => "Spell (1g): Give a minion +3 Health and Taunt.",
        (SPELL_RECRUIT_A_TRAINEE, _) => "Spell (2g): Get a random Tier 1 minion.",
        (SPELL_TAVERN_DISH_BANANA, _) => "Spell (1g): Give a minion +2/+2.",
        (SPELL_THEM_APPLES, _) => "Spell (1g): Give minions in the Tavern +1/+2.",
        (SPELL_CHEFS_CHOICE, _) => {
            "Spell (2g): Choose a minion. Get a different minion of the same type."
        }
        (SPELL_HASTY_EXCAVATION, _) => "Spell (3 HP): Gain 1 Gold. Costs Health instead of Gold.",
        (SPELL_LEAF_THROUGH_THE_PAGES, _) => "Spell (1g): Gain 2 free Refreshes.",
        (SPELL_MIGHT_OF_STORMWIND, _) => "Spell (2g): Give 4 friendly minions +1/+2.",
        (SPELL_SEARCH_THROUGH_TIME, _) => {
            "Spell (2g): Discover a minion from your Tier (locked 1 turn)."
        }
        (SPELL_STRIKE_OIL, _) => "Spell (3g): Increase your maximum Gold by 1.",
        (SPELL_WINNERS_BREAD, _) => "Spell (2g): Give a minion +2/+3 (and 1 Blood Gem if you win).",
        (SPELL_CAREFUL_INVESTMENT, _) => "Spell (1g): Gain 2 Gold next turn.",
        (SPELL_FRIENDLY_BOUNTY, _) => "Spell (1g): Give your minions +1/+1.",
        (SPELL_HEALTHY_BOUNTY, _) => "Spell (1g): Give a minion +6 Health.",
        (SPELL_HOSTILE_BOUNTY, _) => "Spell (1g): Give a minion +6 Attack.",
        (SPELL_OVERCONFIDENCE, _) => {
            "Spell (1g): If you win next combat, gain 3 Gold (or 1 Gold on tie)."
        }
        (SPELL_PLANAR_TELESCOPE, _) => {
            "Spell (3g): Discover a minion of your most common type."
        }
        (SPELL_REPAIR_JOB, _) => "Spell (2g): Give a minion +4/+8.",
        (SPELL_ROBUST_EVOLUTION, _) => {
            "Spell (2g): Choose One — Set a minion's Attack or Health to highest on board."
        }
        (SPELL_SEAFOOD_STEW, _) => "Spell (2g): Give a minion +1/+1 per friendly minion type.",
        (SPELL_SELFISH_BOUNTY, _) => "Spell (1g): Give a minion +3/+3.",
        (SPELL_SHINY_RING, _) => "Spell (2g): Give your minions +1/+1.",
        (SPELL_STAFF_OF_ENRICHMENT, _) => {
            "Spell (3g): Give minions in the Tavern +1/+1 this game."
        }
        (SPELL_TIME_MANAGEMENT, _) => {
            "Spell (2g): Give your minions +2/+2 (or next turn if you have <= 4 Gold)."
        }
        (SPELL_TRICKY_TROUSERS, _) => {
            "Spell (1g): Give a minion +1/+2 and toggle its Taunt."
        }
        (SPELL_WEALTHY_BOUNTY, _) => "Spell (1g): Get a Tavern Coin.",
        // Tier 4 Spells
        (SPELL_BLOOD_GEM_BARRAGE, _) => {
            "Spell (1g): Give minions in the Tavern +2/+1 this game."
        }
        (SPELL_BOON_OF_BEETLES, _) => {
            "Spell (1g): When you have space in combat, summon a 2/2 Beetle with Taunt (4 times)."
        }
        (SPELL_BOUNDLESS_POTENTIAL, _) => {
            "Spell (3g): Choose One — Discover a minion or Tavern spell from your Tier."
        }
        (SPELL_CLONING_CONCH, _) => {
            "Spell (3g): Get a copy of a Murloc in your hand."
        }
        (SPELL_DEFENDERS_RITES, _) => "Spell (2g): Give a friendly minion +8/+8 and Taunt.",
        (SPELL_EASTERLY_WINDS, _) => {
            "Spell (2g): Give a random minion in the Tavern +5/+5; repeat on future Refreshes."
        }
        (SPELL_EONARS_FAVOR, _) => {
            "Spell (1g): Choose a minion. Minions of its type in the Tavern have +3/+2 this game."
        }
        (SPELL_METHODICAL_MADNESS, _) => {
            "Spell (3g): Friendly Demon consumes a random shop minion; repeat 2 more times."
        }
        (SPELL_MIGHTY_DRAGONBREATH, _) => {
            "Spell (2g): Give a minion +3/+5. If it's a Dragon, also give it Windfury."
        }
        (SPELL_MISPLACED_TEA_SET, _) => {
            "Spell (2g): Give a friendly minion of each type +3/+3."
        }
        (SPELL_NATURAL_BLESSING, _) => {
            "Spell (3g): Choose a minion. Give all minions that share a type with it +3/+2."
        }
        (SPELL_TEMPERATURE_SHIFT, _) => {
            "Spell (2g): Get a Fire Baller and a Snow Baller."
        }
        (SPELL_TOMB_TURNING, _) => {
            "Spell (3g): Discover an Undead. If you play it this turn, it dies."
        }
        (SPELL_WEAPONS_FORGE, _) => {
            "Spell (2g): Give a minion +2/+2 for each Bonus Keyword in your warband."
        }
        _ => "",
    }
}

fn format_keywords(u: &Unit) -> String {
    let mut kws = Vec::new();
    if u.taunt {
        kws.push("Taunt");
    }
    if u.divine_shield {
        kws.push("Divine Shield");
    }
    if u.windfury {
        kws.push("Windfury");
    }
    if u.reborn {
        kws.push("Reborn");
    }
    if u.venomous {
        kws.push("Venomous");
    }
    if u.stealth {
        kws.push("Stealth");
    }
    if u.magnetic {
        kws.push("Magnetic");
    }
    if kws.is_empty() {
        String::new()
    } else {
        format!(" [{}]", kws.join(", "))
    }
}

fn format_tribe(t: Tribe) -> &'static str {
    match t {
        Tribe::None => "Neutral",
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
        Tribe::UndeadMech => "Undead/Mech",
        Tribe::DragonPirate => "Dragon/Pirate",
        Tribe::BeastPirate => "Beast/Pirate",
        Tribe::ElementalDemon => "Elem/Demon",
        Tribe::MechMurloc => "Mech/Murloc",
        Tribe::DemonDragon => "Demon/Dragon",
        Tribe::DemonQuilboar => "Demon/Quilboar",
        Tribe::All => "All",
    }
}

fn format_unit_row(idx: usize, u: &Unit, pool: Option<&CardPool>, is_board: bool) -> String {
    if u.is_spell {
        let desc = card_description(u.card_id, u.is_golden);
        return format!("  [{idx}] (Spell) {:<22} | {desc}", u.name);
    }

    let badge = if u.is_golden { "★ " } else { "  " };
    let name_col = format!("{badge}{}", u.name);
    let stats_col = format!("{}/{}", u.attack, u.health);
    let tribe_col = format!("T{} {}", u.tavern_tier, format_tribe(u.tribe));
    let kw_str = format_keywords(u);

    let mut extras = Vec::new();
    if is_board {
        if let Some(cost) = cards::activate_cost(u.card_id) {
            if u.activated_this_turn {
                extras.push("Activate: USED".to_string());
            } else {
                extras.push(format!("Activate: {cost}g READY"));
            }
        }
        if u.eot_health_bonus > 0 {
            extras.push(format!("+{} HP/turn (Magnetized)", u.eot_health_bonus));
        }
    }
    if let Some(p) = pool {
        if u.card_id >= 100 && u.card_id < 800 {
            let rem = p.remaining_copies(u.card_id);
            let max = base_copies_for_tier(u.tavern_tier);
            extras.push(format!("pool {rem}/{max}"));
        }
    }
    let extra_str = if extras.is_empty() {
        String::new()
    } else {
        format!(" ({})", extras.join(", "))
    };
    let desc = card_description(u.card_id, u.is_golden);

    format!(
        "  [{idx}] {name_col:<24} {:>5}  {tribe_col:<13}{kw_str}{extra_str}\n        └─ {desc}",
        stats_col
    )
}

fn print_tavern(state: &TavernState, pool: &CardPool) {
    let frozen_tag = if state.is_frozen { " [FROZEN ❄]" } else { "" };
    let next_gold_tag = if state.bonus_gold_next_turn > 0 {
        format!(" (+{}g next turn)", state.bonus_gold_next_turn)
    } else {
        String::new()
    };
    let upg_tag = if state.tavern_tier < 6 {
        format!("Upgrade: {}g", state.upgrade_cost)
    } else {
        "MAX TIER".to_string()
    };
    let deity_name = match state.auras.deity.kind {
        DeityKind::CThun => "C'Thun",
        DeityKind::YShaarj => "Y'Shaarj",
        DeityKind::None => "None",
    };

    println!("\n================================================================================");
    println!(
        " TURN {}  |  HP: {}/30  |  Gold: {}/{}{}  |  Tavern Tier {} ({}){}",
        state.turn,
        state.health,
        state.gold,
        state.max_gold,
        next_gold_tag,
        state.tavern_tier,
        upg_tag,
        frozen_tag
    );
    println!(
        " Deity: {} ({}/{})  |  Elem Aura: +{}/+{}  |  Spell Disc: -{}g  |  Spells Cast: {}",
        deity_name,
        state.auras.deity.attack,
        state.auras.deity.health,
        state.auras.tavern_elemental_atk,
        state.auras.tavern_elemental_hp,
        state.auras.next_spell_discount,
        state.auras.spells_played,
    );
    println!("================================================================================");

    if let Some(ref opts) = state.discover_pending {
        println!(" >>> DISCOVER PENDING (choose 1 with `d <index>`): <<<");
        for (i, u) in opts.iter().enumerate() {
            println!("{}", format_unit_row(i, u, Some(pool), false));
        }
        println!("--------------------------------------------------------------------------------");
    }

    println!(" BOB'S SHOP ({} minions){}:", state.shop.len(), frozen_tag);
    if state.shop.is_empty() {
        println!("  (empty)");
    } else {
        for (i, u) in state.shop.iter().enumerate() {
            println!("{}", format_unit_row(i, u, Some(pool), false));
        }
    }

    println!("--------------------------------------------------------------------------------");
    println!(" YOUR BOARD ({}/7):", state.board.len());
    if state.board.is_empty() {
        println!("  (empty)");
    } else {
        for (i, u) in state.board.iter().enumerate() {
            println!("{}", format_unit_row(i, u, None, true));
        }
    }

    println!("--------------------------------------------------------------------------------");
    println!(" YOUR HAND ({}/10):", state.hand.len());
    if state.hand.is_empty() {
        println!("  (empty)");
    } else {
        for (i, u) in state.hand.iter().enumerate() {
            println!("{}", format_unit_row(i, u, None, false));
        }
    }
    println!("================================================================================");
}

fn print_help() {
    println!(
        r#"Commands:
  b <shop_idx>            Buy shop[shop_idx] for 3 Gold
  p <hand_idx> [pos]      Play hand[hand_idx] to board[pos] (default: rightmost slot, or 0 for spells)
                          (Tip: playing a Magnetic Mech at pos < board.len() where board[pos] is a Mech fuses it!)
  s <board_pos>           Sell board[board_pos] for +1 Gold
  m <from> <to>           Reposition board[from] to board[to]
  a <board_pos> [target]  Activate ability on board[board_pos] (with optional target board slot)
  r                       Refresh Bob's shop (1 Gold)
  f                       Toggle Freeze on Bob's shop (0 Gold)
  u                       Upgrade Tavern Tier
  d <opt_idx>             Choose Discover option opt_idx
  e / n                   End Turn (runs End-of-Turn triggers) and start next Tavern turn

Sandbox / Debug Helpers:
  give <name|id>          Give yourself a Tier 1 minion into hand (triggers Triples!)
  gold <amount>           Set current Gold (e.g. `gold 10`)
  deity <cthun|yshaarj>   Switch your active Old God Deity
  pool                    Show remaining copies in the shared CardPool
  combat                  Preview your Start-of-Combat board (including Flighty Scout from hand)
  legal                   List all currently legal TavernActions
  reset [seed]            Restart from Turn 1 with optional seed
  h / help                Show this help
  q / quit                Exit"#
    );
}

fn find_template<'a>(templates: &'a [CardTemplate], query: &str) -> Option<&'a CardTemplate> {
    if let Ok(id) = query.parse::<u32>() {
        return templates.iter().find(|t| t.card_id == id);
    }
    let q = query.to_ascii_lowercase();
    templates
        .iter()
        .find(|t| t.name.to_ascii_lowercase() == q)
        .or_else(|| {
            templates
                .iter()
                .find(|t| t.name.to_ascii_lowercase().contains(&q))
        })
}

fn execute_command(
    line: &str,
    state: &mut TavernState,
    pool: &mut CardPool,
    rng: &mut Rng,
    templates: &[CardTemplate],
) -> bool {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return true;
    }
    let cmd = parts[0].to_ascii_lowercase();
    let args = &parts[1..];

    match cmd.as_str() {
        "q" | "quit" | "exit" => return false,
        "h" | "help" => {
            print_help();
            return true;
        }
        "state" | "show" => {
            print_tavern(state, pool);
            return true;
        }
        "legal" => {
            let actions = state.valid_actions();
            println!("Legal actions ({}):", actions.len());
            for (i, act) in actions.iter().enumerate() {
                println!("  ({i}) {act:?}");
            }
            return true;
        }
        "pool" => {
            println!("\nShared CardPool Remaining Copies:");
            for t in templates {
                let rem = pool.remaining_copies(t.card_id);
                let max = base_copies_for_tier(t.tavern_tier);
                println!(
                    "  ID {:>3} | {:<22} | {:>2}/{} copies | {}/{} T{} {}",
                    t.card_id,
                    t.name,
                    rem,
                    max,
                    t.attack,
                    t.health,
                    t.tavern_tier,
                    format_tribe(t.tribe)
                );
            }
            return true;
        }
        "combat" => {
            let cb = state.combat_board();
            println!("\nStart-of-Combat Board Snapshot ({}/7):", cb.len());
            for (i, u) in cb.iter().enumerate() {
                println!("{}", format_unit_row(i, u, None, true));
            }
            return true;
        }
        "gold" => {
            if let Some(Ok(g)) = args.first().map(|s| s.parse::<u32>()) {
                state.gold = g;
                println!("-> Set gold = {g}");
                print_tavern(state, pool);
            } else {
                println!("Usage: gold <amount>");
            }
            return true;
        }
        "deity" => {
            match args.first().map(|s| s.to_ascii_lowercase()).as_deref() {
                Some("cthun") | Some("c'thun") => {
                    state.auras.deity.kind = DeityKind::CThun;
                    println!("-> Set Deity to C'Thun");
                    print_tavern(state, pool);
                }
                Some("yshaarj") | Some("y'shaarj") => {
                    state.auras.deity.kind = DeityKind::YShaarj;
                    println!("-> Set Deity to Y'Shaarj");
                    print_tavern(state, pool);
                }
                _ => println!("Usage: deity <cthun|yshaarj>"),
            }
            return true;
        }
        "give" => {
            let query = args.join(" ");
            if query.is_empty() {
                println!("Usage: give <card name or id>");
                return true;
            }
            if let Some(tpl) = find_template(templates, &query) {
                let unit = tpl.instantiate();
                let cid = unit.card_id;
                println!("-> Added {} to hand", tpl.name);
                state.hand.push(unit);
                state.check_and_resolve_triple(cid);
                print_tavern(state, pool);
            } else {
                println!("Unknown card {query:?} (try `pool` to see all cards)");
            }
            return true;
        }
        "reset" => {
            let seed = args
                .first()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(42);
            *pool = CardPool::new(templates.to_vec());
            *rng = Rng::new(seed);
            *state = TavernState::new();
            state.start_turn(pool, rng);
            println!("-> Reset Tavern game with seed {seed}");
            print_tavern(state, pool);
            return true;
        }
        _ => {}
    }

    match parse_action_cmd(&cmd, args, state) {
        Ok(action) => match state.step(action, pool, rng) {
            Ok(ended_turn) => {
                println!("-> Executed {action:?}");
                if ended_turn {
                    println!(
                        "-> End-of-Turn triggers resolved. Advancing to Turn {}...",
                        state.turn + 1
                    );
                    state.start_turn(pool, rng);
                }
                print_tavern(state, pool);
            }
            Err(e) => {
                println!("!! Illegal action: {e}");
            }
        },
        Err(msg) => println!("!! {msg}"),
    }
    true
}

fn parse_action_cmd(
    cmd: &str,
    args: &[&str],
    state: &TavernState,
) -> Result<TavernAction, String> {
    match cmd {
        "b" | "buy" => args
            .first()
            .ok_or_else(|| "Usage: buy <shop_idx>".to_string())
            .and_then(|s| s.parse::<usize>().map_err(|_| "Invalid index".to_string()))
            .map(|shop_index| TavernAction::Buy { shop_index }),
        "p" | "play" => {
            if args.is_empty() {
                return Err("Usage: play <hand_idx> [board_pos]".to_string());
            }
            let hand_index = args[0]
                .parse::<usize>()
                .map_err(|_| "Invalid hand_idx".to_string())?;
            let default_pos = match state.hand.get(hand_index) {
                Some(c) if c.is_spell => 0,
                _ => state.board.len(),
            };
            let board_pos = match args.get(1) {
                Some(s) => s
                    .parse::<usize>()
                    .map_err(|_| "Invalid board_pos".to_string())?,
                None => default_pos,
            };
            Ok(TavernAction::Play {
                hand_index,
                board_pos,
            })
        }
        "s" | "sell" => args
            .first()
            .ok_or_else(|| "Usage: sell <board_pos>".to_string())
            .and_then(|s| {
                s.parse::<usize>()
                    .map_err(|_| "Invalid board_pos".to_string())
            })
            .map(|board_pos| TavernAction::Sell { board_pos }),
        "m" | "move" | "reposition" => {
            if args.len() < 2 {
                return Err("Usage: move <from_pos> <to_pos>".to_string());
            }
            let from_pos = args[0]
                .parse::<usize>()
                .map_err(|_| "Invalid from_pos".to_string())?;
            let to_pos = args[1]
                .parse::<usize>()
                .map_err(|_| "Invalid to_pos".to_string())?;
            Ok(TavernAction::Reposition { from_pos, to_pos })
        }
        "a" | "act" | "activate" => {
            if args.is_empty() {
                return Err("Usage: act <board_pos> [target_pos]".to_string());
            }
            let board_pos = args[0]
                .parse::<usize>()
                .map_err(|_| "Invalid board_pos".to_string())?;
            let target_pos = match args.get(1) {
                Some(s) => Some(
                    s.parse::<usize>()
                        .map_err(|_| "Invalid target_pos".to_string())?,
                ),
                None => None,
            };
            Ok(TavernAction::Activate {
                board_pos,
                target_pos,
            })
        }
        "r" | "refresh" | "roll" => Ok(TavernAction::Refresh),
        "f" | "freeze" => Ok(TavernAction::ToggleFreeze),
        "u" | "upgrade" | "level" => Ok(TavernAction::UpgradeTavern),
        "d" | "discover" => args
            .first()
            .ok_or_else(|| "Usage: discover <option_idx>".to_string())
            .and_then(|s| {
                s.parse::<usize>()
                    .map_err(|_| "Invalid option_idx".to_string())
            })
            .map(|option_index| TavernAction::ChooseDiscover { option_index }),
        "e" | "end" | "n" | "next" => Ok(TavernAction::EndTurn),
        other => Err(format!("Unknown command {other:?} (type `help` for commands)")),
    }
}

fn run_demo(state: &mut TavernState, pool: &mut CardPool, rng: &mut Rng, templates: &[CardTemplate]) {
    println!("=== SEAGLASS TAVERN PHASE DEMO ===");
    print_tavern(state, pool);

    let script = [
        ("b 0", "Turn 1: Buy the leftmost minion from Bob's Shop (costs 3g)"),
        ("p 0", "Turn 1: Play it from hand onto the board"),
        ("e", "End Turn 1 -> Advance to Turn 2 (4g, upgrade cost decays 5g -> 4g)"),
        ("b 0", "Turn 2: Buy a minion (costs 3g, 1g left)"),
        ("p 0", "Turn 2: Play it onto the board"),
        ("r", "Turn 2: Spend remaining 1g to Refresh Bob's shop"),
        ("f", "Turn 2: Freeze the shop for Turn 3"),
        ("e", "End Turn 2 -> Advance to Turn 3 (5g, shop stays frozen)"),
        ("give Cord Puller", "Sandbox check: Add Cord Puller (Mech) to hand"),
        ("p 0", "Play Cord Puller to board"),
        ("give Lullabot", "Sandbox check: Add Lullabot (Magnetic Mech) to hand"),
        ("p 0 2", "Magnetize Lullabot onto Cord Puller at board[2]!"),
        ("e", "End Turn 3 -> Watch Magnetized Cord Puller gain +1 Health at End of Turn!"),
    ];

    for (cmd, note) in script {
        println!("\n>>> {note}");
        println!(">>> $ {cmd}");
        execute_command(cmd, state, pool, rng, templates);
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let is_demo = args.iter().any(|a| a == "--demo");
    let seed = args
        .iter()
        .find_map(|a| a.parse::<u64>().ok())
        .unwrap_or(42);

    let templates = full_catalog();
    let mut pool = CardPool::new(templates.clone());
    let mut rng = Rng::new(seed);
    let mut state = TavernState::new();
    state.start_turn(&mut pool, &mut rng);

    if is_demo {
        run_demo(&mut state, &mut pool, &mut rng, &templates);
        return;
    }

    println!("Seaglass Tavern Phase CLI (seed: {seed})");
    println!("Type `help` for commands, or `q` to quit.");
    print_tavern(&state, &pool);

    let stdin = io::stdin();
    loop {
        print!("\ntavern> ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        match stdin.read_line(&mut line) {
            Ok(0) => {
                println!();
                break;
            }
            Ok(_) => {
                if !execute_command(line.trim(), &mut state, &mut pool, &mut rng, &templates) {
                    break;
                }
            }
            Err(e) => {
                eprintln!("Read error: {e}");
                break;
            }
        }
    }
}
