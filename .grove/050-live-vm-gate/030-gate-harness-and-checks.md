# 030-gate-harness-and-checks

**Kind:** work

## Goal

Build the **live-VM gate** itself: an `#[ignore]`d integration test, gated on
`TESTANYWARE_LIVE_VM=1`, that clones+starts a golden VM (via the tart runner from
`010`), runs four checkable assertions against the running guest, and tears the
VM down. Skipped by default so `cargo test` stays VM-free; run on demand with
`TESTANYWARE_LIVE_VM=1 cargo test -- --ignored live_vm`.

## Depends on

- `010-tart-runner` — so `vm start` can reach a cheap kept-built golden.
- `020-rfb-encoding-override` — so the ZRLE/Tight/Raw check can force encodings.

## Context

The harness shape (decided at node bootstrap): a new integration test file
(e.g. `cli-rs/crates/testanyware-cli/tests/live-vm-gate.rs`) beside
`cli-contract.rs`, using `env!("CARGO_BIN_EXE_testanyware")` to invoke the built
binary, early-returning when `TESTANYWARE_LIVE_VM` is unset. It drives the CLI as
a subprocess (clone+start → `--vm <id>` resolution → checks → `vm stop`), so it
exercises the real command surface, not crate internals.

The four checks (this is where the leaf may decompose-by-check if too big):

1. **Input landing.** Run `input click`/`key`/`type` at known coordinates/keys,
   then `agent snapshot --json` and assert the focused element / value reflects
   the input. (`agent snapshot` shape: windows[].elements[] with role/label/id/
   value/focused — see `commands/agent.rs:174`.) On a macOS guest, mind the
   `cmd-key-tahoe` mapping (Command=`XK_Alt_L`, Option=`XK_Meta_L`).
2. **`agent show-menu`.** Already implemented as `open_menu_path` over RFB click
   + agent snapshot (`commands/agent.rs:101`). Drive `agent show-menu --menu
   <path>` and assert the final snapshot shows the menu open (expected menu items
   present).
3. **ZRLE + Tight capture correctness.** Using the `020` override, capture the
   same static screen three times — forced ZRLE, forced Tight, forced Raw — and
   assert the decoded pixels match (Raw is ground truth). Proves the live
   decoders agree with the synthetic-fixture decoders.
4. **Live Vision OCR.** With a guest showing known text, `screen find-text
   <query> --json` on this macOS host uses the in-process Vision engine
   (`engine.rs:79`); assert `engine == "vision"` and the query text is found with
   a plausible bounding box. **This closes the live Vision-OCR check deferred by
   `040-macos-vision-ocr` (ADR-0002/0003).** Optionally also assert the
   `TESTANYWARE_OCR_FALLBACK=1` daemon path on the same frame for parity.

## Done when

- The gate exists, is `#[ignore]`d + env-gated, and runs all four assertions
  against a freshly-cloned golden on an arm64 Mac, tearing the VM down after.
- A short doc (in the test header and/or `docs/`) states how to invoke it and
  which golden/platform it needs.
- `cargo test` (no `--ignored`) does not start a VM; the contract suite stays
  green.
- The deferred live Vision-OCR check is recorded closed (here and, on node
  retirement, promoted to the ADR-0002 consequences or a note).

## Notes

Known guest content (the text to OCR, the menu path, the input target) must be
deterministic in the golden — keep it to what the minimal golden already shows
(`minimal-images` memory: don't add test-specific tooling to the image). If a
fixed-content scratch app is needed, prefer something already present in the
golden over modifying the image.

