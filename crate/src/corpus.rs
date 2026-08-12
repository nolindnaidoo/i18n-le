//! The corpus, embedded and run as unit tests.
//!
//! `fixtures/` is the contract this crate is held to. Embedding it means
//! `cargo test` on the published tarball runs every case, so whoever
//! installed this can check the claims in the README instead of trusting
//! them.
//!
//! Every document has the name it would really carry, because the locale
//! of a catalogue is read from its name and that is one of the things
//! under test.
//!
//! **Nothing here reaches a filesystem.** Identification does, so it is
//! tested in `identify.rs` against temporary trees; what the corpus pins
//! is everything downstream of the answer — the reader, the locale
//! names, the content marks and whole audits.

/// One corpus document, by name.
///
/// Panics on a name the corpus does not carry — a test naming a file
/// that is not there is a broken test, not a runtime condition.
pub(crate) fn document(name: &str) -> &'static str {
    match name {
        "en.json" => include_str!("../fixtures/documents/en.json"),
        "es.json" => include_str!("../fixtures/documents/es.json"),
        "fr.json" => include_str!("../fixtures/documents/fr.json"),
        "de.json" => include_str!("../fixtures/documents/de.json"),
        "package.nls.json" => include_str!("../fixtures/documents/package.nls.json"),
        "package.nls.de.json" => include_str!("../fixtures/documents/package.nls.de.json"),
        "package.nls.ja.json" => include_str!("../fixtures/documents/package.nls.ja.json"),
        "package.nls.zh-cn.json" => include_str!("../fixtures/documents/package.nls.zh-cn.json"),
        "bundle.l10n.json" => include_str!("../fixtures/documents/bundle.l10n.json"),
        "bundle.l10n.pt-br.json" => include_str!("../fixtures/documents/bundle.l10n.pt-br.json"),
        "i18n.en.json" => include_str!("../fixtures/documents/i18n.en.json"),
        "i18n.pl.json" => include_str!("../fixtures/documents/i18n.pl.json"),
        "i18n.ja.json" => include_str!("../fixtures/documents/i18n.ja.json"),
        "icu.en.json" => include_str!("../fixtures/documents/icu.en.json"),
        "icu.es.json" => include_str!("../fixtures/documents/icu.es.json"),
        "arb.en.arb" => include_str!("../fixtures/documents/arb.en.arb"),
        "arb.es.arb" => include_str!("../fixtures/documents/arb.es.arb"),
        "signals-i18next.json" => include_str!("../fixtures/documents/signals-i18next.json"),
        "signals-next-intl.json" => include_str!("../fixtures/documents/signals-next-intl.json"),
        "signals-vscode.json" => include_str!("../fixtures/documents/signals-vscode.json"),
        "signals-arb.json" => include_str!("../fixtures/documents/signals-arb.json"),
        "edge-escaped-braces.json" => {
            include_str!("../fixtures/documents/edge-escaped-braces.json")
        }
        "edge-duplicates.json" => include_str!("../fixtures/documents/edge-duplicates.json"),
        "edge-structure.json" => include_str!("../fixtures/documents/edge-structure.json"),
        "edge-broken.json" => include_str!("../fixtures/documents/edge-broken.json"),
        "edge-bom.json" => include_str!("../fixtures/documents/edge-bom.json"),
        "edge-crlf.json" => include_str!("../fixtures/documents/edge-crlf.json"),
        other => panic!("the corpus has no document named {other}"),
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::Value;

    use super::document;
    use crate::catalogue;
    use crate::library::{Id, Mark};
    use crate::locale;
    use crate::message;
    use crate::scan::{self, Document, ScanOptions, System};

    const CORPUS: &str = include_str!("../fixtures/detection.json");

    #[derive(Debug, Deserialize)]
    struct Corpus {
        parsing: Vec<ParsingCase>,
        locales: Vec<LocaleCase>,
        marks: Vec<MarkCase>,
        audit: Vec<AuditCase>,
    }

    #[derive(Debug, Deserialize)]
    struct ParsingCase {
        file: String,
        /// Every path in the document, objects on the way down included
        /// and metadata keys with them: which keys are metadata is the
        /// library's answer, not the reader's.
        keys: Option<Vec<String>>,
        #[serde(default)]
        duplicates: Vec<DuplicateCase>,
        refused: Option<bool>,
    }

    #[derive(Debug, Deserialize)]
    struct DuplicateCase {
        key: String,
        occurrences: usize,
    }

    #[derive(Debug, Deserialize)]
    struct LocaleCase {
        names: Vec<String>,
        locales: Option<Vec<Option<String>>>,
        refused: Option<bool>,
    }

    /// The content-evidence scan: which syntactic marks a document
    /// carries, read with no library in hand. This is the class that
    /// makes a translations-only repository identifiable.
    #[derive(Debug, Deserialize)]
    struct MarkCase {
        file: String,
        marks: Vec<Mark>,
    }

    #[derive(Debug, Deserialize)]
    struct AuditCase {
        name: String,
        library: Id,
        files: Vec<String>,
        #[serde(default)]
        options: CaseOptions,
        expected: Value,
    }

    #[derive(Debug, Default, Deserialize)]
    struct CaseOptions {
        source: Option<String>,
        #[serde(rename = "keysAreSource")]
        keys_are_source: Option<bool>,
    }

    fn corpus() -> Corpus {
        serde_json::from_str(CORPUS).expect("the corpus is valid JSON")
    }

    fn documents(names: &[String]) -> Vec<Document> {
        let locales = locale::locales_of(names).expect("the corpus names a real set");
        names
            .iter()
            .zip(locales)
            .map(|(name, locale)| Document {
                name: name.clone(),
                locale,
                content: document(name).to_string(),
            })
            .collect()
    }

    #[test]
    fn every_corpus_document_parses_the_same() {
        let corpus = corpus();
        assert!(!corpus.parsing.is_empty(), "the corpus parses nothing");
        for case in corpus.parsing {
            let parsed = catalogue::parse(document(&case.file));
            if case.refused == Some(true) {
                assert!(parsed.is_err(), "{} was not refused", case.file);
                continue;
            }
            let parsed = parsed.expect(&case.file);
            let keys: Vec<String> = parsed.entries.into_iter().map(|entry| entry.key).collect();
            assert_eq!(
                keys,
                case.keys.expect("a case pins keys or a refusal"),
                "{} keys",
                case.file
            );
            let duplicates: Vec<(String, usize)> = parsed
                .duplicates
                .into_iter()
                .map(|duplicate| (duplicate.key, duplicate.occurrences))
                .collect();
            let expected: Vec<(String, usize)> = case
                .duplicates
                .into_iter()
                .map(|duplicate| (duplicate.key, duplicate.occurrences))
                .collect();
            assert_eq!(duplicates, expected, "{} duplicates", case.file);
        }
    }

    #[test]
    fn every_corpus_name_yields_the_same_locale() {
        let corpus = corpus();
        assert!(!corpus.locales.is_empty(), "the corpus names nothing");
        for case in corpus.locales {
            let actual = locale::locales_of(&case.names);
            if case.refused == Some(true) {
                assert!(actual.is_err(), "{:?} was not refused", case.names);
                continue;
            }
            assert_eq!(
                actual.expect("locales"),
                case.locales.expect("a case pins locales or a refusal"),
                "{:?}",
                case.names
            );
        }
    }

    /// **The evidence class that stands alone.** Every mark here is a
    /// vote towards a library, and the four `signals-*` documents are
    /// each what one library's catalogues look like with nothing else
    /// around them.
    #[test]
    fn every_corpus_document_carries_the_same_marks() {
        let corpus = corpus();
        assert!(!corpus.marks.is_empty(), "the corpus marks nothing");
        for case in corpus.marks {
            let parsed = catalogue::parse(document(&case.file)).expect("the corpus parses");
            let keys: Vec<String> = parsed
                .entries
                .iter()
                .map(|entry| entry.key.clone())
                .collect();
            let mut found = message::key_marks(&keys);
            for text in parsed
                .entries
                .iter()
                .filter_map(|entry| entry.text.as_deref())
            {
                for mark in message::marks(text) {
                    if !found.contains(&mark) {
                        found.push(mark);
                    }
                }
            }
            found.sort_unstable_by_key(|mark| format!("{mark:?}"));
            let mut expected = case.marks;
            expected.sort_unstable_by_key(|mark| format!("{mark:?}"));
            assert_eq!(found, expected, "{}", case.file);
        }
    }

    #[test]
    fn every_corpus_audit_reproduces() {
        let corpus = corpus();
        assert!(!corpus.audit.is_empty(), "the corpus audits nothing");
        for case in corpus.audit {
            let keys_are_source = case.options.keys_are_source.unwrap_or(false);
            let report = scan::report_for(
                &documents(&case.files),
                System {
                    library: case.library,
                    version: None,
                    layout: None,
                    keys_are_source,
                    evidence: Vec::new(),
                },
                &ScanOptions {
                    source: case.options.source.clone(),
                    keys_are_source,
                },
            )
            .expect(&case.name);
            let actual = serde_json::to_value(&report).expect("a report serializes");
            assert_eq!(actual, case.expected, "{}", case.name);
        }
    }

    /// **The property the whole crate rests on**, checked against the
    /// corpus rather than trusted: no expectation carries a translation.
    #[test]
    fn no_corpus_answer_carries_a_translated_value() {
        for translated in [
            "Bienvenido",
            "Bon retour",
            "Übersicht",
            "Kataloge",
            "Verificar",
            "Déconnexion",
            "Inicio",
            "Accueil",
        ] {
            assert!(
                !CORPUS.contains(translated),
                "the corpus carries {translated}"
            );
        }
    }

    /// The repository goes public. A real product name in a fixture is a
    /// leak that no amount of later care undoes.
    #[test]
    fn no_corpus_document_carries_a_real_product_name() {
        let names = [
            "en.json",
            "es.json",
            "fr.json",
            "de.json",
            "edge-bom.json",
            "edge-crlf.json",
            "package.nls.json",
            "bundle.l10n.json",
        ];
        for name in names {
            let text = document(name);
            // Names from adjacent private work that reached these fixtures once.
            // The list stays short and stays to names already public elsewhere:
            // a guard that has to spell a private name in a public repository
            // leaks the thing it was written to protect.
            for leaked in ["SplitWinner", "splitwinner", "OffensiveEdge"] {
                assert!(!text.contains(leaked), "{name} carries {leaked}");
            }
        }
    }
}
