# Changelog

The Rust CLI and MCP server.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **A file is read only when it is a regular file.** The call-site scan
  refused a source file over 512 KB and never asked what kind of thing
  it was measuring, so a FIFO named `app.ts` — or a `package.json` or a
  `pubspec.yaml` that was one — blocked the read until something wrote
  to it, which for a pipe nobody owns is forever. Manifests pick up an
  8 MiB ceiling with the same reasoning: the path is a name somebody
  else's tree supplied, and no real `package.json` comes near it. The
  catalogue reads needed no guard and say so in a comment — every path
  they get is already a regular file.

- **A config file counts only in an extension that library writes.**
  Matching the stem alone meant `l10n.json` — a translation bundle — and
  `l10n.ts` — a module — each cast a config vote for flutter-arb, and
  `i18next-parser.config.md` would have voted for i18next. The library
  row now carries the extensions with the stem:
  `.js/.cjs/.mjs/.ts/.mts/.cts/.json` for the JavaScript configs,
  `.yaml/.yml` for Flutter's. A project whose only second class of
  evidence was a misspelled config now gets a refusal listing what was
  found rather than an identification resting on it.

### Fixed

- **`system.version` reads whichever manifest identified the library.**
  It read `package.json` and nothing else, so a Flutter project —
  identified from `intl: ^0.19.0` in its `pubspec.yaml`, with no
  `package.json` anywhere in the tree — was reported with
  `"version": null` by the same run that quoted that manifest as its
  evidence. The pubspec line scan is now one reader shared by the signal
  and the version. A package named without a version, which is how
  Flutter's own SDK packages are declared, is still not a version, and
  no longer ends the search for one.

- **A `skipped` diagnostic no longer carries an absolute path.** The
  message was the read error verbatim, and that error names the file in
  full — so two machines auditing the same repository produced different
  reports for the same unreadable catalogue, against SPEC.md's promise
  that every reported path is relative to the catalogues. The
  diagnostic's `file` field already carries the name the report uses, so
  the message is now the reason alone: `not UTF-8 text`, `Permission
  denied (os error 13)`. The stderr line loses a duplicated path with
  it.

## [0.3.0] - 2026-08-12

A rewrite, not a refactor. Identification became the front door, and the
guessing core it used to sit on top of is gone.

### Added

- **Identification from five classes of evidence** — a manifest
  dependency, a config file, the directory layout, the catalogue's own
  syntax, and call sites in source. **Two agreeing is an
  identification**; nothing reaching two is a refusal listing what was
  found, and two libraries reaching it is a refusal naming both. There
  is no fallback that guesses.
- **A decisive content signature**, for syntax no other library writes.
  ARB's `@@locale` beside `@key` siblings identifies on its own, which
  is what makes a translations-only repository — no manifest, no config,
  no call sites — auditable at all.
- **Call sites as evidence**, read from source **for identification
  only**. No finding ever comes from a source file; unused-key and
  undefined-key detection remain non-goals, and the boundary is written
  into SPEC.md so it cannot erode. The scan is bounded — 400 files, six
  levels, 512 KB each — and never enters `node_modules` or a build
  directory.
- **`next-intl`**, the fourth library, alongside i18next, vscode-l10n
  and flutter-arb.
- **`--system <name>`**, replacing `--profile`. Not a convenience: a
  translations-only repository has no manifest, and a VS Code
  extension's `l10n/` bundles depend on no package at all.
- **`system` in the report**, carrying the library, the declared
  version, the resolved layout and **the evidence**. An identification
  nobody can check is a heuristic with better manners.

### Changed

- **`schema` is 3.** v2's guessed `profile` and `grammar` fields are
  gone, and so is `refusals` — with a library in hand there is nothing
  left to refuse per file.
- **The message parser is told the grammar and never classifies.**
  `{name}` is a next-intl interpolation, literal text in i18next, and
  literal text in a VS Code bundle; no amount of looking at the bytes
  settles that, and being told does.
- **`placeholder-style-mismatch` fires only where a placeholder
  belonged** — where the source had a real token at the same key. A
  `{name}` in an i18next catalogue is otherwise prose, which is what the
  loader makes of it.
- **The version gate names a real break instead of an allowlist.**
  i18next below v23 wrote `key_plural`, so reading it with the v23 model
  would fold the wrong keys. An unfamiliar *newer* major is now read and
  reported rather than refused: an allowlist made the tool brittle
  against libraries that keep shipping, and identification is
  corroborated by content anyway.
- **The layout is an output of identification**, not a hardcoded set of
  cases. `shared`, `fixed` and `namespaced` are properties of the
  library row.
- **Metadata keys are filtered after the read, not during it.**
  Identification reads the raw key set, and a reader that had already
  dropped `@greeting` would have destroyed the evidence that says the
  file is ARB.
