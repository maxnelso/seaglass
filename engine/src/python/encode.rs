//! The RL observation encoder and legal-action mask in Rust (`rl/seaglass_rl/features.py`,
//! `actions.py`): a speed-up, bit-identical to the Python reference.
//!
//! The repo's rule is that game logic never hardcodes a neural-network feature, so this lives only
//! in the Python-bindings layer, and Python stays the owner of the layout: the vocabularies, the
//! action table and every offset and size come in through `Encoder(spec)`. This file holds the
//! formulas, ported line by line from `features.py`, over the same observation structs and `Unit`
//! getters Python reads. Each feature is computed in `f64` with the same operations in the same
//! order, then cast to `f32` once, as storing a Python float into a float32 array does.

use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyByteArray, PyDict};

use super::lobby::{PyLobbyContext, PyLobbyGame};
use super::{PyDuosObservation, PyTavernAction, PyTavernObservation, PyUnit};

/// `features.symlog`: `sign(x) * log1p(|x|) / 5`.
fn symlog(x: i64) -> f64 {
    ((x.abs() as f64).ln_1p()).copysign(x as f64) / 5.0
}

/// `math.log1p(max(0, x)) / 5.0`.
fn log1p_pos(x: i64) -> f64 {
    ((x.max(0) as f64).ln_1p()) / 5.0
}

fn flag(b: bool) -> f64 {
    if b {
        1.0
    } else {
        0.0
    }
}

/// Every offset and size of the layout (`features.py`), passed in from Python.
#[derive(Debug, Default)]
struct Layout {
    max_board: usize,
    max_hand: usize,
    max_shop: usize,
    max_discover: usize,
    max_hero_options: usize,
    max_trinket_options: usize,
    max_waiting: usize,
    max_trinkets: usize,
    hero_offset: usize,
    teammate_offset: usize,
    opponent_board_offset: usize,
    teammate_shop_offset: usize,
    opponent_hero_offset: usize,
    card_counter_offset: usize,
    effect_offset: usize,
    max_opponents_slots: usize,
    max_card_counters: usize,
    max_effects: usize,
    num_slots: usize,
    tavern_player_dim: usize,
    lobby_offset: usize,
    lobby_dim: usize,
    opponents_offset: usize,
    max_opponents: usize,
    opponent_dim: usize,
    duos_offset: usize,
    duos_dim: usize,
    phase_offset: usize,
    counters_offset: usize,
    counters_dim: usize,
    player_dim: usize,
    base_slot_num_dim: usize,
    num_trinket_tribes: usize,
    trinket_tribe_offset: usize,
    end_of_turn_stats_offset: usize,
    tribes_offset: usize,
    counter_offset: usize,
    unit_extra_offset: usize,
    unit_extra_dim: usize,
    slot_num_dim: usize,
    action_space_size: usize,
}

fn bad(msg: String) -> PyErr {
    PyValueError::new_err(format!("Encoder spec: {msg}"))
}

fn expect(what: &str, got: usize, want: usize) -> PyResult<()> {
    if got == want {
        Ok(())
    } else {
        Err(bad(format!("{what} is {got}, expected {want}")))
    }
}

