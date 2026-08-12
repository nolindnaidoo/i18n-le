//! Reading one message under a **known** grammar.
//!
//! There is no classification here and no `Option<Grammar>`. By the time
//! anything in this module runs, identification has already said which
//! library wrote the file, so the parser is told what the braces mean
//! instead of guessing from what it finds between them.
//!
//! That is the whole reason the rewrite exists. `{name}` is valid ICU
//! *and* literal text in i18next *and* literal text in a VS Code bundle;
//! `{{ name }}` is an i18next variable *and* an ICU literal brace around
//! a word. No amount of looking at the bytes settles that. Being told
//! does.
//!
//! Two kinds of thing come back besides the placeholders:
//!
//! - **`foreign`** — a construct from another convention, which the
//!   loader will render to a user verbatim. A `convention-mismatch`
//!   finding.
//! - **`others`** — an interpolation in a style this library does not
//!   read, kept separately because on its own it is usually prose. It
//!   only becomes a `placeholder-style-mismatch` when the source had a
//!   real placeholder at the same key.
//!
//! The second half of the module is the **identification** scan:
//! grammar-free predicates that answer "does this look like library X",
//! used by `identify.rs` and by nothing else.

use crate::library::{Grammar, Interpolation, Mark};

/// The CLDR categories a key-suffix plural model writes.
pub(crate) const PLURAL_SUFFIXES: [&str; 6] = ["_zero", "_one", "_two", "_few", "_many", "_other"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Construct {
    /// `{count, plural, ...}` where ICU is not native.
    IcuArgument,
    /// `{ $variable }`.
    Fluent,
    /// `%s`, `%d`, `%1$s`.
    Printf,
    /// `${...}`.
    TemplateLiteral,
    /// `$t(other.key)` where nesting is not native.
    Nesting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Foreign {
    pub(crate) construct: Construct,
    /// Byte offset into the message. A fact about the text, not the
    /// text.
    pub(crate) offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Parsed {
    pub(crate) tokens: Vec<Token>,
    pub(crate) foreign: Vec<Foreign>,
    /// Interpolations written in a style this library does not read.
    pub(crate) others: Vec<Interpolation>,
}

/// Every placeholder in one message, and everything in it that is not
/// one.
///
/// Scans bytes rather than chars so an offset is a byte offset and a
/// multi-byte character cannot be mistaken for a delimiter: no
/// continuation byte is ever `{`, `%` or `$`.
pub(crate) fn parse(grammar: Grammar, text: &str) -> Parsed {
    let bytes = text.as_bytes();
    let mut parsed = Parsed::default();
    let mut index = 0;

    while index < bytes.len() {
        let advanced = match bytes[index] {
            b'{' if bytes.get(index + 1) == Some(&b'{') => {
                double_brace(bytes, index, grammar, &mut parsed)
            }
            b'{' => single_brace(bytes, index, grammar, &mut parsed),
            // An ICU literal `}`. Stepping over it as a pair stops the
            // second half being read as the start of anything.
            b'}' if bytes.get(index + 1) == Some(&b'}') => 2,
            // A literal percent. Without this, "100%% off" is judged for
            // nothing.
            b'%' if bytes.get(index + 1) == Some(&b'%') => 2,
            b'%' => percent(bytes, index, &mut parsed),
            b'$' => dollar(bytes, index, grammar, &mut parsed),
            _ => 1,
        };
        index += advanced;
    }
    parsed
}

/// `{{name}}` — a variable where that is the grammar, and ICU's literal
/// `{` everywhere else.
///
/// The second half matters more than the first: reading `{{` as a
/// placeholder manufactures a finding in every ICU file that escapes a
/// brace.
fn double_brace(bytes: &[u8], index: usize, grammar: Grammar, parsed: &mut Parsed) -> usize {
    if grammar.interpolation != Interpolation::DoubleBrace {
        return 2;
    }
    let start = index + 2;
    if let Some(end) = identifier_end(bytes, start)
        && bytes.get(end) == Some(&b'}')
        && bytes.get(end + 1) == Some(&b'}')
    {
        parsed.tokens.push(Token {
            name: name(bytes, start, end),
        });
        return end + 2 - index;
    }
    if !grammar.trims {
        return 2;
    }
    // `{{ name }}`. i18next trims, so this is the variable `name`.
    let opened = skip_space(bytes, start);
    let Some(end) = identifier_end(bytes, opened) else {
        return 2;
    };
    let closing = skip_space(bytes, end);
    if !(bytes.get(closing) == Some(&b'}') && bytes.get(closing + 1) == Some(&b'}')) {
        return 2;
    }
    parsed.tokens.push(Token {
        name: name(bytes, opened, end),
    });
    closing + 2 - index
}

fn single_brace(bytes: &[u8], index: usize, grammar: Grammar, parsed: &mut Parsed) -> usize {
    let opened = index + 1;
    let start = if grammar.trims {
        skip_space(bytes, opened)
    } else {
        opened
    };
    if bytes.get(start) == Some(&b'$') {
        // No library here writes Fluent, so this is foreign under all of
        // them.
        parsed.foreign.push(Foreign {
            construct: Construct::Fluent,
            offset: index,
        });
        return 1;
    }

    let Some(end) = identifier_end(bytes, start) else {
        return 1;
    };
    let closing = if grammar.trims {
        skip_space(bytes, end)
    } else {
        end
    };

    if bytes.get(closing) == Some(&b',') {
        return icu_argument(bytes, index, start, end, grammar, parsed);
    }
    if bytes.get(closing) != Some(&b'}') {
        return 1;
    }

    let digits = bytes[start..end].iter().all(u8::is_ascii_digit);
    let found = if digits {
        Interpolation::Positional
    } else {
        Interpolation::SingleBrace
    };
    // A `match` rather than a comparison, so a library written in a
    // fourth interpolation style fails the build here instead of
    // silently reading none of its placeholders.
    let reads = match grammar.interpolation {
        // ICU reads `{0}` and `{name}` alike.
        Interpolation::SingleBrace => true,
        Interpolation::Positional => found == Interpolation::Positional,
        Interpolation::DoubleBrace => false,
    };
    if reads {
        parsed.tokens.push(Token {
            name: name(bytes, start, end),
        });
        return closing + 1 - index;
    }

    // Not this library's interpolation. On its own that is prose — the
    // loader renders `{name}` verbatim — so it is recorded rather than
    // reported, and only becomes a finding when the source had a real
    // placeholder at the same key.
    if !parsed.others.contains(&found) {
        parsed.others.push(found);
    }
    closing + 1 - index
}

/// `{count, plural, one {# item} other {# items}}`.
///
/// Where ICU is native this is one token named for its argument, so a
/// translation that drops the whole construct is still caught. The
/// categories inside are not checked — see SPEC.md, "Plurals". Where ICU
/// is not native the braces reach the user verbatim, which is a defect.
fn icu_argument(
    bytes: &[u8],
    index: usize,
    start: usize,
    end: usize,
    grammar: Grammar,
    parsed: &mut Parsed,
) -> usize {
    if !grammar.icu {
        parsed.foreign.push(Foreign {
            construct: Construct::IcuArgument,
            offset: index,
        });
        // Stepping over the whole construct stops its sub-messages being
        // read as placeholders of their own.
        return balanced(bytes, index).unwrap_or(1);
    }
    parsed.tokens.push(Token {
        name: name(bytes, start, end),
    });
    balanced(bytes, index).unwrap_or(1)
}

/// How far past `index` the brace opened there closes, or `None` when it
/// never does.
fn balanced(bytes: &[u8], index: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, byte) in bytes.iter().enumerate().skip(index) {
        match byte {
            b'{' => depth += 1,
            // `checked_sub`, not `-=`: every caller passes the offset of
            // a `{`, so the count cannot go negative — but the crate
            // builds with overflow checks on precisely so a wrong number
            // crashes rather than being reported, and a lexer is not the
            // place to leave a panic resting on a caller's manners.
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(offset + 1 - index);
                }
            }
            _ => {}
        }
    }
    None
}

