//! Head-to-head 2-player Battlegrounds game simulator and full-trace logger.
//!
//! Runs a complete game between two semi-random players sharing a `CardPool` and deterministic `Rng`,
//! printing every Tavern action, state transition, aura update, and Combat event.
//!
//! Usage:
//!   cargo run --bin sim_match
//!   cargo run --bin sim_match -- 123
//!   cargo run --bin sim_match -- --seed 7 --max-turns 20

use std::collections::HashMap;
use std::env;

use seaglass::cards::{self, spells, tier2, tier3, tier4, tier7, tokens};
use seaglass::{
    base_copies_for_tier, full_catalog, BattleOutcome, BattleResult, CardPool, DeityKind, Event,
    PlayerAuras, Rng, Side, TavernAction, TavernState, Tribe, Unit, UnitId,
};

const MAX_BOARD_SIZE: usize = 7;

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

fn format_deity_kind(k: DeityKind) -> &'static str {
    match k {
        DeityKind::None => "None",
        DeityKind::CThun => "C'Thun",
        DeityKind::YShaarj => "Y'Shaarj",
    }
}

fn format_keywords(u: &Unit) -> String {
    let mut kws = Vec::new();
    if u.taunt {
        kws.push("Taunt");
    }
    if u.divine_shield {
        kws.push("DivineShield");
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

fn format_unit_inline(u: &Unit) -> String {
    if u.is_spell {
        let mut extras = Vec::new();
        if u.spell_cost > 0 || u.tavern_tier > 0 {
            let currency = if u.costs_health { "HP" } else { "g" };
            extras.push(format!("T{} {}{currency}", u.tavern_tier, u.spell_cost));
        }
        if u.lockbox_turns_left > 0 {
            extras.push(format!("opens in {}t", u.lockbox_turns_left));
        }
        if u.locked_turns > 0 {
            extras.push(format!("locked {}t", u.locked_turns));
        }
        if u.fandral_combined {
            extras.push("FandralCombined".to_string());
        }
        let extra_str = if extras.is_empty() {
            String::new()
        } else {
            format!(" ({})", extras.join(", "))
        };
        return format!("(Spell) \"{}\"{}", u.name, extra_str);
    }

    let star = if u.is_golden { "★ " } else { "" };
    let kw = format_keywords(u);
    let mut notes = Vec::new();
    if let Some(cost) = cards::activate_cost(u.card_id) {
        if u.activated_this_turn {
            notes.push("Act:USED".to_string());
        } else {
            notes.push(format!("Act:{cost}g"));
        }
    }
    if u.magnetizations_count > 0 {
        notes.push(format!("Mag:{}", u.magnetizations_count));
    }
    if u.blood_gems_played > 0 {
        notes.push(format!("Gems:{}", u.blood_gems_played));
    }
    if u.eot_health_bonus > 0 {
        notes.push(format!("+{}HP/eot", u.eot_health_bonus));
    }
    if u.sot_gold_bonus > 0 {
        notes.push(format!("+{}g/sot", u.sot_gold_bonus));
    }
    if u.locked_turns > 0 {
        notes.push(format!("locked:{}t", u.locked_turns));
    }
    if u.dies_on_play_this_turn {
        notes.push("DiesOnPlay".to_string());
    }
    let note_str = if notes.is_empty() {
        String::new()
    } else {
        format!(" {{{}}}", notes.join(", "))
    };
    format!(
        "\"{star}{}\" {}/{} (T{} {}, id={}){kw}{note_str}",
        u.name,
        u.attack,
        u.health,
        u.tavern_tier,
        format_tribe(u.tribe),
        u.card_id
    )
}

fn format_unit_list(units: &[Unit], indent: &str, pool: Option<&CardPool>) -> String {
    if units.is_empty() {
        return format!("{indent}(empty)\n");
    }
    let mut out = String::new();
    for (i, u) in units.iter().enumerate() {
        let pool_tag = if let Some(p) = pool {
            if !u.is_spell && u.card_id >= 100 && u.card_id < 800 {
                format!(
                    " [pool {}/{}]",
                    p.remaining_copies(u.card_id),
                    base_copies_for_tier(u.tavern_tier)
                )
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        out.push_str(&format!(
            "{indent}[{i}] {}{pool_tag}\n",
            format_unit_inline(u)
        ));
    }
    out
}

fn print_player_snapshot(label: &str, state: &TavernState, pool: &CardPool) {
    let frozen = if state.is_frozen { " [FROZEN]" } else { "" };
    let upg = if state.tavern_tier < 6 {
        format!("upg={}g", state.upgrade_cost)
    } else {
        "MAX_TIER".to_string()
    };
    println!(
        "  ┌── {label} | HP: {} (+{} Armor) | Tier {} ({upg}) | Gold: {}/{} (+{} next){frozen}",
        state.health,
        state.armor,
        state.tavern_tier,
        state.gold,
        state.max_gold,
        state.bonus_gold_next_turn
    );
    println!(
        "  │   Deity: {} ({}/{}) | Spells Cast: {} | Spell Buff: {:+}/{:+} | Blood Gem: +{}/+{} | Undead +{} ATK (EK died: {})",
        format_deity_kind(state.auras.deity.kind),
        state.auras.deity.attack,
        state.auras.deity.health,
        state.auras.spells_played,
        state.auras.spell_bonus_atk,
        state.auras.spell_bonus_hp,
        1 + state.auras.blood_gem_bonus_atk,
        1 + state.auras.blood_gem_bonus_hp,
        state.auras.undead_bonus_attack,
        state.auras.eternal_knights_died,
    );
    println!("  │   Board ({}/7):", state.board.len());
    print!("{}", format_unit_list(&state.board, "  │     ", None));
    println!("  │   Hand ({}/10):", state.hand.len());
    print!("{}", format_unit_list(&state.hand, "  │     ", None));
    println!("  │   Shop ({}):", state.shop.len());
    print!("{}", format_unit_list(&state.shop, "  │     ", Some(pool)));
    if let Some(ref opts) = state.discover_pending {
        println!("  │   Discover Pending ({} options):", opts.len());
        print!("{}", format_unit_list(opts, "  │     ", Some(pool)));
    }
    println!("  └──────────────────────────────────────────────────────────────────────────");
}

fn describe_action(state: &TavernState, action: &TavernAction) -> String {
    match *action {
        TavernAction::Buy { shop_index } => {
            if let Some(c) = state.shop.get(shop_index) {
                let cost_str = if c.is_spell {
                    if c.costs_health {
                        format!("{} HP", c.spell_cost)
                    } else {
                        format!("{}g", state.effective_spell_buy_cost(c))
                    }
                } else {
                    "3g".to_string()
                };
                format!(
                    "Buy shop[{shop_index}] ({}) for {cost_str}",
                    format_unit_inline(c)
                )
            } else {
                format!("Buy shop[{shop_index}]")
            }
        }
        TavernAction::Play {
            hand_index,
            board_pos,
        } => {
            if let Some(c) = state.hand.get(hand_index) {
                if c.is_spell {
                    if spells::spell_requires_board_target(c.card_id) {
                        let target_desc = state
                            .board
                            .get(board_pos)
                            .map(format_unit_inline)
                            .unwrap_or_else(|| format!("board[{board_pos}]"));
                        format!(
                            "Cast Targeted Spell hand[{hand_index}] ({}) -> board[{board_pos}] ({target_desc})",
                            format_unit_inline(c)
                        )
                    } else {
                        format!(
                            "Cast Untargeted Spell hand[{hand_index}] ({})",
                            format_unit_inline(c)
                        )
                    }
                } else if c.magnetic
                    && board_pos < state.board.len()
                    && state.board[board_pos].tribe.matches(c.tribe)
                {
                    format!(
                        "Magnetize hand[{hand_index}] ({}) onto board[{board_pos}] ({})",
                        format_unit_inline(c),
                        format_unit_inline(&state.board[board_pos])
                    )
                } else {
                    let target_note = if board_pos < state.board.len() {
                        format!(
                            " (at/before board[{board_pos}] {})",
                            format_unit_inline(&state.board[board_pos])
                        )
                    } else {
                        String::new()
                    };
                    format!(
                        "Play Minion hand[{hand_index}] ({}) -> slot {board_pos}{target_note}",
                        format_unit_inline(c)
                    )
                }
            } else {
                format!("Play hand[{hand_index}] -> board[{board_pos}]")
            }
        }
        TavernAction::Sell { board_pos } => {
            if let Some(u) = state.board.get(board_pos) {
                format!(
                    "Sell board[{board_pos}] ({}) for +1g",
                    format_unit_inline(u)
                )
            } else {
                format!("Sell board[{board_pos}]")
            }
        }
        TavernAction::Reposition { from_pos, to_pos } => {
            let name = state
                .board
                .get(from_pos)
                .map(|u| u.name.as_str())
                .unwrap_or("?");
            format!("Reposition board[{from_pos}] (\"{name}\") -> board[{to_pos}]")
        }
        TavernAction::Activate {
            board_pos,
            target_pos,
        } => {
            let src = state
                .board
                .get(board_pos)
                .map(format_unit_inline)
                .unwrap_or_else(|| format!("board[{board_pos}]"));
            let cost = state
                .board
                .get(board_pos)
                .and_then(|u| cards::activate_cost(u.card_id))
                .unwrap_or(0);
            let target_str = match target_pos {
                None => String::new(),
                Some(t) => match state
                    .board
                    .get(board_pos)
                    .map(|u| cards::activate_target_kind(u.card_id))
                {
                    Some(seaglass::ActivateTargetKind::HandCard) => {
                        let h = state
                            .hand
                            .get(t)
                            .map(format_unit_inline)
                            .unwrap_or_else(|| format!("hand[{t}]"));
                        format!(" targeting hand[{t}] ({h})")
                    }
                    Some(seaglass::ActivateTargetKind::ShopCard) => {
                        let s = state
                            .shop
                            .get(t)
                            .map(format_unit_inline)
                            .unwrap_or_else(|| format!("shop[{t}]"));
                        format!(" targeting shop[{t}] ({s})")
                    }
                    _ => {
                        let b = state
                            .board
                            .get(t)
                            .map(format_unit_inline)
                            .unwrap_or_else(|| format!("board[{t}]"));
                        format!(" targeting board[{t}] ({b})")
                    }
                },
            };
            format!("Activate ({cost}g) board[{board_pos}] ({src}){target_str}")
        }
        TavernAction::Refresh => {
            let how = if state.auras.free_refreshes > 0 {
                "using Free Refresh"
            } else if state.has_health_refresh() {
                "paying 1 Health (Malchezaar)"
            } else {
                "paying 1g"
            };
            format!("Refresh Shop ({how})")
        }
        TavernAction::UpgradeTavern => format!(
            "Upgrade Tavern Tier (T{} -> T{}, cost {}g)",
            state.tavern_tier,
            state.tavern_tier + 1,
            state.upgrade_cost
        ),
        TavernAction::ToggleFreeze => {
            let next = if state.is_frozen {
                "UNFROZEN"
            } else {
                "FROZEN"
            };
            format!("Toggle Freeze (-> {next})")
        }
        TavernAction::ChooseDiscover { option_index } => {
            if let Some(ref opts) = state.discover_pending {
                let chosen = opts
                    .get(option_index)
                    .map(format_unit_inline)
                    .unwrap_or_else(|| format!("option[{option_index}]"));
                let all_names: Vec<String> = opts.iter().map(|o| format!("\"{}\"", o.name)).collect();
                format!(
                    "ChooseDiscover[{option_index}]: {chosen} (from [{}])",
                    all_names.join(", ")
                )
            } else {
                format!("ChooseDiscover[{option_index}]")
            }
        }
        TavernAction::EndTurn => "End Turn".to_string(),
    }
}

fn diff_auras(before: &PlayerAuras, after: &PlayerAuras) -> Vec<String> {
    let mut diffs = Vec::new();
    if (before.deity.kind, before.deity.attack, before.deity.health)
        != (after.deity.kind, after.deity.attack, after.deity.health)
    {
        diffs.push(format!(
            "Deity: {} {}/{} -> {} {}/{}",
            format_deity_kind(before.deity.kind),
            before.deity.attack,
            before.deity.health,
            format_deity_kind(after.deity.kind),
            after.deity.attack,
            after.deity.health
        ));
    }
    if (before.tavern_elemental_atk, before.tavern_elemental_hp)
        != (after.tavern_elemental_atk, after.tavern_elemental_hp)
    {
        diffs.push(format!(
            "tavern_elemental: +{}/+{} -> +{}/+{}",
            before.tavern_elemental_atk,
            before.tavern_elemental_hp,
            after.tavern_elemental_atk,
            after.tavern_elemental_hp
        ));
    }
    if (before.tavern_all_atk, before.tavern_all_hp) != (after.tavern_all_atk, after.tavern_all_hp)
    {
        diffs.push(format!(
            "tavern_all: +{}/+{} -> +{}/+{}",
            before.tavern_all_atk,
            before.tavern_all_hp,
            after.tavern_all_atk,
            after.tavern_all_hp
        ));
    }
    if before.undead_bonus_attack != after.undead_bonus_attack {
        diffs.push(format!(
            "undead_bonus_attack: +{} -> +{}",
            before.undead_bonus_attack, after.undead_bonus_attack
        ));
    }
    if before.eternal_knights_died != after.eternal_knights_died {
        diffs.push(format!(
            "eternal_knights_died: {} -> {}",
            before.eternal_knights_died, after.eternal_knights_died
        ));
    }
    if (before.beetle_bonus_atk, before.beetle_bonus_hp)
        != (after.beetle_bonus_atk, after.beetle_bonus_hp)
    {
        diffs.push(format!(
            "beetle_bonus: +{}/+{} -> +{}/+{}",
            before.beetle_bonus_atk,
            before.beetle_bonus_hp,
            after.beetle_bonus_atk,
            after.beetle_bonus_hp
        ));
    }
    if (before.blood_gem_bonus_atk, before.blood_gem_bonus_hp)
        != (after.blood_gem_bonus_atk, after.blood_gem_bonus_hp)
    {
        diffs.push(format!(
            "blood_gem_bonus: +{}/+{} -> +{}/+{}",
            before.blood_gem_bonus_atk,
            before.blood_gem_bonus_hp,
            after.blood_gem_bonus_atk,
            after.blood_gem_bonus_hp
        ));
    }
    if (before.spell_bonus_atk, before.spell_bonus_hp)
        != (after.spell_bonus_atk, after.spell_bonus_hp)
    {
        diffs.push(format!(
            "spell_bonus: +{}/+{} -> +{}/+{}",
            before.spell_bonus_atk,
            before.spell_bonus_hp,
            after.spell_bonus_atk,
            after.spell_bonus_hp
        ));
    }
    if (before.volumizer_bonus_atk, before.volumizer_bonus_hp)
        != (after.volumizer_bonus_atk, after.volumizer_bonus_hp)
    {
        diffs.push(format!(
            "volumizer_bonus: +{}/+{} -> +{}/+{}",
            before.volumizer_bonus_atk,
            before.volumizer_bonus_hp,
            after.volumizer_bonus_atk,
            after.volumizer_bonus_hp
        ));
    }
    if before.next_spell_discount != after.next_spell_discount {
        diffs.push(format!(
            "next_spell_discount: {} -> {}",
            before.next_spell_discount, after.next_spell_discount
        ));
    }
    if before.free_refreshes != after.free_refreshes {
        diffs.push(format!(
            "free_refreshes: {} -> {}",
            before.free_refreshes, after.free_refreshes
        ));
    }
    if before.spells_played != after.spells_played {
        diffs.push(format!(
            "spells_played: {} -> {}",
            before.spells_played, after.spells_played
        ));
    }
    if before.cards_discarded != after.cards_discarded {
        diffs.push(format!(
            "cards_discarded: {} -> {}",
            before.cards_discarded, after.cards_discarded
        ));
    }
    if before.deathrattles_triggered != after.deathrattles_triggered {
        diffs.push(format!(
            "deathrattles_triggered: {} -> {}",
            before.deathrattles_triggered, after.deathrattles_triggered
        ));
    }
    if before.golden_minions_played != after.golden_minions_played {
        diffs.push(format!(
            "golden_minions_played: {} -> {}",
            before.golden_minions_played, after.golden_minions_played
        ));
    }
    if before.boon_of_beetles_charges != after.boon_of_beetles_charges {
        diffs.push(format!(
            "boon_of_beetles_charges: {} -> {}",
            before.boon_of_beetles_charges, after.boon_of_beetles_charges
        ));
    }
    if before.overconfidence_stacks != after.overconfidence_stacks {
        diffs.push(format!(
            "overconfidence_stacks: {} -> {}",
            before.overconfidence_stacks, after.overconfidence_stacks
        ));
    }
    if before.time_management_next_turn != after.time_management_next_turn {
        diffs.push(format!(
            "time_management_next_turn: {} -> {}",
            before.time_management_next_turn, after.time_management_next_turn
        ));
    }
    if before.upper_hand_stacks != after.upper_hand_stacks {
        diffs.push(format!(
            "upper_hand_stacks: {} -> {}",
            before.upper_hand_stacks, after.upper_hand_stacks
        ));
    }
    if before.brood_of_nozdormu_stacks != after.brood_of_nozdormu_stacks {
        diffs.push(format!(
            "brood_of_nozdormu_stacks: {} -> {}",
            before.brood_of_nozdormu_stacks, after.brood_of_nozdormu_stacks
        ));
    }
    if before.sharing_is_caring_stacks != after.sharing_is_caring_stacks {
        diffs.push(format!(
            "sharing_is_caring_stacks: {} -> {}",
            before.sharing_is_caring_stacks, after.sharing_is_caring_stacks
        ));
    }
    if before.waveling_stacks != after.waveling_stacks {
        diffs.push(format!(
            "waveling_stacks: {} -> {}",
            before.waveling_stacks, after.waveling_stacks
        ));
    }
    if before.blood_gem_barrage_stacks != after.blood_gem_barrage_stacks {
        diffs.push(format!(
            "blood_gem_barrage_stacks: {} -> {}",
            before.blood_gem_barrage_stacks, after.blood_gem_barrage_stacks
        ));
    }
    if before.fodder_per_refresh != after.fodder_per_refresh {
        diffs.push(format!(
            "fodder_per_refresh: {:?} -> {:?}",
            before.fodder_per_refresh, after.fodder_per_refresh
        ));
    }
    if before.baller_bonus != after.baller_bonus {
        diffs.push(format!(
            "baller_bonus: +{} -> +{}",
            before.baller_bonus, after.baller_bonus
        ));
    }
    if before.tasty_lobster_stacks != after.tasty_lobster_stacks {
        diffs.push(format!(
            "tasty_lobster_stacks: +{} -> +{}",
            before.tasty_lobster_stacks, after.tasty_lobster_stacks
        ));
    }
    diffs
}

fn print_step_diff(before: &TavernState, after: &TavernState, pool: &CardPool) {
    let mut econ = Vec::new();
    if before.gold != after.gold || before.max_gold != after.max_gold {
        econ.push(format!(
            "Gold: {}/{} -> {}/{}",
            before.gold, before.max_gold, after.gold, after.max_gold
        ));
    }
    if before.bonus_gold_next_turn != after.bonus_gold_next_turn {
        econ.push(format!(
            "BonusGoldNextTurn: {} -> {}",
            before.bonus_gold_next_turn, after.bonus_gold_next_turn
        ));
    }
    if before.health != after.health || before.armor != after.armor {
        econ.push(format!(
            "HP/Armor: {} (+{}) -> {} (+{})",
            before.health, before.armor, after.health, after.armor
        ));
    }
    if before.tavern_tier != after.tavern_tier || before.upgrade_cost != after.upgrade_cost {
        econ.push(format!(
            "Tier/Upg: T{} ({}g) -> T{} ({}g)",
            before.tavern_tier, before.upgrade_cost, after.tavern_tier, after.upgrade_cost
        ));
    }
    if before.is_frozen != after.is_frozen {
        econ.push(format!("Frozen: {} -> {}", before.is_frozen, after.is_frozen));
    }
    if !econ.is_empty() {
        println!("      State: {}", econ.join(" | "));
    }

    let aura_diffs = diff_auras(&before.auras, &after.auras);
    if !aura_diffs.is_empty() {
        println!("      Auras: {}", aura_diffs.join(" | "));
    }

    if before.board != after.board {
        println!("      Board ({}/7):", after.board.len());
        print!("{}", format_unit_list(&after.board, "        ", None));
    }
    if before.hand != after.hand {
        println!("      Hand ({}/10):", after.hand.len());
        print!("{}", format_unit_list(&after.hand, "        ", None));
    }
    if before.shop != after.shop {
        println!("      Shop ({}):", after.shop.len());
        print!("{}", format_unit_list(&after.shop, "        ", Some(pool)));
    }
    if after.discover_pending.is_some() && before.discover_pending != after.discover_pending {
        if let Some(ref opts) = after.discover_pending {
            println!("      Discover Prompt ({} options):", opts.len());
            print!("{}", format_unit_list(opts, "        ", Some(pool)));
        }
    }
}

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
        tier3::disguised_graverobber::ID | tier4::maw_caster::ID => {
            if let Some(pos) = state.board.iter().position(|u| u.tribe.matches(Tribe::Undead)) {
                return pos;
            }
        }
        tier3::sprightly_scarab::ID => {
            if let Some(pos) = state.board.iter().position(|u| u.tribe.matches(Tribe::Beast)) {
                return pos;
            }
        }
        tier7::captain_sanders::ID => {
            if let Some(pos) = state
                .board
                .iter()
                .position(|u| !u.is_golden && u.tavern_tier <= 6)
            {
                return pos;
            }
        }
        _ => {}
    }

    state.board.len()
}

/// Semi-random heuristic policy for a player's Tavern Phase.
fn choose_semi_random_action(state: &TavernState, rng: &mut Rng) -> TavernAction {
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
                let hand_score =
                    hand_m.attack + hand_m.health + (hand_m.tavern_tier as i32) * 3 + if hand_m.is_golden { 10 } else { 0 };
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
                if state.board[board_pos].card_id == tier4::sky_hatch_runaway::ID {
                    !matches!(
                        state.board[t].card_id,
                        tier4::heroic_underdog::ID
                            | tier4::sindorei_straight_shot::ID
                            | tier7::obsidian_ravager::ID
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
                tier2::sellemental::ID
                    | tier2::tad::ID
                    | tier2::fire_baller::ID
                    | tier2::snow_baller::ID
                    | tier4::air_baller::ID
                    | tier4::snarky_shark::ID
            ) || (u.card_id == tier2::patient_scout::ID && u.scout_tier >= 3)
                || (u.card_id == tier3::greedy_conniver::ID && u.is_golden)
        }) {
            let sell_act = TavernAction::Sell { board_pos: pos };
            if state.is_legal(&sell_act) {
                return sell_act;
            }
        }
    }

    // 9. Refresh if we have free refreshes, or if we have >= 4 gold and nothing to buy.
    if state.is_legal(&TavernAction::Refresh)
        && (state.auras.free_refreshes > 0 || state.gold >= 4 || (state.gold >= 1 && state.shop.is_empty()))
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

fn run_player_tavern_turn(
    label: &str,
    state: &mut TavernState,
    pool: &mut CardPool,
    rng: &mut Rng,
) {
    println!("\n  >>> {label} TAVERN PHASE (Turn {}) <<<", state.turn);
    print_player_snapshot(label, state, pool);

    let max_steps = 40usize;
    for step_num in 1..=max_steps {
        let action = if step_num == max_steps && state.discover_pending.is_none() {
            TavernAction::EndTurn
        } else {
            choose_semi_random_action(state, rng)
        };

        let before = state.clone();
        let desc = describe_action(&before, &action);
        println!("    [{label} Step {step_num:02}] {desc}");

        match state.step(action, pool, rng) {
            Ok(ended) => {
                print_step_diff(&before, state, pool);
                if ended {
                    break;
                }
            }
            Err(err) => {
                println!("      !! ERROR executing {action:?}: {err}");
                break;
            }
        }
    }
}

fn format_side(s: Side) -> &'static str {
    match s {
        Side::A => "P1",
        Side::B => "P2",
    }
}

