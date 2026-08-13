# Contributor and agent instructions

**Read [`crate/AGENTS.md`](../crate/AGENTS.md) before writing any code.** This
repository is crate-only — everything that is the product lives in
[`crate/`](../crate/) — and that file carries the engineering standard it is held
to: control flow, error handling, structure, the settled decisions and the
definition of done. [`crate/SPEC.md`](../crate/SPEC.md) defines the behaviour.
[AGENTS.md](../AGENTS.md) at the root routes between them; [CLAUDE.md](../CLAUDE.md)
is the short version: gates and traps.

This file exists only to point you there. It is deliberately thin: the standard
lives in one place so it cannot drift between tools.

## Non-negotiables

- **This tool has a verdict.** Most of the family extracts and holds no
  opinion; this one answers a yes-or-no question and **the exit code is the
  product**. There is nothing to grep and nothing to pipe onward.
- **Refuse rather than guess.** A construct the lexer cannot read, a locale it
  cannot name, a set spanning two directories — each is a refusal with a
  reason, never a fabricated answer. A test that passes by resolving something
  that should have been refused is the bug this family exists to prevent.
- Guard clauses first. **No statement-position `else`** — two branches are an
  early return, many are a `match`. Nesting stops at two levels; extract a
  named helper instead.
- **`Result<T, String>` is the error type.** No `anyhow`, no `thiserror` — the
  message *is* the documentation. No `clap` either: arguments are hand-parsed
  in `cli.rs`, and a test holds `FLAGS` equal to the flags named in `USAGE` so
  the help text cannot drift from the parser.
- **`unsafe` is forbidden crate-wide**, and a test is not an exemption.
- **No inline lint attribute** — not `#[allow]`, not `#[expect]`, in `src/` or
  in `tests/`. Fix the lint, or relax it visibly in `[lints.clippy]` or
  `[lints.rust]` in `crate/Cargo.toml` with its reason. The `policy` job greps
  for both spellings.
- **Nothing writes, and nothing reaches the network.** No `--fix`, no key
  insertion, no translation API.
- **Refusals speak both surfaces' vocabulary.** Every message out of
  `scan.rs`, `catalogue.rs` and `locale.rs` reaches a terminal *and* an agent,
  and only one of them has a command line.
- Dependencies are a cost. There are two; `crate/Cargo.toml` carries a comment
  for each one deliberately *not* taken. Justify any addition.
- **Never report success you did not achieve.** Comments explain **why**,
  never what.
- Commits are conventional (`fix:`, `feat:`, `docs:`…), imperative, and
  enforced by a hook and by CI.

## Before you commit

```bash
cd crate && cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test --locked
```

Coverage is a backstop, not a target: a per-module floor set well below where
the code actually is, and never raised to track it.
Every claim in a README, a help text or SPEC.md must be provable against the
code.

**Provable is about behaviour and numbers, not availability.** An install line
for a publish you are about to make is *staged*, not forbidden — write it, and
let the release commit be what makes it true.
