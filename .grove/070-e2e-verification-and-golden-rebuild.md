# 070-e2e-verification-and-golden-rebuild

**Kind:** work

## Goal

Verify the streaming contract end-to-end on each platform with a file larger
than every old cap, and handle the golden-image rebuild the hard cutover
(ADR-0001) requires.

## Context

- ADR-0001 — hard coordinated cutover; mismatched CLI/agent versions fail. New
  agents must be baked into golden images.
- Golden images: `Golden image` in CONTEXT.md; `vm create-golden` subcommand
  (per project memory, golden creation is moving into the CLI). VM clone+start
  is cheap (project memory) — exercising a real VM per platform is affordable.
- Leaves 010–060 must be done first (this leaf depends on all of them).
- `docs/reference/cli-schemas/file-upload.json` — the receipt to validate.

## Done when

- A round-trip (`file upload` then `file download`) of a file comfortably larger
  than the old caps (e.g. ≥ 50 MB, and ideally a few hundred MB to prove the
  ceiling is gone) succeeds and byte-matches on **each** platform's agent —
  macOS, Linux, Windows.
- Error cases verified: bad/unwritable path → `upload_failed`; missing file →
  `download_failed`; connection drop mid-upload leaves no truncated destination
  file (temp cleaned up).
- Agent memory stays bounded during a large transfer (spot-check, not a strict
  benchmark) — confirming no whole-file buffering regressed.
- Golden images rebuilt with the new agents (or a clear, recorded procedure to
  do so if rebuild is out-of-band).

## Notes

This is the leaf that proves the grove's thesis (memory ceiling removed), so
don't shortcut the large-file case. If golden rebuild turns out to be a heavier,
separate workstream, capture a follow-up via `grove-llm inbox-add` rather than
expanding this leaf.
