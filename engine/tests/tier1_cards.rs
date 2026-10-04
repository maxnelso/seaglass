//! Exhaustive per-card functional unit tests for all 21 Solo Tier 1 minions (Patch 36.6.3).

use seaglass::cards::{minions, spells, tokens};
use seaglass::{
    full_catalog, simulate, BattleOutcome, CardPool, DeityKind, GameState, Keyword, Rng,
    TavernAction, TavernState, Tribe, Unit,
};

fn setup_tavern(seed: u64) -> (TavernState, CardPool, Rng) {
    let pool = CardPool::new(full_catalog());
    let rng = Rng::new(seed);
    let mut state = TavernState::new().with_shop_spells(true);
    state.gold = 20;
    (state, pool, rng)
}

#[test]
fn card_101_joyous() {
    let (mut state, mut pool, mut rng) = setup_tavern(101);
    state.auras.deity.kind = DeityKind::CThun;
    state.add_to_hand(minions::joyous::template().instantiate());
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
    assert_eq!(state.auras.deity.attack, 3);
    assert_eq!(state.auras.deity.health, 2);

    // Golden gives +4/+2
    state.add_to_hand(minions::joyous::template().instantiate().with_golden(true));
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
    assert_eq!(state.auras.deity.attack, 7);
    assert_eq!(state.auras.deity.health, 4);
}

#[test]
fn card_102_zoatroid() {
    let (mut state, mut pool, mut rng) = setup_tavern(102);
    state.board.push(minions::zoatroid::template().instantiate());
    state
        .step(TavernAction::Sell { board_pos: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.hand.len(), 1);
    assert_eq!(state.hand[0].card_id, tokens::TOKEN_ABERRANT_TENTACLE);
    assert!(state.hand[0].taunt);
    assert_eq!(state.hand[0].attack, 0);
    assert_eq!(state.hand[0].health, 2);
}

#[test]
fn card_103_buzzing_vermin() {
    let mut gs = GameState::default();
    tokens::add_beetle_bonus(&mut gs.auras_a, 2, 1);
    let board_a = vec![minions::buzzing_vermin::template().instantiate()];
    let board_b = vec![Unit::new("Attacker", 2, 1)];
    let res = simulate(&board_a, &board_b, &gs, 103);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.survivors_a.len(), 1);
    // 2/2 base Beetle + 2/1 aura = 4/3 Beetle
    assert_eq!(res.survivors_a[0].attack, 4);
    assert_eq!(res.survivors_a[0].health, 3);
}

#[test]
fn card_104_flittering_bat() {
    let board_a = vec![minions::flittering_bat::template().instantiate()];
    let board_b = vec![Unit::new("Target", 1, 2)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 104);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    // Flittering Bat (1/4) attacked and summoned a 1/1 Bat; Bat then finished off Target!
    assert!(!res.survivors_a.is_empty());
}

#[test]
fn card_105_wrath_weaver() {
    let (mut state, mut pool, mut rng) = setup_tavern(105);
    state.board.push(minions::wrath_weaver::template().instantiate());
    let pre_hp = state.health;
    state.add_to_hand(minions::ominous_seer::template().instantiate()); // Demon
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
    assert_eq!(state.health, pre_hp - 1);
    assert_eq!(state.board[0].attack, 3);
    assert_eq!(state.board[0].health, 5);
}

#[test]
fn card_106_glim_guardian() {
    let board_a = vec![minions::glim_guardian::template().instantiate()]; // 1/4 Rally: +2 Attack
    let board_b = vec![Unit::new("Dummy", 1, 3)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 106);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    // Glim Guardian gained +2 Attack on Rally (1 -> 3) and one-shot the 1/3 Dummy!
    assert_eq!(res.survivors_a[0].attack, 3);
    assert_eq!(res.survivors_a[0].health, 3);
}

