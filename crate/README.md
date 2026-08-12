<h1 align="center">i18n-le</h1>

<p align="center">
  <b>Audit translation catalogues for missing keys, placeholder drift and structural mismatches</b><br/>
  <i>keys and tokens — never a translated string</i>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rustc-1.88+-93450a.svg" alt="MSRV: Rust 1.88+" />
  <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT" />
  <a href="https://letools.dev">
    <img src="https://img.shields.io/badge/web-letools.dev-00A0FF.svg" alt="letools.dev" />
  </a>
</p>

Spanish shipped last week and the metrics screen has said `{{periodo}}`
to every user since. The catalogue had every key. It parsed. The
placeholder came back from machine translation with its name translated
too, which compiles perfectly and renders the literal.

```bash
i18n-le locales/
```

```
i18next ^26.2.0 — i18next in ../package.json, double-brace, useTranslation( in ../src/app.tsx
de.json: structure app (a string here, an object in the source)
es.json: placeholder renamed metrics.window (timeframe became periodo)
es.json: missing dashboard.empty.title
fr.json: duplicate app.greeting (defined 2×)
4 findings across 25 catalogues
```

Exit code 1.

## This one has a verdict

The exit code is the product:

- **0** — clean. Also 0 when there are no catalogues: nothing to be
  wrong with.
- **1** — findings.
- **2** — the question was malformed. Including *no i18n library could
  be identified*, which is a malformed question rather than a clean
  answer.

## What it checks

| Check | Severity |
|---|---|
| `missing-key` — in the source, absent from the target | error |
| `extra-key` — in the target, absent from the source | error |
| `placeholder-count-mismatch` — a placeholder was dropped or added | error |
| `placeholder-name-mismatch` — `{{timeframe}}` came back as `{{periodo}}` | error |
| `placeholder-style-mismatch` — the target changed grammar | error |
| `empty-value` — a value that is empty or only whitespace | warning |
| `untranslated` — target byte-identical to source | **info** |
| `duplicate-key-within-file` — a key defined twice | error |
| `structure-mismatch` — an object in one locale, a string in another | error |
| `convention-mismatch` — a construct from a convention this project does not use | error |

The libraries it reads: **i18next**, **next-intl**, **vscode-l10n**,
**flutter-arb**. Adding another is a row in a table.

**`untranslated` does not fail a run by default.** A string added to
English this morning is legitimately untranslated everywhere else, and a
tool that broke the build for it gets switched off within a week.
`--fail-on untranslated` and `--fail-on any` promote it.

**`structure-mismatch` is the one a hand-rolled parity test misses.**
Flatten the two catalogues and compare, and an object that became a
string looks like forty missing keys. This reports the one thing that is
actually wrong and suppresses everything under it.

**`duplicate-key-within-file` needs a duplicate-preserving parse.**
`JSON.parse` and `serde_json::Value` both keep the last occurrence and
say nothing, so the earlier translation is text nobody will ever see.

## It works out which library you use, before it reads anything

Identification is the front door. Five classes of evidence — a manifest
dependency, a config file, the directory layout, the catalogue's own
syntax, and call sites in your source — and **two agreeing is an
identification**:

```
i18next ^26.2.0 — i18next in ../package.json, a directory of <locale>.json,
double-brace, dollar-t, useTranslation( in ../src/app.tsx
```

The library then supplies the placeholder grammar, the plural model,
which keys are metadata and how the files are laid out. **Nothing
downstream guesses.**

So `$t(metrics.window.{{timeframe}})` is i18next nesting and reads fine,
`{{ name }}` is the variable `name` because i18next trims, and an ICU
`{count, plural, …}` in that same project is a **finding** — the loader
will render those braces to a user verbatim:

```
en.json: wrong convention in items (icu-argument at offset 0)
```

The same bytes under `--system next-intl` are perfectly ordinary. That
is the point: `{name}` is a next-intl interpolation, literal text in
i18next, and literal text in a VS Code bundle, and no amount of looking
at the bytes settles which. Being told does.

**Two libraries identified is a refusal naming both.** i18next *and*
next-intl in one manifest has two answers, and nothing can know which
these files belong to. **Nothing identified is a refusal too**, listing
every signal it did find. There is no fallback that guesses — guessing
is what this replaced.

`--system <name>` overrides all of it, and it is not a convenience: a
translations-only repository has no manifest, and a VS Code extension's
`l10n/` bundles depend on no package at all — the mechanism is the
editor.

### It reads your source, and only to identify

Call sites are evidence. `useTranslation()`, `useTranslations()`,
`vscode.l10n.t(`, `AppLocalizations.of(` — found by substring, bounded
to 400 files, never inside `node_modules` or a build directory.

**No finding ever comes from a source file.** Unused-key and
undefined-key detection are non-goals and stay non-goals: they make the
tool language-coupled and are wrong about every dynamically built key. A
test asserts that a secret sitting in a scanned source file never
reaches stdout or stderr.

## Plurals

Plural **categories** are deliberately not checked: a CLDR table that is
right in English and French and wrong in Polish, Arabic and Welsh would
be worse than a principled non-answer, and getting it wrong is invisible
until a user sees it.

