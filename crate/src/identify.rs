//! The front door: which i18n library does this project actually use?
//!
//! **Identification runs first and everything else is downstream of
//! it.** It is not a helper the audit consults when it gets stuck — the
//! audit cannot start without an answer, because the grammar, the plural
//! model, the metadata rule and the file layout are all read off the row
//! it returns.
//!
//! **Corroboration, not inference.** Five classes of evidence, and two
//! agreeing is an identification:
//!
//! 1. **Manifest** — a dependency in `package.json` or `pubspec.yaml`.
//! 2. **Config** — `i18next-parser.config.*`, `next-intl.config.*`,
//!    `l10n.yaml`.
//! 3. **Layout** — a directory shape only one library writes.
//! 4. **Content** — the catalogue's own syntax. The strongest class,
//!    because it is what actually breaks at runtime.
//! 5. **Call site** — `useTranslation()`, `vscode.l10n.t(`,
//!    `AppLocalizations.of(` in source files.
//!
//! A signature marked decisive identifies on its own, which is what
//! makes a translations-only repository — no manifest, no config, no
//! call sites — auditable at all.
//!
//! Two libraries reaching an identification is a **refusal naming
//! both**. Nothing reaching one is a refusal naming every signal that
//! was found. There is no fallback that guesses, because guessing is
//! the thing this replaced.
//!
//! ## The call-site boundary
//!
//! Source files are read here, and **only** here, and **only** to answer
//! which library this is. No finding ever comes from a source file:
//! unused-key and undefined-key detection stay non-goals, for the reason
//! they always were — they make the tool language-coupled and wrong
//! about every dynamically built key. The scan looks for fixed
//! substrings, keeps nothing, and reports only the file's path. See
//! SPEC.md, which says the same thing where a reader will find it.

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::catalogue;
use crate::library::{self, Id, Layout, Manifest, Mark, Shape, Strength};
use crate::locale;
use crate::message;

/// How far up to look for a manifest, a config, or a prefixed set's base
/// file. Deep enough for a monorepo package nested a few levels down,
/// shallow enough that a stray `package.json` near `/` is never the
/// answer.
const ANCESTORS: usize = 8;

/// Caps on the call-site scan. It reads other people's source code, so
/// it stays cheap and bounded rather than thorough.
const MAX_SOURCE_FILES: usize = 400;
const MAX_SOURCE_BYTES: u64 = 512 * 1024;
const MAX_SOURCE_DEPTH: usize = 6;

/// Catalogues parsed for content evidence. Enough to see the syntax,
/// not the whole set.
const MAX_SAMPLED: usize = 12;

/// Directories whose contents are somebody else's code or this
/// project's own build output. A call site found in either is true but
/// useless: it names a compiled artefact rather than the source a
/// reader would go and look at.
const SKIPPED_DIRECTORIES: [&str; 11] = [
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    ".next",
    "out",
    "out-test",
    "coverage",
    "vendor",
    "lib",
];

