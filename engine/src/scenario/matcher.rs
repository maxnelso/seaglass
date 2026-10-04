//! Expectations (`docs/scenarios.md` §5): a YAML matcher checked against a [`View`].
//!
//! - A mapping matches a map or unit if each of its fields matches (other fields are not
//!   checked); keyed by card name, it matches a map of cards.
//! - A mapping matches a list through operators: `where` (filter first), `count`, `all`,
//!   `any`, `none`, `contains` (distinct items), `sequence` (an ordered subsequence),
//!   `sum: {field: m}` and indices (`0`, `-1`).
//! - A sequence matches a list of the same length, item by item.
//! - A string matches a unit as a unit spec (`"Joyous 3/2 taunt"`, see [`units::check`]).
//! - A string matches a number as a comparison or range: `">= 3"`, `"!= 0"`, `"1..4"`,
//!   `"0.4..=1"`, where each bound is a sum of numbers, `$capture`s and `@field`s of the
//!   enclosing object (`"@max_gold - 2"`).
//! - Other scalars match by equality.
//! - Single-key combinators: `all_of: [m...]`, `any_of: [m...]`, `not: m`, `capture: name`
//!   (remember the value) and `same_as: name` (equal to a captured value).
//!
//! An expectation that cannot apply (an unknown field or operator, a string against a list,
//! ...) is [`Failure::Invalid`]; one that applies but does not hold is [`Failure::Mismatch`].

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde_yaml::{Mapping, Value};

use super::patch::{join, key_name};
use super::units::{self, card_index};
use super::view::{show, View};
use super::Failure;

/// Values remembered by `capture: <name>`, read by `same_as: <name>` and `$<name>`.
pub type Captures = BTreeMap<String, View>;

const COMBINATORS: [&str; 5] = ["all_of", "any_of", "not", "capture", "same_as"];

const LIST_OPERATORS: &str = "where, count, all, any, none, contains, sequence, sum, or an index";

/// Check `actual` against the matcher `expected`, recording captures.
pub fn check(expected: &Value, actual: &View, captures: &mut Captures) -> Result<(), Failure> {
    Matcher { captures }.check(expected, actual, None, "")
}

/// Whether `actual` matches `expected`, without recording captures (an invalid matcher is
/// still an error).
pub fn probe(expected: &Value, actual: &View, captures: &Captures) -> Result<bool, Failure> {
    let mut scratch = captures.clone();
    outcome(
        Matcher {
            captures: &mut scratch,
        }
        .check(expected, actual, None, ""),
    )
}

fn outcome(result: Result<(), Failure>) -> Result<bool, Failure> {
    match result {
        Ok(()) => Ok(true),
        Err(Failure::Mismatch(_)) => Ok(false),
        Err(invalid) => Err(invalid),
    }
}

/// `"path: "`, or nothing at the root.
fn at(path: &str) -> String {
    if path.is_empty() {
        String::new()
    } else {
        format!("{path}: ")
    }
}

fn invalid(path: &str, message: impl std::fmt::Display) -> Failure {
    Failure::Invalid(format!("{}{message}", at(path)))
}

fn mismatch(path: &str, expected: &Value, actual: &View) -> Failure {
    Failure::Mismatch(format!(
        "{}expected {}, got {}",
        at(path),
        show(expected),
        actual.short()
    ))
}

/// The combinator a matcher is, if it is one: a single-key mapping such as `{not: ...}`.
fn combinator(expected: &Value) -> Option<(&'static str, &Value)> {
    let m = expected.as_mapping().filter(|m| m.len() == 1)?;
    let (key, arg) = m.iter().next()?;
    let key = key.as_str()?;
    COMBINATORS.iter().find(|&&c| c == key).map(|&c| (c, arg))
}

struct Matcher<'c> {
    captures: &'c mut Captures,
}

