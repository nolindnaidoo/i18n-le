# i18n-le (CLI) — engineering standards

This is the source of truth for how code in `crate/` is written, tested,
and reviewed. It applies to every contributor, human or AI-assisted.
[SPEC.md](SPEC.md) defines the product behavior — the checks, the
refusal rule, the exit codes, both surfaces; this file is how the code
gets there. **This file wins on any conflict.**

## What this project is

Audit a set of translation catalogues against one of them and say what
is structurally wrong: keys a locale is missing or has too many of,
placeholders dropped or renamed in translation, values left empty, keys
defined twice, and a path that is an object in one locale and a string
in another.

**This one has a verdict.** Most of the family extracts and holds no
opinion; this answers a yes-or-no question and the exit code is the
product. There is nothing to grep and nothing to pipe onward.

**Status: v0.3.0 core.** Identification, both surfaces, the corpus and
the test layers below are built and green. It has not been through a
hardening pass — see "Left for the owner" at the end.

## Layout

```
crate/src/
├── library.rs    the table: what each i18n library *is*, as data
├── identify.rs   the front door — evidence in, a library out
├── layout.rs     the file set, once the library is known
├── catalogue.rs  the duplicate-preserving JSON reader
├── message.rs    reading one message under a *known* grammar
├── locale.rs     language tags
├── audit.rs      the checks
├── scan.rs       identify → read → audit; the only path either
│                 surface calls
├── cli.rs        the terminal surface
└── mcp/          the agent surface
```

**The pure modules are `library.rs`, `catalogue.rs`, `message.rs`,
`locale.rs` and `audit.rs`.** They take text and return findings; a
`std::fs` call in any of them is a bug. They carry the **90% line
coverage floor per module**, enforced by the `coverage` job — which
lists them by name, so renaming one turns the job red instead of quietly
making it check nothing.

- **`identify.rs`, `layout.rs` and `scan.rs` are the only modules
  allowed to touch the filesystem.** Everything else tests from the
  corpus — no temp directories, no flake.
- **`identify.rs` answers *which library*; `layout.rs` answers *which
  files*.** The dependency runs one way — `identify.rs` calls into
  `layout.rs`, never the reverse — including for the layout class of
  evidence, which is nothing more than "these files read cleanly under
  this shape".
- **Identification runs first and everything is downstream of it.** It
  is not a helper the audit consults when stuck; the audit cannot start
  without an answer, because the grammar, the plural model, the metadata
  rule and the layout are all read off the row it returns.
- **Both surfaces are one implementation.** `cli.rs` and `mcp/` both
  call `scan::report_for`. A surface that grows its own copy of a rule
  is a bug, and a contract test asserts the two return identical reports
  for the same catalogues.
- **`library.rs` holds the tables and none of the I/O.** Adding a
  library is a row there, as long as everything it needs already exists.
  A genuinely new syntax also needs one `Mark` predicate in
  `message.rs`; a new `Manifest` kind needs a reader in `identify.rs`; a
  new `Shape`, `Plurals`, `Metadata` or `Interpolation` variant breaks
  the build at every `match` that has to decide about it, which is the
  design working. Anything *else* outside the table means the table is
  wrong. `CLAUDE.md` lists this precisely.
- **`identify.rs` reads; `library.rs` decides what a reading means.**
- Keep modules flat. No layers, registries, managers, or services. No
  trait with a single implementation.

## Decisions already made (do not relitigate)

- **No translated value ever reaches an answer**, and the *types* are
  what stop it. `Evidence` has six variants — tokens, counts, styles,
  shapes, occurrences, and a construct with a byte offset — and none of
  them can hold a sentence. Adding one that could is the single change
  this crate cannot take. Three tests enforce it: over the comparator,
  over the MCP surface, and over stdout and stderr of a real run.
  **`Evidence` is the enforced half, not the whole boundary.** A
  `Signal`'s `detail`, a `Diagnostic`'s `message` and a `FileSummary`'s
  `path` are free-form strings that nothing but review stops carrying a
  value; they are built from package names, marks, fixed call
  substrings, file names and parser positions, and must stay that way.
- **Keys are the deliberate exception**, including under
  `--keys-are-source` where the key is itself the English string. A
  finding that will not name its key is not a finding. This is stated in
  SPEC.md rather than hidden.
- **The library is identified from evidence, and two agreeing classes
  is the bar.** Not one. A decisive content signature is the only thing
  that identifies alone, and it is reserved for syntax no other library
  writes.
- **Two libraries identified is a refusal naming both.** Never pick one.
- **Nothing identified is a refusal listing what was found.** There is
  no fallback that guesses — guessing is what v0.3 deleted.
- **`--system` is required, not a convenience.** A translations-only
  repository has no manifest, and this family's own `l10n/` bundles
  depend on no package — the mechanism is the editor.
- **Source is read for identification only.** No finding may ever come
  from a source file. If you find yourself reaching into `.ts` to
  produce a finding, stop: unused/undefined-key detection is a non-goal
  and the boundary is written into SPEC.md so it cannot erode.