const SOURCE_EXTENSIONS: [&str; 9] = [
    "ts", "tsx", "js", "jsx", "mjs", "cjs", "vue", "svelte", "dart",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Class {
    Manifest,
    Config,
    Layout,
    Content,
    CallSite,
}

/// One piece of evidence, with enough detail that a reader can check it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Signal {
    pub(crate) class: Class,
    pub(crate) library: Id,
    /// What was seen and where — a path relative to the catalogues, a
    /// package name, a syntactic mark. Never file content.
    pub(crate) detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Located {
    pub(crate) path: PathBuf,
    /// The name this file is reported under.
    pub(crate) name: String,
    pub(crate) locale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Identified {
    pub(crate) library: Id,
    /// As declared, when a manifest declared one.
    pub(crate) version: Option<String>,
    pub(crate) shape: Shape,
    pub(crate) keys_are_source: bool,
    pub(crate) evidence: Vec<Signal>,
    pub(crate) files: Vec<Located>,
}

/// What the caller said about the library question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Requested {
    #[default]
    Detect,
    Named(Id),
}

/// Identify the library behind `inputs`, then read its catalogues.
pub(crate) fn identify(inputs: &[PathBuf], requested: Requested) -> Result<Identified, String> {
    let anchor = anchor_of(inputs)?;
    let evidence = gather(&anchor, inputs);

    let library = match requested {
        Requested::Named(id) => id,
        Requested::Detect => {
            let decided = decide(&evidence)?;
            // The version gate belongs to identification, not to the
            // audit: a caller who names the library has made the claim
            // themselves and does not need to be told it is unfamiliar.
            check_version(&anchor, decided)?;
            decided
        }
    };

    let (shape, keys_are_source, files) = read(inputs, &anchor, library)?;
    Ok(Identified {
        library,
        version: declared_version(&anchor, library),
        shape,
        keys_are_source,
        evidence: evidence
            .into_iter()
            .filter(|signal| signal.library == library)
            .collect(),
        files,
    })
}

fn anchor_of(inputs: &[PathBuf]) -> Result<PathBuf, String> {
    let first = inputs
        .first()
        .ok_or_else(|| "name the directory holding the catalogues".to_string())?;
    let metadata =
        std::fs::metadata(first).map_err(|error| format!("{}: {error}", first.display()))?;
    let directory = if metadata.is_dir() {
        first.clone()
    } else {
        first.parent().unwrap_or(Path::new(".")).to_path_buf()
    };
    std::fs::canonicalize(&directory).map_err(|error| format!("{}: {error}", directory.display()))
}

// ---------------------------------------------------------------------
// The decision
// ---------------------------------------------------------------------

/// Two agreeing classes, or one decisive signature.
///
/// **Never pick between two answers.** A repository that declares
/// i18next and also lays out `messages/` for next-intl has two answers
/// and nothing here can know which the files in front of it belong to;
/// saying so beats choosing, because every later answer would inherit
/// the coin flip.
fn decide(evidence: &[Signal]) -> Result<Id, String> {
    let decisive: Vec<Id> = library::all()
        .iter()
        .filter(|library| {
            library
                .signatures
                .iter()
                .filter(|signature| signature.strength == Strength::Decisive)
                .any(|signature| carries(evidence, library.id, Class::Content, signature.mark))
        })
        .map(|library| library.id)
        .collect();
    if decisive.len() == 1 {
        return Ok(decisive[0]);
    }

    let identified: Vec<Id> = library::all()
        .iter()
        .map(|library| library.id)
        .filter(|id| classes_for(evidence, *id) >= 2)
        .collect();
    let candidates = if decisive.is_empty() {
        identified
    } else {
        decisive
    };

    match candidates.as_slice() {
        [only] => Ok(*only),
        [] => Err(unidentified(evidence)),
        several => Err(conflicted(evidence, several)),
    }
}

fn classes_for(evidence: &[Signal], id: Id) -> usize {
    let mut classes: Vec<Class> = Vec::new();
    for signal in evidence.iter().filter(|signal| signal.library == id) {
        if !classes.contains(&signal.class) {
            classes.push(signal.class);
        }
    }
    classes.len()
}

fn carries(evidence: &[Signal], id: Id, class: Class, mark: Mark) -> bool {
    let wanted = serde_json::to_value(mark)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default();
    evidence
        .iter()
        .any(|signal| signal.library == id && signal.class == class && signal.detail == wanted)
}

fn unidentified(evidence: &[Signal]) -> String {
    if evidence.is_empty() {
        return "no i18n library could be identified here — nothing in the manifests, the \
                config files, the layout, the catalogue syntax or the call sites points at one. \
                Name it with --system <name>."
            .to_string();
    }
    format!(
        "no i18n library could be identified here. Found, but not enough to agree: {}. \
         Two classes of evidence are needed. Name it with --system <name>.",
        listed(evidence)
    )
}

fn conflicted(evidence: &[Signal], several: &[Id]) -> String {
    let each: Vec<String> = several
        .iter()
        .map(|id| {
            let for_this: Vec<&Signal> = evidence
                .iter()
                .filter(|signal| signal.library == *id)
                .collect();
            format!(
                "{} ({})",
                id.as_str(),
                for_this
                    .iter()
                    .map(|signal| describe(signal))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
        .collect();
    format!(
        "{} libraries are identified here and only one can be right: {}. \
         Name the one these catalogues belong to with --system <name>.",
        several.len(),
        each.join("; ")
    )
}

fn listed(evidence: &[Signal]) -> String {
    evidence
        .iter()
        .map(|signal| format!("{} {}", signal.library.as_str(), describe(signal)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn describe(signal: &Signal) -> String {
    let class = match signal.class {
        Class::Manifest => "manifest",
        Class::Config => "config",
        Class::Layout => "layout",
        Class::Content => "content",
        Class::CallSite => "call site",
    };
    format!("{class}: {}", signal.detail)
}

// ---------------------------------------------------------------------
// Gathering
// ---------------------------------------------------------------------

fn gather(anchor: &Path, inputs: &[PathBuf]) -> Vec<Signal> {
    let mut evidence = Vec::new();
    evidence.extend(manifest_signals(anchor));
    evidence.extend(config_signals(anchor));
    evidence.extend(layout_signals(anchor, inputs));
    evidence.extend(content_signals(anchor, inputs));
    evidence.extend(call_signals(anchor));
    evidence
}

/// A path as the report and every refusal shows it: relative to the
/// catalogues, never absolute.
///
/// An absolute `/Users/someone/work/…` in a report meant to be diffed
/// between machines makes every such diff a difference.
fn shown(path: &Path, anchor: &Path) -> String {
    for (up, ancestor) in anchor.ancestors().enumerate() {
        if let Ok(rest) = path.strip_prefix(ancestor) {
            let rest = rest
                .components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            return format!("{}{rest}", "../".repeat(up));
        }
    }
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn manifest_signals(anchor: &Path) -> Vec<Signal> {
    let mut signals = Vec::new();
    for directory in anchor.ancestors().take(ANCESTORS) {
        signals.extend(npm_signals(&directory.join("package.json"), anchor));
        signals.extend(pubspec_signals(&directory.join("pubspec.yaml"), anchor));
    }
    signals
}

fn npm_signals(path: &Path, anchor: &Path) -> Vec<Signal> {
    let Some(manifest) = read_json(path) else {
        return Vec::new();
    };
    let at = shown(path, anchor);
    let mut signals = Vec::new();

    for section in ["dependencies", "devDependencies", "peerDependencies"] {
        let Some(entries) = manifest.get(section).and_then(Value::as_object) else {
            continue;
        };
        for package in entries.keys() {
            for library in library::all() {
                if library.package(package, Manifest::Npm).is_some() {
                    signals.push(Signal {
                        class: Class::Manifest,
                        library: library.id,
                        detail: format!("{package} in {at}"),
                    });
                }
            }
        }
    }

    // VS Code declares its bundle directory in the manifest rather than
    // depending on a package — the mechanism is the editor, so there is
    // nothing to install.
    if manifest.get("l10n").is_some() {
        signals.push(Signal {
            class: Class::Manifest,
            library: Id::VscodeL10n,
            detail: format!("the l10n field in {at}"),
        });
    }
    signals
}

/// `pubspec.yaml`, read for one question only: does it name a package
/// this knows?
///
/// **Not a YAML parser**, and not pretending to be one — a dependency
/// for a line-prefix test would cost more than it answers. It reads the
/// indented keys under `dependencies:` and `dev_dependencies:`, which is
/// how every pubspec in the world is written.
fn pubspec_signals(path: &Path, anchor: &Path) -> Vec<Signal> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let at = shown(path, anchor);
    let mut signals = Vec::new();
    let mut inside = false;

    for line in text.lines() {
        let trimmed = line.trim_end();
        if !trimmed.starts_with(char::is_whitespace) && !trimmed.is_empty() {
            inside = matches!(trimmed, "dependencies:" | "dev_dependencies:");
            continue;
        }
        if !inside {
            continue;
        }
        let Some((package, _)) = trimmed.trim().split_once(':') else {
            continue;
        };
        for library in library::all() {
            if library.package(package.trim(), Manifest::Pubspec).is_some() {
                signals.push(Signal {
                    class: Class::Manifest,
                    library: library.id,
                    detail: format!("{} in {at}", package.trim()),
                });
            }
        }
    }
    signals
}

fn config_signals(anchor: &Path) -> Vec<Signal> {
    let mut signals = Vec::new();
    for directory in anchor.ancestors().take(ANCESTORS) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            // A config is a *file with an extension*: `l10n.yaml` is
            // Flutter's, and a directory called `l10n` is not it.
            let Some((stem, _)) = name.rsplit_once('.') else {
                continue;
            };
            for library in library::all() {
                if library.configs.contains(&stem) {
                    signals.push(Signal {
                        class: Class::Config,
                        library: library.id,
                        detail: shown(&entry.path(), anchor),
                    });
                }
            }
        }
    }
    signals
}

fn layout_signals(anchor: &Path, inputs: &[PathBuf]) -> Vec<Signal> {
    let here = anchor
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();

    let mut signals = Vec::new();
    for library in library::all() {
        for layout in library.layouts {
            if !layout.directories.is_empty() && !layout.directories.contains(&here.as_str()) {
                continue;
            }
            // A directory of `<locale>.json` is what almost every
            // project's translations look like, so that alone must not
            // vote. A directory of `<locale>.arb` is another matter —
            // one library writes that extension.
            if let Shape::Shared { extension } = layout.shape
                && extension == "json"
                && layout.directories.is_empty()
            {
                continue;
            }
            if read_layout(inputs, anchor, layout).ok().flatten().is_some() {
                signals.push(Signal {
                    class: Class::Layout,
                    library: library.id,
                    detail: describe_shape(layout.shape),
                });
            }
        }
    }
    signals
}

fn describe_shape(shape: Shape) -> String {
    match shape {
        Shape::Shared { extension } => format!("a directory of <locale>.{extension}"),
        Shape::Fixed { prefix, extension } => format!("{prefix}.<locale>.{extension}"),
        Shape::Namespaced { extension } => format!("<locale>/<namespace>.{extension}"),
    }
}

/// The catalogues' own syntax — the strongest class, because it is what
/// actually breaks at runtime.
fn content_signals(anchor: &Path, inputs: &[PathBuf]) -> Vec<Signal> {
    let mut marks: Vec<Mark> = Vec::new();
    for path in sample(anchor, inputs) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(parsed) = catalogue::parse(&text) else {
            continue;
        };
        let keys: Vec<String> = parsed
            .entries
            .iter()
            .map(|entry| entry.key.clone())
            .collect();
        for mark in message::key_marks(&keys) {
            if !marks.contains(&mark) {
                marks.push(mark);
            }
        }
        for text in parsed
            .entries
            .iter()
            .filter_map(|entry| entry.text.as_deref())
        {
            for mark in message::marks(text) {
                if !marks.contains(&mark) {
                    marks.push(mark);
                }
            }
        }
    }

    let mut signals = Vec::new();
    for library in library::all() {
        for signature in library.signatures {
            if !marks.contains(&signature.mark) {
                continue;
            }
            // A weak mark is recorded so a refusal can say what it saw,
            // but it does not vote: `{name}` is prose in every library's
            // catalogues, including the ones that do not read it.
            if signature.strength == Strength::Weak {
                continue;
            }
            signals.push(Signal {
                class: Class::Content,
                library: library.id,
                detail: serde_json::to_value(signature.mark)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_string))
                    .unwrap_or_default(),
            });
        }
    }
    signals
}

/// Catalogue files to read for content evidence, layout-independently.
fn sample(anchor: &Path, inputs: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for input in inputs {
        if input.is_file() {
            found.push(input.clone());
        }
    }
    found.extend(catalogues_in(anchor));
    if found.is_empty() {
        // The namespaced layout keeps its catalogues one level down.
        for entry in std::fs::read_dir(anchor).into_iter().flatten().flatten() {
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                found.extend(catalogues_in(&entry.path()));
            }
        }
    }
    found.sort();
    found.dedup();
    found.truncate(MAX_SAMPLED);
    found
}

fn catalogues_in(directory: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(directory).into_iter().flatten().flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        let path = entry.path();
        if path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension == "json" || extension == "arb")
        {
            found.push(path);
        }
    }
    found.sort();
    found
}

