//! The terminal surface.
//!
//! stdout is protocol — **one JSON report for the whole run**, not one
//! line per file, because the answer is about the set of catalogues.
//! stderr is for the human and is a projection of the same report.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::audit::{Evidence, Finding, Kind};
use crate::catalogue::Shape as KeyShape;
use crate::identify::Requested;
use crate::library::{self, Id, Interpolation};
use crate::message::Construct;
use crate::scan::{self, FailOn, Report, ScanOptions, Status};

const USAGE: &str = "usage: i18n-le [options] <dir|file>...
       i18n-le mcp
       i18n-le --version | --help

Audits a set of translation catalogues against one of them and reports
what is structurally wrong: keys a locale is missing or has too many of,
placeholders dropped or renamed in translation, constructs from the
wrong convention, values left empty, keys defined twice, and a path that
is an object in one locale and a string in another.

It works out which i18n library the project uses before it reads
anything, from five kinds of evidence — a manifest dependency, a config
file, the directory layout, the catalogue's own syntax, and call sites
in source. Two agreeing is an identification. The library then supplies
the placeholder grammar, the plural model, which keys are metadata and
how the files are laid out; nothing downstream guesses. If no library
can be identified, the run says which signals it found and stops.

Only key names and structural facts are reported. No translated value
ever reaches stdout, stderr or an agent — a renamed placeholder is
proved by its tokens, and an untranslated string by the fact that it is
byte-identical to the English.

Options:
  --system <name>          the library to audit as, skipping
                           identification
  --source <path|locale>   the catalogue every other is measured against;
                           auto-detected only when exactly one English
                           candidate exists
  --keys-are-source        the key is the English string, as in a VS Code
                           bundle.l10n.json
  --fail-on <what>         untranslated, or any — what else fails the run
                           besides the findings that already do
  --strict                 also fail when a catalogue could not be read
                           or parsed

Exit codes: 0 clean · 1 findings · 2 malformed question. Finding no
catalogues at all is 0 — there is nothing to be wrong with.";

/// Every flag the parser accepts. Held equal to the flags named in USAGE
/// by a test, and consulted at runtime so the list is what the parser
/// actually honours.
const FLAGS: [&str; 5] = [
    "--system",
    "--source",
    "--keys-are-source",
    "--fail-on",
    "--strict",
];

#[derive(Debug)]
struct Cli {
    inputs: Vec<PathBuf>,
    requested: Requested,
    scan: ScanOptions,
    fail_on: FailOn,
    /// Fail the run when a file could not be read.
    strict: bool,
}

pub(crate) fn run() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if let Some(first) = args.first() {
        match first.as_str() {
            "mcp" => return crate::mcp::serve(),
            "--help" | "-h" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--version" | "-V" => {
                println!("i18n-le {}", env!("CARGO_PKG_VERSION"));
                return ExitCode::SUCCESS;
            }
            _ => {}
        }
    }

    match execute(&args) {
        Ok(code) => ExitCode::from(code),
        Err(message) => {
            eprintln!("i18n-le: {message}");
            ExitCode::from(2)
        }
    }
}

fn execute(args: &[String]) -> Result<u8, String> {
    let options = parse(args)?;
    let report = scan::scan(&options.inputs, options.requested, &options.scan)?;

    let line = serde_json::to_string(&report).expect("a report serializes");
    writeln!(std::io::stdout().lock(), "{line}")
        .map_err(|error| format!("could not write the report: {error}"))?;

    summarise(&report);
    Ok(scan::exit_code(&report, options.fail_on, options.strict))
}

fn parse(args: &[String]) -> Result<Cli, String> {
    let mut options = Cli {
        inputs: Vec::new(),
        requested: Requested::Detect,
        scan: ScanOptions::default(),
        fail_on: FailOn::default(),
        strict: false,
    };

    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        // Strict parsing, never a silent default: a typo'd `--surce`
        // that quietly did nothing would auto-detect a different
        // reference and report a clean set that was never checked
        // against the contract asked for.
        if arg.starts_with('-') && !FLAGS.contains(&arg.as_str()) {
            return Err(format!("{arg} is not an option. Try --help."));
        }

        match arg.as_str() {
            "--keys-are-source" => options.scan.keys_are_source = true,
            "--strict" => options.strict = true,
            "--system" => {
                let value = rest.next().ok_or_else(|| {
                    format!("--system needs one of {}", library::names().join(", "))
                })?;
                options.requested = Requested::Named(Id::parse(value).ok_or_else(|| {
                    format!(
                        "{value} is not a library this reads. Try one of {}.",
                        library::names().join(", ")
                    )
                })?);
            }
            "--source" => {
                let value = rest
                    .next()
                    .ok_or_else(|| "--source needs a path or a language tag".to_string())?;
                options.scan.source = Some(value.clone());
            }
            "--fail-on" => {
                let value = rest
                    .next()
                    .ok_or_else(|| "--fail-on needs untranslated or any".to_string())?;
                options.fail_on = FailOn::parse(value)?;
            }
            path => options.inputs.push(PathBuf::from(path)),
        }
    }

    if options.inputs.is_empty() {
        return Err("name the directory holding the catalogues. Try --help.".to_string());
    }
    Ok(options)
}

