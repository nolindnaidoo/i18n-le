//! What each i18n library **is**, as data.
//!
//! This table is the front door's vocabulary and the whole audit's
//! authority. Identification decides which row applies; everything
//! downstream — the message grammar, the plural model, which keys are
//! metadata, how the files on disk are laid out and where a locale's
//! name lives — is read off that row. **Nothing downstream guesses.**
//!
//! That is the difference between this and what came before. v0.1 and
//! v0.2 classified the placeholder grammar by scanning message content,
//! and a profile was bolted on top of that classifier. It was wrong in
//! the way heuristics are wrong: confidently. Twenty-five real
//! catalogues were refused over `$t(metrics.window.{{timeframe}})` —
//! not an unknown construct at all, but i18next nesting, specified by a
//! library the project declares in its own `package.json`.
//!
//! **Adding a library is meant to be a row here**, not a change
//! anywhere else. A genuinely new syntax needs one new `Mark` and its
//! predicate in `message.rs`; everything else — packages, config files,
//! call sites, layouts, grammar, plurals, metadata — is data.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Id {
    I18next,
    NextIntl,
    VscodeL10n,
    FlutterArb,
}

/// Where a manifest dependency is declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Manifest {
    Npm,
    Pubspec,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Package {
    pub(crate) name: &'static str,
    pub(crate) manifest: Manifest,
    /// A major below which this library's model is **known** to be
    /// different, so reading the catalogues as this library would be
    /// wrong rather than merely unverified.
    ///
    /// Only a known break belongs here. v0.2 refused every major outside
    /// an allowlist, which turned "we have not tested this" into "we
    /// will not answer" and made the tool brittle against libraries that
    /// keep shipping. Identification is corroborated by content now, so
    /// an unfamiliar major is reported and read; a *breaking* one is
    /// still refused.
    pub(crate) breaking_below: Option<u64>,
}

/// A config file a library writes: the name without its extension, and
/// the extensions it is actually written in.
///
/// **The extensions are the point.** Matching a stem alone made
/// `l10n.json` — somebody's translation bundle — and `l10n.ts` —
/// somebody's module — each cast a vote for Flutter, whose config is
/// `l10n.yaml` and nothing else. A class of evidence that fires on a
/// file nobody would call a config dilutes the corroboration rule it
/// feeds.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Config {
    pub(crate) stem: &'static str,
    pub(crate) extensions: &'static [&'static str],
}

impl Config {
    pub(crate) fn matches(&self, stem: &str, extension: &str) -> bool {
        self.stem == stem && self.extensions.contains(&extension)
    }
}

/// What a config in the JavaScript ecosystem is written in. One list,
/// because three rows want the same one and a fourth spelling of it
/// would drift from the other three.
const JS_CONFIG: &[&str] = &["js", "cjs", "mjs", "ts", "mts", "cts", "json"];

