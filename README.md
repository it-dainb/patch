# drail

**CLI-first code intelligence for AI agents.** drail gives agents a small, explicit command set for reading code, finding symbols, searching text, tracing callers, listing files, mapping a codebase, and checking file-level dependencies.

The product goal is simple: make code navigation transparent, predictable, and cheap enough that an agent can recover from a bad query without spiraling into tool thrash.

## Why drail exists

Generic shell tools force agents to compose too many steps:

- list files
- guess which file matters
- read too much
- grep again
- re-read a narrower slice

drail turns those loops into explicit commands with stable output contracts. The CLI is the product. There is no query-classification shorthand, no hidden mode switch, and no host/editor install flow to understand before using it.

## Command families

drail uses explicit subcommands only:

```bash
drail read <path>
drail symbol find <query>
drail symbol callers <query>
drail search text <query>
drail search regex <pattern>
drail files <pattern>
drail deps <path>
drail map
```

Every command supports:

- dense text output by default
- `--json` for a stable machine-readable envelope
- `--budget` to cap response size

Scope-aware commands also accept `--scope <dir>`.

## Scope ignore behavior

When you pass `--scope <dir>`, drail reads at most one `.drailignore` file from the active scope root itself. drail does not look in parent directories, and it does not merge multiple ignore files.

Traversal commands honor that scope-root `.drailignore`: `files`, `symbol find`, `symbol callers`, `search text`, `search regex`, `deps`, and `map`.

Supported launch syntax is unchanged: point `--scope` at the directory whose root contains the `.drailignore` file you want honored.

```bash
cargo run -- files "*.rs" --scope tests/fixtures/drailignore
```

In that example, only `tests/fixtures/drailignore/.drailignore` is read.

.gitignore is not read.

- read still works for ignored paths because it reads the explicit path you asked for.
- deps accepts an ignored target path but filters traversal-derived results such as discovered dependents.
- Root-relative rules stay anchored to the active scope root, so nested paths are not treated as parent-root matches.

## Quick start

```bash
cargo build

cargo run -- symbol find main --scope src
cargo run -- files "*.rs" --scope src
cargo run -- deps src/main.rs
cargo run -- map --scope src
```

## What each command is for

### `read`

Read a file in full, by line range, by markdown heading, or by JSON selectors.

```bash
cargo run -- read README.md --lines 7:17
cargo run -- read README.md --heading "## Command families"
cargo run -- read tests/fixtures/json/users.json --key users.0.accounts
cargo run -- read tests/fixtures/json/root-array.json --index 0:1
cargo run -- read tests/fixtures/json/users.json --key users.0.accounts --index 0:1
```

Use `read` when you already know the path and need exact content. Markdown `--lines` remains valid for arbitrary chunk reads; when the first selected line is itself a recognized heading, drail may also suggest a `--heading` follow-up to read the full section. JSON files always render as TOON text, including `--full` and selector reads. `--key` and `--index` are JSON-only selectors: `--key <PATH>` drills into a subtree using dot-separated object keys and numeric array segments, and `--index START:END` slices arrays with zero-based, end-exclusive bounds.

When content is minified, oversized, or parse-unreliable, `read` returns a bounded preview instead of dumping unreadable raw content and emits a `minified_fallback_used` warning diagnostic. This preview fallback is skipped when you explicitly ask for raw/targeted content via `--full` or an explicit selector (`--lines`, `--heading`, `--key`, or `--index`).

### `symbol find`

Find symbol definitions and usages with explicit kind filtering.

```bash
cargo run -- symbol find main --scope src
cargo run -- symbol find render --scope src/output --kind definition
```

Use `symbol find` when the target is code structure, not just matching text.

When structural parsing is skipped or unreliable (for example with minified or oversized input), `symbol find` falls back to usage-only matches with snippet text and emits `text_fallback_used`.

### `symbol callers`

Find call sites plus second-hop impact.

```bash
cargo run -- symbol callers render --scope src/output
```

Use `symbol callers` before changing a symbol that may affect downstream code.

When structural parsing is skipped or unreliable, `symbol callers` returns best-effort text fallback rows, uses `"<text-fallback>"` for `caller`, leaves `impact` empty for fallback-only results, and emits `text_fallback_used`.

### `search text`

Search literal text in comments, strings, docs, and code.

```bash
cargo run -- search text "symbol callers" --scope src
```

Use `search text` for exact phrases, docs, TODOs, or log strings.

### `search regex`

Search with an explicit regex command instead of slash-delimited magic.

```bash
cargo run -- search regex "symbol\\s+callers" --scope src
```

Use `search regex` when the match pattern is genuinely regular-expression based.

### `files`

Find files by glob.

```bash
cargo run -- files "*.rs" --scope src
cargo run -- files "*.rs" --scope tests/fixtures/drailignore
```

Use `files` to narrow the surface area before reading or searching.

### `deps`

Inspect what a file imports and what imports it.

```bash
cargo run -- deps src/main.rs
```

Use `deps` before moving, renaming, or heavily restructuring a file.

If the target path is explicit, `deps` still accepts it even when that path matches `.drailignore`, but any traversal-derived results continue to honor the scope-root ignore rules.

### `map`

Generate a compact structural map of a codebase.

```bash
cargo run -- map --scope src
```