impl Matcher<'_> {
    /// Whether `actual` matches, without recording captures.
    fn probe(
        &self,
        expected: &Value,
        actual: &View,
        parent: Option<&View>,
        path: &str,
    ) -> Result<bool, Failure> {
        let mut scratch = self.captures.clone();
        outcome(
            Matcher {
                captures: &mut scratch,
            }
            .check(expected, actual, parent, path),
        )
    }

    /// Check `actual` (at `path`, a field of `parent`) against `expected`.
    fn check(
        &mut self,
        expected: &Value,
        actual: &View,
        parent: Option<&View>,
        path: &str,
    ) -> Result<(), Failure> {
        if let Some((op, arg)) = combinator(expected) {
            return self.combinator(op, arg, actual, parent, path);
        }
        match (expected, actual) {
            (Value::Tagged(tagged), _) => Err(invalid(
                path,
                format!(
                    "YAML tags are not matchers: match `{}` as a map, `{{{}: ...}}`",
                    show(expected),
                    tagged.tag.to_string().trim_start_matches('!')
                ),
            )),
            (Value::Null, View::Null) => Ok(()),
            (_, View::Null) | (Value::Null, _) => Err(mismatch(path, expected, actual)),
            (Value::Mapping(m), View::Map(_) | View::Unit(..)) => self.fields(m, actual, path),
            (Value::Mapping(m), View::CardMap(entries, default)) => {
                self.cards(m, entries, default.as_deref(), actual, path)
            }
            (Value::Mapping(m), View::List(items)) => self.list(m, items, parent, path),
            (Value::Sequence(expected_items), View::List(items)) => {
                if expected_items.len() != items.len() {
                    return Err(Failure::Mismatch(format!(
                        "{}expected {} item(s), got {}: {}",
                        at(path),
                        expected_items.len(),
                        items.len(),
                        actual.short()
                    )));
                }
                for (i, (e, item)) in expected_items.iter().zip(items).enumerate() {
                    self.check(e, item, parent, &format!("{path}[{i}]"))?;
                }
                Ok(())
            }
            (Value::String(spec), View::Unit(unit, _)) => {
                units::check(spec, unit).map_err(|f| f.prefixed(&at(path)))
            }
            (Value::Bool(want), View::Bool(got)) => {
                if want == got {
                    Ok(())
                } else {
                    Err(mismatch(path, expected, actual))
                }
            }
            (Value::Number(n), View::Int(_) | View::Float(_)) => {
                let want =
                    number_of_yaml(n).ok_or_else(|| invalid(path, format!("bad number {n}")))?;
                if compare(number(actual), want) == Some(Ordering::Equal) {
                    Ok(())
                } else {
                    Err(mismatch(path, expected, actual))
                }
            }
            (Value::String(condition), View::Int(_) | View::Float(_)) => {
                self.condition(condition, actual, parent, path)
            }
            (Value::String(want), View::Str(got)) => {
                if want == got {
                    Ok(())
                } else {
                    Err(mismatch(path, expected, actual))
                }
            }
            _ => Err(invalid(
                path,
                format!(
                    "cannot match {} against {} ({})",
                    show(expected),
                    actual.kind(),
                    actual.short()
                ),
            )),
        }
    }

    fn combinator(
        &mut self,
        op: &str,
        arg: &Value,
        actual: &View,
        parent: Option<&View>,
        path: &str,
    ) -> Result<(), Failure> {
        let list = || {
            arg.as_sequence().ok_or_else(|| {
                invalid(
                    path,
                    format!("`{op}` takes a list of matchers, got {}", show(arg)),
                )
            })
        };
        let name = || {
            arg.as_str()
                .filter(|s| {
                    !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                })
                .ok_or_else(|| {
                    invalid(
                        path,
                        format!("`{op}` takes a name ([A-Za-z0-9_]+), got {}", show(arg)),
                    )
                })
        };
        match op {
            "all_of" => {
                for m in list()? {
                    self.check(m, actual, parent, path)?;
                }
                Ok(())
            }
            "any_of" => {
                let options = list()?;
                for m in options {
                    if self.probe(m, actual, parent, path)? {
                        return self.check(m, actual, parent, path);
                    }
                }
                Err(Failure::Mismatch(format!(
                    "{}expected any of {}, got {}",
                    at(path),
                    show(arg),
                    actual.short()
                )))
            }
            "not" => {
                if self.probe(arg, actual, parent, path)? {
                    Err(Failure::Mismatch(format!(
                        "{}expected not {}, got {}",
                        at(path),
                        show(arg),
                        actual.short()
                    )))
                } else {
                    Ok(())
                }
            }
            "capture" => {
                self.captures.insert(name()?.to_string(), actual.clone());
                Ok(())
            }
            "same_as" => {
                let name = name()?;
                let captured = self
                    .captures
                    .get(name)
                    .ok_or_else(|| self.no_capture(name, path))?;
                if captured == actual {
                    Ok(())
                } else {
                    Err(Failure::Mismatch(format!(
                        "{}expected the same as ${name} ({}), got {}",
                        at(path),
                        captured.short(),
                        actual.short()
                    )))
                }
            }
            _ => unreachable!("combinator {op}"),
        }
    }

    fn no_capture(&self, name: &str, path: &str) -> Failure {
        let names: Vec<&str> = self.captures.keys().map(String::as_str).collect();
        invalid(
            path,
            format!(
                "nothing captured as `{name}` (captures: {})",
                if names.is_empty() {
                    "none".to_string()
                } else {
                    names.join(", ")
                }
            ),
        )
    }

    /// A mapping against a map or unit: each field must match.
    fn fields(&mut self, expected: &Mapping, actual: &View, path: &str) -> Result<(), Failure> {
        for (key, e) in expected {
            let name = key_name(key);
            let field_path = join(path, &name);
            let Some(value) = actual.field(&name) else {
                let keys = actual.keys();
                let close = units::closest(&name, keys.iter().copied());
                return Err(Failure::Invalid(if close.is_empty() {
                    format!("unknown field `{field_path}` (fields: {})", keys.join(", "))
                } else {
                    format!(
                        "unknown field `{field_path}` (did you mean `{}`?)",
                        close.join("`, `")
                    )
                }));
            };
            self.check(e, value, Some(actual), &field_path)?;
        }
        Ok(())
    }

    /// A mapping keyed by card name (or id) against a map of cards.
    fn cards(
        &mut self,
        expected: &Mapping,
        entries: &[(String, View)],
        default: Option<&View>,
        actual: &View,
        path: &str,
    ) -> Result<(), Failure> {
        for (key, e) in expected {
            let name = match key {
                Value::Number(n) => match n.as_u64().and_then(|id| u32::try_from(id).ok()) {
                    Some(id) => super::view::card_name(id),
                    None => return Err(invalid(path, format!("{n} is not a card id"))),
                },
                Value::String(s) => card_index()
                    .resolve(s)
                    .map_err(|e| invalid(path, e))?
                    .name
                    .clone(),
                other => {
                    return Err(invalid(
                        path,
                        format!("expected a card name, got {}", show(other)),
                    ))
                }
            };
            let entry_path = join(path, &name);
            match entries.iter().find(|(k, _)| *k == name) {
                Some((_, value)) => self.check(e, value, Some(actual), &entry_path)?,
                None => match default {
                    Some(value) => self.check(e, value, Some(actual), &entry_path)?,
                    None => {
                        return Err(invalid(
                            path,
                            format!(
                                "no entry for {name} (entries: {})",
                                actual.keys().join(", ")
                            ),
                        ))
                    }
                },
            }
        }
        Ok(())
    }

    /// A mapping of list operators against a list.
    fn list(
        &mut self,
        ops: &Mapping,
        items: &[View],
        parent: Option<&View>,
        path: &str,
    ) -> Result<(), Failure> {
        let mut items: Vec<&View> = items.iter().collect();
        let mut path = path.to_string();
        if let Some(filter) = ops.get("where") {
            let mut kept = Vec::new();
            for item in items {
                if self.probe(filter, item, parent, &path)? {
                    kept.push(item);
                }
            }
            items = kept;
            path = format!("{path}(where)");
        }
        let path = path.as_str();
        let render =
            |items: &[&View]| View::List(items.iter().map(|&v| v.clone()).collect()).short();
        for (key, arg) in ops {
            match key.as_str() {
                Some("where") => {}
                Some("count") => {
                    let count = View::Int(items.len() as i128);
                    self.check(arg, &count, parent, &format!("{path}.count"))
                        .map_err(|f| match f {
                            Failure::Mismatch(m) => {
                                Failure::Mismatch(format!("{m} (items: {})", render(&items)))
                            }
                            invalid => invalid,
                        })?;
                }
                Some("all") => {
                    for (i, item) in items.iter().enumerate() {
                        self.check(arg, item, parent, &format!("{path}[{i}]"))?;
                    }
                }
                Some("any") => {
                    let mut found = None;
                    for (i, item) in items.iter().enumerate() {
                        if self.probe(arg, item, parent, path)? {
                            found = Some(i);
                            break;
                        }
                    }
                    match found {
                        Some(i) => self.check(arg, items[i], parent, &format!("{path}[{i}]"))?,
                        None => {
                            return Err(Failure::Mismatch(format!(
                                "{}no item matches {} (items: {})",
                                at(path),
                                show(arg),
                                render(&items)
                            )))
                        }
                    }
                }
                Some("none") => {
                    for (i, item) in items.iter().enumerate() {
                        if self.probe(arg, item, parent, path)? {
                            return Err(Failure::Mismatch(format!(
                                "{path}[{i}]: expected no item to match {}, got {}",
                                show(arg),
                                item.short()
                            )));
                        }
                    }
                }
                Some("contains") => self.contains(arg, &items, parent, path)?,
                Some("sequence") => self.sequence(arg, &items, parent, path)?,
                Some("sum") => self.sum(arg, &items, parent, path)?,
                _ => {
                    let index = match key {
                        Value::Number(n) => n.as_i64(),
                        Value::String(s) => s.trim().parse().ok(),
                        _ => None,
                    };
                    let Some(i) = index else {
                        return Err(invalid(
                            path,
                            format!(
                                "unknown list operator `{}` (operators: {LIST_OPERATORS})",
                                show(key)
                            ),
                        ));
                    };
                    let len = items.len() as i64;
                    let resolved = if i < 0 { len + i } else { i };
                    if !(0..len).contains(&resolved) {
                        return Err(Failure::Mismatch(format!(
                            "{path}[{i}]: no such item, the list has {len} item(s): {}",
                            render(&items)
                        )));
                    }
                    self.check(
                        arg,
                        items[resolved as usize],
                        parent,
                        &format!("{path}[{i}]"),
                    )?;
                }
            }
        }
        Ok(())
    }

    /// `contains: [m...]`: each matcher matches a different item.
    fn contains(
        &mut self,
        arg: &Value,
        items: &[&View],
        parent: Option<&View>,
        path: &str,
    ) -> Result<(), Failure> {
        let wanted: Vec<&Value> = match arg {
            Value::Sequence(list) => list.iter().collect(),
            one => vec![one],
        };
        let mut fits = vec![vec![false; items.len()]; wanted.len()];
        for (w, m) in wanted.iter().enumerate() {
            for (i, item) in items.iter().enumerate() {
                fits[w][i] = self.probe(m, item, parent, path)?;
            }
        }
        // Maximum bipartite matching of matchers to items (Kuhn's algorithm).
        let mut owner: Vec<Option<usize>> = vec![None; items.len()];
        for w in 0..wanted.len() {
            let mut seen = vec![false; items.len()];
            augment(w, &fits, &mut seen, &mut owner);
        }
        let mut assigned = vec![None; wanted.len()];
        for (i, w) in owner.iter().enumerate() {
            if let Some(w) = *w {
                assigned[w] = Some(i);
            }
        }
        let missing: Vec<String> = wanted
            .iter()
            .zip(&assigned)
            .filter(|(_, a)| a.is_none())
            .map(|(m, _)| show(m))
            .collect();
        if !missing.is_empty() {
            return Err(Failure::Mismatch(format!(
                "{}no {}item matches {} (items: {})",
                at(path),
                if wanted.len() > 1 { "distinct " } else { "" },
                missing.join(", "),
                View::List(items.iter().map(|&v| v.clone()).collect()).short()
            )));
        }
        for (m, i) in wanted.iter().zip(assigned) {
            let i = i.expect("every matcher is assigned");
            self.check(m, items[i], parent, &format!("{path}[{i}]"))?;
        }
        Ok(())
    }

    /// `sequence: [m...]`: the matchers match items in order (not necessarily adjacent).
    fn sequence(
        &mut self,
        arg: &Value,
        items: &[&View],
        parent: Option<&View>,
        path: &str,
    ) -> Result<(), Failure> {
        let wanted = arg.as_sequence().ok_or_else(|| {
            invalid(
                path,
                format!("`sequence` takes a list of matchers, got {}", show(arg)),
            )
        })?;
        let mut next = 0;
        for (k, m) in wanted.iter().enumerate() {
            let mut found = None;
            for (i, item) in items.iter().enumerate().skip(next) {
                if self.probe(m, item, parent, path)? {
                    found = Some(i);
                    break;
                }
            }
            let Some(i) = found else {
                let after = if next > 0 {
                    format!(" after item {}", next - 1)
                } else {
                    String::new()
                };
                return Err(Failure::Mismatch(format!(
                    "{}sequence matcher {k} ({}) matches no item{after} (items: {})",
                    at(path),
                    show(m),
                    View::List(items.iter().map(|&v| v.clone()).collect()).short()
                )));
            };
            self.check(m, items[i], parent, &format!("{path}[{i}]"))?;
            next = i + 1;
        }
        Ok(())
    }

    /// `sum: {field: m}`: the total of a numeric field over the items.
    fn sum(
        &mut self,
        arg: &Value,
        items: &[&View],
        parent: Option<&View>,
        path: &str,
    ) -> Result<(), Failure> {
        let fields = arg.as_mapping().ok_or_else(|| {
            invalid(
                path,
                format!(
                    "`sum` takes a mapping of fields to matchers, got {}",
                    show(arg)
                ),
            )
        })?;
        for (key, m) in fields {
            let field = key_name(key);
            let mut total = Num::Int(0);
            for (i, item) in items.iter().enumerate() {
                let value = item
                    .field(&field)
                    .ok_or_else(|| invalid(path, format!("item {i} has no field `{field}`")))?;
                let n = match value {
                    View::Int(_) | View::Float(_) => number(value),
                    other => {
                        return Err(invalid(
                            path,
                            format!("`{field}` of item {i} is {}, not a number", other.kind()),
                        ))
                    }
                };
                total = add(total, n);
            }
            let total = match total {
                Num::Int(n) => View::Int(n),
                Num::Float(f) => View::Float(f),
            };
            self.check(m, &total, parent, &format!("{path}(sum).{field}"))?;
        }
        Ok(())
    }

    /// A comparison or range (`">= 3"`, `"1..=4"`, `"@max_gold - 2"`) against a number.
    fn condition(
        &self,
        source: &str,
        actual: &View,
        parent: Option<&View>,
        path: &str,
    ) -> Result<(), Failure> {
        let condition = parse_condition(source)
            .map_err(|e| invalid(path, format!("bad condition {source:?}: {e}")))?;
        let got = number(actual);
        let (holds, evaluated) = match &condition {
            Condition::Compare(op, expr) => {
                let want = self.eval(expr, parent, path)?;
                let ordering = compare(got, want);
                let holds = match op {
                    Op::Eq => ordering == Some(Ordering::Equal),
                    Op::Ne => ordering.is_some_and(|o| o != Ordering::Equal),
                    Op::Lt => ordering == Some(Ordering::Less),
                    Op::Le => matches!(ordering, Some(Ordering::Less | Ordering::Equal)),
                    Op::Gt => ordering == Some(Ordering::Greater),
                    Op::Ge => matches!(ordering, Some(Ordering::Greater | Ordering::Equal)),
                };
                (holds, format!("{}{}", op.symbol(), render_num(want)))
            }
            Condition::Range(lo, hi, inclusive) => {
                let (lo, hi) = (self.eval(lo, parent, path)?, self.eval(hi, parent, path)?);
                let above = matches!(compare(got, lo), Some(Ordering::Greater | Ordering::Equal));
                let below = match compare(got, hi) {
                    Some(Ordering::Less) => true,
                    Some(Ordering::Equal) => *inclusive,
                    _ => false,
                };
                let dots = if *inclusive { "..=" } else { ".." };
                (
                    above && below,
                    format!("{}{dots}{}", render_num(lo), render_num(hi)),
                )
            }
        };
        if holds {
            return Ok(());
        }
        let computed = if condition.has_references() {
            format!(" (= {evaluated})")
        } else {
            String::new()
        };
        Err(Failure::Mismatch(format!(
            "{}expected {source}{computed}, got {}",
            at(path),
            actual.short()
        )))
    }

    fn eval(&self, expr: &Expr, parent: Option<&View>, path: &str) -> Result<Num, Failure> {
        let mut total = Num::Int(0);
        for (negative, term) in &expr.terms {
            let value = match term {
                Term::Num(n) => *n,
                Term::Capture(name) => {
                    let captured = self
                        .captures
                        .get(name)
                        .ok_or_else(|| self.no_capture(name, path))?;
                    match captured {
                        View::Int(_) | View::Float(_) => number(captured),
                        other => {
                            return Err(invalid(
                                path,
                                format!("${name} is {}, not a number", other.kind()),
                            ))
                        }
                    }
                }
                Term::Field(name) => {
                    let object = parent.ok_or_else(|| {
                        invalid(path, format!("`@{name}` needs an enclosing object"))
                    })?;
                    match object.field(name) {
                        Some(value @ (View::Int(_) | View::Float(_))) => number(value),
                        Some(other) => {
                            return Err(invalid(
                                path,
                                format!("@{name} is {}, not a number", other.kind()),
                            ))
                        }
                        None => {
                            return Err(invalid(
                                path,
                                format!(
                                    "`@{name}`: no such field (fields: {})",
                                    object.keys().join(", ")
                                ),
                            ))
                        }
                    }
                }
            };
            total = add(total, if *negative { negate(value) } else { value });
        }
        Ok(total)
    }
}

