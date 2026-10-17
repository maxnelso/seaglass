//! Invariant fuzzer over full random Duos games (`docs/duos_tavern.md`, `docs/duos.md`).
//!
//! 2 to 4 teams (`DuosTavern`s) share one `CardPool` of the Duos catalog and play random turns
//! through the public API: each team's `start_turn`, then both of its players act, interleaved at
//! random, with actions picked from `DuosTavern::valid_actions` (Passes included, biased like the
//! Solo games' actions) until both have ended their turn or an action cap. Then the teams fight in
//! pairs (`DuosTavern::resolve_combat_pair`; the odd team out fights the last eliminated team's
//! ghost with `resolve_combat_against`), and teams at 0 Health or less are out, until one team is
//! left or the turn cap. Some games give the players extra maximum Gold or cards first (Duos
//! minions and spells among them, sometimes a full hand), some Pass a lot, and some players hoard
//! cards (they buy and rarely play), so that every card gets played and Passes queue. Half of them
//! give every player a random implemented hero (`docs/heroes.md`), or offer two to choose from,
//! whose Hero Power the policy uses like any action. Every turn the players get a real lobby (a
//! `seaglass::lobby::Lobby` of every team's players, `docs/tavern.md` §3.1: next and last
//! opponents in the same seat of the opposing team, a team that is out counting as both its players
//! dying), and their lobby gifts are delivered after their team's start of turn and turn
//! (`check_lobby`).
//!
//! After every action, start of turn and combat it checks each team:
//! - every Solo check of both players' Taverns (`check_tavern`, `check_synced`, and the actor's
//!   per-action checks, `Expect`), the teammate's too: the cards a step delivers to them (a Pass,
//!   a gift, the queue) are in their Tavern, and complete their Triples;
//! - [`check_team`]: each player's Health and Armor are the team's, their auras mirror the team's
//!   Passes and their teammate's card counters, a queue delivers as soon as its receiver has
//!   room, the gifts are delivered (only Passes wait, after a combat), and both players are on the
//!   same turn;
//! - [`check_duos_legality`]: `valid_actions` and `is_legal` agree for both players, Passes
//!   included: a Pass is legal with Gold or a free Pass, never of a locked card, never while a
//!   Discover is pending, and no action is legal after `EndTurn`; and (at the lookahead's
//!   decision points) `step` rejects illegal actions, leaving the game as it was;
//! - per Pass ([`check_pass`], [`check_pass_keeps_pool`]): its price (a free Pass's charge, else
//!   1 Gold, spent), the card reaches the teammate's hand or, if it is full, their queue, with its
//!   Golden status and in no group, and the pool is left as it was (unless observers act on it);
//! - [`check_counters`]: the team's Passes this turn and this game go up together, by 1 for a
//!   Pass, and only for a Pass, with a card that Passes, or with a Pass gift the action made;
//! - [`check_queues`]: the queues are first in, first out, a queued card is unchanged while it
//!   waits, and the cards that leave a queue reach their receiver's hand;
//! - per action but a Pass, two metamorphic checks: [`check_like_solo`] (the player's Tavern is
//!   the one the same action gives alone, but for the cards delivered to the hand, and the team
//!   loses the Health it costs) and [`check_teammate_untouched`] (without a gift or a change of
//!   the team's Health or counters, the teammate's Tavern doesn't change, but for what its
//!   `after_teammate_sell` observers do with the step's sales, whatever made them);
//! - [`check_free_passes`] (a `Wanderer Cho`'s free Passes: at most 1, or 2 if Golden, reset at
//!   the start of the turn), the first fighter's switch chance, a warband that sat out keeping its
//!   next-combat effects, `EndTurn` discarding the temporary cards, and no minion of no pool
//!   coming into the shop ([`check_shop_offers`]; a player's own can: `Jandice Barov`);
//! - the pool, across every team's players, queues and gifts ([`check_pool_leaks_with`]);
//! - per combat: [`check_duos_battle`] (the outcome and `hero_damage` as specified, the events
//!   framed by `BattleStart` / `BattleEnd`, the warbands that stepped in, every unit accounted
//!   for, and no unit of a waiting warband attacked, hit or killed), and [`check_combat_applied`]:
//!   each player's `WarbandResult` comes back to that player only, the damage to the losing
//!   team's pool, Armor first, the team's result to both players, and the gifts of the combat
//!   arrive once;
//! - a serde round trip of the team ([`check_team_round_trip`]); and every game is replayed from
//!   its seed, which must give the same game.
//!
//! [`random_duos_battles_keep_the_invariants`] checks the combat invariants over random teams.
//!
//! - `SEAGLASS_FUZZ_DUOS_GAMES`: the number of games (default 4); the other settings are the
//!   Solo games' (`SEAGLASS_FUZZ_SEED`, `_THREADS`, `_TURNS`, `_LOOKAHEAD`, `_OUT` (snapshots in
//!   its `duos` directory), `_STRICT`, `_STOP_AT_KNOWN`).
//! - `SEAGLASS_FUZZ_DUOS_BATTLES`: the number of random Duos battles (default 300).
//! - `SEAGLASS_FUZZ_DUOS_GAME=<seed>`: play that one game, printing its trace.
//! - `SEAGLASS_FUZZ_DUOS_REPRO=<file>`: replay a repro snapshot's failing call.
//! - `SEAGLASS_FUZZ_DUOS_COVERAGE=<n>`: the games the policy check plays, printing what they did.

use super::*;
use seaglass::cards::minions::{
    jumping_jack, mirror_monster, private_chef, voidpriest_cloner, well_wisher,
};
use seaglass::cards::spells::{
    SPELL_PORTAL_IN_A_CRYSTAL, SPELL_PORTAL_IN_A_FOUNTAIN, SPELL_PRIVATE_SELECTION,
    SPELL_WELL_WISHES,
};
use seaglass::{DuosResult, DuosTavern, DuosTeam, Warband, WarbandResult};

/// The Duos games' known bugs (by a substring of their signature), tolerated unless
/// `SEAGLASS_FUZZ_STRICT=1`, so that `cargo test` stays green until they are fixed: each one is a
/// `known_bug` scenario.
const DUOS_KNOWN_BUGS: &[(&str, &str)] = &[];

fn duos_known_bug(failure: &Failure) -> Option<&'static str> {
    DUOS_KNOWN_BUGS
        .iter()
        .find(|(pattern, _)| failure.signature.contains(pattern))
        .map(|(_, why)| *why)
}

/// A Pass's base weight in the policy (Solo games never Pass): high, since a card in hand is
/// usually played first.
const PASS_WEIGHT: u32 = 30;

/// The players' cap on a hand (`docs/tavern.md` §3.2).
const HAND_SIZE: usize = 10;

/// The cards whose effect Passes a card (`docs/duos_tavern.md` §8.3): with none of them in a
/// team's Taverns (and no Pass waiting), only the players' Passes count.
fn passes_by_effect(card_id: u32) -> bool {
    [
        jumping_jack::ID,
        mirror_monster::ID,
        voidpriest_cloner::ID,
        private_chef::ID,
        well_wisher::ID,
        SPELL_PORTAL_IN_A_FOUNTAIN,
        SPELL_PORTAL_IN_A_CRYSTAL,
        SPELL_PRIVATE_SELECTION,
        SPELL_WELL_WISHES,
    ]
    .contains(&card_id)
}

// ---------------------------------------------------------------------------------------------
// Setup
// ---------------------------------------------------------------------------------------------

/// One Duos game's setup, drawn from its seed.
#[derive(Clone, Debug)]
struct DuosConfig {
    teams: usize,
    shop_spells: bool,
    max_turns: u32,
    /// Actions a player takes at most in one turn before ending it.
    action_cap: usize,
    /// Each team's heroes' Armor.
    armor: Vec<[i32; 2]>,
    deities: Vec<[DeityKind; 2]>,
    /// Extra maximum Gold every player starts with.
    rich: u32,
    first_fighter_switch_chance: f64,
    /// Cards given to players before the game: `(team, player, card)`.
    gifts: Vec<(usize, usize, Unit)>,
    /// The policy's weight for each [`Kind`] of action, for each team's players: some hoard
    /// cards (they buy, and rarely play or Pass), so that their hands fill up and the cards
    /// passed to them queue.
    player_weights: Vec<[[u32; KINDS]; 2]>,
    /// Each team's players' heroes (`docs/heroes.md`): in half the games, a random implemented
    /// one each, given at the start (`DuosTavern::with_heroes`) or, in a third of those, offered
    /// (2 each), so that the first action is `ChooseHero`.
    heroes: Vec<[Vec<HeroId>; 2]>,
    /// Whether the heroes are offered rather than given.
    offer_heroes: bool,
    /// The players are offered trinkets on turns 6 and 9 (`docs/trinkets.md` §8.1).
    trinkets: bool,
    /// The players have the Dark Discovery button (`docs/dark_gifts.md` §8.1).
    dark_gifts: bool,
}

