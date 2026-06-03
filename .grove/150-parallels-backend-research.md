# 150-parallels-backend-research

**Kind:** planning (research-led)

## Goal

Determine what it would take to add **Parallels Desktop** as a third VM
backend alongside QEMU and tart, and decide whether it belongs in this grove
or a sibling one. The deliverable is a findings write-up plus a scope/sequencing
decision (and, if adopted, the grown tree of implementation leaves and/or an
ADR for the backend-abstraction shape).

**Motivation (settled, operator 2026-06-03) — two drivers:**
1. **Existing user demand.** Potential TestAnyware users already run Parallels
   and want to drive it as their VM backend — meeting them where they are,
   rather than requiring tart/QEMU.
2. **A second macOS surface, especially for golden creation.** A non-tart macOS
   hypervisor exercises the [[Golden image]] / `vm create-golden` machinery
   (`110`) against a second implementation, proving the backend abstraction
   isn't accidentally tart-shaped. Golden creation is the highest-value proving
   ground because it is the most lifecycle-heavy surface.

This is research-led: the bulk is prior-art investigation of `prlctl`/Parallels
capabilities; it closes with a short grill on motivation + scope, not a full
design grill.

## Context

Backends today are a two-arm abstraction; map Parallels onto it:

- `VmTool` enum (`Qemu`, `Tart`) — dispatched in `cli-rs/crates/testanyware-vm/src/lifecycle.rs`
  (`VmLifecycle::start` / `::stop`, `#[cfg(target_os = "macos")]` for the tart arm).
- `tart.rs` is the closest analogue (both are macOS-host hypervisors): study
  `TartRunner` (clone → `run_detached` → `poll_vnc_url` → `poll_ip`), the
  detached-process + per-run log pattern (`detached.rs`), and the spec/meta
  sidecars (`paths.rs`, `VmMeta`). Note the just-fixed same-id log staleness
  trap ([[done/105-tart-restart-stale-vnc]]) — any new log-polling backend
  must avoid the same append-across-runs hazard.
- `qemu.rs` shows the *other* shape: VNC endpoint from a **live monitor
  socket**, not a log; clone dir removed wholesale on stop.
- Goldens: `vm create-golden` is being built for tart/QEMU (`110`); a Parallels
  backend would need its own golden create + clone story.
- Glossary terms in play: [[Golden image]], [[Embedded viewer]] / the RFB layer
  (`testanyware-rfb` — every command needs a VNC endpoint), Host CLI.

## Done when

A findings write-up answers, at minimum:

1. **Guest coverage** — given the two settled drivers (user demand + a second
   golden-creation surface), what guest OSes does Parallels realistically add or
   improve on Apple Silicon vs. tart's Virtualization.framework (e.g.
   Windows-on-ARM, Linux distros)? This sizes the payoff but is no longer the
   make-or-break — adoption + abstraction-proving already justify the look.
2. **VNC / framebuffer access** — the gating question, since the whole RFB stack
   needs a `host:port` (+ password). Does Parallels expose a VNC server
   (`prlctl set <vm> --vnc-mode`, port/password, headless)? Which **edition** is
   required (Pro/Business vs. Standard)? Is it reachable without the GUI app?
3. **Lifecycle CLI mapping** — `prlctl`/`prl_disk_tool` equivalents for
   tart's clone / run-detached / stop / delete / list / ip, and whether a
   long-lived detached process exists to track (or Parallels owns the VM
   lifetime itself, changing the pid/stop model).
4. **Headless start** + **guest IP discovery** (vs. tart's state-gated `tart ip`).
5. **Golden model** — templates / linked clones / register-unregister; how it
   maps onto `vm create-golden`.
6. **Licensing / availability constraints** worth surfacing before committing
   (paid license; edition-gated VNC; CI/local-release implications, cf.
   [[local-release-no-ci]]).
7. **Abstraction fit** — does a `parallels.rs` arm drop cleanly into the
   `VmTool` dispatch (`#[cfg(target_os = "macos")]`, like tart), or does it
   force a refactor of the lifecycle/meta seam? Flag if an ADR is warranted.

Then a **scope decision** (grill the user): in *this* grove vs. a sibling;
Tier-1/Tier-2 placement; or park as "investigated, not adopted." If adopted,
grow the tree (decompose into implementation leaves) and/or raise the ADR.

## Notes

- Research-only until the scope grill — do **not** start implementing a backend
  in this session.
- Cheap to spike if a Parallels install is available: `prlctl` against a
  throwaway VM to confirm the VNC + headless + clone story firsthand beats
  doc-reading. Reuse the [[vm-costs]] mindset (clone+start is cheap).
- Out of scope to resolve here: the in-VM agent for any guest OS (agents are a
  separate workstream per [[CONTEXT.md]]). This leaf is purely about the
  host-side VM backend.