- **A construct from another convention is a `convention-mismatch`
  finding**, because with the library known it is a defect rather than a
  mystery — the loader renders it verbatim. Anything unrecognised stays
  literal text.
- **One version gate, naming a real break.** i18next below v23 wrote
  `key_plural`. An unfamiliar *newer* major is read and reported, not
  refused: an allowlist made the tool brittle and identification is
  corroborated by content anyway.
- **Plural-category completeness is deliberately not checked.** i18next
  key suffixes are folded onto a base key (which fixes a v0.1 false
  positive on Polish `_few`), and an ICU plural is one token named for
  its argument. Asserting a locale carries the categories CLDR requires
  needs a versioned per-locale rule set this does not carry.
- **Manifest paths in the report are relative to the catalogues.** An
  absolute path makes every cross-machine diff a difference.
- **The lexer honours `{{`, `}}` and `%%` as literals.** Without that it
  manufactures findings in every ICU file.
- **printf is read narrowly** — `%s`, `%d`, `%1$s` only. Flags, width
  and precision match ordinary prose: Hungarian "90%-os" is `%` + the
  left-justify flag + the octal conversion. Found by running the binary
  against a real 25-locale set, not by reading the code.
- **The source is one named locale, never the union.** A union makes one
  locale's typo'd key a requirement every other locale is then missing.
- **Auto-detection answers or refuses.** Exactly one `en*`-or-untagged
  candidate, or exit 2 naming the fix.
- **A locale is read from what the names in a set do not share.** Not
  from each name alone — `nls` passes for a language tag on its own.
  Anything left over that is not shaped like a tag is refused, which is
  what stops a repository root being audited as a catalogue set.
- **One directory is one catalogue set.** A run spanning two is exit 2.
- **`untranslated` is `info` and does not fail by default.** A string
  added to English this morning is legitimately untranslated everywhere
  else, and a tool that broke the build for it gets switched off.
- **The report carries no timestamp and a `schema` version.** A report
  is a thing to diff; a clock makes every run differ from every other.
  `schema` is 3.
- **`system.evidence` is in the report.** An identification nobody can
  check is a heuristic with better manners.
- **Every reported path is relative to the catalogues.**
- **`severity` is on findings from day one.** Adding it later changes
  every consumer's exit code.
- **A catalogue that will not parse is excluded, not half-read.** Unlike
  a dotenv file, half a JSON document is not evidence.
- **stdout is protocol, stderr is human, and there is no `--json`.**
- **One crate, self-contained.** No published `-core`, no shared crate,
  and nothing holding this code equal to the similar files in the
  sibling repos.

## Control-flow style

Flat over nested, guards over branches — the same rules as pixelcoords,
pixelactions, scrape-le and envsync-le:

- **No statement-position `else`.** Guard clauses and early `return`
  (`if !ok { return ... }` / `let Some(x) = ... else { return }`), then
  fall through to the happy path.
- **Value-position `if/else` is fine** — `let x = if cond { a } else
  { b }` is Rust's ternary.
- **`match` is fine and preferred** over any chain of condition tests on
  the same value; use match guards instead of `if/else` inside arms.
- Prefer combinators where they read cleanly: `bool::then_some`,
  `Option::map/filter/is_some_and`, `?`.
- No nesting deeper than two levels inside a function; extract a named
  helper instead.

## Hard rules

- **No inline `#[allow(...)]` or `#[expect(...)]`, in `src/` or in
  `tests/`.** Either fix the lint or add a visible, commented relaxation
  to `[lints.clippy]` or `[lints.rust]` in `Cargo.toml`. There are none
  today, and adding the first one needs a reason in the comment. The
  `policy` job greps for both spellings across both directories,
  because a gate that knows one of them is a gate that gets routed
  around by accident.
- **Clippy `all` + `pedantic`, deny warnings.** `cargo clippy
  --all-targets -- -D warnings` must pass exactly as written.
- **No `anyhow`, no `thiserror`.** Fallible functions return
  `Result<T, String>`, and the string is the message a person reads.
- **No `clap`.** Arguments are hand-parsed in `cli.rs`, and `FLAGS` is
  held equal to the flags named in `USAGE` by a test — so the help text
  cannot drift from the parser.
- **`unsafe` is forbidden crate-wide** (`[lints.rust]`).
- **No async runtime.** This tool reads files and compares strings.
- **Dependencies are a cost.** There are two, `serde` and `serde_json`,
  and `Cargo.toml` carries a comment for each dependency deliberately
  *not* taken. `pubspec.yaml` is read by a line scan rather than a YAML
  crate, for the same reason. Justify any addition; prefer the standard
  library.
- **No network, ever.** No translation API, no machine translation.
- **Nothing writes.** No `--fix`, no key insertion.
- **Strict parsing, never silent defaults.** A typo'd `--surce` that
  quietly did nothing would auto-detect a different reference and report
  a clean set that was never checked against the contract asked for.