impl Layout {
    fn from_map(m: &HashMap<String, usize>) -> PyResult<Self> {
        let g = |k: &str| {
            m.get(k)
                .copied()
                .ok_or_else(|| bad(format!("layout has no {k}")))
        };
        Ok(Self {
            max_board: g("MAX_BOARD")?,
            max_hand: g("MAX_HAND")?,
            max_shop: g("MAX_SHOP")?,
            max_discover: g("MAX_DISCOVER")?,
            max_hero_options: g("MAX_HERO_OPTIONS")?,
            max_trinket_options: g("MAX_TRINKET_OPTIONS")?,
            max_waiting: g("MAX_WAITING")?,
            max_trinkets: g("MAX_TRINKETS")?,
            hero_offset: g("HERO_OFFSET")?,
            teammate_offset: g("TEAMMATE_OFFSET")?,
            opponent_board_offset: g("OPPONENT_BOARD_OFFSET")?,
            teammate_shop_offset: g("TEAMMATE_SHOP_OFFSET")?,
            opponent_hero_offset: g("OPPONENT_HERO_OFFSET")?,
            card_counter_offset: g("CARD_COUNTER_OFFSET")?,
            effect_offset: g("EFFECT_OFFSET")?,
            max_opponents_slots: g("MAX_OPPONENTS_SLOTS")?,
            max_card_counters: g("MAX_CARD_COUNTERS")?,
            max_effects: g("MAX_EFFECTS")?,
            num_slots: g("NUM_SLOTS")?,
            tavern_player_dim: g("TAVERN_PLAYER_DIM")?,
            lobby_offset: g("LOBBY_OFFSET")?,
            lobby_dim: g("LOBBY_DIM")?,
            opponents_offset: g("OPPONENTS_OFFSET")?,
            max_opponents: g("MAX_OPPONENTS")?,
            opponent_dim: g("OPPONENT_DIM")?,
            duos_offset: g("DUOS_OFFSET")?,
            duos_dim: g("DUOS_DIM")?,
            phase_offset: g("PHASE_OFFSET")?,
            counters_offset: g("COUNTERS_OFFSET")?,
            counters_dim: g("COUNTERS_DIM")?,
            player_dim: g("PLAYER_DIM")?,
            base_slot_num_dim: g("BASE_SLOT_NUM_DIM")?,
            num_trinket_tribes: g("NUM_TRINKET_TRIBES")?,
            trinket_tribe_offset: g("TRINKET_TRIBE_OFFSET")?,
            end_of_turn_stats_offset: g("END_OF_TURN_STATS_OFFSET")?,
            tribes_offset: g("TRIBES_OFFSET")?,
            counter_offset: g("COUNTER_OFFSET")?,
            unit_extra_offset: g("UNIT_EXTRA_OFFSET")?,
            unit_extra_dim: g("UNIT_EXTRA_DIM")?,
            slot_num_dim: g("SLOT_NUM_DIM")?,
            action_space_size: g("ACTION_SPACE_SIZE")?,
        })
    }

    /// The block sizes must add up the way `features.py`'s comments say, and the formulas' fixed
    /// shapes (33 Tavern features with 4 trinket options, 10 per opponent, 11 Duos, 15 extras)
    /// must fit.
    fn validate(&self, n_gifts: usize, n_single: usize, n_lobby: usize) -> PyResult<()> {
        let l = self;
        let tavern_slots = l.max_board
            + l.max_hand
            + l.max_shop
            + l.max_discover
            + l.max_waiting
            + l.max_trinkets
            + l.max_trinket_options;
        expect("HERO_OFFSET", l.hero_offset, tavern_slots)?;
        expect(
            "TEAMMATE_OFFSET",
            l.teammate_offset,
            l.hero_offset + 1 + l.max_hero_options,
        )?;
        expect(
            "OPPONENT_BOARD_OFFSET",
            l.opponent_board_offset,
            l.teammate_offset + l.max_board + l.max_hand,
        )?;
        expect(
            "TEAMMATE_SHOP_OFFSET",
            l.teammate_shop_offset,
            l.opponent_board_offset + 2 * l.max_board,
        )?;
        expect(
            "OPPONENT_HERO_OFFSET",
            l.opponent_hero_offset,
            l.teammate_shop_offset + l.max_shop,
        )?;
        expect(
            "CARD_COUNTER_OFFSET",
            l.card_counter_offset,
            l.opponent_hero_offset + l.max_opponents_slots,
        )?;
        expect(
            "EFFECT_OFFSET",
            l.effect_offset,
            l.card_counter_offset + l.max_card_counters,
        )?;
        expect("NUM_SLOTS", l.num_slots, l.effect_offset + l.max_effects)?;
        // The Tavern block: 29 fixed features plus one price per trinket option.
        expect(
            "TAVERN_PLAYER_DIM",
            l.tavern_player_dim,
            29 + l.max_trinket_options,
        )?;
        expect("LOBBY_OFFSET", l.lobby_offset, l.tavern_player_dim)?;
        expect("LOBBY_DIM", l.lobby_dim, 10 + n_lobby)?;
        expect(
            "OPPONENTS_OFFSET",
            l.opponents_offset,
            l.lobby_offset + l.lobby_dim,
        )?;
        expect("OPPONENT_DIM", l.opponent_dim, 10)?;
        expect(
            "DUOS_OFFSET",
            l.duos_offset,
            l.opponents_offset + l.max_opponents * l.opponent_dim,
        )?;
        expect("DUOS_DIM", l.duos_dim, 11)?;
        expect("PHASE_OFFSET", l.phase_offset, l.duos_offset + l.duos_dim)?;
        expect("COUNTERS_OFFSET", l.counters_offset, l.phase_offset + 1)?;
        expect(
            "PLAYER_DIM",
            l.player_dim,
            l.counters_offset + l.counters_dim,
        )?;
        if l.counters_dim < 4 {
            return Err(bad("COUNTERS_DIM is below 4".into()));
        }
        expect("BASE_SLOT_NUM_DIM", l.base_slot_num_dim, 14)?;
        expect(
            "TRINKET_TRIBE_OFFSET",
            l.trinket_tribe_offset,
            l.base_slot_num_dim + n_gifts,
        )?;
        expect("NUM_TRINKET_TRIBES", l.num_trinket_tribes, n_lobby)?;
        expect(
            "END_OF_TURN_STATS_OFFSET",
            l.end_of_turn_stats_offset,
            l.trinket_tribe_offset + l.num_trinket_tribes,
        )?;
        expect(
            "TRIBES_OFFSET",
            l.tribes_offset,
            l.end_of_turn_stats_offset + 1,
        )?;
        expect(
            "COUNTER_OFFSET",
            l.counter_offset,
            l.tribes_offset + n_single,
        )?;
        expect(
            "UNIT_EXTRA_OFFSET",
            l.unit_extra_offset,
            l.counter_offset + 1,
        )?;
        expect("UNIT_EXTRA_DIM", l.unit_extra_dim, 15)?;
        expect(
            "SLOT_NUM_DIM",
            l.slot_num_dim,
            l.unit_extra_offset + l.unit_extra_dim,
        )?;
        if l.slot_num_dim < 4 {
            return Err(bad("SLOT_NUM_DIM is below 4".into()));
        }
        Ok(())
    }
}