/// How the files of a set are named and where they sit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "shape")]
pub(crate) enum Shape {
    /// The locale is whatever the names in the directory do **not**
    /// share: `en.json`, `app_en.arb`, `messages.es.json`.
    ///
    /// Weak as evidence on its own — almost any locale directory looks
    /// like this — so it only counts when the directory is one the
    /// library actually writes.
    Shared { extension: &'static str },
    /// A prefix the library fixes, so one name is readable alone. That
    /// is what lets the set span two directories, which is how VS Code
    /// writes the manifest bundle and what nothing before this could
    /// audit.
    Fixed {
        prefix: &'static str,
        extension: &'static str,
    },
    /// `<locale>/<namespace>.json`.
    Namespaced { extension: &'static str },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Layout {
    pub(crate) shape: Shape,
    /// Directory names this library writes. Empty means any.
    pub(crate) directories: &'static [&'static str],
    /// The key *is* the English string, as in `bundle.l10n.json`'s
    /// `"Save file": "Save file"`. A property of the layout, not the
    /// library: VS Code's other layout keys are dotted identifiers.
    pub(crate) keys_are_source: bool,
}

/// How placeholders are written. Supplied, never inferred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Interpolation {
    /// `{{name}}`
    DoubleBrace,
    /// `{name}`
    SingleBrace,
    /// `{0}`
    Positional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Grammar {
    pub(crate) interpolation: Interpolation,
    /// `{count, plural, ...}` is native here.
    pub(crate) icu: bool,
    /// `$t(other.key)` is a key reference here.
    pub(crate) nesting: bool,
    /// `{{ name }}` and `{{name}}` are the same variable.
    pub(crate) trims: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Plurals {
    /// `key_one`, `key_other` — separate keys sharing a base.
    KeySuffix,
    /// Inside the message.
    Icu,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Metadata {
    /// `@@locale` and friends, which no library treats as a message.
    DoubleAt,
    /// ARB: `@@locale` **and** a `@key` object beside every `key`.
    Arb,
}

/// A shape in catalogue content that points at one library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Signature {
    pub(crate) mark: Mark,
    pub(crate) strength: Strength,
}

/// How much one syntactic mark is worth as evidence.
///
/// The distinction earns its keep on `{name}`: next-intl reads it as an
/// interpolation, but it is also ordinary prose in every other library's
/// catalogues, so it must not vote. `{count, plural,}` is another
/// matter — nobody writes that by accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Strength {
    /// Identifies on its own, with no corroboration. Reserved for syntax
    /// no other library writes — it is what makes a translations-only
    /// repository, with no manifest and no call sites, still auditable.
    Decisive,
    /// Counts as this library's content evidence.
    Strong,
    /// Consistent with the library, but too common to vote. Recorded so
    /// a refusal can say what it saw; never enough to identify.
    Weak,
}

/// A syntactic mark the content scan looks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Mark {
    /// `{{var}}`
    DoubleBrace,
    /// `{x, plural, ...}`
    IcuArgument,
    /// `{0}`
    Positional,
    /// `{name}`
    SingleBrace,
    /// `$t(key)`
    DollarT,
    /// `key_one` / `key_other` beside a `key`-less base.
    PluralKeySuffix,
    /// `@@locale` with `@key` siblings.
    ArbMetadata,
    /// The key is an English sentence rather than an identifier.
    SentenceKeys,
}

impl Mark {
    /// The name a refusal and the report both call this mark.
    ///
    /// Held equal to the serde spelling by a test, because
    /// `fixtures/detection.json` names marks the serde way and an
    /// evidence detail names them this way — two spellings of one thing
    /// that must not drift.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Mark::DoubleBrace => "double-brace",
            Mark::IcuArgument => "icu-argument",
            Mark::Positional => "positional",
            Mark::SingleBrace => "single-brace",
            Mark::DollarT => "dollar-t",
            Mark::PluralKeySuffix => "plural-key-suffix",
            Mark::ArbMetadata => "arb-metadata",
            Mark::SentenceKeys => "sentence-keys",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Library {
    pub(crate) id: Id,
    pub(crate) packages: &'static [Package],
    /// Config files this library writes, by stem and extension.
    pub(crate) configs: &'static [Config],
    /// Fixed substrings that appear at a call site.
    ///
    /// **Identification only.** Source files are read to answer "which
    /// library is this", never to produce a finding — see SPEC.md,
    /// "The call-site boundary".
    pub(crate) calls: &'static [&'static str],
    pub(crate) layouts: &'static [Layout],
    pub(crate) signatures: &'static [Signature],
    pub(crate) grammar: Grammar,
    pub(crate) plurals: Plurals,
    pub(crate) metadata: Metadata,
}

impl Id {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Id::I18next => "i18next",
            Id::NextIntl => "next-intl",
            Id::VscodeL10n => "vscode-l10n",
            Id::FlutterArb => "flutter-arb",
        }
    }

    pub(crate) fn parse(text: &str) -> Option<Self> {
        ALL.iter()
            .map(|library| library.id)
            .find(|id| id.as_str() == text)
    }

    pub(crate) fn library(self) -> &'static Library {
        ALL.iter()
            .find(|library| library.id == self)
            .expect("every id has a row")
    }
}

