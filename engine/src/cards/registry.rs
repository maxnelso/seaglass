//! Global card registry: `CardId` → [`CardTemplate`] and [`CardHooks`].
//!
//! Built once from the full minion catalog and the Duos-exclusive minions, plus the behaviour
//! tables exported by the token, spell, and Deity modules.

use std::sync::OnceLock;

use super::hooks::CardHooks;
use super::{
    deities, duos_exclusive_catalog, duos_out_of_pool_catalog, full_catalog, spells, tokens,
    CardTemplate,
};
use crate::model::{CardId, PlayerAuras, Unit};

const NO_SLOT: u16 = u16::MAX;

struct Registry {
    /// The full Solo catalog, then the Duos-exclusive minions of the Duos pool (from `solo_len`
    /// on), then those in no pool (from `duos_len` on: the Naga-only ones).
    templates: Vec<CardTemplate>,
    solo_len: usize,
    duos_len: usize,
    template_slot: Vec<u16>,
    hooks: Vec<CardHooks>,
    hook_slot: Vec<u16>,
}

static REGISTRY: OnceLock<Registry> = OnceLock::new();
static NO_HOOKS: CardHooks = CardHooks::EMPTY;

fn set_slot(slots: &mut Vec<u16>, card_id: CardId, idx: usize, what: &str) {
    let id = card_id as usize;
    if slots.len() <= id {
        slots.resize(id + 1, NO_SLOT);
    }
    assert!(
        slots[id] == NO_SLOT,
        "duplicate {what} registration for card id {card_id}"
    );
    slots[id] = u16::try_from(idx).expect("registry too large");
}

fn build() -> Registry {
    let mut templates = full_catalog();
    let solo_len = templates.len();
    templates.extend(duos_exclusive_catalog());
    let duos_len = templates.len();
    templates.extend(duos_out_of_pool_catalog());
    let mut template_slot = Vec::new();
    let mut hooks = Vec::new();
    let mut hook_slot = Vec::new();
    for (i, t) in templates.iter().enumerate() {
        set_slot(&mut template_slot, t.card_id, i, "template");
        set_slot(&mut hook_slot, t.card_id, hooks.len(), "hooks");
        hooks.push(t.hooks);
    }
    let extra = tokens::behaviors()
        .into_iter()
        .chain(spells::behaviors())
        .chain(deities::behaviors());
    for (card_id, h) in extra {
        set_slot(&mut hook_slot, card_id, hooks.len(), "hooks");
        hooks.push(h);
    }
    Registry {
        templates,
        solo_len,
        duos_len,
        template_slot,
        hooks,
        hook_slot,
    }
}

#[inline]
fn registry() -> &'static Registry {
    REGISTRY.get_or_init(build)
}

/// Behaviour table for `card_id` (an empty table for unknown ids and plain units).
#[inline]
pub fn hooks(card_id: CardId) -> &'static CardHooks {
    #[cfg(feature = "knockout")]
    if knocked_out() == Some(card_id) {
        return &NO_HOOKS;
    }
    let r = registry();
    match r.hook_slot.get(card_id as usize) {
        Some(&slot) if slot != NO_SLOT => &r.hooks[slot as usize],
        _ => &NO_HOOKS,
    }
}

/// Test tooling (`--features knockout`): the card whose hooks are disabled, read from the
/// `SEAGLASS_KNOCKOUT` environment variable. A card's scenarios should fail with its hooks
/// knocked out; if they still pass, they don't test its behaviour (`docs/scenarios.md`).
#[cfg(feature = "knockout")]
fn knocked_out() -> Option<CardId> {
    static KNOCKED_OUT: OnceLock<Option<CardId>> = OnceLock::new();
    *KNOCKED_OUT.get_or_init(|| {
        std::env::var("SEAGLASS_KNOCKOUT")
            .ok()
            .map(|v| v.parse().expect("SEAGLASS_KNOCKOUT must be a card id"))
    })
}

/// Catalog template for the minion `card_id`, if it is a shop minion (Solo or Duos-exclusive).
#[inline]
pub fn template(card_id: CardId) -> Option<&'static CardTemplate> {
    let r = registry();
    match r.template_slot.get(card_id as usize) {
        Some(&slot) if slot != NO_SLOT => Some(&r.templates[slot as usize]),
        _ => None,
    }
}

/// A fresh, unbuffed, non-Golden instance of `unit`'s card: its catalog template, or the plain
/// version of a token. `None` if the card is neither.
pub fn plain_instance(unit: &Unit) -> Option<Unit> {
    match template(unit.card_id) {
        Some(tpl) => Some(tpl.instantiate()),
        None => tokens::make_plain_token(unit, &PlayerAuras::default()),
    }
}

/// All Solo catalog templates (Tier 1..=7), in catalog order: what a Solo player's effects that
/// generate random minions draw from (`docs/duos_tavern.md` §8; see `cards::game_templates`).
pub fn all_templates() -> &'static [CardTemplate] {
    let r = registry();
    &r.templates[..r.solo_len]
}

/// The Duos-exclusive minion templates of the Duos pool, in catalog order
/// (`docs/duos_tavern.md` §8).
pub fn duos_exclusive_templates() -> &'static [CardTemplate] {
    let r = registry();
    &r.templates[r.solo_len..r.duos_len]
}

/// The Duos catalog's templates: the Solo catalog, then the Duos-exclusive minions of the Duos
/// pool. What a Duos player's effects that generate random minions draw from.
pub fn duos_templates() -> &'static [CardTemplate] {
    let r = registry();
    &r.templates[..r.duos_len]
}

/// The Duos-exclusive minion templates in no pool, in catalog order (the Naga-only ones:
/// `docs/duos_tavern.md` §8). Registered like any minion, so unit specs can name them.
pub fn duos_out_of_pool_templates() -> &'static [CardTemplate] {
    let r = registry();
    &r.templates[r.duos_len..]
}
