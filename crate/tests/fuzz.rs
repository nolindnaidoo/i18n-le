//! A standing net over the placeholder lexer and the identification
//! precedence behind it.
//!
//! Time-boxed, not run to convergence: the point is that a panic, a
//! hang, or a slice off a character boundary has somewhere to be caught,
//! not that the input space is proved. Sixty seconds in CI, one second
//! locally, so the net is present on every push without owning the run.
//!
//! **It reaches no network.** The generated catalogues go to
//! `check_catalogues`, the MCP tool that takes file contents and touches
//! no filesystem, so the lexer half of this file is `message.rs` and
//! `catalogue.rs` and nothing else. The identification half needs a
//! project around it and writes a temporary tree, which is why it runs
//! far fewer cases.
//!
//! ## What it aims at
//!
//! `message.rs` walks bytes and slices ranges out of them, and every one
//! of those decisions is made on text somebody else wrote:
//!
//! - **`{{` is a variable under one grammar and ICU's literal `{` under
//!   the rest**, so the same bytes take two paths;
//! - **`{name, plural, …}` is stepped over whole** by a brace counter
//!   that has to terminate on input where the braces do not balance;
//! - **`%` is a printf conversion or a percentage in prose**, decided by
//!   the bytes after it;
//! - **`$t(` is a key reference or a foreign construct**, decided by the
//!   library.
//!
//! The generator writes what nobody writes by hand — eighty open braces,
//! `{{{{{{`, a plural inside a plural inside a plural, an interpolation
//! split across a multi-byte character — because a slice landing off a
//! character boundary is a panic and a counter that does not terminate
//! is a CI job that never ends.
//!
//! ## The one answer that is never allowed
//!
//! Every generated catalogue carries a distinctive sentence as a
//! **value**, and it may never appear in any answer, on either stream.
//! That is the whole privacy boundary: a tool that leaked a translation
//! into a report leaks a product's entire user-facing voice, and it has
//! to hold under generated input and not only under the cases somebody
//! thought of.

use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

const BINARY: &str = env!("CARGO_BIN_EXE_i18n-le");

/// The planted sentence. Nonsense on purpose: distinctive enough that
/// finding it in an answer is proof rather than coincidence, and not a
/// real product's string in a public repository.
const SECRET: &str = "zzq-frase-secreta-zzq";

/// One case may not take longer than this. A brace counter that goes
/// quadratic on an eighty-brace run would otherwise be a CI job that
/// never ends.
const CASE_LIMIT: Duration = Duration::from_secs(20);

const LIBRARIES: [&str; 4] = ["i18next", "next-intl", "vscode-l10n", "flutter-arb"];

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Seconds of fuzzing. CI passes 60; a bare `cargo test` runs one.
fn seconds() -> Duration {
    Duration::from_secs(
        std::env::var("I18N_LE_FUZZ_SECONDS")
            .ok()
            .and_then(|raw| raw.parse().ok())
            .unwrap_or(1),
    )
}

/// Printed on every run, failing or not: a fuzz failure nobody can
/// reproduce is a fuzz failure nobody fixes.
fn seed() -> u64 {
    std::env::var("I18N_LE_FUZZ_SEED")
        .ok()
        .and_then(|raw| raw.parse().ok())
        .unwrap_or(0x118e_1e20_2608_12ff)
}

/// xorshift64*, four lines and identical on every platform. A fuzz run
/// needs to be reproducible, not statistically excellent.
struct Seeded(u64);

impl Seeded {
    fn next(&mut self) -> u64 {
        let mut state = self.0;
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        self.0 = state;
        state.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, limit: usize) -> usize {
        let limit = u64::try_from(limit).unwrap_or(1).max(1);
        usize::try_from(self.next() % limit).unwrap_or(0)
    }

    fn pick<'a, T>(&mut self, from: &'a [T]) -> &'a T {
        &from[self.below(from.len())]
    }
}

// ------------------------------------------------------------- generation

/// The fragments the lexer has to survive, one shape per decision it
/// makes. Multi-byte characters are threaded through so a byte offset
/// that lands mid-character is a panic rather than a wrong number.
fn fragment(seeded: &mut Seeded) -> String {
    let names = ["name", "count", "user.first-name", "n0", "_x", "a-b"];
    let name = *seeded.pick(&names);
    match seeded.below(16) {
        0 => format!("{{{{{name}}}}}"),
        1 => format!("{{{{ {name} }}}}"),
        2 => format!("{{{name}}}"),
        3 => format!("{{ {name} }}"),
        4 => format!("{{{}}}", seeded.below(9)),
        5 => format!("{{{name}, plural, one {{# cosa}} other {{# cosas}}}}"),
        // Unbalanced on purpose: the counter must stop at the end of the
        // text rather than run past it.
        6 => format!("{{{name}, plural, one {{# cosa"),
        7 => "{".repeat(1 + seeded.below(80)),
        8 => "}".repeat(1 + seeded.below(80)),
        9 => "{{".repeat(1 + seeded.below(40)),
        10 => format!("$t({name})"),
        11 => format!("$t(a.b.{{{{{name}}}}})"),
        12 => format!("${{{name}}}"),
        13 => format!("{{ ${name} }}"),
        // Percentages: prose in several languages, printf in none of
        // them. "90%-os" is Hungarian.
        14 => (*seeded.pick(&["100%%", "90%-os", "%90", "%s", "%1$s", "%d", "up to 20%!"]))
            .to_string(),
        _ => {
            (*seeded.pick(&["日本語", "Привет", "🎯", "café", "\u{200b}", "a\u{0301}"])).to_string()
        }
    }
}