/// The fields of a `TavernObservation` / `DuosObservation` the encoder reads, borrowed (the
/// Duos observation carries every Tavern field under the same name).
struct View<'a> {
    turn: u32,
    health: i32,
    armor: i32,
    tavern_tier: u32,
    gold: u32,
    max_gold: u32,
    upgrade_cost: u32,
    free_refreshes: u32,
    discover_pending: &'a Option<Vec<PyUnit>>,
    hero_id: u32,
    hero_power_usable: bool,
    hero_power_cost: u32,
    hero_power_uses_left: u32,
    hero_power_armed: u32,
    hero_power_counter: i32,
    hero_power_stacks: i32,
    hero_options: &'a [u32],
    is_frozen: bool,
    next_minion_discount: i32,
    waiting_for_hand: &'a [PyUnit],
    minion_cost: u32,
    refresh_cost: u32,
    second_hero_power_id: u32,
    second_hero_power_usable: bool,
    second_hero_power_cost: u32,
    second_hero_power_uses_left: u32,
    second_hero_power_armed: u32,
    second_hero_power_counter: i32,
    second_hero_power_stacks: i32,
    trinkets: &'a [u32],
    trinket_counters: &'a [i32],
    trinket_options: &'a [u32],
    trinket_option_costs: &'a [u32],
    trinket_tribes: &'a [u32],
    trinket_option_tribes: &'a [u32],
    dark_discoveries_left: u32,
    dark_discovery_usable: bool,
    board: &'a [PyUnit],
    hand: &'a [PyUnit],
    shop: &'a [PyUnit],
    player_counters: &'a [i32],
    card_counters: &'a [(u32, i32, i32)],
    effects: &'a [(u32, u32, bool)],
}

macro_rules! view_of {
    ($o:expr) => {
        View {
            turn: $o.turn,
            health: $o.health,
            armor: $o.armor,
            tavern_tier: $o.tavern_tier,
            gold: $o.gold,
            max_gold: $o.max_gold,
            upgrade_cost: $o.upgrade_cost,
            free_refreshes: $o.free_refreshes,
            discover_pending: &$o.discover_pending,
            hero_id: $o.hero_id,
            hero_power_usable: $o.hero_power_usable,
            hero_power_cost: $o.hero_power_cost,
            hero_power_uses_left: $o.hero_power_uses_left,
            hero_power_armed: $o.hero_power_armed,
            hero_power_counter: $o.hero_power_counter,
            hero_power_stacks: $o.hero_power_stacks,
            hero_options: &$o.hero_options,
            is_frozen: $o.is_frozen,
            next_minion_discount: $o.next_minion_discount,
            waiting_for_hand: &$o.waiting_for_hand,
            minion_cost: $o.minion_cost,
            refresh_cost: $o.refresh_cost,
            second_hero_power_id: $o.second_hero_power_id,
            second_hero_power_usable: $o.second_hero_power_usable,
            second_hero_power_cost: $o.second_hero_power_cost,
            second_hero_power_uses_left: $o.second_hero_power_uses_left,
            second_hero_power_armed: $o.second_hero_power_armed,
            second_hero_power_counter: $o.second_hero_power_counter,
            second_hero_power_stacks: $o.second_hero_power_stacks,
            trinkets: &$o.trinkets,
            trinket_counters: &$o.trinket_counters,
            trinket_options: &$o.trinket_options,
            trinket_option_costs: &$o.trinket_option_costs,
            trinket_tribes: &$o.trinket_tribes,
            trinket_option_tribes: &$o.trinket_option_tribes,
            dark_discoveries_left: $o.dark_discoveries_left,
            dark_discovery_usable: $o.dark_discovery_usable,
            board: &$o.board,
            hand: &$o.hand,
            shop: &$o.shop,
            player_counters: &$o.player_counters,
            card_counters: &$o.card_counters,
            effects: &$o.effects,
        }
    };
}

