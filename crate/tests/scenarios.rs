//! The tier that needs a catalogue set larger than an editor opens.
//!
//! Gated behind `I18N_LE_SCENARIOS`. Nothing here substitutes for the
//! unit tests, which run everywhere on every push.
//!
//! **A skipped scenario is never reported as a pass.**

use std::fmt::Write as _;

fn enabled(name: &str) -> bool {
    if std::env::var_os("I18N_LE_SCENARIOS").is_some() {
        return true;
    }
    eprintln!("SKIPPED {name}: set I18N_LE_SCENARIOS to run it");
    false
}

struct Tree(std::path::PathBuf);

impl Tree {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("i18n-le-scenario-{name}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a temporary directory");
        Self(root)
    }
    /// A project that identifies as i18next.
    fn i18next(&self) {
        self.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#);
        self.write("src/app.ts", "useTranslation()\n");
    }

    fn write(&self, relative: &str, contents: &str) {
        let target = self.0.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("a parent directory");
        }
        std::fs::write(target, contents).expect("a file");
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(args: &[&str]) -> (i32, serde_json::Value) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_i18n-le"))
        .args(args)
        .output()
        .expect("the binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    (
        output.status.code().expect("an exit code"),
        serde_json::from_str(stdout.trim()).expect("stdout carries JSON"),
    )
}

fn catalogue(keys: usize, value: impl Fn(usize) -> String) -> String {
    let mut content = String::from("{\n");
    for key in 0..keys {
        let separator = if key + 1 == keys { "" } else { "," };
        let _ = writeln!(content, "  \"key.{key}\": \"{}\"{separator}", value(key));
    }
    content.push('}');
    content
}

/// The real shape: a mature product with a few thousand strings and
/// every locale it ships. Comparison is quadratic if done naively —
/// every key of every locale against every key of the source.
#[test]
fn a_full_locale_set_completes() {
    if !enabled("a_full_locale_set_completes") {
        return;
    }
    let tree = Tree::new("wide");
    tree.i18next();
    tree.write(
        "locales/en.json",
        &catalogue(4_000, |key| format!("String {key} for {{{{name}}}}")),
    );
    for locale in [
        "es", "fr", "de", "it", "nl", "pl", "ja", "ko", "tr", "cs", "ro", "sv", "nb", "da", "fi",
        "el", "hu", "uk", "ru", "bg", "id", "vi",
    ] {
        // Every tenth key renames its placeholder, which is the failure
        // machine translation actually produces.
        tree.write(
            &format!("locales/{locale}.json"),
            &catalogue(4_000, |key| {
                let token = if key % 10 == 0 { "nombre" } else { "name" };
                format!("Cadena {key} para {{{{{token}}}}}")
            }),
        );
    }

    let (code, report) = run(&[&tree.0.join("locales").to_string_lossy()]);
    assert_eq!(code, 1);
    assert_eq!(report["summary"]["files"], 23);
    assert_eq!(report["summary"]["findings"], 22 * 400);
}

fn nested(levels: usize) -> String {
    let mut open = String::new();
    let mut close = String::new();
    for level in 0..levels {
        let _ = write!(open, "{{\"level{level}\":");
        close.push('}');
    }
    format!("{open}\"leaf\"{close}")
}

/// One catalogue nested deeply enough that the path join is the
/// expensive part.
#[test]
fn a_deeply_nested_catalogue_completes() {
    if !enabled("a_deeply_nested_catalogue_completes") {
        return;
    }
    let tree = Tree::new("deep");
    tree.i18next();
    let document = nested(100);
    tree.write("locales/en.json", &document);
    tree.write("locales/es.json", &document);

    let (code, report) = run(&[&tree.0.join("locales").to_string_lossy()]);
    assert_eq!(
        code, 0,
        "the two are identical apart from being untranslated"
    );
    assert_eq!(report["findings"][0]["kind"], "untranslated");
}

/// A catalogue nested past the JSON reader's recursion limit is a named
/// diagnostic, not a stack overflow. Nothing writes one on purpose, but
/// a generator can, and a crash here would take the whole audit with it.
#[test]
fn a_pathologically_nested_catalogue_is_named_rather_than_crashing() {
    if !enabled("a_pathologically_nested_catalogue_is_named_rather_than_crashing") {
        return;
    }
    let tree = Tree::new("pathological");
    tree.i18next();
    tree.write("locales/en.json", "{\"a\":\"one\"}");
    tree.write("locales/es.json", &nested(5_000));

    let (code, report) = run(&[&tree.0.join("locales").to_string_lossy()]);
    assert_eq!(code, 0, "the one catalogue that parsed has nothing wrong");
    assert_eq!(report["diagnostics"][0]["file"], "es.json");
    assert_eq!(report["diagnostics"][0]["code"], "unparsable");
}

/// A single string carrying a great many placeholders, and one locale
/// that drops half of them.
#[test]
fn a_string_with_many_placeholders_completes() {
    if !enabled("a_string_with_many_placeholders_completes") {
        return;
    }
    let tree = Tree::new("placeholders");
    tree.i18next();
    let mut source = String::new();
    let mut target = String::new();
    for token in 0..5_000 {
        let _ = write!(source, "{{{{token{token}}}}} ");
        if token % 2 == 0 {
            let _ = write!(target, "{{{{token{token}}}}} ");
        }
    }
    tree.write(
        "locales/en.json",
        &format!("{{\"one\":\"{}\"}}", source.trim()),
    );
    tree.write(
        "locales/es.json",
        &format!("{{\"one\":\"{}\"}}", target.trim()),
    );

    let (code, report) = run(&[&tree.0.join("locales").to_string_lossy()]);
    assert_eq!(code, 1);
    assert_eq!(report["findings"][0]["kind"], "placeholder-count-mismatch");
    assert_eq!(report["findings"][0]["sourceCount"], 5_000);
    assert_eq!(report["findings"][0]["targetCount"], 2_500);
}