#[test]
fn card_107_scarlet_survivor() {
    let (mut state, mut pool, mut rng) = setup_tavern(107);
    state.board.push(minions::scarlet_survivor::template().instantiate()); // 3/3
    assert!(!state.board[0].divine_shield);
    state.add_to_hand(spells::spell_by_name("Tavern Dish Banana").unwrap()); // +2/+2 -> 5/5
    state.add_to_hand(spells::spell_by_name("Tavern Dish Banana").unwrap()); // +2/+2 -> 7/7 (>= 6 Atk!)
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
    assert!(!state.board[0].divine_shield);
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
    assert!(state.board[0].divine_shield);
}

#[test]
fn card_108_crackling_cyclone() {
    let cyclone = minions::crackling_cyclone::template().instantiate();
    assert!(cyclone.divine_shield);
    assert!(cyclone.windfury);
    let board_a = vec![cyclone];
    let board_b = vec![Unit::new("E1", 2, 2), Unit::new("E2", 2, 2)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 108);
    assert_eq!(res.outcome, BattleOutcome::Draw);
}

#[test]
fn card_109_dune_dweller() {
    let (mut state, mut pool, mut rng) = setup_tavern(109);
    state
        .shop
        .push(minions::crackling_cyclone::template().instantiate()); // 2/1 Elemental in shop
    state.add_to_hand(minions::dune_dweller::template().instantiate());
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
    assert_eq!(state.auras.tavern_elemental_atk, 1);
    assert_eq!(state.auras.tavern_elemental_hp, 1);
    assert_eq!(state.shop[0].attack, 3);
    assert_eq!(state.shop[0].health, 2);
}

#[test]
fn card_110_cord_puller() {
    let board_a = vec![minions::cord_puller::template().instantiate()]; // 1/1 DS Deathrattle: 1/1 Microbot
    let board_b = vec![Unit::new("E1", 2, 2)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 110);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.survivors_a.len(), 1);
}

#[test]
fn card_111_lullabot() {
    let (mut state, mut pool, mut rng) = setup_tavern(111);
    state.board.push(minions::cord_puller::template().instantiate()); // 1/1 Mech
    state.add_to_hand(minions::lullabot::template().instantiate()); // 2/2 Magnetic (+1 HP at EOT)
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
    assert_eq!(state.board[0].attack, 3);
    assert_eq!(state.board[0].health, 3);
    state
        .step(TavernAction::EndTurn, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.board[0].health, 4);
}

#[test]
fn card_112_bubble_gunner() {
    let (mut state, mut pool, mut rng) = setup_tavern(112);
    state.add_to_hand(minions::bubble_gunner::template().instantiate());
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
    let u = &state.board[0];
    let has_any = [
        Keyword::Taunt,
        Keyword::DivineShield,
        Keyword::Windfury,
        Keyword::Reborn,
        Keyword::Venomous,
        Keyword::Stealth,
    ]
    .iter()
    .any(|&kw| u.has_keyword(kw));
    assert!(has_any);
}

#[test]
fn card_113_flighty_scout() {
    let mut gs = GameState::default();
    gs.hand_a.push(minions::flighty_scout::template().instantiate()); // 3/3 in hand
    let board_a = vec![Unit::new("OnBoard", 1, 1)];
    let board_b = vec![Unit::new("Enemy", 2, 3)];
    let res = simulate(&board_a, &board_b, &gs, 113);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert!(res.survivors_a.iter().any(|u| u.name == "Flighty Scout"));
}

#[test]
fn card_114_aureate_laureate() {
    let (mut state, mut pool, mut rng) = setup_tavern(114);
    for _ in 0..3 {
        state.add_to_hand(minions::aureate_laureate::template().instantiate());
    }
    // Intrinsic Golden minions never combine into a Triple Reward!
    assert_eq!(state.hand.len(), 3);
    assert!(state.hand.iter().all(|u| u.is_golden && u.intrinsic_golden));
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
    assert!(state.discover_pending.is_none());
}

#[test]
fn card_115_southsea_busker() {
    let (mut state, mut pool, mut rng) = setup_tavern(115);
    state.add_to_hand(minions::southsea_busker::template().instantiate());
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
    assert_eq!(state.bonus_gold_next_turn, 1);
}