/// The output arrays being filled.
struct Out {
    player: Vec<f32>,
    ids: Vec<i64>,
    num: Vec<f32>,
    slot_dim: usize,
}

impl Out {
    fn row(&mut self, slot: usize) -> &mut [f32] {
        let d = self.slot_dim;
        &mut self.num[slot * d..(slot + 1) * d]
    }
}

/// The observation encoder and action mask, built once from Python's layout.
#[pyclass(name = "Encoder")]
pub struct PyEncoder {
    card_index: HashMap<u32, i64>,
    gift_index: HashMap<u32, usize>,
    single_tribes: Vec<String>,
    lobby_tribes: Vec<String>,
    layout: Layout,
    /// Each distinct action kind, by first appearance.
    kinds: Vec<String>,
    /// `(kind id, arg0 + 1 or 0, arg1 + 1 or 0)` -> action index.
    action_index: HashMap<(usize, usize, usize), usize>,
}

impl PyEncoder {
    fn card(&self, id: u32) -> i64 {
        self.card_index.get(&id).copied().unwrap_or(0)
    }

    fn action_of(&self, a: &PyTavernAction) -> Option<usize> {
        let kind = a.kind();
        let k = self.kinds.iter().position(|s| s == kind)?;
        let (a0, a1) = a.args();
        self.action_index
            .get(&(k, a0.map_or(0, |x| x + 1), a1.map_or(0, |x| x + 1)))
            .copied()
    }

    /// `features._encode_unit_into`.
    fn unit(&self, u: &PyUnit, out: &mut [f32]) -> i64 {
        let l = &self.layout;
        out[0] = 1.0; // present flag
        out[1] = (log1p_pos(u.attack() as i64)) as f32;
        out[2] = (log1p_pos(u.health() as i64)) as f32;
        out[3] = (u.tavern_tier() as f64 / 7.0) as f32;
        out[4] = flag(u.is_spell()) as f32;
        out[5] = flag(u.is_golden()) as f32;
        out[6] = flag(u.taunt()) as f32;
        out[7] = flag(u.divine_shield()) as f32;
        out[8] = flag(u.windfury()) as f32;
        out[9] = flag(u.reborn()) as f32;
        out[10] = flag(u.venomous()) as f32;
        out[11] = flag(u.magnetic()) as f32;
        out[12] = flag(u.frozen()) as f32;
        out[13] = flag(u.marked()) as f32;
        for gid in u.dark_gifts() {
            if let Some(&feature) = self.gift_index.get(&gid) {
                out[l.base_slot_num_dim + feature] += 1.0;
            }
        }
        out[l.end_of_turn_stats_offset] = log1p_pos(u.end_of_turn_stats() as i64) as f32;
        for tribe in u.tribes() {
            if let Some(feature) = self.single_tribes.iter().position(|t| t == tribe) {
                out[l.tribes_offset + feature] = 1.0;
            }
        }
        out[l.counter_offset] = log1p_pos(u.counter() as i64) as f32;
        let (perm_atk, perm_hp) = u.perm_stats();
        let extra = [
            flag(u.stealth()),
            if u.is_spell() {
                u.spell_cost() as f64 / 10.0
            } else {
                0.0
            },
            flag(u.costs_health()),
            flag(u.unplayable()),
            u.locked_turns() as f64 / 3.0,
            symlog(u.stacks() as i64),
            u.charges() as f64 / 3.0,
            u.avenge_counter() as f64 / 5.0,
            symlog(perm_atk as i64),
            symlog(perm_hp as i64),
            (u.blood_gems_played() as f64).ln_1p() / 5.0,
            u.extra_deathrattles() as f64 / 3.0,
            symlog(u.attack() as i64 - u.base_attack() as i64),
            symlog(u.health() as i64 - u.base_health() as i64),
            flag(u.activated_this_turn()),
        ];
        for (i, v) in extra.iter().enumerate() {
            out[l.unit_extra_offset + i] = *v as f32;
        }
        self.card(u.card_id())
    }

    fn put_unit(&self, o: &mut Out, slot: usize, u: &PyUnit) {
        let id = self.unit(u, o.row(slot));
        o.ids[slot] = id;
    }

