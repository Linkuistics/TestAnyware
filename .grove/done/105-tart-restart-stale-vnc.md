# 105-tart-restart-stale-vnc

**Kind:** work (bug fix — correctness regression)

## Goal

Fix `vm start` so that **restarting a tart VM under an id that was used before**
resolves the *current* run's VNC endpoint, not a stale one from a prior run.
Found during `090-viewer-live-verify`: a `vm stop` + `vm start` bounce of the
same id writes a dead `host:port`/password into the VM spec, so every VNC
consumer (viewer, `screen`, `input`) connects to a closed port.

## Context

The per-VM tart log is **append-only across runs** and `vm stop` removes the
spec/meta sidecars but **not** the log:

- `testanyware-vm::detached::spawn_detached` opens the log with
  `OpenOptions::new().create(true).append(true)` (`detached.rs:17-20`).
- `TartRunner::run_detached` points every `tart run` at the same
  `<vms_dir>/<id>.tart.log` (`tart.rs:241,248`).
- `VmLifecycle::stop` (tart arm) deletes the clone + sidecars but leaves the log
  (`lifecycle.rs:285-310`).
- `poll_vnc_url` returns the **first** `vnc://` token in the whole file
  (`tart.rs:124-138`: `text.split_whitespace().find(|t| t.starts_with("vnc://"))`).

So on a same-id restart, `poll_vnc_url` reads the *previous* run's `vnc://` line.
The new spec is written with that run's now-dead port + password.

**Live repro (2026-06-02, golden `testanyware-golden-macos-tahoe`):**

```
vm start --id viewer-verify     # log line 1: vnc://…@127.0.0.1:58372
vm stop  viewer-verify          # clone + sidecars gone; .tart.log kept
vm start --id viewer-verify     # log line 2: vnc://…@127.0.0.1:58373 (ACTUAL)
                                # but spec written with :58372 (STALE, dead)
```

`lsof` confirmed the VM listening on **58373**; the spec recorded **58372**
(first run's port + first run's password). A viewer pointed at `--vm
viewer-verify` then hit `Connection refused (os error 61)` and, after the
12-attempt give-up budget, painted the terminal "gave up" overlay.

**Root cause confirmed:** deleting the stale `.tart.log` between stop and start
made the next `vm start` write the correct port (verified twice: spec 58374 then
58375, each matching `lsof`), and the viewer reconnected and rendered live. So
the viewer's auto-reconnect is correct; the defect is purely upstream in the tart
start/stop log handling.

Not specific to the viewer — affects **any** consumer after a same-id bounce.
Normal usage with fresh auto-generated ids (`testanyware-<rand>`) is unaffected
because each id gets a fresh log.

## Done when

- A `vm stop` + `vm start` of the **same id** resolves the current run's VNC
  endpoint; the written spec's `host:port`/password match the live listener.
- A regression test guards it (the pure `poll_vnc_url` path is unit-testable —
  see `tart.rs:432-448`; assert a two-`vnc://`-line log resolves the **last**,
  or that the start path starts from a fresh log).
- Decide & document the chosen fix (ADR only if it's a real trade-off):
  - **A — truncate-on-run:** `run_detached` opens the log with `.truncate(true)`
    instead of `.append(true)` (or removes it first). Simplest; loses the prior
    run's log. Note `spawn_detached` is **shared with QEMU** (`qemu.rs:309`) —
    truncating there changes QEMU log behaviour too, so either gate the mode per
    caller or accept the shared change.
  - **B — remove-on-stop:** `VmLifecycle::stop` deletes `<id>.tart.log` alongside
    the sidecars. Keeps append semantics within a run; symmetric with sidecar
    cleanup.
  - **C — read-last-match:** `poll_vnc_url` returns the *last* `vnc://` token.
    Cheapest diff, but fragile if a run logs no `vnc://` and an older one did
    (would resolve a dead endpoint) — weaker than A/B.
  - **D — per-run log filename:** `<id>.<pid>.tart.log`. No staleness, keeps all
    logs, but orphans accumulate and `stop` must find the right one.
- **Check the QEMU path for the analogous defect:** does QEMU derive its VNC
  endpoint from the appended log the same way, or from a deterministic/assigned
  port? Fix or explicitly clear it.

## Notes

- Sequencing: **placed at `105`, ahead of `110-vm-create-golden`** (operator
  call, 2026-06-03) — golden creation is the VM-lifecycle-heavy leaf most likely
  to restart a VM under a fixed id, so clear this correctness bug first.
  `100-screen-record` (unrelated RFB work) stays the immediate frontier; this
  leaf is picked next after it. Normal fresh-id usage is unaffected, so it does
  **not** block the Tier-1 happy path either way. (Guest-internal reboots that
  keep the same `tart run` process / VNC port are *not* affected; only a real
  `vm stop`→`vm start` cycle is.)
- VM cost is just clone+start (memory [[vm-costs]]); reproduce with a throwaway
  clone. Use `tart list` state column, not `tart ip`, for running/stopped
  (memory [[tart-ip-lies]]).
- This fix would let `090`'s default-bounce path (`vm stop` then `vm start`,
  same id) work without the manual log-clear workaround used to verify the
  viewer's reconnect.
