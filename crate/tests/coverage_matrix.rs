//! Does this crate find what it claims to find, and say everything it
//! claims to say?
//!
//! Five questions, all mechanical, all previously answered by hand:
//!
//! 1. **Every library is identifiable from a real project** — through
//!    the walk and the built binary, not through a unit call on
//!    `decide`. A row nothing reaches is a library this tool advertises
//!    and has never actually identified.
//! 2. **Every finding kind is reachable from real catalogues**, and
//!    nothing produced is outside the documented list. A kind the code
//!    emits and SPEC.md does not name is a value a reader cannot look
//!    up; a kind SPEC.md names and nothing produces is a promise nobody
//!    has checked.
//! 3. **Every `Mark` is reachable**, because a mark is how a library
//!    gets identified from its catalogues alone — the class that makes a
//!    translations-only repository auditable.
//! 4. **Every refusal reason is reachable**, and each writes no report.
//!    The refusals *are* the product here: this crate answers a
//!    yes-or-no question, and "I cannot tell" is one of the answers.
//! 5. **The vocabulary and the documentation agree.**
//!
//! Ubuntu only: nothing here is platform-dependent, and running it three
//! times would only buy three chances to trip over a case-folding
//! filesystem while asserting nothing new.
//!
//! **The marker lines at the end are not decoration.** `cargo test
//! <filter>` exits 0 when the filter matches nothing, so a renamed test
//! would pass this job silently; CI greps for the markers instead.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

const BINARY: &str = env!("CARGO_BIN_EXE_i18n-le");
const SPEC: &str = include_str!("../SPEC.md");
const CORPUS: &str = include_str!("../fixtures/detection.json");

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// The vocabulary, repeated here because an integration test cannot see
/// a `pub(crate)` enum. Held equal to the code by the assertions below:
/// every name must be produced by a real run, and nothing produced may
/// be missing from these lists.
const LIBRARIES: [&str; 4] = ["i18next", "next-intl", "vscode-l10n", "flutter-arb"];

const KINDS: [&str; 10] = [
    "missing-key",
    "extra-key",
    "placeholder-count-mismatch",
    "placeholder-name-mismatch",
    "placeholder-style-mismatch",
    "convention-mismatch",
    "empty-value",
    "untranslated",
    "duplicate-key-within-file",
    "structure-mismatch",
];

const SEVERITIES: [&str; 3] = ["error", "warning", "info"];

const CLASSES: [&str; 5] = ["manifest", "config", "layout", "content", "call-site"];

const MARKS: [&str; 8] = [
    "double-brace",
    "icu-argument",
    "positional",
    "single-brace",
    "dollar-t",
    "plural-key-suffix",
    "arb-metadata",
    "sentence-keys",
];

const CONSTRUCTS: [&str; 5] = [
    "icu-argument",
    "fluent",
    "printf",
    "template-literal",
    "nesting",
];

const STATUSES: [&str; 3] = ["clean", "findings", "no-files"];

const SHAPES: [&str; 3] = ["shared", "fixed", "namespaced"];

const DIAGNOSTICS: [&str; 2] = ["skipped", "unparsable"];

struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new(name: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "i18n-le-matrix-{name}-{}-{unique}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a temporary directory");
        Self {
            root: std::fs::canonicalize(&root).expect("a canonical directory"),
        }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative: &str, contents: &str) -> &Self {
        self.write_bytes(relative, contents.as_bytes())
    }

    fn write_bytes(&self, relative: &str, contents: &[u8]) -> &Self {
        let target = self.root.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("a parent directory");
        }
        std::fs::write(&target, contents).expect("a file");
        self
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(args: &[&str]) -> Run {
    let output = Command::new(BINARY)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("the binary runs");
    Run {
        code: output.status.code().expect("an exit code, not a signal"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// One answer. 0 or 1, never 2: a refusal writes no report, and the
/// refusal half of this matrix runs separately.
fn answer(args: &[&str]) -> serde_json::Value {
    let run = run(args);
    assert!(
        matches!(run.code, 0 | 1),
        "{args:?} was refused: {}",
        run.stderr
    );
    serde_json::from_str(run.stdout.trim())
        .unwrap_or_else(|error| panic!("{args:?}: stdout is not one report ({error})"))
}

fn strings(value: &serde_json::Value, at: &str, field: &str) -> BTreeSet<String> {
    value[at]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|entry| entry[field].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn missing<'a>(declared: &[&'a str], seen: &BTreeSet<String>) -> Vec<&'a str> {
    declared
        .iter()
        .filter(|name| !seen.contains(**name))
        .copied()
        .collect()
}

fn undeclared(declared: &[&str], seen: &BTreeSet<String>) -> Vec<String> {
    seen.iter()
        .filter(|name| !declared.contains(&name.as_str()))
        .cloned()
        .collect()
}

/// A project per library, each identifying the way that library really
/// is identified: a manifest and a call site for the two npm ones, the
/// editor's own `l10n` field for VS Code, and nothing at all for ARB —
/// whose decisive content signature is the whole reason a
/// translations-only repository is auditable.
fn project(library: &str) -> Tree {
    let tree = Tree::new(library);
    match library {
        "i18next" => {
            tree.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#)
                .write("src/app.ts", "const { t } = useTranslation()\n")
                .write("locales/en.json", r#"{"a":"Hi {{name}}","b":"Bye"}"#)
                .write("locales/es.json", r#"{"a":"Hola {{nombre}}"}"#);
        }
        "next-intl" => {
            tree.write("package.json", r#"{"dependencies":{"next-intl":"^4.0.0"}}"#)
                .write("src/page.tsx", "useTranslations()\n")
                .write(
                    "messages/en.json",
                    r#"{"a":"{count, plural, one {# item} other {# items}}"}"#,
                )
                .write("messages/es.json", r#"{"a":"elementos"}"#);
        }
        "vscode-l10n" => {
            tree.write("package.json", r#"{"name":"x","l10n":"./l10n"}"#)
                .write("src/extension.ts", "vscode.l10n.t('Save {0}')\n")
                .write("l10n/bundle.l10n.json", r#"{"Save {0}":"Save {0}"}"#)
                .write(
                    "l10n/bundle.l10n.de.json",
                    r#"{"Save {0}":"Speichern {0}"}"#,
                );
        }
        "flutter-arb" => {
            tree.write(
                "l10n/app_en.arb",
                r#"{"@@locale":"en","greeting":"Hi {name}","@greeting":{"description":"x"}}"#,
            )
            .write(
                "l10n/app_es.arb",
                r#"{"@@locale":"es","greeting":"Hola {nombre}"}"#,
            );
        }
        other => panic!("no project for {other}"),
    }
    tree
}

fn catalogues_of(library: &str) -> &'static str {
    match library {
        "i18next" => "locales",
        "next-intl" => "messages",
        _ => "l10n",
    }
}

#[test]
fn every_library_is_identifiable_from_a_real_project() {
    let mut identified = BTreeSet::new();
    let mut shapes = BTreeSet::new();
    let mut classes = BTreeSet::new();

    for library in LIBRARIES {
        let tree = project(library);
        let report = answer(&[&tree.path().join(catalogues_of(library)).to_string_lossy()]);
        let named = report["system"]["library"]
            .as_str()
            .unwrap_or_else(|| panic!("{library}: an answer with no library"));
        assert_eq!(named, library, "{library} identified as {named}");
        identified.insert(named.to_string());
        if let Some(shape) = report["system"]["layout"]["shape"].as_str() {
            shapes.insert(shape.to_string());
        }
        classes.extend(strings(&report["system"], "evidence", "class"));
    }

    // The namespaced layout is i18next's other shape and needs its own
    // tree, because one directory cannot be both.
    let namespaced = Tree::new("namespaced");
    namespaced
        .write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#)
        .write("src/a.ts", "useTranslation()\n")
        .write("locales/en/common.json", r#"{"a":"Hi {{name}}"}"#)
        .write("locales/de/common.json", r#"{"a":"Hallo {{name}}"}"#);
    let report = answer(&[&namespaced.path().join("locales").to_string_lossy()]);
    if let Some(shape) = report["system"]["layout"]["shape"].as_str() {
        shapes.insert(shape.to_string());
    }

    // A config file, which none of the four projects above needs.
    let configured = Tree::new("configured");
    configured
        .write("l10n.yaml", "arb-dir: lib/l10n\n")
        .write("lib/l10n/app_en.arb", r#"{"greeting":"Hi {name}"}"#)
        .write("lib/l10n/app_es.arb", r#"{"greeting":"Hola {name}"}"#);
    let report = answer(&[&configured.path().join("lib/l10n").to_string_lossy()]);
    classes.extend(strings(&report["system"], "evidence", "class"));

    assert!(
        missing(&LIBRARIES, &identified).is_empty(),
        "no project identifies these libraries: {:?}",
        missing(&LIBRARIES, &identified)
    );
    assert!(
        missing(&SHAPES, &shapes).is_empty(),
        "no project reaches these layout shapes: {:?}",
        missing(&SHAPES, &shapes)
    );
    assert!(
        missing(&CLASSES, &classes).is_empty(),
        "no project produces these evidence classes: {:?}",
        missing(&CLASSES, &classes)
    );
    assert!(
        undeclared(&CLASSES, &classes).is_empty(),
        "these evidence classes are produced and undocumented: {:?}",
        undeclared(&CLASSES, &classes)
    );

    println!(
        "coverage-matrix: {} libraries, {} layout shapes, {} evidence classes reachable",
        identified.len(),
        shapes.len(),
        classes.len()
    );
}

#[test]
fn every_finding_kind_is_reachable_and_nothing_else_is_produced() {
    let tree = Tree::new("kinds");
    tree.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#)
        .write("src/a.ts", "useTranslation()\n")
        // Every check the tool documents, planted once each. `en.json`
        // carries the convention mismatch, so it is the source's own
        // finding as well as the target's.
        .write(
            "locales/en.json",
            r#"{"kept":"Hi","dropped":"{{a}} and {{b}}","renamed":"in {{timeframe}}",
                "styled":"{{name}}","gone":"Bye","empty":"Cancel","same":"Dashboard",
                "shape":{"inner":"x"},"foreign":"{n, plural, other {#}}"}"#,
        )
        .write(
            "locales/es.json",
            r#"{"kept":"Hola","dropped":"{{a}}","renamed":"en {{periodo}}",
                "styled":"{name}","empty":"   ","same":"Dashboard","shape":"uno",
                "extra":"sobrante","foreign":"x","dup":"a","dup":"b"}"#,
        );

    let report = answer(&[
        "--fail-on",
        "any",
        &tree.path().join("locales").to_string_lossy(),
    ]);
    let kinds = strings(&report, "findings", "kind");
    let severities = strings(&report, "findings", "severity");
    let constructs: BTreeSet<String> = report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter_map(|finding| finding["construct"].as_str().map(str::to_string))
        .collect();

    assert!(
        missing(&KINDS, &kinds).is_empty(),
        "no catalogue produces these finding kinds: {:?}",
        missing(&KINDS, &kinds)
    );
    assert!(
        undeclared(&KINDS, &kinds).is_empty(),
        "these finding kinds are produced and undocumented: {:?}",
        undeclared(&KINDS, &kinds)
    );
    assert!(
        missing(&SEVERITIES, &severities).is_empty(),
        "no finding carries these severities: {:?}",
        missing(&SEVERITIES, &severities)
    );
    assert!(
        undeclared(&SEVERITIES, &severities).is_empty(),
        "undocumented severities: {:?}",
        undeclared(&SEVERITIES, &severities)
    );
    assert!(
        constructs.contains("icu-argument"),
        "the convention mismatch named no construct: {constructs:?}"
    );

    println!(
        "coverage-matrix: {} finding kinds and {} severities reachable from real catalogues",
        kinds.len(),
        severities.len()
    );
}

/// Every foreign construct the report can name, each from the library
/// that does *not* write it. A construct nothing reaches is a
/// `convention-mismatch` this tool claims and has never produced.
#[test]
fn every_foreign_construct_is_reachable() {
    let tree = Tree::new("constructs");
    tree.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#)
        .write("src/a.ts", "useTranslation()\n")
        .write(
            "locales/en.json",
            r#"{"icu":"{n, plural, other {#}}","fluent":"Hi { $name }",
                "printf":"Rank %1$s","template":"Hi ${name}","keep":"{{name}}"}"#,
        )
        .write("locales/es.json", r#"{"keep":"{{name}}"}"#);
    let i18next = answer(&[
        "--fail-on",
        "any",
        &tree.path().join("locales").to_string_lossy(),
    ]);

    // `$t(` is a key reference under i18next and foreign everywhere
    // else, so the fifth construct needs a different library.
    let other = Tree::new("nesting");
    other
        .write("package.json", r#"{"dependencies":{"next-intl":"^4.0.0"}}"#)
        .write("src/a.tsx", "useTranslations()\n")
        .write("messages/en.json", r#"{"a":"$t(common.hello)"}"#)
        .write("messages/es.json", r#"{"a":"hola"}"#);
    let next_intl = answer(&[
        "--fail-on",
        "any",
        &other.path().join("messages").to_string_lossy(),
    ]);

    let mut constructs: BTreeSet<String> = BTreeSet::new();
    for report in [&i18next, &next_intl] {
        constructs.extend(
            report["findings"]
                .as_array()
                .expect("findings")
                .iter()
                .filter_map(|finding| finding["construct"].as_str().map(str::to_string)),
        );
    }

    assert!(
        missing(&CONSTRUCTS, &constructs).is_empty(),
        "no catalogue produces these constructs: {:?}",
        missing(&CONSTRUCTS, &constructs)
    );
    assert!(
        undeclared(&CONSTRUCTS, &constructs).is_empty(),
        "undocumented constructs: {:?}",
        undeclared(&CONSTRUCTS, &constructs)
    );
    println!(
        "coverage-matrix: {} foreign constructs reachable",
        constructs.len()
    );
}

/// Every syntactic mark, checked against the corpus that pins them —
/// the file `fixtures/detection.json` exists to hold, read here so a
/// mark nothing in the corpus carries is a mark identification cannot
/// actually use.
#[test]
fn every_mark_is_carried_by_the_corpus() {
    let corpus: serde_json::Value = serde_json::from_str(CORPUS).expect("the corpus is valid JSON");
    let seen: BTreeSet<String> = corpus["marks"]
        .as_array()
        .expect("the corpus has a marks section")
        .iter()
        .flat_map(|case| {
            case["marks"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|mark| mark.as_str().map(str::to_string))
        })
        .collect();

    assert!(
        missing(&MARKS, &seen).is_empty(),
        "no corpus document carries these marks: {:?}",
        missing(&MARKS, &seen)
    );
    assert!(
        undeclared(&MARKS, &seen).is_empty(),
        "the corpus names marks the code does not: {:?}",
        undeclared(&MARKS, &seen)
    );
    println!(
        "coverage-matrix: {} marks carried by the corpus",
        seen.len()
    );
}

/// A refusal must write nothing to the protocol stream, exit 2, and say
/// on stderr what it could not answer. Checked once, here, for every
/// case below.
fn refused(name: &str, args: &[&str], expected: &str) {
    let run = run(args);
    assert_eq!(run.code, 2, "{name}: {}", run.stderr);
    assert!(
        run.stdout.is_empty(),
        "{name}: a refusal wrote to the protocol stream"
    );
    assert!(
        run.stderr.contains(expected),
        "{name}: the refusal does not say {expected:?}: {}",
        run.stderr
    );
}

/// **The refusals are the product.** This crate answers a yes-or-no
/// question and "I cannot tell" is one of the answers, so every way of
/// reaching one is exercised. These are the malformed *questions*: a
/// flag that is not one, a value that is not allowed, a path that is not
/// there.
#[test]
fn every_malformed_question_is_refused_and_writes_no_report() {
    let tree = Tree::new("malformed");
    let missing = tree.path().join("nope").to_string_lossy().into_owned();
    let cases: [(&str, Vec<&str>, &str); 5] = [
        ("no input named", vec![], "name the directory"),
        (
            "an unknown flag",
            vec!["--surce", "en", "."],
            "is not an option",
        ),
        (
            "a library this does not read",
            vec!["--system", "gettext", "."],
            "not a library this reads",
        ),
        (
            "a --fail-on value that is not one",
            vec!["--fail-on", "everything", "."],
            "--fail-on takes",
        ),
        ("a path that is not there", vec![&missing], "nope"),
    ];
    for (name, args, expected) in &cases {
        refused(name, args, expected);
    }
    println!(
        "coverage-matrix: {} malformed questions refused, none writing a report",
        cases.len()
    );
}

/// And these are the refusals identification itself produces — the ones
/// that exist because guessing is what this crate stopped doing.
#[test]
fn every_identification_refusal_is_reachable_and_writes_no_report() {
    let unidentified = Tree::new("unidentified");
    unidentified
        .write("strings/en.json", r#"{"a":"one"}"#)
        .write("strings/es.json", r#"{"a":"uno"}"#);
    let conflicted = Tree::new("conflicted");
    conflicted
        .write(
            "package.json",
            r#"{"dependencies":{"i18next":"^26.0.0","next-intl":"^4.0.0"}}"#,
        )
        .write("src/a.tsx", "useTranslation()\nuseTranslations()\n")
        .write("messages/en.json", r#"{"a":"Hi {{name}}"}"#)
        .write("messages/es.json", r#"{"a":"Hola {{name}}"}"#);
    let old = Tree::new("old-major");
    old.write("package.json", r#"{"dependencies":{"i18next":"^21.0.0"}}"#)
        .write("src/a.ts", "useTranslation()\n")
        .write("locales/en.json", r#"{"a":"Hi {{name}}"}"#)
        .write("locales/es.json", r#"{"a":"Hola {{name}}"}"#);
    let namespaces = Tree::new("namespaces");
    namespaces
        .write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#)
        .write("src/a.ts", "useTranslation()\n")
        .write("locales/en/common.json", r#"{"a":"Hi {{name}}"}"#)
        .write("locales/en/errors.json", "{}")
        .write("locales/de/common.json", r#"{"a":"Hallo {{name}}"}"#)
        .write("locales/de/errors.json", "{}");
    let root = Tree::new("not-a-set");
    root.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#)
        .write("src/a.ts", "useTranslation()\n")
        .write("tsconfig.json", "{}");
    let ambiguous = Tree::new("ambiguous");
    ambiguous
        .write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#)
        .write("src/a.ts", "useTranslation()\n")
        .write("locales/en.json", r#"{"a":"Hi {{name}}"}"#)
        .write("locales/en-GB.json", r#"{"a":"Hi {{name}}"}"#)
        .write("locales/es.json", r#"{"a":"Hola {{name}}"}"#);

    let at = |tree: &Tree, under: &str| tree.path().join(under).to_string_lossy().into_owned();
    let unidentified = at(&unidentified, "strings");
    let conflicted = at(&conflicted, "messages");
    let old = at(&old, "locales");
    let namespaces = at(&namespaces, "locales");
    let root = root.path().to_string_lossy().into_owned();
    let ambiguous = at(&ambiguous, "locales");

    let cases: [(&str, Vec<&str>, &str); 6] = [
        ("nothing identified", vec![&unidentified], "no i18n library"),
        (
            "two libraries identified",
            vec![&conflicted],
            "only one can be right",
        ),
        ("a breaking major", vec![&old], "differently"),
        (
            "several namespaces",
            vec![&namespaces],
            "namespace is its own set",
        ),
        // Past identification, so the library is named: these are the
        // refusals about the *files*.
        (
            "a directory that is not a catalogue set",
            vec!["--system", "i18next", &root],
            "language tag",
        ),
        (
            "two English candidates",
            vec!["--system", "i18next", &ambiguous],
            "could be the English one",
        ),
    ];
    for (name, args, expected) in &cases {
        refused(name, args, expected);
    }
    println!(
        "coverage-matrix: {} identification refusals reachable, none writing a report",
        cases.len()
    );
}

/// Both diagnostic codes, and both statuses a report can carry besides
/// `findings`. A code nothing produces is a code a consumer branches on
/// and never sees.
#[test]
fn every_diagnostic_code_and_status_is_reachable() {
    let tree = Tree::new("diagnostics");
    tree.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#)
        .write("src/a.ts", "useTranslation()\n")
        .write("locales/en.json", r#"{"a":"Hi {{name}}","b":"Bye"}"#)
        // One readable target, so the run has findings as well as
        // diagnostics — `findings` is a status and needs reaching too.
        .write("locales/de.json", r#"{"a":"Hallo {{name}}"}"#)
        .write("locales/es.json", "{ not json")
        .write_bytes("locales/fr.json", b"{\"a\":\"Salut \xff\xfe\"}");
    let report = answer(&[&tree.path().join("locales").to_string_lossy()]);
    let codes = strings(&report, "diagnostics", "code");
    assert!(
        missing(&DIAGNOSTICS, &codes).is_empty(),
        "no catalogue produces these diagnostics: {:?}",
        missing(&DIAGNOSTICS, &codes)
    );
    assert!(
        undeclared(&DIAGNOSTICS, &codes).is_empty(),
        "undocumented diagnostics: {:?}",
        undeclared(&DIAGNOSTICS, &codes)
    );

    let mut statuses = BTreeSet::new();
    statuses.insert(report["status"].as_str().expect("a status").to_string());

    let clean = Tree::new("clean");
    clean
        .write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#)
        .write("src/a.ts", "useTranslation()\n")
        .write("locales/en.json", r#"{"a":"Hi {{name}}"}"#)
        .write("locales/es.json", r#"{"a":"Hola {{name}}"}"#);
    statuses.insert(
        answer(&[&clean.path().join("locales").to_string_lossy()])["status"]
            .as_str()
            .expect("a status")
            .to_string(),
    );

    let empty = Tree::new("empty");
    empty
        .write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#)
        .write("src/a.ts", "useTranslation()\n")
        .write("locales/README.md", "nothing here\n");
    statuses.insert(
        answer(&[
            "--system",
            "i18next",
            &empty.path().join("locales").to_string_lossy(),
        ])["status"]
            .as_str()
            .expect("a status")
            .to_string(),
    );

    assert!(
        missing(&STATUSES, &statuses).is_empty(),
        "no run produces these statuses: {:?}",
        missing(&STATUSES, &statuses)
    );
    println!(
        "coverage-matrix: {} diagnostic codes and {} statuses reachable",
        codes.len(),
        statuses.len()
    );
}

/// The vocabulary and the documentation are one contract. A name the
/// tool reports and SPEC.md does not carry is a value a reader has no
/// way to look up.
#[test]
fn every_name_in_the_vocabulary_is_documented() {
    for name in KINDS
        .iter()
        .chain(LIBRARIES.iter())
        .chain(SEVERITIES.iter())
        .chain(CONSTRUCTS.iter())
        .chain(STATUSES.iter())
        .chain(DIAGNOSTICS.iter())
        .chain(SHAPES.iter())
    {
        assert!(SPEC.contains(name), "SPEC.md does not name {name}");
    }

    // `--system` is the one flag whose values are only discoverable from
    // the help text, and every library has to be there.
    let help = run(&["--help"]).stdout;
    for library in LIBRARIES {
        assert!(help.contains(library), "--help does not name {library}");
    }
    println!(
        "coverage-matrix: {} vocabulary names documented in SPEC.md",
        KINDS.len()
            + LIBRARIES.len()
            + SEVERITIES.len()
            + CONSTRUCTS.len()
            + STATUSES.len()
            + DIAGNOSTICS.len()
            + SHAPES.len()
    );
}