What the identified library buys is the encoding. Under `i18next`,
`item_one` and `item_other` are one key `item` — so Polish's legitimate
`item_few` is not an extra key, Japanese's absent `item_one` is not a
missing one, and a finding names the base once instead of the variants
six times. Under `next-intl`, `{count, plural, …}` is one token named
`count`, so a translation that drops the whole construct is still
caught.

## It never reads a translation

A catalogue is a product's entire user-facing voice. Only key names and
structural facts are reported — the report has no field for a string, and
no flag can ask for one.

That is not a promise, it is the type system. A finding carries
`Evidence`, which has exactly six variants: token names, counts, styles,
shapes, occurrence counts, and a construct with a byte offset. There is
no variant that could hold a sentence.

- An `untranslated` finding is proved by "source and target are
  byte-identical" — a fact about the text, not the text.
- A placeholder finding is proved by the **tokens**, which are metadata.
- A `convention-mismatch` is proved by the construct's **name and byte
  offset**, never the string that carried it.

Tokens yes, sentences no. Keys are the deliberate exception: a finding
that will not name its key is not a finding.

## One source, never the union

`--source` takes a path or a language tag. Without it, the source is
auto-detected **only when exactly one candidate exists** — otherwise the
tool exits 2 and names the fix.

A union of all locales would be actively wrong: one locale's typo'd key
would become a requirement every other locale is then missing, so a
single translator's slip turns into twenty-four findings against
everybody else.

## It finds your catalogues wherever they are

The locale is whatever the names in a directory do **not** share:

```
locales/en.json                 → en
locales/pt-BR.json              → pt-BR
messages/messages.es.json       → es
arb/app_pt_BR.arb               → pt-BR
locales/pl/common.json          → pl   (i18next namespaces)
```

Reading each name alone cannot work — `nls` is three letters and passes
for a language tag. Anything left over that is not shaped like a tag is
refused by name, which is what stops a repository root being audited as
a catalogue set.

**The library can supply the prefix instead**, which makes each name
readable on its own — and that is what lets a set live in two
directories:

```
package.nls.json                → the base, at the extension root
src/i18n/package.nls.zh-cn.json → zh-CN
```

Point it at `src/i18n/` and it walks up for the base. **Every VS Code
extension is written this way.** The `vscode-l10n` row also knows that
`bundle.l10n.json` keys *are* the English strings and `package.nls.json`
keys are not — two layouts, one library, different answers.
`--keys-are-source` forces it by hand.

## Formats

**JSON, nested and flat/dotted** — both normalise to the same dotted
path space, so a project that migrated between the two shapes can still
be compared against its own history. `.arb` reads through the same
parser; under `flutter-arb` its `@key` metadata objects are excluded
too, which only that library knows to do.

Fluent, gettext, `strings.xml`, Apple `.strings`/`.xcstrings`, YAML and
Java `.properties` are **documented deferrals**, not silent gaps — see
[SPEC.md](SPEC.md).

## Options

```
usage: i18n-le [options] <dir|file>...
       i18n-le mcp
       i18n-le --version | --help

  --system <name>          i18next, next-intl, vscode-l10n, flutter-arb
  --source <path|locale>   the catalogue every other is measured against
  --keys-are-source        the key is the English string
  --fail-on <what>         untranslated, or any
  --strict                 also fail when a catalogue could not be read
                           or parsed
```

A *shared* set is one directory: its names are only readable together,
so a run spanning two is refused. A fixed-prefix set is not, because the
library supplies the naming rule.

## Output

stdout is protocol, stderr is human. One JSON report for the whole run —
`schema: 3`, `severity` on every finding, and **no timestamp**, because
a report is a thing to diff.

```json
{
  "schema": 3,
  "status": "findings",
  "system": {
    "library": "i18next",
    "version": "^26.2.0",
    "layout": { "shape": "shared", "extension": "json" },
    "keysAreSource": false,
    "evidence": [
      { "class": "manifest", "library": "i18next", "detail": "i18next in ../package.json" },
      { "class": "content", "library": "i18next", "detail": "double-brace" }
    ]
  },
  "source": { "path": "en.json", "locale": "en", "keys": 708 },
  "files": [{ "path": "es.json", "locale": "es", "keys": 706 }],
  "findings": [
    {
      "severity": "error",
      "kind": "placeholder-name-mismatch",
      "file": "es.json",
      "key": "metrics.window",
      "sourceTokens": ["timeframe"],
      "targetTokens": ["periodo"]
    }
  ],
  "diagnostics": [],
  "summary": { "files": 25, "findings": 1 }
}
```

## As an MCP server

```bash
i18n-le mcp
```

One tool, `check_catalogues`, returning `{ ok, data, diagnostics, meta }`
— file contents in, findings out, no filesystem touched. `ok` means the
audit ran, never that the answer was yes.

It takes a **required** `library` argument and never detects one:
without a filesystem there is no manifest, no config and no call site to
read. A caller that knows the project says so; one that does not gets a
refusal. That is the honest split, not a missing feature.

## Install

| Route | Command | Worth knowing |
|---|---|---|
| **cargo** | `cargo install i18n-le` | Any platform, needs **Rust 1.88+**. |
| **From source** | `cd i18n-le/crate && cargo build --release` | Two dependencies: `serde` and `serde_json`. |

No runtime, no network, nothing written.

## License

MIT — see [LICENSE](LICENSE).