/// `%s`, `%d`, `%1$s` — a conversion straight after the `%`, or a
/// positional argument before it. Anything else is prose.
///
/// **printf's flags, width and precision are deliberately not read.**
/// They match ordinary text in several languages: Hungarian writes
/// "90%-os", which is `%` + the left-justify flag + the octal
/// conversion. Honouring that judged a whole catalogue over a
/// percentage, and it was found by running the binary rather than the
/// tests.
fn percent(bytes: &[u8], index: usize, parsed: &mut Parsed) -> usize {
    let mut cursor = index + 1;
    let digits = cursor;
    while bytes.get(cursor).is_some_and(u8::is_ascii_digit) {
        cursor += 1;
    }
    if cursor > digits {
        if bytes.get(cursor) != Some(&b'$') {
            return 1;
        }
        cursor += 1;
    }
    if !bytes
        .get(cursor)
        .is_some_and(|byte| b"sdiufFgGeExXoc".contains(byte))
    {
        return 1;
    }
    parsed.foreign.push(Foreign {
        construct: Construct::Printf,
        offset: index,
    });
    1
}

fn dollar(bytes: &[u8], index: usize, grammar: Grammar, parsed: &mut Parsed) -> usize {
    if bytes.get(index + 1) == Some(&b'{') {
        parsed.foreign.push(Foreign {
            construct: Construct::TemplateLiteral,
            offset: index,
        });
        return 2;
    }
    if bytes.get(index + 1) == Some(&b't') && bytes.get(index + 2) == Some(&b'(') {
        // **The construct that made the guessing core refuse
        // twenty-five real catalogues.** Where nesting is native it is a
        // key reference, and the scan continues past it so an
        // interpolation inside the referenced key still counts.
        if !grammar.nesting {
            parsed.foreign.push(Foreign {
                construct: Construct::Nesting,
                offset: index,
            });
        }
        return 3;
    }
    1
}

