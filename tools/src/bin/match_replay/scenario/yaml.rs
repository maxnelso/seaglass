//! A small YAML writer for exported scenarios (`match_replay scenario`).
//!
//! Mappings and lists are written in block style when they do not fit on a line, or when the
//! [`Layout`] asks for it; everything else in flow style (`{card: Alleycat, attack: 3}`,
//! `[a, b]`). Long lists of scalars wrap instead of taking a line per item. Entries can have a
//! trailing comment and comment lines before them. [`render`] checks that the text it writes
//! reads back as the value it was given.

use std::collections::HashMap;

use serde_yaml::{Mapping, Value};

/// The line width the writer aims for (long scalars still overflow it).
const WIDTH: usize = 100;

/// How to lay out a document. Paths name entries by their keys and list indices, joined by
/// dots: `scenarios.0.steps.2.buy`.
#[derive(Default)]
pub struct Layout {
    /// Comment lines written before the document.
    pub header: Vec<String>,
    /// Keys whose non-empty values are always written in block style.
    pub block: Vec<&'static str>,
    /// Keys whose list items are written as block mappings (`- buy: 0`) even when they fit on a
    /// line in flow style.
    pub block_items: Vec<&'static str>,
    /// A trailing comment for an entry, by path.
    pub trailing: HashMap<String, String>,
    /// Comment lines before an entry, by path.
    pub leading: HashMap<String, Vec<String>>,
}

/// Write `doc` (a mapping) as YAML laid out by `layout`. Fails if the text does not read back
/// as `doc`.
pub fn render(doc: &Value, layout: &Layout) -> Result<String, String> {
    let Value::Mapping(top) = doc else {
        return Err("the YAML document to write is not a mapping".to_string());
    };
    let mut writer = Writer {
        out: String::new(),
        layout,
    };
    for line in &layout.header {
        writer.comment(0, line);
    }
    writer.mapping(top, 0, "");
    let text = writer.out;
    let back: Value = serde_yaml::from_str(&text)
        .map_err(|e| format!("the YAML written does not parse ({e}):\n{text}"))?;
    if back != *doc {
        return Err(format!(
            "the YAML written does not read back as the value it was given:\n{text}"
        ));
    }
    Ok(text)
}

struct Writer<'a> {
    out: String,
    layout: &'a Layout,
}

