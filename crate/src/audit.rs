//! The checks.
//!
//! **One named source locale is the reference, never the union of all of
//! them.** A union is actively wrong: one locale's typo'd key would
//! become a requirement every other locale is then missing, so a single
//! translator's slip becomes twenty-four findings against everybody
//! else. English is the contract; the others are measured against it.
//!
//! Everything here runs with the library already identified. There is no
//! fallback path, no inference, and no "if we could not tell" branch —
//! the grammar, the plural model and the metadata rule all arrive as
//! values, and if identification could not produce them the run refused
//! before reaching this module.
//!
//! **No translated value crosses this boundary, and the types are what
//! stop it.** A finding carries `Evidence`, which has five variants —
//! token names, counts, styles, shapes, occurrence counts and a
//! construct with a byte offset. None of them can hold a sentence, so
//! there is nothing to remember not to put in one. An `untranslated`
//! finding is proved by "source and target are byte-identical", which is
//! a fact about the text rather than the text.
//!
//! Keys are the exception, and deliberately: a finding that will not
//! name its key is not a finding. Where the layout says the key *is* the
//! English string — the VS Code bundle shape — English reaches the
//! report. A translation never does.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use crate::catalogue::{Duplicate, Entry, Shape};
use crate::library::{Id, Interpolation, Library, Plurals};
use crate::message::{self, Construct, Token};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Severity {
    Error,
    Warning,
    /// Does not fail a run on its own. A string added to English today
    /// is legitimately untranslated until the next translation pass, and
    /// a tool that broke the build for it would be turned off.
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Kind {
    MissingKey,
    ExtraKey,
    PlaceholderCountMismatch,
    PlaceholderNameMismatch,
    /// The target wrote an interpolation in a style this library does
    /// not read, where the source had a real placeholder.
    PlaceholderStyleMismatch,
    /// A construct from a convention this project does not use. The
    /// loader renders it to the user verbatim.
    ConventionMismatch,
    EmptyValue,
    Untranslated,
    DuplicateKeyWithinFile,
    /// A path that is an object in one locale and a string in another.
    /// Invisible to flatten-then-compare, which is why it is a check.
    StructureMismatch,
}

/// What proves a finding.
///
/// **Every variant is metadata.** Counts, styles, shapes, token names,
/// occurrence counts and byte offsets are facts *about* the strings;
/// none of them is a string a translator wrote. There is no variant to
/// add a sentence to, which is the boundary enforced rather than
/// remembered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub(crate) enum Evidence {
    Tokens {
        #[serde(rename = "sourceTokens")]
        source: Vec<String>,
        #[serde(rename = "targetTokens")]
        target: Vec<String>,
    },
    Counts {
        #[serde(rename = "sourceCount")]
        source: usize,
        #[serde(rename = "targetCount")]
        target: usize,
    },
    Styles {
        #[serde(rename = "sourceStyle")]
        source: Interpolation,
        #[serde(rename = "targetStyle")]
        target: Interpolation,
    },
    Shapes {
        #[serde(rename = "sourceShape")]
        source: Shape,
        #[serde(rename = "targetShape")]
        target: Shape,
    },
    Occurrences {
        occurrences: usize,
    },
    Foreign {
        construct: Construct,
        offset: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Finding {
    pub(crate) severity: Severity,
    pub(crate) kind: Kind,
    pub(crate) file: String,
    pub(crate) key: String,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub(crate) evidence: Option<Evidence>,
}

/// One catalogue, as the audit sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Catalogue {
    pub(crate) path: String,
    pub(crate) locale: Option<String>,
    pub(crate) entries: Vec<Entry>,
    pub(crate) duplicates: Vec<Duplicate>,
}

impl Catalogue {
    /// Translatable keys. The objects on the way down are structure, not
    /// something anybody translates.
    pub(crate) fn key_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.shape != Shape::Object)
            .count()
    }

    /// Drop the keys this library calls metadata.
    ///
    /// Done here rather than in the reader because identification reads
    /// the raw key set: `@greeting` beside `greeting` is the evidence
    /// that says the file is ARB, and a reader that had already dropped
    /// it would have thrown that away.
    pub(crate) fn without_metadata(mut self, library: &Library) -> Self {
        self.entries
            .retain(|entry| !library.is_metadata(&entry.key));
        self.duplicates
            .retain(|duplicate| !library.is_metadata(&duplicate.key));
        self
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Options {
    pub(crate) library: Id,
    /// The key itself is the English string, as in a VS Code
    /// `bundle.l10n.json`. A property of the layout, so it arrives here
    /// rather than being read off the library.
    pub(crate) keys_are_source: bool,
}

/// Audit every catalogue in `files` against `files[source]`.
pub(crate) fn audit(files: &[Catalogue], source: usize, options: Options) -> Vec<Finding> {
    let library = options.library.library();
    let reference = &files[source];

    let mut findings = Vec::new();
    findings.extend(convention_findings(reference, options));

    for (index, file) in files.iter().enumerate() {
        if index == source {
            findings.extend(duplicate_findings(file));
            findings.extend(empty_findings(file));
            continue;
        }
        against(reference, file, library, options, &mut findings);
    }
    findings
}

/// Constructs from another convention. The loader renders them verbatim,
/// which is a defect the identified library makes nameable.
fn convention_findings(file: &Catalogue, options: Options) -> Vec<Finding> {
    let grammar = options.library.library().grammar;
    file.entries
        .iter()
        .filter_map(|entry| source_text(entry, options.keys_are_source).map(|t| (&entry.key, t)))
        .flat_map(|(key, text)| {
            message::parse(grammar, text)
                .foreign
                .into_iter()
                .map(move |found| Finding {
                    severity: Severity::Error,
                    kind: Kind::ConventionMismatch,
                    file: file.path.clone(),
                    key: key.clone(),
                    evidence: Some(Evidence::Foreign {
                        construct: found.construct,
                        offset: found.offset,
                    }),
                })
        })
        .collect()
}

/// A key the file defines twice. The loader keeps the last, so the other
/// translation is text nobody will ever see.
fn duplicate_findings(file: &Catalogue) -> impl Iterator<Item = Finding> + '_ {
    file.duplicates.iter().map(|duplicate| Finding {
        severity: Severity::Error,
        kind: Kind::DuplicateKeyWithinFile,
        file: file.path.clone(),
        key: duplicate.key.clone(),
        evidence: Some(Evidence::Occurrences {
            occurrences: duplicate.occurrences,
        }),
    })
}

