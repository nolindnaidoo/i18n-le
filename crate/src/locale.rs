//! Which locale a catalogue is for, read from its file name.
//!
//! Several conventions have to work, and they disagree about where the
//! locale sits in the name:
//!
//! ```text
//! locales/en.json                 locales/pt-BR.json
//! l10n/bundle.l10n.json           l10n/bundle.l10n.pt-br.json
//! src/i18n/package.nls.json       src/i18n/package.nls.zh-cn.json
//! messages/messages.en.json       messages/messages.es.json
//! arb/app_en.arb                  arb/app_pt_BR.arb
//! ```
//!
//! **The locale is whatever the file names in a set do not share.** The
//! segments every name in the directory has in common are the set's
//! prefix — `bundle.l10n`, `package.nls`, `messages`, `app`, or nothing
//! at all — and what is left is the locale, or nothing, which is the
//! base catalogue three of those conventions write without a tag.
//!
//! Reading each name on its own cannot work: `nls` in `package.nls.json`
//! is three letters and would pass for a language tag, and `app_en.arb`
//! is either the locale `app-EN` or the prefix `app` and the locale
//! `en`. Reading the set together is what makes it decidable, and
//! anything left over that is not shaped like a tag is refused rather
//! than guessed at — getting this wrong makes every answer downstream
//! wrong.
//!
//! **Unless the identified library fixes the prefix**, which is what
//! `from_prefix` is for: with `package.nls` given rather than inferred,
//! one name is readable on its own, and a set no longer has to live in
//! one directory for its names to mean anything.

/// The canonical spelling of a language tag, or `None` if the text is
/// not shaped like one.
///
/// Only the subtags a catalogue file name ever carries: language,
/// optional script, optional region. No variants, no extensions — a
/// name carrying those is a question this refuses rather than answers.
pub(crate) fn canonicalise(tag: &str) -> Option<String> {
    let mut subtags = tag.split('-');

    let language = subtags.next()?;
    if !(2..=3).contains(&language.len()) || !language.bytes().all(|b| b.is_ascii_alphabetic()) {
        return None;
    }
    let mut canonical = language.to_ascii_lowercase();

    let mut next = subtags.next();
    if let Some(script) = next.filter(|part| is_script(part)) {
        let (first, rest) = script.split_at(1);
        canonical.push('-');
        canonical.push_str(&first.to_ascii_uppercase());
        canonical.push_str(&rest.to_ascii_lowercase());
        next = subtags.next();
    }
    if let Some(region) = next.filter(|part| is_region(part)) {
        canonical.push('-');
        canonical.push_str(&region.to_ascii_uppercase());
        next = subtags.next();
    }

    next.is_none().then_some(canonical)
}

fn is_script(part: &str) -> bool {
    part.len() == 4 && part.bytes().all(|b| b.is_ascii_alphabetic())
}

fn is_region(part: &str) -> bool {
    (part.len() == 2 && part.bytes().all(|b| b.is_ascii_alphabetic()))
        || (part.len() == 3 && part.bytes().all(|b| b.is_ascii_digit()))
}

/// The locale of every file in a set, in the order given.
///
/// `None` is the base catalogue — `bundle.l10n.json` beside
/// `bundle.l10n.de.json`, which is the English one in both VS Code
/// conventions.
///
/// Refuses when a leftover segment is not shaped like a language tag.
/// That refusal is what stops `i18n-le .` at a repository root from
/// comparing `package.json` against `tsconfig.json` and reporting a
/// hundred missing keys: neither `package` nor `tsconfig` is a tag, so
/// the question is named as the wrong one rather than answered.
pub(crate) fn locales_of(names: &[String]) -> Result<Vec<Option<String>>, String> {
    let stems: Vec<Vec<&str>> = names.iter().map(|name| stem(name)).collect();
    let shared = shared_prefix(&stems);

    names
        .iter()
        .zip(&stems)
        .map(|(name, segments)| locale_of(name, &segments[shared..]))
        .collect()
}