    /// `features._encode_trinket_tribe`.
    fn trinket_tribe(&self, tribes: &[u32], i: usize, row: &mut [f32]) {
        let l = &self.layout;
        if i < tribes.len() && 1 <= tribes[i] as usize && tribes[i] as usize <= l.num_trinket_tribes
        {
            row[l.trinket_tribe_offset + tribes[i] as usize - 1] = 1.0;
        }
    }

    /// `features.encode_observation`.
    fn encode_view(
        &self,
        obs: &View,
        duos: Option<&DuosExtra>,
        context: Option<&PyLobbyContext>,
        arranging: bool,
    ) -> PyResult<Out> {
        let l = &self.layout;
        let mut o = Out {
            player: vec![0.0; l.player_dim],
            ids: vec![0; l.num_slots],
            num: vec![0.0; l.num_slots * l.slot_num_dim],
            slot_dim: l.slot_num_dim,
        };
        let mut tavern: Vec<f64> = Vec::with_capacity(l.tavern_player_dim);
        tavern.extend_from_slice(&[
            obs.turn as f64 / 20.0,
            obs.health as f64 / 30.0,
            obs.armor as f64 / 10.0,
            obs.tavern_tier as f64 / 6.0,
            obs.gold as f64 / 10.0,
            obs.max_gold as f64 / 10.0,
            obs.upgrade_cost as f64 / 10.0,
            obs.free_refreshes as f64 / 5.0,
            flag(obs.discover_pending.is_some()),
            // The hero and its Hero Power; all 0 without a hero.
            flag(obs.hero_id != 0),
            flag(obs.hero_power_usable),
            obs.hero_power_cost as f64 / 10.0,
            obs.hero_power_uses_left as f64,
            obs.hero_power_armed as f64 / 3.0,
            flag(!obs.hero_options.is_empty()),
            // Freezing: the shop as a whole.
            flag(obs.is_frozen),
            // The next minion's discount.
            obs.next_minion_discount as f64 / 3.0,
            // The cards waiting for room in the hand.
            obs.waiting_for_hand.len() as f64 / l.max_waiting as f64,
            // The prices a Hero Power can change.
            obs.minion_cost as f64 / 3.0,
            obs.refresh_cost as f64 / 2.0,
            // The second Hero Power.
            flag(obs.second_hero_power_id != 0),
            flag(obs.second_hero_power_usable),
            obs.second_hero_power_cost as f64 / 10.0,
            obs.second_hero_power_uses_left as f64,
            obs.second_hero_power_armed as f64 / 3.0,
            // Trinkets: a pending offer, the trinkets owned, and the offered prices.
            flag(!obs.trinket_options.is_empty()),
            obs.trinkets.len() as f64 / l.max_trinkets as f64,
        ]);
        for i in 0..l.max_trinket_options {
            tavern.push(
                obs.trinket_option_costs
                    .get(i)
                    .map_or(0.0, |&c| c as f64 / 10.0),
            );
        }
        tavern.push(obs.dark_discoveries_left as f64 / 3.0);
        tavern.push(flag(obs.dark_discovery_usable));
        debug_assert_eq!(tavern.len(), l.tavern_player_dim);
        for (i, v) in tavern.iter().enumerate() {
            o.player[i] = *v as f32;
        }

        let mut offset = 0;
        for (i, u) in obs.board.iter().take(l.max_board).enumerate() {
            self.put_unit(&mut o, offset + i, u);
        }
        offset += l.max_board;
        for (i, u) in obs.hand.iter().take(l.max_hand).enumerate() {
            self.put_unit(&mut o, offset + i, u);
        }
        offset += l.max_hand;
        for (i, u) in obs.shop.iter().take(l.max_shop).enumerate() {
            self.put_unit(&mut o, offset + i, u);
        }
        offset += l.max_shop;
        if let Some(opts) = obs.discover_pending {
            for (i, u) in opts.iter().take(l.max_discover).enumerate() {
                self.put_unit(&mut o, offset + i, u);
            }
        }
        offset += l.max_discover;
        for (i, u) in obs.waiting_for_hand.iter().take(l.max_waiting).enumerate() {
            self.put_unit(&mut o, offset + i, u);
        }
        // Trinkets: a present flag, the card id and the minion type the trinket names.
        offset += l.max_waiting;
        for (i, &tid) in obs.trinkets.iter().take(l.max_trinkets).enumerate() {
            o.ids[offset + i] = self.card(tid);
            let row = o.row(offset + i);
            row[0] = 1.0;
            self.trinket_tribe(obs.trinket_tribes, i, row);
            if i < obs.trinket_counters.len() {
                row[l.counter_offset] = symlog(obs.trinket_counters[i] as i64) as f32;
            }
        }
        offset += l.max_trinkets;
        for (i, &tid) in obs
            .trinket_options
            .iter()
            .take(l.max_trinket_options)
            .enumerate()
        {
            o.ids[offset + i] = self.card(tid);
            let row = o.row(offset + i);
            row[0] = 1.0;
            self.trinket_tribe(obs.trinket_option_tribes, i, row);
        }

        // The hero, then the offered heroes: a present flag and the hero's id.
        let heroes = std::iter::once(obs.hero_id)
            .chain(obs.hero_options.iter().take(l.max_hero_options).copied());
        for (i, hid) in heroes.enumerate() {
            if hid != 0 {
                o.ids[l.hero_offset + i] = self.card(hid);
                o.row(l.hero_offset + i)[0] = 1.0;
            }
        }

        if let Some(d) = duos {
            self.encode_duos(d, &mut o);
        }
        if let Some(c) = context {
            self.encode_lobby(c, obs.turn, &mut o);
        }
        o.player[l.phase_offset] = flag(arranging) as f32;
        self.encode_counters(obs, &mut o)?;
        Ok(o)
    }