fn empty_findings(file: &Catalogue) -> impl Iterator<Item = Finding> + '_ {
    file.entries
        .iter()
        .filter(|entry| {
            entry
                .text
                .as_ref()
                .is_some_and(|text| text.trim().is_empty())
        })
        .map(|entry| Finding {
            severity: Severity::Warning,
            kind: Kind::EmptyValue,
            file: file.path.clone(),
            key: entry.key.clone(),
            evidence: None,
        })
}

fn against(
    reference: &Catalogue,
    target: &Catalogue,
    library: &Library,
    options: Options,
    findings: &mut Vec<Finding>,
) {
    findings.extend(duplicate_findings(target));
    // A key whose text is already the finding is not compared further:
    // a `convention-mismatch` is the root cause, and a count difference
    // on top of it is the same defect reported twice.
    let mismatched: HashSet<String> = convention_findings(target, options)
        .iter()
        .map(|finding| finding.key.clone())
        .collect();
    findings.extend(convention_findings(target, options));

    let index: HashMap<&str, &Entry> = target
        .entries
        .iter()
        .map(|entry| (entry.key.as_str(), entry))
        .collect();

    // Structure first, and everything below a divergence is suppressed.
    // When `a` is an object here and a string there, every leaf under
    // `a` is also "missing" — reporting forty of those instead of the
    // one thing that is actually wrong buries the answer.
    let diverged = structure_findings(reference, &index, target, findings);

    let fold = library.plurals == Plurals::KeySuffix;
    let source_bases = bases(reference, fold);
    let target_bases = bases(target, fold);
    findings.extend(absent(
        reference,
        &target_bases,
        &diverged,
        fold,
        &target.path,
        Kind::MissingKey,
    ));
    findings.extend(absent(
        target,
        &source_bases,
        &diverged,
        fold,
        &target.path,
        Kind::ExtraKey,
    ));

    for entry in &reference.entries {
        let Some(source_text) = source_text(entry, options.keys_are_source) else {
            continue;
        };
        let Some(target_entry) = index.get(entry.key.as_str()) else {
            continue;
        };
        let Some(target_text) = target_entry.text.as_deref() else {
            continue;
        };
        // An empty translation is already the finding; comparing the
        // placeholders of a string that is not there would report the
        // same defect a second time under a different name.
        if target_text.trim().is_empty() {
            continue;
        }
        if !mismatched.contains(&entry.key)
            && let Some(finding) =
                placeholder_finding(&entry.key, source_text, target_text, library)
        {
            findings.push(Finding {
                file: target.path.clone(),
                ..finding
            });
            continue;
        }
        if !source_text.is_empty() && source_text == target_text {
            findings.push(Finding {
                severity: Severity::Info,
                kind: Kind::Untranslated,
                file: target.path.clone(),
                key: entry.key.clone(),
                evidence: None,
            });
        }
    }

    findings.extend(empty_findings(target));
}