fn print_combat_trace(
    turn: u32,
    pre_a: &TavernState,
    pre_b: &TavernState,
    res: &BattleResult,
    post_a: &TavernState,
    post_b: &TavernState,
    pool: &CardPool,
) {
    println!("\n  ==========================================================================");
    println!(
        "  COMBAT PHASE — TURN {turn}: P1 (Tier {}, HP {}+{}) vs P2 (Tier {}, HP {}+{})",
        pre_a.tavern_tier,
        pre_a.health,
        pre_a.armor,
        pre_b.tavern_tier,
        pre_b.health,
        pre_b.armor
    );
    println!("  ==========================================================================");

    let mut unit_names: HashMap<UnitId, (Side, String)> = HashMap::new();
    let mut next_id: UnitId = 0;
    for u in pre_a.board.iter().take(MAX_BOARD_SIZE) {
        unit_names.insert(next_id, (Side::A, u.name.clone()));
        next_id += 1;
    }
    for u in pre_b.board.iter().take(MAX_BOARD_SIZE) {
        unit_names.insert(next_id, (Side::B, u.name.clone()));
        next_id += 1;
    }

    println!("  Pre-Combat Board P1 ({}/7):", pre_a.board.len());
    for (i, u) in pre_a.board.iter().take(MAX_BOARD_SIZE).enumerate() {
        println!("    #{i} {}", format_unit_inline(u));
    }
    println!("  Pre-Combat Board P2 ({}/7):", pre_b.board.len());
    let offset = pre_a.board.len().min(MAX_BOARD_SIZE);
    for (i, u) in pre_b.board.iter().take(MAX_BOARD_SIZE).enumerate() {
        println!("    #{} {}", offset + i, format_unit_inline(u));
    }
    println!("  --------------------------------------------------------------------------");

    let fmt_uid = |map: &HashMap<UnitId, (Side, String)>, uid: UnitId| -> String {
        if let Some((side, name)) = map.get(&uid) {
            format!("{} #{uid} \"{name}\"", format_side(*side))
        } else {
            format!("#{uid}")
        }
    };

    for (ev_idx, ev) in res.events.iter().enumerate() {
        let num = ev_idx + 1;
        match ev {
            Event::BattleStart {
                seed,
                first_attacker,
            } => {
                println!(
                    "    ({num:03}) BattleStart: seed={seed}, first_attacker={}",
                    format_side(*first_attacker)
                );
            }
            Event::AttackDeclared {
                side,
                attacker,
                target,
            } => {
                println!(
                    "    ({num:03}) AttackDeclared ({}): {} -> {}",
                    format_side(*side),
                    fmt_uid(&unit_names, *attacker),
                    fmt_uid(&unit_names, *target)
                );
            }
            Event::StatBuff {
                side,
                unit,
                atk_delta,
                hp_delta,
                attack,
                health,
                reason,
            } => {
                println!(
                    "    ({num:03}) StatBuff ({}): {} {:+}/{:+} -> {attack}/{health} [{reason}]",
                    format_side(*side),
                    fmt_uid(&unit_names, *unit),
                    atk_delta,
                    hp_delta
                );
            }
            Event::UnitSummoned {
                side,
                source,
                unit,
                name,
                attack,
                health,
                reason,
            } => {
                unit_names.insert(*unit, (*side, name.clone()));
                println!(
                    "    ({num:03}) UnitSummoned ({}): #{unit} \"{name}\" ({attack}/{health}) from {} [{reason}]",
                    format_side(*side),
                    fmt_uid(&unit_names, *source)
                );
            }
            Event::DamageDealt { unit, amount, from } => {
                println!(
                    "    ({num:03}) DamageDealt: {} takes {amount} dmg from {}",
                    fmt_uid(&unit_names, *unit),
                    fmt_uid(&unit_names, *from)
                );
            }
            Event::DivineShieldPopped { unit } => {
                println!(
                    "    ({num:03}) DivineShieldPopped: {} loses Divine Shield",
                    fmt_uid(&unit_names, *unit)
                );
            }
            Event::VenomousTriggered { attacker, target } => {
                println!(
                    "    ({num:03}) VenomousTriggered: {} poisons {}",
                    fmt_uid(&unit_names, *attacker),
                    fmt_uid(&unit_names, *target)
                );
            }
            Event::Death { unit } => {
                println!(
                    "    ({num:03}) Death: {} dies",
                    fmt_uid(&unit_names, *unit)
                );
            }
            Event::DeityAwakened {
                side,
                deity,
                unit,
                name,
            } => {
                unit_names.insert(*unit, (*side, name.clone()));
                println!(
                    "    ({num:03}) DeityAwakened ({}): {:?} awakens as #{unit} \"{name}\"",
                    format_side(*side),
                    deity
                );
            }
            Event::TurnSkipped { side } => {
                println!("    ({num:03}) TurnSkipped ({})", format_side(*side));
            }
            Event::BattleEnd {
                outcome,
                hero_damage,
            } => {
                let out_str = match outcome {
                    BattleOutcome::AWin => "P1 Wins",
                    BattleOutcome::BWin => "P2 Wins",
                    BattleOutcome::Draw => "Draw",
                };
                println!(
                    "    ({num:03}) BattleEnd: {out_str} (hero_damage = {hero_damage})"
                );
            }
        }
    }

    println!("  --------------------------------------------------------------------------");
    match res.outcome {
        BattleOutcome::AWin => {
            println!(
                "  Outcome: P1 Wins! P2 takes {} damage (HP: {}+{} -> {}+{})",
                res.hero_damage, pre_b.health, pre_b.armor, post_b.health, post_b.armor
            );
            println!("  P1 Survivors ({}):", res.survivors_a.len());
            print!("{}", format_unit_list(&res.survivors_a, "    ", None));
        }
        BattleOutcome::BWin => {
            println!(
                "  Outcome: P2 Wins! P1 takes {} damage (HP: {}+{} -> {}+{})",
                res.hero_damage, pre_a.health, pre_a.armor, post_a.health, post_a.armor
            );
            println!("  P2 Survivors ({}):", res.survivors_b.len());
            print!("{}", format_unit_list(&res.survivors_b, "    ", None));
        }
        BattleOutcome::Draw => {
            println!("  Outcome: Draw! (0 hero damage)");
        }
    }

    // Print any post-combat persistence diffs for P1 and P2:
    let p1_aura_diffs = diff_auras(&pre_a.auras, &post_a.auras);
    if !p1_aura_diffs.is_empty()
        || pre_a.health != post_a.health
        || pre_a.armor != post_a.armor
        || pre_a.bonus_gold_next_turn != post_a.bonus_gold_next_turn
        || pre_a.board != post_a.board
        || pre_a.hand != post_a.hand
    {
        println!("  Post-Combat Persistence (P1):");
        print_step_diff(pre_a, post_a, pool);
    }
    let p2_aura_diffs = diff_auras(&pre_b.auras, &post_b.auras);
    if !p2_aura_diffs.is_empty()
        || pre_b.health != post_b.health
        || pre_b.armor != post_b.armor
        || pre_b.bonus_gold_next_turn != post_b.bonus_gold_next_turn
        || pre_b.board != post_b.board
        || pre_b.hand != post_b.hand
    {
        println!("  Post-Combat Persistence (P2):");
        print_step_diff(pre_b, post_b, pool);
    }
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let mut seed: u64 = 42;
    let mut max_turns: u32 = 20;
    let mut shop_spells = true;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" | "-s" => {
                if let Some(val) = args.get(i + 1).and_then(|s| s.parse::<u64>().ok()) {
                    seed = val;
                    i += 1;
                }
            }
            "--max-turns" | "-t" => {
                if let Some(val) = args.get(i + 1).and_then(|s| s.parse::<u32>().ok()) {
                    max_turns = val;
                    i += 1;
                }
            }
            "--no-spells" => {
                shop_spells = false;
            }
            other => {
                if let Ok(val) = other.parse::<u64>() {
                    seed = val;
                }
            }
        }
        i += 1;
    }

    let templates = full_catalog();
    let mut pool = CardPool::new(templates.clone());
    let mut rng = Rng::new(seed);

    let mut p1 = TavernState::new().with_shop_spells(shop_spells);
    p1.armor = 5;
    p1.auras.deity.kind = DeityKind::CThun;

    let mut p2 = TavernState::new().with_shop_spells(shop_spells);
    p2.armor = 5;
    p2.auras.deity.kind = DeityKind::YShaarj;

    println!("================================================================================");
    println!(
        " SEAGLASS 2-PLAYER FULL MATCH TRACE | Seed: {seed} | Max Turns: {max_turns} | Catalog: {} Solo Minions | Shop Spells: {shop_spells}",
        templates.len()
    );
    println!(" Player 1 (P1): HP 30 + 5 Armor, Deity = C'Thun");
    println!(" Player 2 (P2): HP 30 + 5 Armor, Deity = Y'Shaarj");
    println!("================================================================================");

    for turn in 1..=max_turns {
        println!("\n################################################################################");
        println!("                              TURN {turn} BEGINS");
        println!("################################################################################");

        p1.start_turn(&mut pool, &mut rng);
        p2.start_turn(&mut pool, &mut rng);

        run_player_tavern_turn("P1", &mut p1, &mut pool, &mut rng);
        run_player_tavern_turn("P2", &mut p2, &mut pool, &mut rng);

        let pre_a = p1.clone();
        let pre_b = p2.clone();
        let combat_seed = rng.next_u64();
        let res = TavernState::resolve_combat_pair(&mut p1, &mut p2, combat_seed);
        print_combat_trace(turn, &pre_a, &pre_b, &res, &p1, &p2, &pool);

        if p1.health <= 0 || p2.health <= 0 {
            println!("\n################################################################################");
            if p1.health <= 0 && p2.health <= 0 {
                println!(" GAME OVER ON TURN {turn}: DOUBLE KNOCKOUT!");
            } else if p2.health <= 0 {
                println!(
                    " GAME OVER ON TURN {turn}: P1 WINS! (P1 HP: {}+{}, P2 HP: {})",
                    p1.health, p1.armor, p2.health
                );
            } else {
                println!(
                    " GAME OVER ON TURN {turn}: P2 WINS! (P2 HP: {}+{}, P1 HP: {})",
                    p2.health, p2.armor, p1.health
                );
            }
            println!("################################################################################");
            return;
        }
    }

    println!("\n################################################################################");
    println!(
        " REACHED MAX TURNS ({max_turns}): P1 HP {}+{} vs P2 HP {}+{}",
        p1.health, p1.armor, p2.health, p2.armor
    );
    println!("################################################################################");
}