    /// `features._encode_duos`.
    fn encode_duos(&self, d: &DuosExtra, o: &mut Out) {
        let l = &self.layout;
        let v = [
            1.0, // a Duos game
            d.obs.teammate_tavern_tier as f64 / 7.0,
            d.obs.teammate_gold as f64 / 10.0,
            flag(d.obs.teammate_ended),
            flag(d.obs.teammate_hero_id != 0),
            flag(d.obs.fights_first),
            d.obs.free_passes as f64 / 3.0,
            d.obs.incoming.len() as f64 / 3.0,
            d.obs.teammate_incoming.len() as f64 / 3.0,
            d.obs.passes_this_turn as f64 / 3.0,
            d.obs.passes_this_game as f64 / 10.0,
        ];
        for (i, x) in v.iter().enumerate() {
            o.player[l.duos_offset + i] = *x as f32;
        }
        for (i, u) in d.obs.teammate_board.iter().take(l.max_board).enumerate() {
            self.put_unit(o, l.teammate_offset + i, u);
        }
        for (i, u) in d.obs.teammate_hand.iter().take(l.max_hand).enumerate() {
            self.put_unit(o, l.teammate_offset + l.max_board + i, u);
        }
        for (i, u) in d.obs.teammate_shop.iter().take(l.max_shop).enumerate() {
            self.put_unit(o, l.teammate_shop_offset + i, u);
        }
    }

    /// `features._encode_counters`.
    fn encode_counters(&self, obs: &View, o: &mut Out) -> PyResult<()> {
        let l = &self.layout;
        let mut values: Vec<i64> = obs.player_counters.iter().map(|&v| v as i64).collect();
        values.extend([
            obs.hero_power_counter as i64,
            obs.hero_power_stacks as i64,
            obs.second_hero_power_counter as i64,
            obs.second_hero_power_stacks as i64,
        ]);
        if values.len() != l.counters_dim {
            return Err(PyValueError::new_err(format!(
                "the observation has {} counters, the layout expects {}",
                values.len(),
                l.counters_dim
            )));
        }
        for (i, &v) in values.iter().enumerate() {
            o.player[l.counters_offset + i] = symlog(v) as f32;
        }
        for (i, &(card_id, a, b)) in obs
            .card_counters
            .iter()
            .take(l.max_card_counters)
            .enumerate()
        {
            let slot = l.card_counter_offset + i;
            o.ids[slot] = self.card(card_id);
            let row = o.row(slot);
            row[0] = 1.0;
            row[1] = symlog(a as i64) as f32;
            row[2] = symlog(b as i64) as f32;
        }
        for (i, &(card_id, stacks, this_turn)) in obs.effects.iter().take(l.max_effects).enumerate()
        {
            let slot = l.effect_offset + i;
            o.ids[slot] = self.card(card_id);
            let row = o.row(slot);
            row[0] = 1.0;
            row[1] = symlog(stacks as i64) as f32;
            row[3] = flag(this_turn) as f32;
        }
        Ok(())
    }