Use `map` once when entering an unfamiliar repo, then switch to targeted commands.

## Output philosophy

drail is designed for agent recovery, not just happy-path demos.

### Text output

Text output is optimized for direct consumption in an agent loop:

1. summary header first
2. `Meta`
3. `Evidence`
4. `Next`
5. `Diagnostics`

Empty `Next` and `Diagnostics` sections render as `(none)`.

Text errors use the same V2 section layout and render on `stderr` with a non-zero exit status.

Diagnostics are ordered by severity:

- errors
- warnings
- hints

That ordering is stable across commands so agents can read the useful evidence before deciding whether a recovery hint matters.

### JSON output

JSON output uses a shared envelope across all commands:

```json
{
  "command": "symbol.find",
  "schema_version": 2,
  "ok": true,
  "data": {
    "meta": {}
  },
  "next": [],
  "diagnostics": []
}
```

Command-specific payloads live under `data`. `data.meta` is always present in V2, and top-level `next` is always present even when empty. Diagnostics contain real warnings, hints, or errors rather than placeholder success entries. See [`docs/cli-contract.md`](docs/cli-contract.md) for the exact contract.

#### Success example

```text
# symbol.find

## Meta
- definitions: 1
- noise: medium
- query: main
- scope: /abs/path/to/src
- stability: medium
- usages: 1

## Evidence
symbol find "main" in /abs/path/to/src — 2 matches

- main.rs:5-15 [definition]
  fn main() {

## Next
(none)

## Diagnostics
(none)
```

#### No-match example with `Next`

```bash
cargo run -- files "*.definitely-nope" --scope src --json
```

```json
{
  "command": "files",
  "schema_version": 2,
  "ok": true,
  "data": {
    "meta": {
      "files": 0,
      "noise": "low",
      "pattern": "*.definitely-nope",
      "scope": "/abs/path/to/src",
      "stability": "high"
    },
    "files": [],
    "pattern": "*.definitely-nope",
    "scope": "/abs/path/to/src"
  },
  "next": [
    {
      "kind": "suggestion",
      "message": "Try a broader or available file pattern for /abs/path/to/src",
      "command": "drail files \"*.rs\" --scope /abs/path/to/src",
      "confidence": "high"
    }
  ],
  "diagnostics": [
    {
      "level": "hint",
      "code": "no_file_matches",
      "message": "no file matches found for \"*.definitely-nope\""
    }
  ]
}
```

#### Error example

```bash
cargo run -- search regex "(" --scope src
```

```text
# search.regex

## Meta
(none)

## Evidence
(none)

## Next
(none)

## Diagnostics
- error: invalid query "(": regex parse error: … [code: invalid_query]
```

#### Markdown heading-aligned `read` example

```bash
cargo run -- read README.md --lines 7:17
```

```text
# read

## Meta
- file_kind: markdown
- heading_aligned: true
- noise: low
- path: README.md
- selector_display: 7:17
- selector_kind: lines
- stability: high

## Evidence
# README.md (11 lines, ~103 tokens) [section]

 7 │ ## Why drail exists
 8 │
 9 │ Generic shell tools force agents to compose too many steps:

## Next
- Read the full markdown section starting at line 7 with --heading (command: drail read "README.md" --heading "## Why drail exists"; confidence: high)

## Diagnostics
(none)
```

## Diagnostics and recovery

drail does not silently reinterpret user intent.

- Wrong selector? Return an error diagnostic.
- No matches? Return a sparse recovery hint plus a high-confidence `Next` suggestion when one is available.
- Probably meant a different command? Return a high-confidence suggestion only.

The CLI prefers explicit nudges over clever fallback behavior because predictable failures are easier for agents to recover from than magical behavior that changes across releases.

Current output limits are intentionally strict:

- at most 2 warnings
- at most 1 hint
- invalid command inputs aim to produce exactly 1 error diagnostic

## Agent workflow recommendations

For an unfamiliar codebase:

1. `map --scope src`
2. `files "*.rs" --scope src`
3. `symbol find <target> --scope src`
4. `symbol callers <target> --scope src` before signature changes
5. `read <path>` only after you know the exact file or section you need

For a likely text match rather than a symbol:

1. `search text`
2. `search regex` only if the literal search is too broad

For change planning:

1. `deps <path>`
2. `symbol callers <symbol>`

When a scoped traversal misses an expected path, check the scope root for `.drailignore` before assuming the file is unavailable.

## Installation

### Cargo

```bash
cargo install --path .
```

### Local installer

The repository ships a CLI-only installer that targets a user-local bin directory.

```bash
./install.sh --dry-run
./install.sh
```

The installer does not mutate editor settings, host configs, or external tool manifests.

## Build and test

```bash
cargo build --release
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

## Stability promises

drail aims to keep these surfaces stable:

- explicit subcommand names
- shared JSON envelope
- diagnostics schema
- text section ordering

What is intentionally *not* supported:

- legacy query-shorthand mode
- removed install hosts or editor-integration flows
- undocumented aliases or fuzzy flag spellings

## Maintainers and contributors

If you change command names, JSON shape, or diagnostic behavior, update:

- `README.md`
- `docs/cli-contract.md`
- relevant integration tests in `tests/`

## License

MIT