/// The human half. Every line restates something already on stdout.
///
/// Write failures are dropped rather than reported, and that is the one
/// place in this crate where swallowing is right: the report has already
/// been written, the exit code is already decided, and a closed stderr
/// (`i18n-le … 2>/dev/null`, a `head` upstream) is a caller saying they
/// do not want this half. Turning it into a refusal would fail a run
/// that answered correctly.
fn summarise(report: &Report) {
    let mut stderr = std::io::stderr().lock();

    // First, because it frames everything after it: which library the
    // findings below were judged against, and on what evidence.
    let version = report
        .system
        .version
        .as_ref()
        .map(|version| format!(" {version}"))
        .unwrap_or_default();
    let _ = writeln!(
        stderr,
        "{}{version} — {}",
        report.system.library.as_str(),
        evidence(report)
    );

    for diagnostic in &report.diagnostics {
        let _ = writeln!(stderr, "{}: {}", diagnostic.file, diagnostic.message);
    }
    for finding in &report.findings {
        let _ = writeln!(stderr, "{}", describe(finding));
    }

    let _ = match report.status {
        Status::NoFiles => writeln!(stderr, "no catalogues found"),
        Status::Clean => writeln!(
            stderr,
            "clean — {}",
            plural(report.summary.files, "catalogue", "catalogues")
        ),
        Status::Findings => writeln!(
            stderr,
            "{} across {}",
            plural(report.summary.findings, "finding", "findings"),
            plural(report.summary.files, "catalogue", "catalogues")
        ),
    };
}

fn evidence(report: &Report) -> String {
    if report.system.evidence.is_empty() {
        return "as asked for".to_string();
    }
    report
        .system
        .evidence
        .iter()
        .map(|signal| signal.detail.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// One finding, as a line. Tokens, counts, styles, shapes and offsets
/// only — there is no branch here that can print a translated string,
/// because `Evidence` has no variant that carries one.
fn describe(finding: &Finding) -> String {
    let head = format!("{}: {} {}", finding.file, kind(finding.kind), finding.key);
    match &finding.evidence {
        None => head,
        Some(Evidence::Tokens { source, target }) => {
            format!(
                "{head} ({} became {})",
                source.join(", "),
                target.join(", ")
            )
        }
        Some(Evidence::Counts { source, target }) => {
            format!(
                "{head} ({} became {target})",
                plural(*source, "placeholder", "placeholders")
            )
        }
        Some(Evidence::Styles { source, target }) => {
            format!(
                "{head} ({} became {})",
                interpolation(*source),
                interpolation(*target)
            )
        }
        Some(Evidence::Shapes { source, target }) => {
            format!(
                "{head} ({} here, {} in the source)",
                key_shape(*target),
                key_shape(*source)
            )
        }
        Some(Evidence::Occurrences { occurrences }) => format!("{head} (defined {occurrences}×)"),
        Some(Evidence::Foreign { construct, offset }) => {
            format!(
                "{head} ({} at offset {offset})",
                self::construct(*construct)
            )
        }
    }
}

fn kind(kind: Kind) -> &'static str {
    match kind {
        Kind::MissingKey => "missing",
        Kind::ExtraKey => "extra",
        Kind::PlaceholderCountMismatch => "placeholder count",
        Kind::PlaceholderNameMismatch => "placeholder renamed",
        Kind::PlaceholderStyleMismatch => "placeholder style",
        Kind::ConventionMismatch => "wrong convention in",
        Kind::EmptyValue => "empty",
        Kind::Untranslated => "untranslated",
        Kind::DuplicateKeyWithinFile => "duplicate",
        Kind::StructureMismatch => "structure",
    }
}

fn interpolation(style: Interpolation) -> &'static str {
    match style {
        Interpolation::DoubleBrace => "{{name}}",
        Interpolation::SingleBrace => "{name}",
        Interpolation::Positional => "{0}",
    }
}

fn key_shape(shape: KeyShape) -> &'static str {
    match shape {
        KeyShape::Text => "a string",
        KeyShape::Object => "an object",
        KeyShape::Other => "neither a string nor an object",
    }
}

/// The kebab spelling the report uses, so a terminal line and a JSON
/// field name the same thing.
fn construct(construct: Construct) -> &'static str {
    match construct {
        Construct::IcuArgument => "icu-argument",
        Construct::Fluent => "fluent",
        Construct::Printf => "printf",
        Construct::TemplateLiteral => "template-literal",
        Construct::Nesting => "nesting",
    }
}