fn message(seeded: &mut Seeded) -> String {
    let mut text = String::new();
    for _ in 0..=seeded.below(6) {
        text.push_str(&fragment(seeded));
        text.push(' ');
    }
    text
}

/// One generated catalogue, always carrying the sentence that may never
/// come back out.
fn catalogue(seeded: &mut Seeded, keys: usize) -> String {
    let mut document = String::from("{");
    let _ = write!(document, "\"secreto\":\"{SECRET}\"");
    for index in 0..keys {
        let key = match seeded.below(6) {
            0 => format!("item_{}", seeded.pick(&["one", "other", "few", "many"])),
            1 => format!("@{index}"),
            2 => "@@locale".to_string(),
            3 => format!("a.b.c{index}"),
            4 => format!("Save the file {index}"),
            _ => format!("key{index}"),
        };
        let value = message(seeded).replace('"', "'").replace('\\', "/");
        let _ = write!(document, ",{}:{}", json_string(&key), json_string(&value));
    }
    document.push('}');
    document
}

fn json_string(text: &str) -> String {
    serde_json::to_string(text).expect("a string serializes")
}

// ------------------------------------------------------------ the server

/// A server held open across the whole run: spawning a process per case
/// would measure `fork`, not the lexer.
struct Server {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
}

impl Server {
    fn start() -> Self {
        let mut child = Command::new(BINARY)
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the server starts");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let (sender, lines) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if sender.send(line).is_err() {
                    return;
                }
            }
        });
        Self {
            child,
            stdin,
            lines,
        }
    }

    /// One request, one reply, or a named failure. A reply that never
    /// comes is the hang this file exists to catch.
    fn call(&mut self, case: &str, request: &serde_json::Value) -> serde_json::Value {
        writeln!(self.stdin, "{request}").unwrap_or_else(|error| {
            panic!("{case}: the server stopped reading ({error})");
        });
        self.stdin
            .flush()
            .unwrap_or_else(|error| panic!("{case}: {error}"));
        match self.lines.recv_timeout(CASE_LIMIT) {
            Ok(line) => serde_json::from_str(&line)
                .unwrap_or_else(|error| panic!("{case}: the reply is not JSON ({error}): {line}")),
            Err(RecvTimeoutError::Timeout) => {
                let _ = self.child.kill();
                panic!("{case}: no reply within {CASE_LIMIT:?}");
            }
            Err(RecvTimeoutError::Disconnected) => {
                panic!("{case}: the server died — a panic, not an answer");
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn check(seeded: &mut Seeded, server: &mut Server, case: &str) {
    let library = *seeded.pick(&LIBRARIES);
    let keys = 1 + seeded.below(12);
    let english = catalogue(seeded, keys);
    let keys = 1 + seeded.below(12);
    let spanish = catalogue(seeded, keys);
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "check_catalogues",
            "arguments": {
                "library": library,
                "files": [
                    { "path": "en.json", "content": english },
                    { "path": "es.json", "content": spanish },
                ],
            },
        },
    });
    let reply = server.call(case, &request);

    // A protocol error means the server decided the *question* was
    // malformed, and none of these are: the arguments are always well
    // formed and only the catalogue text is hostile.
    assert!(
        reply.get("error").is_none(),
        "{case}: {library} — a generated catalogue became a protocol error: {reply}"
    );
    let rendered = serde_json::to_string(&reply).expect("the reply serializes");
    assert!(
        !rendered.contains(SECRET),
        "{case}: {library} — a translated value reached the answer: {rendered}"
    );
}

#[test]
fn the_lexer_survives_what_grows_out_of_its_own_rules() {
    let seed = seed();
    let budget = seconds();
    eprintln!("fuzz: seed {seed:#x}, budget {budget:?}");

    let mut seeded = Seeded(seed);
    let mut server = Server::start();
    let deadline = Instant::now() + budget;
    let mut cases = 0u64;
    while Instant::now() < deadline {
        cases += 1;
        check(&mut seeded, &mut server, &format!("case {cases}"));
    }
    assert!(cases > 0, "the budget ran out before a single case");
    eprintln!("fuzz: {cases} generated catalogue pairs, seed {seed:#x}");
}