/// Every path a file defines, reduced to its plural base.
///
/// **Objects count.** A path that is an object here and a string there
/// is a structure mismatch, not an extra key, and leaving objects out
/// made it both.
fn bases(file: &Catalogue, fold: bool) -> HashSet<&str> {
    file.entries
        .iter()
        .map(|entry| base_of(&entry.key, fold))
        .collect()
}

/// Keys of `file` whose base is not in `present`, deduplicated by base.
///
/// Reporting the base rather than the variant is what makes a plural
/// finding one line instead of six.
fn absent(
    file: &Catalogue,
    present: &HashSet<&str>,
    diverged: &[String],
    fold: bool,
    reported_as: &str,
    kind: Kind,
) -> Vec<Finding> {
    let mut seen: HashSet<&str> = HashSet::new();
    file.entries
        .iter()
        .filter(|entry| entry.shape != Shape::Object)
        .filter(|entry| !is_below(&entry.key, diverged))
        .filter_map(|entry| {
            let base = base_of(&entry.key, fold);
            (!present.contains(base) && seen.insert(base)).then(|| Finding {
                severity: Severity::Error,
                kind,
                file: reported_as.to_string(),
                key: base.to_string(),
                evidence: None,
            })
        })
        .collect()
}

fn base_of(key: &str, fold: bool) -> &str {
    if fold { message::plural_base(key) } else { key }
}

/// The paths that hold different kinds of thing in the two files.
fn structure_findings(
    reference: &Catalogue,
    index: &HashMap<&str, &Entry>,
    target: &Catalogue,
    findings: &mut Vec<Finding>,
) -> Vec<String> {
    let mut diverged = Vec::new();
    for entry in &reference.entries {
        let Some(other) = index.get(entry.key.as_str()) else {
            continue;
        };
        if other.shape == entry.shape {
            continue;
        }
        diverged.push(entry.key.clone());
        findings.push(Finding {
            severity: Severity::Error,
            kind: Kind::StructureMismatch,
            file: target.path.clone(),
            key: entry.key.clone(),
            evidence: Some(Evidence::Shapes {
                source: entry.shape,
                target: other.shape,
            }),
        });
    }
    diverged
}

fn is_below(key: &str, diverged: &[String]) -> bool {
    diverged.iter().any(|prefix| {
        key.len() > prefix.len()
            && key.starts_with(prefix.as_str())
            && key.as_bytes()[prefix.len()] == b'.'
    })
}

/// The one placeholder finding a key deserves, or none.
///
/// Style before count before names: a target that wrote its
/// interpolation in another style has token names that are not
/// comparable to the source's, so reporting a name-set difference on top
/// would be two findings for one mistake.
fn placeholder_finding(
    key: &str,
    source: &str,
    target: &str,
    library: &Library,
) -> Option<Finding> {
    let grammar = library.grammar;
    let source_parsed = message::parse(grammar, source);
    let target_parsed = message::parse(grammar, target);

    // A `{name}` in an i18next catalogue is prose — until the source has
    // a real placeholder at the same key, at which point it is a
    // translator having written the wrong braces.
    if !source_parsed.tokens.is_empty()
        && let Some(other) = target_parsed.others.first()
    {
        return Some(finding(
            key,
            Kind::PlaceholderStyleMismatch,
            Evidence::Styles {
                source: grammar.interpolation,
                target: *other,
            },
        ));
    }
    if source_parsed.tokens.len() != target_parsed.tokens.len() {
        return Some(finding(
            key,
            Kind::PlaceholderCountMismatch,
            Evidence::Counts {
                source: source_parsed.tokens.len(),
                target: target_parsed.tokens.len(),
            },
        ));
    }

    // The failure machine translation produces: same count, renamed
    // token. `{{timeframe}}` comes back as `{{periodo}}`, compiles, and
    // renders the literal to a user.
    let (source_names, target_names) = (names(&source_parsed.tokens), names(&target_parsed.tokens));
    (source_names != target_names).then(|| {
        finding(
            key,
            Kind::PlaceholderNameMismatch,
            Evidence::Tokens {
                source: source_names,
                target: target_names,
            },
        )
    })
}

