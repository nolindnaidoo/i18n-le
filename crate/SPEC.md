# i18n-le — Rust specification

Audit a set of translation catalogues against one of them and report
what is structurally wrong. A CLI and an MCP server, one implementation
behind both.

## The one question

**Is this locale actually shippable, or does it only look like it is?**

A catalogue that parses, has every key, and renders `{{periodo}}` to a
user is a catalogue that passed every check most projects have.

## Identification is the front door

**The tool works out which i18n library the project implements before it
reads a single catalogue, and everything after that is read off the
answer** — the placeholder grammar, the plural model, which keys are
metadata, and how the files on disk are laid out.

**Nothing downstream guesses.** There is no inference step, no fallback
classifier, and no "if we could not tell" branch. If no library can be
identified and none was named, the run refuses and says which signals it
found.

This is what v0.3 replaced. v0.1 classified the placeholder grammar by
scanning message content; v0.2 bolted library profiles on top of that
classifier. Both were wrong in the way heuristics are wrong:
confidently. Twenty-five real catalogues were refused over
`$t(metrics.window.{{timeframe}})` — not an unknown construct at all,
but i18next nesting, specified by a library the project declares in its
own `package.json`. The tool refused because it did not know what it was
looking at. Now it asks.

### Five classes of evidence

**Corroboration, not inference. Two agreeing is an identification.**

| Class | What it reads |
|---|---|
| **Manifest** | `package.json` dependencies (`dependencies`, `devDependencies`, `peerDependencies`) and its `l10n` field; `pubspec.yaml` |
| **Config** | `i18next-parser.config.*`, `next-i18next.config.*`, `next-intl.config.*`, `l10n.yaml` |
| **Layout** | a directory shape only one library writes |
| **Content** | the catalogue's own syntax — the strongest class, because it is what actually breaks at runtime |
| **Call site** | `useTranslation()`, `useTranslations()`, `vscode.l10n.t(`, `AppLocalizations.of(` in source files |

Manifests and configs are looked for by walking **up** from the
catalogues, eight levels at most: in a monorepo the declaring manifest
can sit well above them.

### Signature strength

A content mark is worth one of three things, because `{name}` is a
next-intl interpolation *and* ordinary prose in every other library's
catalogues:

- **Decisive** — identifies on its own. Reserved for syntax no other
  library writes. It is what makes a translations-only repository, with
  no manifest and no call sites, auditable at all. ARB's `@@locale`
  beside `@key` siblings is the one v0.3 ships.
- **Strong** — counts as that library's content evidence.
- **Weak** — recorded so a refusal can say what it saw; never enough to
  vote.

Layout evidence follows the same logic: a directory of `<locale>.json`
is what almost every project's translations look like, so it only votes
when the directory is one the library actually names (`locales/`,
`messages/`). A directory of `<locale>.arb`, or a `bundle.l10n.*` /
`package.nls.*` prefix, is distinctive on its own.

### Refusals

- **Two libraries identified** → exit 2, naming both and the evidence
  for each. **Never pick one**: every later answer would inherit the
  coin flip.
- **Nothing identified** → exit 2, listing every signal that *was*
  found and what was missing.
- **`--system <name>`** overrides all of it, and is not a convenience: a
  translations-only repository has no manifest, and a VS Code
  extension's `l10n/` bundles depend on no package at all — the
  mechanism is the editor.

### The call-site boundary

**Source files are read, and only to answer which library this is.**

No finding ever comes from a source file. Unused-key and undefined-key
detection remain non-goals, for the reason they always were: they make
the tool language-coupled, need a parser per framework, and are wrong
about every dynamically built key. The scan looks for fixed substrings,
keeps nothing, and reports only the path it found one in — a test
asserts that a secret sitting in a scanned source file never reaches
stdout or stderr.

It is bounded: it starts at the nearest ancestor holding a manifest,
descends at most six levels and 400 files, skips files over 512 KB, and
never enters `node_modules`, `.git`, `target`, `dist`, `build`, `.next`,
`out`, `out-test`, `coverage`, `vendor` or `lib`.

### The version gate

One, and it names a real break rather than an allowlist: i18next moved
plural key suffixes from `_plural` to `_one`/`_other` between v21 and
v23, so reading a v22 catalogue with the v23 model would fold the wrong
keys. A declared major below that floor is exit 2 naming the manifest.

An *unfamiliar* major — newer than anything this was written against —
is read and reported. v0.2 refused every major outside an allowlist,
which turned "we have not tested this" into "we will not answer" and
made the tool brittle against libraries that keep shipping.
Identification is corroborated by content now; a version allowlist was
carrying weight it did not need to.

`--system` skips the gate, because a caller naming the library has made
the claim themselves.

## The libraries

