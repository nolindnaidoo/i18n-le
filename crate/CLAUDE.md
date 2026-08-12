# Instructions for AI coding assistants

Read [AGENTS.md](AGENTS.md) first — it is the engineering-standards
document for this crate and the source of truth for layout, control-flow
style, the settled decisions, testing requirements, and the definition
of done. [SPEC.md](SPEC.md) defines the product behavior. AGENTS.md wins
on any conflict.

- Before declaring any change complete, run
  `cargo fmt --all --check`,
  `cargo clippy --all-targets -- -D warnings`,
  `cargo test --locked`. All three must pass.
- Never add inline `#[allow(...)]`. Fix the lint, or add a commented
  relaxation to `[lints.clippy]` in `Cargo.toml`. There are none today.
- **Identification is the front door, not a helper.** It runs first and
  everything else is downstream of its answer: the placeholder grammar,
  the plural model, which keys are metadata, the layout. If you find
  yourself writing a branch that carries on without a library — an
  inference, a default, a "best effort" — stop. That branch is what the
  rewrite deleted, and it was wrong in the way heuristics are wrong:
  confidently.
- **Two agreeing classes of evidence is the bar.** One is not. Two
  libraries identified is a refusal naming both; nothing identified is a
  refusal listing what was found. Do not add a tie-breaker.
- **Source files are read for identification only.** No finding may ever
  come from a `.ts`, `.dart` or `.vue`. Unused-key and undefined-key
  detection are non-goals — they make the tool language-coupled — and
  the boundary is written into SPEC.md so it cannot erode.
- **No translated value ever reaches an answer.** The enforcement is
  `Evidence` having no variant that can hold a sentence. Adding one that
  could is the one change this crate cannot take.
- **The exit code is the product.** 0 clean, 1 findings, 2 could not
  answer; no catalogues is 0. Do not "improve" that into a failure.
- **`untranslated` is `info`.** It must not fail a run by default. A
  string added to English this morning is legitimately untranslated
  everywhere else.
- **Refusals from `scan.rs`, `audit.rs` and `library.rs` may not name a
  flag.** They reach an MCP caller too, and a test asserts it.
  `identify.rs` and `cli.rs` may, because only a terminal reaches them.
- **Adding a library is a row in `library.rs`.** If it needs a change
  anywhere else except one new `Mark` predicate in `message.rs`, the
  table is wrong — fix the table, not the caller.
- **The pure modules are `library.rs`, `catalogue.rs`, `message.rs`,
  `locale.rs` and `audit.rs`**, they carry a 90% coverage floor, and CI
  lists them by name. Renaming one means updating
  `.github/workflows/ci-crate.yml` in the same change, or the job
  quietly checks nothing.
- `fixtures/` is the corpus both the audit tests and the MCP tests run
  against; changing it is a behaviour change and needs a CHANGELOG
  entry. **No real product name goes in a fixture** — this repository is
  public, and a test asserts it.
- Write regression tests for every bug you fix; keep unit tests free of
  clocks, randomness, and the filesystem outside `identify`/`scan`.
- **Run the binary, not only the tests.** The printf false-positive on
  Hungarian "90%-os" was green in a full suite and wrong against a real
  catalogue set.
