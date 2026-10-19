//! Text formatting of Tavern states, actions and combats, shared by `sim_match` and
//! `match_replay`.

use std::collections::HashMap;

use seaglass::cards::{self, minions, spells, tokens};
use seaglass::{
    base_copies_for_tier, BattleOutcome, BattleResult, CardId, CardPool, DeityKind, Event,
    PlayerAuras, Side, TavernAction, TavernState, Tribe, Unit, UnitId,
};

const MAX_BOARD_SIZE: usize = 7;

/// Copies left in a card pool, by card.
pub trait PoolCounts {
    fn copies_left(&self, card_id: CardId) -> u32;
}

impl PoolCounts for CardPool {
    fn copies_left(&self, card_id: CardId) -> u32 {
        self.remaining_copies(card_id)
    }
}

pub fn format_tribe(t: Tribe) -> &'static str {
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
        Tribe::Naga => "Naga",
        Tribe::DragonNaga => "Dragon/Naga",
        Tribe::DemonNaga => "Demon/Naga",
        Tribe::DragonMurloc => "Dragon/Murloc",
        Tribe::MechDemon => "Mech/Demon",
    }
}

pub fn format_deity_kind(k: DeityKind) -> &'static str {
    match k {
        DeityKind::None => "None",
        DeityKind::CThun => "C'Thun",
        DeityKind::YShaarj => "Y'Shaarj",
    }
}