impl Writer<'_> {
    fn line(&mut self, text: &str) {
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn comment(&mut self, col: usize, text: &str) {
        let line = if text.is_empty() {
            format!("{}#", indent(col))
        } else {
            format!("{}# {text}", indent(col))
        };
        self.line(&line);
    }

    fn leading(&mut self, col: usize, path: &str) {
        let layout = self.layout;
        for line in layout.leading.get(path).into_iter().flatten() {
            self.comment(col, line);
        }
    }

    fn trailing(&self, path: &str) -> String {
        self.layout
            .trailing
            .get(path)
            .map_or_else(String::new, |c| format!("  # {c}"))
    }

    fn fits(&self, prefix: &str, value: &Value, path: &str) -> bool {
        prefix.len() + 1 + flow(value).len() + self.trailing(path).len() <= WIDTH
    }

    /// The entries of a block mapping whose keys start at column `col`.
    fn mapping(&mut self, m: &Mapping, col: usize, path: &str) {
        for (key, value) in m {
            let name = key_name(key);
            let path = join(path, &name);
            self.leading(col, &path);
            let prefix = format!("{}{}:", indent(col), scalar(key));
            self.entry(&prefix, value, col, &path, Some(&name));
        }
    }

    /// The items of a block list whose dashes are at column `col`; `key` is the list's key.
    fn sequence(&mut self, items: &[Value], col: usize, path: &str, key: Option<&str>) {
        let block_items = key.is_some_and(|k| self.layout.block_items.contains(&k));
        for (i, item) in items.iter().enumerate() {
            let path = join(path, &i.to_string());
            self.leading(col, &path);
            let dash = format!("{}-", indent(col));
            match item {
                Value::Mapping(m)
                    if !m.is_empty() && (block_items || !self.fits(&dash, item, &path)) =>
                {
                    // A block mapping, its first entry on the dash's line.
                    for (j, (k, v)) in m.iter().enumerate() {
                        let name = key_name(k);
                        let entry_path = join(&path, &name);
                        let lead = if j == 0 {
                            self.leading(col, &entry_path);
                            format!("{dash} ")
                        } else {
                            self.leading(col + 2, &entry_path);
                            indent(col + 2)
                        };
                        let prefix = format!("{lead}{}:", scalar(k));
                        self.entry(&prefix, v, col + 2, &entry_path, Some(&name));
                    }
                }
                _ => self.entry(&dash, item, col, &path, None),
            }
        }
    }

    /// `prefix` (`key:` with the key at column `col`, or a dash at column `col`), then `value`
    /// on the same line or in a block below.
    fn entry(&mut self, prefix: &str, value: &Value, col: usize, path: &str, key: Option<&str>) {
        let trailing = self.trailing(path);
        if let Value::String(s) = value {
            if !self.fits(prefix, value, path) && s.contains(' ') {
                self.folded(prefix, s, col + 2, &trailing);
                return;
            }
        }
        let empty = match value {
            Value::Mapping(m) => m.is_empty(),
            Value::Sequence(s) => s.is_empty(),
            _ => true,
        };
        let forced = !empty && key.is_some_and(|k| self.layout.block.contains(&k));
        if empty || (!forced && self.fits(prefix, value, path)) {
            self.line(&format!("{prefix} {}{trailing}", flow(value)));
            return;
        }
        match value {
            Value::Sequence(items) if !forced && items.iter().all(is_scalar) => {
                self.wrapped(prefix, items, col + 4, &trailing);
            }
            Value::Sequence(items) => {
                self.line(&format!("{prefix}{trailing}"));
                self.sequence(items, col + 2, path, key);
            }
            Value::Mapping(m) => {
                self.line(&format!("{prefix}{trailing}"));
                self.mapping(m, col + 2, path);
            }
            _ => unreachable!("scalars are written in flow style"),
        }
    }

    /// A flow list of scalars over several lines, continued at column `cont`.
    fn wrapped(&mut self, prefix: &str, items: &[Value], cont: usize, trailing: &str) {
        let mut line = format!("{prefix} [");
        let mut first = true;
        for (i, item) in items.iter().enumerate() {
            let end = if i + 1 == items.len() { "]" } else { "," };
            let piece = format!("{}{end}", scalar(item));
            if first {
                line.push_str(&piece);
            } else if line.len() + 1 + piece.len() > WIDTH {
                self.line(&line);
                line = format!("{}{piece}", indent(cont));
            } else {
                line.push(' ');
                line.push_str(&piece);
            }
            first = false;
        }
        line.push_str(trailing);
        self.line(&line);
    }

    /// A string too long for one line: double-quoted, folded at spaces over several lines
    /// continued at column `cont` (a line break in a double-quoted scalar reads back as a space).
    fn folded(&mut self, prefix: &str, s: &str, cont: usize, trailing: &str) {
        let quoted = serde_json::to_string(s).expect("strings serialize to JSON");
        let words: Vec<&str> = quoted.split(' ').collect();
        let mut line = format!("{prefix} {}", words[0]);
        for pair in words.windows(2) {
            let (before, word) = (pair[0], pair[1]);
            // Only a single space between two words can become a line break.
            let breakable = !before.is_empty() && !word.is_empty();
            if breakable && line.len() + 1 + word.len() > WIDTH {
                self.line(&line);
                line = format!("{}{word}", indent(cont));
            } else {
                line.push(' ');
                line.push_str(word);
            }
        }
        line.push_str(trailing);
        self.line(&line);
    }
}

fn indent(col: usize) -> String {
    " ".repeat(col)
}

fn join(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_string()
    } else {
        format!("{path}.{name}")
    }
}

fn key_name(key: &Value) -> String {
    match key {
        Value::String(s) => s.clone(),
        other => flow(other),
    }
}

fn is_scalar(value: &Value) -> bool {
    !matches!(
        value,
        Value::Mapping(_) | Value::Sequence(_) | Value::Tagged(_)
    )
}

/// `value` on one line, in flow style.
pub fn flow(value: &Value) -> String {
    match value {
        Value::Sequence(items) => {
            let items: Vec<String> = items.iter().map(flow).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Mapping(m) => {
            let entries: Vec<String> = m
                .iter()
                .map(|(k, v)| format!("{}: {}", scalar(k), flow(v)))
                .collect();
            format!("{{{}}}", entries.join(", "))
        }
        Value::Tagged(tagged) => format!("{} {}", tagged.tag, flow(&tagged.value)),
        _ => scalar(value),
    }
}

/// A scalar: plain if that reads back as the same string, otherwise double-quoted.
fn scalar(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) if plain(s) => s.clone(),
        // A JSON string is a valid double-quoted YAML scalar.
        Value::String(s) => serde_json::to_string(s).expect("strings serialize to JSON"),
        other => flow(other),
    }
}

