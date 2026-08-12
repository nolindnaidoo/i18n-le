//! One audit end to end — the only path either surface calls.
//!
//! `cli.rs` and `mcp/` both come through here, so a rule can only be
//! written once. `tests/contracts.rs` asserts the two agree.
//!
//! The order is the architecture: **identify, then read, then audit.**
//! Nothing here reaches the audit without a library in hand, and there
//! is no branch that carries on without one.
//!
//! The report is one object for the whole run, not one line per file.
//! The answer is about the *set* of catalogues, which is the difference
//! between this crate and the extractors in the family.
//!
//! **There is no timestamp.** A report is a thing to diff — against the
//! last run, against a baseline, in a pull request — and a clock makes
//! every run differ from every other for a reason that is not the
//! catalogues.

use std::path::PathBuf;

use serde::Serialize;

use crate::audit::{self, Catalogue, Finding, Severity};
use crate::catalogue;
use crate::identify::{self, Requested, Signal};
use crate::layout;
use crate::library::{Id, Shape};

/// The report's shape. A consumer branching on it needs to know when the
/// shape moved: **3 replaced the guessed `profile`/`grammar` pair with
/// `system`**, which names the identified library and the evidence for
/// it, and dropped `refusals` — with a library in hand there is nothing
/// left to refuse per file.
const SCHEMA: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Status {
    Clean,
    Findings,
    NoFiles,
}

/// What fails a run.
///
/// `untranslated` is `info` and does not fail by default: an English
/// string added this morning is legitimately untranslated, and a tool
/// that broke the build for it gets switched off within a week.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum FailOn {
    /// Anything that is not `info`.
    #[default]
    Findings,
    /// Also the untranslated ones.
    Untranslated,
    /// Every finding, whatever its severity.
    Any,
}

