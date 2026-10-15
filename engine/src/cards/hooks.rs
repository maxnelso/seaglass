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
//!   `rally`, `on_sell`, a spell's `cast`, ...).
//! - **Observer hooks** fire for every friendly unit carrying them, left to right in board
//!   order, and receive the observer's own board index (`end_of_turn`, `on_spell_cast`,
//!   `on_friendly_death`, ...).
//! - **Passives** are numeric contributions combined across the board ([`Passive`]).
//! - **Player-effect hooks** fire for each of the player's
//!   [`PlayerEffect`](crate::model::PlayerEffect)s of the card, in the order they were recorded
//!   (`player_turn_start`, `player_start_of_combat`, ...).
//! - **Duos hooks** fire when a card is Passed to the teammate (`passed`, `after_pass`,
//!   `after_team_pass`), when the teammate sells (`after_teammate_sell`), and when a card's gift
//!   reaches the teammate (`teammate_gift`, `teammate_combat_gift`; `docs/duos_tavern.md` §7).
//! - **Hero Power hooks** fire for the player whose hero has the Hero Power: a Hero Power is a
//!   card with hooks (`hero_power`, `hero_start_of_combat`, ...; `docs/heroes.md` §4).

use crate::cards::{
    ActivateTargetKind, BoardCtx, CombatCastCtx, CombatSides, HeroCombatCtx, RallyCtx,
    TeammateWarband,
};
use crate::model::{
    CardId, CombatResult, HeroState, PlayerAuras, PlayerEffect, Tribe, Unit, UnitId,
};
use crate::rng::Rng;
use crate::tavern::{CardPool, MinionSlot, TavernState};

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
    /// While this has `charges` left, each Refresh uses a charge and costs Health instead of Gold:
    /// 1 Health, or 0 if the Refresh is free (`free_refreshes`).
    pub const HEALTH_REFRESHES: CardFlags = CardFlags(1 << 7);
    /// While this has `charges` left, a Choose One has both effects combined, using a charge.
    pub const COMBINES_CHOOSE_ONE: CardFlags = CardFlags(1 << 8);
    /// A Bounty spell: cast [`Passive::BountyCasts`] times.
    pub const BOUNTY: CardFlags = CardFlags(1 << 9);
    /// Never offered by the shop or by random / Discover Tavern-spell pools.
    pub const NOT_IN_POOL: CardFlags = CardFlags(1 << 10);
    /// A spell card that is not a Tavern spell (casting it doesn't count as casting one).
    pub const NOT_TAVERN_SPELL: CardFlags = CardFlags(1 << 11);
    /// A Blood Gem: played from your hand, it is cast [`Passive::ExtraHandBloodGemCasts`] more
    /// times, each a cast of its own.
    pub const BLOOD_GEM: CardFlags = CardFlags(1 << 12);
    /// Duos: while this has `charges` left, a Pass is free, using a charge
    /// (`docs/duos_tavern.md` §5.2).
    pub const FREE_PASS: CardFlags = CardFlags(1 << 13);
    /// A temporary card: still in the hand when its player ends their turn, it is discarded,
    /// without discard effects (a Spellcraft spell; `docs/tavern.md` §7.4).
    pub const TEMPORARY: CardFlags = CardFlags(1 << 14);

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
    /// Extra casts of Blood Gems played from hand, each a cast of its own (summed across the board).
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

/// A Start-of-Combat effect resolved by the engine in the minion's Start-of-Combat turn, right
/// after its `start_of_combat` hook (see [`CardHooks::start_of_combat_action`]).
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

/// A minion played from hand (or a card Magnetized onto a friendly minion), as seen by
/// `after_friendly_play` observers.
#[derive(Clone, Copy, Debug)]
pub struct Played {
    pub card_id: CardId,
    pub tribe: Tribe,
    /// Board index of the played minion (or of the minion it was Magnetized onto).
    pub board_pos: usize,
    pub magnetized: bool,
    /// `false` if this is not a play but a card effect: a Magnetization by a card (`Spark
    /// Snapper` and `Glambot` Satellites, `Polarizing Beatboxer` copies) or an extra one (`Drone
    /// Duplicator`).
    pub from_hand: bool,
}

