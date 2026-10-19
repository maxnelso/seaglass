//! The semi-random heuristic bot `sim_match` plays with.

use seaglass::cards::{minions, spells, tokens};
use seaglass::{Rng, TavernAction, TavernState, Tribe, Unit};

const MAX_BOARD_SIZE: usize = 7;

/// Choose a preferred board slot when playing `card` from hand onto `board`.
fn choose_play_position(state: &TavernState, card: &Unit, rng: &mut Rng) -> usize {
    if card.is_spell {
        if !spells::spell_requires_board_target(card.card_id) || state.board.is_empty() {
            return 0;
        }
        // Match spell target preferences (e.g. Undead for Butchering, Demon for Corrupted Cupcakes, Tier <= 4 plain for Eyes of the Earth Mother).
        let matching: Vec<usize> = match card.card_id {
            spells::SPELL_BUTCHERING => state
                .board
                .iter()
                .enumerate()
                .filter(|(_, u)| u.tribe.matches(Tribe::Undead))
                .map(|(i, _)| i)
                .collect(),
            spells::SPELL_CORRUPTED_CUPCAKES => state
                .board
                .iter()
                .enumerate()
                .filter(|(_, u)| u.tribe.matches(Tribe::Demon))
                .map(|(i, _)| i)
                .collect(),
            spells::SPELL_EYES_OF_THE_EARTH_MOTHER => state
                .board
                .iter()
                .enumerate()
                .filter(|(_, u)| !u.is_golden && u.tavern_tier <= 4)
                .map(|(i, _)| i)
                .collect(),
            tokens::SPELL_ARCANE_ABSORPTION => state
                .board
                .iter()
                .enumerate()
                .filter(|(_, u)| u.tribe.matches(Tribe::Elemental))
                .map(|(i, _)| i)
                .collect(),
            _ => Vec::new(),
        };
        if !matching.is_empty() {
            return matching[rng.below(matching.len())];
        }
        return rng.below(state.board.len());
    }

    // If Magnetic and a compatible minion is on the board, fuse 75% of the time (or 100% if board is full).
    if card.magnetic {
        let targets: Vec<usize> = state
            .board
            .iter()
            .enumerate()
            .filter(|(_, u)| u.tribe.matches(card.tribe))
            .map(|(i, _)| i)
            .collect();
        if !targets.is_empty() && (state.board.len() >= MAX_BOARD_SIZE || rng.below(4) < 3) {
            return targets[rng.below(targets.len())];
        }
    }

    // Positional / targeted Battlecries:
    match card.card_id {
        minions::disguised_graverobber::ID | minions::maw_caster::ID => {
            if let Some(pos) = state
                .board
                .iter()
                .position(|u| u.tribe.matches(Tribe::Undead))
            {
                return pos;
            }
        }
        minions::sprightly_scarab::ID => {
            if let Some(pos) = state
                .board
                .iter()
                .position(|u| u.tribe.matches(Tribe::Beast))
            {
                return pos;
            }
        }
        minions::captain_sanders::ID => {
            if let Some(pos) = state
                .board
                .iter()
                .position(|u| !u.is_golden && u.tavern_tier <= 6)
            {
                return pos;
            }
        }
        minions::elite_navigator::ID => {
            if let Some(pos) = state
                .board
                .iter()
                .position(|u| u.tribe.matches(Tribe::Pirate) && !u.is_golden && u.tavern_tier <= 4)
            {
                return pos;
            }
        }
        _ => {}
    }

    state.board.len()
}

