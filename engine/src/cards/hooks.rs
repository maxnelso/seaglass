//! Card behaviour hooks: the only channel through which card text reaches the engine.
//!
//! Every card registers a [`CardHooks`] table, usually through the builder methods on its
//! [`CardTemplate`](super::CardTemplate) (`.on_battlecry(..)`, `.on_deathrattle(..)`, ...).
//! Tokens, spells, and Deities register theirs through `behaviors()` functions in their
//! modules. The Tavern and Combat engines never name a specific card: they look up the
//! hooks of the units involved in an event (see [`super::hooks`]) and invoke them.
//!
//! Hook families:
//! - **Own-text hooks** fire for the unit whose card text it is (`battlecry`, `deathrattle`,
//!   `rally`, `on_sell`, ...).
//! - **Observer hooks** fire for every friendly unit carrying them, left to right in board
//!   order, and receive the observer's own board index (`end_of_turn`, `on_spell_cast`,
//!   `on_friendly_death`, ...).
//! - **Passives** are numeric contributions combined across the board ([`Passive`]).
//! - **Player-effect hooks** fire for each of the player's
//!   [`PlayerEffect`](crate::model::PlayerEffect)s of the card, in the order they were recorded
//!   (`player_turn_start`, `player_start_of_combat`, ...).

use crate::cards::{ActivateTargetKind, BoardCtx, CombatSides, RallyCtx};
use crate::model::{CardId, CombatResult, PlayerAuras, PlayerEffect, Tribe, Unit, UnitId};
use crate::rng::Rng;
use crate::tavern::{CardPool, TavernState};

/// Static yes/no properties of a card that the engine implements generically.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub struct CardFlags(u32);

impl CardFlags {
    pub const NONE: CardFlags = CardFlags(0);
    /// Attacks also damage the minions adjacent to the target.
    pub const CLEAVE: CardFlags = CardFlags(1 << 0);
    /// Excess attack damage spills onto a neighbour of the target (both if Golden).
    pub const EXCESS_DAMAGE_TO_NEIGHBORS: CardFlags = CardFlags(1 << 1);
    /// Can never be part of a triple.
    pub const NO_TRIPLE: CardFlags = CardFlags(1 << 2);
    /// Counts as a copy of any Elemental when checking for triples.
    pub const TRIPLE_WILDCARD_ELEMENTAL: CardFlags = CardFlags(1 << 3);
    /// Cannot be bought from the shop.
    pub const UNBUYABLE: CardFlags = CardFlags(1 << 4);
    /// While on board, hero damage taken in the Tavern is rewound (so Health costs are payable).
    pub const REWINDS_HERO_DAMAGE: CardFlags = CardFlags(1 << 5);
    /// A Choose-One option card (as opposed to a Discover-style option such as a Hero Power).
    pub const CHOOSE_ONE_OPTION: CardFlags = CardFlags(1 << 6);
    /// While this has `charges` left, a Refresh costs 1 Health instead of Gold, using a charge.
    pub const HEALTH_REFRESHES: CardFlags = CardFlags(1 << 7);
    /// While this has `charges` left, a Choose One has both effects combined, using a charge.
    pub const COMBINES_CHOOSE_ONE: CardFlags = CardFlags(1 << 8);

    #[inline]
    pub const fn contains(self, other: CardFlags) -> bool {
        self.0 & other.0 == other.0
    }

    #[inline]
    pub const fn union(self, other: CardFlags) -> CardFlags {
        CardFlags(self.0 | other.0)
    }
}

impl std::ops::BitOr for CardFlags {
    type Output = CardFlags;
    fn bitor(self, rhs: CardFlags) -> CardFlags {
        self.union(rhs)
    }
}

/// Numeric board-wide modifiers contributed by minions (see [`CardHooks::passive`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Passive {
    /// Times a played Battlecry triggers (max across the board, at least 1).
    BattlecryTriggers,
    /// Times End-of-Turn effects trigger (max across the board, at least 1).
    EndOfTurnTriggers,
    /// Times a Bounty spell is cast (max across the board, at least 1).
    BountyCasts,
    /// Times a targeted Tavern spell is cast (max across the board, at least 1).
    TargetedSpellCasts,
    /// Extra Deathrattle triggers (summed across living minions).
    ExtraDeathrattles,
    /// Extra casts of Blood Gems played from hand (summed across the board).
    ExtraHandBloodGemCasts,
}

