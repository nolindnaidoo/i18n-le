# AGENTS.md — i18n-le

## What this is

**This repository is crate-only.** Everything that is the product lives
in [`crate/`](crate/): the Rust CLI, the MCP server, the library table,
the corpus and the tests. There is no VS Code extension beside it, no
`src/` at the root, and no npm package — so unlike the two-frontend
siblings in this family there is no parity contract to keep and nothing
here holds two implementations equal.

That makes this file a router rather than a standard of its own:

| Question | File |
|---|---|
| How should this code be written? | [`crate/AGENTS.md`](crate/AGENTS.md) — the engineering standard, the layout, the settled decisions, the testing requirements, the definition of done |
| What is this tool supposed to do? | [`crate/SPEC.md`](crate/SPEC.md) — the checks, the refusal rule, the exit codes, both surfaces |
| What does a user see? | [`crate/README.md`](crate/README.md) — the full user-facing document |
| What changed? | [CHANGELOG.md](CHANGELOG.md) for the repository, [`crate/CHANGELOG.md`](crate/CHANGELOG.md) for the published crate |

**`crate/AGENTS.md` wins on any conflict.** It is the source of truth
for anything inside `crate/`, which is everything that runs.

## Gates

```bash
cd crate
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
```

All three, exactly as CI runs them. A change is not done because it
compiles; it is done when it is tested, linted, documented where
behaviour changed, and honest — every claim in a README, a help text or
a spec must match the code.

## Things that will bite you

- **This tool has a verdict, and the exit code is the product.** Most of
  the family extracts and holds no opinion; this one answers a
  yes-or-no question. 0 is clean — including when there are no
  catalogues, because there is nothing to be wrong with. 1 is findings.
  2 is a malformed question, and *no library could be identified* is a
  malformed question rather than a clean answer. Do not "improve" the
  no-catalogues case into a failure.
- **No translated value ever reaches an answer.** Keys and placeholder
  tokens only. The types are shaped so a translated string has nowhere
  to go, and that is the design rather than a discipline.
- **Identification runs first and everything is downstream of it.** The
  grammar, the plural model, the metadata rule and the layout are all
  read off the row `identify.rs` returns; the audit cannot start without
  one. It is not a helper the audit consults when stuck.
- **`identify.rs` reads; `library.rs` decides what a reading means.**
  Adding a library is a row in the table *as long as everything it needs
  already exists*. A genuinely new syntax also needs one `Mark`
  predicate in `message.rs`; a new manifest kind needs a reader in
  `identify.rs`. Anything else outside the table means the table is
  wrong — `crate/CLAUDE.md` states this precisely, so "adding a library
  is a row" is not read as more than it claims.
- **Refuse rather than guess.** A construct the lexer does not read, a
  locale it cannot name, a set spanning two directories — each is a
  refusal with a reason, never a fabricated answer. A test that passes
  by resolving something that should have been refused is the bug this
  whole family exists to prevent.
- **Never add an inline lint attribute** — not `#[allow]`, not
  `#[expect]`, in `src/` or in `tests/`. The `policy` CI job greps for
  both spellings across both directories, because a gate that knows one
  of them is a gate that gets routed around by accident.
- **Coverage floors are a backstop, not a target.** 75% of lines per
  module across the pure modules — `library.rs`, `catalogue.rs`,
  `message.rs`, `locale.rs`, `audit.rs` — and the `coverage` job lists
  them by name, so renaming one turns the job red instead of quietly
  making it check nothing. The floor sits well below where the code
  actually is and is not raised to track it.
- **This repo shares its scaffolding with the other crate-only repos,
  not with the extension repos.** `.editorconfig`, `.gitattributes`,
  `.githooks/commit-msg`, `.github/dependabot.yml`,
  `.github/codeql-config.yml`, `codeql.yml` and
  `dependabot-auto-merge.yml` are byte-identical across the six, and
  `letools-site/scripts/check-fleet.ts` is what holds them there — run
  `bun run check:fleet ../` from a checkout of the site.

  Three things are **not** shared, each for its own reason:
  - `ci-crate.yml` and `release-crate.yml` are each repo's own. The
    crates stand on their own, and a job one needs and another does not
    is the point rather than a failure.
  - The agent instruction files are one document *within* a repo and
    never across them — each states its own tool's non-negotiables.
  - The extension-shaped files — `ci.yml`, `biome.json`,
    `tsconfig*.json`, `release.yml`, `zed-sync.yml` — do not exist here
    at all. Copying one across from a two-frontend sibling re-imposes a
    shape this repo does not have.
- **CI narrows itself on a docs-only push.** `ci-crate.yml` fires on
  `*.md` and the agent instruction files — it has to, because the
  `policy` job greps them. On such a push `policy` and `commits` run and
  every Rust job skips. Anything unrecognised, and an unreadable diff,
  counts as code and runs everything.
- **Run the binary, not only the tests.** A green suite has never been
  the whole answer anywhere in this family.

## Git and commits

Conventional, imperative, scoped to the files the change touches. Every
commit uses the GitHub noreply address
`13629544+nolindnaidoo@users.noreply.github.com` — a real address in
commit metadata is public forever and gets scraped. No AI attribution of
any kind: not a trailer, not a footer, not a comment. Commits are the
author's alone.

## Release

The crate publishes from `crate/` by **dispatching
`release-crate.yml`**, never by pushing a tag: a crates.io version can
never be reused, so the irreversible step is one a person chooses on
purpose, and the workflow refuses a version the registry already
carries. **It is on crates.io**; `crate/Cargo.toml` and
`crate/CHANGELOG.md` are the source of truth for what ships next, and
`crate/Cargo.toml` running ahead of the registry is a release waiting to
be dispatched rather than a mismatch.

## Known limitations (documented, not bugs)

Listed in full under "Left for the owner" in
[`crate/AGENTS.md`](crate/AGENTS.md), so none of it reads as an
oversight. In short: there is no VS Code extension and therefore no
parity script or shared corpus; nothing tags the commit a release came
from; formats beyond JSON and ARB are a documented deferral rather than
a plan; plural-category completeness needs a CLDR rule set; the library
table will need rows; and the call-site scan is substring matching
rather than parsing, which is enough to vote and cheap enough to run.
