//! Exhaustive per-card functional unit tests for all 34 Solo Tier 2 minions (Patch 36.6.3).

use seaglass::cards::{tier1, tier2, tokens};
use seaglass::{
    full_catalog, simulate, BattleOutcome, CardPool, DeityKind, GameState, Rng, TavernAction,
    TavernState, Tribe, Unit,
};

fn setup_tavern(seed: u64) -> (TavernState, CardPool, Rng) {
    let pool = CardPool::new(full_catalog());
    let rng = Rng::new(seed);
    let mut state = TavernState::new().with_shop_spells(true);
    state.tavern_tier = 2;
    state.gold = 20;
    (state, pool, rng)
}

#[test]
fn card_201_bilgewater_breakout() {
    let (mut state, mut pool, mut rng) = setup_tavern(201);
    state.add_to_hand(tier2::bilgewater_breakout::template().instantiate());
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 0,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, tokens::SPELL_LOCKBOX);
    assert_eq!(state.hand[0].lockbox_turns_left, 5);

    // Playing a second Bilgewater Breakout accelerates the Lockbox by 1 turn (5 -> 4).
    state.add_to_hand(tier2::bilgewater_breakout::template().instantiate());
    state
        .step(
            TavernAction::Play {
                hand_index: 1,
                board_pos: 1,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.hand[0].lockbox_turns_left, 4);
}

#[test]
fn card_202_blue_volumizer() {
    let (mut state, mut pool, mut rng) = setup_tavern(202);
    state.board.push(tier1::cord_puller::template().instantiate()); // 1/1 Mech
    state.add_to_hand(tier2::blue_volumizer::template().instantiate()); // 1/3 Magnetic (+3 HP to Volumizers)
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 0,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(tier2::blue_volumizer::volumizer_bonus(&state.auras).1, 3);
    // Blue Volumizer became 1/6 before fusing onto 1/1 Cord Puller -> 2/7!
    assert_eq!(state.board[0].attack, 2);
    assert_eq!(state.board[0].health, 7);
}