- **Refuse rather than guess.** A construct the lexer does not read, a
  locale it cannot name, a set spanning two directories — all refusals
  with a reason, never a fabricated answer.
- **Refusals speak both surfaces' vocabulary.** Every message from
  `scan.rs`, `catalogue.rs` and `locale.rs` reaches a terminal *and* an
  agent, and only one of them has a command line. A test asserts no
  message from that shared layer contains `--`. Only `cli.rs`,
  `identify.rs` and `layout.rs` may name a flag, and only because the
  MCP surface enters at `scan::report_for` and so never reaches them.

## The corpus contract

`fixtures/` lives inside this crate so the published package is
self-contained — `cargo package` cannot reach above its own directory.
The corpus is not needed to *build* the binary; it is needed to
*verify*. `cargo test` on the published crate runs every case, so a
consumer can check the refusal claims in the README instead of trusting
them.

- `fixtures/documents/` — one matched set per JSON shape, each carrying
  a planted instance of every finding kind; one file per supported
  grammar and one per unsupported construct, pinning **refusal** rather
  than mismatch; plus BOM, CRLF, escaped-brace, duplicate-key and
  structure edge cases.
- `fixtures/detection.json` — parsing, locale naming, grammar
  classification and whole audits, as expected values.
- `fixtures/mcp-check-catalogues.json` — the MCP envelope, case by case.

Changing a document or an expectation is a behaviour change and needs a
CHANGELOG entry. A test asserts no corpus **answer** carries a
translated value.

## Testing

- **The pure modules are unit-tested from text alone.** `library.rs`,
  `catalogue.rs`, `message.rs`, `locale.rs` and `audit.rs` take text and
  return findings; if something is hard to test there, the design is
  wrong.
- **Every refusal has a test**, and so does every identification path:
  two classes, a decisive signature alone, a conflict, nothing at all,
  the override, and the version gate.
- **The corpus runs the same documents under different libraries.**
  `fixtures/detection.json` pins `icu.en.json`/`icu.es.json` twice —
  clean under next-intl, `convention-mismatch` under i18next. That pair
  is the whole architecture in one test.
- **Identification is tested against temporary trees in `identify.rs`**,
  because it and `layout.rs` are the only pure-logic-plus-filesystem
  modules and the second is only ever reached through the first.
  Everything they decide is data in `library.rs` and unit-tested there.
- **Exit codes belong in `tests/contracts.rs`.** They are the API —
  callers branch on them — so they are pinned by tests that drive the
  built binary against a temporary directory. A new refusal adds its
  case there.
- **Anything needing a set larger than an editor opens is
  `tests/scenarios.rs`**, gated behind `I18N_LE_SCENARIOS`. A skipped
  scenario is never reported as a pass; each one says plainly that it
  did not run.
- **Every bug fix ships with a regression test** that fails before the
  fix. The printf narrowing above is one: it was green in the suite and
  wrong against a real catalogue. **Run the binary, not only the tests.**
- Tests are deterministic: no clocks, no randomness, and **no filesystem
  in a pure module's tests**.

## Verification — the definition of done

All of it, before every push:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
I18N_LE_SCENARIOS=1 cargo test --test scenarios   # before a release
```

A change is not done because it compiles; it is done when it is tested,
linted, documented where behaviour changed (README / CHANGELOG / SPEC /
this file), and honest — claims in docs must match the code.

## Left for the owner

Named here so none of it reads as an oversight:

- **No VS Code extension**, so no detection-parity script and no shared
  corpus with a second frontend. `fixtures/` is currently a contract
  between this crate and itself, and `ci-crate.yml` says in its own
  header that the `parity` and `differential` jobs are absent rather
  than vacuous until one exists.
- **No `crate-v*` tagging convention.** `release-crate.yml` is
  dispatch-only and reads the version from `Cargo.toml`, so nothing tags
  the commit that was released.
- **Nothing checks the file kind before reading it.** The call-site scan
  refuses a source file over 512 KB but not a FIFO named `x.ts`, which
  would block the read for as long as nothing writes to it. The
  siblings' `tests/hazards.rs` is where that class of input belongs and
  this has no equivalent.
- **A library declared in a manifest kind nothing reads yet.** Adding
  one is a reader in `identify.rs` beside `npm_signals` and
  `pubspec_signals`, with `declared_version` and `check_version`
  following. Stated above and in `CLAUDE.md` so "adding a library is a
  row" is not read as more than it claims.
- **Formats beyond JSON/ARB** are a documented deferral, not a plan. A
  library row can now *describe* one; the parser is still the work.
- **Plural-category completeness**, which needs a CLDR rule set.
- **The library table will need rows.** vue-i18n, lingui, gettext, Rails
  and Angular are all absent, and each is a row plus — where the syntax
  is genuinely new — one `Mark` predicate. Nothing checks the table
  against a registry, by design, since this makes no network calls.
- **The call-site scan is substring matching, not parsing.** It is
  enough to vote and cheap enough to run; it will miss an aliased import
  and it will match a string in a comment. Both are survivable because
  identification needs corroboration.
