//! Behaviour that differs by operating system, asserted rather than
//! hoped.
//!
//! Everything here runs the **built binary** on all three platforms. A
//! case that cannot be constructed on one of them says so by name and
//! keeps asserting whatever the platform did instead. **A skip is never
//! reported as a pass.**
//!
//! The case this file exists for: a sibling in this family shipped a
//! release whose report used `\` on Windows and `/` everywhere else, red
//! on Windows CI for the whole release before anyone looked. stdout here
//! is protocol, and it is a report meant to be diffed — against the last
//! run, against a baseline, in a pull request. A consumer comparing one
//! machine's answer against another's must not have to know which
//! operating system produced either.
//!
//! This crate has two places a separator can reach a report: the
//! `<locale>/<namespace>.json` name a namespaced set is reported under,
//! and the relative path in a piece of identification evidence. Both are
//! built by joining with `/` on purpose, and both are pinned below.

use std::io::Write as _;
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
            "i18n-le-platform-{name}-{}-{unique}",
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

    fn i18next(&self) -> &Self {
        self.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#);
        self.write("src/app.ts", "const { t } = useTranslation()\n");
        self
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let target = self.root.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("a parent directory");
        }
        std::fs::write(&target, contents).expect("a file");
        target
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

fn run_with(args: &[&str], environment: &[(&str, Option<&str>)]) -> Run {
    let mut command = Command::new(BINARY);
    command.args(args).stdin(Stdio::null());
    for (name, value) in environment {
        match value {
            Some(value) => command.env(name, value),
            None => command.env_remove(name),
        };
    }
    let output = command.output().expect("the binary runs");
    Run {
        code: output.status.code().expect("an exit code, not a signal"),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn run(args: &[&str]) -> Run {
    run_with(args, &[])
}

fn report(run: &Run) -> serde_json::Value {
    serde_json::from_str(run.stdout.trim())
        .unwrap_or_else(|error| panic!("stdout is not one JSON report ({error}): {}", run.stdout))
}

fn paths(report: &serde_json::Value) -> Vec<String> {
    report["files"]
        .as_array()
        .map(|files| {
            files
                .iter()
                .filter_map(|file| file["path"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

fn skipped(case: &str, why: &str) {
    eprintln!("SKIPPED {case}: {why}");
}

/// **The bug a sibling shipped.** Every path in the report is built by
/// joining with `/`, never by handing a `Path` to `to_string_lossy`, so
/// a Windows report and a Linux one are the same bytes.
#[test]
fn every_reported_path_uses_one_separator_on_every_platform() {
    let tree = Tree::new("separators");
    tree.write(
        "apps/web/package.json",
        r#"{"dependencies":{"i18next":"^26.0.0"}}"#,
    );
    tree.write("apps/web/src/app.ts", "useTranslation()\n");
    tree.write(
        "apps/web/locales/en/common.json",
        r#"{"a":"Hi {{name}}","b":"Bye"}"#,
    );
    tree.write(
        "apps/web/locales/de/common.json",
        r#"{"a":"Hallo {{name}}"}"#,
    );

    let run = run(&[&tree.path().join("apps/web/locales").to_string_lossy()]);
    assert_eq!(run.code, 1, "{}", run.stderr);
    let report = report(&run);

    let paths = paths(&report);
    assert_eq!(
        paths,
        ["de/common.json", "en/common.json"],
        "a namespaced set is reported under <locale>/<file>"
    );

    // Nothing anywhere in the protocol stream may carry the other
    // separator: not a file path, not a finding, not a piece of
    // evidence, not a diagnostic.
    assert!(
        !run.stdout.contains('\\'),
        "a backslash reached the report: {}",
        run.stdout
    );
    let details: Vec<&str> = report["system"]["evidence"]
        .as_array()
        .expect("evidence")
        .iter()
        .filter_map(|signal| signal["detail"].as_str())
        .collect();
    assert!(
        details.iter().any(|detail| detail.contains("../")),
        "no relative evidence path to check: {details:?}"
    );
}

/// The tool holds no clock and reads no date, and the report says so by
/// carrying no timestamp. Windows ignores `TZ` entirely, so a suite that
/// quietly depended on it would be red there and nowhere else.
#[test]
fn the_answer_does_not_depend_on_the_time_zone() {
    let tree = Tree::new("timezone");
    tree.i18next();
    tree.write("locales/en.json", r#"{"a":"Hi {{name}}","b":"Bye"}"#);
    tree.write("locales/es.json", r#"{"a":"Hola {{nombre}}"}"#);
    let path = tree.path().join("locales").to_string_lossy().into_owned();

    let utc = run_with(&[&path], &[("TZ", Some("UTC"))]);
    let kiritimati = run_with(&[&path], &[("TZ", Some("Pacific/Kiritimati"))]);
    let unset = run_with(&[&path], &[("TZ", None)]);

    assert_eq!(utc.stdout, kiritimati.stdout, "the report moved with TZ");
    assert_eq!(utc.stdout, unset.stdout);
    assert_eq!(utc.stderr, kiritimati.stderr, "the summary moved with TZ");
    assert_eq!(utc.code, unset.code);
}

/// A case-folding filesystem — macOS and Windows by default — hands back
/// whichever spelling was written first. Each catalogue must be reported
/// once, and its locale canonicalised whatever the case on disk.
#[test]
fn a_case_folding_filesystem_reports_each_catalogue_once() {
    let tree = Tree::new("case");
    tree.i18next();
    tree.write("locales/en.json", r#"{"a":"Hi {{name}}"}"#);
    tree.write("locales/PT-br.json", r#"{"a":"Ola {{name}}"}"#);

    let directory = tree.path().join("locales");
    let folding = std::fs::write(directory.join("EN.json"), r#"{"a":"Hi {{name}}"}"#).is_ok()
        && std::fs::read_dir(&directory)
            .expect("the directory")
            .count()
            == 2;
    if folding {
        skipped(
            "case-folding",
            "this filesystem folded EN.json onto en.json, which is the case under test",
        );
    }

    let run = run(&[&directory.to_string_lossy()]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let report = report(&run);
    let paths = paths(&report);
    assert_eq!(
        paths.len(),
        std::fs::read_dir(&directory)
            .expect("the directory")
            .count(),
        "a catalogue was reported twice or lost: {paths:?}"
    );
    let locales: Vec<&str> = report["files"]
        .as_array()
        .expect("files")
        .iter()
        .filter_map(|file| file["locale"].as_str())
        .collect();
    assert!(
        locales.contains(&"pt-BR"),
        "a tag was not canonicalised: {locales:?}"
    );
}

/// `CON`, `AUX`, `NUL` and `PRN` are device names on Windows and cannot
/// be created there. On a platform that allows them they are read as
/// ordinary three-letter tags, which is the same rule that makes `nls`
/// readable — the set decides, not the name.
#[test]
fn reserved_windows_names_do_not_break_the_run() {
    let tree = Tree::new("reserved");
    tree.i18next();
    tree.write("locales/en.json", r#"{"a":"Hi {{name}}"}"#);

    let mut created = Vec::new();
    for name in ["con.json", "aux.json", "nul.json", "prn.json"] {
        if std::fs::write(
            tree.path().join("locales").join(name),
            r#"{"a":"Hola {{name}}"}"#,
        )
        .is_ok()
        {
            created.push(name);
        }
    }
    if created.is_empty() {
        skipped(
            "reserved-names",
            "this platform reserves CON/AUX/NUL/PRN as device names",
        );
    }

    let run = run(&[&tree.path().join("locales").to_string_lossy()]);
    assert!(
        matches!(run.code, 0 | 1),
        "a reserved name ended the run: {}",
        run.stderr
    );
    let paths = paths(&report(&run));
    for name in created {
        assert!(paths.contains(&name.to_string()), "{name}: {paths:?}");
    }
}

/// A catalogue written on Windows and one written on Linux are the same
/// catalogue. The line endings are outside every string, so they must
/// not reach a key, a value, or a placeholder offset.
#[test]
fn a_crlf_catalogue_reads_exactly_like_an_lf_one() {
    let body = "{\n  \"greeting\": \"Hi {{name}}\",\n  \"note\": \"100% done\"\n}\n";

    let lf = Tree::new("lf");
    lf.i18next();
    lf.write("locales/en.json", body);
    lf.write("locales/es.json", body);

    let crlf = Tree::new("crlf");
    crlf.i18next();
    crlf.write("locales/en.json", &body.replace('\n', "\r\n"));
    crlf.write("locales/es.json", &body.replace('\n', "\r\n"));

    let from_lf = run(&[&lf.path().join("locales").to_string_lossy()]);
    let from_crlf = run(&[&crlf.path().join("locales").to_string_lossy()]);
    assert_eq!(from_lf.code, from_crlf.code, "{}", from_crlf.stderr);
    assert_eq!(
        from_lf.stdout, from_crlf.stdout,
        "a carriage return changed the answer"
    );
}

/// The MCP server reads a line at a time from stdin. A caller that
/// closes it immediately — every `spawn` that fails, and the shell's
/// `</dev/null` — must end the process cleanly rather than block or
/// fault.
#[test]
fn the_mcp_server_exits_cleanly_when_stdin_closes_immediately() {
    let run = run(&["mcp"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(run.stdout.is_empty(), "{}", run.stdout);
}

/// A frame that is not JSON has no id to answer against, so it is
/// dropped rather than replied to — and the frames after it are still
/// served. Written here because it is the line-reading half of the
/// stdio surface, which is where a platform difference would show.
#[test]
fn the_mcp_server_survives_a_frame_it_cannot_read() {
    let mut child = Command::new(BINARY)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the server starts");
    {
        let stdin = child.stdin.as_mut().expect("stdin");
        writeln!(stdin, "not json at all").expect("written");
        writeln!(stdin).expect("written");
        writeln!(
            stdin,
            r#"{{"jsonrpc":"2.0","id":7,"method":"ping","params":{{}}}}"#
        )
        .expect("written");
    }
    let output = child.wait_with_output().expect("finishes");
    assert_eq!(output.status.code(), Some(0));
    let replies: Vec<serde_json::Value> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("a JSON reply"))
        .collect();
    assert_eq!(replies.len(), 1, "{replies:?}");
    assert_eq!(replies[0]["id"], 7);
}