/// Every library name, for a refusal that has to list them.
pub(crate) fn names() -> Vec<&'static str> {
    ALL.iter().map(|library| library.id.as_str()).collect()
}

pub(crate) fn all() -> &'static [Library] {
    ALL
}

const ALL: &[Library] = &[
    Library {
        id: Id::I18next,
        packages: &[
            Package {
                name: "i18next",
                manifest: Manifest::Npm,
                // v21 and v22 wrote plural keys as `key_plural`; v23
                // moved to the CLDR categories this reads. Reading a v22
                // catalogue as v23 would fold the wrong keys.
                breaking_below: Some(23),
            },
            Package {
                name: "react-i18next",
                manifest: Manifest::Npm,
                breaking_below: None,
            },
            Package {
                name: "i18next-vue",
                manifest: Manifest::Npm,
                breaking_below: None,
            },
        ],
        configs: &[
            Config {
                stem: "i18next-parser.config",
                extensions: JS_CONFIG,
            },
            Config {
                stem: "next-i18next.config",
                extensions: JS_CONFIG,
            },
        ],
        // `$t(` is deliberately absent: it appears in Vue templates and
        // in plenty of unrelated code, and it is already a content mark
        // where it is trustworthy — inside a catalogue.
        calls: &["useTranslation(", "i18next.t("],
        layouts: &[
            Layout {
                shape: Shape::Namespaced { extension: "json" },
                directories: &[],
                keys_are_source: false,
            },
            Layout {
                shape: Shape::Shared { extension: "json" },
                directories: &["locales", "translations", "lang"],
                keys_are_source: false,
            },
        ],
        signatures: &[
            Signature {
                mark: Mark::DoubleBrace,
                strength: Strength::Strong,
            },
            Signature {
                mark: Mark::DollarT,
                strength: Strength::Strong,
            },
            Signature {
                mark: Mark::PluralKeySuffix,
                strength: Strength::Strong,
            },
        ],
        grammar: Grammar {
            interpolation: Interpolation::DoubleBrace,
            icu: false,
            nesting: true,
            trims: true,
        },
        plurals: Plurals::KeySuffix,
        metadata: Metadata::DoubleAt,
    },
    Library {
        id: Id::NextIntl,
        packages: &[Package {
            name: "next-intl",
            manifest: Manifest::Npm,
            breaking_below: None,
        }],
        configs: &[Config {
            stem: "next-intl.config",
            extensions: JS_CONFIG,
        }],
        calls: &["useTranslations(", "getTranslations(", "next-intl"],
        layouts: &[Layout {
            shape: Shape::Shared { extension: "json" },
            directories: &["messages"],
            keys_are_source: false,
        }],
        signatures: &[
            Signature {
                mark: Mark::IcuArgument,
                strength: Strength::Strong,
            },
            Signature {
                // Every library's catalogues carry a `{word}` in prose
                // somewhere. It is consistent with next-intl and proves
                // nothing.
                mark: Mark::SingleBrace,
                strength: Strength::Weak,
            },
        ],
        grammar: Grammar {
            interpolation: Interpolation::SingleBrace,
            icu: true,
            nesting: false,
            trims: true,
        },
        plurals: Plurals::Icu,
        metadata: Metadata::DoubleAt,
    },
    Library {
        id: Id::VscodeL10n,
        packages: &[Package {
            name: "@vscode/l10n",
            manifest: Manifest::Npm,
            breaking_below: None,
        }],
        configs: &[],
        calls: &["vscode.l10n.t(", "l10n.t("],
        layouts: &[
            Layout {
                shape: Shape::Fixed {
                    prefix: "bundle.l10n",
                    extension: "json",
                },
                directories: &[],
                keys_are_source: true,
            },
            Layout {
                shape: Shape::Fixed {
                    prefix: "package.nls",
                    extension: "json",
                },
                directories: &[],
                keys_are_source: false,
            },
        ],
        signatures: &[
            Signature {
                mark: Mark::Positional,
                strength: Strength::Weak,
            },
            Signature {
                mark: Mark::SentenceKeys,
                strength: Strength::Strong,
            },
        ],
        grammar: Grammar {
            interpolation: Interpolation::Positional,
            icu: false,
            nesting: false,
            trims: false,
        },
        plurals: Plurals::None,
        metadata: Metadata::DoubleAt,
    },
    Library {
        id: Id::FlutterArb,
        packages: &[
            Package {
                name: "flutter_localizations",
                manifest: Manifest::Pubspec,
                breaking_below: None,
            },
            Package {
                name: "intl",
                manifest: Manifest::Pubspec,
                breaking_below: None,
            },
        ],
        // `l10n.yaml` and nothing else. `l10n.json` is a translation
        // bundle and `l10n.ts` is a module; neither is Flutter's config.
        configs: &[Config {
            stem: "l10n",
            extensions: &["yaml", "yml"],
        }],
        calls: &["AppLocalizations.of("],
        layouts: &[Layout {
            shape: Shape::Shared { extension: "arb" },
            // Empty rather than `l10n`, because the extension is already
            // the distinctive part and Flutter projects put ARB files
            // wherever `l10n.yaml` points them.
            directories: &[],
            keys_are_source: false,
        }],
        signatures: &[
            Signature {
                // `@@locale` beside `@key` objects is ARB and nothing
                // else, which is what makes a bare directory of `.arb`
                // files identifiable with no project around it.
                mark: Mark::ArbMetadata,
                strength: Strength::Decisive,
            },
            Signature {
                mark: Mark::IcuArgument,
                strength: Strength::Weak,
            },
        ],
        grammar: Grammar {
            interpolation: Interpolation::SingleBrace,
            icu: true,
            nesting: false,
            trims: true,
        },
        plurals: Plurals::Icu,
        metadata: Metadata::Arb,
    },
];

