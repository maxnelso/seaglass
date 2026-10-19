//! Match replays (`docs/replays.md`).
//!
//! A replay stores a match's *inputs*: its setup, its seeds and every engine call in order, as a
//! protobuf message ([`pb::Replay`], `proto/replay.proto`). A [`Match`](crate::game::Match)
//! records one as it runs, and a [`Timeline`] re-simulates one to rebuild every state.
//!
//! Replays only re-simulate faithfully with the engine build that recorded them
//! ([`pb::Header::engine`]). A [`Timeline`] flags every call whose re-simulated result differs from
//! the recorded one.

mod timeline;

use std::path::Path;

use prost::Message;

use crate::model::{BattleOutcome, CardId, DeityKind, Keyword};
use crate::tavern::{TavernAction, TavernState};

pub use timeline::{Decision, Divergence, Frame, FrameKind, ReplayError, Timeline};

/// The protobuf types generated from `proto/replay.proto`.
#[allow(clippy::all, missing_docs)]
pub mod pb {
    include!(concat!(env!("OUT_DIR"), "/seaglass.replay.rs"));
}

/// The replay format version this engine reads and writes ([`pb::Header::format`]).
pub const FORMAT_VERSION: u32 = 2;

/// This engine build: its crate version and git commit (`0.1.0+708acd9e1b2c`).
pub fn engine_version() -> &'static str {
    concat!(env!("CARGO_PKG_VERSION"), "+", env!("SEAGLASS_GIT_COMMIT"))
}

/// Encode a replay as protobuf bytes.
pub fn encode(replay: &pb::Replay) -> Vec<u8> {
    replay.encode_to_vec()
}

/// Decode a replay from protobuf bytes.
pub fn decode(bytes: &[u8]) -> Result<pb::Replay, String> {
    pb::Replay::decode(bytes).map_err(|e| format!("not a replay: {e}"))
}

/// Write a replay file, creating its parent directories. The file is written to a temporary
/// path first and then renamed, so an interrupted save never leaves a truncated replay.
pub fn save(replay: &pb::Replay, path: impl AsRef<Path>) -> std::io::Result<()> {
    let path = path.as_ref();
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    std::fs::write(&tmp, encode(replay))?;
    std::fs::rename(&tmp, path)
}

