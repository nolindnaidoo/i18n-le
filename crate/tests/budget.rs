//! A wall-clock ceiling on a generated corpus, and linearity in both
//! directions.
//!
//! A sibling in this family was fifty times slower than the rest for a
//! whole release and nothing noticed, because nothing measured it. The
//! ceilings here are deliberately loose — a shared runner is not a
//! benchmark rig — and exist to catch an order of magnitude, not a
//! percent.
//!
//! **The corpus is generated from a fixed seed**, not checked in: a
//! twenty-five locale set in git would be twenty-five files a reviewer
//! has to ignore, and the generator is twenty lines. The seed is
//! constant, so two runs measure the same corpus.
//!
//! Gated behind `I18N_LE_BUDGET` and run by CI on one platform with
//! `--test-threads=1`; a timing assertion measured against five other
//! tests on the same cores is noise. **A skipped run says so by name.**
//!
//! ## The quadratic this pins
//!
//! Reading a catalogue resolved duplicate keys by scanning every pair
//! for every pair — once to find a name already seen, once more to count
//! its occurrences — so a flat document cost time in the square of its
//! key count. Nothing in the suite noticed, because every fixture is
//! small and the one large scenario used dotted keys under a shape that
//! never grew.
//!
//! Measured on the machine this was written on, release build, Apple
//! M-series laptop, macOS 15, 2026-08:
//!
//! | keys in one flat catalogue | before | after |
//! |---|---|---|
//! | 12,000 | 0.73 s | 0.01 s |
//! | 24,000 | 2.93 s | 0.03 s |
//!
//! Both assertions below catch it, and that is on purpose. Four times
//! the keys cost sixteen times the clock before and four times after, so
//! the linearity bound sees it; and 2.93 s is far past the absolute
//! ceiling, so the ceiling sees it too. A ceiling alone can be met by
//! something quadratic and small; a ratio alone can be met by something
//! linear and uniformly slow.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const BINARY: &str = env!("CARGO_BIN_EXE_i18n-le");

const SEED: u64 = 0x118e_1e20_2608_12ff;

/// A real product's shape: twenty-five locales of two thousand keys.
/// The tags are real, because the locale reader refuses a name that is
/// not one and a corpus it will not read measures nothing.
const TAGS: [&str; 24] = [
    "es", "fr", "de", "it", "nl", "pl", "ja", "ko", "tr", "cs", "ro", "sv", "nb", "da", "fi", "el",
    "hu", "uk", "ru", "bg", "id", "vi", "pt-BR", "zh-CN",
];
const KEYS: usize = 2_000;

/// **10× the local measurement**, recorded with the machine it came
/// from: 0.38 s for the 25×2,000 corpus below, debug build, Apple
/// M-series laptop, macOS 15, 2026-08. Ten times that leaves a shared
/// runner room to be several times slower and still be right; it does
/// not leave room for an order of magnitude.
const SET_CEILING: Duration = Duration::from_millis(3_800);

/// 10× the local measurement for one flat catalogue of 24,000 keys
/// compared against another: 0.27 s debug. The same pair cost 2.93 s in
/// a *release* build before duplicate resolution stopped rescanning, so
/// the debug figure before it was far past this.
const FLAT_CEILING: Duration = Duration::from_millis(2_700);

/// Four times the input may not cost six times the clock. Loose enough
/// for a noisy runner, tight enough that the 16× a quadratic produces
/// cannot hide under it.
const LINEARITY: f64 = 6.0;

fn enabled(name: &str) -> bool {
    if std::env::var_os("I18N_LE_BUDGET").is_some() {
        return true;
    }
    eprintln!("SKIPPED {name}: set I18N_LE_BUDGET to run it");
    false
}

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
}

struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("i18n-le-budget-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a temporary directory");
        let root = std::fs::canonicalize(&root).expect("a canonical directory");
        let tree = Self { root };
        tree.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#);
        tree.write("src/app.ts", "const { t } = useTranslation()\n");
        tree
    }

    fn write(&self, relative: &str, contents: &str) {
        let target = self.root.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("a parent directory");
        }
        std::fs::write(&target, contents).expect("a file");
    }

    fn locales(&self) -> PathBuf {
        self.root.join("locales")
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// A flat catalogue of `keys` keys. Flat and dotted, which is the shape
/// the duplicate resolver walks in one object rather than in many small
/// ones — the shape the quadratic lived in.
fn flat(seeded: &mut Seeded, keys: usize, translated: bool) -> String {
    let mut document = String::with_capacity(keys * 64);
    document.push('{');
    for index in 0..keys {
        if index > 0 {
            document.push(',');
        }
        let placeholder = if seeded.below(10) == 0 && translated {
            "{{nombre}}"
        } else {
            "{{name}}"
        };
        let _ = write!(
            document,
            "\"section{}.key{index}\":\"Texto {index} con {placeholder} y relleno\"",
            index % 40
        );
    }
    document.push('}');
    document
}

/// Best of three: a shared runner will occasionally lose a scheduling
/// slice, and one unlucky run is not a regression.
fn measure(target: &Path) -> Duration {
    let mut best = Duration::MAX;
    for _ in 0..3 {
        let started = Instant::now();
        let output = Command::new(BINARY)
            .arg(target)
            .stdin(Stdio::null())
            .output()
            .expect("the binary runs");
        let elapsed = started.elapsed();
        assert!(
            matches!(output.status.code(), Some(0 | 1)),
            "the run was refused: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        best = best.min(elapsed);
    }
    best
}

fn ratio(one: Duration, four: Duration) -> f64 {
    four.as_secs_f64() / one.as_secs_f64().max(0.000_001)
}

/// The shape a mature product actually has, end to end: identification,
/// twenty-five catalogues read, and every one of them compared against
/// the source.
#[test]
fn a_twenty_five_locale_set_completes_inside_its_budget() {
    if !enabled("a_twenty_five_locale_set_completes_inside_its_budget") {
        return;
    }
    let tree = Tree::new("set");
    let mut seeded = Seeded(SEED);
    tree.write("locales/en.json", &flat(&mut seeded, KEYS, false));
    for tag in TAGS {
        tree.write(
            &format!("locales/{tag}.json"),
            &flat(&mut seeded, KEYS, true),
        );
    }

    let locales = TAGS.len() + 1;
    let elapsed = measure(&tree.locales());
    eprintln!("budget: {locales}×{KEYS} keys in {elapsed:?} (ceiling {SET_CEILING:?})");
    assert!(
        elapsed < SET_CEILING,
        "a {locales}-locale set took {elapsed:?}, over the {SET_CEILING:?} ceiling"
    );
}

/// **The one that catches the quadratic.** Four times the keys in one
/// flat catalogue cost sixteen times the clock while duplicate
/// resolution rescanned; it costs about four now.
#[test]
fn four_times_the_keys_in_one_catalogue_is_not_six_times_the_clock() {
    if !enabled("four_times_the_keys_in_one_catalogue_is_not_six_times_the_clock") {
        return;
    }
    let small = Tree::new("keys-small");
    let large = Tree::new("keys-large");
    for (tree, keys) in [(&small, 6_000), (&large, 24_000)] {
        let mut seeded = Seeded(SEED);
        tree.write("locales/en.json", &flat(&mut seeded, keys, false));
        let mut seeded = Seeded(SEED);
        tree.write("locales/es.json", &flat(&mut seeded, keys, true));
    }

    let one = measure(&small.locales());
    let four = measure(&large.locales());
    let ratio = ratio(one, four);
    eprintln!("budget: 6,000 keys in {one:?}, 24,000 in {four:?} — {ratio:.1}× for 4× the keys");
    assert!(
        four < FLAT_CEILING,
        "24,000 keys took {four:?}, over the {FLAT_CEILING:?} ceiling"
    );
    assert!(
        ratio < LINEARITY,
        "four times the keys cost {ratio:.1}× the clock, over {LINEARITY}× — \
         duplicate resolution or the comparator is scanning more than once"
    );
}

/// And the other direction: four times the *locales*, one source, every
/// one of them compared against it.
#[test]
fn four_times_the_locales_is_not_six_times_the_clock() {
    if !enabled("four_times_the_locales_is_not_six_times_the_clock") {
        return;
    }
    let small = Tree::new("locales-small");
    let large = Tree::new("locales-large");
    for (tree, locales) in [(&small, 6), (&large, 24)] {
        let mut seeded = Seeded(SEED);
        tree.write("locales/en.json", &flat(&mut seeded, KEYS, false));
        for tag in TAGS.iter().take(locales - 1) {
            tree.write(
                &format!("locales/{tag}.json"),
                &flat(&mut seeded, KEYS, true),
            );
        }
    }

    let one = measure(&small.locales());
    let four = measure(&large.locales());
    let ratio = ratio(one, four);
    eprintln!("budget: 6 locales in {one:?}, 24 in {four:?} — {ratio:.1}× for 4× the locales");
    assert!(
        ratio < LINEARITY,
        "four times the locales cost {ratio:.1}× the clock, over {LINEARITY}×"
    );
}

/// A catalogue that is one enormous object of repeated keys — the exact
/// input the duplicate resolver walks — held to the same bound. Two
/// thousand occurrences of one key used to be two thousand scans of two
/// thousand entries.
#[test]
fn a_catalogue_of_repeated_keys_stays_linear() {
    if !enabled("a_catalogue_of_repeated_keys_stays_linear") {
        return;
    }
    let repeated = |count: usize| {
        let mut document = String::from("{\"a\":\"uno\"");
        for index in 0..count {
            let _ = write!(document, ",\"dup\":\"valor {index}\"");
        }
        document.push('}');
        document
    };

    let small = Tree::new("dup-small");
    let large = Tree::new("dup-large");
    for (tree, count) in [(&small, 5_000), (&large, 20_000)] {
        tree.write("locales/en.json", "{\"a\":\"one {{name}}\"}");
        tree.write("locales/es.json", &repeated(count));
    }

    let one = measure(&small.locales());
    let four = measure(&large.locales());
    let ratio = ratio(one, four);
    eprintln!("budget: 5,000 duplicates in {one:?}, 20,000 in {four:?} — {ratio:.1}×");
    assert!(
        ratio < LINEARITY,
        "four times the duplicates cost {ratio:.1}× the clock, over {LINEARITY}× — \
         the resolver is scanning the object once per key again"
    );
}