/// Semi-random heuristic policy for a player's Tavern Phase.
pub fn choose_semi_random_action(state: &TavernState, rng: &mut Rng) -> TavernAction {
    // 1. Always resolve pending Discover / Choose-One immediately.
    if let Some(ref opts) = state.discover_pending {
        let pick = if opts.len() <= 1 {
            0
        } else {
            rng.below(opts.len())
        };
        return TavernAction::ChooseDiscover { option_index: pick };
    }

    // 2. Play playable spells from hand if legal.
    for (h_idx, card) in state.hand.iter().enumerate() {
        if !card.is_spell || card.unplayable || card.lockbox_turns_left > 0 || card.locked_turns > 0
        {
            continue;
        }
        // Skip Butchering if no Undead on board, or Corrupted Cupcakes if no Demon on board.
        if card.card_id == spells::SPELL_BUTCHERING
            && !state.board.iter().any(|u| u.tribe.matches(Tribe::Undead))
        {
            continue;
        }
        if card.card_id == spells::SPELL_CORRUPTED_CUPCAKES
            && !state.board.iter().any(|u| u.tribe.matches(Tribe::Demon))
        {
            continue;
        }
        let pos = choose_play_position(state, card, rng);
        let act = TavernAction::Play {
            hand_index: h_idx,
            board_pos: pos,
        };
        if state.is_legal(&act) {
            return act;
        }
    }

    // 3. Play minions from hand (or Magnetize onto board).
    for (h_idx, card) in state.hand.iter().enumerate() {
        if card.is_spell || card.unplayable || card.lockbox_turns_left > 0 || card.locked_turns > 0
        {
            continue;
        }
        let pos = choose_play_position(state, card, rng);
        let act = TavernAction::Play {
            hand_index: h_idx,
            board_pos: pos,
        };
        if state.is_legal(&act) {
            return act;
        }
    }

    // 4. If board is full (7 minions) and hand has a minion that is significantly stronger or higher-tier
    //    than our weakest non-golden board minion, sell the weakest board minion to make room!
    if state.board.len() >= MAX_BOARD_SIZE {
        let best_hand_minion = state
            .hand
            .iter()
            .filter(|c| {
                !c.is_spell && !c.unplayable && c.lockbox_turns_left == 0 && c.locked_turns == 0
            })
            .max_by_key(|c| (c.is_golden, c.tavern_tier, c.attack + c.health));

        if let Some(hand_m) = best_hand_minion {
            let weakest_board = state
                .board
                .iter()
                .enumerate()
                .filter(|(_, b)| !b.is_golden)
                .min_by_key(|(_, b)| (b.attack + b.health, b.tavern_tier));

            if let Some((b_pos, weak_b)) = weakest_board {
                let hand_score = hand_m.attack
                    + hand_m.health
                    + (hand_m.tavern_tier as i32) * 3
                    + if hand_m.is_golden { 10 } else { 0 };
                let board_score = weak_b.attack + weak_b.health + (weak_b.tavern_tier as i32) * 2;
                if hand_score > board_score {
                    let sell_act = TavernAction::Sell { board_pos: b_pos };
                    if state.is_legal(&sell_act) {
                        return sell_act;
                    }
                }
            }
        }
    }

    // 5. Consider ready Activate abilities on board.
    let legal = state.valid_actions();
    let activate_actions: Vec<TavernAction> = legal
        .iter()
        .copied()
        .filter(|a| match *a {
            TavernAction::Activate {
                board_pos,
                target_pos: Some(t),
            } => {
                if state.board[board_pos].card_id == minions::sky_hatch_runaway::ID {
                    !matches!(
                        state.board[t].card_id,
                        minions::heroic_underdog::ID
                            | minions::sindorei_straight_shot::ID
                            | minions::obsidian_ravager::ID
                    )
                } else {
                    true
                }
            }
            TavernAction::Activate { .. } => true,
            _ => false,
        })
        .collect();
    if !activate_actions.is_empty() && rng.below(10) < 7 {
        let pick = rng.below(activate_actions.len());
        return activate_actions[pick];
    }

    // 6. Consider upgrading Tavern Tier if affordable.
    let can_upgrade = state.is_legal(&TavernAction::UpgradeTavern);
    if can_upgrade {
        let should_upgrade = state.upgrade_cost <= 4
            || (state.board.len() >= 4 && rng.below(10) < 6)
            || rng.below(10) < 3;
        if should_upgrade {
            return TavernAction::UpgradeTavern;
        }
    }

    // 7. Buy minions or spells from the shop if affordable.
    let buy_actions: Vec<TavernAction> = legal
        .iter()
        .copied()
        .filter(|a| match *a {
            TavernAction::Buy { shop_index } => {
                if let Some(c) = state.shop.get(shop_index) {
                    if !c.is_spell && state.board.len() >= MAX_BOARD_SIZE && state.hand.len() >= 3 {
                        // Avoid hoarding unplayable minions in hand when board is already full, unless it pairs/triples.
                        state
                            .board
                            .iter()
                            .chain(state.hand.iter())
                            .any(|u| !u.is_golden && u.card_id == c.card_id)
                    } else {
                        true
                    }
                } else {
                    false
                }
            }
            _ => false,
        })
        .collect();

    if !buy_actions.is_empty() {
        // Prioritize buying a minion that matches a card_id already on board or hand (for triples!).
        let pair_buys: Vec<TavernAction> = buy_actions
            .iter()
            .copied()
            .filter(|a| {
                if let TavernAction::Buy { shop_index } = *a {
                    let cid = state.shop[shop_index].card_id;
                    !state.shop[shop_index].is_spell
                        && state
                            .board
                            .iter()
                            .chain(state.hand.iter())
                            .any(|u| !u.is_golden && u.card_id == cid)
                } else {
                    false
                }
            })
            .collect();
        if !pair_buys.is_empty() {
            return pair_buys[rng.below(pair_buys.len())];
        }
        return buy_actions[rng.below(buy_actions.len())];
    }

    // 8. If we have Patient Scout / Tad / Greedy Conniver (Golden) / Ballers on board and want gold or value, sell occasionally.
    if state.gold < 3 {
        if let Some(pos) = state.board.iter().position(|u| {
            matches!(
                u.card_id,
                minions::sellemental::ID
                    | minions::tad::ID
                    | minions::fire_baller::ID
                    | minions::snow_baller::ID
                    | minions::air_baller::ID
                    | minions::snarky_shark::ID
            ) || (u.card_id == minions::patient_scout::ID
                && minions::patient_scout::discover_tier(u) >= 3)
                || (u.card_id == minions::greedy_conniver::ID && u.is_golden)
        }) {
            let sell_act = TavernAction::Sell { board_pos: pos };
            if state.is_legal(&sell_act) {
                return sell_act;
            }
        }
    }

    // 9. Refresh if we have free refreshes, or if we have >= 4 gold and nothing to buy.
    if state.is_legal(&TavernAction::Refresh)
        && (state.auras.free_refreshes > 0
            || state.gold >= 4
            || (state.gold >= 1 && state.shop.is_empty()))
    {
        return TavernAction::UpgradeTavern
            .into_option(can_upgrade && rng.below(2) == 0)
            .unwrap_or(TavernAction::Refresh);
    }

    TavernAction::EndTurn
}

trait IntoOption: Sized {
    fn into_option(self, cond: bool) -> Option<Self> {
        if cond {
            Some(self)
        } else {
            None
        }
    }
}
impl IntoOption for TavernAction {}