fn augment(w: usize, fits: &[Vec<bool>], seen: &mut [bool], owner: &mut [Option<usize>]) -> bool {
    for i in 0..seen.len() {
        if fits[w][i] && !seen[i] {
            seen[i] = true;
            let current = owner[i];
            if current.is_none_or(|other| augment(other, fits, seen, owner)) {
                owner[i] = Some(w);
                return true;
            }
        }
    }
    false
}

#[derive(Clone, Copy, Debug)]
enum Num {
    Int(i128),
    Float(f64),
}

fn number(view: &View) -> Num {
    match view {
        View::Int(n) => Num::Int(*n),
        View::Float(f) => Num::Float(*f),
        _ => Num::Float(f64::NAN),
    }
}

fn number_of_yaml(n: &serde_yaml::Number) -> Option<Num> {
    match (n.as_i64(), n.as_u64(), n.as_f64()) {
        (Some(i), _, _) => Some(Num::Int(i.into())),
        (_, Some(u), _) => Some(Num::Int(u.into())),
        (_, _, Some(f)) => Some(Num::Float(f)),
        _ => None,
    }
}

fn as_f64(n: Num) -> f64 {
    match n {
        Num::Int(i) => i as f64,
        Num::Float(f) => f,
    }
}

fn compare(a: Num, b: Num) -> Option<Ordering> {
    match (a, b) {
        (Num::Int(a), Num::Int(b)) => Some(a.cmp(&b)),
        _ => as_f64(a).partial_cmp(&as_f64(b)),
    }
}

