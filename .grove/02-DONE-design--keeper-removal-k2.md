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

## Decisions (running log)

- 2026-11-21, live state. The TestAnyware app answered at build 2.9.0, so
  this session ran the live checks with the installed app and CLI. The host
  ran macOS 27.2.1 (26B131) with tart 2.42.0. The logs stayed in scratch.
- 2026-11-21, issue #12 measured. `task input:live` passed on 2.9.0, and its
  guest stayed. With that guest's keeper killed, the control script sent a
  scroll in one write and closed with a reset. `row-20` moved 0 of 50 points
  in 3 runs of 3. The ordinary close before and after moved it 50 points
  each time. So the fault of issue #12 is still present on the latest host.
- 2026-11-21, issue #12 decided. `confirm_input_read` stays, because its
  fault is not gone. It is also not a pure workaround. RFB has no
  acknowledgement of input, and the app resets the guest connection of every
  guest it relays, QEMU guests included.
- 2026-11-21, reset trigger measured. On 27.0.1, a reset closure crashed
  tart at the next connection. The keeper hid that trigger from the app's
  reset of each guest connection. With no holder, three closes with an
  unread frame update were each followed by a connection 5 s later. This
  held on the kept guest and on a fresh clone, and tart survived all six. The
  three control resets and two relayed `screen size` commands 5 s apart also
  left tart running. No crash report appeared. Removing the keeper therefore
  exposes no reset fault on the latest host.
- 2026-11-21, other workarounds. The citation sweep found four sites for a
  macOS 27 VNC fault. The keeper and the app's VNC probe gate go. The banner
  probe keeps its full read before the close, because that read is the
  probe, and only its comment changes. `confirm_input_read` stays.
- 2026-11-21, host requirement. The README "Requirements" table, the quick
  start "Prerequisites" and the LLM instructions state it. The glossary
  gains the term "Host requirement". The troubleshooting entry for the idle
  fault names the symptom on an older host and the remedy, an update. The
  manual `nc` holder goes. No run-time check is added, because the owner put
  the requirement in the docs.
- 2026-11-21, records. ADR-0018 is reworked in place. Its file name loses
  the keeper slug, and the number stays, as this repository's ADRs keep
  their numbers. ADR-0004 drops its keeper sentences. The glossary entry for
  the VNC keeper says that TestAnyware has none, and why.
- 2026-11-21, live seam form. `task vm:vnc-keeper-live` becomes
  `task vm:vnc-idle-live`, because the root brief requires the keeper gone
  from the tasks. Its macOS check starts with two `screen size` commands 5 s
  apart, which tests the reset trigger through the app at no real cost.
- 2026-11-21, consumers. A sweep of `~/Development` for `_vnc-keeper`, the
  keeper fields, the keeper log and `not_probed` found no project that reads
  them. Koine's ADR `settings-harness-takes-no-keepalive-captures` already
  says that its harness does not rely on the keeper. The other hits are
  archived snapshots and fixtures in other repositories.
- 2026-11-21, review. This session cut no `review-design` leaf. The design
  removes code on the basis of measurements, and the live seams of the
  implementation leaves test the same doubts again. No doubt remains that
  needs an independent read.
- 2026-11-21, leaves. `keeper-removal-code-k5` removes the keeper and the
  probe gate from the code, the tests, the scripts, the Taskfile and the
  specs. `host-requirement-docs-k6` states the requirement and removes the
  keeper from the other docs. Both sit ahead of the release leaf, as flat
  siblings, because they share the root's context.