/// `.`, `-` and `_` are inside a name: `{{user.name}}` and
/// `{{first-name}}` are both ordinary.
fn identifier_end(bytes: &[u8], start: usize) -> Option<usize> {
    let first = bytes.get(start)?;
    if !(first.is_ascii_alphanumeric() || *first == b'_') {
        return None;
    }
    let mut end = start;
    while bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        end += 1;
    }
    Some(end)
}

fn skip_space(bytes: &[u8], mut index: usize) -> usize {
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    index
}

fn name(bytes: &[u8], start: usize, end: usize) -> String {
    String::from_utf8_lossy(&bytes[start..end]).into_owned()
}

/// A key's plural base, where the library writes plurals as suffixes.
///
/// Polish needs `_few`, Japanese needs only `_other`, and English needs
/// `_one` and `_other`. Requiring the source's exact variant set of
/// every locale would report a finding on every correctly translated
/// plural in the project. A base key that genuinely ends `_one` is
/// folded too; that only ever relaxes a check.
pub(crate) fn plural_base(key: &str) -> &str {
    PLURAL_SUFFIXES
        .iter()
        .find_map(|suffix| key.strip_suffix(suffix))
        .unwrap_or(key)
}

// ---------------------------------------------------------------------
// Identification: grammar-free predicates, used only to answer "which
// library wrote this". Nothing here reads a message in order to report
// something about it.
// ---------------------------------------------------------------------