fn add(a: Num, b: Num) -> Num {
    match (a, b) {
        (Num::Int(a), Num::Int(b)) => Num::Int(a + b),
        _ => Num::Float(as_f64(a) + as_f64(b)),
    }
}

fn negate(n: Num) -> Num {
    match n {
        Num::Int(i) => Num::Int(-i),
        Num::Float(f) => Num::Float(-f),
    }
}

fn render_num(n: Num) -> String {
    match n {
        Num::Int(i) => i.to_string(),
        Num::Float(f) => f.to_string(),
    }
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Op {
    fn symbol(self) -> &'static str {
        match self {
            Op::Eq => "",
            Op::Ne => "!= ",
            Op::Lt => "< ",
            Op::Le => "<= ",
            Op::Gt => "> ",
            Op::Ge => ">= ",
        }
    }
}

#[derive(Debug)]
enum Term {
    Num(Num),
    Capture(String),
    Field(String),
}

/// A sum of terms; each term is negated or not.
#[derive(Debug)]
struct Expr {
    terms: Vec<(bool, Term)>,
}

#[derive(Debug)]
enum Condition {
    Compare(Op, Expr),
    /// `lo..hi` or `lo..=hi` (inclusive).
    Range(Expr, Expr, bool),
}

impl Condition {
    fn has_references(&self) -> bool {
        let refs = |e: &Expr| e.terms.iter().any(|(_, t)| !matches!(t, Term::Num(_)));
        match self {
            Condition::Compare(_, e) => refs(e),
            Condition::Range(lo, hi, _) => refs(lo) || refs(hi),
        }
    }
}

