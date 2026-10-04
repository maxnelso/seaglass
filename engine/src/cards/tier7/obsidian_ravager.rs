//! `Obsidian Ravager` (`BG27_017`) — Tier 7 Dragon (`7/7`).
//!
//! Rally: Deal damage equal to this minion's Attack to the target and an adjacent minion
//! (to the target and its neighbors if Golden).

use crate::cards::{self, CardTemplate};
use crate::events::Event;
use crate::model::{CardId, Tribe, Unit};
use crate::rng::Rng;

pub const ID: CardId = 706;

pub fn template() -> CardTemplate {
    CardTemplate::new(ID, "Obsidian Ravager", 7, 7, 7)
        .with_tribe(Tribe::Dragon)
        .on_rally(|c| {
            if let Some((def_board, def_pos)) = c.def_target.as_mut() {
                on_rally(
                    &c.board[c.attacker_pos],
                    def_board,
                    *def_pos,
                    c.rng,
                    c.events,
                );
            }
            Vec::new()
        })
}

pub fn on_rally(
    attacker: &Unit,
    def_board: &mut [Unit],
    def_pos: usize,
    rng: &mut Rng,
    events: &mut Vec<Event>,
) {
    let dmg = attacker.attack.max(0);
    if dmg == 0 || def_pos >= def_board.len() {
        return;
    }

    let mut targets = vec![def_pos];
    let mut neighbors = Vec::new();
    if def_pos > 0 && def_board[def_pos - 1].health > 0 {
        neighbors.push(def_pos - 1);
    }
    if def_pos + 1 < def_board.len() && def_board[def_pos + 1].health > 0 {
        neighbors.push(def_pos + 1);
    }

    if attacker.is_golden {
        targets.extend(neighbors);
    } else if !neighbors.is_empty() {
        let pick = if neighbors.len() == 1 {
            0
        } else {
            rng.below(neighbors.len())
        };
        targets.push(neighbors[pick]);
    }

    for idx in targets {
        if def_board[idx].health <= 0 {
            continue;
        }
        if def_board[idx].divine_shield {
            def_board[idx].divine_shield = false;
            events.push(Event::DivineShieldPopped {
                unit: def_board[idx].id,
            });
            cards::tier5::hopebringer::on_friendly_divine_shield_lost(def_board);
        } else {
            let target = &mut def_board[idx];
            target.health -= dmg;
            events.push(Event::DamageDealt {
                unit: target.id,
                amount: dmg,
                from: attacker.id,
            });
            if target.health <= 0 {
                target.killed_by = Some(attacker.id);
            }
        }
    }
}