#[test]
fn card_203_brain_rotter() {
    let (mut state, mut pool, mut rng) = setup_tavern(203);
    state.auras.deity.kind = DeityKind::CThun;
    state.board.push(tier2::brain_rotter::template().instantiate());
    state.add_to_hand(tokens::make_blood_gem());
    state
        .step(
            TavernAction::Activate {
                board_pos: 0,
                target_pos: Some(0),
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert!(state.hand.is_empty());
    assert_eq!(state.auras.deity.attack, 3);
    assert_eq!(state.auras.deity.health, 3);
}

#[test]
fn card_204_bronze_warden() {
    let warden = tier2::bronze_warden::template().instantiate();
    assert!(warden.divine_shield);
    assert!(warden.reborn);
    let board_a = vec![warden]; // 2/1 DS Reborn (Reborn copy also has DS!)
    let board_b = vec![Unit::new("E1", 2, 4), Unit::new("E2", 2, 2)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 204);
    assert_eq!(res.outcome, BattleOutcome::AWin);
}

#[test]
fn card_205_clever_castaway() {
    let (mut state, mut pool, mut rng) = setup_tavern(205);
    state
        .board
        .push(tier2::clever_castaway::template().instantiate());
    let pre_gold = state.gold;
    state
        .step(
            TavernAction::Activate {
                board_pos: 0,
                target_pos: None,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.gold, pre_gold - 2);
    assert!(state.discover_pending.is_some());
    assert!(state.discover_pending.as_ref().unwrap()[0].is_spell);
    state
        .step(
            TavernAction::ChooseDiscover { option_index: 0 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.hand.len(), 1);
    assert!(state.hand[0].is_spell);
}

#[test]
fn card_206_crater_miner() {
    let (mut state, mut pool, mut rng) = setup_tavern(206);
    state.add_to_hand(tier2::crater_miner::template().instantiate());
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 0,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert!(state.discover_pending.is_some());
    // Option 0: Get 2 Blood Gems
    state
        .step(
            TavernAction::ChooseDiscover { option_index: 0 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.hand.len(), 2);
    assert!(state
        .hand
        .iter()
        .all(|c| c.card_id == tokens::SPELL_BLOOD_GEM));
}

#[test]
fn card_207_decoy_conjurer() {
    let (mut state, mut pool, mut rng) = setup_tavern(207);
    state.shop.push(Unit::new("Small", 1, 1));
    state.shop.push(Unit::new("Big", 9, 4));
    state
        .board
        .push(tier2::decoy_conjurer::template().instantiate());
    state
        .step(
            TavernAction::Activate {
                board_pos: 0,
                target_pos: None,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].name, "Big");
    assert_eq!(state.shop.len(), 1);
}

#[test]
fn card_208_electric_synthesizer() {
    let (mut state, mut pool, mut rng) = setup_tavern(208);
    state
        .board
        .push(tier1::glim_guardian::template().instantiate()); // 1/4 Dragon
    state.add_to_hand(tier2::electric_synthesizer::template().instantiate());
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 1,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    // Battlecry buffed Glim Guardian from 1/4 -> 2/5
    assert_eq!(state.board[0].attack, 2);
    assert_eq!(state.board[0].health, 5);
}

#[test]
fn card_209_eternal_knight() {
    let mut gs = GameState::default();
    gs.auras_a.add_counter(tier2::eternal_knight::ID, 2);
    let board_a = vec![tier2::eternal_knight::template().instantiate()]; // 4/2 + 2*(4/2) = 12/6
    let board_b = vec![Unit::new("Enemy", 10, 10)];
    let res = simulate(&board_a, &board_b, &gs, 209);
    assert_eq!(res.outcome, BattleOutcome::Draw);
    assert_eq!(res.auras_a.counter(tier2::eternal_knight::ID), 3);
}

#[test]
fn card_210_expert_aviator() {
    let mut gs = GameState::default();
    gs.hand_a
        .push(Unit::new("HandMurloc", 8, 8).with_tribe(Tribe::Murloc));
    let board_a = vec![tier2::expert_aviator::template().instantiate()]; // 3/5 Rally: summon highest-Atk Murloc from hand
    let board_b = vec![Unit::new("Enemy", 4, 6)];
    let res = simulate(&board_a, &board_b, &gs, 210);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert!(res.survivors_a.iter().any(|u| u.name == "HandMurloc"));
}

#[test]
fn card_211_fire_baller() {
    let (mut state, mut pool, mut rng) = setup_tavern(211);
    state.board.push(Unit::new("Ally", 2, 2));
    state.board.push(tier2::fire_baller::template().instantiate());
    state
        .step(TavernAction::Sell { board_pos: 1 }, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.board[0].attack, 3);
    assert_eq!(tier2::fire_baller::baller_bonus(&state.auras), 1);
}

#[test]
fn card_212_forest_rover() {
    let (mut state, mut pool, mut rng) = setup_tavern(212);
    state.add_to_hand(tier2::forest_rover::template().instantiate());
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 0,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(tokens::beetle_bonus(&state.auras).0, 2);
    assert_eq!(tokens::beetle_bonus(&state.auras).1, 1);

    let opp = vec![Unit::new("Enemy", 3, 1)];
    let res = state.resolve_combat_against(&opp, 1, &Default::default(), &[], 212);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    // Summoned 2/2 + 2/1 = 4/3 Beetle!
    assert_eq!(res.survivors_a[0].attack, 4);
    assert_eq!(res.survivors_a[0].health, 3);
}

#[test]
fn card_213_green_volumizer() {
    let (mut state, mut pool, mut rng) = setup_tavern(213);
    state.add_to_hand(tier2::green_volumizer::template().instantiate());
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 0,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(tier2::blue_volumizer::volumizer_bonus(&state.auras).0, 1);
    assert_eq!(tier2::blue_volumizer::volumizer_bonus(&state.auras).1, 1);
    assert_eq!(state.board[0].attack, 4);
    assert_eq!(state.board[0].health, 4);
}

#[test]
fn card_214_humming_bird() {
    let board_a = vec![
        tier2::humming_bird::template().instantiate(), // 1/4 Beast (+1 Atk to Beasts at SoC)
        tier1::flittering_bat::template().instantiate(), // 1/4 Beast -> 2/4, summons 1/1 -> 2/1 Beast
    ];
    let board_b = vec![Unit::new("Enemy", 2, 3)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 214);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert!(res.survivors_a.iter().all(|u| u.attack >= 2));
}

#[test]
fn card_215_intrepid_botanist() {
    let (mut state, mut pool, mut rng) = setup_tavern(215);
    state.add_to_hand(tier2::intrepid_botanist::template().instantiate());
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 0,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    // Option 0: +1 Attack to Tavern spells
    state
        .step(
            TavernAction::ChooseDiscover { option_index: 0 },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.auras.spell_bonus_atk, 1);
}

#[test]
fn card_216_laboratory_assistant() {
    let (mut state, mut pool, mut rng) = setup_tavern(216);
    state
        .board
        .push(tier1::ominous_seer::template().instantiate()); // 2/1 Demon
    state.add_to_hand(tier2::laboratory_assistant::template().instantiate()); // 3/4 Demon
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 1,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.auras.fodder_per_refresh, [1, 1, 1]);
    state
        .step(TavernAction::Refresh, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.auras.fodder_per_refresh, [1, 1, 0]);
}

#[test]
fn card_217_lurking_lionfish() {
    let (mut state, mut pool, mut rng) = setup_tavern(217);
    state.shop.push(Unit::new("ShopCard", 2, 2));
    state
        .board
        .push(tier2::lurking_lionfish::template().instantiate()); // 3/4 Beast
    state
        .step(
            TavernAction::Activate {
                board_pos: 0,
                target_pos: Some(0),
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.board[0].attack, 8);
    assert_eq!(state.board[0].health, 9);
}

#[test]
fn card_218_mechagnome_interpreter() {
    let (mut state, mut pool, mut rng) = setup_tavern(218);
    state
        .board
        .push(tier2::mechagnome_interpreter::template().instantiate());
    state.add_to_hand(tier1::cord_puller::template().instantiate()); // 1/1 Mech
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 1,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.board[1].attack, 4);
    assert_eq!(state.board[1].health, 2);
}

#[test]
fn card_219_mind_muck() {
    let (mut state, mut pool, mut rng) = setup_tavern(219);
    state.shop.push(Unit::new("Food", 4, 5));
    state
        .board
        .push(tier1::ominous_seer::template().instantiate()); // 2/1 Demon
    state.add_to_hand(tier2::mind_muck::template().instantiate());
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 1,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.board[0].attack, 6);
    assert_eq!(state.board[0].health, 6);
    assert!(state.shop.is_empty());
}

#[test]
fn card_220_nerubian_deathswarmer() {
    let (mut state, mut pool, mut rng) = setup_tavern(220);
    state
        .board
        .push(tier1::risen_rider::template().instantiate()); // 2/1 Undead
    state.add_to_hand(tier2::nerubian_deathswarmer::template().instantiate()); // 1/4 Undead
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 1,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.auras.undead_bonus_attack, 1);
    assert_eq!(state.board[0].attack, 3);
    assert_eq!(state.board[1].attack, 2);
}

#[test]
fn card_221_patient_scout() {
    let (mut state, mut pool, mut rng) = setup_tavern(221);
    state.add_to_hand(tier2::patient_scout::template().instantiate());
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 0,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(tier2::patient_scout::discover_tier(&state.board[0]), 1);
    state.start_turn(&mut pool, &mut rng);
    assert_eq!(tier2::patient_scout::discover_tier(&state.board[0]), 2);
    state
        .step(TavernAction::Sell { board_pos: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert!(state.discover_pending.is_some());
    assert!(state
        .discover_pending
        .as_ref()
        .unwrap()
        .iter()
        .all(|u| u.tavern_tier == 2));
}

#[test]
fn card_222_prodigious_tusker() {
    let board_a = vec![
        Unit::new("Attacker", 2, 3),
        tier2::prodigious_tusker::template().instantiate(), // 2/5: plays a Blood Gem on another attacking friendly minion
    ];
    let board_b = vec![Unit::new("Enemy", 3, 3)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 222);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    // Attacker (2/3) gained +1/+1 (to 3/4) before striking 3/3 Enemy, so Attacker survived at 3/1!
    assert_eq!(res.survivors_a.len(), 2);
    assert_eq!(res.survivors_a[0].attack, 3);
    assert_eq!(res.survivors_a[0].health, 1);
}

#[test]
fn card_223_red_volumizer() {
    let (mut state, mut pool, mut rng) = setup_tavern(223);
    state.add_to_hand(tier2::red_volumizer::template().instantiate());
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 0,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(tier2::blue_volumizer::volumizer_bonus(&state.auras).0, 3);
    assert_eq!(state.board[0].attack, 6);
    assert_eq!(state.board[0].health, 1);
}

#[test]
fn card_224_roadboar() {
    let board_a = vec![tier2::roadboar::template().instantiate()]; // 2/4 Rally: Get a Blood Gem
    let board_b = vec![Unit::new("Enemy", 0, 1)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 224);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.hand_a.len(), 1);
    assert_eq!(res.hand_a[0].card_id, tokens::SPELL_BLOOD_GEM);
}

#[test]
fn card_225_scarlet_skull() {
    let board_a = vec![
        tier2::scarlet_skull::template().instantiate(), // 2/1 Reborn DR: Give a friendly Undead +1/+2
        tier1::risen_rider::template().instantiate(),   // 2/1 Undead
    ];
    let board_b = vec![Unit::new("Enemy", 2, 2)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 225);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    // Risen Rider was buffed from 2/1 -> 3/3 by Scarlet Skull's Deathrattle!
    assert!(res.survivors_a.iter().any(|u| u.attack == 3 && u.health == 3));
}

#[test]
fn card_226_sellemental() {
    let (mut state, mut pool, mut rng) = setup_tavern(226);
    state.board.push(tier2::sellemental::template().instantiate());
    state
        .step(TavernAction::Sell { board_pos: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, tokens::TOKEN_WATER_DROPLET);
    assert_eq!(state.hand[0].attack, 3);
    assert_eq!(state.hand[0].health, 3);
}

#[test]
fn card_227_snow_baller() {
    let (mut state, mut pool, mut rng) = setup_tavern(227);
    state.board.push(Unit::new("Ally", 2, 2));
    state.board.push(tier2::snow_baller::template().instantiate());
    state
        .step(TavernAction::Sell { board_pos: 1 }, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.board[0].health, 3);
    assert_eq!(tier2::fire_baller::baller_bonus(&state.auras), 1);
}

#[test]
fn card_228_soul_rewinder() {
    let (mut state, _pool, _rng) = setup_tavern(228);
    state.board.push(tier2::soul_rewinder::template().instantiate()); // 4/2
    let pre_hp = state.health;
    state.deal_hero_damage(2);
    assert_eq!(state.health, pre_hp);
    assert_eq!(state.board[0].health, 4);
}

#[test]
fn card_229_surfing_sylvar() {
    let (mut state, mut pool, mut rng) = setup_tavern(229);
    state.board.push(Unit::new("Left", 2, 2).with_golden(true));
    state.board.push(tier2::surfing_sylvar::template().instantiate());
    state
        .board
        .push(Unit::new("Right", 2, 2).with_golden(true));
    state
        .step(TavernAction::EndTurn, &mut pool, &mut rng)
        .unwrap();
    // Base + 2 repeats for 2 Golden minions = 3 triggers of +1 Attack = +3 Attack!
    assert_eq!(state.board[0].attack, 5);
    assert_eq!(state.board[2].attack, 5);
}

#[test]
fn card_230_tad() {
    let (mut state, mut pool, mut rng) = setup_tavern(230);
    state.board.push(tier2::tad::template().instantiate());
    state
        .step(TavernAction::Sell { board_pos: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.hand.len(), 1);
    assert!(state.hand[0].tribe.matches(Tribe::Murloc));
    assert_ne!(state.hand[0].card_id, tier2::tad::ID);
}

#[test]
fn card_231_tarecgosa() {
    let (mut state, _pool, _rng) = setup_tavern(231);
    state.board.push(tier2::tarecgosa::template().instantiate()); // 4/4 Dragon
    state
        .board
        .push(tier2::electric_synthesizer::template().instantiate()); // +1/+1 at SoC
    let opp = vec![Unit::new("Dummy", 1, 1)];
    state.resolve_combat_against(&opp, 1, &Default::default(), &[], 231);
    assert_eq!(state.board[0].attack, 5);
    assert_eq!(state.board[0].health, 5);
}

#[test]
fn card_232_underrot_spawn() {
    let board_a = vec![
        tier2::underrot_spawn::template().instantiate(),
        Unit::new("Ally", 2, 5),
    ];
    let board_b = vec![Unit::new("Enemy", 3, 2)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 232);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    // Summoned 0/2 Tentacle got +1 Attack -> 1/2 Taunt, and Ally got +1 Attack -> 3/5!
    assert_eq!(res.survivors_a[0].attack, 1);
    assert_eq!(res.survivors_a[0].health, 2);
    assert!(res.survivors_a[0].taunt);
    assert_eq!(res.survivors_a[1].attack, 3);
}

#[test]
fn card_233_very_hungry_winterfinner() {
    let mut gs = GameState::default();
    gs.hand_a.push(Unit::new("HandMinion", 2, 2));
    let board_a = vec![tier2::very_hungry_winterfinner::template().instantiate()]; // 2/6 Taunt
    let board_b = vec![Unit::new("Enemy", 1, 2)];
    let res = simulate(&board_a, &board_b, &gs, 233);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.hand_a[0].attack, 4);
    assert_eq!(res.hand_a[0].health, 3);
}

#[test]
fn card_234_wandering_willbreaker() {
    let (mut state, mut pool, mut rng) = setup_tavern(234);
    state
        .board
        .push(tier2::wandering_willbreaker::template().instantiate());
    state
        .step(TavernAction::Sell { board_pos: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.hand.len(), 2);
    assert!(state.hand[0].willbreaker_group > 0);
    state.board.push(Unit::new("Target", 2, 2));
    state
        .step(
            TavernAction::Play {
                hand_index: 0,
                board_pos: 0,
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    if state.discover_pending.is_some() {
        state
            .step(
                TavernAction::ChooseDiscover { option_index: 0 },
                &mut pool,
                &mut rng,
            )
            .unwrap();
    }
    assert!(!state.hand.iter().any(|c| c.willbreaker_group > 0));
}
