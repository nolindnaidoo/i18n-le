//! Reading a JSON catalogue in a way a plain deserialize cannot.
//!
//! `serde_json::Value` folds a duplicate key into the last one and says
//! nothing, which is exactly the defect `duplicate-key-within-file`
//! exists to find — the loader at runtime does the same fold, so the
//! second translation of a key is dead text nobody will ever see. The
//! visitor below keeps every occurrence, then reports the fold rather
//! than performing it silently.
//!
//! **Nested and flat are the same catalogue.** `{"a":{"b":"x"}}` and
//! `{"a.b":"x"}` both flatten to the path `a.b`, so a project that
//! migrated between the two shapes still compares against its own
//! history.
//!
//! Values are read, never carried: `Entry` holds a leaf's text only so
//! the audit can ask whether it is empty and whether it is
//! byte-identical to the source. Nothing downstream may put one in an
//! answer — see `audit::Evidence`.
//!
//! **This module knows nothing about any library.** It reads JSON and
//! reports what is in it, metadata keys included; which of those keys
//! are metadata is the identified library's answer, applied afterwards
//! in `audit.rs`. That ordering is deliberate — identification reads the
//! raw key set, and a reader that had already dropped `@greeting` would
//! have destroyed the evidence that says the file is ARB.

use std::collections::HashMap;
use std::fmt;

use serde::de::{Deserializer, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};

/// What a path holds, which is the thing `structure-mismatch` compares.
///
/// `Other` is a number, a boolean, `null` or an array. None of those is
/// a translatable string, and a locale that turned one into a string —
/// or the reverse — has changed shape under whatever reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Shape {
    Text,
    Object,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Node {
    Text(String),
    /// Every entry as written, duplicates included and in file order.
    Object(Vec<(String, Node)>),
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) key: String,
    pub(crate) shape: Shape,
    /// The leaf's text, for the two questions the comparator may ask of
    /// it: is it empty, and is it byte-identical to the source's.
    pub(crate) text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Duplicate {
    pub(crate) key: String,
    pub(crate) occurrences: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Parsed {
    pub(crate) entries: Vec<Entry>,
    pub(crate) duplicates: Vec<Duplicate>,
}

/// Parse one catalogue.
///
/// A leading byte-order mark is dropped here rather than at the
/// filesystem seam, because the MCP surface is handed content by a
/// caller that may have read the file any way at all. Three invisible
/// bytes before the opening `{` make `serde_json` reject the whole
/// document, which reads as "this locale has no keys" — the loudest
/// possible wrong answer.
pub(crate) fn parse(content: &str) -> Result<Parsed, String> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let node: Node = serde_json::from_str(content).map_err(|error| error.to_string())?;
    let Node::Object(root) = node else {
        return Err("a catalogue must be a JSON object".to_string());
    };

    let mut parsed = Parsed::default();
    walk("", &root, &mut parsed);
    Ok(parsed)
}

fn walk(prefix: &str, object: &[(String, Node)], parsed: &mut Parsed) {
    for (name, node) in resolve(prefix, object, parsed) {
        let key = path(prefix, name);
        match node {
            Node::Text(text) => parsed.entries.push(Entry {
                key,
                shape: Shape::Text,
                text: Some(text.clone()),
            }),
            Node::Other => parsed.entries.push(Entry {
                key,
                shape: Shape::Other,
                text: None,
            }),
            Node::Object(children) => {
                parsed.entries.push(Entry {
                    key: key.clone(),
                    shape: Shape::Object,
                    text: None,
                });
                walk(&key, children, parsed);
            }
        }
    }
}

fn path(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        return name.to_string();
    }
    format!("{prefix}.{name}")
}

/// One entry per distinct name, in first-appearance order, carrying the
/// **last** value written for it.
///
/// Last, because that is what every JSON loader hands the application —
/// reporting the duplicate while comparing the first would answer a
/// question about a file the runtime never sees.
///
/// One pass and a map, rather than a scan per name: this ran over every
/// pair for every pair, so a flat catalogue cost time in the square of
/// its key count — 0.73s at 12,000 keys and 2.93s at 24,000 on the
/// machine this was measured on. Order is still the vector's, so the
/// map never decides anything a reader can see.
fn resolve<'a>(
    prefix: &str,
    object: &'a [(String, Node)],
    parsed: &mut Parsed,
) -> Vec<(&'a str, &'a Node)> {
    let mut resolved: Vec<(&'a str, &'a Node)> = Vec::new();
    let mut occurrences: Vec<usize> = Vec::new();
    let mut first: HashMap<&'a str, usize> = HashMap::new();

    for (name, node) in object {
        // Both indexes come from `first`, which only ever holds
        // positions this loop pushed.
        if let Some(&at) = first.get(name.as_str()) {
            resolved[at].1 = node;
            occurrences[at] += 1;
            continue;
        }
        first.insert(name.as_str(), resolved.len());
        resolved.push((name.as_str(), node));
        occurrences.push(1);
    }

    for ((name, _), count) in resolved.iter().zip(&occurrences) {
        if *count < 2 {
            continue;
        }
        parsed.duplicates.push(Duplicate {
            key: path(prefix, name),
            occurrences: *count,
        });
    }
    resolved
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(NodeVisitor)
    }
}