impl Passive {
    /// `true` if contributions combine by `max` with a floor of 1, `false` if they are summed.
    pub const fn is_multiplier(self) -> bool {
        matches!(
            self,
            Passive::BattlecryTriggers
                | Passive::EndOfTurnTriggers
                | Passive::BountyCasts
                | Passive::TargetedSpellCasts
        )
    }
}

/// A Start-of-Combat effect resolved by the engine in a dedicated phase after all
/// ordinary Start-of-Combat triggers (see [`CardHooks::start_of_combat_action`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SocAction {
    /// Destroy the friendly minion to the left (and to the right if `both_sides`), skipping
    /// copies of the source card, and store the victims inside the source.
    DestroyNeighbors { both_sides: bool },
    /// Deal `amount` damage to every other minion (both boards), `waves` times.
    DamageAll { amount: i32, waves: u32 },
    /// Attack immediately `times` times.
    Attack { times: u32 },
}

/// A minion played from hand, as seen by `after_friendly_play` observers.
#[derive(Clone, Copy, Debug)]
pub struct Played {
    pub card_id: CardId,
    pub tribe: Tribe,
    /// Board index of the played minion (or of the minion it was Magnetized onto).
    pub board_pos: usize,
    pub magnetized: bool,
}

// --- Hook signatures --------------------------------------------------------------------

