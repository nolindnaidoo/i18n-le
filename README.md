<p align="center">
  <img src="https://raw.githubusercontent.com/nolindnaidoo/i18n-le/main/assets/icon.png" alt="i18n-LE logo" width="96" height="96"/>
</p>
<h1 align="center">i18n-LE</h1>
<p align="center">
  <b>Audit translation catalogues for missing keys, placeholder drift and structural mismatches</b><br/>
  <i>keys and tokens — never a translated string</i>
</p>

<p align="center">
  <a href="https://crates.io/crates/i18n-le">
    <img src="https://img.shields.io/crates/v/i18n-le?style=for-the-badge&label=Rust%20CLI&color=blue&logo=rust" alt="i18n-le on crates.io" />
  </a>
  <a href="https://crates.io/crates/i18n-le">
    <img src="https://img.shields.io/crates/d/i18n-le?style=for-the-badge&label=Downloads&color=blue" alt="crates.io downloads" />
  </a>
  <a href="https://github.com/nolindnaidoo/i18n-le/actions/workflows/ci-crate.yml">
    <img src="https://img.shields.io/github/actions/workflow/status/nolindnaidoo/i18n-le/ci-crate.yml?branch=main&style=for-the-badge&label=CI&color=blue&logo=githubactions&logoColor=white" alt="CI" />
  </a>
  <a href="https://github.com/nolindnaidoo/i18n-le/blob/main/crate/Cargo.toml">
    <img src="https://img.shields.io/badge/rustc-1.88+-blue?style=for-the-badge&logo=rust" alt="MSRV: Rust 1.88+" />
  </a>
  <a href="https://github.com/nolindnaidoo/i18n-le/blob/main/LICENSE">
    <img src="https://img.shields.io/badge/license-MIT-blue?style=for-the-badge" alt="MIT licensed" />
  </a>
  <a href="https://letools.dev/tools/i18n-le">
    <img src="https://img.shields.io/badge/LE%20Tools-letools.dev-blue?style=for-the-badge" alt="LE Tools" />
  </a>
</p>

---

<p align="center">
  <img src="https://raw.githubusercontent.com/nolindnaidoo/i18n-le/main/assets/demo.gif" alt="i18n-LE demo — the real binary, recorded by assets/demo.tape" style="max-width: 100%; height: auto;" />
</p>

> **Useful?** A star is how other developers find it —
> [★ GitHub](https://github.com/nolindnaidoo/i18n-le) ·
> [letools.dev/tools/i18n-le](https://letools.dev/tools/i18n-le)

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
and installation. It is not duplicated here — two copies of one long
document is the drift this family spends its gates preventing.

| | |
|---|---|
| What it does, in full | [`crate/README.md`](crate/README.md) |
| The behavioural contract | [`crate/SPEC.md`](crate/SPEC.md) |
| How the code is written | [`crate/AGENTS.md`](crate/AGENTS.md), and [AGENTS.md](AGENTS.md) for the repository around it |
| What changed | [CHANGELOG.md](CHANGELOG.md) · [`crate/CHANGELOG.md`](crate/CHANGELOG.md) |

## Install

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

## Options

Taken from `i18n-le --help`, which is the authority. The full reference,
with what each one is for, is in
[`crate/README.md`](crate/README.md).

| Option | What it does |
|---|---|
| `--system <name>` | Audit as a named library, skipping identification: `i18next`, `next-intl`, `vscode-l10n`, `flutter-arb` |
| `--source <path\|locale>` | The catalogue every other is measured against; auto-detected only when exactly one English candidate exists |
| `--keys-are-source` | The key is the English string, as in a VS Code `bundle.l10n.json` |
| `--fail-on <what>` | `untranslated`, or `any` — what else fails the run besides the findings that already do |
| `--strict` | Also fail when a catalogue could not be read or parsed |

## Documentation

| What | Where |
|---|---|
| What the tool is allowed to say — the checks, the refusals, the exit codes, the privacy boundary | [`crate/SPEC.md`](crate/SPEC.md) |
| How the code is written and held together — architecture, invariants, the gates | [`crate/AGENTS.md`](crate/AGENTS.md) |
| What the user sees — the full front page, install and all | [`crate/README.md`](crate/README.md) |
| What changed | [CHANGELOG.md](CHANGELOG.md) · [`crate/CHANGELOG.md`](crate/CHANGELOG.md) |
| The tool's page, and the other fifteen | [letools.dev/tools/i18n-le](https://letools.dev/tools/i18n-le) |

## More from the LE family

Sixteen single-purpose tools, one line: **get your data right before the
model sees it.** [letools.dev](https://letools.dev)

## License

MIT — see [LICENSE](LICENSE).
