# i18n-le-mcp

<p align="center">
  <a href="https://marketplace.visualstudio.com/items?itemName=nolindnaidoo.i18n-le">
    <img src="https://img.shields.io/badge/Install%20from-VS%20Code-blue?style=for-the-badge&logo=visualstudiocode" alt="Install from VS Code Marketplace" />
  </a>
  <a href="https://open-vsx.org/extension/nolindnaidoo/i18n-le">
    <img src="https://img.shields.io/open-vsx/dt/nolindnaidoo/i18n-le?style=for-the-badge&label=Open%20VSX&color=blue" alt="Open VSX downloads" />
  </a>
  <a href="https://www.npmjs.com/package/i18n-le-mcp">
    <img src="https://img.shields.io/npm/v/i18n-le-mcp?style=for-the-badge&label=MCP%20server&color=blue&logo=npm" alt="i18n-le-mcp on npm" />
  </a>
  <a href="https://letools.dev/tools/i18n-le">
    <img src="https://img.shields.io/badge/LE%20Tools-letools.dev-blue?style=for-the-badge" alt="LE Tools" />
  </a>
</p>

An [MCP](https://modelcontextprotocol.io) server that audits a set of
translation catalogues against one of them — missing and extra keys,
placeholders dropped or renamed in translation, constructs from another i18n
convention, empty values, keys defined twice, and a path that is an object in
one locale and a string in another — the audit engine behind the
[i18n-LE](https://letools.dev/tools/i18n-le) editor extension, exposed as a
tool an agent can call.

**Only key names and structural facts come back — never a translated string.**
An agent asking whether Spanish is complete does not need the Spanish: a
renamed placeholder is proved by its tokens, and an untranslated string by the
fact that it is byte-identical to the source.

**The library is named, never guessed.** Which library wrote a catalogue
decides what `{name}` means — a placeholder in next-intl, literal text in
i18next — and working that out needs manifests, config files and call sites,
none of which this server can see. The caller names it, or gets a refusal.

No dependencies, no network calls, no filesystem access. Content goes in,
structured results come out.

## Use it

Point any MCP host at `npx i18n-le-mcp`.

**Claude Code**

```bash
claude mcp add i18n-le -- npx -y i18n-le-mcp
```

**Anything with a JSON config** — Cursor, Windsurf, Claude Desktop:

```json
{
  "mcpServers": {
    "i18n-le": {
      "command": "npx",
      "args": ["-y", "i18n-le-mcp"]
    }
  }
}
```

**VS Code** needs nothing here. Install the extension instead — it
carries this server and registers it for you:
[VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=nolindnaidoo.i18n-le)
· [Open VSX](https://open-vsx.org/extension/nolindnaidoo/i18n-le)

**No Node?** The same `check_catalogues` tool ships in a static Rust binary:
`cargo install i18n-le`, then `i18n-le mcp`
([crates.io](https://crates.io/crates/i18n-le)). The two servers answer
identically — one corpus runs against both, and a differential test feeds both
thousands of generated catalogue sets, broken ones included, and compares every
answer. **This server reads no files.**

Prefer a global install to `npx` on every launch:

```bash
npm install -g i18n-le-mcp
```

```json
{
  "mcpServers": {
    "i18n-le": { "command": "i18n-le-mcp" }
  }
}
```

No environment variables, no API key, no configuration of its own. To check it
before wiring it into anything:

```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | npx -y i18n-le-mcp
```

If that prints the tool name, the server works.

## The tool

### `check_catalogues`

| argument | type | |
|---|---|---|
| `library` | string | **required.** Which library wrote the catalogues: `i18next`, `next-intl`, `vscode-l10n` or `flutter-arb`. It supplies the placeholder grammar, the plural model and which keys are metadata. |
| `files` | object[] | **required.** The catalogues, each `{ path, content }` with an optional `locale`. JSON, nested or flat. A locale left out on every file is read from the names together. |
| `source` | string | Which catalogue is the contract — a path or a language tag. Without it, exactly one English candidate must exist or the audit is refused. |
| `keysAreSource` | boolean | The key is itself the English string, as in a VS Code `bundle.l10n.json`. |
| `maxResults` | number | Default `500`, ceiling `5000`. |

Findings name the file and the key, and carry only structural evidence —
tokens, counts, styles, shapes, occurrence counts, or a construct and its byte
offset:

```json
{
  "ok": true,
  "data": {
    "schema": 3,
    "status": "findings",
    "system": { "library": "i18next", "version": null, "layout": null, "keysAreSource": false, "evidence": [] },
    "source": { "path": "en.json", "locale": "en", "keys": 2 },
    "files": [
      { "path": "en.json", "locale": "en", "keys": 2 },
      { "path": "es.json", "locale": "es", "keys": 1 }
    ],
    "findings": [
      { "severity": "error", "kind": "missing-key", "file": "es.json", "key": "nav.home" },
      {
        "severity": "error",
        "kind": "placeholder-name-mismatch",
        "file": "es.json",
        "key": "metrics.window",
        "sourceTokens": ["timeframe"],
        "targetTokens": ["periodo"]
      }
    ],
    "diagnostics": [],
    "summary": { "files": 2, "findings": 2 }
  },
  "diagnostics": [],
  "meta": { "tool": "check_catalogues", "count": 2, "truncated": false }
}
```

A catalogue that will not parse is named in `diagnostics` with the parser's
reason and position, and the rest are still audited. `ok` means the audit ran,
not that the answer was yes.

## Also in the MCP registry

`io.github.nolindnaidoo/i18n-le` —
[registry.modelcontextprotocol.io](https://registry.modelcontextprotocol.io)

## Eleven more like it

One tool each, same shape: content in, structured data out, no network and no
filesystem. Every one is on npm as `<name>-mcp` and in the MCP registry as
`io.github.nolindnaidoo/<name>`.

| Package | Tool | Does |
|---|---|---|
| [`urls-le-mcp`](https://www.npmjs.com/package/urls-le-mcp) | `extract_urls` | URLs, with protocol and position |
| [`colors-le-mcp`](https://www.npmjs.com/package/colors-le-mcp) | `extract_colors` | colors from stylesheets and code |
| [`dates-le-mcp`](https://www.npmjs.com/package/dates-le-mcp) | `extract_dates` | dates and timestamps |
| [`numbers-le-mcp`](https://www.npmjs.com/package/numbers-le-mcp) | `extract_numbers` | numeric values |
| [`paths-le-mcp`](https://www.npmjs.com/package/paths-le-mcp) | `extract_paths` | file and directory paths |
| [`string-le-mcp`](https://www.npmjs.com/package/string-le-mcp) | `extract_strings` | string values |
| [`regex-le-mcp`](https://www.npmjs.com/package/regex-le-mcp) | `extract_patterns` | regexes, with a ReDoS verdict |
| [`secrets-le-mcp`](https://www.npmjs.com/package/secrets-le-mcp) | `detect_secrets` | credentials, masked — never the value |
| [`envsync-le-mcp`](https://www.npmjs.com/package/envsync-le-mcp) | `compare_env_files` | dotenv key drift, names only |
| [`scrape-le-mcp`](https://www.npmjs.com/package/scrape-le-mcp) | `analyze_robots_txt` | whether a path may be crawled |
| [`unicode-le-mcp`](https://www.npmjs.com/package/unicode-le-mcp) | `detect_unicode_risks` | Unicode that hides meaning, as codepoints |

Every tool in the family, one page: **[letools.dev](https://letools.dev)**

## Built by

**[Nolin Naidoo](https://nolindnaidoo.com)** — Chief Engineer, AI/ML & Platform
Architecture. [nolindnaidoo.com](https://nolindnaidoo.com) ·
[GitHub](https://github.com/nolindnaidoo) ·
[LinkedIn](https://www.linkedin.com/in/nolindnaidoo/)

### Also from the same workshop

Twelve Rust tools built the same way: small, single-purpose, and driven by a
machine rather than a person. pixelcoords and pixelactions make up one loop —
pixelcoords answers *where*, pixelactions *acts* there. The ten LE crates are
the terminal half of the extensions they sit in: the same detection, held to
the extension's own corpus, and an exit code instead of a results editor.

| | | |
|---|---|---|
| **[pixelcoords](https://github.com/nolindnaidoo/pixelcoords)** | Freeze your screen, mark regions, get pixel-exact coordinates and crops | [site](https://pixelcoords.dev) · [crates.io](https://crates.io/crates/pixelcoords) · [docs.rs](https://docs.rs/pixelcoords) |
| **[pixelactions](https://github.com/nolindnaidoo/pixelactions)** | Consume human-verified coordinates, perform the interaction, confirm it landed | [site](https://pixelactions.dev) · [crates.io](https://crates.io/crates/pixelactions) · [docs.rs](https://docs.rs/pixelactions) |
| **[paths-le](https://github.com/nolindnaidoo/paths-le/tree/main/crate)** | Find every path in a codebase and report whether it still points at anything | [crates.io](https://crates.io/crates/paths-le) |
| **[secrets-le](https://github.com/nolindnaidoo/secrets-le/tree/main/crate)** | Find hardcoded credentials, and never print one | [crates.io](https://crates.io/crates/secrets-le) |
| **[urls-le](https://github.com/nolindnaidoo/urls-le/tree/main/crate)** | Extract every URL from a codebase, with its protocol and exact position | [crates.io](https://crates.io/crates/urls-le) |
| **[regex-le](https://github.com/nolindnaidoo/regex-le/tree/main/crate)** | Find every regex in a codebase and report which can be driven into catastrophic backtracking | [crates.io](https://crates.io/crates/regex-le) |
| **[string-le](https://github.com/nolindnaidoo/string-le/tree/main/crate)** | Get every string in a codebase out where a person can read them | [crates.io](https://crates.io/crates/string-le) |
| **[numbers-le](https://github.com/nolindnaidoo/numbers-le/tree/main/crate)** | Find every hardcoded number in a codebase so a person can check them | [crates.io](https://crates.io/crates/numbers-le) |
| **[envsync-le](https://github.com/nolindnaidoo/envsync-le/tree/main/crate)** | Compare the dotenv files in a tree and say which keys are missing from which | [crates.io](https://crates.io/crates/envsync-le) |
| **[colors-le](https://github.com/nolindnaidoo/colors-le/tree/main/crate)** | Find every colour in a codebase, and say which are not in your palette | [crates.io](https://crates.io/crates/colors-le) |
| **[dates-le](https://github.com/nolindnaidoo/dates-le/tree/main/crate)** | Extract every date and timestamp, and the exact instant each one resolves to | [crates.io](https://crates.io/crates/dates-le) |
| **[scrape-le](https://github.com/nolindnaidoo/scrape-le/tree/main/crate)** | Check whether a page is scrapeable before the scraper is written | [crates.io](https://crates.io/crates/scrape-le) |

## Licence

MIT © [Nolin Naidoo](https://nolindnaidoo.com)
