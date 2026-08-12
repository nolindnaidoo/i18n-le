//! Catalogues and filesystems that break tools, run against the **built
//! binary**.
//!
//! Not a fixture directory: Windows cannot check in a FIFO, a symlink
//! loop or a permission-denied file, so every tree here is built at
//! runtime and each case the platform cannot express says plainly, by
//! name, that it did not run. **A skip is never reported as a pass.**
//!
//! Every case asserts the same floor first: the process does not panic,
//! does not hang, and exits 0, 1 or 2 — never on a signal.
//!
//! The three outcomes a catalogue can have here are all real and all
//! different, and a case names which one it expects:
//!
//! - **audited** — a file summary in the report, with its key count;
//! - **skipped** — a `skipped` diagnostic, because the bytes were not
//!   UTF-8 or the file could not be opened;
//! - **unparsable** — an `unparsable` diagnostic, because the bytes were
//!   read and were not a JSON object.
//!
//! Never a fourth: a catalogue that vanishes from the report reads to
//! whoever ran the audit as a catalogue that was clean.

use std::fmt::Write as _;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const BINARY: &str = env!("CARGO_BIN_EXE_i18n-le");

/// Generous enough for a shared runner reading a fifty-megabyte
/// catalogue, tight enough that a blocking read on a FIFO is a failure
/// rather than a coffee break.
const LIMIT: Duration = Duration::from_secs(120);

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new(name: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "i18n-le-hazard-{name}-{}-{unique}",
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

    /// A project that identifies as i18next from its manifest and a call
    /// site, so every case below is about the *catalogues* rather than
    /// about whether identification could run at all.
    fn i18next(&self) -> &Self {
        self.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#);
        self.write("src/app.ts", "const { t } = useTranslation()\n");
        self
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        self.write_bytes(relative, contents.as_bytes())
    }

    fn write_bytes(&self, relative: &str, contents: &[u8]) -> PathBuf {
        let target = self.root.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("a parent directory");
        }
        std::fs::write(&target, contents).expect("a file");
        target
    }

    fn locales(&self) -> String {
        self.root.join("locales").to_string_lossy().into_owned()
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

/// Runs the binary and **fails rather than blocks**. A hang is one of
/// the two failure modes this file exists to catch — a FIFO with no
/// writer is one `read` away from an eternal CI job — so the child is
/// killed at the deadline and the case is named.
fn run(case: &str, args: &[&str]) -> Run {
    let mut child = Command::new(BINARY)
        .args(args)
        // Never inherit the terminal: a child waiting on a keyboard that
        // is not there is a hang with an innocent explanation.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");

    // Drained on threads: a child writing more than a pipe buffer would
    // deadlock against a parent that waits before reading.
    let mut out = child.stdout.take().expect("stdout");
    let mut err = child.stderr.take().expect("stderr");
    let out = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = out.read_to_end(&mut buffer);
        buffer
    });
    let err = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = err.read_to_end(&mut buffer);
        buffer
    });

    let deadline = Instant::now() + LIMIT;
    let status = loop {
        match child.try_wait().expect("the child is waitable") {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("{case}: hung for {LIMIT:?} on {args:?}");
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    };

    // `None` means the process died on a signal — the SIGSEGV/SIGABRT
    // class, which no input may produce.
    let code = status.code().unwrap_or_else(|| {
        panic!("{case}: died on a signal rather than exiting ({status:?}) on {args:?}")
    });
    assert!(
        (0..=2).contains(&code),
        "{case}: exit {code} is outside the documented 0/1/2 on {args:?}"
    );
    Run {
        code,
        stdout: String::from_utf8_lossy(&out.join().expect("stdout thread")).into_owned(),
        stderr: String::from_utf8_lossy(&err.join().expect("stderr thread")).into_owned(),
    }
}