impl Library {
    /// Whether a key is metadata rather than a message.
    ///
    /// Applied to the **first** dotted segment, so an ARB `@greeting`
    /// object takes its children with it.
    pub(crate) fn is_metadata(&self, key: &str) -> bool {
        let head = key.split('.').next().unwrap_or(key);
        match self.metadata {
            Metadata::DoubleAt => head.starts_with("@@"),
            Metadata::Arb => head.starts_with('@'),
        }
    }

    pub(crate) fn package(&self, name: &str, manifest: Manifest) -> Option<&'static Package> {
        self.packages
            .iter()
            .find(|package| package.name == name && package.manifest == manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_library_round_trips_through_its_name() {
        for library in all() {
            let name = library.id.as_str();
            assert_eq!(Id::parse(name), Some(library.id), "{name}");
            assert_eq!(Id::parse(name).expect("an id").library().id, library.id);
        }
        assert_eq!(Id::parse("gettext"), None);
    }

    /// The four the owner actually ships. A row disappearing is a
    /// regression nobody would notice from a green suite.
    #[test]
    fn the_four_that_must_work_are_all_here() {
        let ids: Vec<&str> = names();
        for expected in ["i18next", "next-intl", "vscode-l10n", "flutter-arb"] {
            assert!(ids.contains(&expected), "{expected} is missing");
        }
    }

    /// Identification needs two agreeing classes or one decisive
    /// signature, so a library that can offer neither could never be
    /// identified at all.
    #[test]
    fn every_library_can_reach_an_identification() {
        for library in all() {
            let voting = library
                .signatures
                .iter()
                .any(|s| s.strength != Strength::Weak);
            let classes = usize::from(!library.packages.is_empty())
                + usize::from(!library.configs.is_empty())
                + usize::from(!library.layouts.is_empty())
                + usize::from(voting)
                + usize::from(!library.calls.is_empty());
            assert!(classes >= 2, "{:?} can never be identified", library.id);
        }
    }

    /// `Id::library()` says "every id has a row" and would panic if one
    /// did not. The `match` is what keeps that honest: adding a variant
    /// makes it non-exhaustive and fails the build, rather than leaving
    /// the panic to be found at runtime by whoever ran the tool.
    #[test]
    fn every_id_variant_has_a_row() {
        let every = [Id::I18next, Id::NextIntl, Id::VscodeL10n, Id::FlutterArb];
        for id in every {
            match id {
                Id::I18next | Id::NextIntl | Id::VscodeL10n | Id::FlutterArb => {}
            }
            assert_eq!(id.library().id, id, "{id:?} has no row");
        }
        assert_eq!(all().len(), every.len(), "a row nothing names");
    }

    /// `Mark::as_str` and the serde spelling are two names for one thing:
    /// an evidence detail is written with the first and
    /// `fixtures/detection.json` names marks with the second. If they
    /// drift, a corpus case passes against a mark no refusal ever says.
    #[test]
    fn every_mark_spells_itself_the_way_serde_does() {
        for mark in [
            Mark::DoubleBrace,
            Mark::IcuArgument,
            Mark::Positional,
            Mark::SingleBrace,
            Mark::DollarT,
            Mark::PluralKeySuffix,
            Mark::ArbMetadata,
            Mark::SentenceKeys,
        ] {
            match mark {
                Mark::DoubleBrace
                | Mark::IcuArgument
                | Mark::Positional
                | Mark::SingleBrace
                | Mark::DollarT
                | Mark::PluralKeySuffix
                | Mark::ArbMetadata
                | Mark::SentenceKeys => {}
            }
            assert_eq!(
                serde_json::to_value(mark).expect("a mark serializes"),
                serde_json::Value::String(mark.as_str().to_string()),
                "{mark:?}"
            );
        }
    }

    /// A decisive signature is a claim that no other library writes that
    /// syntax. If two rows claim the same decisive mark, one of them is
    /// wrong.
    #[test]
    fn no_two_libraries_claim_the_same_decisive_signature() {
        let mut seen: Vec<Mark> = Vec::new();
        for library in all() {
            for signature in library
                .signatures
                .iter()
                .filter(|s| s.strength == Strength::Decisive)
            {
                assert!(
                    !seen.contains(&signature.mark),
                    "{:?} is claimed twice",
                    signature.mark
                );
                seen.push(signature.mark);
            }
        }
    }

    #[test]
    fn arb_metadata_takes_its_children_with_it() {
        let arb = Id::FlutterArb.library();
        assert!(arb.is_metadata("@@locale"));
        assert!(arb.is_metadata("@greeting"));
        assert!(arb.is_metadata("@greeting.description"));
        assert!(!arb.is_metadata("greeting"));

        let other = Id::I18next.library();
        assert!(other.is_metadata("@@locale"));
        assert!(!other.is_metadata("@greeting"), "somebody else's naming");
    }

    #[test]
    fn the_two_vscode_layouts_disagree_about_what_a_key_is() {
        let vscode = Id::VscodeL10n.library();
        let bundle = vscode
            .layouts
            .iter()
            .find(|layout| {
                matches!(
                    layout.shape,
                    Shape::Fixed {
                        prefix: "bundle.l10n",
                        ..
                    }
                )
            })
            .expect("the bundle layout");
        let nls = vscode
            .layouts
            .iter()
            .find(|layout| {
                matches!(
                    layout.shape,
                    Shape::Fixed {
                        prefix: "package.nls",
                        ..
                    }
                )
            })
            .expect("the manifest layout");
        assert!(bundle.keys_are_source);
        assert!(!nls.keys_are_source);
    }

    /// The only version gate left, and it names a real break rather than
    /// an allowlist.
    #[test]
    fn only_a_known_break_carries_a_version_floor() {
        let i18next = Id::I18next.library();
        assert_eq!(
            i18next
                .package("i18next", Manifest::Npm)
                .expect("the package")
                .breaking_below,
            Some(23)
        );
        assert_eq!(
            i18next
                .package("react-i18next", Manifest::Npm)
                .expect("the package")
                .breaking_below,
            None
        );
    }
}