fn finding(key: &str, kind: Kind, evidence: Evidence) -> Finding {
    Finding {
        severity: Severity::Error,
        kind,
        file: String::new(),
        key: key.to_string(),
        evidence: Some(evidence),
    }
}

fn names(tokens: &[Token]) -> Vec<String> {
    let mut names: Vec<String> = tokens.iter().map(|token| token.name.clone()).collect();
    names.sort_unstable();
    names.dedup();
    names
}

/// Which text of an entry is the English one.
fn source_text(entry: &Entry, keys_are_source: bool) -> Option<&str> {
    if entry.shape != Shape::Text {
        return None;
    }
    if keys_are_source {
        return Some(entry.key.as_str());
    }
    entry.text.as_deref()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogue;

    fn catalogue(path: &str, content: &str) -> Catalogue {
        let parsed = catalogue::parse(content).expect("parses");
        Catalogue {
            path: path.to_string(),
            locale: crate::locale::canonicalise(path.trim_end_matches(".json")),
            entries: parsed.entries,
            duplicates: parsed.duplicates,
        }
    }

    fn under(library: Id, source: &str, target: &str) -> Vec<Finding> {
        let files = [
            catalogue("en.json", source).without_metadata(library.library()),
            catalogue("es.json", target).without_metadata(library.library()),
        ];
        audit(
            &files,
            0,
            Options {
                library,
                keys_are_source: false,
            },
        )
    }

    fn kinds(findings: &[Finding]) -> Vec<Kind> {
        findings.iter().map(|finding| finding.kind).collect()
    }

    #[test]
    fn matching_catalogues_have_nothing_to_say() {
        assert_eq!(under(Id::I18next, r#"{"a":"one"}"#, r#"{"a":"uno"}"#), []);
    }

    #[test]
    fn a_key_the_target_lacks_is_missing() {
        let findings = under(Id::I18next, r#"{"a":"one","b":"two"}"#, r#"{"a":"uno"}"#);
        assert_eq!(kinds(&findings), [Kind::MissingKey]);
        assert_eq!(findings[0].key, "b");
        assert_eq!(findings[0].file, "es.json");
    }

    #[test]
    fn a_key_the_source_lacks_is_extra() {
        let findings = under(Id::I18next, r#"{"a":"one"}"#, r#"{"a":"uno","z":"zeta"}"#);
        assert_eq!(kinds(&findings), [Kind::ExtraKey]);
        assert_eq!(findings[0].key, "z");
    }

    /// **The check both hand-rolled parity tests in this tree miss.**
    /// Flatten-then-compare cannot see it, because after flattening the
    /// two files simply have different key sets.
    #[test]
    fn an_object_in_one_locale_and_a_string_in_another_is_a_structure_mismatch() {
        let findings = under(Id::I18next, r#"{"a":{"b":"one"}}"#, r#"{"a":"uno"}"#);
        assert_eq!(kinds(&findings), [Kind::StructureMismatch]);
        assert_eq!(
            findings[0].evidence,
            Some(Evidence::Shapes {
                source: Shape::Object,
                target: Shape::Text
            })
        );
    }

    #[test]
    fn nothing_below_a_structure_mismatch_is_reported_twice() {
        let findings = under(
            Id::I18next,
            r#"{"a":{"b":"one","c":"two","d":"three"}}"#,
            r#"{"a":"uno"}"#,
        );
        assert_eq!(kinds(&findings), [Kind::StructureMismatch]);
    }

    #[test]
    fn a_duplicated_key_is_reported_with_its_count() {
        let findings = under(Id::I18next, r#"{"a":"one"}"#, r#"{"a":"uno","a":"otra"}"#);
        assert_eq!(kinds(&findings), [Kind::DuplicateKeyWithinFile]);
        assert_eq!(
            findings[0].evidence,
            Some(Evidence::Occurrences { occurrences: 2 })
        );
    }

    #[test]
    fn an_empty_translation_is_a_warning_and_not_also_untranslated() {
        let findings = under(Id::I18next, r#"{"a":"one"}"#, r#"{"a":"   "}"#);
        assert_eq!(kinds(&findings), [Kind::EmptyValue]);
        assert_eq!(findings[0].severity, Severity::Warning);
        assert_eq!(
            kinds(&under(Id::I18next, r#"{"a":""}"#, r#"{"a":""}"#)),
            [Kind::EmptyValue, Kind::EmptyValue]
        );
    }

    #[test]
    fn a_byte_identical_translation_is_information() {
        let findings = under(Id::I18next, r#"{"a":"Dashboard"}"#, r#"{"a":"Dashboard"}"#);
        assert_eq!(kinds(&findings), [Kind::Untranslated]);
        assert_eq!(findings[0].severity, Severity::Info);
    }

    #[test]
    fn a_dropped_placeholder_is_a_count_mismatch() {
        let findings = under(Id::I18next, r#"{"a":"{{x}} of {{y}}"}"#, r#"{"a":"{{x}}"}"#);
        assert_eq!(kinds(&findings), [Kind::PlaceholderCountMismatch]);
        assert_eq!(
            findings[0].evidence,
            Some(Evidence::Counts {
                source: 2,
                target: 1
            })
        );
    }

    /// The failure machine translation actually produces.
    #[test]
    fn a_renamed_placeholder_is_a_name_mismatch() {
        let findings = under(
            Id::I18next,
            r#"{"a":"in {{timeframe}}"}"#,
            r#"{"a":"en {{periodo}}"}"#,
        );
        assert_eq!(kinds(&findings), [Kind::PlaceholderNameMismatch]);
        assert_eq!(
            findings[0].evidence,
            Some(Evidence::Tokens {
                source: vec!["timeframe".to_string()],
                target: vec!["periodo".to_string()],
            })
        );
    }

    /// A `{name}` in an i18next file is prose — until the source had a
    /// real placeholder at the same key.
    #[test]
    fn a_wrong_brace_style_is_a_style_mismatch_only_where_a_placeholder_belonged() {
        let findings = under(Id::I18next, r#"{"a":"{{name}}"}"#, r#"{"a":"{name}"}"#);
        assert_eq!(kinds(&findings), [Kind::PlaceholderStyleMismatch]);
        assert_eq!(
            findings[0].evidence,
            Some(Evidence::Styles {
                source: Interpolation::DoubleBrace,
                target: Interpolation::SingleBrace
            })
        );

        let prose = under(
            Id::I18next,
            r#"{"a":"Use braces"}"#,
            r#"{"a":"Escribe {llaves}"}"#,
        );
        assert_eq!(kinds(&prose), [], "no placeholder belonged there");
    }

    /// **The finding the identification makes possible.** An ICU plural
    /// in an i18next project renders its braces to the user.
    #[test]
    fn a_construct_from_another_convention_is_a_finding() {
        let findings = under(
            Id::I18next,
            r#"{"a":"{n, plural, other {#}}","b":"in {{timeframe}}"}"#,
            r#"{"a":"x","b":"en {{periodo}}"}"#,
        );
        assert_eq!(
            kinds(&findings),
            [Kind::ConventionMismatch, Kind::PlaceholderNameMismatch]
        );
        assert_eq!(findings[0].file, "en.json");
        assert_eq!(
            findings[0].evidence,
            Some(Evidence::Foreign {
                construct: Construct::IcuArgument,
                offset: 0
            })
        );
    }

    #[test]
    fn a_convention_mismatch_suppresses_the_placeholder_check_for_that_key() {
        let findings = under(
            Id::I18next,
            r#"{"a":"{{count}} items"}"#,
            r#"{"a":"{count, plural, other {# elementos}}"}"#,
        );
        assert_eq!(kinds(&findings), [Kind::ConventionMismatch]);
        assert_eq!(findings[0].file, "es.json");
    }

    /// The same construct under the library that writes it is ordinary.
    #[test]
    fn an_icu_plural_is_compared_as_one_token_under_an_icu_library() {
        assert_eq!(
            kinds(&under(
                Id::NextIntl,
                r#"{"a":"{count, plural, one {# item} other {# items}}"}"#,
                r#"{"a":"{count, plural, other {# elementos}}"}"#,
            )),
            []
        );
        assert_eq!(
            kinds(&under(
                Id::NextIntl,
                r#"{"a":"{count, plural, one {# item} other {# items}}"}"#,
                r#"{"a":"elementos"}"#,
            )),
            [Kind::PlaceholderCountMismatch]
        );
    }

    /// **Plural key suffixes.** Polish needs `_few` and Japanese needs
    /// only `_other`; requiring English's exact variant set would report
    /// a finding on every correctly translated plural in the project.
    #[test]
    fn plural_variants_are_one_key_where_the_library_writes_them_that_way() {
        assert_eq!(
            kinds(&under(
                Id::I18next,
                r#"{"item_one":"{{count}} item","item_other":"{{count}} items"}"#,
                r#"{"item_one":"{{count}} element","item_few":"{{count}} elementy","item_many":"{{count}} elementów","item_other":"{{count}} elementu"}"#,
            )),
            []
        );
        let gone = under(
            Id::I18next,
            r#"{"item_one":"{{count}} item","item_other":"{{count}} items"}"#,
            r#"{"other":"x"}"#,
        );
        assert_eq!(kinds(&gone), [Kind::MissingKey, Kind::ExtraKey]);
        assert_eq!(gone[0].key, "item", "the base, named once");
    }

    /// And under a library whose plurals live in the message, the
    /// suffixes are ordinary keys.
    #[test]
    fn plural_variants_are_ordinary_keys_under_an_icu_library() {
        assert_eq!(
            kinds(&under(
                Id::NextIntl,
                r#"{"item_one":"a","item_other":"b"}"#,
                r#"{"item_one":"c","item_few":"d","item_other":"e"}"#,
            )),
            [Kind::ExtraKey]
        );
    }

    /// ARB's `@key` siblings are metadata, and comparing them would
    /// report a finding on every message in the file.
    #[test]
    fn arb_metadata_is_not_compared() {
        let findings = under(
            Id::FlutterArb,
            r#"{"@@locale":"en","greeting":"Hi {name}","@greeting":{"description":"a"}}"#,
            r#"{"@@locale":"es","greeting":"Hola {name}"}"#,
        );
        assert_eq!(kinds(&findings), []);
    }

    /// The VS Code bundle shape: the key is the English sentence.
    #[test]
    fn keys_can_be_the_source() {
        let library = Id::VscodeL10n;
        let files = [
            catalogue("bundle.l10n.json", r#"{"Save {0}":"Save {0}"}"#),
            catalogue("bundle.l10n.de.json", r#"{"Save {0}":"Save {0}"}"#),
        ];
        let findings = audit(
            &files,
            0,
            Options {
                library,
                keys_are_source: true,
            },
        );
        assert_eq!(kinds(&findings), [Kind::Untranslated]);
    }

    /// **The property the whole crate rests on.**
    #[test]
    fn no_translated_value_reaches_the_report() {
        let findings = under(
            Id::I18next,
            r#"{"a":"in {{timeframe}}","b":"Dashboard","c":"one"}"#,
            r#"{"a":"en {{periodo}}","b":"Dashboard","c":"uno","d":"sobrante"}"#,
        );
        let rendered = serde_json::to_string(&findings).expect("serializes");
        for translated in ["en {{periodo}}", "uno", "sobrante", "Dashboard"] {
            assert!(!rendered.contains(translated), "{rendered}");
        }
        assert!(rendered.contains("periodo"), "the token is the finding");
        assert!(rendered.contains(r#""key":"d""#), "the key is the finding");
    }

    #[test]
    fn a_convention_mismatch_carries_no_value_either() {
        let findings = under(
            Id::I18next,
            r#"{"a":"one"}"#,
            r#"{"a":"{n, plural, other {# elementos secretos}}"}"#,
        );
        let rendered = serde_json::to_string(&findings).expect("serializes");
        assert!(!rendered.contains("elementos"), "{rendered}");
        assert!(rendered.contains("icu-argument"), "{rendered}");
    }

    #[test]
    fn a_single_catalogue_has_nothing_to_compare_against() {
        let files = [catalogue("en.json", r#"{"a":"one"}"#)];
        assert_eq!(
            audit(
                &files,
                0,
                Options {
                    library: Id::I18next,
                    keys_are_source: false
                }
            ),
            []
        );
    }
}
