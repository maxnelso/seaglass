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
    base_copies_for_tier, tier1_catalog, CardId, CardPool, DeityKind, Rng, TavernAction,
    TavernState, Tribe, Unit,
};

fn card_description(card_id: CardId, is_golden: bool) -> &'static str {
    use cards::tier1::*;
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
        (tokens::TOKEN_ABERRANT_TENTACLE, _) => "Taunt.",
        (tokens::SPELL_BLOOD_GEM, _) => "Spell: Give a friendly minion +1/+1.",
        (tokens::SPELL_TAVERN_COIN, _) => "Spell: Gain 1 Gold.",
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

    let templates = tier1_catalog();
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
