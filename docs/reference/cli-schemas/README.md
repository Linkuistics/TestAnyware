# CLI JSON Schemas

JSON Schemas (Draft 2020-12) for the `--json` output of every
`testanyware` subcommand. Authoritative for the Rust port.

The contract that defines how these schemas are versioned, what the
error envelope looks like, and the per-command coverage list lives in
[../../architecture/cli-design-contract.md](../../architecture/cli-design-contract.md).

## Status

These are **stubs** during the Rust port. Each file declares only
`schema_version` and a `$comment: "TODO"` marker. They become real
schemas as commands land in the Rust binary.

The directory exists from day one so:

- The schema tree stays parallel to the command tree as it grows.
- `testanyware schema <command>` (see contract §8.2) has a path to
  `include_str!` from build time, even when the underlying command
  has not been ported yet.
- CI can fail if a port adds a command without a corresponding schema.

## File naming

`<schema-id>.json` where `<schema-id>` matches the table in contract
§3.1. Use kebab-case.

## What every schema MUST include

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://testanyware.dev/schemas/<schema-id>.json",
  "title": "<Command human title>",
  "type": "object",
  "required": ["schema_version", "ok"],
  "properties": {
    "schema_version": { "type": "string" },
    "ok":             { "type": "boolean" }
  }
}
```

Plus per-command fields. Add new optional properties freely between
major versions; renaming or removing requires a major version bump per
contract §3.2.

## Error envelope

When a command fails in `--json` mode, the output validates against
[`error.json`](error.json) instead of the per-command schema. See
contract §3.4 for the envelope shape.
