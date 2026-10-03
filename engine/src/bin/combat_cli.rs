//! CLI tool to run a combat matchup between two boards, print a complete
//! step-by-step battle log, and compute Win / Tie / Loss probabilities.
//!
//! Usage:
//!   cargo run --bin combat_cli
//!   cargo run --bin combat_cli -- path/to/scenario.yaml [--seed 42] [--sims 10000]
//!   cargo run --bin combat_cli -- --team-a "1/1 card:harmless_bonehead; 1/4 card:rot_hide_gnoll" \
//!                                 --team-b "2/1 card:risen_rider; 1/1 card:cord_puller"

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

use seaglass::{
    parse_unit, simulate, simulate_batch, teams_and_state, BattleOutcome, Defaults, DeityKind,
    Event, GameState, Scenario, Side, Tribe, Unit, UnitId,
};

#[derive(Clone, Debug)]
struct TrackedUnit {
    side: Side,
    name: String,
    attack: i32,
    health: i32,
}

impl TrackedUnit {
    fn tag(&self, id: UnitId) -> String {
        let s = match self.side {
            Side::A => "A",
            Side::B => "B",
        };
        format!("[{s}#{id}] {}", self.name)
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

fn format_deity(kind: DeityKind, atk: i32, hp: i32) -> String {
    match kind {
        DeityKind::None => "None".to_string(),
        DeityKind::CThun => format!("C'Thun ({atk}/{hp})"),
        DeityKind::YShaarj => format!("Y'Shaarj ({atk}/{hp})"),
    }
}

fn print_board_line(side_label: &str, units: &[Unit], start_id: UnitId) {
    println!(" {side_label} ({} minions):", units.len());
    if units.is_empty() {
        println!("   (empty)");
        return;
    }
    for (idx, u) in units.iter().enumerate() {
        let uid = if u.id != 0 {
            u.id
        } else {
            start_id + idx as u32
        };
        let badge = if u.is_golden { "★ " } else { "  " };
        let kws = format_keywords(u);
        println!(
            "   #{uid:<2} {badge}{:<24} {:>2}/{:<2}  (T{} {}){kws}",
            u.name,
            u.attack,
            u.health,
            u.tavern_tier,
            format_tribe(u.tribe)
        );
    }
}

fn bar(rate: f64, width: usize) -> String {
    let filled = ((rate * width as f64).round() as usize).min(width);
    let empty = width - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

fn print_help() {
    println!(
        r#"Seaglass Combat Phase CLI
Usage:
  cargo run --bin combat_cli                                    # Run default sample matchup
  cargo run --bin combat_cli -- <path/to/matchup.yaml>          # Run a YAML scenario/matchup
  cargo run --bin combat_cli -- --team-a "<u1>; <u2>" --team-b "<u1>; <u2>"

Options:
  --seed <u64>       Override single-battle seed for the detailed combat log
  --sims <u32>       Override Monte Carlo rollout count for Win/Tie/Loss odds (default: 10000)
  --team-a "<specs>" Semicolon-separated unit specs for Side A (e.g. "2/1 card:risen_rider; 1/4 card:rot_hide_gnoll")
  --team-b "<specs>" Semicolon-separated unit specs for Side B
  -h, --help         Show this help"#
    );
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return;
    }

    let mut yaml_path: Option<PathBuf> = None;
    let mut seed_override: Option<u64> = None;
    let mut sims_override: Option<u32> = None;
    let mut team_a_inline: Option<String> = None;
    let mut team_b_inline: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--seed" => {
                i += 1;
                seed_override = Some(
                    args.get(i)
                        .expect("--seed requires a number")
                        .parse()
                        .expect("invalid --seed"),
                );
            }
            "--sims" => {
                i += 1;
                sims_override = Some(
                    args.get(i)
                        .expect("--sims requires a number")
                        .parse()
                        .expect("invalid --sims"),
                );
            }
            "--team-a" => {
                i += 1;
                team_a_inline = Some(args.get(i).expect("--team-a requires specs").clone());
            }
            "--team-b" => {
                i += 1;
                team_b_inline = Some(args.get(i).expect("--team-b requires specs").clone());
            }
            other if !other.starts_with('-') => {
                yaml_path = Some(PathBuf::from(other));
            }
            other => {
                eprintln!("Unknown argument {other:?}. Run with --help for usage.");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let (title, board_a, board_b, state, single_seed, base_seed, sims) =
        if team_a_inline.is_some() || team_b_inline.is_some() {
            let defaults = Defaults::default();
            let parse_list = |raw: Option<String>| -> Vec<Unit> {
                raw.unwrap_or_default()
                    .split(';')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|spec| {
                        parse_unit(spec, &defaults)
                            .unwrap_or_else(|e| panic!("invalid unit spec {spec:?}: {e}"))
                    })
                    .collect()
            };
            let ba = parse_list(team_a_inline);
            let bb = parse_list(team_b_inline);
            let s = seed_override.unwrap_or(42);
            let n = sims_override.unwrap_or(10_000);
            ("inline_cli_matchup".to_string(), ba, bb, GameState::default(), s, s, n)
        } else {
            let path = yaml_path.unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("examples/matchups/tier1_showdown.yaml")
            });
            let content = fs::read_to_string(&path).unwrap_or_else(|e| {
                panic!("failed to read YAML file {}: {e}", path.display())
            });
            let scenario: Scenario = serde_yaml::from_str(&content).unwrap_or_else(|e| {
                panic!("failed to parse YAML file {}: {e}", path.display())
            });
            let (ba, bb, st) = teams_and_state(&scenario).unwrap_or_else(|e| {
                panic!("invalid scenario in {}: {e}", path.display())
            });
            let s = seed_override.unwrap_or(scenario.seed);
            let (b_seed, b_n) = match &scenario.batch {
                Some(b) => (b.base_seed, b.n),
                None => (s, 10_000),
            };
            let n = sims_override.unwrap_or(b_n);
            (
                format!("{} ({})", scenario.name, path.display()),
                ba,
                bb,
                st,
                s,
                b_seed,
                n,
            )
        };

    println!("================================================================================");
    println!(" SEAGLASS COMBAT SIMULATOR — {title}");
    println!("================================================================================");
    println!(
        " Side A Hero Tier: {}  |  Deity: {}",
        state.hero_tier_a,
        format_deity(
            state.auras_a.deity.kind,
            state.auras_a.deity.attack,
            state.auras_a.deity.health
        )
    );
    print_board_line("SIDE A STARTING BOARD", &board_a, 0);
    println!("--------------------------------------------------------------------------------");
    println!(
        " Side B Hero Tier: {}  |  Deity: {}",
        state.hero_tier_b,
        format_deity(
            state.auras_b.deity.kind,
            state.auras_b.deity.attack,
            state.auras_b.deity.health
        )
    );
    print_board_line("SIDE B STARTING BOARD", &board_b, board_a.len() as u32);
    println!("================================================================================");

    // Track units by UnitId so the combat log prints human-readable names & live stats.
    let mut tracker: HashMap<UnitId, TrackedUnit> = HashMap::new();
    let mut next_uid: UnitId = 0;
    for u in &board_a {
        tracker.insert(
            next_uid,
            TrackedUnit {
                side: Side::A,
                name: u.name.clone(),
                attack: u.attack,
                health: u.health,
            },
        );
        next_uid += 1;
    }
    for u in &board_b {
        tracker.insert(
            next_uid,
            TrackedUnit {
                side: Side::B,
                name: u.name.clone(),
                attack: u.attack,
                health: u.health,
            },
        );
        next_uid += 1;
    }

    let res = simulate(&board_a, &board_b, &state, single_seed);

    println!("\n>>> SINGLE BATTLE LOG (seed = {single_seed}) <<<");
    let mut strike_num = 0usize;
    let mut header_printed_for_current_strike = false;

    for ev in &res.events {
        match ev {
            Event::BattleStart {
                seed,
                first_attacker,
            } => {
                println!(
                    "  [Start]  Seed {seed} | Side {first_attacker:?} attacks first ({} vs {} minions)",
                    board_a.len(),
                    board_b.len()
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
                if *reason == "Rally" && !header_printed_for_current_strike {
                    strike_num += 1;
                    header_printed_for_current_strike = true;
                    println!("\n  --- Strike {strike_num} (Side {side:?}) ---");
                }
                if let Some(t) = tracker.get_mut(unit) {
                    t.attack = *attack;
                    t.health = *health;
                    let tag = t.tag(*unit);
                    println!(
                        "  [Buff]   ({reason}) {tag} gains {:+}/{:+} -> now {attack}/{health}",
                        atk_delta, hp_delta
                    );
                }
            }
            Event::AttackDeclared {
                side,
                attacker,
                target,
            } => {
                if !header_printed_for_current_strike {
                    strike_num += 1;
                    println!("\n  --- Strike {strike_num} (Side {side:?}) ---");
                }
                header_printed_for_current_strike = false;
                let atk_str = tracker
                    .get(attacker)
                    .map(|t| format!("{} ({}/{})", t.tag(*attacker), t.attack, t.health))
                    .unwrap_or_else(|| format!("#{attacker}"));
                let def_str = tracker
                    .get(target)
                    .map(|t| format!("{} ({}/{})", t.tag(*target), t.attack, t.health))
                    .unwrap_or_else(|| format!("#{target}"));
                println!("  [Attack] {atk_str}  -->  {def_str}");
            }
            Event::DivineShieldPopped { unit } => {
                let u_str = tracker
                    .get(unit)
                    .map(|t| t.tag(*unit))
                    .unwrap_or_else(|| format!("#{unit}"));
                println!("  [Shield] {u_str}'s Divine Shield pops! (0 damage taken)");
            }
            Event::DamageDealt { unit, amount, from } => {
                let from_str = tracker
                    .get(from)
                    .map(|t| t.tag(*from))
                    .unwrap_or_else(|| format!("#{from}"));
                if let Some(t) = tracker.get_mut(unit) {
                    t.health -= *amount;
                    let u_str = t.tag(*unit);
                    println!(
                        "  [Damage] {u_str} takes {amount} dmg from {from_str} -> {}/{}",
                        t.attack, t.health
                    );
                }
            }
            Event::VenomousTriggered { attacker, target } => {
                let atk_str = tracker
                    .get(attacker)
                    .map(|t| t.tag(*attacker))
                    .unwrap_or_else(|| format!("#{attacker}"));
                let def_str = tracker
                    .get(target)
                    .map(|t| t.tag(*target))
                    .unwrap_or_else(|| format!("#{target}"));
                println!("  [Venom]  {atk_str}'s Venomous destroys {def_str}!");
            }
            Event::Death { unit } => {
                let u_str = tracker
                    .get(unit)
                    .map(|t| t.tag(*unit))
                    .unwrap_or_else(|| format!("#{unit}"));
                println!("  [Death]  ☠ {u_str} dies");
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
                if *reason == "Rally" && !header_printed_for_current_strike {
                    strike_num += 1;
                    header_printed_for_current_strike = true;
                    println!("\n  --- Strike {strike_num} (Side {side:?}) ---");
                }
                let src_str = tracker
                    .get(source)
                    .map(|t| t.tag(*source))
                    .unwrap_or_else(|| format!("#{source}"));
                let tracked = TrackedUnit {
                    side: *side,
                    name: name.clone(),
                    attack: *attack,
                    health: *health,
                };
                let new_tag = tracked.tag(*unit);
                tracker.insert(*unit, tracked);
                println!(
                    "  [Summon] ({reason}) {src_str} summons {new_tag} ({attack}/{health})"
                );
            }
            Event::DeityAwakened {
                side,
                unit,
                name,
                ..
            } => {
                let (atk, hp) = match side {
                    Side::A => (state.auras_a.deity.attack, state.auras_a.deity.health),
                    Side::B => (state.auras_b.deity.attack, state.auras_b.deity.health),
                };
                let tracked = TrackedUnit {
                    side: *side,
                    name: name.clone(),
                    attack: atk,
                    health: hp,
                };
                let new_tag = tracked.tag(*unit);
                tracker.insert(*unit, tracked);
                println!(
                    "  [Deity]  ✦ Side {side:?}'s Deity awakens: {new_tag} ({atk}/{hp}) joins the battle!"
                );
            }
            Event::TurnSkipped { side } => {
                println!("  [Skip]   Side {side:?} has no minions with Attack > 0; turn skipped");
            }
            Event::BattleEnd {
                outcome,
                hero_damage,
            } => {
                let label = match outcome {
                    BattleOutcome::AWin => format!("Side A Wins! (deals {hero_damage} hero damage to Side B)"),
                    BattleOutcome::BWin => format!("Side B Wins! (deals {hero_damage} hero damage to Side A)"),
                    BattleOutcome::Draw => "Draw! (0 hero damage)".to_string(),
                };
                println!("\n  [End]    {label}");
            }
        }
    }

    println!("\n--------------------------------------------------------------------------------");
    print_board_line("SIDE A SURVIVORS", &res.survivors_a, 0);
    print_board_line("SIDE B SURVIVORS", &res.survivors_b, 0);

    // Run Monte Carlo batch simulation to report Win / Tie / Loss probabilities.
    let dist = simulate_batch(&board_a, &board_b, &state, base_seed, sims);
    println!("================================================================================");
    println!(
        " MONTE CARLO OUTCOME PROBABILITIES ({} battles, base_seed = {})",
        dist.battles, base_seed
    );
    println!("================================================================================");
    println!(
        "  Side A Win : {:6.2}%  {}  ({}/{})",
        dist.a_win_rate * 100.0,
        bar(dist.a_win_rate, 30),
        dist.a_wins,
        dist.battles
    );
    println!(
        "  Tie        : {:6.2}%  {}  ({}/{})",
        dist.draw_rate * 100.0,
        bar(dist.draw_rate, 30),
        dist.draws,
        dist.battles
    );
    println!(
        "  Side B Win : {:6.2}%  {}  ({}/{})",
        dist.b_win_rate * 100.0,
        bar(dist.b_win_rate, 30),
        dist.b_wins,
        dist.battles
    );
    println!("--------------------------------------------------------------------------------");
    println!(
        "  Hero Damage (Side A perspective): min {:+}, max {:+}, mean {:+.2}",
        dist.damage.min, dist.damage.max, dist.damage.mean
    );
    println!("================================================================================");
}