/// Call sites, read from source **only** to answer which library this
/// is. Nothing read here reaches a finding.
fn call_signals(anchor: &Path) -> Vec<Signal> {
    let root = project_root(anchor);
    let mut signals: Vec<Signal> = Vec::new();
    let mut budget = MAX_SOURCE_FILES;
    walk_source(&root, 0, &mut budget, &mut |path, text| {
        for library in library::all() {
            let already = signals
                .iter()
                .any(|signal| signal.library == library.id && signal.class == Class::CallSite);
            if already {
                continue;
            }
            if let Some(call) = library.calls.iter().find(|call| text.contains(**call)) {
                signals.push(Signal {
                    class: Class::CallSite,
                    library: library.id,
                    detail: format!("{call} in {}", shown(path, anchor)),
                });
            }
        }
    });
    signals
}

/// The deepest ancestor that looks like a project, so the walk down does
/// not start at the filesystem root.
fn project_root(anchor: &Path) -> PathBuf {
    anchor
        .ancestors()
        .take(ANCESTORS)
        .find(|directory| {
            directory.join("package.json").is_file() || directory.join("pubspec.yaml").is_file()
        })
        .unwrap_or(anchor)
        .to_path_buf()
}

fn walk_source(
    directory: &Path,
    depth: usize,
    budget: &mut usize,
    visit: &mut impl FnMut(&Path, &str),
) {
    if depth > MAX_SOURCE_DEPTH || *budget == 0 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    let mut directories = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            if !name.starts_with('.') && !SKIPPED_DIRECTORIES.contains(&name.as_str()) {
                directories.push(path);
            }
            continue;
        }
        let is_source = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| SOURCE_EXTENSIONS.contains(&extension));
        if !is_source || *budget == 0 {
            continue;
        }
        if std::fs::metadata(&path).is_ok_and(|meta| meta.len() > MAX_SOURCE_BYTES) {
            continue;
        }
        *budget -= 1;
        if let Ok(text) = std::fs::read_to_string(&path) {
            visit(&path, &text);
        }
    }
    directories.sort();
    for child in directories {
        walk_source(&child, depth + 1, budget, visit);
    }
}