/// The leftover segments are one tag, whatever separator wrote them:
/// `pt-br`, `pt_BR` and `zh.CN` all reach here as the same two pieces.
fn locale_of(name: &str, remainder: &[&str]) -> Result<Option<String>, String> {
    if remainder.is_empty() {
        return Ok(None);
    }
    let tag = remainder.join("-");
    canonicalise(&tag).map(Some).ok_or_else(|| {
        format!("{name}: \"{tag}\" is not a language tag, so this is not a catalogue set")
    })
}

/// The locale of a file whose convention writes a **known** prefix —
/// `package.nls.de.json` under the VS Code profile, where the prefix is
/// `package.nls` and the leftover is the tag.
///
/// This is what lets a profile audit a set the shared-prefix rule
/// cannot: with the prefix given rather than inferred, the files no
/// longer have to sit in one directory for their names to be readable.
/// `package.nls.json` at a repository root and
/// `src/i18n/package.nls.de.json` are one set, and v0.1 could not say
/// so.
pub(crate) fn from_prefix(name: &str, prefix: &str) -> Result<Option<String>, String> {
    let stem = name
        .strip_suffix(".json")
        .or_else(|| name.strip_suffix(".arb"))
        .unwrap_or(name);
    let rest = stem
        .strip_prefix(prefix)
        .ok_or_else(|| format!("{name} is not a {prefix} catalogue"))?;
    if rest.is_empty() {
        return Ok(None);
    }
    let tag = rest.trim_start_matches(['.', '_']);
    canonicalise(tag)
        .map(Some)
        .ok_or_else(|| format!("{name}: \"{tag}\" is not a language tag"))
}

/// The segments of a file name, without its extension.
///
/// `_` separates as much as `.` does. Which of the two a project uses is
/// a house style, and both appear in the same conventions — `pt_BR.json`
/// beside `zh-CN.json` is an ordinary sight.
fn stem(name: &str) -> Vec<&str> {
    let stem = name
        .strip_suffix(".json")
        .or_else(|| name.strip_suffix(".arb"))
        .unwrap_or(name);
    stem.split(['.', '_']).collect()
}

/// How many leading segments every name in the set has in common.
///
/// A set of one shares all of its own segments, so a lone file is the
/// base catalogue — which is honest, because a lone file has nothing to
/// be compared against anyway.
fn shared_prefix(stems: &[Vec<&str>]) -> usize {
    let Some((first, rest)) = stems.split_first() else {
        return 0;
    };
    let mut shared = first.len();
    for other in rest {
        let common = first
            .iter()
            .zip(other.iter())
            .take_while(|(a, b)| a == b)
            .count();
        shared = shared.min(common);
    }
    shared
}

#[cfg(test)]
mod tests {
    use super::*;

    fn locales(names: &[&str]) -> Vec<Option<String>> {
        let owned: Vec<String> = names.iter().map(|name| (*name).to_string()).collect();
        locales_of(&owned).expect("a locale for every name")
    }

    #[test]
    fn a_language_tag_gets_one_spelling() {
        assert_eq!(canonicalise("en").as_deref(), Some("en"));
        assert_eq!(canonicalise("EN").as_deref(), Some("en"));
        assert_eq!(canonicalise("pt-br").as_deref(), Some("pt-BR"));
        assert_eq!(canonicalise("zh-cn").as_deref(), Some("zh-CN"));
        assert_eq!(canonicalise("zh-hans").as_deref(), Some("zh-Hans"));
        assert_eq!(canonicalise("zh-Hant-HK").as_deref(), Some("zh-Hant-HK"));
        assert_eq!(canonicalise("es-419").as_deref(), Some("es-419"));
        assert_eq!(canonicalise("fil").as_deref(), Some("fil"));
    }

    #[test]
    fn what_is_not_a_language_tag() {
        for text in [
            "l10n",
            "bundle",
            "package",
            "tsconfig",
            "e",
            "",
            "en-",
            "en-GB-x-private",
        ] {
            assert_eq!(canonicalise(text), None, "{text}");
        }
    }