/// The report. Doubles as the assertion that stdout is one JSON object
/// and nothing else — a stray human message there would fail to parse.
fn report(run: &Run) -> serde_json::Value {
    serde_json::from_str(run.stdout.trim())
        .unwrap_or_else(|error| panic!("stdout is not one JSON report ({error}): {}", run.stdout))
}

fn summary_for<'a>(report: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    report["files"]
        .as_array()?
        .iter()
        .find(|file| file["path"] == name)
}

fn diagnostic_for<'a>(report: &'a serde_json::Value, name: &str) -> Option<&'a serde_json::Value> {
    report["diagnostics"]
        .as_array()?
        .iter()
        .find(|diagnostic| diagnostic["file"] == name)
}

/// Every catalogue the run said anything at all about.
fn accounted(report: &serde_json::Value) -> Vec<String> {
    let named = |field: &str, key: &str| -> Vec<String> {
        report[field]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(|entry| entry[key].as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    [named("files", "path"), named("diagnostics", "file")].concat()
}

/// A case the platform cannot express. Named on stderr so a green run
/// still says what it did not check — a silent skip is a lie.
fn skipped(case: &str, why: &str) {
    eprintln!("SKIPPED {case}: {why}");
}

/// UTF-16LE bytes with a byte-order mark: what Notepad writes when asked
/// for "Unicode", and what a PowerShell redirect produces.
fn utf16le(text: &str) -> Vec<u8> {
    let mut bytes = vec![0xff, 0xfe];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

// ---------------------------------------------------------------- content

/// What a catalogue is allowed to come back as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// A file summary, carrying this many keys.
    Audited(u64),
    /// A `skipped` diagnostic: the bytes never became text.
    Skipped,
    /// An `unparsable` diagnostic: the bytes were read and were not a
    /// JSON object. Half a JSON document is not evidence.
    Unparsable,
}

/// Every content hazard, and the outcome each one must have.
///
/// They are here rather than in a unit test because the shapes are
/// byte-level and only exist on the way in from a filesystem: a
/// byte-order mark, a CRLF line ending, a UTF-16 encoding, bytes that
/// are not UTF-8 at all.
fn content_hazards() -> Vec<(&'static str, Vec<u8>, Outcome)> {
    let plain = r#"{"greeting":"Hola {{name}}","farewell":"Adios"}"#;

    // Fifty megabytes of real catalogue. Not a pathological shape — just
    // more of one than anybody expects a tool to hold, and the size at
    // which reading the whole file into memory stops being free.
    let mut huge = String::with_capacity(52_000_000);
    huge.push_str("{\"greeting\":\"Hola {{name}}\"");
    for index in 0..600_000 {
        let _ = write!(
            huge,
            ",\"key{index}\":\"valor {index} con relleno de texto\""
        );
    }
    huge.push('}');

    vec![
        ("es.json", plain.as_bytes().to_vec(), Outcome::Audited(2)),
        // Three invisible bytes Notepad, Excel and a PowerShell redirect
        // all add, and that no editor shows. Before the `{` they make
        // the parse fail, which reads as "this locale has no keys".
        (
            "fr.json",
            format!("\u{feff}{plain}").into_bytes(),
            Outcome::Audited(2),
        ),
        // A catalogue written on Windows.
        (
            "de.json",
            plain.replace(',', ",\r\n").into_bytes(),
            Outcome::Audited(2),
        ),
        ("it.json", b"{}".to_vec(), Outcome::Audited(0)),
        // Not UTF-8 by any reading: named rather than silently absent.
        (
            "pl.json",
            b"{\"greeting\":\"Hola \xff\xfe\"}".to_vec(),
            Outcome::Skipped,
        ),
        // A UTF-16 catalogue is mostly NUL bytes and never becomes text.
        ("ja.json", utf16le(plain), Outcome::Skipped),
        // Read, and not a JSON object.
        ("ko.json", b"{\"greeting\":".to_vec(), Outcome::Unparsable),
        ("nl.json", b"[1,2,3]".to_vec(), Outcome::Unparsable),
        ("pt.json", Vec::new(), Outcome::Unparsable),
        ("sv.json", huge.into_bytes(), Outcome::Audited(600_001)),
    ]
}

/// One tree, one run of the binary, and each catalogue held to the
/// outcome its case names.
#[test]
fn every_content_hazard_is_audited_skipped_or_named() {
    let tree = Tree::new("content");
    tree.i18next();
    tree.write("locales/en.json", r#"{"greeting":"Hi {{name}}"}"#);

    let cases = content_hazards();
    for (name, bytes, _) in &cases {
        tree.write_bytes(&format!("locales/{name}"), bytes);
    }

    let started = Instant::now();
    let run = run("content", &[&tree.locales()]);
    eprintln!("hazards: the content tree took {:?}", started.elapsed());
    assert_eq!(run.code, 1, "there are findings here\n{}", run.stderr);
    let report = report(&run);

    for (name, _, outcome) in &cases {
        let accounted = accounted(&report);
        assert!(
            accounted.iter().any(|seen| seen == name),
            "{name} vanished from the audit entirely: {accounted:?}"
        );
        match outcome {
            Outcome::Audited(keys) => {
                let summary = summary_for(&report, name)
                    .unwrap_or_else(|| panic!("{name} has no file summary"));
                assert_eq!(summary["keys"], *keys, "{name}: {summary}");
                assert!(
                    diagnostic_for(&report, name).is_none(),
                    "{name} was audited and diagnosed"
                );
            }
            Outcome::Skipped | Outcome::Unparsable => {
                let diagnostic = diagnostic_for(&report, name)
                    .unwrap_or_else(|| panic!("{name} has no diagnostic"));
                let code = if *outcome == Outcome::Skipped {
                    "skipped"
                } else {
                    "unparsable"
                };
                assert_eq!(diagnostic["code"], code, "{name}: {diagnostic}");
                assert!(
                    summary_for(&report, name).is_none(),
                    "{name} was diagnosed and audited"
                );
            }
        }
    }

    // The report is meant to be diffed between machines, and a read
    // error is the one message that used to carry the machine in it.
    let root = tree.path().to_string_lossy().into_owned();
    assert!(
        !run.stdout.contains(&root),
        "an absolute path reached the report"
    );
}

/// A catalogue nested past the JSON reader's own recursion limit is a
/// named diagnostic, not a stack overflow. Nothing writes one on
/// purpose, but a generator can, and a crash here takes the audit of
/// every other locale with it.
#[test]
fn a_pathologically_nested_catalogue_never_takes_the_run_down() {
    let tree = Tree::new("nested");
    tree.i18next();
    tree.write("locales/en.json", r#"{"greeting":"Hi {{name}}"}"#);

    let depth = 50_000;
    let document = format!("{}\"leaf\"{}", "{\"a\":".repeat(depth), "}".repeat(depth));
    tree.write("locales/es.json", &document);

    let run = run("nested", &[&tree.locales()]);
    assert_eq!(run.code, 0, "the one catalogue that parsed is clean");
    let report = report(&run);
    assert_eq!(
        diagnostic_for(&report, "es.json").expect("a diagnostic")["code"],
        "unparsable"
    );
}

// ------------------------------------------------------------- filesystem

/// Names a filesystem accepts and a reader can trip over. Every one of
/// them is a language tag under some spelling, so each has to be read as
/// a locale rather than refused.
#[test]
fn awkward_catalogue_names_are_read() {
    let tree = Tree::new("names");
    tree.i18next();
    tree.write("locales/en.json", r#"{"a":"Hi"}"#);

    let mut written = Vec::new();
    for name in ["pt-BR.json", "zh-Hant-HK.json", "es-419.json"] {
        if std::fs::write(tree.path().join("locales").join(name), r#"{"a":"Hola"}"#).is_err() {
            skipped("awkward-names", name);
            continue;
        }
        written.push(name);
    }
    assert!(!written.is_empty(), "this filesystem refused every name");

    let run = run("names", &[&tree.locales()]);
    let report = report(&run);
    for name in written {
        assert!(
            summary_for(&report, name).is_some(),
            "{name} was not read: {:?}",
            accounted(&report)
        );
    }
}

/// Where Windows differs: `MAX_PATH` is 260 characters unless long paths
/// are enabled, so the creation itself is the platform's answer. What is
/// asserted is that the run survives whichever happened.
#[test]
fn a_path_over_260_characters_is_read_or_skipped_by_name() {
    let tree = Tree::new("long-path");
    tree.i18next();

    // Wide names rather than deep nesting: the manifest that identifies
    // the project has to stay inside the eight ancestors the walk looks
    // through, so this case is about the length of a path and not about
    // how far up a `package.json` can sit.
    let mut deep = tree.path().to_path_buf();
    for part in ['a', 'b', 'c'] {
        deep.push(part.to_string().repeat(90));
    }
    let created = std::fs::create_dir_all(&deep)
        .and_then(|()| std::fs::write(deep.join("en.json"), r#"{"a":"Hi {{name}}"}"#))
        .and_then(|()| std::fs::write(deep.join("es.json"), r#"{"a":"Hola {{name}}"}"#))
        .is_ok();
    if !created {
        skipped(
            "long-path",
            "this filesystem refused a path over 260 characters",
        );
        return;
    }

    let run = run("long-path", &[&deep.to_string_lossy()]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert!(summary_for(&report(&run), "es.json").is_some());
}

/// **The hang this file exists for.** A FIFO with no writer blocks a
/// `read` forever, and the call-site scan reads every file whose
/// extension looks like source. It checked the size and never the kind,
/// so `read_to_string` on a pipe called `app.ts` was one `read` away
/// from a CI job that never ended.
#[cfg(unix)]
#[test]
fn a_fifo_in_the_source_tree_never_blocks_identification() {
    let tree = Tree::new("fifo");
    tree.i18next();
    tree.write("locales/en.json", r#"{"a":"Hi {{name}}"}"#);
    tree.write("locales/es.json", r#"{"a":"Hola {{name}}"}"#);

    // Shelled out rather than called through libc: `unsafe` is forbidden
    // crate-wide and a test is not an exemption.
    let mut made = 0;
    for pipe in ["src/blocking.ts", "package.json.d/pipe", "pubspec.yaml"] {
        let path = tree.path().join(pipe);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if Command::new("mkfifo")
            .arg(&path)
            .status()
            .is_ok_and(|status| status.success())
        {
            made += 1;
        }
    }
    if made == 0 {
        skipped("fifo", "mkfifo is not available on this runner");
        return;
    }

    let run = run("fifo", &[&tree.locales()]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let report = report(&run);
    assert_eq!(
        report["system"]["library"], "i18next",
        "the pipe cost the run its identification"
    );
    assert!(summary_for(&report, "es.json").is_some());
}

#[cfg(not(unix))]
#[test]
fn a_fifo_in_the_source_tree_never_blocks_identification() {
    skipped("fifo", "Windows has no FIFO in a directory tree");
}

/// Symlinks are never followed into the walk, so a loop is not a loop
/// here. Asserted rather than assumed: the call-site scan descends by
/// directory-entry type, and a link to the tree's own root is the shape
/// that turns a walk into an infinite descent when that is got wrong.
#[cfg(unix)]
#[test]
fn a_symlink_loop_never_hangs_the_call_site_scan() {
    let tree = Tree::new("loop");
    tree.i18next();
    tree.write("locales/en.json", r#"{"a":"Hi {{name}}"}"#);
    tree.write("locales/es.json", r#"{"a":"Hola {{name}}"}"#);

    let first = tree.path().join("src/loop-a");
    let second = tree.path().join("src/loop-b");
    if std::os::unix::fs::symlink(&second, &first).is_err()
        || std::os::unix::fs::symlink(&first, &second).is_err()
        || std::os::unix::fs::symlink(tree.path(), tree.path().join("src/self")).is_err()
    {
        skipped("symlink-loop", "this platform refused to create a symlink");
        return;
    }

    let run = run("loop", &[&tree.locales()]);
    assert_eq!(run.code, 0, "a symlink loop ended the run: {}", run.stderr);
    assert_eq!(report(&run)["system"]["library"], "i18next");
}

#[cfg(not(unix))]
#[test]
fn a_symlink_loop_never_hangs_the_call_site_scan() {
    skipped(
        "symlink-loop",
        "creating one needs Developer Mode or elevation on Windows",
    );
}

/// **A catalogue the filesystem refuses is named, never silently
/// dropped.** A locale that vanishes from the report reads to whoever
/// ran the audit as a locale that was clean, which is the one outcome
/// this tool may never produce. `--strict` is how a pipeline asks for
/// zero tolerance.
#[cfg(unix)]
#[test]
fn permission_denied_is_named_carried_and_never_ends_the_run() {
    use std::os::unix::fs::PermissionsExt as _;

    let tree = Tree::new("denied");
    tree.i18next();
    tree.write("locales/en.json", r#"{"a":"Hi {{name}}","b":"Bye"}"#);
    let denied = tree.write("locales/es.json", r#"{"a":"Hola {{name}}"}"#);
    tree.write("locales/fr.json", r#"{"a":"Salut {{name}}"}"#);
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o000)).expect("chmod");

    let readable_anyway = std::fs::read(&denied).is_ok();
    let lenient = run("denied", &[&tree.locales()]);
    let strict = run("denied-strict", &["--strict", &tree.locales()]);

    // Restored before asserting, or a failure leaves a file the cleanup
    // cannot remove.
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o644)).expect("chmod");

    if readable_anyway {
        skipped(
            "permission-denied",
            "this runner reads a mode-000 path anyway (root)",
        );
        return;
    }

    assert_eq!(
        lenient.code, 1,
        "an unreadable catalogue ended the run\n{}",
        lenient.stderr
    );
    let report = report(&lenient);
    assert!(
        summary_for(&report, "fr.json").is_some(),
        "the readable half of the set was lost"
    );
    let diagnostic = diagnostic_for(&report, "es.json").expect("a diagnostic for the denied file");
    assert_eq!(diagnostic["code"], "skipped");
    let root = tree.path().to_string_lossy().into_owned();
    assert!(
        !diagnostic["message"]
            .as_str()
            .is_some_and(|message| message.contains(&root)),
        "the read error carried the machine: {diagnostic}"
    );
    assert!(lenient.stderr.contains("es.json"), "{}", lenient.stderr);

    assert_eq!(
        strict.code, 2,
        "--strict ignored a catalogue that could not be read\n{}",
        strict.stderr
    );
}

#[cfg(not(unix))]
#[test]
fn permission_denied_is_named_carried_and_never_ends_the_run() {
    skipped(
        "permission-denied",
        "Windows ACLs are not chmod; the unix case covers the read failure",
    );
}

/// A directory holding one catalogue and a pile of things that are not
/// catalogues is the wrong question, and being told so beats a hundred
/// invented findings.
#[test]
fn a_directory_that_is_not_a_catalogue_set_is_refused_rather_than_audited() {
    let tree = Tree::new("not-a-set");
    tree.i18next();
    tree.write("locales/en.json", r#"{"a":"Hi {{name}}"}"#);
    tree.write("locales/tsconfig.json", "{}");
    tree.write("locales/package-lock.json", "{}");

    let run = run("not-a-set", &[&tree.locales()]);
    assert_eq!(run.code, 2, "{}", run.stderr);
    assert!(run.stderr.contains("language tag"), "{}", run.stderr);
    assert!(run.stdout.is_empty(), "a refusal wrote a report");
}
