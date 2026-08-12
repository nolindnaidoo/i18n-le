//! The exit codes and the stdout contract, driven against the built
//! binary.
//!
//! These are the API — and here more than anywhere else in the family,
//! because this crate's whole product is the exit code. A shell branches
//! on it, so it is pinned by tests that drive the real binary against a
//! real directory.
//!
//! A new refusal adds its case here.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

const BINARY: &str = env!("CARGO_BIN_EXE_i18n-le");
static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new(name: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "i18n-le-contract-{name}-{}-{unique}",
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

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let target = self.root.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("a parent directory");
        }
        std::fs::write(&target, contents).expect("a file");
        target
    }

    /// A project that identifies as i18next: the manifest declares it and
    /// a source file calls it.
    fn i18next(&self) -> &Self {
        self.write("package.json", r#"{"dependencies":{"i18next":"^26.2.0"}}"#);
        self.write("src/app.ts", "const { t } = useTranslation()\n");
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
        .output()
        .expect("the binary runs");
    Run {
        code: output.status.code().expect("an exit code"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// The whole report, parsed. Doubles as the assertion that stdout is one
/// JSON object and nothing else.
fn report(run: &Run) -> serde_json::Value {
    serde_json::from_str(run.stdout.trim()).expect("stdout carries one JSON report")
}

fn locales(tree: &Tree) -> String {
    tree.path().join("locales").to_string_lossy().into_owned()
}

/// A set that is behind: a missing key, a renamed placeholder, an empty
/// value and a string nobody has translated yet.
fn drifted(name: &str) -> Tree {
    let tree = Tree::new(name);
    tree.i18next();
    tree.write(
        "locales/en.json",
        r#"{"title":"Dashboard","window":"in {{timeframe}}","save":"Save","cancel":"Cancel"}"#,
    );
    tree.write(
        "locales/es.json",
        r#"{"title":"Dashboard","window":"en {{periodo}}","save":""}"#,
    );
    tree
}

// ---------------------------------------------------------------------
// Identification — the front door
// ---------------------------------------------------------------------

/// **The premise of the whole tool.** Identification runs first, from
/// evidence, and everything after it is read off the answer.
#[test]
fn the_library_is_identified_before_anything_is_read() {
    let tree = drifted("identified");
    let run = run(&[&locales(&tree)]);
    let report = report(&run);
    assert_eq!(report["system"]["library"], "i18next");
    assert_eq!(report["system"]["version"], "^26.2.0");
    assert_eq!(report["system"]["layout"]["shape"], "shared");

    let classes: Vec<&str> = report["system"]["evidence"]
        .as_array()
        .expect("evidence")
        .iter()
        .filter_map(|signal| signal["class"].as_str())
        .collect();
    assert!(classes.contains(&"manifest"), "{classes:?}");
    assert!(classes.contains(&"content"), "{classes:?}");
    assert!(classes.contains(&"call-site"), "{classes:?}");
    assert!(run.stderr.contains("i18next ^26.2.0"), "{}", run.stderr);
}

/// **No library, no audit.** There is no fallback that guesses.
#[test]
fn an_unidentifiable_project_exits_two_and_says_what_it_found() {
    let tree = Tree::new("unidentified");
    tree.write("strings/en.json", r#"{"a":"one"}"#);
    tree.write("strings/es.json", r#"{"a":"uno"}"#);
    let run = run(&[&tree.path().join("strings").to_string_lossy()]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("no i18n library"), "{}", run.stderr);
    assert!(run.stderr.contains("--system"), "{}", run.stderr);
    assert!(run.stdout.is_empty(), "a refusal writes no report");
}

/// One class of evidence is not corroboration.
#[test]
fn one_class_of_evidence_is_not_an_identification() {
    let tree = Tree::new("one-class");
    tree.write("strings/en.json", r#"{"a":"Hello {{name}}"}"#);
    tree.write("strings/es.json", r#"{"a":"Hola {{name}}"}"#);
    let run = run(&[&tree.path().join("strings").to_string_lossy()]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("not enough to agree"), "{}", run.stderr);
    assert!(run.stderr.contains("double-brace"), "{}", run.stderr);
}

/// **Never pick between two answers.**
#[test]
fn two_identified_libraries_exit_two_and_name_both() {
    let tree = Tree::new("conflict");
    tree.write(
        "package.json",
        r#"{"dependencies":{"i18next":"^26.0.0","next-intl":"^4.0.0"}}"#,
    );
    tree.write("src/a.tsx", "useTranslation()\nuseTranslations()\n");
    tree.write("messages/en.json", r#"{"a":"Hello {{name}}"}"#);
    tree.write("messages/es.json", r#"{"a":"Hola {{name}}"}"#);
    let run = run(&[&tree.path().join("messages").to_string_lossy()]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("i18next"), "{}", run.stderr);
    assert!(run.stderr.contains("next-intl"), "{}", run.stderr);
    assert!(
        run.stderr.contains("only one can be right"),
        "{}",
        run.stderr
    );
}

/// **A translations-only repository.** No manifest, no config, no call
/// sites — and ARB's metadata shape identifies it anyway.
#[test]
fn a_decisive_signature_identifies_with_no_project_around_it() {
    let tree = Tree::new("decisive");
    tree.write(
        "l10n/app_en.arb",
        r#"{"@@locale":"en","greeting":"Hi {name}","@greeting":{"description":"x"}}"#,
    );
    tree.write(
        "l10n/app_es.arb",
        r#"{"@@locale":"es","greeting":"Hola {nombre}"}"#,
    );
    let run = run(&[&tree.path().join("l10n").to_string_lossy()]);
    assert_eq!(run.code, 1, "{}", run.stderr);
    let report = report(&run);
    assert_eq!(report["system"]["library"], "flutter-arb");
    assert_eq!(
        report["findings"][0]["kind"], "placeholder-name-mismatch",
        "{}",
        run.stderr
    );
}

/// The override, which is what makes a repository nothing points at
/// auditable.
#[test]
fn a_named_library_needs_no_evidence() {
    let tree = Tree::new("named");
    tree.write("strings/en.json", r#"{"a":"Hello {{name}}"}"#);
    tree.write("strings/es.json", r#"{"a":"Hola {{nombre}}"}"#);
    let path = tree.path().join("strings").to_string_lossy().into_owned();

    assert_eq!(run(&[&path]).code, 2, "nothing identifies it");
    let named = run(&["--system", "i18next", &path]);
    assert_eq!(named.code, 1, "{}", named.stderr);
    assert_eq!(
        report(&named)["findings"][0]["kind"],
        "placeholder-name-mismatch"
    );
}

/// **The one version gate**, and it names a real break rather than an
/// allowlist: i18next moved plural key suffixes between v21 and v23.
#[test]
fn a_breaking_major_exits_two_and_names_the_version() {
    let tree = Tree::new("old-major");
    tree.write("package.json", r#"{"dependencies":{"i18next":"^21.6.0"}}"#);
    tree.write("src/a.ts", "useTranslation()\n");
    tree.write("locales/en.json", r#"{"a":"Hello {{name}}"}"#);
    tree.write("locales/es.json", r#"{"a":"Hola {{name}}"}"#);
    let path = locales(&tree);

    let refused = run(&[&path]);
    assert_eq!(refused.code, 2);
    assert!(refused.stderr.contains("21"), "{}", refused.stderr);
    assert!(refused.stdout.is_empty(), "a refusal writes no report");

    // And the caller can say they know better.
    assert_eq!(run(&["--system", "i18next", &path]).code, 0);
}

/// **The layout that matters most in practice**, and the one nothing
/// before this could audit: VS Code writes `package.nls.json` in the
/// extension root and its translations elsewhere.
#[test]
fn the_split_manifest_layout_is_one_set() {
    let tree = Tree::new("split");
    tree.write("package.json", r#"{"name":"x","l10n":"./l10n"}"#);
    tree.write("src/extension.ts", "vscode.l10n.t('Save')\n");
    tree.write(
        "package.nls.json",
        r#"{"command.title":"Check catalogues","command.hint":"Runs the audit"}"#,
    );
    tree.write(
        "src/i18n/package.nls.de.json",
        r#"{"command.title":"Kataloge"}"#,
    );
    let run = run(&[&tree.path().join("src/i18n").to_string_lossy()]);
    assert_eq!(run.code, 1, "{}", run.stderr);
    let report = report(&run);
    assert_eq!(report["system"]["library"], "vscode-l10n");
    assert_eq!(report["system"]["layout"]["shape"], "fixed");
    assert_eq!(report["source"]["path"], "package.nls.json");
    assert_eq!(report["findings"][0]["kind"], "missing-key");
    assert_eq!(report["findings"][0]["key"], "command.hint");
}

// ---------------------------------------------------------------------
// The verdict
// ---------------------------------------------------------------------

#[test]
fn agreeing_catalogues_exit_zero() {
    let tree = Tree::new("clean");
    tree.i18next();
    tree.write("locales/en.json", r#"{"a":"Save {{name}}"}"#);
    tree.write("locales/es.json", r#"{"a":"Guardar {{name}}"}"#);
    let run = run(&[&locales(&tree)]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(report(&run)["status"], "clean");
    assert!(run.stderr.contains("clean"), "{}", run.stderr);
}

#[test]
fn a_missing_key_exits_one() {
    let tree = drifted("missing");
    let run = run(&[&locales(&tree)]);
    assert_eq!(run.code, 1, "{}", run.stderr);
    assert_eq!(report(&run)["status"], "findings");
    assert!(run.stderr.contains("cancel"), "{}", run.stderr);
}

/// A directory with no catalogues is not a failure.
#[test]
fn no_catalogues_exits_zero() {
    let tree = Tree::new("none");
    tree.i18next();
    tree.write("locales/README.md", "nothing here\n");
    let run = run(&["--system", "i18next", &locales(&tree)]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(report(&run)["status"], "no-files");
    assert!(run.stderr.contains("no catalogues"), "{}", run.stderr);
}

/// **The default that keeps the tool switched on.** A string added to
/// English this morning is legitimately untranslated everywhere else.
#[test]
fn an_untranslated_string_fails_only_when_asked() {
    let tree = Tree::new("untranslated");
    tree.i18next();
    tree.write("locales/en.json", r#"{"a":"Dashboard {{name}}"}"#);
    tree.write("locales/es.json", r#"{"a":"Dashboard {{name}}"}"#);
    let path = locales(&tree);

    let default = run(&[&path]);
    assert_eq!(default.code, 0, "{}", default.stderr);
    assert_eq!(report(&default)["findings"][0]["kind"], "untranslated");
    assert_eq!(run(&["--fail-on", "untranslated", &path]).code, 1);
    assert_eq!(run(&["--fail-on", "any", &path]).code, 1);
}

/// **The finding the identification makes possible.** An ICU plural in
/// an i18next project renders its braces to a user verbatim — a defect,
/// where the guessing core could only shrug at it.
#[test]
fn a_construct_from_the_wrong_convention_is_a_finding() {
    let tree = Tree::new("convention");
    tree.i18next();
    tree.write(
        "locales/en.json",
        r#"{"items":"{count, plural, one {# item} other {# items}}","w":"in {{timeframe}}"}"#,
    );
    tree.write(
        "locales/es.json",
        r#"{"items":"{count} elementos","w":"en {{periodo}}"}"#,
    );
    let run = run(&[&locales(&tree)]);
    assert_eq!(run.code, 1, "{}", run.stderr);
    let report = report(&run);
    let kinds: Vec<&str> = report["findings"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|finding| finding["kind"].as_str())
        .collect();
    assert!(kinds.contains(&"convention-mismatch"), "{kinds:?}");
    assert!(kinds.contains(&"placeholder-name-mismatch"), "{kinds:?}");
    assert_eq!(report["findings"][0]["construct"], "icu-argument");
    assert!(run.stderr.contains("wrong convention"), "{}", run.stderr);
}

/// The same bytes under the library that writes them are ordinary.
#[test]
fn the_same_catalogues_read_differently_under_a_different_library() {
    let tree = Tree::new("reread");
    tree.write(
        "locales/en.json",
        r#"{"items":"{count, plural, one {# item} other {# items}}"}"#,
    );
    tree.write(
        "locales/es.json",
        r#"{"items":"{count, plural, other {#}}"}"#,
    );
    let path = locales(&tree);

    let as_next_intl = run(&["--system", "next-intl", &path]);
    assert_eq!(as_next_intl.code, 0, "{}", as_next_intl.stderr);

    let as_i18next = run(&["--system", "i18next", &path]);
    assert_eq!(as_i18next.code, 1);
    assert_eq!(
        report(&as_i18next)["findings"][0]["kind"],
        "convention-mismatch"
    );
}

/// Polish needs `_few` and Japanese needs only `_other`. Requiring
/// English's exact variant set would report a finding on every correctly
/// translated plural in the project.
#[test]
fn plural_key_suffixes_are_one_key_under_i18next() {
    let tree = Tree::new("plurals");
    tree.i18next();
    tree.write(
        "locales/en.json",
        r#"{"item_one":"{{count}} item","item_other":"{{count}} items"}"#,
    );
    tree.write(
        "locales/pl.json",
        r#"{"item_one":"{{count}} element","item_few":"{{count}} elementy","item_many":"{{count}} elementów","item_other":"{{count}} elementu"}"#,
    );
    tree.write("locales/ja.json", r#"{"item_other":"{{count}} 個"}"#);
    let run = run(&[&locales(&tree)]);
    assert_eq!(run.code, 0, "{}", run.stderr);
}

/// **The property this tool exists under.** A catalogue is a product's
/// whole user-facing voice and no translated value may reach the output.
#[test]
fn no_translated_value_ever_reaches_stdout_or_stderr() {
    let tree = drifted("values");
    let run = run(&[&locales(&tree)]);
    let everything = format!("{}{}", run.stdout, run.stderr);
    for translated in ["en {{periodo}}", "Guardar", "Panel de control"] {
        assert!(!everything.contains(translated), "{everything}");
    }
    assert!(everything.contains("periodo"), "the token is the finding");
    assert!(everything.contains("cancel"), "the key is the finding");
}

/// The call-site scan reads source, and nothing it reads may reach the
/// output. Identification only.
#[test]
fn nothing_from_a_source_file_reaches_the_report() {
    let tree = Tree::new("call-sites");
    tree.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#);
    tree.write(
        "src/secret.ts",
        "// SUPER_SECRET_TOKEN=hunter2\nconst { t } = useTranslation()\n",
    );
    tree.write("locales/en.json", r#"{"a":"Save {{name}}"}"#);
    tree.write("locales/es.json", r#"{"a":"Guardar {{name}}"}"#);
    let run = run(&[&locales(&tree)]);
    let everything = format!("{}{}", run.stdout, run.stderr);
    assert!(!everything.contains("hunter2"), "{everything}");
    assert!(!everything.contains("SUPER_SECRET"), "{everything}");
    assert!(
        everything.contains("useTranslation("),
        "the call is the evidence"
    );
}

#[test]
fn the_source_can_be_named_by_path_or_by_tag() {
    let tree = drifted("source");
    let path = locales(&tree);
    for named in ["es.json", "es"] {
        let report = report(&run(&["--source", named, &path]));
        assert_eq!(report["source"]["path"], "es.json", "{named}");
    }
}

/// **Auto-detection answers or refuses, it never guesses.**
#[test]
fn two_english_candidates_exit_two_and_name_the_fix() {
    let tree = Tree::new("ambiguous");
    tree.i18next();
    tree.write("locales/en.json", r#"{"a":"Save {{name}}"}"#);
    tree.write("locales/en-GB.json", r#"{"a":"Save {{name}}"}"#);
    tree.write("locales/es.json", r#"{"a":"Guardar {{name}}"}"#);
    let run = run(&[&locales(&tree)]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("source"), "{}", run.stderr);
    assert!(run.stderr.contains("en-GB.json"), "{}", run.stderr);
}

/// Pointing this at a repository root is the wrong question, and being
/// told so beats a hundred invented findings.
#[test]
fn a_directory_that_is_not_a_catalogue_set_exits_two() {
    let tree = Tree::new("not-a-set");
    tree.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#);
    tree.write("tsconfig.json", r#"{"compilerOptions":{}}"#);
    tree.write("src/a.ts", "useTranslation()\n");
    let run = run(&["--system", "i18next", &tree.path().to_string_lossy()]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("language tag"), "{}", run.stderr);
}

/// A catalogue that will not parse is named, not dropped. A file that
/// silently vanished from the report reads to whoever ran it as a file
/// that was clean.
#[test]
fn a_catalogue_that_will_not_parse_is_named_and_the_rest_still_run() {
    let tree = Tree::new("broken");
    tree.i18next();
    tree.write("locales/en.json", r#"{"a":"Save {{name}}","b":"Cancel"}"#);
    tree.write("locales/es.json", r#"{"a":"Guardar {{name}}"}"#);
    tree.write("locales/fr.json", "{ not json");
    let path = locales(&tree);

    let relaxed = run(&[&path]);
    assert_eq!(relaxed.code, 1, "{}", relaxed.stderr);
    assert_eq!(report(&relaxed)["diagnostics"][0]["file"], "fr.json");
    assert_eq!(report(&relaxed)["summary"]["files"], 2);
    assert_eq!(run(&["--strict", &path]).code, 2);
}

#[test]
fn an_unreadable_input_exits_two() {
    assert_eq!(run(&["/no/such/place-xyz"]).code, 2);
}

#[test]
fn an_unknown_flag_exits_two_and_names_itself() {
    let tree = drifted("badflag");
    let run = run(&["--surce", "en", &locales(&tree)]);
    assert_eq!(run.code, 2);
    assert!(run.stderr.contains("--surce"), "{}", run.stderr);
    assert!(run.stdout.is_empty(), "a refusal writes no report");
}

/// The tool reads structure, never a translation, and no flag may
/// suggest otherwise.
#[test]
fn no_flag_offers_to_print_a_value() {
    let tree = drifted("novalues");
    for attempt in ["--show-values", "--values", "--print-strings", "--fix"] {
        assert_eq!(
            run(&[attempt, &locales(&tree)]).code,
            2,
            "{attempt} was accepted"
        );
    }
}

#[test]
fn version_and_help_exit_clear() {
    let version = run(&["--version"]);
    assert_eq!(version.code, 0);
    assert!(version.stdout.contains("i18n-le"));
    let help = run(&["--help"]);
    assert_eq!(help.code, 0);
    assert!(help.stdout.contains("usage: i18n-le"));
    assert!(
        help.stdout.contains("Two agreeing is an identification"),
        "the rule is unsaid"
    );
}

#[test]
fn stdout_carries_one_report_and_stderr_never_json() {
    let tree = drifted("streams");
    let run = run(&[&locales(&tree)]);
    assert_eq!(
        run.stdout.trim().lines().count(),
        1,
        "one report, not one per file"
    );
    assert!(!run.stderr.contains('{'), "{}", run.stderr);
    assert!(run.stderr.contains("findings across"), "{}", run.stderr);
}

/// The report is a thing to diff, so two runs over an unchanged
/// directory must be byte-identical — and nothing in it may name the
/// machine it ran on.
#[test]
fn two_runs_are_byte_identical_and_carry_no_absolute_path() {
    let tree = drifted("stable");
    let path = locales(&tree);
    let first = run(&[&path]);
    assert_eq!(first.stdout, run(&[&path]).stdout);
    assert!(
        !first
            .stdout
            .contains(&tree.path().to_string_lossy().to_string()),
        "{}",
        first.stdout
    );
}

/// **The cross-surface contract.** Both surfaces call one entry point,
/// so they must answer identically for the same catalogues — with the
/// difference that the MCP side is *told* the library rather than
/// identifying it, which is the only honest split.
#[test]
fn the_cli_and_the_mcp_server_report_the_same_findings() {
    let tree = drifted("agreement");
    let from_cli = report(&run(&[&locales(&tree)]));

    let read = |name: &str| {
        std::fs::read_to_string(tree.path().join("locales").join(name)).expect("a catalogue")
    };
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "check_catalogues",
            "arguments": {
                "library": "i18next",
                "files": [
                    { "path": "en.json", "content": read("en.json") },
                    { "path": "es.json", "content": read("es.json") },
                ],
            },
        },
    });
    let mut child = Command::new(BINARY)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the server starts");
    writeln!(child.stdin.as_mut().expect("stdin"), "{request}").expect("written");
    let output = child.wait_with_output().expect("finishes");
    let response: serde_json::Value = serde_json::from_slice(
        output
            .stdout
            .split(|byte| *byte == b'\n')
            .next()
            .expect("a line"),
    )
    .expect("the reply is JSON");
    let from_mcp = &response["result"]["structuredContent"]["data"];

    assert_eq!(
        from_mcp["findings"], from_cli["findings"],
        "the two disagree"
    );
    assert_eq!(from_mcp["summary"], from_cli["summary"]);
    assert_eq!(
        from_mcp["system"]["layout"],
        serde_json::Value::Null,
        "the agent surface reaches no filesystem, so it has no layout"
    );
}