/// The shapes that would not terminate rather than the shapes that
/// would be wrong. Every one is a run of the delimiters the lexer counts,
/// long enough that a quadratic rule shows up as a timeout.
#[test]
fn pathological_runs_terminate() {
    let mut server = Server::start();
    let runs = [
        "{".repeat(20_000),
        "}".repeat(20_000),
        "{{".repeat(10_000),
        "}}".repeat(10_000),
        "$t(".repeat(10_000),
        "%".repeat(20_000),
        format!("{}{}", "{a, plural, one {".repeat(2_000), "}".repeat(2_000)),
        format!("{{{}}}", "a".repeat(20_000)),
        format!("{{{{{}", "名".repeat(10_000)),
    ];

    for (index, run) in runs.iter().enumerate() {
        for library in LIBRARIES {
            let case = format!("run {index} under {library}");
            let started = Instant::now();
            let request = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": "check_catalogues",
                    "arguments": {
                        "library": library,
                        "files": [
                            { "path": "en.json", "content": serde_json::json!({ "a": run, "s": SECRET }).to_string() },
                            { "path": "es.json", "content": serde_json::json!({ "a": run, "s": SECRET }).to_string() },
                        ],
                    },
                },
            });
            let reply = server.call(&case, &request);
            assert!(reply.get("error").is_none(), "{case}: {reply}");
            assert!(
                !serde_json::to_string(&reply)
                    .expect("serializes")
                    .contains(SECRET),
                "{case}: a value reached the answer"
            );
            eprintln!("fuzz: {case} in {:?}", started.elapsed());
        }
    }
}

// --------------------------------------------------- identification

/// Generated projects, aimed at the precedence rule rather than the
/// lexer: two agreeing classes identifies, two libraries identified is a
/// refusal, one class is a refusal. It writes a tree, so it runs a fixed
/// small number of cases rather than to a clock.
#[test]
fn identification_never_picks_between_two_answers() {
    let seed = seed();
    eprintln!("fuzz: identification seed {seed:#x}");
    let mut seeded = Seeded(seed ^ 0x5bd1_e995);

    for case in 0..48 {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("i18n-le-fuzz-{}-{unique}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("locales")).expect("a temporary directory");

        // A manifest naming zero, one or two libraries at once. Two is
        // the case that must refuse and name both.
        let declared: Vec<&str> = ["i18next", "next-intl", "@vscode/l10n"]
            .into_iter()
            .filter(|_| seeded.below(2) == 0)
            .collect();
        let dependencies: Vec<String> = declared
            .iter()
            .map(|name| format!("\"{name}\":\"^26.0.0\""))
            .collect();
        std::fs::write(
            root.join("package.json"),
            format!("{{\"dependencies\":{{{}}}}}", dependencies.join(",")),
        )
        .expect("a manifest");
        std::fs::write(
            root.join("app.ts"),
            format!(
                "{}\n{}\n",
                if seeded.below(2) == 0 {
                    "useTranslation()"
                } else {
                    ""
                },
                if seeded.below(2) == 0 {
                    "useTranslations()"
                } else {
                    ""
                }
            ),
        )
        .expect("a source file");
        for locale in ["en", "es"] {
            let keys = 1 + seeded.below(8);
            std::fs::write(
                root.join("locales").join(format!("{locale}.json")),
                catalogue(&mut seeded, keys),
            )
            .expect("a catalogue");
        }

        let output = Command::new(BINARY)
            .arg(root.join("locales"))
            .stdin(Stdio::null())
            .output()
            .expect("the binary runs");
        let code = output
            .status
            .code()
            .unwrap_or_else(|| panic!("case {case}: died on a signal"));
        assert!(
            (0..=2).contains(&code),
            "case {case}: exit {code} is outside 0/1/2"
        );
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        assert!(
            !stdout.contains(SECRET) && !stderr.contains(SECRET),
            "case {case}: a translated value reached a stream"
        );

        // Exit 2 is a refusal and writes no report; 0 and 1 are answers
        // and write exactly one.
        if code == 2 {
            assert!(stdout.is_empty(), "case {case}: a refusal wrote a report");
            assert!(!stderr.is_empty(), "case {case}: a silent refusal");
        } else {
            let report: serde_json::Value = serde_json::from_str(stdout.trim())
                .unwrap_or_else(|error| panic!("case {case}: {error}: {stdout}"));
            let library = report["system"]["library"]
                .as_str()
                .unwrap_or_else(|| panic!("case {case}: an answer with no library"));
            assert!(
                LIBRARIES.contains(&library),
                "case {case}: {library} is not a library this reads"
            );
        }

        let _ = std::fs::remove_dir_all(&root);
    }
}

/// The generator writes documents; this asserts it writes ones the tool
/// can actually read, so a green fuzz run is not a run of nothing.
#[test]
fn the_generator_produces_documents_the_reader_accepts() {
    let mut seeded = Seeded(seed());
    let mut parsed = 0;
    for _ in 0..200 {
        let keys = 1 + seeded.below(12);
        let document = catalogue(&mut seeded, keys);
        if serde_json::from_str::<serde_json::Value>(&document).is_ok() {
            parsed += 1;
        }
    }
    assert_eq!(parsed, 200, "the generator writes documents nothing reads");
}