/// Whether `s` can be written unquoted, in block and flow context alike.
fn plain(s: &str) -> bool {
    const INDICATORS: &str = "-?:,[]{}#&*!|>'\"%@`";
    !s.is_empty()
        && s.trim() == s
        && !s.starts_with(|c: char| INDICATORS.contains(c))
        && !s.contains([':', '#', ',', '[', ']', '{', '}'])
        && !s.contains(char::is_control)
        && serde_yaml::from_str::<Value>(s).is_ok_and(|v| v.as_str() == Some(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> Value {
        serde_yaml::from_str(text).unwrap()
    }

    #[test]
    fn scalars_are_quoted_only_when_needed() {
        for (s, want) in [
            ("Alleycat", "Alleycat"),
            ("Fandral's Fortune", "Fandral's Fortune"),
            ("Gem Day (+1 Attack) 3/2", "Gem Day (+1 Attack) 3/2"),
            (
                "golden Annoy-o-Tron +1/+0 -taunt",
                "golden Annoy-o-Tron +1/+0 -taunt",
            ),
            ("true", "\"true\""),
            ("12", "\"12\""),
            ("null", "\"null\""),
            ("-golden", "\"-golden\""),
            ("a: b", "\"a: b\""),
            ("a, b", "\"a, b\""),
            ("", "\"\""),
            (" x", "\" x\""),
        ] {
            assert_eq!(scalar(&Value::String(s.to_string())), want, "{s:?}");
        }
    }

    #[test]
    fn layouts_read_back() {
        let value = doc(
            "topic: t\nscenarios:\n- name: n\n  steps: [{buy: 0}, refresh, {play: {hand: 0, pos: 1}}]\n  \
             tavern: {board: [Alleycat, {card: Beetle 3/3, charges: 2}], hand: [], gold: 3}\n",
        );
        let mut layout = Layout {
            block: vec!["scenarios", "tavern", "board", "steps"],
            block_items: vec!["scenarios", "steps"],
            ..Layout::default()
        };
        layout.trailing.insert(
            "scenarios.0.steps.0.buy".to_string(),
            "Alleycat".to_string(),
        );
        layout.leading.insert(
            "scenarios.0.steps".to_string(),
            vec!["the steps".to_string()],
        );
        let text = render(&value, &layout).unwrap();
        assert_eq!(
            text,
            "topic: t\nscenarios:\n  - name: n\n    # the steps\n    steps:\n      - buy: 0  # Alleycat\n      \
             - refresh\n      - play: {hand: 0, pos: 1}\n    tavern:\n      board:\n        - Alleycat\n        \
             - {card: Beetle 3/3, charges: 2}\n      hand: []\n      gold: 3\n"
        );
    }

    #[test]
    fn long_lists_wrap() {
        let names: Vec<Value> = (0..60)
            .map(|i| Value::String(format!("Card {i}")))
            .collect();
        let mut top = Mapping::new();
        top.insert("take_from_pool".into(), Value::Sequence(names));
        let value = Value::Mapping(top);
        let text = render(&value, &Layout::default()).unwrap();
        assert!(text.lines().count() > 3, "{text}");
        assert!(text.lines().all(|l| l.len() <= WIDTH), "{text}");
    }

    #[test]
    fn long_strings_fold() {
        let long = format!(
            "P1 at frame 54: {}, then \"quoted\"  double  spaces: {} and a #hash",
            ["buy Prodigious Tusker"; 8].join(", "),
            "x".repeat(120)
        );
        let mut entry = Mapping::new();
        entry.insert("description".into(), long.as_str().into());
        let mut top = Mapping::new();
        top.insert(
            "scenarios".into(),
            Value::Sequence(vec![Value::Mapping(entry)]),
        );
        // `render` fails unless the text reads back as the value.
        let text = render(&Value::Mapping(top), &Layout::default()).unwrap();
        assert!(text.lines().count() > 3, "{text}");
        let overflowing: Vec<&str> = text.lines().filter(|l| l.len() > WIDTH).collect();
        assert_eq!(
            overflowing.len(),
            1,
            "only the long word overflows:\n{text}"
        );
        assert!(
            text.starts_with("scenarios:\n  - description: \"P1 at frame 54: buy"),
            "{text}"
        );
    }
}
