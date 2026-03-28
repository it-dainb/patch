# drail

Rust CLI for AST-aware code intelligence. The product is an explicit subcommand-based command line for AI agents first and humans second. It exposes stable text and JSON output for reading files, finding symbols, tracing callers, searching text/regex, listing files, mapping a repo, and checking file-level dependencies.

## Product position

- CLI-first, not MCP-first
- explicit commands only; no query-classification shorthand
- no edit flow
- no host/editor installer surface beyond the local binary installer in `install.sh`
- AI agents are the primary user, but human debugging ergonomics still matter

## Project structure

```text
src/
  main.rs              CLI entrypoint.
  cli/                 Clap types for top-level and nested subcommands.
  commands/            Thin command handlers that call engine code and render output.
  engine/              Typed command execution logic.
  output/
    json/              Shared JSON envelope + command-specific data renderers.
    text/              Dense text renderers with stable ordering.
  read/                File reading and outlining internals.
  search/              Symbol, callers, text, regex, files, and deps search logic.
  map.rs               Codebase map generation.
  types.rs             Shared structural types.
  error.rs             Error and exit-code handling.
tests/                 Integration and output-contract coverage.
install.sh             CLI-only installer into a user-local bin directory.
README.md              User-facing CLI docs.
docs/cli-contract.md   Public command/output contract.
```

## Public CLI surface

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

Shared flags:

- `--json`
- `--budget <tokens>`

Scoped commands additionally accept:

- `--scope <dir>`

Selector-specific flags:

- `read --lines <start:end>`
- `read --heading <heading>`
- `read --full`
- `symbol find --kind <definition|usage>`
- `map --depth <n>`

## Output contract

JSON uses a shared envelope:

```json
{
  "command": "files",
  "schema_version": 1,
  "ok": true,
  "data": {},
  "diagnostics": []
}
```

Diagnostics are objects with:

- `level`: `error` | `warning` | `hint`
- `code`: stable string
- `message`: user-facing string
- `suggestion`: optional string

Text output is ordered as:

1. summary header
2. `## Evidence`
3. `## Diagnostics`

Diagnostics must stay sorted by severity: error, then warning, then hint. Diagnostics render at the end of text output.

## Verification expectations

Primary verification commands:

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

When changing public output or CLI shape, also run the targeted integration tests that pin the contract, especially:

- `tests/output_contract.rs`
- command-specific tests under `tests/`

## Documentation sync rules

If you change any of the following, update both `README.md` and `docs/cli-contract.md` in the same change:

- command names or nesting
- shared flags
- JSON envelope keys
- diagnostics schema or limits
- text ordering guarantees

## Repository notes

- `install.sh` is a CLI-only local installer. Keep it idempotent and side-effect minimal.
- Avoid adding shorthand or compatibility aliases unless the plan explicitly calls for them.
- Prefer predictable failures with sparse guidance over hidden fallback behavior.