    /// `features._encode_lobby`.
    fn encode_lobby(&self, c: &PyLobbyContext, turn: u32, o: &mut Out) {
        let l = &self.layout;
        let mut next = c.opponents.iter().filter(|x| x.is_next);
        let first = next.next();
        let cap = c.damage_cap;
        let mut v: Vec<f64> = vec![
            1.0, // a lobby game
            c.units as f64 / 8.0,
            c.alive_units as f64 / 8.0,
            (c.health_rank as i64 - 1) as f64 / 7.0,
            flag(c.next_is_ghost),
            first.map_or(0.0, |f| f.health as f64 / 30.0),
            first.map_or(0.0, |f| f.armor as f64 / 20.0),
            first.map_or(0.0, |f| f.tier as f64 / 7.0),
            flag(cap.is_some()),
            cap.map_or(0.0, |x| x as f64 / 15.0),
        ];
        // Every type is in a lobby that names none.
        for t in &self.lobby_tribes {
            v.push(flag(c.tribes.is_empty() || c.tribes.iter().any(|x| x == t)));
        }
        for (i, x) in v.iter().enumerate() {
            o.player[l.lobby_offset + i] = *x as f32;
        }
        for (i, opp) in c.opponents.iter().take(l.max_opponents).enumerate() {
            let at = l.opponents_offset + i * l.opponent_dim;
            let seen = opp.seen_turn;
            let v = [
                1.0,
                flag(opp.alive),
                opp.health as f64 / 30.0,
                opp.armor as f64 / 20.0,
                opp.tier as f64 / 7.0,
                opp.win_streak.min(10) as f64 / 5.0,
                flag(opp.is_next),
                flag(opp.is_last),
                flag(seen.is_some()),
                // How stale the board seen is: the turns since that combat.
                seen.map_or(0.0, |s| (turn as i64 - s as i64).max(0) as f64 / 10.0),
            ];
            for (j, x) in v.iter().enumerate() {
                o.player[at + j] = *x as f32;
            }
        }
        for (i, opp) in c.opponents.iter().take(l.max_opponents_slots).enumerate() {
            if opp.hero_id != 0 {
                let slot = l.opponent_hero_offset + i;
                o.ids[slot] = self.card(opp.hero_id);
                o.row(slot)[0] = if opp.alive { 1.0 } else { 0.5 };
            }
        }
        for (k, opp) in c.opponents.iter().filter(|x| x.is_next).take(2).enumerate() {
            for (i, u) in opp.seen_board.iter().take(l.max_board).enumerate() {
                self.put_unit(o, l.opponent_board_offset + k * l.max_board + i, u);
            }
        }
    }

    /// The observation of player `p` and its encoding, Solo or Duos.
    fn encode_game(
        &self,
        game: &PyLobbyGame,
        p: usize,
        arranging: bool,
        context: bool,
    ) -> PyResult<Out> {
        let lobby = game.lobby();
        if p >= lobby.num_players() {
            return Err(PyValueError::new_err(format!(
                "player {p} does not exist (the lobby has {} players)",
                lobby.num_players()
            )));
        }
        let ctx = context.then(|| PyLobbyContext::from(lobby.context(p)));
        let g = lobby.game();
        if g.is_duos() {
            let obs = PyDuosObservation::new(&g.teams()[p / 2], p % 2);
            let view = view_of!(obs);
            self.encode_view(
                &view,
                Some(&DuosExtra { obs: &obs }),
                ctx.as_ref(),
                arranging,
            )
        } else {
            let obs = PyTavernObservation::from(g.player(p));
            let view = view_of!(obs);
            self.encode_view(&view, None, ctx.as_ref(), arranging)
        }
    }
}

struct DuosExtra<'a> {
    obs: &'a PyDuosObservation,
}

fn field<'py, T: for<'a> FromPyObject<'a>>(spec: &Bound<'py, PyDict>, key: &str) -> PyResult<T> {
    match spec.get_item(key)? {
        Some(v) => v.extract::<T>().map_err(|e| bad(format!("{key}: {e}"))),
        None => Err(bad(format!("missing {key}"))),
    }
}

fn bytes_f32(py: Python<'_>, v: &[f32]) -> PyResult<Py<PyByteArray>> {
    Ok(PyByteArray::new_bound_with(py, v.len() * 4, |b| {
        for (c, x) in b.chunks_exact_mut(4).zip(v) {
            c.copy_from_slice(&x.to_le_bytes());
        }
        Ok(())
    })?
    .unbind())
}

