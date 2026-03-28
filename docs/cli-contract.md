# CLI contract

`patch` is a subcommand-only CLI for AI-agent-first code navigation. This document captures the public command surface and the output contract that the integration tests pin.

## Command families

The supported top-level commands are:

- `read`
- `symbol find`
- `symbol callers`
- `search text`
- `search regex`
- `files`
- `deps`
- `map`

There is no query-shorthand mode, no MCP runtime, and no editor/host install flow.

## Read selectors and flags

`read` supports these selectors:

- `--full`
- `--lines <start:end>`
- `--heading <heading>`
- `--key <PATH>`
- `--index <START:END>`

`--key` and `--index` are JSON-only selectors. Non-JSON targets with either selector return a parse error diagnostic.

## Scope ignore contract

When a command accepts `--scope <dir>`, patch may read one `.patchignore` file from that active scope root. only one .patchignore at the scope root is read.

patch does not look up parent directories for additional ignore rules, does not merge nested `.patchignore` files, and .gitignore is not read.

Traversal commands honor that file. Traversal commands honor that file: `files`, `symbol find`, `symbol callers`, `search text`, `search regex`, `deps`, and `map`.

Scope behavior stays explicit:

- launch the command with `--scope <dir>` pointing at the directory whose root owns the `.patchignore`
- root-relative ignore rules are evaluated against that scope root only
- `read` still accepts an explicit ignored path because it does not depend on traversal; read still accepts an explicit ignored path
- deps accepts an explicit ignored target path but filters traversal-derived results

## Shared JSON envelope

Every command supports `--json` and returns the same top-level shape:

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

### Envelope fields

- `command`: stable command identifier such as `read`, `symbol.find`, or `search.regex`
- `schema_version`: currently `2`
- `ok`: boolean success flag
- `data`: command-specific payload object
- `data.meta`: always-present shared metadata object; use `{}` when a command has no safe command-specific metadata to add
- `next`: always-present ordered list of high-confidence follow-up suggestions; use `[]` when there is no concrete next action
- `diagnostics`: ordered list of real recovery diagnostics

### `next` contract

Each `next` item uses this shape:

```json
{
  "kind": "suggestion",
  "message": "Read the full markdown section starting at line 7 with --heading",
  "command": "patch read \"README.md\" --heading \"## Why patch exists\"",
  "confidence": "high"
}
```

- `kind` is currently `suggestion`
- `message` is a human-readable action summary
- `command` is the concrete follow-up CLI command when one is known
- `confidence` is currently `high`

## Diagnostics contract

Diagnostics are shared across commands and use this shape:

```json
{
  "level": "hint",
  "code": "no_file_matches",
  "message": "no file matches found for \"*.definitely-nope\""
}
```

- `level` is one of `error`, `warning`, or `hint`
- `code` is a stable machine-readable identifier
- `message` is a human-readable explanation

High-confidence follow-up actions belong in top-level `next`, not in `diagnostics`.

Current behavior stays intentionally sparse:

- invalid command inputs aim to produce exactly 1 error diagnostic
- successful commands emit at most 2 warnings
- successful commands emit at most 1 hint

## Text output ordering

Dense text output is designed for agent loops and follows a stable section order:

1. summary header
2. `Meta`
3. `Evidence`
4. `Next`
5. `Diagnostics`

Empty `Meta`, `Next`, and `Diagnostics` sections render exactly as `(none)` when they have no entries.

Text errors use the same V2 structure on `stderr` and preserve the existing non-zero exit behavior.

Within the diagnostics section, entries are ordered by severity:

1. errors
2. warnings
3. hints

Example text error shape:

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

## Command-specific data

Each command stores its structured payload under `data`.

- `read`: `meta`, rendered content, path, and selector; `meta` includes `path`, `selector_kind`, `selector_display`, `file_kind`, `stability`, `noise`, and `heading_aligned`
- `symbol.find`: `matches`
- `symbol.callers`: `callers` and `impact`
- `search.text`: `matches`
- `search.regex`: `matches`
- `files`: `files`
- `deps`: `uses_local`, `uses_external`, `used_by`
- `map`: `entries`, `total_files`

### `read` selector contract

For `read`, `data.meta` and `data.selector` expose a stable selector contract.

- `selector_kind`: `full`, `lines`, `heading`, `key`, `index`, `key_index`
- `selector_display` uses raw selector text:
  - key: raw path (example: `users.0.accounts`)
  - index: raw range (example: `0:1`)
  - key+index: `<path> @ <range>` (example: `users.0.accounts @ 0:1`)

`data.selector` retains enum-tagged serialization variants:

- `{"Lines": {"start": 7, "end": 17}}`
- `{"Heading": {"value": "## Command families"}}`
- `{"Key": {"value": "users.0.accounts"}}`
- `{"Index": {"start": 0, "end": 1}}`
- `{"KeyIndex": {"value": "users.0.accounts", "start": 0, "end": 1}}`

JSON `read` content is TOON text. `file_kind` remains `structured_data` for JSON reads.

Stable JSON read error substrings callers can rely on:

- `invalid JSON`
- `missing key segment`
- `expected numeric array index`
- `expected index range in START:END format`
- `index range end must be greater than or equal to start`
- `index range starts at`
- `failed to encode JSON as TOON`

Traversal-derived command payloads exclude paths filtered by the active scope root `.patchignore`. That filtering affects discovered matches, callers, reverse dependencies, and map entries, but does not change the command names, shared flags, JSON envelope keys, or text section ordering.

### Markdown heading guidance rule for `read`

`read --lines` remains valid for markdown in general. patch adds a heading-oriented suggestion only when the first selected line itself is recognized as a markdown heading by the existing heading parser.

That means patch does **not** emit the heading suggestion when:

- a heading appears later in the selected range
- the selection starts inside section body text
- the selection starts before a heading

## Maintenance rule

If command names, JSON shape, diagnostics behavior, or text ordering changes, update this file, `README.md`, and the matching integration tests in `tests/` together.
