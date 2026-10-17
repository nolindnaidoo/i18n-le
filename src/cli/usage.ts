/**
 * The command line's help text and the flags it names.
 *
 * Word for word the Rust CLI's, because both install as `i18n-le` and a
 * person reading `--help` cannot tell which one they have.
 * `scripts/check-cli-parity.ts` compares the two outputs, and `cli.test.ts`
 * holds FLAGS equal to the flags this text names.
 */

export const USAGE = `usage: i18n-le [options] <dir|file>...
       i18n-le mcp
       i18n-le --version | --help

Audits a set of translation catalogues against one of them and reports
what is structurally wrong: keys a locale is missing or has too many of,
placeholders dropped or renamed in translation, constructs from the
wrong convention, values left empty, keys defined twice, and a path that
is an object in one locale and a string in another.

It works out which i18n library the project uses before it reads
anything, from five kinds of evidence — a manifest dependency, a config
file, the directory layout, the catalogue's own syntax, and call sites
in source. Two agreeing is an identification. The library then supplies
the placeholder grammar, the plural model, which keys are metadata and
how the files are laid out; nothing downstream guesses. If no library
can be identified, the run says which signals it found and stops.

One JSON report on stdout when it is piped or redirected, human summary
on stderr.

Only key names and structural facts are reported. No translated value
ever reaches stdout, stderr or an agent — a renamed placeholder is
proved by its tokens, and an untranslated string by the fact that it is
byte-identical to the English.

Options:
  --system <name>          the library to audit as, skipping
                           identification: i18next, next-intl,
                           vscode-l10n, flutter-arb
  --source <path|locale>   the catalogue every other is measured against;
                           auto-detected only when exactly one English
                           candidate exists
  --keys-are-source        the key is the English string, as in a VS Code
                           bundle.l10n.json
  --fail-on <what>         untranslated, or any — what else fails the run
                           besides the findings that already do
  --strict                 also fail when a catalogue could not be read
                           or parsed

Exit codes: 0 clean · 1 findings · 2 malformed question. Finding no
catalogues at all is 0 — there is nothing to be wrong with.`;

export const FLAGS: readonly string[] = Object.freeze([
	'--system',
	'--source',
	'--keys-are-source',
	'--fail-on',
	'--strict',
]);
