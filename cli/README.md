# i18n-le

Audit a set of translation catalogues against one of them and get back
what is structurally wrong: keys a locale is missing or has too many of,
placeholders dropped or renamed in translation, constructs from the wrong
convention, values left empty, keys defined twice, and a path that is an
object in one locale and a string in another.

```bash
npx i18n-le locales/
npx i18n-le --system next-intl messages/ > audit.json
npx i18n-le --fail-on untranslated src/locales
```

The command it installs is `i18n-le`, the same command line as the Rust
crate [`i18n-le`](https://crates.io/crates/i18n-le), and it behaves
identically: same identification, same audit, same output, same exit
codes. Install whichever toolchain you already have.

## How it works

It works out which i18n library the project uses before it reads
anything: i18next, next-intl, a VS Code extension's `bundle.l10n` files, or
Flutter's ARB files. It looks at five kinds of evidence, a manifest
dependency, a config file, the directory layout, the catalogues' own syntax
and call sites in source, and two agreeing is an identification. The
library supplies the placeholder grammar, the plural model and the layout.
If nothing can be identified, it says which signals it found and stops;
`--system` names the library instead.

**No translated value ever reaches its output.** A renamed placeholder is
shown by its tokens, and an untranslated string by the fact that it is
byte-identical to the source.

## Output

stdout carries one JSON report whenever it is piped or redirected, and
nothing when it is a terminal. stderr is the summary.

```text
$ i18n-le locales
i18next ^23.10.0 — i18next in ../package.json, a directory of <locale>.json, double-brace
de.json: missing bye
de.json: placeholder renamed greeting (name became nom)
de.json: untranslated same
3 findings across 2 catalogues
```

## Exit codes

- **0**: clean, or no catalogues at all.
- **1**: findings. An untranslated string is reported as `info` and does
  not fail the run unless `--fail-on untranslated` or `--fail-on any` asks
  it to.
- **2**: the question was malformed, no library could be identified, or
  `--strict` and a catalogue could not be read.

## For agents

`i18n-le mcp` serves the Model Context Protocol on stdio with one tool,
`check_catalogues`, which audits catalogue contents it is handed. Hosts
can also install
[`i18n-le-mcp`](https://www.npmjs.com/package/i18n-le-mcp).

`i18n-le --help` lists every option.

MIT licensed. Part of [letools](https://letools.dev/tools/i18n-le).