fn read_json(path: &Path) -> Option<Value> {
    // A manifest that will not parse is not a refusal: it is somebody
    // else's broken file, and the catalogues beside it may be fine.
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
}

// ---------------------------------------------------------------------
// Version
// ---------------------------------------------------------------------

fn declared_version(anchor: &Path, library: Id) -> Option<String> {
    let row = library.library();
    for directory in anchor.ancestors().take(ANCESTORS) {
        // `continue`, not `?`: the catalogues' own directory rarely has a
        // manifest, and returning at the first one that does not would
        // never look any higher.
        let Some(manifest) = read_json(&directory.join("package.json")) else {
            continue;
        };
        for section in ["dependencies", "devDependencies", "peerDependencies"] {
            let Some(entries) = manifest.get(section).and_then(Value::as_object) else {
                continue;
            };
            for (package, spec) in entries {
                if row.package(package, Manifest::Npm).is_some() {
                    return spec.as_str().map(str::to_string);
                }
            }
        }
    }
    None
}

/// The only version gate: a major below which the library's own model
/// was different, so reading its catalogues as this one would be wrong
/// rather than merely unverified.
fn check_version(anchor: &Path, library: Id) -> Result<(), String> {
    let row = library.library();
    for directory in anchor.ancestors().take(ANCESTORS) {
        let Some(manifest) = read_json(&directory.join("package.json")) else {
            continue;
        };
        for section in ["dependencies", "devDependencies", "peerDependencies"] {
            let Some(entries) = manifest.get(section).and_then(Value::as_object) else {
                continue;
            };
            for (package, spec) in entries {
                let Some(known) = row.package(package, Manifest::Npm) else {
                    continue;
                };
                let Some(floor) = known.breaking_below else {
                    continue;
                };
                let Some(major) = spec.as_str().and_then(major_of) else {
                    continue;
                };
                if major < floor {
                    return Err(format!(
                        "{package} {} in {} writes its catalogues differently from {floor} and \
                         later, which is the model this reads. Auditing it as {} would be wrong.",
                        spec.as_str().unwrap_or_default(),
                        shown(&directory.join("package.json"), anchor),
                        library.as_str()
                    ));
                }
            }
        }
    }
    Ok(())
}

