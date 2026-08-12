//! Reading the file set, once the library is known.
//!
//! `identify.rs` answers *which library*; this answers *which files, and
//! what locale is each*. The dependency runs one way and only one way:
//! the shapes below are read off the identified row, and nothing here
//! knows what a manifest or a call site is. `identify.rs` calls into it
//! twice — once to read the set for real, and once while gathering
//! evidence, because "these files read cleanly under this layout" is
//! itself a class of evidence.
//!
//! Three shapes, and what separates them is where the locale lives:
//!
//! - **`shared`** — the locale is whatever the names do *not* share, so
//!   the names are only readable together and the set has to live in one
//!   directory.
//! - **`fixed`** — the library supplies the prefix, so one name is
//!   readable on its own, so the set may span two directories. That is
//!   how every VS Code extension is written, and it is the layout
//!   nothing before v0.3 could audit at all.
//! - **`namespaced`** — the parent directory is the locale. One
//!   namespace is one set; auditing several together would compare
//!   `common.json` against `errors.json`.

use std::path::{Path, PathBuf};

use crate::library::{Id, Layout, Shape};
use crate::locale;

/// How far up this tool ever looks — for a manifest, a config, or a
/// prefixed set's base file. Deep enough for a monorepo package nested a
/// few levels down, shallow enough that a stray `package.json` near `/`
/// is never the answer.
pub(crate) const ANCESTORS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Located {
    pub(crate) path: PathBuf,
    /// The name this file is reported under.
    pub(crate) name: String,
    pub(crate) locale: Option<String>,
}

/// A set of files read under one layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Reading {
    pub(crate) files: Vec<Located>,
    pub(crate) shape: Shape,
    pub(crate) keys_are_source: bool,
}

/// The first of the library's layouts that reads the files in `inputs`.
pub(crate) fn read(inputs: &[PathBuf], anchor: &Path, library: Id) -> Result<Reading, String> {
    let row = library.library();
    let mut refusal = None;
    for layout in row.layouts {
        match read_layout(inputs, anchor, layout) {
            Ok(Some(reading)) => return Ok(reading),
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
        return Ok(Reading {
            files: Vec::new(),
            shape: layout.shape,
            keys_are_source: layout.keys_are_source,
        });
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

pub(crate) fn read_layout(
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

pub(crate) fn describe_shape(shape: Shape) -> String {
    match shape {
        Shape::Shared { extension } => format!("a directory of <locale>.{extension}"),
        Shape::Fixed { prefix, extension } => format!("{prefix}.<locale>.{extension}"),
        Shape::Namespaced { extension } => format!("<locale>/<namespace>.{extension}"),
    }
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

    let Some(directories) = locale_directories(root) else {
        return Ok(None);
    };

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

    Ok(Some(Reading {
        files: files.iter().map(|path| in_locale_directory(path)).collect(),
        shape: layout.shape,
        keys_are_source: layout.keys_are_source,
    }))
}

/// The subdirectories of `root`, when **every** one of them is named for
/// a locale. One that is not means this is not a namespaced set, which
/// is what stops an ordinary source tree being read as one.
fn locale_directories(root: &Path) -> Option<Vec<PathBuf>> {
    let mut directories = Vec::new();
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        locale::canonicalise(&entry.file_name().to_string_lossy())?;
        directories.push(entry.path());
    }
    if directories.is_empty() {
        return None;
    }
    directories.sort();
    Some(directories)
}

/// A namespaced file, named `<locale>/<file>` because the base names
/// collide across locales.
///
/// The tag is known to be a language tag: `locale_directories` refused
/// the whole layout otherwise.
fn in_locale_directory(path: &Path) -> Located {
    let tag = path
        .parent()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    Located {
        name: format!("{tag}/{}", name_of(path)),
        locale: locale::canonicalise(&tag),
        path: path.to_path_buf(),
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

/// The `.json` and `.arb` files directly inside one directory. Never a
/// recursive walk: a catalogue directory is flat by every convention
/// this reads, and descending would sweep up `package.json` and force
/// the tool to guess what a catalogue is.
pub(crate) fn catalogues_in(directory: &Path) -> Vec<PathBuf> {
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

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|found| found.to_str())
        .is_some_and(|found| found == extension)
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
fn name_of(path: &Path) -> String {
    path.file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned()
}

/// One catalogue's text, or why it could not be had.
///
/// **The reason never names the path.** It becomes a `skipped`
/// diagnostic, whose `file` already carries the name the report uses,
/// and SPEC.md promises every reported path is relative to the
/// catalogues: an absolute one here made two machines auditing the same
/// repository produce different reports for the same defect.
pub(crate) fn read_text(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    String::from_utf8(bytes).map_err(|_| "not UTF-8 text".to_string())
}