fn plural(count: usize, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::Severity;
    use crate::library::Shape;

    #[test]
    fn every_documented_flag_is_parsed_and_the_reverse() {
        let mut documented: Vec<&str> = USAGE
            .split_whitespace()
            .filter(|word| word.starts_with("--"))
            .map(|word| word.trim_end_matches([',', '.', ':', ';']))
            .filter(|word| !matches!(*word, "--version" | "--help"))
            .collect();
        documented.sort_unstable();
        documented.dedup();

        let mut implemented = FLAGS.to_vec();
        implemented.sort_unstable();
        assert_eq!(documented, implemented);
    }

    #[test]
    fn the_parser_accepts_every_flag_it_lists() {
        for flag in FLAGS {
            let args: Vec<String> = match flag {
                "--system" => vec![flag.into(), "i18next".into(), "dir".into()],
                "--source" => vec![flag.into(), "en".into(), "dir".into()],
                "--fail-on" => vec![flag.into(), "any".into(), "dir".into()],
                _ => vec![flag.into(), "dir".into()],
            };
            assert!(parse(&args).is_ok(), "{flag}");
        }
    }

    #[test]
    fn an_unknown_flag_is_refused_rather_than_ignored() {
        let error = parse(&["--surce".into(), "en".into(), "dir".into()]).expect_err("a refusal");
        assert!(error.contains("--surce"), "{error}");
    }

    #[test]
    fn a_flag_with_no_value_is_refused() {
        for flag in ["--system", "--source", "--fail-on"] {
            assert!(parse(&[flag.into()]).is_err(), "{flag}");
        }
    }

    #[test]
    fn a_library_this_does_not_read_is_refused_and_the_known_ones_are_listed() {
        let error =
            parse(&["--system".into(), "gettext".into(), "dir".into()]).expect_err("a refusal");
        assert!(error.contains("gettext"), "{error}");
        for known in library::names() {
            assert!(error.contains(known), "{error} omits {known}");
        }
    }

    #[test]
    fn a_fail_on_value_that_is_not_one_is_refused_by_name() {
        let error =
            parse(&["--fail-on".into(), "everything".into(), "dir".into()]).expect_err("refusal");
        assert!(error.contains("everything"), "{error}");
    }

    /// There is no working-directory default. A bare `i18n-le` would
    /// have to guess which directory holds the catalogues.
    #[test]
    fn naming_nothing_is_refused() {
        let error = parse(&[]).expect_err("a refusal");
        assert!(error.contains("name the directory"), "{error}");
    }

    /// The tool reads structure, never a translation, and no flag may
    /// suggest otherwise.
    #[test]
    fn no_flag_offers_to_print_a_value() {
        for attempt in [
            "--show-values",
            "--values",
            "--print-strings",
            "--fix",
            "--write",
        ] {
            assert!(
                parse(&[attempt.into(), "dir".into()]).is_err(),
                "{attempt} was accepted"
            );
        }
        for word in ["--show-values", "translate for you", "rewrite"] {
            assert!(!USAGE.contains(word), "the usage text offers {word}");
        }
    }

    #[test]
    fn the_usage_text_states_the_exit_codes_and_the_identification_rule() {
        for code in ["0", "1", "2"] {
            assert!(USAGE.contains(code), "exit code {code} is undocumented");
        }
        assert!(
            USAGE.contains("Two agreeing is an identification"),
            "the rule is unexplained"
        );
        assert!(USAGE.contains("call sites"), "the fifth class is unsaid");
    }

    /// A finding line can only say what `Evidence` carries, and
    /// `Evidence` cannot carry a sentence.
    #[test]
    fn a_finding_line_carries_tokens_and_never_a_string() {
        let line = describe(&Finding {
            severity: Severity::Error,
            kind: Kind::PlaceholderNameMismatch,
            file: "es.json".to_string(),
            key: "metrics.window".to_string(),
            evidence: Some(Evidence::Tokens {
                source: vec!["timeframe".to_string()],
                target: vec!["periodo".to_string()],
            }),
        });
        assert_eq!(
            line,
            "es.json: placeholder renamed metrics.window (timeframe became periodo)"
        );
    }

    /// Every shape the report can carry has a human spelling, so a new
    /// one cannot reach a terminal as a debug dump.
    #[test]
    fn every_layout_shape_is_describable() {
        for library in library::all() {
            for layout in library.layouts {
                let rendered = serde_json::to_string(&layout.shape).expect("serializes");
                assert!(rendered.contains("shape"), "{rendered}");
                assert!(
                    matches!(
                        layout.shape,
                        Shape::Shared { .. } | Shape::Fixed { .. } | Shape::Namespaced { .. }
                    ),
                    "{:?}",
                    layout.shape
                );
            }
        }
    }
}