#[pymethods]
impl PyEncoder {
    /// `Encoder(spec)`: `card_index`, `dark_gift_index`, `single_tribes`, `lobby_tribes`,
    /// `actions` (the action table in index order, `(kind, arg0, arg1)`) and `layout` (every
    /// constant of `features.py`), all built by `features.encoder_spec()`.
    #[new]
    fn new(spec: &Bound<'_, PyDict>) -> PyResult<Self> {
        let card_index: HashMap<u32, i64> = field(spec, "card_index")?;
        let gift_index: HashMap<u32, usize> = field(spec, "dark_gift_index")?;
        let single_tribes: Vec<String> = field(spec, "single_tribes")?;
        let lobby_tribes: Vec<String> = field(spec, "lobby_tribes")?;
        let actions: Vec<(String, Option<usize>, Option<usize>)> = field(spec, "actions")?;
        let layout_map: HashMap<String, usize> = field(spec, "layout")?;
        let layout = Layout::from_map(&layout_map)?;
        layout.validate(gift_index.len(), single_tribes.len(), lobby_tribes.len())?;
        if gift_index.values().any(|&i| i >= gift_index.len()) {
            return Err(bad("dark_gift_index is not 0..n".into()));
        }
        expect("ACTION_SPACE_SIZE", layout.action_space_size, actions.len())?;
        let mut kinds: Vec<String> = Vec::new();
        let mut action_index = HashMap::new();
        for (idx, (kind, a0, a1)) in actions.iter().enumerate() {
            let k = match kinds.iter().position(|s| s == kind) {
                Some(k) => k,
                None => {
                    kinds.push(kind.clone());
                    kinds.len() - 1
                }
            };
            let key = (k, a0.map_or(0, |x| x + 1), a1.map_or(0, |x| x + 1));
            if action_index.insert(key, idx).is_some() {
                return Err(bad(format!("action {idx} ({kind}) is a duplicate")));
            }
        }
        Ok(Self {
            card_index,
            gift_index,
            single_tribes,
            lobby_tribes,
            layout,
            kinds,
            action_index,
        })
    }

    /// `features.encode_observation(game.observe(p), game.context(p) if context else None,
    /// arranging=arranging)` as raw little-endian buffers: `player_vec` float32, `slot_card_ids`
    /// int64, `slot_num_feats` float32 (`NUM_SLOTS * SLOT_NUM_DIM`, row-major).
    fn encode(
        &self,
        py: Python<'_>,
        game: &PyLobbyGame,
        p: usize,
        arranging: bool,
        context: bool,
    ) -> PyResult<(Py<PyByteArray>, Py<PyByteArray>, Py<PyByteArray>)> {
        let o = self.encode_game(game, p, arranging, context)?;
        let ids = PyByteArray::new_bound_with(py, o.ids.len() * 8, |b| {
            for (c, x) in b.chunks_exact_mut(8).zip(&o.ids) {
                c.copy_from_slice(&x.to_le_bytes());
            }
            Ok(())
        })?
        .unbind();
        Ok((bytes_f32(py, &o.player)?, ids, bytes_f32(py, &o.num)?))
    }

    /// `actions.action_mask(seat, allow_reposition, allow_freeze, prune_dominated, obs)` as one
    /// byte (0 or 1) per action.
    fn mask(
        &self,
        py: Python<'_>,
        game: &PyLobbyGame,
        p: usize,
        allow_reposition: bool,
        allow_freeze: bool,
        prune_dominated: bool,
    ) -> PyResult<Py<PyByteArray>> {
        let lobby = game.lobby();
        if p >= lobby.num_players() {
            return Err(PyValueError::new_err(format!(
                "player {p} does not exist (the lobby has {} players)",
                lobby.num_players()
            )));
        }
        let (can_sell, can_refresh) = if prune_dominated {
            let s = lobby.player(p);
            let board_len = s.board.len();
            let hand_has_minion = s.hand.iter().any(|u| !u.is_spell);
            let gold = s.gold;
            (
                (board_len >= self.layout.max_board && (hand_has_minion || gold >= 2))
                    || (board_len >= 4 && gold == 2),
                s.auras.free_refreshes > 0 || gold >= 4,
            )
        } else {
            (true, true)
        };
        let mut mask = vec![0u8; self.layout.action_space_size];
        for inner in lobby.valid_actions(p) {
            let act = PyTavernAction { inner };
            let kind = act.kind();
            if (!allow_reposition && kind == "reposition")
                || (!allow_freeze && kind == "toggle_freeze")
                || (!can_sell && kind == "sell")
                || (!can_refresh && kind == "refresh")
            {
                continue;
            }
            if let Some(idx) = self.action_of(&act) {
                mask[idx] = 1;
            }
        }
        Ok(PyByteArray::new_bound(py, &mask).unbind())
    }

    /// `len(game.observe(p).board)`.
    fn board_len(&self, game: &PyLobbyGame, p: usize) -> PyResult<usize> {
        let lobby = game.lobby();
        if p >= lobby.num_players() {
            return Err(PyValueError::new_err(format!(
                "player {p} does not exist (the lobby has {} players)",
                lobby.num_players()
            )));
        }
        Ok(lobby.player(p).board.len())
    }
}
