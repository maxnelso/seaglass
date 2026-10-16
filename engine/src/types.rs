//! Counting and choosing "different minion types": the Menagerie rule (`docs/tavern.md` §7.8,
//! confirmed). Each minion stands for one of its types at most (a maximum matching of minions to
//! types over [`SINGLE_TRIBES`], Naga included); an All-type minion can take any type.

use crate::model::{Unit, SINGLE_TRIBES};
use crate::rng::Rng;

/// Each minion's types on `board`, as bits over [`SINGLE_TRIBES`] (none for a dead minion).
fn type_bits(board: &[Unit]) -> Vec<u16> {
    board
        .iter()
        .map(|u| {
            SINGLE_TRIBES
                .iter()
                .enumerate()
                .filter(|&(_, &tribe)| u.health > 0 && u.tribe.has_single(tribe))
                .fold(0, |bits, (t, _)| bits | 1 << t)
        })
        .collect()
}

/// Give minion `i` one of its `types`, moving the minion holding that type (`holder`, by type) to
/// another of its types if needed (Kuhn's augmenting path). `tried` holds the types already tried.
fn give_type(i: usize, types: &[u16], holder: &mut [Option<usize>], tried: &mut u16) -> bool {
    for t in 0..SINGLE_TRIBES.len() {
        let bit = 1 << t;
        if types[i] & bit == 0 || *tried & bit != 0 {
            continue;
        }
        *tried |= bit;
        if holder[t].is_none_or(|j| give_type(j, types, holder, tried)) {
            holder[t] = Some(i);
            return true;
        }
    }
    false
}

/// How many of the minions in `set` (bits over board indices) can each take a different type:
/// the size of a maximum matching of those minions to types.
fn matched_count(types: &[u16], set: u32) -> usize {
    let mut holder = [None; SINGLE_TRIBES.len()];
    (0..types.len())
        .filter(|&i| set >> i & 1 == 1 && give_type(i, types, &mut holder, &mut 0))
        .count()
}

/// How many ways the minions in `set` can each take a different type. All-type minions take any
/// of the types the others leave, so only the others' (at most 2 types each) are enumerated.
fn assignment_count(types: &[u16], set: u32) -> u64 {
    fn count(minions: &[usize], types: &[u16], used: u16) -> u64 {
        let Some((&i, rest)) = minions.split_first() else {
            return 1;
        };
        (0..SINGLE_TRIBES.len())
            .map(|t| 1 << t)
            .filter(|&bit| types[i] & !used & bit != 0)
            .map(|bit| count(rest, types, used | bit))
            .sum()
    }
    const ALL_TYPES: u16 = (1 << SINGLE_TRIBES.len()) - 1;
    let (all_type, others): (Vec<usize>, Vec<usize>) = (0..types.len())
        .filter(|&i| set >> i & 1 == 1)
        .partition(|&i| types[i] == ALL_TYPES);
    let left = SINGLE_TRIBES.len().saturating_sub(others.len());
    let all_type_ways: u64 = (0..all_type.len())
        .map(|k| left.saturating_sub(k) as u64)
        .product();
    count(&others, types, 0) * all_type_ways
}

/// "How many different types" (`docs/tavern.md` §7.8, the Menagerie rule): the most types the
/// living minions of `board` can stand for at once, each minion for one type at most (a maximum
/// matching of minions to types; an All-type minion can take any type). `Menagerie Tableware`,
/// `Multilayered Shield`, `Glowing Crystal`.
pub fn menagerie_type_count(board: &[Unit]) -> usize {
    let everyone = (0..board.len()).fold(0u32, |set, i| set | 1 << i);
    matched_count(&type_bits(board), everyone)
}

/// "A minion of each type" (`docs/tavern.md` §7.8, the Menagerie rule): the indices (in board
/// order) of the living minions of `board` in the best assignment of minions to types, each minion
/// for one type at most (a maximum matching; an All-type minion can take any type). Among the best
/// assignments, one is chosen uniformly at random (one `rng.below` when there are several).
/// `Misplaced Tea Set`, `The Last One Standing`, Queen Wagtoggle, `Aggem Sticker`.
pub fn select_menagerie_targets(board: &[Unit], rng: &mut Rng) -> Vec<usize> {
    let types = type_bits(board);
    let eligible = (0..board.len())
        .filter(|&i| types[i] != 0)
        .fold(0u32, |set, i| set | 1 << i);
    let best = matched_count(&types, eligible);
    let mut chosen = eligible;
    if best < eligible.count_ones() as usize {
        // The sets of `best` minions that can each take a different type, weighted by how many
        // best assignments give each one.
        let mut options: Vec<(u32, u64)> = Vec::new();
        let mut set = eligible;
        while set != 0 {
            if set.count_ones() as usize == best && matched_count(&types, set) == best {
                options.push((set, assignment_count(&types, set)));
            }
            set = (set - 1) & eligible;
        }
        chosen = options[0].0;
        if options.len() > 1 {
            let total: u64 = options.iter().map(|&(_, ways)| ways).sum();
            let mut pick = rng.below(total as usize) as u64;
            for &(set, ways) in &options {
                if pick < ways {
                    chosen = set;
                    break;
                }
                pick -= ways;
            }
        }
    }
    (0..board.len()).filter(|&i| chosen >> i & 1 == 1).collect()
}