/// Battlecry / Choose One: `(state, played_unit, board_pos, pool, rng)`.
pub type PlayFn = fn(&mut TavernState, &mut Unit, usize, &mut CardPool, &mut Rng);
/// Activate: `(state, source_pos, target_pos, pool, rng)`.
pub type ActivateFn = fn(&mut TavernState, usize, Option<usize>, &mut CardPool, &mut Rng);
/// On-sell / on-discard / on-choose for the card itself: `(state, card, pool, rng)`.
pub type CardEventFn = fn(&mut TavernState, &Unit, &mut CardPool, &mut Rng);
/// Generic Tavern observer: `(state, self_idx, pool, rng)`.
pub type TavernFn = fn(&mut TavernState, usize, &mut CardPool, &mut Rng);
/// Tavern hand-card hook: `(state, hand_idx, rng)`.
pub type HandFn = fn(&mut TavernState, usize, &mut Rng);
/// Per-unit state hook.
pub type UnitFn = fn(&mut Unit);
/// Deathrattle: `(dying, ctx)`.
pub type DeathrattleFn = fn(&Unit, &mut BoardCtx<'_>);
/// Rally (On-Attack): returns tokens to summon to the attacker's right.
pub type RallyFn = fn(&mut RallyCtx<'_>) -> Vec<Unit>;
/// Start of Combat: `(ctx, self_id, is_golden)`.
pub type SocFn = fn(&mut BoardCtx<'_>, UnitId, bool);
/// Combat observer: `(ctx, self_idx)`.
pub type CombatFn = fn(&mut BoardCtx<'_>, usize);
/// After a friendly minion is played or Magnetized: `(state, self_idx, played, pool, rng)`.
pub type PlayedFn = fn(&mut TavernState, usize, &Played, &mut CardPool, &mut Rng);
/// After a friendly Battlecry triggers: `(state, self_idx, played_unit_if_not_on_board)`.
pub type BattlecryObserverFn = fn(&mut TavernState, usize, Option<&mut Unit>);
/// After a card is Magnetized: `(state, self_idx, magnetized_card, target_pos, pool, rng)`.
pub type MagnetizeObserverFn = fn(&mut TavernState, usize, &Unit, usize, &mut CardPool, &mut Rng);
/// After a targeted spell is cast: `(state, self_idx, target_pos, pool, rng)`.
pub type TargetedSpellFn = fn(&mut TavernState, usize, usize, &mut CardPool, &mut Rng);
/// Another friendly minion dealt damage: `(ctx, self_idx, source_id, source_tribe)`.
pub type DamageDealtObserverFn = fn(&mut BoardCtx<'_>, usize, UnitId, Tribe);
/// A friendly minion is being summoned: `(self, summoned, in_combat)`.
pub type SummonFn = fn(&mut Unit, &mut Unit, bool);
/// Player effect in the Tavern: `(state, effect)`.
pub type EffectFn = fn(&mut TavernState, &mut PlayerEffect);
/// Player effect after a shop Refresh: `(state, effect, rng)`.
pub type EffectRefreshFn = fn(&mut TavernState, &mut PlayerEffect, &mut Rng);
/// Player effect at Start of Combat: `(sides, effect)`.
pub type EffectSocFn = fn(&mut CombatSides<'_>, &mut PlayerEffect);
/// Player effect after combat: `(state, effect, result)`.
pub type EffectAfterCombatFn = fn(&mut TavernState, &mut PlayerEffect, CombatResult);

macro_rules! card_hooks {
    ($( $(#[$meta:meta])* $field:ident($builder:ident): $ty:ty ),* $(,)?) => {
        /// Behaviour table of a single card. Every hook is optional.
        #[derive(Clone, Copy, Debug)]
        pub struct CardHooks {
            pub flags: CardFlags,
            /// Gold cost of the card's `Activate` ability, if it has one.
            pub activate_cost: Option<u32>,
            /// Target domain of the card's `Activate` ability.
            pub activate_target: ActivateTargetKind,
            /// Board-aura bonus to Tavern spell stats while on board, `(atk, hp)` per
            /// non-Golden copy (doubled when Golden).
            pub spell_aura: (i32, i32),
            $( $(#[$meta])* pub $field: Option<$ty>, )*
        }

        impl CardHooks {
            /// A card with no behaviour.
            pub const EMPTY: CardHooks = CardHooks {
                flags: CardFlags::NONE,
                activate_cost: None,
                activate_target: ActivateTargetKind::None,
                spell_aura: (0, 0),
                $( $field: None, )*
            };

            $(
                pub fn $builder(mut self, f: $ty) -> Self {
                    self.$field = Some(f);
                    self
                }
            )*
        }

        impl crate::cards::CardTemplate {
            $(
                pub fn $builder(mut self, f: $ty) -> Self {
                    self.hooks.$field = Some(f);
                    self
                }
            )*
        }
    };
}

card_hooks! {
    // ---- Own card text: Tavern ------------------------------------------------------
    /// Battlecry (repeated by [`Passive::BattlecryTriggers`]).
    battlecry(on_battlecry): PlayFn,
    /// Choose One (presents or applies the option cards).
    choose_one(on_choose_one): PlayFn,
    /// First time this is played or Magnetized: `(state, card)`.
    play_or_magnetize(on_play_or_magnetize): fn(&mut TavernState, &mut Unit),
    /// Extra state carried over when this card is Magnetized onto `target`: `(card, target)`.
    magnetize_transfer(on_magnetize_transfer): fn(&Unit, &mut Unit),
    /// Extra times the next Magnetization onto this unit happens (consuming what queued them).
    extra_magnetizations(on_extra_magnetizations): fn(&mut Unit) -> u32,
    /// This card was sold.
    sell(on_sell): CardEventFn,
    /// `Activate` ability (see also `activate_cost` / `activate_target`).
    activate(on_activate): ActivateFn,
    /// This option card was chosen from a Choose-One / Discover-style prompt.
    choose(on_chosen): CardEventFn,
    /// This card was discarded from hand.
    discarded(on_discarded): CardEventFn,
    /// Start of turn while this card is in hand.
    turn_start_in_hand(on_turn_start_in_hand): HandFn,
    /// Checked after hand-affecting phases (destroys, combat) while this card is in hand.
    ready_in_hand(on_ready_in_hand): HandFn,
    /// Reset per-turn charges (on creation and at the start of each turn).
    reset_turn_charges(on_reset_turn_charges): UnitFn,
    /// Adjust card-specific state when this unit becomes Golden.
    made_golden(on_made_golden): UnitFn,
    /// This Golden was merged from a triple: adjust its card-specific state `(copies, golden)`.
    merge_golden(on_merge_golden): fn(&[Unit], &mut Unit),
    /// Stats this unit's own "wherever this is" aura gives it under the player's auras (applied
    /// and kept in sync through `Unit::aura_applied`).
    aura_bonus(on_aura_bonus): fn(&Unit, &PlayerAuras) -> (i32, i32),
    /// Re-sync aura-driven state of this unit other than stats (see `aura_bonus`).
    sync_aura(on_sync_aura): fn(&mut Unit, &PlayerAuras),
    /// Check once-per-game stat thresholds after this unit's stats change.
    stat_threshold(on_stat_threshold): UnitFn,
    /// `count` Blood Gems were played on this unit.
    blood_gems_played(on_blood_gems_played): fn(&mut Unit, u32),
    /// Flush effects this unit queued (e.g. Blood Gem procs): `(board, self_idx, auras, rng)`.
    resolve_pending(on_resolve_pending): fn(&mut [Unit], usize, &PlayerAuras, &mut Rng),
    /// Copy combat results onto the Tavern copy: `(pre_combat, post_combat_units, tavern_unit)`.
    post_combat(on_post_combat): fn(&Unit, &[Unit], &mut Unit),
    /// Multiplier with which the neighbour at `neighbour_idx` permanently keeps what it gained in
    /// combat (see [`keep_combat_gains`](super::keep_combat_gains); the largest across both
    /// neighbours applies, 0 = none): `(pre_board, self_idx, neighbour_idx)`.
    post_combat_neighbor_mult(on_post_combat_neighbor_mult): fn(&[Unit], usize, usize) -> i32,

    // ---- Observers: Tavern (fire for each friendly board unit, left to right) ----------
    /// Start of turn (after Gold is refreshed).
    turn_start(on_turn_start): TavernFn,
    /// Start of turn, for this unit only (before any other start-of-turn effect).
    turn_start_unit(on_turn_start_unit): UnitFn,
    /// End of turn (repeated by [`Passive::EndOfTurnTriggers`]).
    end_of_turn(on_end_of_turn): TavernFn,
    /// After a friendly minion is played or Magnetized: `(state, self_idx, played, pool, rng)`.
    after_friendly_play(on_after_friendly_play): PlayedFn,
    /// While this card is in hand, after a friendly minion is played or Magnetized.
    after_friendly_play_in_hand(on_after_friendly_play_in_hand): fn(&mut Unit, &Played),
    /// After a friendly Battlecry triggers: `(state, self_idx, played_unit_if_not_on_board)`.
    after_friendly_battlecry(on_after_friendly_battlecry): BattlecryObserverFn,
    /// After a Choose-One option is chosen.
    after_friendly_choose_one(on_after_friendly_choose_one): TavernFn,
    /// After a card is Magnetized onto another friendly minion:
    /// `(state, self_idx, magnetized_card, target_pos, pool, rng)`.
    after_friendly_magnetize(on_after_friendly_magnetize): MagnetizeObserverFn,
    /// After another friendly minion is sold.
    after_friendly_sell(on_after_friendly_sell): TavernFn,
    /// A Tavern spell is cast.
    spell_cast(on_spell_cast): TavernFn,
    /// After any spell is cast.
    after_spell_cast(on_after_spell_cast): TavernFn,
    /// After a targeted spell is cast: `(state, self_idx, target_pos, pool, rng)`.
    after_targeted_spell(on_after_targeted_spell): TargetedSpellFn,
    /// Gold was spent: `(state, self_idx, amount, pool, rng)`.
    gold_spent(on_gold_spent): fn(&mut TavernState, usize, u32, &mut CardPool, &mut Rng),
    /// A minion is being bought (before it reaches the hand): `(state, self_idx, bought)`.
    minion_bought(on_minion_bought): fn(&mut TavernState, usize, &mut Unit),
    /// After any card is bought: `(state, self_idx, bought, pool, rng)`.
    after_buy(on_after_buy): fn(&mut TavernState, usize, &Unit, &mut CardPool, &mut Rng),
    /// After a card is Discovered.
    discover(on_discover): TavernFn,
    /// After a card is discarded from hand.
    after_friendly_discard(on_after_friendly_discard): TavernFn,
    /// A card was added to hand (Tavern or Combat): `(self, auras)`.
    card_added_to_hand(on_card_added_to_hand): fn(&Unit, &mut PlayerAuras),
    /// Hero is about to take Tavern damage: `(state, self_idx, amount)`. Whether the damage is
    /// rewound is decided by [`CardFlags::REWINDS_HERO_DAMAGE`].
    hero_damage(on_hero_damage): fn(&mut TavernState, usize, i32),
    /// After the hero took damage: `(state, self_idx, amount)`.
    after_hero_damage(on_after_hero_damage): fn(&mut TavernState, usize, i32),
    /// Numeric contribution to a board-wide [`Passive`] (0 = none).
    passive(with_passive): fn(&Unit, Passive) -> u32,

    // ---- Own card text: Combat ---------------------------------------------------------
    deathrattle(on_deathrattle): DeathrattleFn,
    rally(on_rally): RallyFn,
    start_of_combat(on_start_of_combat): SocFn,
    /// Engine-resolved Start-of-Combat action (destroys, AoE, immediate attacks).
    start_of_combat_action(on_start_of_combat_action): fn(&Unit) -> SocAction,
    /// Copies of this card to summon at Start of Combat while it is in hand.
    combat_copies_from_hand(on_combat_copies_from_hand): fn(&Unit) -> u32,
    /// Reset per-combat state on the combat copy.
    combat_start(on_combat_start): UnitFn,
    /// This unit dealt `amount` damage: `(ctx, self_idx, amount)`.
    damage_dealt(on_damage_dealt): fn(&mut BoardCtx<'_>, usize, i32),
    /// This unit took damage: `(self, own_hand, rng)`.
    damage_taken(on_damage_taken): fn(&Unit, &mut [Unit], &mut Rng),
    /// Bonus damage dealt to the highest-Health enemy after this unit attacks.
    after_attack_damage(on_after_attack_damage): fn(&Unit) -> i32,
    /// This unit died (Tavern or Combat): `(self, auras)`.
    died(on_died): fn(&Unit, &mut PlayerAuras),
    /// This Deity awakened: `(ctx, self_idx)`.
    awaken(on_awaken): CombatFn,

    // ---- Observers: Combat (fire for each living friendly unit, left to right) ------------
    /// After a friendly Rally minion attacks: `(ctx, self_idx)`.
    after_friendly_rally(on_after_friendly_rally): CombatFn,
    /// A friendly minion attacks: `(ctx, self_idx, attacker_id)`.
    friendly_attack(on_friendly_attack): fn(&mut BoardCtx<'_>, usize, UnitId),
    /// Another friendly minion dealt damage: `(ctx, self_idx, source_id, source_tribe)`.
    after_friendly_damage_dealt(on_after_friendly_damage_dealt): DamageDealtObserverFn,
    /// A friendly minion died (Tavern or Combat): `(ctx, self_idx, dying)`.
    friendly_death(on_friendly_death): fn(&mut BoardCtx<'_>, usize, &Unit),
    /// A friendly minion is being summoned (Tavern or Combat): `(self, summoned, in_combat)`.
    friendly_summon(on_friendly_summon): SummonFn,
    /// After a friendly minion is Reborn: `(ctx, self_idx, reborn_attack)`.
    after_friendly_reborn(on_after_friendly_reborn): fn(&mut BoardCtx<'_>, usize, i32),
    /// A friendly minion lost its Divine Shield: `(self)`.
    friendly_divine_shield_lost(on_friendly_divine_shield_lost): UnitFn,

    // ---- Player effects (fire for each of the player's effects of this card, in the order
    // they were recorded) ----------------------------------------------------------------
    /// Start of turn (after minions' start-of-turn upkeep, before hand cards').
    player_turn_start(on_player_turn_start): EffectFn,
    /// After a shop Refresh (including the start-of-turn one).
    player_after_refresh(on_player_after_refresh): EffectRefreshFn,
    /// Start of Combat, before any minion's trigger.
    player_start_of_combat(on_player_start_of_combat): EffectSocFn,
    /// Room may have opened on the board in combat (Start of Combat, after each round of deaths).
    player_combat_space(on_player_combat_space): fn(&mut BoardCtx<'_>, &mut PlayerEffect),
    /// A friendly minion is being summoned in combat: `(effect, summoned)`.
    player_combat_summon(on_player_combat_summon): fn(&PlayerEffect, &mut Unit),
    /// After combat.
    player_after_combat(on_player_after_combat): EffectAfterCombatFn,
}

impl Default for CardHooks {
    fn default() -> Self {
        CardHooks::EMPTY
    }
}

impl CardHooks {
    #[inline]
    pub fn has(&self, flag: CardFlags) -> bool {
        self.flags.contains(flag)
    }

    pub fn with_flags(mut self, flags: CardFlags) -> Self {
        self.flags = self.flags | flags;
        self
    }

    /// Contribution of `unit` to `passive` (0 if this card has no such passive).
    #[inline]
    pub fn passive_of(&self, unit: &Unit, passive: Passive) -> u32 {
        self.passive.map_or(0, |f| f(unit, passive))
    }
}