/// Read a replay file.
pub fn load(path: impl AsRef<Path>) -> Result<pb::Replay, String> {
    let path = path.as_ref();
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    decode(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// The protobuf form of a [`TavernAction`].
pub fn action_to_pb(action: &TavernAction) -> pb::Action {
    use pb::action::Kind;
    let kind = match *action {
        TavernAction::Buy { shop_index } => Kind::Buy(shop_index as u32),
        TavernAction::Play {
            hand_index,
            board_pos,
        } => Kind::Play(pb::Play {
            hand: hand_index as u32,
            pos: board_pos as u32,
        }),
        TavernAction::Sell { board_pos } => Kind::Sell(board_pos as u32),
        TavernAction::Reposition { from_pos, to_pos } => Kind::Reposition(pb::Reposition {
            from: from_pos as u32,
            to: to_pos as u32,
        }),
        TavernAction::Activate {
            board_pos,
            target_pos,
        } => Kind::Activate(pb::Activate {
            pos: board_pos as u32,
            target: target_pos.map(|t| t as u32),
        }),
        TavernAction::Refresh => Kind::Refresh(pb::Empty {}),
        TavernAction::UpgradeTavern => Kind::UpgradeTavern(pb::Empty {}),
        TavernAction::ToggleFreeze => Kind::ToggleFreeze(pb::Empty {}),
        TavernAction::ChooseDiscover { option_index } => Kind::ChooseDiscover(option_index as u32),
        TavernAction::EndTurn => Kind::EndTurn(pb::Empty {}),
        TavernAction::PlayOnShop {
            hand_index,
            shop_index,
        } => Kind::PlayOnShop(pb::PlayOnShop {
            hand: hand_index as u32,
            shop: shop_index as u32,
        }),
        TavernAction::Pass { hand_index } => Kind::Pass(hand_index as u32),
        TavernAction::HeroPower { target } => Kind::HeroPower(pb::Target {
            target: target.map(|t| t as u32),
        }),
        TavernAction::ChooseHero { option_index } => Kind::ChooseHero(option_index as u32),
        TavernAction::HeroPower2 { target } => Kind::HeroPower2(pb::Target {
            target: target.map(|t| t as u32),
        }),
        TavernAction::ChooseTrinket { option_index } => Kind::ChooseTrinket(option_index as u32),
        TavernAction::DarkDiscovery => Kind::DarkDiscovery(pb::Empty {}),
    };
    pb::Action { kind: Some(kind) }
}

/// The [`TavernAction`] a protobuf action encodes.
pub fn action_from_pb(action: &pb::Action) -> Result<TavernAction, String> {
    use pb::action::Kind;
    let kind = action.kind.as_ref().ok_or("the action is empty")?;
    Ok(match kind {
        Kind::Buy(i) => TavernAction::Buy {
            shop_index: *i as usize,
        },
        Kind::Play(p) => TavernAction::Play {
            hand_index: p.hand as usize,
            board_pos: p.pos as usize,
        },
        Kind::Sell(i) => TavernAction::Sell {
            board_pos: *i as usize,
        },
        Kind::Reposition(r) => TavernAction::Reposition {
            from_pos: r.from as usize,
            to_pos: r.to as usize,
        },
        Kind::Activate(a) => TavernAction::Activate {
            board_pos: a.pos as usize,
            target_pos: a.target.map(|t| t as usize),
        },
        Kind::Refresh(_) => TavernAction::Refresh,
        Kind::UpgradeTavern(_) => TavernAction::UpgradeTavern,
        Kind::ToggleFreeze(_) => TavernAction::ToggleFreeze,
        Kind::ChooseDiscover(i) => TavernAction::ChooseDiscover {
            option_index: *i as usize,
        },
        Kind::EndTurn(_) => TavernAction::EndTurn,
        Kind::PlayOnShop(p) => TavernAction::PlayOnShop {
            hand_index: p.hand as usize,
            shop_index: p.shop as usize,
        },
        Kind::Pass(i) => TavernAction::Pass {
            hand_index: *i as usize,
        },
        Kind::HeroPower(t) => TavernAction::HeroPower {
            target: t.target.map(|t| t as usize),
        },
        Kind::ChooseHero(i) => TavernAction::ChooseHero {
            option_index: *i as usize,
        },
        Kind::HeroPower2(t) => TavernAction::HeroPower2 {
            target: t.target.map(|t| t as usize),
        },
        Kind::ChooseTrinket(i) => TavernAction::ChooseTrinket {
            option_index: *i as usize,
        },
        Kind::DarkDiscovery(_) => TavernAction::DarkDiscovery,
    })
}

/// The protobuf form of a [`DeityKind`].
pub fn deity_to_pb(kind: DeityKind) -> pb::Deity {
    match kind {
        DeityKind::None => pb::Deity::None,
        DeityKind::CThun => pb::Deity::Cthun,
        DeityKind::YShaarj => pb::Deity::Yshaarj,
    }
}

/// The [`DeityKind`] a protobuf Deity encodes.
pub fn deity_from_pb(deity: pb::Deity) -> DeityKind {
    match deity {
        pb::Deity::None => DeityKind::None,
        pb::Deity::Cthun => DeityKind::CThun,
        pb::Deity::Yshaarj => DeityKind::YShaarj,
    }
}

/// The protobuf form of a [`BattleOutcome`].
pub fn outcome_to_pb(outcome: BattleOutcome) -> pb::Outcome {
    match outcome {
        BattleOutcome::AWin => pb::Outcome::AWin,
        BattleOutcome::BWin => pb::Outcome::BWin,
        BattleOutcome::Draw => pb::Outcome::Draw,
    }
}

/// The card `action` uses in `state`: the card it buys, plays, casts, sells, repositions,
/// activates, passes or chooses ([`pb::Act::card_id`]). `None` for actions without a card (a hero,
/// trinket or Hero Power is not a card).
pub fn action_card(state: &TavernState, action: &TavernAction) -> Option<CardId> {
    let unit = match *action {
        TavernAction::Buy { shop_index } => state.shop.get(shop_index),
        TavernAction::Play { hand_index, .. }
        | TavernAction::PlayOnShop { hand_index, .. }
        | TavernAction::Pass { hand_index } => state.hand.get(hand_index),
        TavernAction::Sell { board_pos } | TavernAction::Activate { board_pos, .. } => {
            state.board.get(board_pos)
        }
        TavernAction::Reposition { from_pos, .. } => state.board.get(from_pos),
        TavernAction::ChooseDiscover { option_index } => state
            .discover_pending
            .as_ref()
            .and_then(|options| options.get(option_index)),
        TavernAction::Refresh
        | TavernAction::UpgradeTavern
        | TavernAction::ToggleFreeze
        | TavernAction::EndTurn
        | TavernAction::HeroPower { .. }
        | TavernAction::HeroPower2 { .. }
        | TavernAction::ChooseHero { .. }
        | TavernAction::ChooseTrinket { .. }
        | TavernAction::DarkDiscovery => None,
    };
    unit.map(|u| u.card_id)
}

/// A summary of a player's state, compared when a replay is re-simulated.
pub fn checkpoint(state: &TavernState) -> pb::Checkpoint {
    pb::Checkpoint {
        health: state.health,
        armor: state.armor,
        tier: state.tavern_tier,
        fingerprint: fingerprint(state),
    }
}

const KEYWORDS: [Keyword; 7] = [
    Keyword::Taunt,
    Keyword::DivineShield,
    Keyword::Windfury,
    Keyword::Reborn,
    Keyword::Venomous,
    Keyword::Stealth,
    Keyword::Magnetic,
];

/// A 64-bit FNV-1a hash of a player's board and hand: each unit's card, Attack, Health, keywords
/// and Golden flag ([`pb::Checkpoint::fingerprint`]).
pub fn fingerprint(state: &TavernState) -> u64 {
    let mut hash = Fnv1a::new();
    for zone in [&state.board, &state.hand] {
        hash.write(&(zone.len() as u32).to_le_bytes());
        for unit in zone {
            let keywords = KEYWORDS
                .iter()
                .enumerate()
                .filter(|(_, &kw)| unit.has_keyword(kw))
                .fold(0u8, |bits, (i, _)| bits | (1 << i));
            hash.write(&unit.card_id.to_le_bytes());
            hash.write(&unit.attack.to_le_bytes());
            hash.write(&unit.health.to_le_bytes());
            hash.write(&[keywords, unit.is_golden as u8]);
        }
    }
    hash.0
}

struct Fnv1a(u64);

impl Fnv1a {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= byte as u64;
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}