    /// **Why the set decides and not the name.** `nls` is three letters
    /// and passes for a language tag on its own, so
    /// `package.nls.json` read in isolation is a catalogue for the
    /// language `nls`. Only the other names in the directory say it is a
    /// prefix.
    #[test]
    fn a_name_alone_is_not_enough_to_decide() {
        assert_eq!(canonicalise("nls").as_deref(), Some("nls"));
        assert_eq!(locales(&["package.nls.json"]), [None]);
        assert_eq!(
            locales(&["package.nls.json", "package.nls.de.json"]),
            [None, Some("de".to_string())]
        );
    }

    #[test]
    fn a_directory_of_bare_locales_reads_as_itself() {
        assert_eq!(
            locales(&["en.json", "pt-BR.json", "zh-CN.json"]),
            [
                Some("en".to_string()),
                Some("pt-BR".to_string()),
                Some("zh-CN".to_string())
            ]
        );
    }

    /// The VS Code runtime bundle: the untagged file is the English one.
    #[test]
    fn a_shared_prefix_is_not_part_of_the_locale() {
        assert_eq!(
            locales(&[
                "bundle.l10n.json",
                "bundle.l10n.pt-br.json",
                "bundle.l10n.zh-cn.json"
            ]),
            [None, Some("pt-BR".to_string()), Some("zh-CN".to_string())]
        );
    }

    #[test]
    fn the_manifest_convention_reads_the_same_way() {
        assert_eq!(
            locales(&["package.nls.json", "package.nls.zh-cn.json"]),
            [None, Some("zh-CN".to_string())]
        );
    }

    #[test]
    fn a_single_shared_segment_works_too() {
        assert_eq!(
            locales(&["messages.en.json", "messages.es.json"]),
            [Some("en".to_string()), Some("es".to_string())]
        );
    }

    /// `app_en.arb` is either the locale `app-EN` or the prefix `app`
    /// and the locale `en`. Only the set says which.
    #[test]
    fn an_underscore_separates_like_a_dot_does() {
        assert_eq!(
            locales(&["app_en.arb", "app_es.arb", "app_pt_BR.arb"]),
            [
                Some("en".to_string()),
                Some("es".to_string()),
                Some("pt-BR".to_string())
            ]
        );
        assert_eq!(
            locales(&["pt_BR.json", "zh_CN.json"]),
            [Some("pt-BR".to_string()), Some("zh-CN".to_string())]
        );
    }

    /// **The refusal that stops a repository root being audited as a
    /// catalogue set.**
    #[test]
    fn a_leftover_that_is_not_a_tag_is_refused_by_name() {
        let names = ["package.json".to_string(), "tsconfig.json".to_string()];
        let error = locales_of(&names).expect_err("a refusal");
        assert!(error.contains("package.json"), "{error}");
        assert!(error.contains("not a language tag"), "{error}");
    }

    #[test]
    fn a_leftover_of_several_segments_is_refused_rather_than_joined() {
        let names = ["en.json".to_string(), "fr.CA.extra.json".to_string()];
        assert!(locales_of(&names).is_err());
    }

    #[test]
    fn a_lone_file_is_the_base_catalogue() {
        assert_eq!(locales(&["bundle.l10n.json"]), [None]);
        assert_eq!(locales(&["en.json"]), [None]);
    }

    #[test]
    fn no_files_is_no_locales() {
        assert_eq!(locales_of(&[]).expect("nothing to refuse"), []);
    }

    /// **What a profile buys.** With the prefix given rather than
    /// inferred, one file is readable on its own — which is what lets a
    /// set live in two directories.
    #[test]
    fn a_known_prefix_reads_one_name_at_a_time() {
        assert_eq!(
            from_prefix("package.nls.json", "package.nls").expect("a locale"),
            None
        );
        assert_eq!(
            from_prefix("package.nls.zh-cn.json", "package.nls").expect("a locale"),
            Some("zh-CN".to_string())
        );
        assert_eq!(
            from_prefix("bundle.l10n.pt-br.json", "bundle.l10n").expect("a locale"),
            Some("pt-BR".to_string())
        );
    }

    #[test]
    fn a_known_prefix_still_refuses_a_leftover_that_is_not_a_tag() {
        assert!(from_prefix("package.nls.backup.json", "package.nls").is_err());
        assert!(from_prefix("something.else.json", "package.nls").is_err());
    }
}