fn parse_condition(source: &str) -> Result<Condition, String> {
    let s = source.trim();
    for (symbol, op) in [
        (">=", Op::Ge),
        ("<=", Op::Le),
        ("!=", Op::Ne),
        ("==", Op::Eq),
        (">", Op::Gt),
        ("<", Op::Lt),
    ] {
        if let Some(rest) = s.strip_prefix(symbol) {
            return Ok(Condition::Compare(op, parse_expr(rest)?));
        }
    }
    if let Some((lo, hi)) = s.split_once("..=") {
        return Ok(Condition::Range(parse_expr(lo)?, parse_expr(hi)?, true));
    }
    if let Some((lo, hi)) = s.split_once("..") {
        return Ok(Condition::Range(parse_expr(lo)?, parse_expr(hi)?, false));
    }
    Ok(Condition::Compare(Op::Eq, parse_expr(s)?))
}

/// `[+-] term {(+|-) term}` where a term is a number, `$capture` or `@field`.
fn parse_expr(source: &str) -> Result<Expr, String> {
    let chars: Vec<char> = source.chars().filter(|c| !c.is_whitespace()).collect();
    let mut terms = Vec::new();
    let mut i = 0;
    loop {
        let mut negative = false;
        if let Some(&sign @ ('+' | '-')) = chars.get(i) {
            negative = sign == '-';
            i += 1;
        } else if !terms.is_empty() {
            return Err(format!(
                "expected `+` or `-` at {:?}",
                chars[i..].iter().collect::<String>()
            ));
        }
        let start = i;
        let word = |i: &mut usize, ok: fn(char) -> bool| {
            let begin = *i;
            while chars.get(*i).is_some_and(|&c| ok(c)) {
                *i += 1;
            }
            chars[begin..*i].iter().collect::<String>()
        };
        let term = match chars.get(i) {
            Some('$') | Some('@') => {
                i += 1;
                let name = word(&mut i, |c| c.is_ascii_alphanumeric() || c == '_');
                if name.is_empty() {
                    return Err(format!("`{}` needs a name", chars[start]));
                }
                if chars[start] == '$' {
                    Term::Capture(name)
                } else {
                    Term::Field(name)
                }
            }
            Some(c) if c.is_ascii_digit() => {
                let text = word(&mut i, |c| c.is_ascii_digit() || c == '.');
                let n = if text.contains('.') {
                    Num::Float(text.parse().map_err(|_| format!("bad number {text:?}"))?)
                } else {
                    Num::Int(text.parse().map_err(|_| format!("bad number {text:?}"))?)
                };
                Term::Num(n)
            }
            Some(c) => return Err(format!("unexpected {c:?}")),
            None => return Err("expected a number, `$capture` or `@field`".to_string()),
        };
        terms.push((negative, term));
        if i >= chars.len() {
            return Ok(Expr { terms });
        }
    }
}