| Library | Manifest | Layout | Content signature | Plurals |
|---|---|---|---|---|
| **i18next** | `i18next`, `react-i18next`, `i18next-vue` | `locales/{lng}/{ns}.json`, or `locales/<locale>.json` | `{{var}}`, `$t(key)`, `_one`/`_other` suffixes | key suffixes |
| **next-intl** | `next-intl` | `messages/<locale>.json` | ICU `{count, plural,}`, `{name}` | ICU |
| **vscode-l10n** | `@vscode/l10n`, or the `l10n` field | `l10n/bundle.l10n.<locale>.json`, **and** root `package.nls.json` with translations elsewhere | sentence keys, `{0}` | none |
| **flutter-arb** | `flutter_localizations`, `intl` (pubspec) | `<anything>_<locale>.arb` | `@@locale` with `@key` siblings, ICU | ICU |

**Adding a library is a row in `library.rs`**, not a change anywhere
else. A genuinely new syntax needs one new `Mark` and its predicate in
`message.rs`; packages, config files, call sites, layouts, grammar,
plurals and metadata are all data.

`pubspec.yaml` is read by a line scan, not a YAML parser — it answers
one question ("does this name a package I know?") and a YAML dependency
would cost more than that answer is worth.

## Layouts

**`shared`** — the locale is whatever the file names in the set do
**not** share:

```text
locales/en.json                 → en
locales/pt-BR.json              → pt-BR
messages/es.json                → es
lib/l10n/app_pt_BR.arb          → pt-BR
```

Reading each name on its own cannot work: `nls` in `package.nls.json` is
three letters and passes for a language tag, and `app_en.arb` is either
the locale `app-EN` or the prefix `app` and the locale `en`. Only the
set makes it decidable, so a shared set must live in one directory, and
a leftover segment that is not shaped like a tag is refused by name.

**`fixed`** — the library supplies the prefix, so each name is readable
alone, so **the set may span two directories**:

```text
package.nls.json                  → the base, at the extension root
src/i18n/package.nls.zh-cn.json   → zh-CN
l10n/bundle.l10n.json             → the base
l10n/bundle.l10n.pt-br.json       → pt-BR
```

Pointing at `src/i18n/` walks up for the base. **Every VS Code extension
is written this way**, and it is the layout nothing before v0.3 could
audit at all.

The two VS Code layouts disagree about what a key is:
`bundle.l10n.json` keys **are** the English strings, `package.nls.json`
keys are dotted identifiers. That is a property of the layout, so the
tool sets `keysAreSource` itself; `--keys-are-source` forces it by hand.

**`namespaced`** — `<root>/<locale>/<namespace>.json`, i18next's other
shape. The parent directory is the locale, and files are reported as
`<locale>/<file>` because the base names collide. **One namespace is one
set**; a directory carrying several is refused, naming them, because
auditing them together would compare `common.json` against
`errors.json`.

A tag is `language[-Script][-REGION]`, canonicalised to one spelling, so
`pt-br`, `pt_BR` and `pt-BR` are the same locale.

## The checks

| Check | Severity | What it means |
|---|---|---|
| `missing-key` | error | In the source, absent from the target. |
| `extra-key` | error | In the target, absent from the source. |
| `placeholder-count-mismatch` | error | A placeholder was dropped or added. |
| `placeholder-name-mismatch` | error | Same count, different names — `{{timeframe}}` came back as `{{periodo}}`. |
| `placeholder-style-mismatch` | error | The target wrote its interpolation in a style this library does not read, where the source had a real placeholder. |
| `convention-mismatch` | error | A construct from another convention. The loader renders it verbatim. |
| `empty-value` | warning | A value that is empty or only whitespace. |
| `untranslated` | **info** | Target byte-identical to source. |
| `duplicate-key-within-file` | error | A key defined twice. JSON permits it and every loader silently keeps the last. |
| `structure-mismatch` | error | A path that is an object in one locale and a string in another. |

**`untranslated` is `info` and does not fail a run by default.** A string
added to English this morning is legitimately untranslated everywhere
else, and a tool that broke the build for that gets switched off within
a week. `--fail-on untranslated` and `--fail-on any` promote it.

**`convention-mismatch` covers exactly the constructs that cannot be
anything else** in the identified library: an ICU comma-form where ICU
is not native, a Fluent `{ $x }`, a printf conversion, a `${x}`, and
`$t(…)` outside i18next. A bare `{name}` in an i18next catalogue is
**not** one — it is literal text, which is what i18next renders it as.
Drift there is still caught, by `placeholder-style-mismatch`, and only
where the source had a real placeholder at the same key. It is emitted
per occurrence and suppresses the placeholder comparison for that key:
the mismatch is the root cause and a count difference on top would be
the same defect twice.

**`structure-mismatch` is the one a hand-rolled parity test misses.**
Flatten-then-compare cannot see it: after flattening, the two files
simply have different key sets, and the answer comes back as forty
missing keys instead of the one thing that is wrong. It is reported
first, and everything below the divergence is suppressed.

