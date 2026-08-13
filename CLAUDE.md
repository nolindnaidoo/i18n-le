# CLAUDE.md

[AGENTS.md](AGENTS.md) is the technical source of truth for this repo. It is a
router: **this repository is crate-only**, so everything that is the product
lives in [`crate/`](crate/) and [`crate/AGENTS.md`](crate/AGENTS.md) is the
engineering standard the code is held to — control flow, error handling,
structure, the settled decisions, the definition of done. Read it before
writing code. [`crate/SPEC.md`](crate/SPEC.md) defines the product behaviour,
and [`crate/CLAUDE.md`](crate/CLAUDE.md) is the crate-side short version.

## Where to look

| Question | File |
|---|---|
| How should this code be written? | [`crate/AGENTS.md`](crate/AGENTS.md) — the standard, the architecture, the invariants |
| What is the tool supposed to do? | [`crate/SPEC.md`](crate/SPEC.md) — checks, refusals, exit codes |
| What does the user see? | [`crate/README.md`](crate/README.md) |
| What changed? | [CHANGELOG.md](CHANGELOG.md) · [`crate/CHANGELOG.md`](crate/CHANGELOG.md) |

## Gates

```bash
cd crate && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --locked
```

All three, exactly as CI runs them.

## Things that will bite you

- **The exit code is the product.** 0 clean — including no catalogues at all,
  because there is nothing to be wrong with. 1 findings. 2 a malformed
  question, and *no library identified* is malformed rather than clean. Never
  "improve" the no-catalogues case into a failure.
- **No translated value ever reaches an answer.** Keys and placeholder tokens
  only; the types are shaped so a translated string has nowhere to go.
- **Identification runs first, and everything is downstream of it.** Grammar,
  plural model, metadata rule and layout all come off the row `identify.rs`
  returns. `identify.rs` reads; `library.rs` decides what a reading means.
- **"Adding a library is a row" has limits.** A row is enough only when
  everything it needs exists already — a genuinely new syntax needs a `Mark`
  predicate in `message.rs`, a new manifest kind needs a reader in
  `identify.rs`. Anything else outside the table means the table is wrong.
- **Refuse rather than guess.** A construct the lexer cannot read, a locale it
  cannot name, a set spanning two directories: each is a refusal with a reason.
  A test that passes by resolving something that should have been refused is
  the bug this family exists to prevent.
- **`untranslated` is info, not an error, by default.** A string added to the
  source this morning is legitimately untranslated everywhere else, and a tool
  that broke the build for it gets switched off within a week.
- **No inline lint attribute** — `#[allow]` or `#[expect]`, in `src/` or
  `tests/`. The `policy` job greps for both spellings across both directories.
- **Coverage floors are a backstop, not a target** — 75% per module across the
  pure modules, listed by name in the `coverage` job so a rename turns it red
  rather than leaving it checking nothing. Well below where the code actually
  is, and never raised to track it.
- **CI narrows itself on a docs-only push.** `ci-crate.yml` fires on `*.md` and
  the agent instruction files, because the `policy` job greps them; every Rust
  job skips. Anything unrecognised counts as code and runs everything.
- **Every claim must be provable.** Nothing goes in a README, a help text or
  SPEC.md unless the code backs it. That governs **behaviour and numbers**, not
  **availability**: an install line for a publish you are about to make is
  **staged, not forbidden**. Write it, and let the release commit be what makes
  it true.
- **Nothing here is on crates.io yet.** `crate/Cargo.toml` and
  `crate/CHANGELOG.md` are the version's source of truth when it lands.