/// What a spell is cast on (see [`CardHooks::spell_target`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpellTarget {
    /// Untargeted.
    None,
    /// A friendly minion.
    Friendly,
    /// A friendly minion of the given type.
    FriendlyTribe(Tribe),
    /// A friendly minion with a minion type (the spell uses its target's type).
    FriendlyTyped,
    /// Any minion ("a minion"): a friendly one or one in the Tavern (cast with `cast_at`).
    Minion,
    /// A minion in the Tavern, and only there ("a minion in the Tavern"; cast with `cast_at`).
    TavernMinion,
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
/// A Tavern spell is cast in combat: `(ctx, self_idx)`.
pub type CombatCastFn = fn(&mut CombatCastCtx<'_>, usize);
/// After a friendly minion is played or Magnetized: `(state, self_idx, played, pool, rng)`.
pub type PlayedFn = fn(&mut TavernState, usize, &Played, &mut CardPool, &mut Rng);
/// After a friendly Battlecry triggers: `(state, self_idx, played_unit_if_not_on_board)`.
pub type BattlecryObserverFn = fn(&mut TavernState, usize, Option<&mut Unit>);
/// After a card is Magnetized: `(state, self_idx, magnetized_card, target_pos, pool, rng)`.
pub type MagnetizeObserverFn = fn(&mut TavernState, usize, &Unit, usize, &mut CardPool, &mut Rng);
/// After a targeted spell is cast: `(state, self_idx, target, pool, rng)`; `target` is on the board
/// or in the Tavern.
pub type TargetedSpellFn = fn(&mut TavernState, usize, MinionSlot, &mut CardPool, &mut Rng);
/// Another friendly minion dealt damage: `(ctx, self_idx, source_id, source_tribe)`.
pub type DamageDealtObserverFn = fn(&mut BoardCtx<'_>, usize, UnitId, Tribe);
/// A friendly minion is being summoned: `(self, summoned, in_combat)`.
pub type SummonFn = fn(&mut Unit, &mut Unit, bool);
/// Spell cast: `(state, spell, target_pos, pool, rng)`.
pub type CastFn = fn(&mut TavernState, &Unit, usize, &mut CardPool, &mut Rng);
/// Spell cast on a friendly minion or one in the Tavern: `(state, spell, target, pool, rng)`.
pub type CastAtFn = fn(&mut TavernState, &Unit, MinionSlot, &mut CardPool, &mut Rng);
/// Player effect in the Tavern: `(state, effect)`.
pub type EffectFn = fn(&mut TavernState, &mut PlayerEffect);
/// Player effect after a shop Refresh: `(state, effect, rng)`.
pub type EffectRefreshFn = fn(&mut TavernState, &mut PlayerEffect, &mut Rng);
/// Player effect at Start of Combat: `(sides, effect)`.
pub type EffectSocFn = fn(&mut CombatSides<'_>, &mut PlayerEffect);
/// Player effect after combat: `(state, effect, result)`.
pub type EffectAfterCombatFn = fn(&mut TavernState, &mut PlayerEffect, CombatResult);
/// Observer of a card event (a Pass, a sale): `(state, self_idx, card, pool, rng)`.
pub type CardObserverFn = fn(&mut TavernState, usize, &Unit, &mut CardPool, &mut Rng);
/// A Tavern event for the player's Hero Power: `(state, pool, rng)`.
pub type TavernEventFn = fn(&mut TavernState, &mut CardPool, &mut Rng);
/// The `HeroPower` action: `(state, target, pool, rng)` (`docs/heroes.md` §5).
pub type HeroPowerFn = fn(&mut TavernState, Option<usize>, &mut CardPool, &mut Rng);

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
            /// What this spell is cast on.
            pub spell_target: SpellTarget,
            $( $(#[$meta])* pub $field: Option<$ty>, )*
        }

        impl CardHooks {
            /// A card with no behaviour.
            pub const EMPTY: CardHooks = CardHooks {
                flags: CardFlags::NONE,
                activate_cost: None,
                activate_target: ActivateTargetKind::None,
                spell_aura: (0, 0),
                spell_target: SpellTarget::None,
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
    /// The friendly minion this card's Battlecry targets: `(state, played_unit, board_pos)` → its
    /// board index (`None`: no valid target). Picked before the first trigger: every trigger has
    /// that target ([`battlecry_target_pos`](super::battlecry_target_pos)), and the triggers after
    /// it has left the board do nothing.
    battlecry_target(on_battlecry_target): fn(&TavernState, &Unit, usize) -> Option<usize>,
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
    /// "When you buy or Discover this": this card was bought (it is in the hand, its triple
    /// checked) or Discovered (added to the hand); `card` is the card as it reached the hand.
    obtained(on_obtained): CardEventFn,
    /// This card, given by the teammate (a `TeammateGift::Card`), arrives: in their Tavern (its
    /// incoming queue) or their combat hand, `(card, receiver's Tavern Tier)`.
    received(on_received): fn(&mut Unit, u32),
    /// Spellcraft: the spell this minion gives its player when it is played and at the start of
    /// each turn (a [`CardFlags::TEMPORARY`] card; `docs/tavern.md` §7.4).
    spellcraft(on_spellcraft): fn(&Unit) -> Unit,
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
    /// Multiplier with which this unit itself permanently keeps what it gained in combat (see
    /// [`keep_combat_gains`](super::keep_combat_gains); 0 = none).
    post_combat_keep_mult(on_post_combat_keep_mult): fn(&Unit) -> i32,
    /// Multiplier with which the neighbour at `neighbour_idx` permanently keeps what it gained in
    /// combat (see [`keep_combat_gains`](super::keep_combat_gains); the largest across self and
    /// both neighbours applies, 0 = none): `(pre_board, self_idx, neighbour_idx)`.
    post_combat_neighbor_mult(on_post_combat_neighbor_mult): fn(&[Unit], usize, usize) -> i32,

    // ---- Own card text: spells ----------------------------------------------------------
    /// This spell is cast on `target_pos` (0 if untargeted; see `spell_target`), once per cast. A
    /// spell without one adds its stats to the target.
    cast(on_cast): CastFn,
    /// This spell is cast on `target`, a friendly minion or one in the Tavern, once per cast
    /// (instead of `cast`; [`SpellTarget::Minion`] and [`SpellTarget::TavernMinion`] spells).
    cast_at(on_cast_at): CastAtFn,
    /// Which minions this targeted spell can be cast on, on top of its `spell_target` ("a
    /// non-Golden minion").
    spell_target_filter(on_spell_target_filter): fn(&Unit) -> bool,

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
    /// After a targeted spell is cast on a minion on the board or in the Tavern:
    /// `(state, self_idx, target, pool, rng)`.
    after_targeted_spell(on_after_targeted_spell): TargetedSpellFn,
    /// Gold was spent: `(state, self_idx, amount, pool, rng)`.
    gold_spent(on_gold_spent): fn(&mut TavernState, usize, u32, &mut CardPool, &mut Rng),
    /// A minion is being bought (before it reaches the hand): `(state, self_idx, bought)`.
    minion_bought(on_minion_bought): fn(&mut TavernState, usize, &mut Unit),
    /// After any card is bought: `(state, self_idx, bought, pool, rng)`.
    after_buy(on_after_buy): fn(&mut TavernState, usize, &Unit, &mut CardPool, &mut Rng),
    /// After a card is Discovered.
    discover(on_discover): TavernFn,
    /// After a card is discarded from hand: `(state, self_idx, discarded, pool, rng)`.
    after_friendly_discard(on_after_friendly_discard): CardObserverFn,
    /// A card was added to hand (Tavern or Combat): `(self, auras)`.
    card_added_to_hand(on_card_added_to_hand): fn(&Unit, &mut PlayerAuras),
    /// Hero is about to take Tavern damage: `(state, self_idx, amount)`. Whether the damage is
    /// rewound is decided by [`CardFlags::REWINDS_HERO_DAMAGE`].
    hero_damage(on_hero_damage): fn(&mut TavernState, usize, i32),
    /// After the hero took Tavern damage: `(state, self_idx, amount)`. Combat damage triggers
    /// no hero-damage hook (`docs/tavern.md` §7.2).
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
    /// A friendly minion casts a Tavern spell in combat, before the spell takes effect: `(ctx,
    /// self_idx)`. The combat counterpart of `spell_cast` (see
    /// [`cast_tavern_spell_in_combat`](super::cast_tavern_spell_in_combat)).
    combat_spell_cast(on_combat_spell_cast): CombatCastFn,

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

    // ---- Duos: Passes and the teammate (`docs/duos_tavern.md` §7) ------------------------
    /// This card was Passed to the teammate (its own text: "After this is Passed"), as it leaves
    /// the passer's hand.
    passed(on_passed): UnitFn,
    /// Observer on the passer's board: after the player Passes a card: `(state, self_idx,
    /// passed_card, pool, rng)`.
    after_pass(on_after_pass): CardObserverFn,
    /// Observer on both teammates' boards: after the team Passes a card (either teammate); the
    /// team's counters are in `auras.team_passes_this_turn` / `team_passes_this_game`.
    after_team_pass(on_after_team_pass): TavernFn,
    /// Observer on the teammate's board: after the player sold a minion: `(state, self_idx,
    /// sold, pool, rng)`, `state` being the teammate's Tavern.
    after_teammate_sell(on_after_teammate_sell): CardObserverFn,
    /// This card's `Effect` gift (`TeammateGift::Effect`), on a receiving Tavern: `(state,
    /// amount, rng)`. Returns the cards the receiver gets (they arrive like passed cards).
    teammate_gift(on_teammate_gift): fn(&mut TavernState, i32, &mut Rng) -> Vec<Unit>,
    /// This card's `Effect` gift given in a Duos combat, on the receiving warband, at once
    /// (`docs/duos.md` §2): `(warband, amount, rng)`. Without it, the gift waits for the
    /// receiver's Tavern (`teammate_gift`).
    teammate_combat_gift(on_teammate_combat_gift): fn(&mut TeammateWarband<'_>, i32, &mut Rng),

    // ---- Hero Powers (fire for the player whose hero has this Hero Power; `docs/heroes.md`
    // §4) -----------------------------------------------------------------------------------
    /// The `HeroPower` action: `(state, target, pool, rng)`. A Hero Power with this hook is active.
    hero_power(on_hero_power): HeroPowerFn,
    /// Extra conditions for the `HeroPower` action (unlock turns, once per game): `(state)`.
    hero_power_usable(on_hero_power_usable): fn(&TavernState) -> bool,
    /// Which targets the `HeroPower` action can take, on top of its target domain
    /// (`activate_target`): `(state, target)` ("a non-Golden minion", "a minion in the Tavern").
    hero_power_target(on_hero_power_target): fn(&TavernState, usize) -> bool,
    /// Start of turn, after `uses_left` was reset: `(hero)`.
    hero_turn_reset(on_hero_turn_reset): fn(&mut HeroState),
    /// The player got this hero (its start-of-game text): `(state)`.
    hero_chosen(on_hero_chosen): fn(&mut TavernState),
    /// Gold was spent: `(state, amount)`.
    hero_gold_spent(on_hero_gold_spent): fn(&mut TavernState, u32),
    /// Start of Combat, before every other Start of Combat effect: `(ctx)`.
    hero_start_of_combat(on_hero_start_of_combat): fn(&mut HeroCombatCtx<'_>),
    /// Room may have opened on the board in combat (like `player_combat_space`): `(ctx)`.
    hero_combat_space(on_hero_combat_space): fn(&mut BoardCtx<'_>),
    /// A friendly minion killed an enemy minion in combat: `(ctx, killer_id, victim)`.
    hero_enemy_killed(on_hero_enemy_killed): fn(&mut BoardCtx<'_>, UnitId, &Unit),
    /// A friendly minion died in combat, once its side's round of deaths resolved (summons go
    /// to the right end): `(ctx, dying)`.
    hero_friendly_death(on_hero_friendly_death): fn(&mut BoardCtx<'_>, &Unit),
    /// Duos: the player's warband never stepped in, once the combat is over: `(auras, hero_tier,
    /// rng)`. A card it returns goes to the warband's hand if it has room.
    hero_sat_out(on_hero_sat_out): fn(&mut PlayerAuras, u32, &mut Rng) -> Option<Unit>,
    /// A friendly minion is being summoned in combat: `(hero, summoned)`.
    hero_combat_summon(on_hero_combat_summon): fn(&HeroState, &mut Unit),
    /// Start of turn, once the shop is filled: `(state, pool, rng)`.
    hero_turn_start(on_hero_turn_start): TavernEventFn,
    /// After a card is bought (once its own text and the `after_buy` observers resolved):
    /// `(state, bought, pool, rng)`.
    hero_after_buy(on_hero_after_buy): CardEventFn,
    /// After a minion is sold (once its sell triggers resolved): `(state, sold, pool, rng)`.
    hero_after_sell(on_hero_after_sell): CardEventFn,
    /// After a minion is played or Magnetized (once the `after_friendly_play` observers
    /// resolved): `(state, played, pool, rng)`.
    hero_after_play(on_hero_after_play): fn(&mut TavernState, &Played, &mut CardPool, &mut Rng),
    /// After the Tavern is upgraded: `(state, pool, rng)`.
    hero_after_upgrade(on_hero_after_upgrade): TavernEventFn,
    /// The Tavern was Refreshed (not a frozen shop kept), before the cards' after-Refresh
    /// effects: `(state, pool, rng)`.
    hero_after_refresh(on_hero_after_refresh): TavernEventFn,
    /// The player ends their turn, after the cards' end-of-turn triggers: `(state, pool, rng)`.
    hero_end_turn(on_hero_end_turn): TavernEventFn,
    /// The player chose an option of a Discover the Hero Power keeps (`Unit::to_hero_power`):
    /// `(state, chosen, unchosen, pool, rng)`.
    hero_option_chosen(on_hero_option_chosen): fn(&mut TavernState, Unit, Vec<Unit>, &mut CardPool, &mut Rng),
    /// The combat's results are back in the Tavern: `(state, dead)`, the player's minions that
    /// died in it.
    hero_after_combat(on_hero_after_combat): fn(&mut TavernState, &[Unit]),
    /// A friendly minion attacks (in combat or in the Tavern), once the friendly-attack
    /// observers resolved: `(ctx, attacker_id)`.
    hero_friendly_attack(on_hero_friendly_attack): fn(&mut BoardCtx<'_>, UnitId),
    /// A friendly minion dealt damage (in combat or in the Tavern), after the cards'
    /// `on_damage_dealt` observers: `(ctx, dealer_id, amount)`.
    hero_friendly_damage_dealt(on_hero_friendly_damage_dealt): fn(&mut BoardCtx<'_>, UnitId, i32),
    /// The player's board stats or minions changed (`TavernState::sync_all_auras`): `(state)`.
    hero_board_changed(on_hero_board_changed): fn(&mut TavernState),
    /// The Tavern's prices and rules the Hero Power changes, whenever they are read:
    /// `(rules)` (`docs/tavern.md` §3.3).
    hero_tavern_rules(on_hero_tavern_rules): fn(&mut crate::heroes::TavernRules),
    /// Whether the player skips the current turn (only `EndTurn` and `ChooseDiscover` are
    /// legal): `(state)`.
    hero_skips_turn(on_hero_skips_turn): fn(&TavernState) -> bool,
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

    pub fn with_spell_target(mut self, target: SpellTarget) -> Self {
        self.spell_target = target;
        self
    }

    /// The target domain of this card's `Activate` ability, or of this Hero Power
    /// (`docs/heroes.md` §3.3).
    pub fn with_activate_target(mut self, target: ActivateTargetKind) -> Self {
        self.activate_target = target;
        self
    }

    /// Contribution of `unit` to `passive` (0 if this card has no such passive).
    #[inline]
    pub fn passive_of(&self, unit: &Unit, passive: Passive) -> u32 {
        self.passive.map_or(0, |f| f(unit, passive))
    }
}