pub fn format_keywords(u: &Unit) -> String {
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

pub fn format_unit_inline(u: &Unit) -> String {
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
        if u.combine_choose_one {
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

pub fn format_unit_list(units: &[Unit], indent: &str, pool: Option<&dyn PoolCounts>) -> String {
    if units.is_empty() {
        return format!("{indent}(empty)\n");
    }
    let mut out = String::new();
    for (i, u) in units.iter().enumerate() {
        let pool_tag = if let Some(p) = pool {
            if !u.is_spell && u.card_id >= 100 && u.card_id < 800 {
                format!(
                    " [pool {}/{}]",
                    p.copies_left(u.card_id),
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

pub fn print_player_snapshot(label: &str, state: &TavernState, pool: &dyn PoolCounts) {
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
        "  │   Deity: {} ({}/{}) | Tavern Spells Cast: {} | Spell Buff: {:+}/{:+} | Blood Gem: +{}/+{} | Undead +{} ATK (EK died: {})",
        format_deity_kind(state.auras.deity.kind),
        state.auras.deity.attack,
        state.auras.deity.health,
        state.auras.tavern_spells_cast,
        state.auras.spell_bonus_atk,
        state.auras.spell_bonus_hp,
        1 + state.auras.blood_gem_bonus_atk,
        1 + state.auras.blood_gem_bonus_hp,
        state.auras.undead_bonus_attack,
        state.auras.counter(minions::eternal_knight::ID),
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

pub fn describe_action(state: &TavernState, action: &TavernAction) -> String {
    match *action {
        TavernAction::Buy { shop_index } => {
            if let Some(c) = state.shop.get(shop_index) {
                let cost_str = if c.is_spell {
                    if c.costs_health {
                        format!("{} HP", state.effective_spell_buy_cost(c))
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
        TavernAction::PlayOnShop {
            hand_index,
            shop_index,
        } => {
            let spell_desc = state
                .hand
                .get(hand_index)
                .map(format_unit_inline)
                .unwrap_or_else(|| format!("hand[{hand_index}]"));
            let target_desc = state
                .shop
                .get(shop_index)
                .map(format_unit_inline)
                .unwrap_or_else(|| format!("shop[{shop_index}]"));
            format!(
                "Cast Targeted Spell hand[{hand_index}] ({spell_desc}) -> shop[{shop_index}] ({target_desc})"
            )
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
                let all_names: Vec<String> =
                    opts.iter().map(|o| format!("\"{}\"", o.name)).collect();
                format!(
                    "ChooseDiscover[{option_index}]: {chosen} (from [{}])",
                    all_names.join(", ")
                )
            } else {
                format!("ChooseDiscover[{option_index}]")
            }
        }
        TavernAction::EndTurn => "End Turn".to_string(),
        TavernAction::Pass { hand_index } => {
            let card = state
                .hand
                .get(hand_index)
                .map(format_unit_inline)
                .unwrap_or_else(|| format!("hand[{hand_index}]"));
            format!("Pass {card}")
        }
        TavernAction::HeroPower { target } | TavernAction::HeroPower2 { target } => {
            let power = state
                .hero()
                .and_then(|h| seaglass::heroes::data(h.hero_id))
                .map_or("?", |h| h.power.name);
            match target {
                Some(t) => format!("Hero Power {power} -> {t}"),
                None => format!("Hero Power {power}"),
            }
        }
        TavernAction::ChooseHero { option_index } => {
            let hero = state
                .hero_options
                .get(option_index)
                .and_then(|&id| seaglass::heroes::data(id))
                .map_or("?", |h| h.name);
            format!("Choose hero {hero}")
        }
        TavernAction::ChooseTrinket { option_index } => {
            match state.trinket_options.get(option_index) {
                Some(o) => {
                    let name = seaglass::trinkets::data(o.card_id).map_or("?", |t| t.name);
                    format!("Choose trinket {name} for {}g", o.cost)
                }
                None => format!("Choose trinket {option_index}"),
            }
        }
        TavernAction::DarkDiscovery => "Dark Discovery for 3g".to_string(),
    }
}

pub fn diff_auras(before: &PlayerAuras, after: &PlayerAuras) -> Vec<String> {
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
            before.tavern_all_atk, before.tavern_all_hp, after.tavern_all_atk, after.tavern_all_hp
        ));
    }
    if before.undead_bonus_attack != after.undead_bonus_attack {
        diffs.push(format!(
            "undead_bonus_attack: +{} -> +{}",
            before.undead_bonus_attack, after.undead_bonus_attack
        ));
    }
    if before.undead_bonus_health != after.undead_bonus_health {
        diffs.push(format!(
            "undead_bonus_health: +{} -> +{}",
            before.undead_bonus_health, after.undead_bonus_health
        ));
    }
    let ek = |a: &PlayerAuras| a.counter(minions::eternal_knight::ID);
    if ek(before) != ek(after) {
        diffs.push(format!(
            "eternal_knights_died: {} -> {}",
            ek(before),
            ek(after)
        ));
    }
    let (beetle_before, beetle_after) = (tokens::beetle_bonus(before), tokens::beetle_bonus(after));
    if beetle_before != beetle_after {
        diffs.push(format!(
            "beetle_bonus: +{}/+{} -> +{}/+{}",
            beetle_before.0, beetle_before.1, beetle_after.0, beetle_after.1
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
    let volumizers = cards::volumizers::volumizer_bonus;
    let (vol_before, vol_after) = (volumizers(before), volumizers(after));
    if vol_before != vol_after {
        diffs.push(format!(
            "volumizer_bonus: +{}/+{} -> +{}/+{}",
            vol_before.0, vol_before.1, vol_after.0, vol_after.1
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
    if before.tavern_spells_cast != after.tavern_spells_cast {
        diffs.push(format!(
            "tavern_spells_cast: {} -> {}",
            before.tavern_spells_cast, after.tavern_spells_cast
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
    let effects = [
        (spells::SPELL_BOON_OF_BEETLES, "boon_of_beetles_charges"),
        (spells::SPELL_OVERCONFIDENCE, "overconfidence_stacks"),
        (spells::SPELL_TIME_MANAGEMENT, "time_management_next_turn"),
        (spells::SPELL_UPPER_HAND, "upper_hand_stacks"),
        (spells::SPELL_BROOD_OF_NOZDORMU, "brood_of_nozdormu_stacks"),
        (spells::SPELL_SHARING_IS_CARING, "sharing_is_caring_stacks"),
    ];
    for (card_id, label) in effects {
        let (b, a) = (before.effect_stacks(card_id), after.effect_stacks(card_id));
        if b != a {
            diffs.push(format!("{label}: {b} -> {a}"));
        }
    }
    if before.refresh_random_buffs != after.refresh_random_buffs {
        diffs.push(format!(
            "refresh_random_buffs: {:?} -> {:?}",
            before.refresh_random_buffs, after.refresh_random_buffs
        ));
    }
    if before.refresh_blood_gems != after.refresh_blood_gems {
        diffs.push(format!(
            "refresh_blood_gems: {} -> {}",
            before.refresh_blood_gems, after.refresh_blood_gems
        ));
    }
    let fodder = tokens::fodder_per_refresh;
    if fodder(before) != fodder(after) {
        diffs.push(format!(
            "fodder_per_refresh: {:?} -> {:?}",
            fodder(before),
            fodder(after)
        ));
    }
    let ballers = cards::ballers::baller_bonus;
    if ballers(before) != ballers(after) {
        diffs.push(format!(
            "baller_bonus: +{} -> +{}",
            ballers(before),
            ballers(after)
        ));
    }
    let lobsters = |a: &PlayerAuras| a.counter(minions::tasty_lobster::ID);
    if lobsters(before) != lobsters(after) {
        diffs.push(format!(
            "tasty_lobster_stacks: +{} -> +{}",
            lobsters(before),
            lobsters(after)
        ));
    }
    diffs
}

pub fn print_step_diff(before: &TavernState, after: &TavernState, pool: &dyn PoolCounts) {
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
        econ.push(format!(
            "Frozen: {} -> {}",
            before.is_frozen, after.is_frozen
        ));
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

/// Names the units of one combat the way combat traces do: side A's pre-combat board (at most 7
/// units) gets ids `0..lenA`, side B's board the ids after it, and units summoned or awakened
/// during the combat take the names their events give them.
pub struct CombatNames<'a> {
    labels: [&'a str; 2],
    units: HashMap<UnitId, (Side, String)>,
}

impl<'a> CombatNames<'a> {
    /// The names before a combat between `labels[0]` (side A, `pre_a`) and `labels[1]` (side B,
    /// `pre_b`).
    pub fn new(labels: [&'a str; 2], pre_a: &TavernState, pre_b: &TavernState) -> Self {
        let mut units = HashMap::new();
        let mut next_id: UnitId = 0;
        for (side, state) in [(Side::A, pre_a), (Side::B, pre_b)] {
            for u in state.board.iter().take(MAX_BOARD_SIZE) {
                units.insert(next_id, (side, u.name.clone()));
                next_id += 1;
            }
        }
        Self { labels, units }
    }

    /// The names of the given units (`(id, side, name)`), for a combat whose boards don't come
    /// from two Taverns (a Duos combat, a combat from a game log).
    pub fn from_units(
        labels: [&'a str; 2],
        units: impl IntoIterator<Item = (UnitId, Side, String)>,
    ) -> Self {
        let units = units
            .into_iter()
            .map(|(id, side, name)| (id, (side, name)))
            .collect();
        Self { labels, units }
    }

    /// The label of a side.
    pub fn label(&self, side: Side) -> &'a str {
        match side {
            Side::A => self.labels[0],
            Side::B => self.labels[1],
        }
    }

    /// The side and name of unit `uid`, if it is known yet.
    pub fn unit(&self, uid: UnitId) -> Option<(Side, &str)> {
        self.units
            .get(&uid)
            .map(|(side, name)| (*side, name.as_str()))
    }

    /// A unit as traces show it: `P1 #3 "Name"`, or `#3` if it is not known.
    pub fn format_unit(&self, uid: UnitId) -> String {
        match self.units.get(&uid) {
            Some((side, name)) => format!("{} #{uid} \"{name}\"", self.label(*side)),
            None => format!("#{uid}"),
        }
    }

    /// An event as traces show it, without its number (`AttackDeclared (P1): P1 #0 "Joyous" ->
    /// P2 #1 "Zoatroid"`). Call it on every event in order: it learns the names of the units an
    /// event summons or awakens.
    pub fn describe(&mut self, ev: &Event) -> String {
        match ev {
            Event::BattleStart {
                seed,
                first_attacker,
            } => format!(
                "BattleStart: seed={seed}, first_attacker={}",
                self.label(*first_attacker)
            ),
            Event::AttackDeclared {
                side,
                attacker,
                target,
            } => format!(
                "AttackDeclared ({}): {} -> {}",
                self.label(*side),
                self.format_unit(*attacker),
                self.format_unit(*target)
            ),
            Event::StatBuff {
                side,
                unit,
                atk_delta,
                hp_delta,
                attack,
                health,
                reason,
            } => format!(
                "StatBuff ({}): {} {:+}/{:+} -> {attack}/{health} [{reason}]",
                self.label(*side),
                self.format_unit(*unit),
                atk_delta,
                hp_delta
            ),
            Event::UnitSummoned {
                side,
                source,
                unit,
                name,
                attack,
                health,
                reason,
            } => {
                self.units.insert(*unit, (*side, name.clone()));
                format!(
                    "UnitSummoned ({}): #{unit} \"{name}\" ({attack}/{health}) from {} [{reason}]",
                    self.label(*side),
                    self.format_unit(*source)
                )
            }
            Event::UnitTransformed {
                side,
                unit,
                name,
                attack,
                health,
            } => {
                let old = self.format_unit(*unit);
                self.units.insert(*unit, (*side, name.clone()));
                format!(
                    "UnitTransformed ({}): {old} becomes \"{name}\" ({attack}/{health})",
                    self.label(*side)
                )
            }
            Event::WarbandSteppedIn {
                team,
                player,
                first_attacker,
            } => format!(
                "WarbandSteppedIn: team {} player {player}, first_attacker={}",
                self.label(*team),
                self.label(*first_attacker)
            ),
            Event::DamageDealt { unit, amount, from } => format!(
                "DamageDealt: {} takes {amount} dmg from {}",
                self.format_unit(*unit),
                self.format_unit(*from)
            ),
            Event::DivineShieldPopped { unit } => format!(
                "DivineShieldPopped: {} loses Divine Shield",
                self.format_unit(*unit)
            ),
            Event::VenomousTriggered { attacker, target } => format!(
                "VenomousTriggered: {} poisons {}",
                self.format_unit(*attacker),
                self.format_unit(*target)
            ),
            Event::Death { unit } => format!("Death: {} dies", self.format_unit(*unit)),
            Event::DeityAwakened {
                side,
                deity,
                unit,
                name,
            } => {
                self.units.insert(*unit, (*side, name.clone()));
                format!(
                    "DeityAwakened ({}): {:?} awakens as #{unit} \"{name}\"",
                    self.label(*side),
                    deity
                )
            }
            Event::TurnSkipped { side } => format!("TurnSkipped ({})", self.label(*side)),
            Event::BattleEnd {
                outcome,
                hero_damage,
            } => {
                let out_str = match outcome {
                    BattleOutcome::AWin => format!("{} Wins", self.labels[0]),
                    BattleOutcome::BWin => format!("{} Wins", self.labels[1]),
                    BattleOutcome::Draw => "Draw".to_string(),
                };
                format!("BattleEnd: {out_str} (hero_damage = {hero_damage})")
            }
        }
    }
}

/// Print a combat between `labels[0]` (side A) and `labels[1]` (side B): both boards, every
/// event and what persisted into each player's Tavern state.
#[allow(clippy::too_many_arguments)]
pub fn print_combat_trace(
    turn: u32,
    labels: [&str; 2],
    pre_a: &TavernState,
    pre_b: &TavernState,
    res: &BattleResult,
    post_a: &TavernState,
    post_b: &TavernState,
    pool: &dyn PoolCounts,
) {
    let (a, b) = (labels[0], labels[1]);
    println!("\n  ==========================================================================");
    println!(
        "  COMBAT PHASE — TURN {turn}: {a} (Tier {}, HP {}+{}) vs {b} (Tier {}, HP {}+{})",
        pre_a.tavern_tier, pre_a.health, pre_a.armor, pre_b.tavern_tier, pre_b.health, pre_b.armor
    );
    println!("  ==========================================================================");

    println!("  Pre-Combat Board {a} ({}/7):", pre_a.board.len());
    for (i, u) in pre_a.board.iter().take(MAX_BOARD_SIZE).enumerate() {
        println!("    #{i} {}", format_unit_inline(u));
    }
    println!("  Pre-Combat Board {b} ({}/7):", pre_b.board.len());
    let offset = pre_a.board.len().min(MAX_BOARD_SIZE);
    for (i, u) in pre_b.board.iter().take(MAX_BOARD_SIZE).enumerate() {
        println!("    #{} {}", offset + i, format_unit_inline(u));
    }
    println!("  --------------------------------------------------------------------------");

    let mut names = CombatNames::new(labels, pre_a, pre_b);
    for (ev_idx, ev) in res.events.iter().enumerate() {
        println!("    ({:03}) {}", ev_idx + 1, names.describe(ev));
    }

    println!("  --------------------------------------------------------------------------");
    match res.outcome {
        BattleOutcome::AWin => {
            println!(
                "  Outcome: {a} Wins! {b} takes {} damage (HP: {}+{} -> {}+{})",
                res.hero_damage, pre_b.health, pre_b.armor, post_b.health, post_b.armor
            );
            println!("  {a} Survivors ({}):", res.survivors_a.len());
            print!("{}", format_unit_list(&res.survivors_a, "    ", None));
        }
        BattleOutcome::BWin => {
            println!(
                "  Outcome: {b} Wins! {a} takes {} damage (HP: {}+{} -> {}+{})",
                res.hero_damage, pre_a.health, pre_a.armor, post_a.health, post_a.armor
            );
            println!("  {b} Survivors ({}):", res.survivors_b.len());
            print!("{}", format_unit_list(&res.survivors_b, "    ", None));
        }
        BattleOutcome::Draw => {
            println!("  Outcome: Draw! (0 hero damage)");
        }
    }

    // Print any post-combat persistence diffs for both players:
    let p1_aura_diffs = diff_auras(&pre_a.auras, &post_a.auras);
    if !p1_aura_diffs.is_empty()
        || pre_a.health != post_a.health
        || pre_a.armor != post_a.armor
        || pre_a.bonus_gold_next_turn != post_a.bonus_gold_next_turn
        || pre_a.board != post_a.board
        || pre_a.hand != post_a.hand
    {
        println!("  Post-Combat Persistence ({a}):");
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
        println!("  Post-Combat Persistence ({b}):");
        print_step_diff(pre_b, post_b, pool);
    }
}