**`duplicate-key-within-file` needs a duplicate-preserving parse.**
`serde_json::Value` and `JSON.parse` both fold a repeated key into the
last one and say nothing, which is exactly the defect — the earlier
translation is text nobody will ever see.

## Plurals

**v0.3 does not check plural-category completeness, and that is a
decision rather than an omission.**

What the identified library does buy is the *encoding*:

- **Key-suffix plurals are folded onto a base key.** `item_one` and
  `item_other` are one key `item`, so Polish's legitimate `item_few` is
  not an extra key, Japanese's absent `item_one` is not a missing one,
  and a finding names the base once rather than the variants six times.
- **An ICU plural is one token named for its argument.**
  `{count, plural, one {# item} other {# items}}` contributes the token
  `count`, so a translation that drops the whole construct is caught.
  The sub-messages inside are not scanned, so a placeholder nested in a
  plural branch is not counted on either side — symmetric, and therefore
  never a false mismatch.

What it does **not** do: assert that a locale carries the categories
CLDR requires for it. That needs a versioned per-locale plural-rule set
this crate does not carry, and getting it wrong is invisible until a
user sees it. A category table that is right in English and French and
wrong in Polish, Arabic and Welsh would be worse than a principled
non-answer.

## Reading a message

The parser is **told** the grammar. It never classifies.

What it must honour, whatever the library:

- **`{{` is ICU's literal `{`** and `}}` its literal `}`. A parser that
  read `{{` as a placeholder would manufacture a finding in every ICU
  file that escapes a brace.
- **`%%` is a literal percent.**
- **Whitespace inside braces is the library's business**: i18next trims,
  ICU allows it, VS Code does not.
- **printf is read narrowly**: `%s`, `%d`, `%1$s` — a conversion
  straight after the `%`, or a positional argument before it. Flags,
  width and precision are deliberately not read, because they match
  ordinary text: Hungarian writes "90%-os", which is `%` + the
  left-justify flag + the octal conversion. That false positive judged a
  whole catalogue over a percentage, and it was found by running the
  binary rather than the tests.

## The reference model

**One named source locale.** Never the union of all of them: a union
would make one locale's typo'd key a requirement every other locale is
then missing, so a single translator's slip becomes twenty-four findings
against everybody else.

`--source` takes a path or a language tag. Without it, the source is
auto-detected **only when exactly one candidate exists** — a catalogue
tagged `en`-something, or the untagged one, which is what
`bundle.l10n.json` and `package.nls.json` both mean. Zero or several is
exit 2 naming the fix.

## The privacy boundary

**No translated value ever crosses the report, stderr, or the MCP
boundary.** A catalogue is a product's entire user-facing voice, and an
answer about whether Spanish is complete does not need the Spanish.

This is enforced in the types. A finding carries `Evidence`, which has
exactly six variants — token names, counts, styles, shapes, occurrence
counts, and a construct with a byte offset. There is no variant that can
hold a sentence, so there is nothing to remember not to put in one.

- An `untranslated` finding is proved by "source and target are
  byte-identical" — a fact about the text, not the text.
- A placeholder finding is proved by the **tokens**, which are metadata.
- A `convention-mismatch` is proved by the construct's **name and byte
  offset**.
- A call site is proved by the **substring searched for and the path**,
  never by anything else in the file.

**There is no `--show-values`, and no MCP argument that asks for one.**

**Keys are the deliberate exception.** A finding that will not name its
key is not a finding. Where the layout says the key *is* the English
string, English reaches the report — that is the shape the VS Code
bundle has, stated rather than hidden. A *translation* never does.

**Paths are reported relative to the catalogues**, never absolute — with
one exception, recorded in AGENTS.md rather than left to be discovered:
a `skipped` diagnostic's `message` is the underlying read error and
names the file it could not open in full.

## Formats

**JSON only** — nested and flat/dotted, which normalise to the same
dotted path space, so a project that migrated between the two shapes can
still be compared against its own history. `.arb` reads through the same
parser; under `flutter-arb` its `@@` document metadata *and* its `@key`
per-message metadata are excluded.

**Which keys are metadata is the library's answer, applied after the
read.** The reader returns everything, because identification reads the
raw key set: `@greeting` beside `greeting` is the evidence that says the
file is ARB, and a reader that had already dropped it would have thrown
that away.

Deferred, and named here rather than left as a silent gap:

| Format | Why not yet |
|---|---|
| `.ftl` (Fluent) | Its own grammar with attributes, terms and selectors. A library row could describe it; the parser is the work. |
| `.po` / `.pot` (gettext) | Plural forms are the file's own header expression, which is the CLDR problem in a different spelling. |
| `strings.xml` (Android) | Plurals, string arrays, and a quoting model that changes what "byte-identical" means. |
| `.strings` / `.stringsdict` / `.xcstrings` (Apple) | Three formats sharing a name; `.xcstrings` carries per-locale state. |
| YAML (Rails, vue-i18n) | An extra parser for a shape that is JSON underneath. Cheap to add; not free. |
| `.properties` (Java) | Escapes and encoding rules that make a naive read wrong. |

## Output contract

**stdout is protocol, stderr is human.** One JSON report for the whole
run. **There is no `--json` flag** — one mode, nothing to misremember,
and the human summary is a projection of the same report so the two
cannot drift.

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
      { "class": "manifest", "detail": "i18next in ../package.json" },
      { "class": "content", "detail": "double-brace" },
      { "class": "call-site", "detail": "useTranslation( in ../src/app.ts" }
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
  "summary": { "files": 2, "findings": 1 }
}
```

- **`schema` is 3.** v2 had a guessed `profile` and a `grammar` field
  and a `refusals` array; a v2 reader would not recognise `system`, and
  with a library in hand there is nothing left to refuse per file.
- **`system.evidence` is reported** because "why did it decide this was
  i18next" is the first question anyone asks of an answer they did not
  expect — and an identification nobody can check is a heuristic with
  better manners.
- **`system.layout` is `null` on the MCP surface**, which reaches no
  filesystem and so has none to report.
- **`severity` is on every finding.**
- **`keys` is a count, not a list.**
- **There is no timestamp.** A report is a thing to diff, and a clock
  makes every run differ from every other for a reason that is not the
  catalogues.

## The CLI surface

```
usage: i18n-le [options] <dir|file>...
       i18n-le mcp
       i18n-le --version | --help

Options:
  --system <name>          i18next, next-intl, vscode-l10n, flutter-arb
  --source <path|locale>   the catalogue every other is measured against
  --keys-are-source        the key is the English string
  --fail-on <what>         untranslated, or any
  --strict                 also fail when a catalogue could not be read
                           or parsed
```

Exit codes: **0** clean · **1** findings · **2** malformed question.
Finding no catalogues at all is 0. There is no working-directory
default.

## The MCP surface

One tool, `check_catalogues`, taking `library` and
`files: [{ path, locale, content }]`, touching no filesystem.

**The library is required and never detected here.** Identification
reads manifests, config files, directory layouts and source call sites,
none of which exist on this surface. A tool that guessed instead would
be the thing this crate stopped doing, so the caller names the library
or gets a refusal. That is the honest split, not a missing feature.

The result is the family envelope, `{ ok, data, diagnostics, meta }`,
where **`ok` means the audit ran, never that the answer was yes**. A
contract test asserts the two surfaces produce identical findings for
the same catalogues.

## Files that cannot be read

Exit 2 means the *question* was malformed — an unknown flag, no library
identified, two libraries identified, a directory that is not a
catalogue set, a source that is not there, a breaking major.

A file that is not UTF-8 text, or that will not parse as JSON, is named
on stderr and carried in the report with a diagnostic, and the rest are
still audited. `--strict` turns that into exit 2. What is never allowed
is the third option: a file that silently vanishes from the report,
which reads to whoever ran it as a file that was clean.

A manifest that will not parse is **not** a refusal: it is somebody
else's broken file, and the catalogues beside it may be fine.

## The byte-order mark

A leading BOM is stripped before parsing. Three invisible bytes that
Notepad, Excel and a PowerShell redirect all add. Before a `{` they make
the parse fail outright, which reads as "this locale has no keys".

## Non-goals

- **Unused and undefined key detection.** Finding keys the code never
  references means scanning source code *for findings* — language-
  coupled, a parser per framework, and wrong about every dynamically
  built key. Source is read for identification only, and the boundary is
  written down here so it cannot erode.
- **HTML-tag parity**, **length and truncation heuristics**,
  **glossary and terminology enforcement** — the last needs the
  translations, which crosses the privacy boundary this is built on.
- **Ordering and normalization.** Key order is not meaning.
- **It never writes.** No `--fix` — the right translation for a missing
  key is not something a tool can know.
- **No network, ever.** No translation API, no registry lookup to
  resolve a version.

## Not in v0.3

- **Plural-category completeness**, as above.
- **A shared set split across two directories.** A fixed-prefix set may
  span them because the library supplies the naming rule; a shared one
  cannot.
- **Several namespaces, or several catalogue directories, in one run.**
- **A baseline file** for accepting known findings.
- **Per-locale severity policy** — "es must be complete, vi may lag".
- **i18next context suffixes** (`_male`, `_female`).
- **The libraries not in the table**: vue-i18n, lingui, gettext,
  Rails i18n, Angular, Paraglide. Each is a row plus, where its syntax
  is new, one predicate.
- **The formats in the deferral table above.**