- **The coverage floor moved with the modules.** CI enforced 90% on
  `detect/`, which no longer exists; the job now lists the pure modules
  by name and fails if one of them is not measured at all, so a rename
  cannot leave it vacuously green.

### Removed

- **The per-file grammar classifier** (`detect/placeholder.rs`), the
  profile layer built on it (`detect/profile.rs`), and the layout
  hardcoding in `discover.rs`. Roughly 2,100 lines, replaced by
  identification plus a data table.
- **`--profile`**, including `--profile none`. There is no inferred mode
  to fall back to.
- **`placeholder-grammar-unsupported` and the `refusals` array.**

### Fixed

- **Fixtures no longer carry a real product name.** `en.json`,
  `es.json`, `fr.json`, the BOM and CRLF documents and the VS Code
  fixtures were copied from a private catalogue and carried its product
  name into a repository that is going public. A test now asserts none
  of them does.
- **A `pubspec.yaml`-only project is identified.** The `.arb` extension
  now votes as layout evidence on its own, because one library writes
  it — a directory of `<locale>.json` still does not.
- **A directory named `l10n` is no longer mistaken for `l10n.yaml`.**
  Config evidence requires a file with an extension.
- **The declared version is found above the catalogues.** The search
  returned at the first ancestor without a `package.json`, which is
  almost always the catalogue directory itself, so the version was
  always absent.

[0.3.0]: https://github.com/nolindnaidoo/i18n-le/releases/tag/crate-v0.3.0

## [0.2.0] - 2026-08-12

Placeholder grammar stops being inferred from content and starts coming
from the project's own declaration. The refusals v0.1 produced on real
catalogues were not caution — they were the tool not knowing what it was
looking at.

### Added

- **Profiles.** The nearest `package.json` or `pubspec.yaml` above the
  catalogues picks one of four conventions — `i18next`, `vscode-l10n`,
  `icu`, `flutter-arb` — and each declares its placeholder grammar,
  nesting syntax, plural encoding, metadata keys and file layouts.
  Content is then validated **against** the profile rather than sniffed.
- **`convention-mismatch`**, the finding that replaces most refusals. An
  ICU `{count, plural, …}` in an i18next catalogue is a defect, not a
  mystery: the loader renders those braces to a user verbatim. It covers
  only constructs that cannot be anything else — ICU comma-forms, Fluent
  variables, printf conversions, template literals, and `$t(…)` outside
  i18next. A bare `{name}` in an i18next file is literal text, which is
  what i18next makes of it.
- **`--profile <name>`**, and `--profile none` for the inferred path.
  Not a convenience: a translations-only repository has no manifest, and
  a VS Code extension's `l10n/` bundles depend on no package at all —
  the mechanism is the editor.
- **Version pinning, with an unknown major as a refusal.** i18next moved
  plural key suffixes from `_plural` to `_one`/`_other` between v21 and
  v23, so a profile that silently spanned that would be wrong for one of
  them. `*`, `latest` and `workspace:*` are unknown majors too — they
  name no version at all. The supported majors are in SPEC.md.
- **A refusal when two conventions are declared at one level**, naming
  both packages and both manifests. Never pick one.
- **The prefixed layout**, which lets a set span two directories because
  the profile supplies the naming rule rather than inferring it from
  what the names share. `package.nls.json` at an extension root with its
  translations in `src/i18n/` is now audited; v0.1 exited 2 on it.
- **The namespaced layout** — i18next's `<root>/<locale>/<ns>.json`. One
  namespace is one set; a directory carrying several is refused, naming
  them.
- **`profile` in the report**, with the pinned major and where it came
  from. "Why did it decide this was i18next" is the first question
  anyone asks of an answer they did not expect.

### Changed

- **`schema` is 2.** v1 had no `profile` and no `convention-mismatch`,
  and a v1 reader would not recognise either.
- **i18next plural key suffixes are folded onto a base key.**
  `item_one` and `item_other` are one key `item`, so Polish's legitimate
  `item_few` is no longer an extra key and Japanese's absent `item_one`
  is no longer a missing one — **a false positive v0.1 produced on every
  correctly translated plural in a project.** A finding names the base
  once instead of the variants six times.
- **An ICU plural is one token named for its argument** where ICU is the
  grammar, so dropping the whole construct is caught. The categories
  inside it are still not checked — that needs a versioned per-locale
  CLDR rule set, and a table that is right in English and wrong in
  Welsh would be worse than a principled non-answer. Stated in SPEC.md
  under "Plurals".
- **`{ name }` is no longer ambiguous under a profile.** i18next trims,
  ICU allows it, VS Code does not. Without a profile it is still
  refused, because from the bytes alone it is all three.