#[test]
fn card_116_razorfen_geomancer() {
    let (mut state, mut pool, mut rng) = setup_tavern(116);
    state.add_to_hand(minions::razorfen_geomancer::template().instantiate());
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
    assert_eq!(state.hand.len(), 2);
    assert!(state
        .hand
        .iter()
        .all(|c| c.card_id == tokens::SPELL_BLOOD_GEM));
}

#[test]
fn card_117_tusked_camper() {
    let mut gs = GameState::default();
    gs.auras_a.blood_gem_bonus_atk = 1;
    gs.auras_a.blood_gem_bonus_hp = 1;
    let board_a = vec![minions::tusked_camper::template().instantiate()]; // 2/3 Rally: Plays a Blood Gem (+2/+2 -> 4/5)
    let board_b = vec![Unit::new("Enemy", 3, 4)];
    let res = simulate(&board_a, &board_b, &gs, 117);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.survivors_a[0].attack, 4);
    assert_eq!(res.survivors_a[0].health, 2);
}

#[test]
fn card_118_harmless_bonehead() {
    let board_a = vec![minions::harmless_bonehead::template().instantiate()]; // 1/1 DR: two 1/1 Skeletons
    let board_b = vec![Unit::new("Enemy", 1, 2)];
    let res = simulate(&board_a, &board_b, &GameState::default(), 118);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.survivors_a.len(), 1);
    assert_eq!(res.survivors_a[0].tribe, Tribe::Undead);
}

#[test]
fn card_119_risen_rider() {
    let mut rider = minions::risen_rider::template().instantiate();
    assert!(rider.taunt);
    assert!(rider.reborn);
    // Buff Risen Rider to 7/2 externally; on Reborn it must return as a base 2/1 (+ any global Undead Attack aura), not 7/1.
    rider.add_stats(5, 1);
    let mut gs = GameState::default();
    gs.auras_a.undead_bonus_attack = 1; // +1 Undead Attack everywhere
    seaglass::cards::sync_unit_auras(&mut rider, &gs.auras_a);
    assert_eq!(rider.attack, 8);
    let board_a = vec![rider];
    let board_b = vec![Unit::new("Enemy", 5, 8)];
    let res = simulate(&board_a, &board_b, &gs, 119);
    assert_eq!(res.outcome, BattleOutcome::AWin);
    assert_eq!(res.survivors_a.len(), 1);
    assert!(!res.survivors_a[0].reborn);
    assert_eq!(res.survivors_a[0].attack, 3); // 2 base + 1 undead_bonus_attack (external +5 stripped!)
    assert_eq!(res.survivors_a[0].health, 1);
}

#[test]
fn card_120_ominous_seer() {
    let (mut state, mut pool, mut rng) = setup_tavern(120);
    state.add_to_hand(minions::ominous_seer::template().instantiate());
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
    assert_eq!(state.auras.next_spell_discount, 1);

    // Put a 2-cost spell in shop and buy it -> costs 1 Gold instead of 2, and consumes discount!
    state.shop.clear();
    state
        .shop
        .push(spells::spell_by_name("Recruit a Trainee").unwrap()); // 2-cost
    let pre_gold = state.gold;
    state
        .step(TavernAction::Buy { shop_index: 0 }, &mut pool, &mut rng)
        .unwrap();
    assert_eq!(state.gold, pre_gold - 1);
    assert_eq!(state.auras.next_spell_discount, 0);
}

#[test]
fn card_121_suspicious_prisonguard() {
    let (mut state, mut pool, mut rng) = setup_tavern(121);
    state.board.push(Unit::new("Target", 2, 2));
    state
        .board
        .push(minions::suspicious_prisonguard::template().instantiate());
    let pre_gold = state.gold;
    state
        .step(
            TavernAction::Activate {
                board_pos: 1,
                target_pos: Some(0),
            },
            &mut pool,
            &mut rng,
        )
        .unwrap();
    assert_eq!(state.gold, pre_gold - 1);
    assert_eq!(state.board[0].attack, 5);
    assert_eq!(state.board[0].health, 5);
}