struct NodeVisitor;

impl<'de> Visitor<'de> for NodeVisitor {
    type Value = Node;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_str<E>(self, value: &str) -> Result<Node, E> {
        Ok(Node::Text(value.to_string()))
    }

    fn visit_string<E>(self, value: String) -> Result<Node, E> {
        Ok(Node::Text(value))
    }

    fn visit_bool<E>(self, _: bool) -> Result<Node, E> {
        Ok(Node::Other)
    }

    fn visit_i64<E>(self, _: i64) -> Result<Node, E> {
        Ok(Node::Other)
    }

    fn visit_u64<E>(self, _: u64) -> Result<Node, E> {
        Ok(Node::Other)
    }

    fn visit_f64<E>(self, _: f64) -> Result<Node, E> {
        Ok(Node::Other)
    }

    fn visit_unit<E>(self) -> Result<Node, E> {
        Ok(Node::Other)
    }

    fn visit_none<E>(self) -> Result<Node, E> {
        Ok(Node::Other)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Node, A::Error> {
        // Drained rather than skipped: an array's elements still have to
        // come off the token stream or the parse ends mid-document.
        while sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {}
        Ok(Node::Other)
    }

    /// **The reason this visitor exists.** Every pair reaches the vector,
    /// so a key written twice is still two entries here.
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
        let mut entries = Vec::new();
        while let Some((key, value)) = map.next_entry::<String, Node>()? {
            entries.push((key, value));
        }
        Ok(Node::Object(entries))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(content: &str) -> Vec<String> {
        parse(content)
            .expect("parses")
            .entries
            .into_iter()
            .map(|entry| entry.key)
            .collect()
    }

    #[test]
    fn a_flat_catalogue_yields_its_keys() {
        assert_eq!(keys(r#"{"a":"one","b":"two"}"#), ["a", "b"]);
    }

    /// Nested and flat normalise to the same path space, which is what
    /// lets a project that migrated between the two shapes still be
    /// compared against its own history.
    #[test]
    fn a_nested_catalogue_flattens_to_dotted_paths() {
        assert_eq!(
            keys(r#"{"a":{"b":{"c":"x"}}}"#),
            ["a", "a.b", "a.b.c"],
            "the objects on the way down are entries too"
        );
        let nested: Vec<String> = parse(r#"{"a":{"b":"x"}}"#)
            .expect("parses")
            .entries
            .into_iter()
            .filter(|entry| entry.shape == Shape::Text)
            .map(|entry| entry.key)
            .collect();
        assert_eq!(nested, ["a.b"]);
        assert_eq!(keys(r#"{"a.b":"x"}"#), ["a.b"]);
    }

    /// **The check a plain deserialize cannot make.** `serde_json::Value`
    /// and `JSON.parse` both keep the last and say nothing.
    #[test]
    fn a_duplicate_key_is_reported_and_the_last_value_wins() {
        let parsed = parse(r#"{"a":"first","b":"other","a":"second"}"#).expect("parses");
        assert_eq!(parsed.duplicates.len(), 1);
        assert_eq!(parsed.duplicates[0].key, "a");
        assert_eq!(parsed.duplicates[0].occurrences, 2);
        assert_eq!(
            parsed.entries[0].text.as_deref(),
            Some("second"),
            "the runtime would read the last one"
        );
        assert_eq!(keys(r#"{"a":"first","b":"o","a":"second"}"#), ["a", "b"]);
    }

    #[test]
    fn a_duplicate_nested_key_is_named_by_its_path() {
        let parsed = parse(r#"{"menu":{"open":"a","open":"b"}}"#).expect("parses");
        assert_eq!(parsed.duplicates[0].key, "menu.open");
    }

    #[test]
    fn the_shape_of_every_path_is_recorded() {
        let parsed = parse(r#"{"t":"x","o":{"n":"y"},"n":1,"a":[1,2],"z":null}"#).expect("parses");
        let shapes: Vec<(String, Shape)> = parsed
            .entries
            .into_iter()
            .map(|entry| (entry.key, entry.shape))
            .collect();
        assert_eq!(
            shapes,
            [
                ("t".to_string(), Shape::Text),
                ("o".to_string(), Shape::Object),
                ("o.n".to_string(), Shape::Text),
                ("n".to_string(), Shape::Other),
                ("a".to_string(), Shape::Other),
                ("z".to_string(), Shape::Other),
            ]
        );
    }

    /// Metadata keys are read like any other key. **Which of them are
    /// metadata is the identified library's answer**, and applying it
    /// here would destroy the evidence that identifies ARB in the first
    /// place.
    #[test]
    fn metadata_keys_are_read_rather_than_dropped() {
        assert_eq!(
            keys(r#"{"@@locale":"es","greeting":"Hola","@greeting":{"description":"x"}}"#),
            ["@@locale", "greeting", "@greeting", "@greeting.description"]
        );
    }

    /// Three invisible bytes that Notepad, Excel and a PowerShell
    /// redirect all add. Before a `{` they make the parse fail, which
    /// reads as "this locale has no keys".
    #[test]
    fn a_leading_byte_order_mark_is_not_part_of_the_document() {
        assert_eq!(keys("\u{feff}{\"a\":\"x\"}"), ["a"]);
    }

    /// Every JSON scalar a catalogue can carry lands as `Other`, which
    /// is what makes `structure-mismatch` able to see a number where a
    /// string belonged. The visitor has an arm per scalar and a silent
    /// one is a hole.
    #[test]
    fn every_scalar_shape_is_read_as_neither_text_nor_object() {
        let parsed =
            parse(r#"{"i":-3,"u":7,"f":1.5,"t":true,"f2":false,"z":null,"a":[1,{"b":"x"}]}"#)
                .expect("parses");
        let shapes: Vec<Shape> = parsed.entries.iter().map(|entry| entry.shape).collect();
        assert_eq!(shapes, [Shape::Other; 7]);
    }

    #[test]
    fn a_document_that_is_not_an_object_is_refused() {
        assert!(parse("[1,2]").is_err());
        assert!(parse("\"just a string\"").is_err());
    }

    #[test]
    fn malformed_json_is_refused_rather_than_guessed_at() {
        assert!(parse("{\"a\":").is_err());
    }

    /// The visitor recurses, and so does `serde_json` reading into it.
    /// A document nested past the reader's own limit has to come back as
    /// a refusal rather than take the process down with it: `scan.rs`
    /// turns a refusal into an `unparsable` diagnostic and audits the
    /// rest of the set, where a stack overflow loses every locale.
    #[test]
    fn a_document_nested_past_the_readers_limit_is_refused_rather_than_crashing() {
        let depth = 2_000;
        let document = format!("{}\"leaf\"{}", "{\"a\":".repeat(depth), "}".repeat(depth));
        assert!(parse(&document).is_err());
    }

    /// A key repeated a great many times is still one entry and one
    /// duplicate, and it is resolved in one pass rather than one scan
    /// per occurrence.
    #[test]
    fn a_key_repeated_many_times_is_counted_once() {
        let repeats = 500;
        let pairs: Vec<String> = (0..repeats).map(|n| format!("\"a\":\"{n}\"")).collect();
        let parsed = parse(&format!("{{{}}}", pairs.join(","))).expect("parses");
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].text.as_deref(), Some("499"));
        assert_eq!(parsed.duplicates.len(), 1);
        assert_eq!(parsed.duplicates[0].occurrences, repeats);
    }

    #[test]
    fn an_empty_catalogue_yields_nothing() {
        assert_eq!(parse("{}").expect("parses"), Parsed::default());
    }

    /// **The privacy boundary's blind spot, held shut.** A refusal here
    /// becomes an `unparsable` diagnostic in the report and on the MCP
    /// surface, and a `String` is the one thing on that path the types
    /// cannot vet. It has to name the position and the token it wanted,
    /// never the bytes it was reading — every case below is a real way a
    /// catalogue breaks, each with a translation sitting next to the
    /// break.
    #[test]
    fn a_refusal_names_the_position_and_never_the_content() {
        for document in [
            r#"{"a":"Bienvenido de nuevo""#,
            r#"{"a":"Bienvenido", }"#,
            r#"{"a": Bienvenido}"#,
            r#"{"a":"Bienvenido"} Bienvenido"#,
            "{\"a\":\"Bienv\u{1}enido\"}",
            r#"{"a":"Bienvenido\q"}"#,
            r#"{"a":"Bienvenido\ud800"}"#,
            r#"{"Bienvenido"}"#,
            r#"{a:"Bienvenido"}"#,
            r#"{"a":'Bienvenido'}"#,
            r#"{"a":"Bienvenido",,}"#,
            r#"{"a":1e999999,"b":"Bienvenido"}"#,
            r#"["Bienvenido"]"#,
            r#""Bienvenido""#,
        ] {
            let refusal = parse(document).expect_err(document);
            assert!(!refusal.contains("Bienvenido"), "{refusal}");
        }
    }
}