/// The syntactic marks a message carries, read with no grammar in hand.
///
/// Deliberately generous where the audit is strict: a mark is a vote
/// towards a library, and a wrong vote is outvoted by the other evidence
/// classes. A wrong *finding* has no such safety net, which is why the
/// two scans are separate functions rather than one with a flag.
pub(crate) fn marks(text: &str) -> Vec<Mark> {
    let bytes = text.as_bytes();
    let mut found: Vec<Mark> = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        let (mark, advanced) = mark_at(bytes, index);
        if let Some(mark) = mark
            && !found.contains(&mark)
        {
            found.push(mark);
        }
        index += advanced;
    }
    found
}

/// The mark starting at `index`, if any, and how far past it to carry on.
fn mark_at(bytes: &[u8], index: usize) -> (Option<Mark>, usize) {
    match bytes[index] {
        b'{' if bytes.get(index + 1) == Some(&b'{') => {
            let opened = skip_space(bytes, index + 2);
            (identifier_end(bytes, opened).map(|_| Mark::DoubleBrace), 2)
        }
        b'{' => (brace_mark(bytes, index), 1),
        b'$' if bytes.get(index + 1) == Some(&b't') && bytes.get(index + 2) == Some(&b'(') => {
            (Some(Mark::DollarT), 3)
        }
        _ => (None, 1),
    }
}

/// What a single `{` opens, read with no grammar in hand.
fn brace_mark(bytes: &[u8], index: usize) -> Option<Mark> {
    let start = skip_space(bytes, index + 1);
    let end = identifier_end(bytes, start)?;
    let closing = skip_space(bytes, end);
    match bytes.get(closing) {
        Some(b',') => Some(Mark::IcuArgument),
        Some(b'}') if bytes[start..end].iter().all(u8::is_ascii_digit) => Some(Mark::Positional),
        Some(b'}') => Some(Mark::SingleBrace),
        _ => None,
    }
}

/// Marks that live in the key set rather than in a message.
pub(crate) fn key_marks(keys: &[&str]) -> Vec<Mark> {
    let mut found = Vec::new();

    if has_plural_family(keys) {
        found.push(Mark::PluralKeySuffix);
    }

    // ARB: `@@locale` and a `@key` object beside a real `key`.
    let has_document_metadata = keys.iter().any(|key| key.starts_with("@@"));
    let has_sibling_metadata = keys.iter().any(|key| {
        key.strip_prefix('@')
            .filter(|rest| !rest.starts_with('@') && !rest.contains('.'))
            .is_some_and(|rest| keys.contains(&rest))
    });
    if has_document_metadata && has_sibling_metadata {
        found.push(Mark::ArbMetadata);
    }

    // VS Code bundles use the English sentence as the key. A dotted
    // identifier never contains a space; a sentence almost always does.
    let sentences = keys
        .iter()
        .filter(|key| !key.starts_with('@') && key.contains(' '))
        .count();
    if sentences * 2 > keys.len() && sentences > 0 {
        found.push(Mark::SentenceKeys);
    }
    found
}

