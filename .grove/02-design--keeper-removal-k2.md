# keeper-removal-k2

## Goal

Design the removal of the VNC keeper, and of each other workaround for a
macOS 27 VNC fault that the latest host no longer has. Decide where the docs
state the host requirement, and what they say. Deliver the reworked decision
records, then insert the implementation leaves ahead of the release leaf.

## Context

- The settled decisions are in the running log of `plan-k1`, from
  "deliverable revised" onwards. The reproduction and attribution results are
  in its notes.
- The keeper spans the VM lifecycle, the meta, the start records and the
  legacy stop path. It also reaches app adoption, the app status protocol and
  the hidden `_vnc-keeper` command. `task vm:vnc-keeper-live` tests it.
- Issue #12's fix in the CLI's VNC client avoids a second fault, where a
  reset drops unread input. Its research names the control script and the
  live task.
- The user guide documents a manual `nc` holder for the idle fault.

## Done when

- The design states how each keeper site goes. It keeps nothing working for
  state that 2.9.0 created.
- A run of `scripts/tart-vnc-input-control.py` on the latest host decides
  whether the fault of issue #12 is gone. The design then keeps or removes
  that workaround, and it gives the reason.
- The design names where the docs state the host requirement, and its
  wording.
- ADR-0018 and the glossary entry for the VNC keeper describe the new current
  state, as `ADR-FORMAT.md` and `CONTEXT-FORMAT.md` direct.
- Implementation leaves exist ahead of the release leaf. Each one names the
  seams from the root brief that it must pass.

## Notes

- The idle reproduction stays unchanged. It guards against a later macOS
  release that reintroduces the fault.
- The research document of the idle fault still lists a fixing point release
  as unknown. Record the macOS 27.2.1 results from `plan-k1` there.
- A live run needs a VM. A run through `vm start` also needs the TestAnyware
  app, which did not run on 2026-11-21 after the macOS upgrade.