impl FailOn {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "untranslated" => Ok(FailOn::Untranslated),
            "any" => Ok(FailOn::Any),
            other => Err(format!("--fail-on takes untranslated or any, not {other}")),
        }
    }

    fn fails(self, finding: &Finding) -> bool {
        match self {
            FailOn::Any | FailOn::Untranslated => true,
            FailOn::Findings => finding.severity != Severity::Info,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Diagnostic {
    pub(crate) severity: String,
    pub(crate) code: String,
    pub(crate) file: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct FileSummary {
    pub(crate) path: String,
    pub(crate) locale: Option<String>,
    /// A **count**, not a list. Key names appear only where they are the
    /// finding.
    pub(crate) keys: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct Summary {
    pub(crate) files: usize,
    pub(crate) findings: usize,
}

/// Which library the audit was carried out under, and why it thought so.
///
/// The evidence is reported because "why did it decide this was
/// i18next" is the first question anyone asks of an answer they did not
/// expect — and because an identification nobody can check is a
/// heuristic with better manners.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct System {
    pub(crate) library: Id,
    /// As declared, when a manifest declared one.
    pub(crate) version: Option<String>,
    /// `null` when nothing looked at a filesystem to find out — the MCP
    /// surface is handed file contents, so it has no layout to report.
    pub(crate) layout: Option<Shape>,
    #[serde(rename = "keysAreSource")]
    pub(crate) keys_are_source: bool,
    pub(crate) evidence: Vec<Signal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Report {
    pub(crate) schema: u32,
    pub(crate) status: Status,
    pub(crate) system: System,
    pub(crate) source: Option<FileSummary>,
    pub(crate) files: Vec<FileSummary>,
    pub(crate) findings: Vec<Finding>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) summary: Summary,
}

/// One catalogue as either surface supplies it: the CLI after reading a
/// file, the MCP server straight from its caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Document {
    pub(crate) name: String,
    /// Settled before this point: `None` is the base catalogue, not
    /// "unknown".
    pub(crate) locale: Option<String>,
    pub(crate) content: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ScanOptions {
    /// A path or a language tag. Absent means auto-detect, which only
    /// answers when exactly one candidate exists.
    pub(crate) source: Option<String>,
    /// Forced on. The layout normally decides this on its own —
    /// `bundle.l10n.json` keys are English strings and `package.nls.json`
    /// keys are not, and both belong to the same library.
    pub(crate) keys_are_source: bool,
}

/// Identify, read, audit.
pub(crate) fn scan(
    inputs: &[PathBuf],
    requested: Requested,
    options: &ScanOptions,
) -> Result<Report, String> {
    let identified = identify::identify(inputs, requested)?;

    let mut documents = Vec::new();
    let mut diagnostics = Vec::new();
    for file in &identified.files {
        match layout::read_text(&file.path) {
            Ok(content) => documents.push(Document {
                name: file.name.clone(),
                locale: file.locale.clone(),
                content,
            }),
            // One unreadable file does not abort the audit of the rest —
            // every repository has something the runner cannot open. It
            // is named so the answer is not quietly narrower than it
            // looks.
            Err(reason) => diagnostics.push(Diagnostic {
                severity: "warning".to_string(),
                code: "skipped".to_string(),
                file: file.name.clone(),
                message: reason,
            }),
        }
    }

    let system = System {
        library: identified.library,
        version: identified.version.clone(),
        layout: Some(identified.shape),
        keys_are_source: options.keys_are_source || identified.keys_are_source,
        evidence: identified.evidence.clone(),
    };
    let mut report = report_for(&documents, system, options)?;
    report.diagnostics.splice(0..0, diagnostics);
    Ok(report)
}

/// The audit itself, over content that is already in hand and a library
/// that has already been settled.
pub(crate) fn report_for(
    documents: &[Document],
    system: System,
    options: &ScanOptions,
) -> Result<Report, String> {
    let library = system.library.library();
    let keys_are_source = system.keys_are_source || options.keys_are_source;

    let mut files = Vec::new();
    let mut diagnostics = Vec::new();
    for document in documents {
        match catalogue::parse(&document.content) {
            Ok(parsed) => files.push(
                Catalogue {
                    path: document.name.clone(),
                    locale: document.locale.clone(),
                    entries: parsed.entries,
                    duplicates: parsed.duplicates,
                }
                .without_metadata(library),
            ),
            // A catalogue that will not parse has no keys to compare and
            // no half-answer to give, unlike a dotenv file where the
            // lines above the break are still evidence.
            Err(reason) => diagnostics.push(Diagnostic {
                severity: "warning".to_string(),
                code: "unparsable".to_string(),
                file: document.name.clone(),
                message: reason,
            }),
        }
    }

    if files.is_empty() {
        return Ok(Report {
            schema: SCHEMA,
            status: Status::NoFiles,
            system,
            source: None,
            files: Vec::new(),
            findings: Vec::new(),
            diagnostics,
            summary: Summary {
                files: 0,
                findings: 0,
            },
        });
    }

    let source = resolve_source(&files, options.source.as_deref())?;
    let Some(set) = audit::Set::new(&files, source) else {
        // `resolve_source` answers with a position in `files`, so there
        // is no path from it to one that is not there. Said as a refusal
        // rather than left as an index, because a bug should not be a
        // panic in a tool whose product is an exit code.
        return Err("the resolved source is not one of the catalogues".to_string());
    };
    let findings = audit::audit(
        set,
        audit::Options {
            library: system.library,
            keys_are_source,
        },
    );

    let summaries: Vec<FileSummary> = files.iter().map(summarise).collect();
    Ok(Report {
        schema: SCHEMA,
        status: if findings.is_empty() {
            Status::Clean
        } else {
            Status::Findings
        },
        source: summaries.get(set.index()).cloned(),
        summary: Summary {
            files: summaries.len(),
            findings: findings.len(),
        },
        files: summaries,
        findings,
        diagnostics,
        system,
    })
}

fn summarise(file: &Catalogue) -> FileSummary {
    FileSummary {
        path: file.path.clone(),
        locale: file.locale.clone(),
        keys: file.key_count(),
    }
}

/// Which catalogue is the contract.
///
/// **Never the union of all of them.** A union would make one locale's
/// typo'd key a requirement every other locale is then missing, so a
/// single translator's slip becomes twenty-four findings against
/// everybody else.
///
/// Auto-detection answers only when there is exactly one candidate, and
/// names the fix when there is not. Guessing between `en.json` and
/// `en-GB.json` would put every later answer on a coin flip.
fn resolve_source(files: &[Catalogue], wanted: Option<&str>) -> Result<usize, String> {
    let Some(wanted) = wanted else {
        return auto_source(files);
    };

    if let Some(index) = files.iter().position(|file| file.path == wanted) {
        return Ok(index);
    }
    if let Some(tag) = crate::locale::canonicalise(wanted)
        && let Some(index) = files
            .iter()
            .position(|file| file.locale.as_deref() == Some(tag.as_str()))
    {
        return Ok(index);
    }
    // Refusals from here speak both surfaces' vocabulary: this text
    // reaches a terminal *and* an agent, and only one of them has a
    // command line. It names the thing, not the flag.
    Err(format!(
        "no catalogue here is called {wanted}. Found: {}",
        listed(files)
    ))
}

fn auto_source(files: &[Catalogue]) -> Result<usize, String> {
    let candidates: Vec<usize> = files
        .iter()
        .enumerate()
        .filter(|(_, file)| is_english(file))
        .map(|(index, _)| index)
        .collect();

    match candidates.as_slice() {
        [only] => Ok(*only),
        [] => Err(format!(
            "no English catalogue here, so say which one is the source. Found: {}",
            listed(files)
        )),
        several => Err(format!(
            "{} catalogues could be the English one, so say which is the source: {}",
            several.len(),
            several
                .iter()
                .map(|index| files[*index].path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// The English catalogue is either tagged `en`-something or untagged —
/// `bundle.l10n.json` and `package.nls.json` both write it without a
/// tag, and both mean English.
fn is_english(file: &Catalogue) -> bool {
    file.locale.as_ref().is_none_or(|locale| {
        locale
            .split('-')
            .next()
            .is_some_and(|language| language == "en")
    })
}

fn listed(files: &[Catalogue]) -> String {
    files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// 0 clean, 1 findings, 2 could not answer.
///
/// **This crate has a verdict**, unlike the extractors in the family.
/// The exit code is the product.
pub(crate) fn exit_code(report: &Report, fail_on: FailOn, strict: bool) -> u8 {
    if strict && !report.diagnostics.is_empty() {
        return 2;
    }
    if report.findings.iter().any(|finding| fail_on.fails(finding)) {
        return 1;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::Kind;
    use crate::testing::TempTree;

    fn document(name: &str, locale: Option<&str>, content: &str) -> Document {
        Document {
            name: name.to_string(),
            locale: locale.map(str::to_string),
            content: content.to_string(),
        }
    }

    fn system(library: Id) -> System {
        System {
            library,
            version: None,
            layout: None,
            keys_are_source: false,
            evidence: Vec::new(),
        }
    }

    fn report(documents: &[Document]) -> Report {
        report_for(documents, system(Id::I18next), &ScanOptions::default()).expect("a report")
    }

    #[test]
    fn matching_catalogues_are_clean_and_exit_zero() {
        let report = report(&[
            document("en.json", Some("en"), r#"{"a":"one"}"#),
            document("es.json", Some("es"), r#"{"a":"uno"}"#),
        ]);
        assert_eq!(report.status, Status::Clean);
        assert_eq!(report.source.as_ref().expect("a source").path, "en.json");
        assert_eq!(exit_code(&report, FailOn::Findings, false), 0);
    }

    #[test]
    fn a_missing_key_exits_one() {
        let report = report(&[
            document("en.json", Some("en"), r#"{"a":"one","b":"two"}"#),
            document("es.json", Some("es"), r#"{"a":"uno"}"#),
        ]);
        assert_eq!(report.status, Status::Findings);
        assert_eq!(report.summary.findings, 1);
        assert_eq!(exit_code(&report, FailOn::Findings, false), 1);
    }

    /// The default that keeps the tool switched on.
    #[test]
    fn an_untranslated_string_does_not_fail_a_run_until_asked() {
        let report = report(&[
            document("en.json", Some("en"), r#"{"a":"Dashboard"}"#),
            document("es.json", Some("es"), r#"{"a":"Dashboard"}"#),
        ]);
        assert_eq!(report.findings[0].kind, Kind::Untranslated);
        assert_eq!(exit_code(&report, FailOn::Findings, false), 0);
        assert_eq!(exit_code(&report, FailOn::Untranslated, false), 1);
        assert_eq!(exit_code(&report, FailOn::Any, false), 1);
    }

    #[test]
    fn no_catalogues_is_not_a_failure() {
        let report = report(&[]);
        assert_eq!(report.status, Status::NoFiles);
        assert_eq!(exit_code(&report, FailOn::Findings, false), 0);
    }

    #[test]
    fn the_source_can_be_named_by_path_or_by_tag() {
        let documents = [
            document("en.json", Some("en"), r#"{"a":"one"}"#),
            document("fr.json", Some("fr"), r#"{"a":"un","b":"deux"}"#),
        ];
        for named in ["fr.json", "fr", "FR"] {
            let report = report_for(
                &documents,
                system(Id::I18next),
                &ScanOptions {
                    source: Some(named.to_string()),
                    ..ScanOptions::default()
                },
            )
            .expect("a report");
            assert_eq!(report.source.expect("a source").path, "fr.json", "{named}");
        }
    }

    /// **Auto-detection answers or refuses, it never guesses.**
    #[test]
    fn two_english_candidates_name_the_fix_rather_than_pick_one() {
        let error = report_for(
            &[
                document("en.json", Some("en"), "{}"),
                document("en-GB.json", Some("en-GB"), "{}"),
                document("es.json", Some("es"), "{}"),
            ],
            system(Id::I18next),
            &ScanOptions::default(),
        )
        .expect_err("a refusal");
        assert!(error.contains("source"), "{error}");
        assert!(error.contains("en-GB.json"), "{error}");
    }

    /// The untagged file in both VS Code layouts is the English one.
    #[test]
    fn an_untagged_catalogue_is_the_english_one() {
        let report = report_for(
            &[
                document("bundle.l10n.json", None, r#"{"Save":"Save"}"#),
                document("bundle.l10n.de.json", Some("de"), r#"{"Save":"Speichern"}"#),
            ],
            System {
                keys_are_source: true,
                ..system(Id::VscodeL10n)
            },
            &ScanOptions::default(),
        )
        .expect("a report");
        assert_eq!(report.source.expect("a source").path, "bundle.l10n.json");
    }

    /// A refusal from the shared layer reaches a terminal **and** an
    /// agent, and only one of them has a command line.
    #[test]
    fn no_refusal_from_the_shared_layer_names_a_flag() {
        let sets: [&[Document]; 3] = [
            &[
                document("en.json", Some("en"), "{}"),
                document("en-GB.json", Some("en-GB"), "{}"),
            ],
            &[document("es.json", Some("es"), "{}")],
            &[document("en.json", Some("en"), "{}")],
        ];
        for (index, documents) in sets.iter().enumerate() {
            let options = ScanOptions {
                source: (index == 2).then(|| "de".to_string()),
                ..ScanOptions::default()
            };
            let error =
                report_for(documents, system(Id::I18next), &options).expect_err("a refusal");
            assert!(!error.contains("--"), "{error}");
        }
    }

    /// **Every free-form string that reaches an answer**, in one place.
    /// `Evidence` cannot hold a sentence and that is checked elsewhere;
    /// these three are `String`s that only review keeps honest, so the
    /// review is written down as a test. A diagnostic carries the
    /// parser's message, a signal carries what was seen and where, and a
    /// summary carries a file name — none of them a value.
    #[test]
    fn no_string_field_in_a_report_carries_a_translated_value() {
        let report = report_for(
            &[
                document("en.json", Some("en"), r#"{"a":"Welcome back","b":"Save"}"#),
                document(
                    "es.json",
                    Some("es"),
                    r#"{"a":"Bienvenido de nuevo","z":"Sobrante"}"#,
                ),
                document("fr.json", Some("fr"), r#"{"a":"Bon retour"#),
            ],
            System {
                version: Some("^26.2.0".to_string()),
                evidence: vec![Signal {
                    class: crate::identify::Class::Manifest,
                    library: Id::I18next,
                    detail: "i18next in ../package.json".to_string(),
                }],
                ..system(Id::I18next)
            },
            &ScanOptions::default(),
        )
        .expect("a report");

        let strings = [
            report
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone())
                .collect::<Vec<_>>(),
            report
                .system
                .evidence
                .iter()
                .map(|signal| signal.detail.clone())
                .collect(),
            report.files.iter().map(|file| file.path.clone()).collect(),
            report.system.version.clone().into_iter().collect(),
        ]
        .concat();
        assert!(!strings.is_empty(), "nothing was checked");
        for carried in strings {
            for translated in ["Bienvenido", "Bon retour", "Sobrante", "Welcome back"] {
                assert!(!carried.contains(translated), "{carried}");
            }
        }
    }

    #[test]
    fn a_catalogue_that_will_not_parse_is_named_rather_than_dropped() {
        let report = report(&[
            document("en.json", Some("en"), r#"{"a":"one"}"#),
            document("es.json", Some("es"), "{ not json"),
        ]);
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].file, "es.json");
        assert_eq!(report.diagnostics[0].code, "unparsable");
        assert_eq!(report.files.len(), 1);
        assert_eq!(exit_code(&report, FailOn::Findings, true), 2);
    }

    #[test]
    fn the_report_counts_keys_rather_than_listing_them() {
        let report = report(&[
            document("en.json", Some("en"), r#"{"a":{"b":"one","c":"two"}}"#),
            document("es.json", Some("es"), r#"{"a":{"b":"uno","c":"dos"}}"#),
        ]);
        assert_eq!(report.files[0].keys, 2, "the object itself is not a key");
    }

    #[test]
    fn the_report_carries_no_clock_and_names_its_schema() {
        let report = report(&[document("en.json", Some("en"), r#"{"a":"one"}"#)]);
        let rendered = serde_json::to_string(&report).expect("serializes");
        for absent in ["timestamp", "lastChecked", "generatedAt", "checkedAt"] {
            assert!(!rendered.contains(absent), "{rendered}");
        }
        assert!(rendered.contains(r#""schema":3"#), "{rendered}");
    }

    #[test]
    fn a_directory_is_identified_read_and_audited() {
        let tree = TempTree::new("scan-directory");
        tree.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#);
        tree.write("src/a.ts", "useTranslation()\n");
        tree.write("locales/en.json", r#"{"a":"Hello {{name}}","b":"two"}"#);
        tree.write("locales/es.json", r#"{"a":"Hola {{name}}"}"#);
        let report = scan(
            &[tree.path().join("locales")],
            Requested::Detect,
            &ScanOptions::default(),
        )
        .expect("scans");
        assert_eq!(report.system.library, Id::I18next);
        assert_eq!(report.summary.files, 2);
        assert_eq!(report.findings[0].kind, Kind::MissingKey);
    }

    /// **Every reported path is relative to the catalogues** — SPEC.md
    /// says so, and the whole reason is that a report is a thing to diff
    /// between machines. A `skipped` diagnostic carried the read error
    /// verbatim, and that error names the file in full: two machines
    /// auditing the same repository produced different reports for the
    /// same defect.
    #[test]
    fn a_diagnostic_names_the_catalogue_and_not_the_machine() {
        let tree = TempTree::new("scan-unreadable");
        tree.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#);
        tree.write("src/a.ts", "useTranslation()\n");
        tree.write("locales/en.json", r#"{"a":"Hello {{name}}"}"#);
        // Not UTF-8 by any reading, so the read fails after the file has
        // already been found and named.
        tree.write_bytes("locales/es.json", b"{\"a\":\"Hola \xff\xfe\"}");

        let report = scan(
            &[tree.path().join("locales")],
            Requested::Detect,
            &ScanOptions::default(),
        )
        .expect("scans");

        let diagnostic = &report.diagnostics[0];
        assert_eq!(diagnostic.code, "skipped");
        assert_eq!(diagnostic.file, "es.json", "the name is the report's");
        let root = tree.path().to_string_lossy().into_owned();
        assert!(
            !diagnostic.message.contains(&root),
            "the machine is in the report: {}",
            diagnostic.message
        );
        assert!(
            !diagnostic.message.contains(std::path::MAIN_SEPARATOR),
            "a path is in the message: {}",
            diagnostic.message
        );
    }

    #[test]
    fn a_missing_directory_is_refused() {
        let tree = TempTree::new("scan-missing");
        assert!(
            scan(
                &[tree.path().join("nope")],
                Requested::Detect,
                &ScanOptions::default()
            )
            .is_err()
        );
    }

    /// **The property the whole crate rests on**, on a real read.
    #[test]
    fn the_report_carries_no_translated_value() {
        let tree = TempTree::new("scan-values");
        tree.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#);
        tree.write("src/a.ts", "useTranslation()\n");
        tree.write("locales/en.json", r#"{"a":"in {{timeframe}}","b":"Save"}"#);
        tree.write(
            "locales/es.json",
            r#"{"a":"en {{periodo}}","z":"sobrante"}"#,
        );
        let report = scan(
            &[tree.path().join("locales")],
            Requested::Detect,
            &ScanOptions::default(),
        )
        .expect("scans");
        let rendered = serde_json::to_string(&report).expect("serializes");
        for translated in ["en {{periodo}}", "sobrante"] {
            assert!(!rendered.contains(translated), "{rendered}");
        }
        assert!(rendered.contains("periodo"), "the token is the finding");
    }
}
