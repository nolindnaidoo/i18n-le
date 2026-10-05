# Changelog

All notable changes to i18n-LE will be documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file covers the **VS Code extension**. The Rust CLI in `crate/` is a
separate product on its own cadence and keeps its own
[CHANGELOG](crate/CHANGELOG.md). The entries below 1.0.0 describe this
repository while it held the CLI alone.

## [1.0.1] - 2026-10-04

### Fixed

- The Open VSX links and the Open VSX downloads badge in the README and the
  npm README pointed at a namespace the listing has left, so they led nowhere.
  The listing is under `nolindnaidoo` now, and so are they.

### Removed

- The Zed extension in `zed/`, with the CI job that built it and the workflow
  that synced it. It was never listed in Zed's registry.

## [1.0.0] - 2026-10-03

### Added

- **The VS Code extension.** `i18n-LE: Audit Catalogues` audits the set the
  open catalogue belongs to — or a folder picked in the Explorer — against the
  source locale, and opens a report naming each finding by file, key, kind and
  severity with its structural evidence. No translated string is ever shown.
- **Identification before anything is read**, as the crate does it: manifests,
  config files, the layout, the catalogue syntax and call sites, two agreeing,
  or a refusal naming what was found. `i18n-le.library` names the library and
  skips identification; `i18n-le.source` names the source catalogue.
- **The MCP server in the VSIX and on npm** as `i18n-le-mcp`: the same
  `check_catalogues` tool the Rust CLI serves, answering identically.
- **The engine is a port of the crate's**, held to it by the shared corpus, a
  differential that feeds both servers thousands of generated catalogue sets —
  broken JSON included, reported in serde_json's own words — a check that runs
  the extension's identification and the CLI's over generated project trees,
  and a check that both servers define the tool identically.
- Localized into twelve languages: the manifest and every runtime string.
- A Zed extension that runs the MCP server as a context server.

## [0.3.1] - 2026-08-14

The crate has shipped from this repository three times — 0.3.0 is the
one on crates.io — but the repository around it had never been
versioned, so everything below is its first record. The crate's own
behaviour changes are in [`crate/CHANGELOG.md`](crate/CHANGELOG.md).

### Added

- **A terminal demo** at [`assets/demo.gif`](assets/demo.gif), driving
  the real binary over the catalogues in
  [`assets/demo/`](assets/demo/). [`assets/demo.tape`](assets/demo.tape)
  is the `vhs` script that produced it, so `cd assets && vhs demo.tape`
  reproduces the recording rather than leaving an artifact nobody can
  regenerate. Both sit above `crate/`, where `cargo package` cannot
  reach them.

- **The repository has a front door.** `README.md`, `AGENTS.md`,
  `CLAUDE.md`, `CHANGELOG.md` and `LICENSE` at the root. This repository
  had none of them: the crate was complete and documented while the
  repository around it was five agent instruction files and a `crate/`
  directory. The MIT licence in particular was declared in
  `crate/Cargo.toml` and in the crate README's badge without the file
  existing anywhere in the tree.

  `README.md` routes to [`crate/README.md`](crate/README.md) rather than
  restating it. Two copies of a 349-line user-facing document is exactly
  the drift the fleet's gates exist to prevent, and the sibling repos
  that carry a full root README carry it because they have something at
  the root to describe.

### Changed

- **New icon artwork.** All sixteen tools were redrawn in one style, so
  the family reads as one set wherever the cards sit side by side. The
  framing is unchanged — the drawing fills 65.8% of an 800×800 canvas
  and every smaller size is derived from that one file rather than drawn
  again.

### Fixed

- **The README's images resolve away from GitHub.** They were repository
  paths, which crates.io and every other renderer resolves against its
  own origin, so the demo and the icon were broken everywhere this file
  is read that is not this repository. They are absolute URLs now.

- **The agent instruction files pointed at documents that did not
  exist.** All five named `AGENTS.md` and `CLAUDE.md` at the root; both
  were absent, so every link in every one of them was dead. They were
  rebuilt from `GEMINI.md` — the only one of the six that had been kept
  current — and now point at `crate/AGENTS.md`, `crate/SPEC.md` and
  `crate/CLAUDE.md`, each with the `../` its own directory needs. A gate
  in the `policy` job holds the six to one document and checks that
  every link resolves from where the file actually sits.

- **`ci-crate.yml` could not see the files its `policy` job guards.**
  The trigger admitted `crate/**` alone, while the job greps every
  `*.md` and holds the agent instruction files equal — so both gates
  could only run when the files they guard had *not* been touched. The
  trigger now admits documentation and the instruction files, and the
  Rust jobs skip themselves when nothing under `crate/` moved.

- **The coverage floor was a tripwire rather than a backstop.** 90% of
  lines per module, against real coverage that leaves single-digit
  headroom in the family's tightest module — one new branch from failing
  a build. It is 75% now, and documented as a backstop that is not
  raised to track actual coverage.