/// `item_one` beside `item_other` — two variants agreeing on a base.
///
/// One variant alone is not a family: `step_one` is an ordinary key in
/// every project that numbers its steps.
fn has_plural_family(keys: &[&str]) -> bool {
    let mut bases: Vec<&str> = Vec::new();
    for key in keys {
        let base = plural_base(key);
        if base == *key {
            continue;
        }
        if bases.contains(&base) {
            return true;
        }
        bases.push(base);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Id;

    fn grammar(id: Id) -> Grammar {
        id.library().grammar
    }

    fn tokens(id: Id, text: &str) -> Vec<String> {
        parse(grammar(id), text)
            .tokens
            .into_iter()
            .map(|token| token.name)
            .collect()
    }

    fn foreign(id: Id, text: &str) -> Vec<Construct> {
        parse(grammar(id), text)
            .foreign
            .into_iter()
            .map(|one| one.construct)
            .collect()
    }

    #[test]
    fn each_library_reads_its_own_interpolation() {
        assert_eq!(tokens(Id::I18next, "Hello {{name}}"), ["name"]);
        assert_eq!(tokens(Id::NextIntl, "Hello {name}"), ["name"]);
        assert_eq!(tokens(Id::VscodeL10n, "Rank {0} of {1}"), ["0", "1"]);
        assert_eq!(tokens(Id::FlutterArb, "Hello {name}"), ["name"]);
    }

    #[test]
    fn a_dotted_or_hyphenated_name_is_one_token() {
        assert_eq!(
            tokens(Id::I18next, "{{user.first-name}}"),
            ["user.first-name"]
        );
    }

    /// **The ambiguity no classifier could settle**, settled by being
    /// told. The same six bytes, three answers.
    #[test]
    fn the_same_bytes_read_differently_under_each_library() {
        assert_eq!(tokens(Id::NextIntl, "Hello {name}"), ["name"]);
        assert_eq!(tokens(Id::I18next, "Hello {name}"), [] as [String; 0]);
        assert_eq!(tokens(Id::VscodeL10n, "Hello {name}"), [] as [String; 0]);
        // And the ones that read it as prose say so, so a botched
        // translation is still catchable.
        assert_eq!(
            parse(grammar(Id::I18next), "Hello {name}").others,
            [Interpolation::SingleBrace]
        );
    }

    #[test]
    fn i18next_trims_inside_its_braces_and_others_do_not() {
        assert_eq!(tokens(Id::I18next, "Hello {{ name }}"), ["name"]);
        assert_eq!(tokens(Id::NextIntl, "Hello { name }"), ["name"]);
        assert_eq!(
            tokens(Id::VscodeL10n, "Hello { 0 }"),
            [] as [String; 0],
            "VS Code does not trim"
        );
    }

    /// The lexer's first duty: `{{` is ICU's literal `{`.
    #[test]
    fn an_icu_literal_brace_is_never_a_token() {
        for id in [Id::NextIntl, Id::FlutterArb, Id::VscodeL10n] {
            assert_eq!(tokens(id, "use {{ to open"), [] as [String; 0], "{id:?}");
            assert_eq!(foreign(id, "use {{ to open"), [], "{id:?}");
        }
        assert_eq!(tokens(Id::I18next, "a }} b"), [] as [String; 0]);
    }

    /// And its second. A percentage in prose is not printf, in any
    /// language — "90%-os" is Hungarian, not a left-justified octal.
    #[test]
    fn a_percentage_in_prose_is_not_printf() {
        for prose in [
            "100%% off",
            "50% off",
            "a % b",
            "90%-os lefedettség",
            "%90 oranında",
            "up to 20%!",
        ] {
            assert_eq!(foreign(Id::I18next, prose), [], "{prose}");
        }
        assert_eq!(foreign(Id::I18next, "Hello %s"), [Construct::Printf]);
        assert_eq!(foreign(Id::I18next, "Rank %1$s"), [Construct::Printf]);
    }

    /// **The construct the guessing core refused twenty-five real
    /// catalogues over.**
    #[test]
    fn nesting_is_a_key_reference_where_the_library_writes_it() {
        let parsed = parse(grammar(Id::I18next), "$t(metrics.window.{{timeframe}})");
        assert_eq!(parsed.foreign, [], "not foreign under i18next");
        assert_eq!(parsed.tokens[0].name, "timeframe");
    }

    #[test]
    fn nesting_is_foreign_where_the_library_does_not() {
        assert_eq!(
            foreign(Id::NextIntl, "$t(common.hello)"),
            [Construct::Nesting]
        );
    }

    /// An ICU plural in an i18next catalogue renders its braces to the
    /// user, and the sub-messages inside are not placeholders of their
    /// own.
    #[test]
    fn an_icu_plural_is_foreign_where_icu_is_not_native() {
        let parsed = parse(
            grammar(Id::I18next),
            "{count, plural, one {# item} other {# items}}",
        );
        assert_eq!(
            parsed
                .foreign
                .iter()
                .map(|f| f.construct)
                .collect::<Vec<_>>(),
            [Construct::IcuArgument]
        );
        assert_eq!(parsed.tokens, []);
    }

    #[test]
    fn an_icu_plural_is_one_token_where_it_is_native() {
        assert_eq!(
            tokens(
                Id::NextIntl,
                "{count, plural, one {# item} other {# items}}"
            ),
            ["count"]
        );
        assert_eq!(
            tokens(Id::NextIntl, "{count, plural, one {# item}"),
            ["count"],
            "an unclosed construct must not run away"
        );
    }

    #[test]
    fn every_foreign_construct_is_named() {
        assert_eq!(foreign(Id::I18next, "Hello { $name }"), [Construct::Fluent]);
        assert_eq!(
            foreign(Id::I18next, "Hello ${name}"),
            [Construct::TemplateLiteral]
        );
        let offsets: Vec<usize> = parse(grammar(Id::I18next), "Hello %s and %d")
            .foreign
            .into_iter()
            .map(|one| one.offset)
            .collect();
        assert_eq!(offsets, [6, 13]);
    }

    #[test]
    fn a_multibyte_message_parses_and_the_offset_is_a_byte_offset() {
        assert_eq!(tokens(Id::I18next, "Привет {{name}}"), ["name"]);
        assert_eq!(
            parse(grammar(Id::I18next), "日本語 %s").foreign[0].offset,
            10
        );
    }

    #[test]
    fn a_lone_brace_in_prose_is_not_a_finding() {
        for id in [Id::I18next, Id::NextIntl, Id::VscodeL10n] {
            assert_eq!(foreign(id, "write {} for an empty object"), [], "{id:?}");
            assert_eq!(
                tokens(id, "write {} for an empty object"),
                [] as [String; 0],
                "{id:?}"
            );
        }
    }

    #[test]
    fn a_plural_variant_folds_onto_its_base() {
        assert_eq!(plural_base("item_one"), "item");
        assert_eq!(plural_base("item_other"), "item");
        assert_eq!(plural_base("item"), "item");
        assert_eq!(plural_base("nav.home"), "nav.home");
    }

    #[test]
    fn the_identification_scan_reads_every_mark() {
        assert_eq!(marks("Hello {{name}}"), [Mark::DoubleBrace]);
        assert_eq!(marks("{count, plural, other {#}}"), [Mark::IcuArgument]);
        assert_eq!(marks("Rank {0}"), [Mark::Positional]);
        assert_eq!(marks("Hello {name}"), [Mark::SingleBrace]);
        assert_eq!(marks("$t(a.b)"), [Mark::DollarT]);
        assert_eq!(marks("nothing here"), []);
        assert_eq!(marks("use {{ to open"), [Mark::DoubleBrace]);
    }

    #[test]
    fn a_plural_family_is_a_mark_and_a_lone_variant_is_not() {
        assert!(key_marks(&["item_one", "item_other"]).contains(&Mark::PluralKeySuffix));
        assert!(!key_marks(&["step_one", "title"]).contains(&Mark::PluralKeySuffix));
    }

    /// ARB is identifiable from a catalogue with no project around it,
    /// which is what its decisive signature claims.
    #[test]
    fn arb_metadata_needs_both_halves() {
        assert!(
            key_marks(&["@@locale", "greeting", "@greeting"]).contains(&Mark::ArbMetadata),
            "the document metadata and a sibling"
        );
        assert!(!key_marks(&["@@locale", "greeting"]).contains(&Mark::ArbMetadata));
        assert!(!key_marks(&["greeting", "@greeting"]).contains(&Mark::ArbMetadata));
    }

    #[test]
    fn sentence_keys_are_a_mark_and_dotted_identifiers_are_not() {
        let sentences = ["Save file", "No catalogues found", "Extract"];
        assert!(key_marks(&sentences).contains(&Mark::SentenceKeys));

        let identifiers = ["manifest.command.title", "nav.home"];
        assert!(!key_marks(&identifiers).contains(&Mark::SentenceKeys));
    }
}
