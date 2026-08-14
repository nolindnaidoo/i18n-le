<h1 align="center">i18n-le</h1>

<p align="center">
  <b>Audit translation catalogues for missing keys, placeholder drift and structural mismatches</b><br/>
  <i>keys and tokens — never a translated string</i>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rustc-1.88+-93450a.svg" alt="MSRV: Rust 1.88+" />
  <img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="License: MIT" />
  <a href="https://crates.io/crates/i18n-le">
    <img src="https://img.shields.io/crates/v/i18n-le.svg" alt="crates.io" />
  </a>
  <a href="https://letools.dev">
    <img src="https://img.shields.io/badge/web-letools.dev-00A0FF.svg" alt="letools.dev" />
  </a>
</p>

---

<p align="center">
  <img src="https://raw.githubusercontent.com/nolindnaidoo/i18n-le/main/assets/demo.gif" alt="i18n-LE demo — the real binary, recorded by assets/demo.tape" style="max-width: 100%; height: auto;" />
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

## This repository

**Everything that is the product lives in [`crate/`](crate/)** — the Rust
CLI, the MCP server, the library table, the corpus and the tests. There
is no VS Code extension beside it and no npm package.

**[`crate/README.md`](crate/README.md) is the user-facing document**: what
it checks, how it works out which i18n library you use before it reads
anything, plurals, formats, options, output, running it as an MCP server,
and installation. It is not duplicated here — two copies of a 349-line
document is the drift this family spends its gates preventing.

| | |
|---|---|
| What it does, in full | [`crate/README.md`](crate/README.md) |
| The behavioural contract | [`crate/SPEC.md`](crate/SPEC.md) |
| How the code is written | [`crate/AGENTS.md`](crate/AGENTS.md), and [AGENTS.md](AGENTS.md) for the repository around it |
| What changed | [CHANGELOG.md](CHANGELOG.md) · [`crate/CHANGELOG.md`](crate/CHANGELOG.md) |

## Status

**Published on crates.io.** Identification, both surfaces, the corpus and
every test layer are built and green, and CI runs all of them. What is
deliberately still absent is listed under "Left for the owner" in
[`crate/AGENTS.md`](crate/AGENTS.md), so none of it reads as an oversight.

`crate/Cargo.toml` runs a patch ahead of the registry between releases —
that is a release waiting to be dispatched, not a mismatch. The badge
above reads the live version rather than restating one here.

## Install it

```bash
cargo install i18n-le
```

Or build from this repository:

```bash
cd crate
cargo build --release
```

The gates, exactly as CI runs them:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
```

## The LE family

Sixteen single-purpose tools, one line: **get your data right before the
model sees it.** [letools.dev](https://letools.dev)

## License

MIT — see [LICENSE](LICENSE).