fn major_of(spec: &str) -> Option<u64> {
    let spec = match spec.strip_prefix("npm:") {
        Some(alias) => alias.rsplit_once('@').map(|(_, version)| version)?,
        None => spec,
    };
    let digits: String = spec
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

// ---------------------------------------------------------------------
// Reading the set, once the library is known
// ---------------------------------------------------------------------

struct Reading {
    files: Vec<Located>,
    shape: Shape,
    keys_are_source: bool,
}

fn read(
    inputs: &[PathBuf],
    anchor: &Path,
    library: Id,
) -> Result<(Shape, bool, Vec<Located>), String> {
    let row = library.library();
    let mut refusal = None;
    for layout in row.layouts {
        match read_layout(inputs, anchor, layout) {
            Ok(Some(reading)) => {
                return Ok((reading.shape, reading.keys_are_source, reading.files));
            }
            Ok(None) => {}
            // Kept rather than returned: another layout of the same
            // library may still read these files cleanly, and only if
            // none does is this the answer.
            Err(problem) => refusal = Some(problem),
        }
    }
    if let Some(problem) = refusal {
        return Err(problem);
    }
    // A directory with no catalogues in it at all is not a malformed
    // question — there is simply nothing to be wrong with, and the
    // report says so with `no-files`.
    let empty = row.layouts.iter().all(|layout| match layout.shape {
        Shape::Shared { extension }
        | Shape::Fixed { extension, .. }
        | Shape::Namespaced { extension } => collect(inputs, extension).is_empty(),
    });
    if empty && let Some(layout) = row.layouts.first() {
        return Ok((layout.shape, layout.keys_are_source, Vec::new()));
    }
    Err(format!(
        "{} was identified, but none of its layouts reads the files here ({}).",
        library.as_str(),
        row.layouts
            .iter()
            .map(|layout| describe_shape(layout.shape))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

fn read_layout(
    inputs: &[PathBuf],
    anchor: &Path,
    layout: &Layout,
) -> Result<Option<Reading>, String> {
    match layout.shape {
        Shape::Shared { extension } => shared(inputs, extension, layout),
        Shape::Fixed { prefix, extension } => fixed(inputs, anchor, prefix, extension, layout),
        Shape::Namespaced { extension } => namespaced(inputs, extension, layout),
    }
}

fn collect(inputs: &[PathBuf], extension: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for input in inputs {
        if input.is_file() {
            files.push(input.clone());
            continue;
        }
        files.extend(
            catalogues_in(input)
                .into_iter()
                .filter(|path| has_extension(path, extension)),
        );
    }
    files.sort();
    files.dedup();
    files
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|found| found.to_str())
        .is_some_and(|found| found == extension)
}

fn shared(inputs: &[PathBuf], extension: &str, layout: &Layout) -> Result<Option<Reading>, String> {
    let files = collect(inputs, extension);
    if files.is_empty() {
        return Ok(None);
    }
    // A flat set's names are only readable together, so it must live in
    // one directory.
    one_directory(&files)?;

    let names: Vec<String> = files.iter().map(|path| name_of(path)).collect();
    // Propagated rather than swallowed: it names the file that is not a
    // locale, which is exactly what a person pointing this at the wrong
    // directory needs to read. Evidence gathering discards it, so a
    // directory that merely does not fit this layout still votes
    // nothing rather than exploding.
    let locales = locale::locales_of(&names)?;
    Ok(Some(Reading {
        files: zip(&files, names, locales),
        shape: layout.shape,
        keys_are_source: layout.keys_are_source,
    }))
}

/// A prefix the library fixes makes each name readable on its own, which
/// is what lets the set span two directories.
///
/// **This is the layout that matters most in practice.** Every VS Code
/// extension puts `package.nls.json` in its root and its translations
/// wherever the build wants them, and nothing before this could audit
/// that shape at all.
fn fixed(
    inputs: &[PathBuf],
    anchor: &Path,
    prefix: &str,
    extension: &str,
    layout: &Layout,
) -> Result<Option<Reading>, String> {
    let mut files: Vec<PathBuf> = collect(inputs, extension)
        .into_iter()
        .filter(|path| name_of(path).starts_with(prefix))
        .collect();
    if files.is_empty() {
        return Ok(None);
    }

    let base = format!("{prefix}.{extension}");
    if !files.iter().any(|path| name_of(path) == base) {
        let found = anchor
            .ancestors()
            .take(ANCESTORS)
            .map(|directory| directory.join(&base))
            .find(|candidate| candidate.is_file());
        if let Some(found) = found {
            files.push(found);
        }
    }
    files.sort();
    files.dedup();

    let names: Vec<String> = files.iter().map(|path| name_of(path)).collect();
    let locales = names
        .iter()
        .map(|name| locale::from_prefix(name, prefix))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(Reading {
        files: zip(&files, names, locales),
        shape: layout.shape,
        keys_are_source: layout.keys_are_source,
    }))
}

/// `<root>/<locale>/<namespace>.json`.
///
/// One namespace is one set. Several are several sets, and auditing them
/// together would compare `common.json` against `errors.json`, so they
/// are named and the caller picks.
fn namespaced(
    inputs: &[PathBuf],
    extension: &str,
    layout: &Layout,
) -> Result<Option<Reading>, String> {
    let [root] = inputs else {
        return Ok(None);
    };
    if !root.is_dir() {
        return Ok(None);
    }

    let mut directories = Vec::new();
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if locale::canonicalise(&name).is_none() {
            return Ok(None);
        }
        directories.push(entry.path());
    }
    if directories.is_empty() {
        return Ok(None);
    }
    directories.sort();

    let mut namespaces: Vec<String> = Vec::new();
    let mut files = Vec::new();
    for directory in &directories {
        for path in catalogues_in(directory)
            .into_iter()
            .filter(|path| has_extension(path, extension))
        {
            let name = name_of(&path);
            if !namespaces.contains(&name) {
                namespaces.push(name);
            }
            files.push(path);
        }
    }
    if files.is_empty() {
        return Ok(None);
    }
    if namespaces.len() > 1 {
        namespaces.sort();
        return Err(format!(
            "this set has {} namespaces ({}) and a namespace is its own set. \
             Name one namespace's files.",
            namespaces.len(),
            namespaces.join(", ")
        ));
    }

    let located = files
        .iter()
        .map(|path| {
            let tag = path
                .parent()
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let locale =
                locale::canonicalise(&tag).ok_or_else(|| format!("{tag} is not a language tag"))?;
            Ok(Located {
                name: format!("{tag}/{}", name_of(path)),
                locale: Some(locale),
                path: path.clone(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(Some(Reading {
        files: located,
        shape: layout.shape,
        keys_are_source: layout.keys_are_source,
    }))
}

fn zip(files: &[PathBuf], names: Vec<String>, locales: Vec<Option<String>>) -> Vec<Located> {
    files
        .iter()
        .zip(names)
        .zip(locales)
        .map(|((path, name), locale)| Located {
            path: path.clone(),
            name,
            locale,
        })
        .collect()
}

fn one_directory(files: &[PathBuf]) -> Result<(), String> {
    let directories: Vec<&Path> =
        files
            .iter()
            .filter_map(|file| file.parent())
            .fold(Vec::new(), |mut seen, parent| {
                if !seen.contains(&parent) {
                    seen.push(parent);
                }
                seen
            });
    if directories.len() > 1 {
        return Err(format!(
            "these files are in {} directories, and a set read by what its names share is one \
             directory. Audit them one directory at a time.",
            directories.len()
        ));
    }
    Ok(())
}

/// The name a file is reported under: a base name, because a full path
/// would put the machine that ran the audit into a report meant to be
/// diffed against one from another machine.
pub(crate) fn name_of(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn read_text(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    String::from_utf8(bytes).map_err(|_| format!("{}: not UTF-8 text", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TempTree;

    fn found(tree: &TempTree, at: &str) -> Identified {
        identify(&[tree.path().join(at)], Requested::Detect).expect("identifies")
    }

    fn refusal(tree: &TempTree, at: &str) -> String {
        identify(&[tree.path().join(at)], Requested::Detect).expect_err("a refusal")
    }

    fn names(identified: &Identified) -> Vec<String> {
        identified
            .files
            .iter()
            .map(|file| file.name.clone())
            .collect()
    }

    fn classes(identified: &Identified) -> Vec<Class> {
        let mut seen: Vec<Class> = Vec::new();
        for signal in &identified.evidence {
            if !seen.contains(&signal.class) {
                seen.push(signal.class);
            }
        }
        seen
    }

    /// **The front door, end to end.** Manifest plus content plus call
    /// sites, and the layout falls out of the identification.
    #[test]
    fn a_declared_library_is_identified_from_several_classes() {
        let tree = TempTree::new("identify-i18next");
        tree.write(
            "package.json",
            r#"{"dependencies":{"i18next":"^26.2.0","react-i18next":"^17.0.0"}}"#,
        );
        tree.write("src/app.tsx", "const { t } = useTranslation()\n");
        tree.write("locales/en.json", r#"{"greeting":"Hello {{name}}"}"#);
        tree.write("locales/es.json", r#"{"greeting":"Hola {{name}}"}"#);

        let identified = found(&tree, "locales");
        assert_eq!(identified.library, Id::I18next);
        assert_eq!(identified.version.as_deref(), Some("^26.2.0"));
        assert!(classes(&identified).contains(&Class::Manifest));
        assert!(classes(&identified).contains(&Class::Content));
        assert!(classes(&identified).contains(&Class::CallSite));
        assert_eq!(names(&identified), ["en.json", "es.json"]);
    }

    /// **A translations-only repository.** No manifest, no config, no
    /// call sites — and ARB's metadata shape is decisive on its own.
    #[test]
    fn a_decisive_signature_identifies_with_nothing_else_present() {
        let tree = TempTree::new("identify-arb");
        tree.write(
            "l10n/app_en.arb",
            r#"{"@@locale":"en","greeting":"Hi {name}","@greeting":{"description":"x"}}"#,
        );
        tree.write(
            "l10n/app_es.arb",
            r#"{"@@locale":"es","greeting":"Hola {name}"}"#,
        );
        let identified = found(&tree, "l10n");
        assert_eq!(identified.library, Id::FlutterArb);
        assert!(
            classes(&identified).contains(&Class::Content),
            "the decisive class"
        );
        assert!(
            !classes(&identified).contains(&Class::Manifest),
            "there is no project here at all"
        );
        assert_eq!(
            identified.files[0].locale.as_deref(),
            Some("en"),
            "app_en.arb"
        );
    }

    /// One class is not an identification. Corroboration is the whole
    /// rule.
    #[test]
    fn a_single_class_is_not_enough() {
        let tree = TempTree::new("identify-thin");
        // A directory no library claims, so the only signal is content.
        tree.write("strings/en.json", r#"{"a":"Hello {{name}}"}"#);
        tree.write("strings/es.json", r#"{"a":"Hola {{name}}"}"#);
        let error = refusal(&tree, "strings");
        assert!(error.contains("not enough to agree"), "{error}");
        assert!(error.contains("double-brace"), "{error}");
        assert!(error.contains("--system"), "{error}");
    }

    /// A directory named the way a library names it, plus that library's
    /// syntax, is two classes — and that is the whole point: a
    /// translations-only repository stays auditable.
    #[test]
    fn a_layout_and_its_syntax_are_enough_on_their_own() {
        let tree = TempTree::new("identify-two-thin");
        tree.write("locales/en.json", r#"{"a":"Hello {{name}}"}"#);
        tree.write("locales/es.json", r#"{"a":"Hola {{name}}"}"#);
        let identified = found(&tree, "locales");
        assert_eq!(identified.library, Id::I18next);
        assert_eq!(classes(&identified), [Class::Layout, Class::Content]);
    }

    #[test]
    fn nothing_at_all_is_refused_and_says_where_it_looked() {
        let tree = TempTree::new("identify-nothing");
        tree.write("strings/en.json", r#"{"a":"one"}"#);
        tree.write("strings/es.json", r#"{"a":"uno"}"#);
        let error = refusal(&tree, "strings");
        assert!(error.contains("manifest"), "{error}");
        assert!(error.contains("call sites"), "{error}");
    }

    /// **Never pick between two answers.**
    #[test]
    fn two_identified_libraries_are_refused_and_both_are_named() {
        let tree = TempTree::new("identify-conflict");
        tree.write(
            "package.json",
            r#"{"dependencies":{"i18next":"^26.0.0","next-intl":"^4.0.0"}}"#,
        );
        tree.write("src/a.tsx", "useTranslation()\nuseTranslations()\n");
        tree.write("messages/en.json", r#"{"a":"Hello {{name}}"}"#);
        tree.write("messages/es.json", r#"{"a":"Hola {{name}}"}"#);
        let error = refusal(&tree, "messages");
        assert!(error.contains("i18next"), "{error}");
        assert!(error.contains("next-intl"), "{error}");
        assert!(error.contains("only one can be right"), "{error}");
    }

    /// The override, which is what makes a repository nothing points at
    /// auditable.
    #[test]
    fn a_named_library_needs_no_evidence_at_all() {
        let tree = TempTree::new("identify-named");
        tree.write("l10n/bundle.l10n.json", r#"{"Save":"Save"}"#);
        tree.write("l10n/bundle.l10n.de.json", r#"{"Save":"Speichern"}"#);
        let identified = identify(
            &[tree.path().join("l10n")],
            Requested::Named(Id::VscodeL10n),
        )
        .expect("identifies");
        assert_eq!(identified.library, Id::VscodeL10n);
        assert!(identified.keys_are_source, "a bundle key is the English");
    }

    #[test]
    fn a_named_library_overrules_the_evidence() {
        let tree = TempTree::new("identify-override");
        tree.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#);
        tree.write("locales/en.json", r#"{"a":"Hello {{name}}"}"#);
        tree.write("locales/es.json", r#"{"a":"Hola {{name}}"}"#);
        let identified = identify(
            &[tree.path().join("locales")],
            Requested::Named(Id::NextIntl),
        )
        .expect("identifies");
        assert_eq!(identified.library, Id::NextIntl);
    }

    /// **The layout nothing before this could audit.** The base sits in
    /// the extension root and the translations do not.
    #[test]
    fn the_split_manifest_layout_is_read_as_one_set() {
        let tree = TempTree::new("identify-split");
        tree.write("package.json", r#"{"name":"x","l10n":"./l10n"}"#);
        tree.write("src/extension.ts", "vscode.l10n.t('Save')\n");
        tree.write("package.nls.json", r#"{"command.title":"Save"}"#);
        tree.write(
            "src/i18n/package.nls.de.json",
            r#"{"command.title":"Speichern"}"#,
        );
        tree.write(
            "src/i18n/package.nls.ja.json",
            r#"{"command.title":"保存"}"#,
        );

        let identified = found(&tree, "src/i18n");
        assert_eq!(identified.library, Id::VscodeL10n);
        assert_eq!(
            identified.shape,
            Shape::Fixed {
                prefix: "package.nls",
                extension: "json"
            }
        );
        assert_eq!(
            names(&identified),
            [
                "package.nls.json",
                "package.nls.de.json",
                "package.nls.ja.json"
            ]
        );
        assert!(!identified.keys_are_source, "an nls key is an identifier");
    }

    #[test]
    fn the_bundle_layout_reads_its_own_prefix() {
        let tree = TempTree::new("identify-bundle");
        tree.write("package.json", r#"{"name":"x","l10n":"./l10n"}"#);
        tree.write("l10n/bundle.l10n.json", r#"{"Save file":"Save file"}"#);
        tree.write("l10n/bundle.l10n.pt-br.json", r#"{"Save file":"Salvar"}"#);
        let identified = found(&tree, "l10n");
        assert_eq!(identified.library, Id::VscodeL10n);
        assert!(identified.keys_are_source);
        assert_eq!(
            identified.files[1].locale.as_deref(),
            Some("pt-BR"),
            "canonicalised from pt-br"
        );
    }

    #[test]
    fn the_namespaced_layout_reads_the_directory_as_the_locale() {
        let tree = TempTree::new("identify-namespaced");
        tree.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#);
        tree.write("locales/en/common.json", r#"{"a":"Hello {{name}}"}"#);
        tree.write("locales/de/common.json", r#"{"a":"Hallo {{name}}"}"#);
        let identified = found(&tree, "locales");
        assert_eq!(identified.library, Id::I18next);
        assert_eq!(names(&identified), ["de/common.json", "en/common.json"]);
        assert_eq!(identified.files[0].locale.as_deref(), Some("de"));
    }

    #[test]
    fn several_namespaces_are_refused_and_named() {
        let tree = TempTree::new("identify-namespaces");
        tree.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#);
        tree.write("locales/en/common.json", r#"{"a":"Hello {{name}}"}"#);
        tree.write("locales/en/errors.json", "{}");
        tree.write("locales/de/common.json", r#"{"a":"Hallo {{name}}"}"#);
        tree.write("locales/de/errors.json", "{}");
        let error = refusal(&tree, "locales");
        assert!(error.contains("common.json"), "{error}");
        assert!(error.contains("errors.json"), "{error}");
    }

    /// In a monorepo the declaring manifest can sit well above the
    /// catalogues.
    #[test]
    fn a_manifest_several_directories_up_still_counts() {
        let tree = TempTree::new("identify-monorepo");
        tree.write("package.json", r#"{"dependencies":{"next-intl":"^4.0.0"}}"#);
        tree.write(
            "apps/web/messages/en.json",
            r#"{"a":"{count, plural, other {#}}"}"#,
        );
        tree.write(
            "apps/web/messages/es.json",
            r#"{"a":"{count, plural, other {#}}"}"#,
        );
        let identified = found(&tree, "apps/web/messages");
        assert_eq!(identified.library, Id::NextIntl);
        let manifest = identified
            .evidence
            .iter()
            .find(|signal| signal.class == Class::Manifest)
            .expect("a manifest signal");
        assert_eq!(manifest.detail, "next-intl in ../../../package.json");
    }

    /// A pubspec names its packages without a version, and the line scan
    /// still finds them.
    #[test]
    fn a_pubspec_declares_the_flutter_library() {
        let tree = TempTree::new("identify-pubspec");
        tree.write(
            "pubspec.yaml",
            "name: app\ndependencies:\n  flutter_localizations:\n    sdk: flutter\n  intl: ^0.19.0\n",
        );
        tree.write("lib/l10n/app_en.arb", r#"{"greeting":"Hi {name}"}"#);
        tree.write("lib/l10n/app_es.arb", r#"{"greeting":"Hola {name}"}"#);
        let identified = found(&tree, "lib/l10n");
        assert_eq!(identified.library, Id::FlutterArb);
        assert!(classes(&identified).contains(&Class::Manifest));
    }

    /// **The one version gate**, and it names a real break rather than
    /// an allowlist.
    #[test]
    fn a_breaking_major_is_refused_by_name() {
        let tree = TempTree::new("identify-old");
        tree.write("package.json", r#"{"dependencies":{"i18next":"^21.6.0"}}"#);
        tree.write("src/a.ts", "useTranslation()\n");
        tree.write("locales/en.json", r#"{"a":"Hello {{name}}"}"#);
        tree.write("locales/es.json", r#"{"a":"Hola {{name}}"}"#);
        let error = refusal(&tree, "locales");
        assert!(error.contains("21"), "{error}");
        assert!(error.contains("differently"), "{error}");
    }

    /// A newer major than anything here was written against is read, not
    /// refused: identification is corroborated by content, and refusing
    /// every unfamiliar version made the tool brittle.
    #[test]
    fn an_unfamiliar_major_is_read_and_reported() {
        let tree = TempTree::new("identify-new");
        tree.write("package.json", r#"{"dependencies":{"i18next":"^99.0.0"}}"#);
        tree.write("src/a.ts", "useTranslation()\n");
        tree.write("locales/en.json", r#"{"a":"Hello {{name}}"}"#);
        tree.write("locales/es.json", r#"{"a":"Hola {{name}}"}"#);
        let identified = found(&tree, "locales");
        assert_eq!(identified.version.as_deref(), Some("^99.0.0"));
    }

    /// The call-site scan must not wander into a dependency tree, where
    /// every library in the world is installed.
    #[test]
    fn the_call_site_scan_skips_the_directories_nobody_means() {
        let tree = TempTree::new("identify-skip");
        tree.write("package.json", r#"{"name":"x"}"#);
        tree.write("node_modules/next-intl/index.js", "useTranslations()\n");
        tree.write("dist/bundle.js", "useTranslation()\n");
        tree.write("strings/en.json", r#"{"a":"one"}"#);
        tree.write("strings/es.json", r#"{"a":"uno"}"#);
        let error = refusal(&tree, "strings");
        assert!(!error.contains("next-intl"), "{error}");
        assert!(!error.contains("i18next"), "{error}");
    }

    #[test]
    fn a_missing_input_is_refused_by_name() {
        let tree = TempTree::new("identify-missing");
        let error =
            identify(&[tree.path().join("nope")], Requested::Detect).expect_err("a refusal");
        assert!(error.contains("nope"), "{error}");
    }

    /// Reported paths are relative, so two machines produce the same
    /// report for the same repository.
    #[test]
    fn no_reported_path_is_absolute() {
        let tree = TempTree::new("identify-relative");
        tree.write("package.json", r#"{"dependencies":{"i18next":"^26.0.0"}}"#);
        tree.write("src/a.ts", "useTranslation()\n");
        tree.write("locales/en.json", r#"{"a":"Hello {{name}}"}"#);
        tree.write("locales/es.json", r#"{"a":"Hola {{name}}"}"#);
        let identified = found(&tree, "locales");
        for signal in &identified.evidence {
            assert!(!signal.detail.contains("/private/"), "{}", signal.detail);
            assert!(!signal.detail.starts_with('/'), "{}", signal.detail);
        }
    }
}