impl DuosConfig {
    fn random(seed: u64, turns: u32) -> Self {
        let mut rng = Rng::new(seed ^ 0xD005_F022_5EED_0002);
        let teams = 2 + rng.below(3);
        let shop_spells = rng.below(10) != 0;
        let action_cap = 25 + rng.below(40);
        let mut weights = BASE_WEIGHTS;
        weights[Kind::Pass as usize] = PASS_WEIGHT;
        // (Not the Dark Discovery's weight, added later: every seed's other choices stay as they
        // were.)
        for w in weights.iter_mut().take(Kind::DarkDiscovery as usize) {
            *w = (*w * (2 + rng.below(7) as u32) / 4).max(1);
        }
        // Some games Pass a lot, so that cards go back and forth and queues fill up.
        if rng.below(4) == 0 {
            weights[Kind::Pass as usize] *= 5;
        }
        let armor = (0..teams)
            .map(|_| [rng.below(11) as i32, rng.below(11) as i32])
            .collect();
        let mut deity = || [DeityKind::CThun, DeityKind::YShaarj, DeityKind::None][rng.below(3)];
        let deities = (0..teams).map(|_| [deity(), deity()]).collect();
        let rich = if rng.below(4) == 0 {
            3 + rng.below(13) as u32
        } else {
            0
        };
        let first_fighter_switch_chance = [0.0, 0.5, 0.75, 1.0][rng.below(4)];
        let mut gifts = Vec::new();
        let solo = cards::all_templates();
        let duos: Vec<&CardTemplate> = cards::duos_exclusive_templates()
            .iter()
            .chain(cards::duos_out_of_pool_templates())
            .collect();
        let solo_spells = cards::spells::all_spells();
        let duos_spells = cards::spells::all_duos_spells();
        let card = |rng: &mut Rng| match rng.below(10) {
            0..=5 => {
                let mut unit = if rng.below(3) == 0 {
                    solo[rng.below(solo.len())].instantiate()
                } else {
                    duos[rng.below(duos.len())].instantiate()
                };
                if rng.below(5) == 0 {
                    unit.make_golden();
                }
                unit
            }
            6..=7 => duos_spells[rng.below(duos_spells.len())].clone(),
            _ => solo_spells[rng.below(solo_spells.len())].clone(),
        };
        if rng.below(2) == 0 {
            for team in 0..teams {
                for player in 0..2 {
                    for _ in 0..1 + rng.below(3) {
                        gifts.push((team, player, card(&mut rng)));
                    }
                }
            }
        }
        // Some games start with full hands, so that Passes and gifts queue.
        if rng.below(5) == 0 {
            for _ in 0..1 + rng.below(2) {
                let (team, player) = (rng.below(teams), rng.below(2));
                for _ in 0..7 + rng.below(4) {
                    gifts.push((team, player, card(&mut rng)));
                }
            }
        }
        let player_weights = (0..teams)
            .map(|_| {
                [0, 1].map(|_| {
                    let mut w = weights;
                    if rng.below(6) == 0 {
                        for kind in [
                            Kind::PlayMinion,
                            Kind::PlaySpell,
                            Kind::PlayOnShop,
                            Kind::Pass,
                        ] {
                            w[kind as usize] = (w[kind as usize] / 10).max(1);
                        }
                        w[Kind::BuyMinion as usize] *= 2;
                        w[Kind::BuySpell as usize] *= 2;
                    }
                    w
                })
            })
            .collect();
        // The Dark Discovery button, from an RNG of its own: in half the games. (Drawn before the
        // heroes, which offer the Dark Gift heroes only with it.)
        let dark_gifts = Rng::new(seed ^ 0xDA2C_61F7_D005_0002).below(2) == 0;
        // Heroes come from an RNG of their own, so that they leave the rest of the setup as it is.
        let mut hero_rng = Rng::new(seed ^ 0x4E20_E5ED_D005_0002);
        let (heroes, offer_heroes) = if hero_rng.below(2) == 0 {
            let offer = hero_rng.below(3) == 0;
            let per_player = if offer { 2 } else { 1 };
            let mut offers =
                heroes::draw_offers(2 * teams, per_player, true, dark_gifts, &mut hero_rng)
                    .into_iter();
            let heroes = (0..teams)
                .map(|_| [0, 1].map(|_| offers.next().unwrap_or_default()))
                .collect();
            (heroes, offer)
        } else {
            (vec![[Vec::new(), Vec::new()]; teams], false)
        };
        // Trinkets too, from an RNG of their own: offered in half the games.
        let trinkets = Rng::new(seed ^ 0x7121_4E75_D005_0002).below(2) == 0;
        Self {
            teams,
            shop_spells,
            max_turns: turns / 2 + rng.below(turns as usize + 1) as u32,
            action_cap,
            armor,
            deities,
            rich,
            first_fighter_switch_chance,
            gifts,
            player_weights,
            heroes,
            offer_heroes,
            trinkets,
            dark_gifts,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Invariants
// ---------------------------------------------------------------------------------------------

/// The checks of one team between engine calls (`docs/duos_tavern.md` §3, §5.4, §7): each
/// player's Health and Armor are the team's, their auras mirror the team's Passes and their
/// teammate's card counters and say it is a Duos game, a queue delivers as soon as its receiver
/// has room, the gifts are delivered (after a combat, `all_delivered` is `false`: Passes wait for
/// a pool), and both players are on the same turn.
fn check_team(team: &DuosTavern, all_delivered: bool) -> Result<(), Violation> {
    for (p, s) in team.players.iter().enumerate() {
        if (s.health, s.armor) != (team.health, team.armor) {
            return Err(broken(
                "a player's Health and Armor aren't the team's",
                format!(
                    "player {p}: {}+{} for the team's {}+{}",
                    s.health, s.armor, team.health, team.armor
                ),
            ));
        }
        let mirrored = (s.auras.team_passes_this_turn, s.auras.team_passes_this_game);
        if mirrored != (team.passes_this_turn, team.passes_this_game) {
            return Err(broken(
                "a player's auras don't mirror the team's Passes",
                format!(
                    "player {p}: {mirrored:?} for the team's ({}, {})",
                    team.passes_this_turn, team.passes_this_game
                ),
            ));
        }
        let mate = &team.players[1 - p];
        if s.auras.teammate_counters != mate.auras.card_counters {
            return Err(broken(
                "a player's teammate_counters aren't their teammate's card counters",
                format!(
                    "player {p}: {:?} for {:?}",
                    s.auras.teammate_counters, mate.auras.card_counters
                ),
            ));
        }
        if !s.auras.duos_game {
            return Err(broken(
                "a Duos player's auras don't say it is a Duos game",
                "",
            ));
        }
        if !team.incoming[p].is_empty() && s.hand.len() < HAND_SIZE {
            return Err(broken(
                "a queued card waits while its receiver's hand has room",
                format!(
                    "player {p}: {} cards in hand, queue {}",
                    s.hand.len(),
                    units_desc(&team.incoming[p])
                ),
            ));
        }
        let waiting: Vec<&TeammateGift> = s
            .auras
            .to_teammate
            .iter()
            .filter(|g| all_delivered || !matches!(g, TeammateGift::Pass(_)))
            .collect();
        if !waiting.is_empty() {
            return Err(broken(
                "a gift to the teammate wasn't delivered",
                format!("player {p}: {waiting:?}"),
            ));
        }
        if let Some(u) = team.incoming[p]
            .iter()
            .find(|u| u.id >= MARKER_IDS || (!u.is_spell && u.health < 1))
        {
            return Err(broken(
                "a queued card has a marker id or no Health",
                format!("player {p}: {} (id {})", unit_desc(u), u.id),
            ));
        }
    }
    if team.armor < 0 {
        return Err(broken(format!("the team's Armor is {}", team.armor), ""));
    }
    if team.passes_this_turn > team.passes_this_game {
        return Err(broken(
            "the team has more Passes this turn than this game",
            format!("{} > {}", team.passes_this_turn, team.passes_this_game),
        ));
    }
    if team.first_fighter > 1 {
        return Err(broken(
            format!("the first fighter is player {}", team.first_fighter),
            "",
        ));
    }
    if team.players[0].turn != team.players[1].turn {
        return Err(broken(
            "the teammates are on different turns",
            format!("{} and {}", team.players[0].turn, team.players[1].turn),
        ));
    }
    Ok(())
}

/// A team survives a serde round trip unchanged (snapshots and the Python bindings rely on its
/// serde form holding the whole team).
fn check_team_round_trip(team: &DuosTavern) -> Result<(), Violation> {
    let text = yaml(team);
    match serde_yaml::from_str::<DuosTavern>(&text) {
        Ok(back) if format!("{back:?}") == format!("{team:?}") => Ok(()),
        Ok(back) => Err(broken(
            "a Duos team changes in a serde round trip",
            field_changes(team, &back),
        )),
        Err(e) => Err(broken(
            "a Duos team doesn't deserialize from its serde form",
            e.to_string(),
        )),
    }
}

/// `valid_actions(player)` lists each legal action once, `is_legal` agrees with it on every
/// candidate action (Passes included: legal with a free Pass or 1 Gold, of a card that isn't
/// locked, with no Discover pending, `docs/duos_tavern.md` §5.1), a pending Discover allows only
/// `ChooseDiscover`, and a player who has ended their turn can take no action (§4.2). Returns the
/// actions `is_legal` accepts but `valid_actions` doesn't list.
fn check_duos_legality(
    team: &DuosTavern,
    p: usize,
    valid: &[TavernAction],
) -> Result<Vec<TavernAction>, Violation> {
    let s = &team.players[p];
    if team.ended[p] {
        if !valid.is_empty() {
            return Err(violation(
                FailureKind::IllegalActionAccepted,
                "valid_actions lists actions after EndTurn",
                format!("{valid:?}"),
            ));
        }
        if let Some(a) = candidate_actions(s)
            .into_iter()
            .find(|a| team.is_legal(p, a))
        {
            return Err(violation(
                FailureKind::IllegalActionAccepted,
                format!("is_legal accepts a {:?} after EndTurn", kind_of(s, &a)),
                format!("{a:?}"),
            ));
        }
        return Ok(Vec::new());
    }
    if valid.is_empty() {
        return Err(broken("no action is valid", ""));
    }
    for (i, a) in valid.iter().enumerate() {
        if valid[..i].contains(a) {
            return Err(broken(format!("valid_actions lists {a:?} twice"), ""));
        }
        if !team.is_legal(p, a) {
            return Err(violation(
                FailureKind::LegalActionRejected,
                format!(
                    "valid_actions lists {:?} but is_legal rejects it",
                    kind_of(s, a)
                ),
                format!("{a:?}"),
            ));
        }
    }
    if !s.hero_options.is_empty() {
        // Choosing a hero comes first (`docs/heroes.md` §6).
        let expected: Vec<TavernAction> = (0..s.hero_options.len())
            .map(|option_index| TavernAction::ChooseHero { option_index })
            .collect();
        if valid != expected.as_slice() {
            return Err(broken(
                "a pending hero choice allows other actions",
                format!("{valid:?}"),
            ));
        }
    } else if !s.trinket_options.is_empty() {
        // Then the trinket pick (`docs/trinkets.md` §4).
        let expected: Vec<TavernAction> = (0..s.trinket_options.len())
            .filter(|&i| trinkets::option_price(s, i).is_some())
            .map(|option_index| TavernAction::ChooseTrinket { option_index })
            .collect();
        if valid != expected.as_slice() {
            return Err(broken(
                "a pending trinket offer allows other actions",
                format!("{valid:?}"),
            ));
        }
    } else if let Some(opts) = &s.discover_pending {
        let expected: Vec<TavernAction> = (0..opts.len())
            .map(|option_index| TavernAction::ChooseDiscover { option_index })
            .collect();
        if valid != expected.as_slice() {
            return Err(broken(
                "a pending Discover allows other actions",
                format!("{valid:?}"),
            ));
        }
    }
    let can_pay = team.free_passes(p) > 0 || s.gold >= 1;
    let mut unlisted = Vec::new();
    for a in candidate_actions(s) {
        let legal = team.is_legal(p, &a);
        if let TavernAction::Pass { hand_index } = a {
            let expected = s.discover_pending.is_none()
                && s.hero_options.is_empty()
                && s.trinket_options.is_empty()
                && s.hand
                    .get(hand_index)
                    .is_some_and(|card| card.locked_turns == 0 && card.lockbox_turns_left == 0)
                && can_pay;
            if legal != expected {
                let card = s.hand.get(hand_index).map(unit_desc).unwrap_or_default();
                return Err(violation(
                    if legal {
                        FailureKind::IllegalActionAccepted
                    } else {
                        FailureKind::LegalActionRejected
                    },
                    format!("is_legal says {legal} for a Pass"),
                    format!(
                        "Pass {{ hand_index: {hand_index} }} of {card}: gold {}, free Passes {}, \
                         Discover pending {}, heroes offered {}",
                        s.gold,
                        team.free_passes(p),
                        s.discover_pending.is_some(),
                        s.hero_options.len()
                    ),
                ));
            }
        }
        if legal && !valid.contains(&a) {
            unlisted.push(a);
        }
    }
    Ok(unlisted)
}

/// `(card_id, Golden)` of a card, to follow it across zones.
fn identity(u: &Unit) -> (u32, bool) {
    (u.card_id, u.is_golden)
}

/// How many of `units` are the card `id`.
fn count(units: &[Unit], id: (u32, bool)) -> usize {
    units.iter().filter(|u| identity(u) == id).count()
}

/// The copies left in the pool of each catalog card.
fn pool_counts(pool: &CardPool, catalog: &[CardTemplate]) -> Vec<u32> {
    catalog
        .iter()
        .map(|t| pool.remaining_copies(t.card_id))
        .collect()
}

/// Whether observers act on `p`'s Pass (`docs/duos_tavern.md` §5.6): the Gold-spent ones of a
/// Pass paid in Gold (the passer's minions' `gold_spent`, their Hero Power's `hero_gold_spent`),
/// and the `after_pass` and `after_team_pass` ones. They can reach the pool: a Pass's Gold that
/// has an `Air Revenant` cast `Easterly Winds` can have a `Felboar` consume a minion in the
/// Tavern.
fn observers_act_on_the_pass(before: &DuosTavern, p: usize) -> bool {
    let passer = &before.players[p];
    let paid = before.free_passes(p) == 0;
    let board = |s: &TavernState, has: fn(&cards::CardHooks) -> bool| {
        s.board.iter().any(|u| has(cards::hooks(u.card_id)))
    };
    let hero_spends = passer.hero().is_some_and(|h| {
        cards::hooks(h.power.card_id).hero_gold_spent.is_some()
            || h.second_power
                .as_ref()
                .is_some_and(|sp| cards::hooks(sp.card_id).hero_gold_spent.is_some())
    });
    (paid && (board(passer, |h| h.gold_spent.is_some()) || hero_spends))
        || board(passer, |h| h.after_pass.is_some())
        || before
            .players
            .iter()
            .any(|s| board(s, |h| h.after_team_pass.is_some()))
}

/// A Pass moves a card from hand to hand (`docs/duos_tavern.md` §5.3): it leaves the pool as it
/// was (`before`, [`pool_counts`]), unless the card was in a group, whose rest is then discarded,
/// or observers act on it ([`observers_act_on_the_pass`]).
fn check_pass_keeps_pool(
    card: &Unit,
    before: &[u32],
    pool: &CardPool,
    catalog: &[CardTemplate],
) -> Result<(), Violation> {
    if card.willbreaker_group != 0 {
        return Ok(());
    }
    let changed: Vec<String> = catalog
        .iter()
        .zip(before)
        .filter(|(t, &n)| pool.remaining_copies(t.card_id) != n)
        .map(|(t, &n)| format!("{}: {n} -> {}", t.name, pool.remaining_copies(t.card_id)))
        .collect();
    if !changed.is_empty() {
        return Err(broken(
            "a Pass changed the pool",
            format!("{}: {}", unit_desc(card), changed.join(", ")),
        ));
    }
    Ok(())
}

/// A Pass (`docs/duos_tavern.md` §5.2-§5.6): it uses a free Pass if the passer has one, else
/// costs 1 Gold, spent; and the card reaches the teammate's hand, or their queue if the hand is
/// full, with its Golden status (or completes a Triple there).
fn check_pass(
    before: &DuosTavern,
    after: &DuosTavern,
    p: usize,
    hand_index: usize,
) -> Result<(), Violation> {
    let mate = 1 - p;
    let (b, a) = (&before.players[p], &after.players[p]);
    let card = &b.hand[hand_index];
    if before.free_passes(p) > 0 {
        // (Unless free-Pass minions left the board in the step, with their charges: a queued
        // Wanderer Cho the Pass made room for completes a Triple with the two on the board.)
        let free_pass_minions = |s: &TavernState| {
            s.board
                .iter()
                .filter(|u| cards::hooks(u.card_id).has(CardFlags::FREE_PASS))
                .count()
        };
        let one_used = after.free_passes(p) + 1 == before.free_passes(p)
            || free_pass_minions(a) != free_pass_minions(b);
        if !one_used || a.gold != b.gold {
            return Err(broken(
                "a free Pass didn't use exactly one free Pass",
                format!(
                    "free Passes {} -> {}, gold {} -> {}",
                    before.free_passes(p),
                    after.free_passes(p),
                    b.gold,
                    a.gold
                ),
            ));
        }
    } else if a.gold + 1 != b.gold || a.gold_spent_this_turn != b.gold_spent_this_turn + 1 {
        return Err(broken(
            "a Pass didn't cost 1 Gold, spent",
            format!(
                "gold {} -> {}, gold spent this turn {} -> {}",
                b.gold, a.gold, b.gold_spent_this_turn, a.gold_spent_this_turn
            ),
        ));
    }
    let id = identity(card);
    let (mb, ma) = (&before.players[mate], &after.players[mate]);
    let held = |s: &TavernState, queue: &[Unit]| count(&s.hand, id) + count(queue, id);
    let arrived = held(ma, &after.incoming[mate]) > held(mb, &before.incoming[mate]);
    // (Two copies of a minion there make a Triple: the Golden goes to the hand. A wildcard
    // `Elemental of Surprise` triples into another Elemental's Golden.)
    let golden = (card.card_id, true);
    let goldens = |s: &TavernState| s.hand.iter().filter(|u| u.is_golden).count();
    let tripled = !card.is_spell
        && !card.is_golden
        && (count(&ma.hand, golden) > count(&mb.hand, golden)
            || (is_wildcard(card) && goldens(ma) > goldens(mb)));
    if !arrived && !tripled {
        return Err(broken(
            "a passed card didn't reach the teammate",
            format!(
                "{}: hand {} queue {}",
                unit_desc(card),
                units_desc(&ma.hand),
                units_desc(&after.incoming[mate])
            ),
        ));
    }
    // It arrives as a plain card, in no group (§5.3).
    let plain =
        |u: &Unit| identity(u) != id || (u.willbreaker_group == 0 && u.willbreaker_remaining == 0);
    let grouped = ma.hand.len() > mb.hand.len() && !ma.hand[mb.hand.len()..].iter().all(plain);
    if grouped || !after.incoming[mate].iter().all(plain) {
        return Err(broken(
            "a passed card arrived in a card group",
            format!("{} (group {})", unit_desc(card), card.willbreaker_group),
        ));
    }
    Ok(())
}

/// The Pass gifts `s`'s player has for their teammate (`TeammateGift::Pass`).
fn pass_gifts(s: &TavernState) -> usize {
    s.auras
        .to_teammate
        .iter()
        .filter(|g| matches!(g, TeammateGift::Pass(_)))
        .count()
}

/// Whether a card of the team can Pass a card by its effect this step, or a Pass waits.
fn could_pass_by_effect(team: &DuosTavern) -> bool {
    team.players.iter().any(|s| {
        let prompts = s.discover_pending.iter().chain(&s.discover_queue).flatten();
        s.board
            .iter()
            .chain(&s.hand)
            .chain(&s.shop)
            .chain(prompts)
            .any(|u| passes_by_effect(u.card_id))
            || s.auras
                .to_teammate
                .iter()
                .any(|g| matches!(g, TeammateGift::Pass(_)))
    })
}

/// The team counters (`docs/duos_tavern.md` §7.1): the Passes this turn and this game go up
/// together, by 1 for a player's Pass, and only with a Pass, the player's or a card effect's: a
/// card that Passes, in the team's Taverns before or after the step, or a Pass gift the action
/// itself made (`gift_passes`, read from the action on the player's Tavern alone: Madam Goya's
/// Hero Power).
fn check_counters(
    before: &DuosTavern,
    after: &DuosTavern,
    pass: bool,
    gift_passes: usize,
) -> Result<(), Violation> {
    let d_turn = i64::from(after.passes_this_turn) - i64::from(before.passes_this_turn);
    let d_game = i64::from(after.passes_this_game) - i64::from(before.passes_this_game);
    let detail = format!(
        "this turn {} -> {}, this game {} -> {}",
        before.passes_this_turn,
        after.passes_this_turn,
        before.passes_this_game,
        after.passes_this_game
    );
    if d_turn != d_game {
        return Err(broken(
            "the team's Passes this turn and this game went up by different amounts",
            detail,
        ));
    }
    // (A card can Pass by its effect in the step it arrives: a minion transformed into a
    // `Voidpriest Cloner` sees the rest of a used-up card group discarded.)
    let by_effect = gift_passes > 0 || could_pass_by_effect(before) || could_pass_by_effect(after);
    let by_player = i64::from(pass);
    if d_game < by_player || (d_game > by_player && !by_effect) {
        return Err(broken(
            if pass {
                "a Pass didn't count once for the team"
            } else {
                "the team's Passes went up without a Pass"
            },
            detail,
        ));
    }
    Ok(())
}

/// The queues (`docs/duos_tavern.md` §5.4): first in, first out (cards leave from the front, new
/// ones join at the back), a queued card is unchanged while it waits, and the cards that left a
/// queue reached their receiver's hand.
fn check_queues(before: &DuosTavern, after: &DuosTavern) -> Result<(), Violation> {
    for p in 0..2 {
        let (b, a) = (&before.incoming[p], &after.incoming[p]);
        let Some(left) =
            (0..=b.len()).find(|&k| a.len() >= b.len() - k && a[..b.len() - k] == b[k..])
        else {
            return Err(broken(
                "a queue isn't first in, first out",
                format!("player {p}: {} -> {}", units_desc(b), units_desc(a)),
            ));
        };
        let hand = &after.players[p].hand;
        // (A card that completed a Triple is in its Golden; a wildcard `Elemental of Surprise`
        // in another Elemental's.)
        let golden = |u: &Unit| {
            hand.iter()
                .any(|h| h.is_golden && (h.card_id == u.card_id || is_wildcard(u)))
        };
        if let Some(u) = b[..left]
            .iter()
            .find(|u| !hand.iter().any(|h| identity(h) == identity(u)) && !golden(u))
        {
            return Err(broken(
                "a card left its queue for nowhere",
                format!(
                    "player {p}: {} not in hand {}",
                    unit_desc(u),
                    units_desc(hand)
                ),
            ));
        }
    }
    Ok(())
}

/// The team's Health never goes up (rewinds prevent damage instead), but for the 30 Health
/// Patchwerk's Hero Power gives another hero as they get it (`docs/heroes.md` §11.2: Sir Finley's
/// or Genn's Discover, The Nameless One's copy).
fn check_team_health_kept(before: &DuosTavern, team: &DuosTavern) -> Result<(), Violation> {
    let patched = |s: &seaglass::TavernState| {
        s.hero().is_some_and(|h| {
            h.hero_id != heroes::patchwerk::ID
                && std::iter::once(&h.power)
                    .chain(&h.second_power)
                    .any(|p| p.card_id == heroes::patchwerk::POWER)
        })
    };
    let gained = (0..2)
        .filter(|&p| patched(&team.players[p]) && !patched(&before.players[p]))
        .count() as i32;
    if team.health > before.health + 30 * gained {
        return Err(broken(
            "the team's Health went up",
            format!("{} -> {}", before.health, team.health),
        ));
    }
    Ok(())
}

/// No minion of no pool (the Naga-only ones) comes into a Duos shop (`docs/duos_tavern.md` §8):
/// the shop holds no more copies of one than the player had in their shop, on their board and in
/// their hand before (`Jandice Barov` swaps a friendly minion into the shop as it is).
fn check_shop_offers(before: &TavernState, after: &TavernState) -> Result<(), Violation> {
    let copies = |units: &[Unit], id: u32| units.iter().filter(|u| u.card_id == id).count();
    for u in &after.shop {
        let id = u.card_id;
        let had = copies(&before.shop, id)
            + copies(&before.board, id)
            + copies(&before.hand, id)
            + copies(before.last_opponent_warband(), id);
        if out_of_pool_ids().contains(&id) && copies(&after.shop, id) > had {
            return Err(broken(
                format!("a minion of no pool is offered in the shop: {}", u.name),
                unit_desc(u),
            ));
        }
    }
    Ok(())
}

/// A hero's Health beyond 30 and Duos Armor (`heroes::starting_bonus`), `(0, 0)` for none.
fn hero_bonus(s: &TavernState) -> (i32, i32) {
    s.hero()
        .and_then(|h| heroes::data(h.hero_id))
        .map_or((0, 0), |h| heroes::starting_bonus(h, true))
}

/// `ChooseHero` gave player `p` the hero `id` (`docs/heroes.md` §2.3, §6): no option is left,
/// and the team's pool took the hero's Health beyond 30 and Duos Armor (`Patchwerk`'s 30 extra
/// Health), less a previous hero's.
fn check_hero_chosen(
    before: &DuosTavern,
    team: &DuosTavern,
    p: usize,
    id: HeroId,
) -> Result<(), Violation> {
    let s = &team.players[p];
    let hero = s.hero().map(|h| h.hero_id);
    if hero != Some(id) || !s.hero_options.is_empty() {
        return Err(broken(
            "ChooseHero did not give the chosen hero",
            format!(
                "chose {id}, has {hero:?}, options left {:?}",
                s.hero_options
            ),
        ));
    }
    let ((old_health, old_armor), (health, armor)) =
        (hero_bonus(&before.players[p]), hero_bonus(s));
    let expected = (
        before.health - old_health + health,
        before.armor - old_armor + armor,
    );
    if (team.health, team.armor) != expected {
        return Err(broken(
            "the team's pool did not take the chosen hero's Health and Armor",
            format!(
                "{} Health {} Armor -> {} Health {} Armor, expected {expected:?}",
                before.health, before.armor, team.health, team.armor
            ),
        ));
    }
    Ok(())
}

/// A Duos action other than a Pass is its player's own Tavern action (`docs/duos_tavern.md`
/// §4.2), plus what the team adds: the gifts to the teammate delivered, and the cards the queue
/// delivers to the hand. So the player's Tavern after it is the Tavern the same action gives
/// alone (`solo`, stepped with the same pool and RNG): the same Gold, Tier, board, shop and
/// Discover, the same hand but for the cards added after its own, and the team's pool took the
/// Health it cost (a metamorphic check). Not when the action gave the whole team something, or
/// Passed a card (its observers and the team counters act on the player too), or a card the
/// queue delivered completed a Triple.
fn check_like_solo(
    solo: &TavernState,
    before: &DuosTavern,
    after: &DuosTavern,
    p: usize,
) -> Result<(), Violation> {
    let team_wide = solo.auras.to_teammate.iter().any(|g| {
        matches!(
            g,
            TeammateGift::Effect { team: true, .. } | TeammateGift::Pass(_)
        )
    });
    let a = &after.players[p];
    let goldens = |s: &TavernState| s.hand.iter().filter(|u| u.is_golden).count();
    // A trinket that reacts to cards entering the hand (`Trusty Crowbar`) also sees the cards the
    // queue delivers.
    let sees_deliveries = a.hand.len() > solo.hand.len()
        && a.auras.trinkets.iter().any(|t| {
            seaglass::cards::hooks(t.card_id)
                .trinket_card_added
                .is_some()
        });
    if team_wide || sees_deliveries || goldens(a) != goldens(solo) {
        return Ok(());
    }
    let stats = |units: &[Unit]| -> Vec<(u32, bool, i32, i32)> {
        units
            .iter()
            .map(|u| (u.card_id, u.is_golden, u.attack, u.health))
            .collect()
    };
    let ids = |units: &[Unit]| -> Vec<u32> { units.iter().map(|u| u.card_id).collect() };
    let prompt = |s: &TavernState| s.discover_pending.as_deref().map(ids);
    let d_solo = hit_points(solo) - hit_points(&before.players[p]);
    let d_team = (after.health + after.armor) - (before.health + before.armor);
    let problem = if d_team != d_solo {
        Some((
            "Health",
            format!("the team's Health + Armor {d_team:+}, the player's alone {d_solo:+}"),
        ))
    } else if (a.gold, a.max_gold, a.tavern_tier) != (solo.gold, solo.max_gold, solo.tavern_tier) {
        Some((
            "Gold or Tier",
            format!(
                "gold {}/{} tier {}, alone {}/{} tier {}",
                a.gold, a.max_gold, a.tavern_tier, solo.gold, solo.max_gold, solo.tavern_tier
            ),
        ))
    } else if stats(&a.board) != stats(&solo.board) {
        Some((
            "board",
            format!(
                "{}, alone {}",
                units_desc(&a.board),
                units_desc(&solo.board)
            ),
        ))
    } else if ids(&a.shop) != ids(&solo.shop) {
        Some((
            "shop",
            format!("{}, alone {}", units_desc(&a.shop), units_desc(&solo.shop)),
        ))
    } else if a.hand.len() < solo.hand.len() || ids(&a.hand[..solo.hand.len()]) != ids(&solo.hand) {
        Some((
            "hand",
            format!("{}, alone {}", units_desc(&a.hand), units_desc(&solo.hand)),
        ))
    } else if prompt(a) != prompt(solo) {
        Some((
            "Discover",
            format!("{:?}, alone {:?}", prompt(a), prompt(solo)),
        ))
    } else {
        None
    };
    match problem {
        Some((what, detail)) => Err(broken(
            format!("a Duos action's {what} isn't the same action's alone"),
            detail,
        )),
        None => Ok(()),
    }
}

/// Teammates are not friendly (`docs/duos_tavern.md` §2, §7): a player's action other than a
/// Pass reaches their teammate's Tavern only through the team's channels: a gift, the
/// `after_teammate_sell` observers of the step's sales (whatever made them: a Sell, Mutanus'
/// Devour, Channel the Devourer), and the team's Health and counters (whose "wherever this is"
/// auras then resync). Without a gift or a change of the team's Health, Armor or counters (`solo`
/// gave no gift and kept the card counters), the teammate's Tavern and queue are as they were,
/// but for what their `after_teammate_sell` observers do with `solo`'s sales (run on a copy).
fn check_teammate_untouched(
    solo: &TavernState,
    before: &DuosTavern,
    after: &DuosTavern,
    p: usize,
    action: TavernAction,
    pool: &CardPool,
) -> Result<(), Violation> {
    let channel = !solo.auras.to_teammate.is_empty()
        || (after.health, after.armor) != (before.health, before.armor)
        || solo.auras.card_counters != before.players[p].auras.card_counters;
    if channel {
        return Ok(());
    }
    let mate = 1 - p;
    let mut expected = before.players[mate].clone();
    let (mut pool, mut rng) = (pool.clone(), Rng::new(0));
    for sold in &solo.sold_this_step {
        cards::after_teammate_sell(&mut expected, sold, &mut pool, &mut rng);
    }
    let (b, a) = (&expected, &after.players[mate]);
    let stats = |units: &[Unit]| -> Vec<(u32, bool, i32, i32)> {
        units
            .iter()
            .map(|u| (u.card_id, u.is_golden, u.attack, u.health))
            .collect()
    };
    let view = |s: &TavernState| {
        (
            stats(&s.board),
            stats(&s.hand),
            stats(&s.shop),
            (s.gold, s.tavern_tier, s.upgrade_cost),
            s.discover_pending.as_deref().map(stats),
        )
    };
    if view(a) != view(b) || after.incoming[mate] != before.incoming[mate] {
        let kind = kind_of(&before.players[p], &action);
        let (what, sales) = match solo.sold_this_step.len() {
            0 => (
                format!("a player's {kind:?} changed their teammate's Tavern"),
                String::new(),
            ),
            n => (
                format!("a player's {kind:?} left their teammate's Tavern other than its sales do"),
                format!(" (after {n} sales: {})", units_desc(&solo.sold_this_step)),
            ),
        };
        return Err(broken(
            what,
            format!(
                "teammate board {} -> {}, hand {} -> {}, gold {} -> {}{sales}",
                units_desc(&b.board),
                units_desc(&a.board),
                units_desc(&b.hand),
                units_desc(&a.hand),
                b.gold,
                a.gold
            ),
        ));
    }
    Ok(())
}

/// The free Passes (`docs/duos_tavern.md` §5.2): each `FREE_PASS` minion on the board (`Wanderer
/// Cho`) has at most 1 charge, or 2 if Golden; at the start of the turn (`reset`), exactly that.
fn check_free_passes(s: &TavernState, reset: bool) -> Result<(), Violation> {
    for u in &s.board {
        if cards::hooks(u.card_id).has(CardFlags::FREE_PASS) {
            let most = if u.is_golden { 2 } else { 1 };
            if u.charges > most || (reset && u.charges != most) {
                return Err(broken(
                    format!(
                        "a free-Pass minion has {} free Passes{}",
                        u.charges,
                        if reset {
                            " at the start of the turn"
                        } else {
                            ""
                        }
                    ),
                    unit_desc(u),
                ));
            }
        }
    }
    Ok(())
}

/// A team as it fights (`DuosTavern::team` after the combat's aura sync): its warbands and its
/// first fighter.
fn fighting_team(team: &DuosTavern) -> DuosTeam {
    let mut team = team.clone();
    team.mirror();
    for s in &mut team.players {
        s.sync_all_auras();
    }
    team.team()
}

/// The checks of one Duos combat's result (`docs/duos.md` §3-§7): the events start with
/// `BattleStart` and end with the result's `BattleEnd`; each team's first warband steps in first
/// and every warband steps in at most once, as its result's `fought` says; every unit is on a
/// starting board or summoned with a fresh id, dies at most once, and attacks, is attacked or is
/// hit only while alive on a fighting warband (no attack, damage or death reaches a waiting
/// warband); each warband's survivors and dead units are its units that didn't die and did, and a
/// warband that never fought comes back as it was given; the winning team's fighting warband has
/// minions left; and `hero_damage` is the winning warband's player's Tier plus its survivors'
/// Tiers, filled out with its teammate's board if the teammate never fought, up to 7 minions
/// (0 on a draw).
fn check_duos_battle(res: &DuosResult, a: &DuosTeam, b: &DuosTeam) -> Result<(), Violation> {
    let teams = [a, b];
    let results = [&res.team_a, &res.team_b];
    let side_index = |side: Side| usize::from(side == Side::B);
    if !matches!(res.events.first(), Some(Event::BattleStart { .. })) {
        return Err(broken("the events don't start with BattleStart", ""));
    }
    match res.events.last() {
        Some(Event::BattleEnd {
            outcome,
            hero_damage,
        }) if *outcome == res.outcome && *hero_damage == res.hero_damage => {}
        other => {
            return Err(broken(
                "the events don't end with the BattleEnd of the result",
                format!("{other:?}"),
            ))
        }
    }
    // Unit ids (§3.1): the first warbands, A's then B's, then the second warbands.
    let mut owner: BTreeMap<UnitId, (usize, usize)> = BTreeMap::new();
    let mut next_id: UnitId = 0;
    for nth in 0..2 {
        for (t, team) in teams.iter().enumerate() {
            let player = if nth == 0 { team.first } else { 1 - team.first };
            for _ in &team.warbands[player].board {
                owner.insert(next_id, (t, player));
                next_id += 1;
            }
        }
    }
    let mut fighting: [Option<usize>; 2] = [None; 2];
    let mut stepped_in = [[false; 2]; 2];
    let mut entries = 0;
    let mut dead: BTreeSet<UnitId> = BTreeSet::new();
    for (i, ev) in res.events.iter().enumerate() {
        let on_fighting = |id: &UnitId, fighting: &[Option<usize>; 2]| {
            owner
                .get(id)
                .filter(|(t, p)| fighting[*t] == Some(*p) && !dead.contains(id))
                .map(|(t, _)| *t)
        };
        let problem = match ev {
            Event::WarbandSteppedIn { team, player, .. } => {
                let t = side_index(*team);
                entries += 1;
                if *player > 1 || stepped_in[t][(*player).min(1)] {
                    Some(format!("warband {player} of team {team:?} steps in again"))
                } else if entries <= 2 && (t != entries - 1 || *player != teams[t].first) {
                    Some(format!(
                        "the opening warband {entries} is team {team:?}'s player {player}"
                    ))
                } else {
                    stepped_in[t][*player] = true;
                    fighting[t] = Some(*player);
                    None
                }
            }
            Event::UnitSummoned { side, unit, .. } | Event::DeityAwakened { side, unit, .. } => {
                let t = side_index(*side);
                match fighting[t] {
                    None => Some(format!("unit {unit} is summoned before its team fights")),
                    Some(p) => owner
                        .insert(*unit, (t, p))
                        .map(|_| format!("unit {unit} is summoned with an id already used")),
                }
            }
            Event::Death { unit } => match on_fighting(unit, &fighting) {
                Some(_) => {
                    dead.insert(*unit);
                    None
                }
                None => Some(format!(
                    "unit {unit} dies, but it isn't alive on a fighting warband"
                )),
            },
            Event::AttackDeclared {
                side,
                attacker,
                target,
            } => {
                let t = side_index(*side);
                if on_fighting(attacker, &fighting) != Some(t) {
                    Some(format!(
                        "unit {attacker} attacks for side {side:?}, but it isn't alive there"
                    ))
                } else if on_fighting(target, &fighting) != Some(1 - t) {
                    Some(format!(
                        "unit {target} is attacked, but it isn't an enemy alive on a fighting warband"
                    ))
                } else {
                    None
                }
            }
            Event::DamageDealt { unit, .. } | Event::DivineShieldPopped { unit } => {
                on_fighting(unit, &fighting).is_none().then(|| {
                    format!("unit {unit} is hit, but it isn't alive on a fighting warband")
                })
            }
            _ => None,
        };
        if let Some(problem) = problem {
            return Err(broken(
                format!(
                    "the Duos battle's events don't add up: {}",
                    normalize(&problem)
                ),
                format!("event {i}: {problem}"),
            ));
        }
    }
    for t in 0..2 {
        for p in 0..2 {
            let r = &results[t][p];
            if r.fought != stepped_in[t][p] {
                return Err(broken(
                    "a warband's fought isn't whether it stepped in",
                    format!("team {t} player {p}: fought {}", r.fought),
                ));
            }
            if r.survivors.len() > 7 {
                return Err(broken(
                    format!("a warband has {} survivors", r.survivors.len()),
                    units_desc(&r.survivors),
                ));
            }
            if let Some(u) = r.survivors.iter().find(|u| u.health <= 0) {
                return Err(broken(
                    format!("a dead minion survived: {}", u.name),
                    units_desc(&r.survivors),
                ));
            }
            let mine = |dead_ones: bool| -> BTreeSet<UnitId> {
                owner
                    .iter()
                    .filter(|(id, o)| **o == (t, p) && dead.contains(id) == dead_ones)
                    .map(|(id, _)| *id)
                    .collect()
            };
            let ids = |units: &[Unit]| -> Vec<UnitId> { units.iter().map(|u| u.id).collect() };
            let survivors = ids(&r.survivors);
            let died = ids(&r.dead_units);
            let unique = |v: &[UnitId]| v.iter().collect::<BTreeSet<_>>().len() == v.len();
            if survivors.iter().copied().collect::<BTreeSet<_>>() != mine(false)
                || !unique(&survivors)
            {
                return Err(broken(
                    "a warband's survivors aren't its units that didn't die",
                    format!(
                        "team {t} player {p}: survivors {survivors:?}, expected {:?}",
                        mine(false)
                    ),
                ));
            }
            if died.iter().copied().collect::<BTreeSet<_>>() != mine(true) || !unique(&died) {
                return Err(broken(
                    "a warband's dead units aren't its units that died",
                    format!(
                        "team {t} player {p}: dead_units {died:?}, died {:?}",
                        mine(true)
                    ),
                ));
            }
            if !r.fought {
                let given = &teams[t].warbands[p].board;
                let same = r.survivors.len() == given.len()
                    && r.survivors.iter().zip(given).all(|(s, g)| {
                        (s.card_id, s.attack, s.health) == (g.card_id, g.attack, g.health)
                    });
                if !same {
                    return Err(broken(
                        "a warband that never fought doesn't come back as it was given",
                        format!(
                            "team {t} player {p}: {} -> {}",
                            units_desc(given),
                            units_desc(&r.survivors)
                        ),
                    ));
                }
            }
        }
    }
    let expected = match res.outcome {
        BattleOutcome::Draw => 0,
        outcome => {
            let w = usize::from(outcome == BattleOutcome::BWin);
            let side = if w == 0 { Side::A } else { Side::B };
            // The winning warband is the last one of the winning team that stepped in.
            let f = res
                .events
                .iter()
                .rev()
                .find_map(|e| match e {
                    Event::WarbandSteppedIn { team, player, .. } if *team == side => Some(*player),
                    _ => None,
                })
                .unwrap_or(teams[w].first);
            let r = results[w];
            if r[f].survivors.is_empty() {
                return Err(broken(
                    format!("outcome {outcome:?}, but the winning warband has no minions left"),
                    format!("team {w} player {f}"),
                ));
            }
            let fill: &[Unit] = if r[1 - f].fought {
                &[]
            } else {
                &r[1 - f].survivors
            };
            let tiers: u32 = r[f]
                .survivors
                .iter()
                .chain(fill)
                .take(7)
                .map(|u| u.tavern_tier)
                .sum();
            teams[w].warbands[f].hero_tier + tiers
        }
    };
    if res.hero_damage != expected || (res.hero_damage == 0) != (res.outcome == BattleOutcome::Draw)
    {
        return Err(broken(
            format!(
                "Duos hero damage {} for {:?} (expected {expected})",
                res.hero_damage, res.outcome
            ),
            describe_results(res),
        ));
    }
    Ok(())
}

/// Each player's survivors and hand after a Duos combat.
fn describe_results(res: &DuosResult) -> String {
    let mut out = String::new();
    for (team, results) in [("A", &res.team_a), ("B", &res.team_b)] {
        for (p, r) in results.iter().enumerate() {
            let _ = write!(
                out,
                "{team}{p} fought {} survivors {} hand {}; ",
                r.fought,
                units_desc(&r.survivors),
                units_desc(&r.hand)
            );
        }
    }
    out
}

/// The `(card_id, Golden)` multiset of `units`.
fn multiset<'a>(units: impl IntoIterator<Item = &'a Unit>) -> BTreeMap<(u32, bool), usize> {
    let mut out = BTreeMap::new();
    for u in units {
        *out.entry(identity(u)).or_default() += 1;
    }
    out
}

/// Each player's `WarbandResult` comes back to their own Tavern (`docs/duos_tavern.md` §6.2):
/// the team's pool takes the combat's damage if it lost, once, Armor first; each player's
/// Tavern keeps its board, shop, Gold, Tier and turn (Combat-to-Tavern Persistence), takes the
/// team's result and their own warband's hand (its cards lead the hand); `sat_out` is the
/// warbands that didn't fight; and the gifts of the combat arrive once: each player's hand and
/// queue hold their warband's hand, their queue from before and the card gifts their teammate's
/// warband still had to deliver, no more.
fn check_combat_applied(
    before: &DuosTavern,
    after: &DuosTavern,
    results: &[WarbandResult; 2],
    res: &DuosResult,
    side: Side,
) -> Result<(), Violation> {
    let won = matches!(
        (res.outcome, side),
        (BattleOutcome::AWin, Side::A) | (BattleOutcome::BWin, Side::B)
    );
    let lost = !won && res.outcome != BattleOutcome::Draw;
    let damage = res.hero_damage as i32;
    let loss = if lost { damage } else { 0 };
    let (hp_before, hp_after) = (before.health + before.armor, after.health + after.armor);
    if hp_after != hp_before - loss {
        return Err(broken(
            "the team lost other than the combat's damage",
            format!(
                "{hp_before} -> {hp_after} Health + Armor, expected -{loss} ({:?}, side {side:?})",
                res.outcome
            ),
        ));
    }
    if lost && after.armor != before.armor - damage.min(before.armor.max(0)) {
        return Err(broken(
            "the combat damage didn't take the Armor first",
            format!(
                "armor {} -> {}, health {} -> {}, damage {damage}",
                before.armor, after.armor, before.health, after.health
            ),
        ));
    }
    let effects = results.iter().any(|r| {
        r.auras
            .to_teammate
            .iter()
            .any(|g| matches!(g, TeammateGift::Effect { .. }))
    });
    for p in 0..2 {
        let (b, a, r) = (&before.players[p], &after.players[p], &results[p]);
        check_combat_keeps_tavern(b, a)?;
        if (a.last_combat_won, a.last_combat_lost) != (won, lost) {
            return Err(broken(
                "a player's last combat isn't the team's result",
                format!(
                    "player {p}: won {} lost {} for the team's {:?} on side {side:?}",
                    a.last_combat_won, a.last_combat_lost, res.outcome
                ),
            ));
        }
        if after.sat_out[p] == r.fought {
            return Err(broken(
                "sat_out isn't the warbands that didn't fight",
                format!(
                    "player {p}: sat_out {}, fought {}",
                    after.sat_out[p], r.fought
                ),
            ));
        }
        let golden = |hand: &[Unit]| hand.iter().filter(|u| u.is_golden).count();
        let changing = |u: &Unit| u.unplayable || u.lockbox_turns_left > 0;
        if golden(&a.hand) > golden(&r.hand) || r.hand.iter().any(changing) {
            continue; // a Triple formed, or a card became ready: the hand changed on its own
        }
        let leads = a.hand.len() >= r.hand.len()
            && a.hand
                .iter()
                .zip(&r.hand)
                .all(|(x, y)| identity(x) == identity(y));
        if !leads {
            return Err(broken(
                "a player's hand isn't their warband's hand after the combat",
                format!(
                    "player {p}: warband hand {}, Tavern hand {}",
                    units_desc(&r.hand),
                    units_desc(&a.hand)
                ),
            ));
        }
        if effects {
            continue; // an effect gift can give cards
        }
        let gifts = results[1 - p]
            .auras
            .to_teammate
            .iter()
            .filter_map(|g| match g {
                TeammateGift::Card(u) => Some(u),
                _ => None,
            });
        let expected = multiset(r.hand.iter().chain(&before.incoming[p]).chain(gifts));
        let got = multiset(a.hand.iter().chain(&after.incoming[p]));
        if expected != got {
            return Err(broken(
                "a player's cards after a combat aren't their warband's hand and the gifts to them, once",
                format!(
                    "player {p}: warband hand {}, queue {}, gifts {:?}; now hand {}, queue {}",
                    units_desc(&r.hand),
                    units_desc(&before.incoming[p]),
                    results[1 - p].auras.to_teammate,
                    units_desc(&a.hand),
                    units_desc(&after.incoming[p])
                ),
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// One game
// ---------------------------------------------------------------------------------------------

/// What Duos games did, for the fuzzer's own coverage: the players' Passes (free ones, ones that
/// queued, ones that completed a Triple), the Passes by card effects, the cards the queues
/// delivered, the warbands that stepped in for their teammate or sat out, the card gifts a
/// combat left for the Tavern, and the heroes chosen and Hero Powers used.
#[derive(Clone, Copy, Debug, Default)]
struct DuosCoverage {
    passes: u64,
    free_passes: u64,
    queued_passes: u64,
    passed_triples: u64,
    effect_passes: u64,
    queue_deliveries: u64,
    step_ins: u64,
    sat_out: u64,
    combat_card_gifts: u64,
    heroes_chosen: u64,
    hero_powers: u64,
}

impl DuosCoverage {
    /// Count `before`'s player `p`'s `action`.
    fn step(&mut self, before: &DuosTavern, after: &DuosTavern, p: usize, action: TavernAction) {
        let mate = 1 - p;
        let pass = matches!(action, TavernAction::Pass { .. });
        self.heroes_chosen += u64::from(matches!(action, TavernAction::ChooseHero { .. }));
        self.hero_powers += u64::from(matches!(
            action,
            TavernAction::HeroPower { .. } | TavernAction::HeroPower2 { .. }
        ));
        let added = |b: &[Unit], a: &[Unit]| a.len().saturating_sub(b.len()) as u64;
        let goldens = |s: &TavernState| s.hand.iter().filter(|u| u.is_golden).count();
        if pass {
            self.passes += 1;
            self.free_passes += u64::from(before.free_passes(p) > 0);
            self.queued_passes += added(&before.incoming[mate], &after.incoming[mate]);
            self.passed_triples +=
                u64::from(goldens(&after.players[mate]) > goldens(&before.players[mate]));
        }
        let counted = u64::from(
            after
                .passes_this_game
                .saturating_sub(before.passes_this_game),
        );
        self.effect_passes += counted.saturating_sub(u64::from(pass));
        for q in 0..2 {
            self.queue_deliveries += added(&after.incoming[q], &before.incoming[q]);
        }
    }

    /// Count a combat's results for one team (`first`: its first warband).
    fn combat(&mut self, results: &[WarbandResult; 2], first: usize) {
        self.step_ins += u64::from(results[1 - first].fought);
        self.sat_out += results.iter().filter(|r| !r.fought).count() as u64;
        for r in results {
            self.combat_card_gifts += r
                .auras
                .to_teammate
                .iter()
                .filter(|g| matches!(g, TeammateGift::Card(_)))
                .count() as u64;
        }
    }

    fn add(&mut self, other: &DuosCoverage) {
        self.passes += other.passes;
        self.free_passes += other.free_passes;
        self.queued_passes += other.queued_passes;
        self.passed_triples += other.passed_triples;
        self.effect_passes += other.effect_passes;
        self.queue_deliveries += other.queue_deliveries;
        self.step_ins += other.step_ins;
        self.sat_out += other.sat_out;
        self.combat_card_gifts += other.combat_card_gifts;
        self.heroes_chosen += other.heroes_chosen;
        self.hero_powers += other.hero_powers;
    }
}

/// The inputs of the engine call a game stopped at (`DuosGame::stop_at`).
struct DuosTaken {
    call: Call,
    team: DuosTavern,
    opponent: Option<DuosTavern>,
    pool: CardPool,
    rng: Rng,
    combat_seed: Option<u64>,
}

/// Everything needed to replay one failing engine call of a Duos game: its teams just before it.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct DuosSnapshot {
    seed: u64,
    call: u64,
    turn: u32,
    phase: Phase,
    kind: FailureKind,
    signature: String,
    detail: String,
    cards: String,
    /// The engine RNG's state (`Rng::new(rng_state)`).
    rng_state: u64,
    /// The copies of each card the Duos pool is missing.
    pool_taken: BTreeMap<String, u32>,
    action: Option<Act>,
    combat_seed: Option<u64>,
    team_index: usize,
    player: usize,
    team: DuosTavern,
    /// The other team of a combat (or the ghost).
    opponent: Option<DuosTavern>,
}

struct DuosGame {
    seed: u64,
    cfg: DuosConfig,
    catalog: Vec<CardTemplate>,
    pool: CardPool,
    rng: Rng,
    /// The fuzzer's own choices (never the engine's).
    policy: Rng,
    teams: Vec<DuosTavern>,
    alive: Vec<bool>,
    /// The last eliminated team, whose ghost the odd team out fights, and which team it is.
    ghost: Option<DuosTavern>,
    ghost_team: Option<usize>,
    /// What the players know about each other (`docs/tavern.md` §3.1): team `t`'s players are
    /// `2t` and `2t + 1`.
    lobby: Lobby,
    /// The coming turn's pairings of teams (the odd team out alone).
    pairing: Vec<Vec<usize>>,
    call: Call,
    /// Stop just before this engine call, keeping its inputs in `taken`.
    stop_at: Option<u64>,
    taken: Option<DuosTaken>,
    /// The ghost the current combat is against.
    ghost_now: Option<DuosTavern>,
    digests: Vec<u64>,
    lookahead: u64,
    trace: bool,
    /// The cards involved in the current call.
    cards: String,
    /// How many times each card was played from hand (coverage).
    played: BTreeMap<u32, u32>,
    /// What the game did (the fuzzer's own coverage).
    coverage: DuosCoverage,
    /// Go on past the failures of known bugs ([`DUOS_KNOWN_BUGS`]), keeping them in `known`.
    continue_known: bool,
    known: Vec<Failure>,
}

impl DuosGame {
    fn new(seed: u64, settings: &Settings) -> Self {
        let cfg = DuosConfig::random(seed, settings.turns);
        let catalog = catalog_for("duos").expect("the Duos catalog");
        let pool = CardPool::new(catalog.clone());
        let mut teams: Vec<DuosTavern> = (0..cfg.teams)
            .map(|t| {
                let mut team = DuosTavern::new()
                    .with_armor(cfg.armor[t])
                    .with_shop_spells(cfg.shop_spells)
                    .with_trinkets(cfg.trinkets)
                    .with_dark_gifts(cfg.dark_gifts)
                    .with_first_fighter_switch_chance(cfg.first_fighter_switch_chance);
                for (p, s) in team.players.iter_mut().enumerate() {
                    s.auras.deity.kind = cfg.deities[t][p];
                    s.auras.base_max_gold_bonus = cfg.rich;
                }
                let heroes = &cfg.heroes[t];
                if cfg.offer_heroes {
                    for (p, options) in heroes.iter().enumerate() {
                        team.players[p].offer_heroes(options.clone());
                    }
                } else {
                    team = team
                        .with_heroes([heroes[0].first().copied(), heroes[1].first().copied()])
                        .expect("implemented heroes");
                }
                team
            })
            .collect();
        for (t, p, card) in &cfg.gifts {
            teams[*t].players[*p].add_to_hand(card.clone());
        }
        for team in &mut teams {
            team.settle();
        }
        Self {
            seed,
            alive: vec![true; cfg.teams],
            cfg,
            catalog,
            pool,
            rng: Rng::new(seed),
            policy: Rng::new(seed ^ 0xD005_9071_C700_0002),
            lobby: Lobby::new(2 * teams.len(), true),
            teams,
            ghost: None,
            ghost_team: None,
            pairing: Vec::new(),
            call: Call::first(),
            stop_at: None,
            taken: None,
            ghost_now: None,
            digests: Vec::new(),
            lookahead: settings.lookahead,
            trace: false,
            cards: String::new(),
            played: BTreeMap::new(),
            coverage: DuosCoverage::default(),
            continue_known: settings.continue_known,
            known: Vec::new(),
        }
    }

    /// The failure of `v` at the current call.
    fn failure(&self, v: Violation) -> Failure {
        Failure {
            seed: self.seed,
            kind: v.kind,
            signature: format!("{:?}: {}", v.kind, normalize(&v.what)),
            detail: format!("{}: {}", v.what, v.detail),
            call: self.call.clone(),
            cards: self.cards.clone(),
        }
    }

    /// Stop at the first failed check, unless it is a known bug and the game goes on past
    /// those (`continue_known`): then keep it in `known` (once per game) and go on.
    fn tolerate(&mut self, checks: impl IntoIterator<Item = Result<(), Violation>>) -> Flow {
        for check in checks {
            if let Err(v) = check {
                let failure = self.failure(v.clone());
                if !(self.continue_known && duos_known_bug(&failure).is_some()) {
                    return Err(Halt::Failed(v));
                }
                if !self.known.iter().any(|f| f.signature == failure.signature) {
                    self.known.push(failure);
                }
            }
        }
        Ok(())
    }

    /// Start the next engine call; stop here if it is the one to snapshot.
    fn begin(
        &mut self,
        phase: Phase,
        team: usize,
        player: usize,
        opponent: Option<usize>,
        combat_seed: Option<u64>,
    ) -> Flow {
        self.call.index += 1;
        self.call.phase = phase;
        self.call.team = Some(team);
        self.call.player = player;
        self.call.opponent = opponent;
        self.call.action = None;
        self.cards.clear();
        if self.stop_at == Some(self.call.index) {
            let opponent = match opponent {
                Some(o) => Some(self.teams[o].clone()),
                None => self.ghost_now.clone().filter(|_| phase == Phase::Ghost),
            };
            self.taken = Some(DuosTaken {
                call: self.call.clone(),
                team: self.teams[team].clone(),
                opponent,
                pool: self.pool.clone(),
                rng: self.rng.clone(),
                combat_seed,
            });
            return Err(Halt::Snapshot);
        }
        Ok(())
    }

    /// The pool across every team's players, queues and gifts.
    fn check_pool(&self) -> Result<(), Violation> {
        let players: Vec<TavernState> = self
            .teams
            .iter()
            .flat_map(|t| t.players.iter().cloned())
            .collect();
        let queued: Vec<Unit> = self
            .teams
            .iter()
            .flat_map(|t| t.incoming.iter().flatten().cloned())
            .collect();
        check_pool_leaks_with(&players, &queued, &self.pool, &self.catalog)
    }

    /// No minion of no pool (the Naga-only ones, `docs/duos_tavern.md` §8) is in team `t`'s
    /// Taverns or queues in a game none was given to: the pool, the random minions and the
    /// Discovers are the Duos catalog's.
    fn check_catalog(&self, t: usize) -> Result<(), Violation> {
        let given = |u: &Unit| out_of_pool_ids().contains(&u.card_id);
        if self.cfg.gifts.iter().any(|(_, _, u)| given(u)) {
            return Ok(());
        }
        let team = &self.teams[t];
        for (p, s) in team.players.iter().enumerate() {
            let prompts = s.discover_pending.iter().chain(&s.discover_queue).flatten();
            let found = s
                .board
                .iter()
                .chain(&s.hand)
                .chain(&s.shop)
                .chain(prompts)
                .chain(&team.incoming[p])
                .find(|u| given(u));
            if let Some(u) = found {
                return Err(broken(
                    format!("a minion of no pool is in a Duos game: {}", u.name),
                    format!("player {p}: {}", unit_desc(u)),
                ));
            }
        }
        Ok(())
    }

    fn record_digest(&mut self) {
        let mut h = DefaultHasher::new();
        for team in &self.teams {
            format!("{team:?}").hash(&mut h);
        }
        for t in &self.catalog {
            self.pool.remaining_copies(t.card_id).hash(&mut h);
        }
        rng_state(&self.rng).hash(&mut h);
        self.digests.push(h.finish());
    }

    fn print_team(&self, t: usize) {
        let team = &self.teams[t];
        println!(
            "  t{} team {t}: {} hp {} armor, passes {}/{}, first fighter {}",
            self.call.turn,
            team.health,
            team.armor,
            team.passes_this_turn,
            team.passes_this_game,
            team.first_fighter
        );
        for (p, s) in team.players.iter().enumerate() {
            println!(
                "    p{p}: tier {}, {} gold\n      board {}\n      hand {}\n      shop {}\n      queue {}",
                s.tavern_tier,
                s.gold,
                units_desc(&s.board),
                units_desc(&s.hand),
                units_desc(&s.shop),
                units_desc(&team.incoming[p])
            );
        }
    }

    fn play(&mut self) -> Flow {
        for turn in 1..=self.cfg.max_turns {
            self.call.turn = turn;
            self.pair_up()?;
            for t in 0..self.teams.len() {
                if self.alive[t] {
                    self.start_turn(t)?;
                    self.deliver_lobby_gifts()?;
                }
            }
            for t in 0..self.teams.len() {
                if self.alive[t] {
                    self.team_turn(t)?;
                    // A Hero Power got during the turn may have made gifts (Master Nguyen's Mukla).
                    self.deliver_lobby_gifts()?;
                    let checks = [self.check_pool(), check_team_round_trip(&self.teams[t])];
                    self.tolerate(checks)?;
                }
            }
            self.combats()?;
            self.record_digest();
            if self.alive.iter().filter(|&&a| a).count() <= 1 {
                break;
            }
        }
        Ok(())
    }

    /// Team `t`'s start of turn, with the checks of its result (`docs/duos_tavern.md` §4.1).
    fn start_turn(&mut self, t: usize) -> Flow {
        self.begin(Phase::StartTurn, t, 0, None, None)?;
        let before = self.teams[t].clone();
        self.teams[t].start_turn(&mut self.pool, &mut self.rng);
        if self.trace {
            self.print_team(t);
        }
        let team = &self.teams[t];
        let mut checks = Vec::new();
        // The turn's Passes start from 0: only Passes that waited (a combat's) count for it.
        let waited = i64::from(team.passes_this_game) - i64::from(before.passes_this_game);
        if i64::from(team.passes_this_turn) != waited {
            checks.push(Err(broken(
                "the team's Passes this turn aren't the Passes made since the start of the turn",
                format!(
                    "this turn {}, this game {} -> {}",
                    team.passes_this_turn, before.passes_this_game, team.passes_this_game
                ),
            )));
        }
        if team.ended != [false, false] {
            checks.push(Err(broken(
                "a player has ended their turn at the start of the turn",
                format!("{:?}", team.ended),
            )));
        }
        // The first fighter (§4.3): after the first turn, a chance of 0 never switches it, and
        // 1 always does.
        let chance = team.first_fighter_switch_chance;
        if team.players[0].turn > 1
            && ((chance <= 0.0 && team.first_fighter != before.first_fighter)
                || (chance >= 1.0 && team.first_fighter == before.first_fighter))
        {
            checks.push(Err(broken(
                "the first fighter didn't follow its switch chance",
                format!(
                    "chance {chance}: player {} -> {}",
                    before.first_fighter, team.first_fighter
                ),
            )));
        }
        if team.sat_out != [false, false] {
            checks.push(Err(broken(
                "sat_out is still set after the start of the turn",
                format!("{:?}", team.sat_out),
            )));
        }
        // A warband that sat out keeps its unused Start of Combat effects (§6.3).
        for p in (0..2).filter(|&p| before.sat_out[p]) {
            let kept = |e: &&seaglass::model::PlayerEffect| {
                e.duration == seaglass::model::EffectDuration::Turn
                    && cards::is_start_of_combat_effect(e)
            };
            for e in before.players[p].auras.effects.iter().filter(kept) {
                let still = team.players[p]
                    .auras
                    .effects
                    .iter()
                    .any(|f| f.card_id == e.card_id && f.stacks >= e.stacks);
                if !still {
                    checks.push(Err(broken(
                        "a warband that sat out lost its next-combat effect",
                        format!("player {p}: {e:?} -> {:?}", team.players[p].auras.effects),
                    )));
                }
            }
        }
        for (p, s) in team.players.iter().enumerate() {
            let b = &before.players[p];
            if s.is_frozen {
                checks.push(Err(broken("the shop is still frozen after start_turn", "")));
            }
            let expected = upgrade_after_turn_start(b.upgrade_cost, &b.auras, s);
            if s.turn > 1 && s.tavern_tier < 6 && s.upgrade_cost != expected {
                checks.push(Err(broken(
                    "the upgrade cost did not go down by 1 at the start of the turn",
                    format!("player {p}: {} -> {}", b.upgrade_cost, s.upgrade_cost),
                )));
            }
            if !b.is_frozen && !hero_acts_after_the_roll(s) {
                checks.push(check_refilled_shop(
                    s,
                    &self.pool,
                    &self.catalog,
                    "start_turn",
                ));
            }
            checks.push(check_free_passes(s, true));
            checks.push(check_shop_offers(b, s));
            checks.push(check_synced(s));
            checks.push(check_tavern(s));
        }
        checks.extend([
            check_team_health_kept(&before, team),
            check_queues(&before, team),
            check_team(team, true),
            self.check_pool(),
            self.check_catalog(t),
        ]);
        self.tolerate(checks)
    }

    /// Team `t`'s turn: both players take random actions, interleaved at random, until both
    /// have ended their turn (`EndTurn`, at the latest at the action cap).
    fn team_turn(&mut self, t: usize) -> Flow {
        let mut steps = [0usize; 2];
        let mut discovers_in_a_row = [0usize; 2];
        let mut decisions = 0;
        while !self.teams[t].turn_over() {
            decisions += 1;
            if decisions > 4 * (self.cfg.action_cap + 60) {
                return Err(broken("the team's turn doesn't end", "").into());
            }
            let p = match self.teams[t].ended {
                [false, false] => self.policy.below(2),
                [true, false] => 1,
                _ => 0,
            };
            self.begin(Phase::Action, t, p, None, None)?;
            let team = &self.teams[t];
            let valid = team.valid_actions(p);
            let unlisted = check_duos_legality(team, p, &valid)?;
            // The teammate's legality, too (after EndTurn, nothing is).
            check_duos_legality(team, 1 - p, &team.valid_actions(1 - p))?;
            if let Some(&a) = unlisted.first() {
                // Step it first: a crash says more than the disagreement.
                self.step_on_copy(t, p, a, Phase::Unlisted)?;
                let s = &self.teams[t].players[p];
                self.call.action = Some(a);
                self.cards = Game::involved(s, &a);
                return Err(broken(
                    format!(
                        "is_legal accepts a {:?} that valid_actions doesn't list",
                        kind_of(s, &a)
                    ),
                    format!("{a:?}"),
                )
                .into());
            }
            if self.policy.below(1000) < self.lookahead as usize {
                for &a in &valid {
                    self.step_on_copy(t, p, a, Phase::Lookahead)?;
                }
                self.reject_on_copy(t)?;
            }
            let s = &self.teams[t].players[p];
            let action = if s.discover_pending.is_some()
                || !s.hero_options.is_empty()
                || !s.trinket_options.is_empty()
            {
                discovers_in_a_row[p] += 1;
                if discovers_in_a_row[p] > 50 {
                    return Err(broken("more than 50 Discovers in a row", "").into());
                }
                valid[self.policy.below(valid.len())]
            } else if steps[p] >= self.cfg.action_cap {
                TavernAction::EndTurn
            } else {
                discovers_in_a_row[p] = 0;
                let weights = &self.cfg.player_weights[t][p];
                choose(s, &valid, weights, steps[p], &mut self.policy)
            };
            steps[p] += 1;
            self.step(t, p, action, Phase::Action)?;
        }
        Ok(())
    }

    /// Step `action` for team `t`'s player `p`, with the checks of the result.
    fn step(
        &mut self,
        t: usize,
        p: usize,
        action: TavernAction,
        phase: Phase,
    ) -> Result<bool, Halt> {
        self.call.phase = phase;
        self.call.action = Some(action);
        self.cards = Game::involved(&self.teams[t].players[p], &action);
        if self.trace {
            println!(
                "    t{} team {t} p{p} {action:?} {}",
                self.call.turn, self.cards
            );
        }
        let before = self.teams[t].clone();
        let s = &before.players[p];
        let pass = match action {
            TavernAction::Pass { hand_index } => Some(hand_index),
            _ => None,
        };
        let expect = pass
            .is_none()
            .then(|| Expect::new(s, action, &self.pool, &self.catalog));
        let pool_before = pass.map(|_| pool_counts(&self.pool, &self.catalog));
        // The same action on the player's Tavern alone (`check_like_solo`).
        let solo = pass.is_none().then(|| {
            let mut solo = s.clone();
            let (mut pool, mut rng) = (self.pool.clone(), self.rng.clone());
            let _ = solo.step(action, &mut pool, &mut rng);
            solo
        });
        // A chosen hero's Health beyond 30 goes into the team's pool (`check_hero_chosen`).
        let chosen = match action {
            TavernAction::ChooseHero { option_index } => s.hero_options.get(option_index).copied(),
            _ => None,
        };
        if let TavernAction::Play { hand_index, .. } | TavernAction::PlayOnShop { hand_index, .. } =
            action
        {
            if let Some(card) = s.hand.get(hand_index) {
                *self.played.entry(card.card_id).or_default() += 1;
            }
        }
        let ended = match self.teams[t].step(p, action, &mut self.pool, &mut self.rng) {
            Ok(ended) => ended,
            Err(e) => {
                return Err(violation(
                    FailureKind::LegalActionRejected,
                    format!("step rejects a listed {:?}", kind_of(s, &action)),
                    e,
                )
                .into())
            }
        };
        if ended != (action == TavernAction::EndTurn) {
            return Err(broken(format!("step returned {ended} for {action:?}"), "").into());
        }
        let team = &self.teams[t];
        let mut checks = Vec::new();
        if ended && !team.ended[p] {
            checks.push(Err(broken(
                "a player who ended their turn isn't marked as ended",
                "",
            )));
        }
        // `EndTurn` discards the temporary cards (Spellcraft spells, §8.3): only ones the queue
        // delivered in the same step can be left.
        let temporary = |units: &[Unit]| units.iter().filter(|u| cards::is_temporary(u)).count();
        if ended && temporary(&team.players[p].hand) > temporary(&before.incoming[p]) {
            checks.push(Err(broken(
                "a temporary card is still in hand after EndTurn",
                units_desc(&team.players[p].hand),
            )));
        }
        // What the action itself does, on the player's Tavern alone (`check_like_solo` ties it to
        // the team's): the cards the queue delivers after it don't get in the way.
        if let (Some(expect), Some(solo)) = (&expect, &solo) {
            checks.extend(expect.check(solo, &self.pool, &self.catalog));
        }
        if let Some(solo) = &solo {
            checks.push(check_like_solo(solo, &before, team, p));
            checks.push(check_teammate_untouched(
                solo, &before, team, p, action, &self.pool,
            ));
        }
        if let Some(hand_index) = pass {
            checks.push(check_pass(&before, team, p, hand_index));
            let quiet = !observers_act_on_the_pass(&before, p);
            if let Some(counts) = pool_before.as_ref().filter(|_| quiet) {
                let card = &before.players[p].hand[hand_index];
                checks.push(check_pass_keeps_pool(
                    card,
                    counts,
                    &self.pool,
                    &self.catalog,
                ));
            }
        }
        // The Pass gifts the action made on the player's Tavern alone.
        let gift_passes = solo.as_ref().map_or(0, |solo| {
            pass_gifts(solo).saturating_sub(pass_gifts(&before.players[p]))
        });
        checks.extend([
            check_counters(&before, team, pass.is_some(), gift_passes),
            check_queues(&before, team),
            check_team(team, true),
            match chosen {
                Some(id) => check_hero_chosen(&before, team, p, id),
                None => check_team_health_kept(&before, team),
            },
        ]);
        for (s, b) in team.players.iter().zip(&before.players) {
            checks.push(check_shop_offers(b, s));
            checks.push(check_synced(s));
            checks.push(check_tavern(s));
            checks.push(check_free_passes(s, false));
        }
        checks.push(self.check_catalog(t));
        if self.trace {
            checks.push(self.check_pool());
        }
        if phase == Phase::Action {
            self.coverage.step(&before, &self.teams[t], p, action);
        }
        self.tolerate(checks)?;
        Ok(ended)
    }

    /// Step `action` for team `t`'s player `p` on a copy of the game, with the checks of the
    /// result.
    fn step_on_copy(&mut self, t: usize, p: usize, action: TavernAction, phase: Phase) -> Flow {
        let (team, pool, rng) = (self.teams[t].clone(), self.pool.clone(), self.rng.clone());
        let (trace, coverage) = (std::mem::replace(&mut self.trace, false), self.coverage);
        let result = self.step(t, p, action, phase);
        self.trace = trace;
        self.coverage = coverage;
        self.teams[t] = team;
        self.pool = pool;
        self.rng = rng;
        if result.is_ok() {
            self.call.action = None;
        }
        result.map(|_| ())
    }

    /// A few actions `is_legal` rejects, for each of team `t`'s players (every one, after
    /// `EndTurn`): `step` rejects them too, and leaves the game as it was.
    fn reject_on_copy(&mut self, t: usize) -> Flow {
        for p in 0..2 {
            let team = &self.teams[t];
            let illegal: Vec<TavernAction> = candidate_actions(&team.players[p])
                .into_iter()
                .filter(|a| !team.is_legal(p, a))
                .collect();
            for _ in 0..illegal.len().min(2) {
                let a = illegal[self.policy.below(illegal.len())];
                let mut copy = self.teams[t].clone();
                let (mut pool, mut rng) = (self.pool.clone(), self.rng.clone());
                let kind = kind_of(&copy.players[p], &a);
                let before = format!("{copy:?}");
                let result = copy.step(p, a, &mut pool, &mut rng);
                if result.is_ok() {
                    return Err(violation(
                        FailureKind::IllegalActionAccepted,
                        format!("step accepts a {kind:?} that is_legal rejects"),
                        format!("player {p}: {a:?}"),
                    )
                    .into());
                }
                let same_pool = self.catalog.iter().all(|c| {
                    pool.remaining_copies(c.card_id) == self.pool.remaining_copies(c.card_id)
                });
                if format!("{copy:?}") != before
                    || !same_pool
                    || rng_state(&rng) != rng_state(&self.rng)
                {
                    return Err(broken(
                        format!("a rejected {kind:?} changed the game"),
                        format!("player {p}: {a:?}"),
                    )
                    .into());
                }
            }
        }
        Ok(())
    }

    /// Every player's Tavern, player `2t + i` being team `t`'s player `i`.
    fn player_states(&mut self) -> Vec<&mut TavernState> {
        self.teams
            .iter_mut()
            .flat_map(|team| team.players.iter_mut())
            .collect()
    }

    /// Pair the teams still in the game for the coming combats, and give every player their view
    /// of the lobby (`docs/tavern.md` §3.1): their next opponent is the opposing team's player in
    /// their seat.
    fn pair_up(&mut self) -> Flow {
        let mut order: Vec<usize> = (0..self.teams.len()).filter(|&t| self.alive[t]).collect();
        for i in (1..order.len()).rev() {
            let j = self.policy.below(i + 1);
            order.swap(i, j);
        }
        self.pairing = order.chunks(2).map(<[usize]>::to_vec).collect();
        let pairs: Vec<(usize, usize)> = self
            .pairing
            .iter()
            .filter_map(|pair| match **pair {
                [a, b] => Some((a, b)),
                _ => None,
            })
            .flat_map(|(a, b)| [(2 * a, 2 * b), (2 * a + 1, 2 * b + 1)])
            .collect();
        self.lobby.set_next_pairs(&pairs);
        let lobby = self.lobby.clone();
        lobby.apply(&mut self.player_states());
        let checks: Vec<_> = self
            .teams
            .iter()
            .flat_map(|team| team.players.iter().map(check_lobby))
            .collect();
        self.tolerate(checks)
    }

    /// Deliver the cards the players' Hero Powers made for the other teams' players
    /// (`docs/tavern.md` §3.1).
    fn deliver_lobby_gifts(&mut self) -> Flow {
        let lobby = self.lobby.clone();
        lobby.deliver_gifts(&mut self.player_states());
        let checks: Vec<_> = self
            .teams
            .iter()
            .flat_map(|team| team.players.iter().map(check_lobby))
            .collect();
        self.tolerate(checks)
    }

    /// Record a team's combat in the lobby: each player's board as it entered it, and their
    /// opponent in the same seat of `opponent` (a team), if any.
    fn record_lobby_combat(&mut self, t: usize, before: &DuosTavern, opponent: Option<usize>) {
        for (i, player) in before.players.iter().enumerate() {
            let board = &player.board;
            self.lobby
                .record_combat(2 * t + i, opponent.map(|o| 2 * o + i), board);
        }
    }

    /// Resolve the coming combats (`pair_up`'s pairings) and record them in the lobby.
    fn combats(&mut self) -> Flow {
        let pairing = std::mem::take(&mut self.pairing);
        for pair in &pairing {
            if let [a, b] = **pair {
                self.combat_pair(a, b)?;
            } else {
                self.combat_ghost(pair[0])?;
            }
            let leaks = self.check_pool();
            self.tolerate([leaks])?;
        }
        for t in 0..self.teams.len() {
            if self.alive[t] && self.teams[t].is_out() {
                self.alive[t] = false;
                self.ghost = Some(self.teams[t].clone());
                self.ghost_team = Some(t);
            }
        }
        let alive: Vec<bool> = self.alive.iter().flat_map(|&a| [a, a]).collect();
        self.lobby.record_deaths(&alive);
        Ok(())
    }

    /// The checks of a team after its combat.
    fn after_combat(&mut self, t: usize) -> Flow {
        let team = &self.teams[t];
        let mut checks = vec![
            check_team(team, false),
            check_team_round_trip(team),
            self.check_catalog(t),
        ];
        for s in &team.players {
            checks.push(check_synced(s));
            checks.push(check_tavern(s));
        }
        self.tolerate(checks)
    }

    fn combat_pair(&mut self, a: usize, b: usize) -> Flow {
        let seed = self.rng.next_u64();
        self.begin(Phase::Combat, a, 0, Some(b), Some(seed))?;
        let (before_a, before_b) = (self.teams[a].clone(), self.teams[b].clone());
        let (fight_a, fight_b) = (fighting_team(&before_a), fighting_team(&before_b));
        let (mut team_a, mut team_b) = (before_a.clone(), before_b.clone());
        let res = DuosTavern::resolve_combat_pair(&mut team_a, &mut team_b, seed);
        self.teams[a] = team_a;
        self.teams[b] = team_b;
        self.record_lobby_combat(a, &before_a, Some(b));
        self.record_lobby_combat(b, &before_b, Some(a));
        self.lobby.record_outcome(2 * a, 2 * b, res.outcome);
        if self.trace {
            println!(
                "  combat team {a} v team {b}: {:?}, {} damage; {}",
                res.outcome,
                res.hero_damage,
                describe_results(&res)
            );
        }
        check_duos_battle(&res, &fight_a, &fight_b)?;
        self.coverage.combat(&res.team_a, fight_a.first);
        self.coverage.combat(&res.team_b, fight_b.first);
        let checks = [
            check_combat_applied(&before_a, &self.teams[a], &res.team_a, &res, Side::A),
            check_combat_applied(&before_b, &self.teams[b], &res.team_b, &res, Side::B),
        ];
        self.tolerate(checks)?;
        self.after_combat(a)?;
        self.after_combat(b)
    }

    /// The odd team out fights the last eliminated team's ghost (or, before any team is out, a
    /// copy of a random other team).
    fn combat_ghost(&mut self, t: usize) -> Flow {
        let others: Vec<usize> = (0..self.teams.len())
            .filter(|&o| o != t && self.alive[o])
            .collect();
        let (opponent, source) = match &self.ghost {
            Some(g) => (g.clone(), self.ghost_team),
            None if !others.is_empty() => {
                let o = others[self.policy.below(others.len())];
                (self.teams[o].clone(), Some(o))
            }
            None => return Ok(()),
        };
        self.ghost_now = Some(opponent.clone());
        let seed = self.rng.next_u64();
        self.begin(Phase::Ghost, t, 0, None, Some(seed))?;
        let before = self.teams[t].clone();
        self.record_lobby_combat(t, &before, source);
        let (fight, ghost) = (fighting_team(&before), fighting_team(&opponent));
        let res = self.teams[t].resolve_combat_against(&ghost, seed);
        if self.trace {
            println!(
                "  combat team {t} v ghost: {:?}, {} damage; {}",
                res.outcome,
                res.hero_damage,
                describe_results(&res)
            );
        }
        check_duos_battle(&res, &fight, &ghost)?;
        self.coverage.combat(&res.team_a, fight.first);
        let checks = [check_combat_applied(
            &before,
            &self.teams[t],
            &res.team_a,
            &res,
            Side::A,
        )];
        self.tolerate(checks)?;
        self.after_combat(t)
    }
}

/// Play the Duos game of `seed` once, catching its panics.
fn run_duos_game(seed: u64, settings: &Settings, trace: bool) -> (Run, DuosCoverage) {
    install_panic_hook();
    let mut game = DuosGame::new(seed, settings);
    game.trace = trace;
    if trace {
        println!("Duos game {seed}: {:?}", game.cfg);
    }
    CAPTURING.with(|c| *c.borrow_mut() = true);
    let result = panic::catch_unwind(AssertUnwindSafe(|| game.play()));
    CAPTURING.with(|c| *c.borrow_mut() = false);
    let failure = match result {
        Ok(Ok(())) | Ok(Err(Halt::Snapshot)) => None,
        Ok(Err(Halt::Failed(v))) => Some(game.failure(v)),
        Err(_) => {
            let message = LAST_PANIC
                .with(|p| p.borrow_mut().take())
                .unwrap_or_else(|| "<panic>".to_string());
            let first_line = message.lines().next().unwrap_or_default().to_string();
            Some(Failure {
                seed,
                kind: FailureKind::Crash,
                signature: format!("Crash: {}", normalize(&first_line)),
                detail: message,
                call: game.call.clone(),
                cards: game.cards.clone(),
            })
        }
    };
    let run = Run {
        digests: game.digests,
        failure,
        known: game.known,
        played: game.played,
    };
    (run, game.coverage)
}

/// Play the Duos game of `seed` twice: the failure of the first run, or a nondeterminism if the
/// replay differs.
fn fuzz_duos_game(seed: u64, settings: &Settings) -> (Vec<Failure>, BTreeMap<u32, u32>) {
    let (first, _) = run_duos_game(seed, settings, false);
    let played = first.played.clone();
    let mut failures = first.known.clone();
    let (replay, _) = run_duos_game(seed, settings, false);
    failures.extend(compare_replay(seed, first, replay));
    (failures, played)
}

/// Replay the Duos game of `failure` up to its failing call and snapshot the teams just before
/// it.
fn duos_snapshot(failure: &Failure, settings: &Settings) -> Option<DuosSnapshot> {
    let mut game = DuosGame::new(failure.seed, settings);
    game.stop_at = Some(failure.call.index);
    CAPTURING.with(|c| *c.borrow_mut() = true);
    let _ = panic::catch_unwind(AssertUnwindSafe(|| game.play()));
    CAPTURING.with(|c| *c.borrow_mut() = false);
    let DuosTaken {
        call,
        team,
        opponent,
        pool,
        rng,
        combat_seed,
    } = game.taken.take()?;
    Some(DuosSnapshot {
        seed: failure.seed,
        call: call.index,
        turn: call.turn,
        phase: failure.call.phase,
        kind: failure.kind,
        signature: failure.signature.clone(),
        detail: failure.detail.clone(),
        cards: failure.cards.clone(),
        rng_state: rng_state(&rng),
        pool_taken: pool_taken(&pool, &game.catalog),
        action: failure.call.action.map(Act::from),
        combat_seed,
        team_index: call.team.unwrap_or(0),
        player: call.player,
        team,
        opponent,
    })
}

/// Write each group's repro snapshot to `dir`.
fn write_duos_snapshots(groups: &Groups, settings: &Settings, dir: &str) {
    std::fs::create_dir_all(dir).expect("create the output directory");
    for (i, (_, f)) in groups.groups.values().enumerate() {
        if let Some(snap) = duos_snapshot(f, settings) {
            let mut h = DefaultHasher::new();
            f.signature.hash(&mut h);
            let path = format!("{dir}/{:02}-{:016x}-seed{}.yaml", i, h.finish(), f.seed);
            std::fs::write(&path, yaml(&snap)).expect("write a snapshot");
        }
    }
}

/// The Duos catalog's minions, the Tavern spells and the Duos spells, for the coverage report.
fn duos_coverage() -> Vec<(u32, String)> {
    catalog_for("duos")
        .expect("the Duos catalog")
        .iter()
        .map(|t| (t.card_id, t.name.clone()))
        .chain(
            cards::spells::all_spells()
                .iter()
                .chain(&cards::spells::all_duos_spells())
                .map(|u| (u.card_id, u.name.clone())),
        )
        .collect()
}

fn duos_settings() -> Settings {
    let mut settings = Settings::from_env();
    settings.games = env_or("SEAGLASS_FUZZ_DUOS_GAMES", 4);
    settings
}

// ---------------------------------------------------------------------------------------------
// Random Duos battles
// ---------------------------------------------------------------------------------------------

/// A random Duos battle's inputs: four warbands of the Duos minions and tokens (some Golden,
/// buffed, with extra keywords; some empty), hands, player auras (in a Duos game) and Tiers, and
/// each team's first warband.
#[derive(Clone, Debug)]
struct DuosBattleCase {
    seed: u64,
    a: DuosTeam,
    b: DuosTeam,
}

impl DuosBattleCase {
    fn random(seed: u64) -> Self {
        let mut rng = Rng::new(seed ^ 0xD005_BA77_1E00_0002);
        let minions: Vec<CardTemplate> = cards::duos_catalog()
            .into_iter()
            .chain(cards::duos_out_of_pool_catalog())
            .collect();
        let tokens = cards::tokens::all_token_minions();
        let spells: Vec<Unit> = cards::spells::all_spells()
            .into_iter()
            .chain(cards::spells::all_duos_spells())
            .collect();
        let combat_spells: Vec<u32> = spells
            .iter()
            .map(|u| u.card_id)
            .filter(|&id| {
                let h = cards::hooks(id);
                h.player_start_of_combat.is_some()
                    || h.player_combat_space.is_some()
                    || h.player_combat_summon.is_some()
            })
            .collect();
        let warband = |rng: &mut Rng| -> Warband {
            let board = (0..rng.below(8))
                .map(|_| random_minion(rng, &minions, &tokens))
                .collect();
            let hand = (0..rng.below(4) + 6 * usize::from(rng.below(8) == 0))
                .map(|_| {
                    if rng.below(2) == 0 {
                        random_minion(rng, &minions, &tokens)
                    } else {
                        spells[rng.below(spells.len())].clone()
                    }
                })
                .collect();
            let mut auras = random_auras(rng, &combat_spells);
            auras.duos_game = true;
            // Half the warbands have a hero, whose combat Hero Power (if it has one) acts at its
            // warband's Start of Combat, also when it steps in (`docs/heroes.md` §7).
            if rng.below(2) == 0 {
                let ids = heroes::offerable(true, false);
                let hero = heroes::data(ids[rng.below(ids.len())]).expect("an offerable hero");
                auras.hero = Some(heroes::new_state(hero));
            }
            Warband {
                board,
                hand,
                auras,
                hero_tier: 1 + rng.below(6) as u32,
            }
        };
        let team = |rng: &mut Rng| DuosTeam {
            warbands: [warband(rng), warband(rng)],
            first: rng.below(2),
        };
        let (a, b) = (team(&mut rng), team(&mut rng));
        Self {
            seed: rng.next_u64(),
            a,
            b,
        }
    }

    fn describe(&self) -> String {
        let team = |t: &DuosTeam| {
            let w = |w: &Warband| {
                format!(
                    "{} (hand {}, Tier {})",
                    units_desc(&w.board),
                    units_desc(&w.hand),
                    w.hero_tier
                )
            };
            format!(
                "[{}; {}] first {}",
                w(&t.warbands[0]),
                w(&t.warbands[1]),
                t.first
            )
        };
        format!(
            "seed {}: A {} v B {}",
            self.seed,
            team(&self.a),
            team(&self.b)
        )
    }
}

/// Play the Duos battle of `case` twice, with the checks of the result (its outcome).
fn fuzz_duos_battle(case: &DuosBattleCase) -> Result<BattleOutcome, Violation> {
    let res = seaglass::resolve_duos_battle(&case.a, &case.b, case.seed);
    check_duos_battle(&res, &case.a, &case.b)?;
    for (team, results) in [("A", &res.team_a), ("B", &res.team_b)] {
        for (p, r) in results.iter().enumerate() {
            if r.hand.len() > HAND_SIZE {
                return Err(broken(
                    format!(
                        "a warband's hand has {} cards after the battle",
                        r.hand.len()
                    ),
                    format!("{team}{p}: {}", units_desc(&r.hand)),
                ));
            }
        }
    }
    let again = seaglass::resolve_duos_battle(&case.a, &case.b, case.seed);
    if again != res {
        return Err(violation(
            FailureKind::Nondeterminism,
            "a Duos battle resolves differently twice",
            "",
        ));
    }
    Ok(res.outcome)
}

/// `SEAGLASS_FUZZ_DUOS_BATTLES` random Duos battles (default 300, from `SEAGLASS_FUZZ_SEED`)
/// keep the combat invariants of the games ([`check_duos_battle`]).
#[test]
fn random_duos_battles_keep_the_invariants() {
    install_panic_hook();
    let battles: u64 = env_or("SEAGLASS_FUZZ_DUOS_BATTLES", 300);
    let first_seed: u64 = env_or("SEAGLASS_FUZZ_SEED", 1);
    let threads: usize = env_or("SEAGLASS_FUZZ_THREADS", default_threads()).max(1);
    let strict = std::env::var("SEAGLASS_FUZZ_STRICT").is_ok_and(|v| v == "1");
    let next = AtomicUsize::new(0);
    let groups: Mutex<BTreeMap<String, (usize, String)>> = Mutex::new(BTreeMap::new());
    let outcomes: Mutex<[usize; 3]> = Mutex::new([0; 3]);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed) as u64;
                if i >= battles {
                    break;
                }
                let case = DuosBattleCase::random(first_seed + i);
                CAPTURING.with(|c| *c.borrow_mut() = true);
                let result = panic::catch_unwind(AssertUnwindSafe(|| fuzz_duos_battle(&case)));
                CAPTURING.with(|c| *c.borrow_mut() = false);
                let failure = match result {
                    Ok(Ok(outcome)) => {
                        outcomes.lock().unwrap()[outcome as usize] += 1;
                        None
                    }
                    Ok(Err(v)) => Some((
                        format!("{:?}: {}", v.kind, normalize(&v.what)),
                        format!("{}: {}", v.what, v.detail),
                    )),
                    Err(_) => {
                        let message = LAST_PANIC
                            .with(|p| p.borrow_mut().take())
                            .unwrap_or_default();
                        let first = message.lines().next().unwrap_or_default().to_string();
                        Some((format!("Crash: {}", normalize(&first)), message))
                    }
                };
                if let Some((signature, detail)) = failure {
                    let mut g = groups.lock().unwrap();
                    let entry = g
                        .entry(signature)
                        .or_insert_with(|| (0, format!("{detail}\n  {}", case.describe())));
                    entry.0 += 1;
                }
            });
        }
    });
    let groups = groups.into_inner().unwrap();
    let [a, b, draws] = outcomes.into_inner().unwrap();
    let mut text = format!(
        "{battles} Duos battles from seed {first_seed} ({a} A wins, {b} B wins, {draws} draws): {} groups\n",
        groups.len()
    );
    for (signature, (count, first)) in &groups {
        let detail: String = first.chars().take(4000).collect();
        let _ = writeln!(text, "\n{count} x {signature}\n  {detail}");
    }
    println!("{text}");
    let unknown: Vec<&String> = groups
        .keys()
        .filter(|sig| strict || !DUOS_KNOWN_BUGS.iter().any(|(p, _)| sig.contains(p)))
        .collect();
    assert!(
        unknown.is_empty(),
        "new Duos battle failures: {unknown:#?}\n{text}"
    );
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[test]
fn random_duos_games_keep_the_invariants() {
    let single = [
        "SEAGLASS_FUZZ_GAME",
        "SEAGLASS_FUZZ_REPRO",
        "SEAGLASS_FUZZ_DUOS_GAME",
        "SEAGLASS_FUZZ_DUOS_REPRO",
    ];
    if single.iter().any(|v| std::env::var(v).is_ok()) {
        return;
    }
    let settings = duos_settings();
    let groups = run_games(&settings, fuzz_duos_game);
    let text = report_with(&groups, &settings, &duos_coverage(), duos_known_bug);
    println!("Duos: {text}");
    if let Some(dir) = &settings.out {
        write_duos_snapshots(&groups, &settings, &format!("{dir}/duos"));
    }
    let unknown: Vec<&String> = groups
        .groups
        .iter()
        .filter(|(_, (_, f))| settings.strict || duos_known_bug(f).is_none())
        .map(|(sig, _)| sig)
        .collect();
    assert!(
        unknown.is_empty(),
        "new Duos failures: {unknown:#?}\n{text}"
    );
}

/// `SEAGLASS_FUZZ_DUOS_GAME=<seed>`: play one Duos game, printing its trace and failure.
#[test]
fn trace_one_duos_game() {
    let Some(seed) = std::env::var("SEAGLASS_FUZZ_DUOS_GAME")
        .ok()
        .and_then(|v| v.parse().ok())
    else {
        return;
    };
    let settings = duos_settings();
    let (run, coverage) = run_duos_game(seed, &settings, true);
    println!("{coverage:?}");
    for f in &run.known {
        println!(
            "KNOWN: {}\n  {}\n  {}",
            f.signature,
            describe_call(&f.call),
            f.detail
        );
    }
    match run.failure {
        Some(f) => {
            println!(
                "FAILED: {}\n  {}\n  {}",
                f.signature,
                describe_call(&f.call),
                f.detail
            );
            if let Some(snap) = duos_snapshot(&f, &settings) {
                println!("{}", yaml(&snap));
            }
        }
        None => println!("no failure"),
    }
}

/// A Duos combat's outcome, its checks and its events, one per line.
fn describe_duos_battle(res: &DuosResult) -> String {
    let mut out = format!(
        "{:?}, {} damage; {}\n",
        res.outcome,
        res.hero_damage,
        describe_results(res)
    );
    for (i, e) in res.events.iter().enumerate() {
        let _ = writeln!(out, "  {i:3} {e:?}");
    }
    out
}

/// `SEAGLASS_FUZZ_DUOS_REPRO=<file>`: replay a Duos snapshot's failing call and print the
/// outcome.
#[test]
fn replay_a_duos_snapshot() {
    let Ok(path) = std::env::var("SEAGLASS_FUZZ_DUOS_REPRO") else {
        return;
    };
    let text = std::fs::read_to_string(&path).expect("read the snapshot");
    let snap: DuosSnapshot = serde_yaml::from_str(&text).expect("parse the snapshot");
    let catalog = catalog_for("duos").expect("the Duos catalog");
    let mut pool = CardPool::new(catalog.clone());
    for t in &catalog {
        for _ in 0..snap.pool_taken.get(&t.name).copied().unwrap_or(0) {
            pool.take_copy(t.card_id);
        }
    }
    let mut rng = Rng::new(snap.rng_state);
    let mut team = snap.team.clone();
    let p = snap.player;
    println!("{}: {}", snap.signature, snap.detail);
    install_panic_hook();
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| match (snap.phase, snap.action) {
        (Phase::StartTurn, _) => {
            let before = team.clone();
            team.start_turn(&mut pool, &mut rng);
            format!(
                "team {:?}; queues {:?}; players {:?} / {:?}",
                check_team(&team, true),
                check_queues(&before, &team),
                check_tavern(&team.players[0]),
                check_tavern(&team.players[1])
            )
        }
        (Phase::Combat, _) => {
            let mut other = snap.opponent.clone().expect("a combat has an opponent");
            let (fight_a, fight_b) = (fighting_team(&team), fighting_team(&other));
            let res = DuosTavern::resolve_combat_pair(
                &mut team,
                &mut other,
                snap.combat_seed.expect("a combat seed"),
            );
            format!(
                "battle {:?}\n{}",
                check_duos_battle(&res, &fight_a, &fight_b),
                describe_duos_battle(&res)
            )
        }
        (Phase::Ghost, _) => {
            let other = snap.opponent.clone().expect("a ghost");
            let (fight, ghost) = (fighting_team(&team), fighting_team(&other));
            let res = team.resolve_combat_against(&ghost, snap.combat_seed.expect("a combat seed"));
            format!(
                "battle {:?}\n{}",
                check_duos_battle(&res, &fight, &ghost),
                describe_duos_battle(&res)
            )
        }
        (_, Some(act)) => {
            let action = TavernAction::from(act);
            let before = team.clone();
            let valid = team.valid_actions(p);
            let legality = check_duos_legality(&team, p, &valid).map(|unlisted| unlisted.len());
            // The same action on the player's Tavern alone, for the metamorphic checks.
            let mut solo = team.players[p].clone();
            let _ = solo.step(action, &mut pool.clone(), &mut rng.clone());
            let result = team.step(p, action, &mut pool, &mut rng);
            let pass = match action {
                TavernAction::Pass { hand_index } => {
                    format!("{:?}", check_pass(&before, &team, p, hand_index))
                }
                _ => format!(
                    "none; like solo {:?}; teammate {:?}",
                    check_like_solo(&solo, &before, &team, p),
                    check_teammate_untouched(&solo, &before, &team, p, action, &pool)
                ),
            };
            format!(
                "legality {legality:?}; step {result:?}; pass {pass}; counters {:?}; queues {:?}; \
                 team {:?}; players {:?} / {:?}",
                check_counters(
                    &before,
                    &team,
                    matches!(action, TavernAction::Pass { .. }),
                    pass_gifts(&solo).saturating_sub(pass_gifts(&before.players[p]))
                ),
                check_queues(&before, &team),
                check_team(&team, true),
                check_tavern(&team.players[0]),
                check_tavern(&team.players[1])
            )
        }
        (_, None) => {
            let valid = team.valid_actions(p);
            format!(
                "{:?}",
                check_duos_legality(&team, p, &valid).map(|u| u.len())
            )
        }
    }));
    match outcome {
        Ok(text) => println!("{text}"),
        Err(_) => println!("panicked"),
    }
    for (q, s) in team.players.iter().enumerate() {
        println!(
            "p{q}: board {}\n    hand {}\n    shop {}\n    queue {}",
            units_desc(&s.board),
            units_desc(&s.hand),
            units_desc(&s.shop),
            units_desc(&team.incoming[q])
        );
    }
}

#[test]
fn the_duos_policy_passes_and_both_players_act() {
    // A check of the fuzzer itself: over a few games (`SEAGLASS_FUZZ_DUOS_COVERAGE` of them, 4
    // by default), the players Pass, for free too, to full hands, and warbands step in for
    // their teammate and sit out.
    let games: u64 = env_or("SEAGLASS_FUZZ_DUOS_COVERAGE", 4);
    let settings = Settings {
        lookahead: 0,
        ..duos_settings()
    };
    let mut total = DuosCoverage::default();
    for seed in settings.first_seed..settings.first_seed + games {
        total.add(&run_duos_game(seed, &settings, false).1);
    }
    println!("{games} Duos games: {total:?}");
    assert!(total.passes > 0, "the Duos policy never Passed: {total:?}");
    assert!(total.step_ins > 0, "no warband stepped in: {total:?}");
    assert!(total.sat_out > 0, "no warband sat out: {total:?}");
}
