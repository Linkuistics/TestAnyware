# host-cli-control-k240

**Integrates:** host-cli-control-k239

## Goal

Triage the committed findings of host-cli-control-k239 against the current
Host CLI implementation and its agreed lifecycle and evidence-transfer
contract. Apply the findings that meet the acceptance and material-risk
threshold before acceptance-plan-k223 consumes this implementation.

## Context

- Review: host-cli-control-k239. Read its findings from that review's commit,
  not from a restatement in this task.
- Producer: host-cli-control-k222, commit
  `282ee507f4036ae74fc41ab959cf9b66f86fd91d`.
- Contract: `docs/specs/guest-command-lifecycle.md`, ADR-0025, ADR-0026,
  and the producer's Done when and notes.
- The review records its direct-source scope and unavailable graph checks.
  Recheck each finding against source and the actual supported route.
- Reuse `Taskfile.yml` and the existing client/CLI/app test seams. Required
  verification remains cli:test, app:routing-test, cli:clippy and cli:fmt,
  plus the concrete affected controls. Invoke project tasks through mise.

## Done when

- Every review finding has a source-verified disposition under the
  integration family's acceptance and material-risk threshold.
- Valid actionable findings are repaired and verified at their responsible
  boundary, with controls that detect the stated wrong outcomes.
- Accepted trade-offs and rejected findings name their evidence and the
  agreed contract. Optional polish does not widen this increment.
- Existing text output, JSON fields, exit codes, routing, cancellation and
  download contracts remain accounted for. Record actual verification
  results and preserve adverse evidence.
- acceptance-plan-k223 has a concrete reviewed implementation handoff.
  The separate frozen acceptance trial remains required.

## Notes

This is a flat continuation under the root because it shares the existing
Host CLI and acceptance contract. It precedes acceptance-plan-k223 so that
intervening work cannot silently move the review's source coordinates.
Do not patch the global CLI or app, replace a golden, or operate on unrelated
guests. Use private builds and explicitly owned test resources only.

## Decisions (running log)

- Triage reads the five findings from review commit `208372790b` and checks
  each against the source at `282ee507`. The graph server failed to connect
  in this session too, so direct source reads and local experiments are the
  evidence.
- R1 is a real defect. Tokio 1.52.1 keeps its signal-hook-registry handler
  after a `Signal` drops, and nothing restores the default action after the
  request. The red control reproduces the defect: SIGINT and SIGTERM during
  output to a full pipe leave the Host CLI alive past 10 s.
- R2 is a real defect. `std::fs::File::create` opens an existing FIFO on the
  task that must see the signal. The red control reproduces it: the Host CLI
  never reaches the next temp name.
- R3 is a real defect. No code compares a declared `Content-Length` with
  the decoded count when hyper decodes chunks. The red control gives exit 0
  for a chunked body of 6 bytes and of 4 bytes under `content-length: 5`.
- R4 is a real trade-off, accepted visibly. On Darwin, a Unix stream socket
  gives its peer a plain end of stream after `SO_LINGER` 0, after unread
  data and after both. So the app cannot report a guest reset out of band.
  An in-band error needs a change of the ADR-0004 stream protocol, which
  issue #8 owns. Darwin TCP delivers every byte that reached the host before
  it reports `ECONNRESET`, so the relay forwards surplus bytes first, and
  the framing check rejects them. Only bytes that never left the guest stay
  hidden. The declared body then arrived whole and exact.
- R5 is a real gap in the controls. Each old-path signal case becomes one
  test for each signal, and the runner lists all four cases.
- R1 repair: when the request ends, the Host CLI restores the default
  action of each signal that it listened for. A second registry action
  stores each caught signal in an atomic, and a signal caught before the
  restore is raised again without a cancellation. A handler that runs on
  another thread during the restore, before its store, is missed. That
  window lasts a few instructions.
- R2 repair: the temp file opens with `create_new`. An existing entry fails
  with `EEXIST` and is never opened, and the next name is tried, up to 16
  names. The Host CLI never removes an entry that it did not create.
- R3 repair: the client compares the decoded count with the declared
  `Content-Length`. More bytes give `DOWNLOAD_FRAMING_INVALID`, and fewer
  give `CONNECTION_DROPPED`. An unreadable value, or two different values,
  give `DOWNLOAD_FRAMING_INVALID`. A chunked body without a declared length
  still succeeds. The error tables and the download help name the new
  contradictory-length cause.
- Red runs on the unrepaired code: both R1 controls ended at the 10 s
  limit. Both R2 controls never reached the second temp name. Both R3
  conflicting replies exited 0. Each new control passes after its repair.
- Final verification ran on one frozen state, with subject digest
  `fdd8291e5c6f8fde` before and after. `cli:clippy`, `cli:test`,
  `app:routing-test` and `cli:control-old-path` exited 0. Clippy reports
  only the existing warnings and none in a changed file.
- The old-path run now fails each of the four signal cases on the binary of
  `251bf072e3` with its known defect, including both SIGTERM cases. The
  first run of this session gave the same five results. The old binary hash
  differs between the two runs because each build uses a new temporary path.
- `cli:fmt` reports drift only in lines that this leaf does not change:
  `main.rs` from line 1384 and `tests/common/mod.rs`.
- One earlier `cli:test` run was stopped and discarded, because source
  changed under it.
- No provider evidence is set and this is not a legacy launch, so no
  Creator line is written. No review names k240.
