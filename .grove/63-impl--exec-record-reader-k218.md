# exec-record-reader-k218

## Goal

The Host CLI reads the typed exec record from an agent that writes one. It
keeps every legacy output for an agent that does not. This leaf is
candidate A.

## Context

- Spec: sections "Exec record" (with "Validity"), "Host CLI semantics" and
  "Compatibility".
- ADR-0023.
- Crates: `cli-rs/crates/testanyware-protocol`, `testanyware-cli` and
  `testanyware-agent-client`.
- Fixtures: the new Linux replies from leaf k217. Legacy macOS, legacy Linux and
  Windows replies come from the retired diagnostic evidence.

## Done when

- The protocol crate holds the record types as optional fields.
  `schema_version` stays 1.0.
- The validity rules reject each contradiction that the spec lists. They
  accept both leader orders and `leaderStatus` `unavailable`. They derive
  `withinBounds`.
- A guest outcome never sets `ok` to false.
- JSON mode adds `outcome` and `lifecycle`, including
  `lifecycle.finalisedAfterMs`. Text mode does not change.
- The Host CLI detects an old agent by the missing `outcome`.
- The errors, help, schemas and docs describe the new fields.
- Tests read the recorded new Linux, legacy macOS, legacy Linux and Windows
  replies. They include clock skew.
- `cli:test`, `cli:clippy` and `cli:fmt` pass.
- When the change exists, insert a `review-impl` leaf directly after this
  leaf.

## Notes

The cancel types belong to leaf k222, because `file cancel` is their only
reader. Leaf 63 adds the new macOS replies to these tests. Many consumers use
only the Host CLI, so its surface must stay compatible.
