# Changelog

The i18n-le repository. The crate keeps its own history in
[`crate/CHANGELOG.md`](crate/CHANGELOG.md); this file covers the
repository around it.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