- **`flutter-arb` skips `@key` metadata objects**, not only `@@`-prefixed
  document metadata. Without that profile, only `@@` is skipped, because
  a `@greeting` key is somebody else's naming scheme.
- **The `vscode-l10n` profile decides `keys-are-source` itself** — true
  for `bundle.l10n.json`, false for `package.nls.json`. Two layouts, one
  profile, different answers.

### Fixed

- **Manifest paths in the report are relative to the catalogues.** An
  absolute `/Users/someone/work/…` put the machine that ran the audit
  into a report meant to be diffed against one from another machine —
  the same rule the file names already followed.
- **A path that is an object in the source and a string in the target no
  longer reports an extra key on top of the structure mismatch.** Object
  paths were left out of the "what does this file define" set, so the
  string looked like a key the source had never heard of.

[0.2.0]: https://github.com/nolindnaidoo/i18n-le/releases/tag/crate-v0.2.0

## [0.1.0] - 2026-08-12

First release. Core functionality: every check, both surfaces, and the
corpus they are pinned against. Everything below describes v0.1 as it
shipped; where 0.2.0 changed it, that entry says so.

### Added

- **Seven checks over a set of translation catalogues** — `missing-key`,
  `extra-key`, `placeholder-count-mismatch`,
  `placeholder-name-mismatch`, `placeholder-style-mismatch`,
  `empty-value`, `untranslated`, `duplicate-key-within-file` and
  `structure-mismatch`, plus `placeholder-style-mixed` for a source that
  speaks two grammars at once.
- **`structure-mismatch`**, which flatten-then-compare cannot see: a
  path that is an object in one locale and a string in another comes
  back as forty missing keys instead of the one thing that is wrong.
  Reported first, with everything below the divergence suppressed.
- **`duplicate-key-within-file`**, which needs a duplicate-preserving
  parse. `serde_json::Value` and `JSON.parse` both fold a repeated key
  into the last one silently, so the earlier translation is text nobody
  will ever see.
- **The placeholder grammar**, decided per file over every string in it:
  `{0}`, `{{name}}`, `{name}`. A file carrying anything else — an ICU
  plural, a Fluent `{ $var }`, a `%s`, a `${...}`, an `$t(...)` — has
  its placeholder checking switched off and says so, naming the file,
  the key, the byte offset and the construct. Refusals are kept out of
  the findings and out of the exit code.
- **Locale discovery** across the conventions that disagree about where
  the locale sits in the name — `en.json`, `bundle.l10n.pt-br.json`,
  `package.nls.zh-cn.json`, `messages.es.json`, `app_pt_BR.arb`. The
  locale is whatever the names in a set do not share; anything left over
  that is not shaped like a language tag is refused by name.
- **`--source <path|locale>`** and auto-detection that answers only when
  exactly one English candidate exists. **`--keys-are-source`** for the
  VS Code bundle shape where the key is the English string.
  **`--fail-on untranslated|any`** and **`--strict`**.
- **The CLI**: one JSON report on stdout, a human summary on stderr, and
  exit codes — 0 clean, 1 findings, 2 malformed question. No catalogues
  at all is 0, and a refused check is 0.
- **The MCP server** (`i18n-le mcp`) with `check_catalogues`: file
  contents in, findings out, no filesystem. Same `{ ok, data,
  diagnostics, meta }` envelope as the rest of the family, and a
  contract test asserting it answers identically to the CLI.

### The shape of it

**No translated value ever reaches an answer**, and the types are what
stop it rather than a convention. A finding carries `Evidence`, which
has five variants — token names, counts, styles, shapes, occurrence
counts — and none of them can hold a sentence. An `untranslated` finding
is proved by "source and target are byte-identical", a fact about the
text rather than the text. Keys are the deliberate exception, because a
finding that will not name its key is not a finding.

**One named source locale, never the union of all of them.** A union
would make one locale's typo'd key a requirement every other locale is
then missing, so a single translator's slip becomes twenty-four findings
against everybody else.

**The report carries a `schema` version, a `severity` on every finding,
and no timestamp.** A report is a thing to diff — against the last run,
against a baseline, in a pull request — and a clock makes every run
differ from every other for a reason that is not the catalogues.

### Fixed

- **printf detection no longer refuses a catalogue over a percentage.**
  Reading printf's flags, width and precision made `%` + `-` + `o` a
  conversion, which is how Hungarian writes "90%-os" — one ordinary
  sentence switched placeholder checking off for a whole 25-locale set.
  Only `%s`, `%d` and `%1$s` are read now. Found by running the binary
  against a real catalogue set, with a green test suite.

[0.1.0]: https://github.com/nolindnaidoo/i18n-le/releases/tag/crate-v0.1.0
