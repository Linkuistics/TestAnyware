# app-all-guest-traffic-k46

**Integrates:** app-all-guest-traffic-k43

<!-- tc: off because the review session wrote this charter, and an integration does not rewrite its own charter -->

## Goal

Triage the findings of `app-all-guest-traffic-k43` against the design produced by `app-all-guest-traffic-k42`. Apply the valid design corrections before `app-all-guest-traffic-increments-k45` cuts the implementation leaves. Record a reason for each finding that is rejected or accepted as a trade-off.

## Context

- Read the review's own committed findings by its stable handle `app-all-guest-traffic-k43`. Its evidence is anchored to producer commit `880100aa018707d9a96a217ba2a133b54966b9fb`; task-file paths can move, so resolve handles rather than assuming the old positions.
- The design is `docs/specs/testanyware-app.md` and its cited ADR set. The producer's decision log is `app-all-guest-traffic-k42`.
- The root brief states the current requirements. `non-macos-runtime-check-k40` records the owner's later instructions and the failure that prompted the design.
- The planning charter is `app-all-guest-traffic-increments-k45`. The release node is `app-release-k21`; `release-without-windows-host-k44` already owns the distribution narrowing and its related document corrections.

## Done when

- Every finding in `app-all-guest-traffic-k43` has a reasoned disposition based on the requirements and evidence, rather than being treated as an instruction to implement automatically.
- Valid design corrections are applied consistently to the spec, the affected ADRs and the planning charter, with any necessary glossary or brief correction.
- A question that needs the owner's decision is framed with the specific trade-off, a recommendation and its evidence. Substantial new design work is externalised ahead of planning under Grove's procedure.
- The resulting design and planning charter form a coherent handoff. This integration changes no production or test code; implementation and its verification belong to the later leaves.

## Notes

The review is inspection-only and ran no test, build, lint or format command. Its graph index could not start, so its source claims use direct reads and disclose that limit. Triage the review's findings from its commit; this leaf deliberately does not restate them as its own charter.

<!-- tc: on -->

## Decisions (running log)

- **F1 is a real issue, and the fix bounds the Linux HUT to tart.** `wants_tart` (`cli-rs/crates/testanyware-vm/src/lifecycle.rs`) gives a same-named qcow2 priority over the tart golden, and `CONTEXT.md` documents Linux-on-qcow2 as a supported path. With a QEMU HUT, the gateway and the peer address of the planned forward are wrong. `vm list --json` reports `backend` for each running clone, so the harness can read it through the Host CLI. The fix: after the start, the harness fails unless the HUT's backend is `tart`, and it names the cause. It does not change the backend selection of `vm start`.
- **F4 is a real issue, and the fix keeps the boundary that the OCR band exercised.** The release ships the `ocr_analyzer` module, and the Linux formula installs a pinned EasyOCR into a venv (`scripts/templates/testanyware.rb.tmpl`). The OCR band installs EasyOCR and its native wheels in the HUT. On a desktop golden, a wheel that needs a desktop library would pass unseen. The fix extends the planned library check to the native libraries of the OCR venv, after provisioning. A library that resolves outside the venv must be a part of the C or C++ runtime. The alternative, to accept the narrower proof, would need the owner's acceptance for no saving that matters.
- **F5 is a real issue, and the fix is one sentence in ADR-0005.** ADR-0006 makes `screen record` the second long-lived RFB consumer, and `CONTEXT.md` agrees. ADR-0005 still says that the viewer is the only one. The edit keeps the thread-ownership decision of ADR-0005.
- **F2 and F3 go to the owner.** Each touches a rule that the owner set. F2: the later placement rule against the explicit endpoints that `plan-k1` kept. F3: an exception to "Only the user who runs the app can reach it". A record says that the open risk "Guest endpoints on every interface" was part of the spec that the owner agreed on 2026-10-20 (commit `fc60711`). Today's harness forwards bind `0.0.0.0`, which is wider than the loopback forward of the design.
- **The owner's answer to F2: explicit endpoints are a compatibility exception.** I asked: on 2026-10-20 the owner kept `--agent`, `--vnc` and `--connect`, and on 2026-10-21 the owner said that only the app knows where a VM runs. The owner chose "Endpoints as exception". The app owns placement for each request by VM id, and TestAnyware's own tests and scripts name VMs only by id. With an explicit endpoint, the user names the address, and the app only makes the connection. The flags keep their meaning. The spec no longer claims that the app limits every request to the ports of its VMs.
- **The owner's answer to F3: a bounded exception for the Windows harness.** The owner chose "Accept bounded exception". During a manual run of the Windows host harness, another local user can connect to the forward on loopback. Through it, that user reaches the macOS golden's agent and VNC by the app. The exception covers only that: manual runs, loopback, one run, and the golden's two endpoints. No release runs that harness. The rejected option was a check that the HUT's QEMU process owns the peer socket, through `/usr/sbin/lsof`. After F1, a Linux harness run never uses a QEMU HUT, so the exception does not reach the release harness.
- **The F2 answer changes one note of the planning charter.** Increment 2 had the two live-check scripts call `agent health --agent localhost:<port>`. Under the owner's answer, TestAnyware's own scripts name a VM only by its id. So the charter now has them call `agent health --vm <id>`. The app then dials the endpoint that the VM's spec records, which is the endpoint that the start reported.
- **No new review chain.** Each fix keeps the design and adds no new design, so this session cuts no producer review chain. ADR-0005 got a conversion to controlled English first, as its own commit.
- **Seen, and left alone.** ADR-0009 still describes four distribution triples, Windows included. `release-without-windows-host-k44` owns that fix. The module docs of both harnesses still describe SSH and a stock image. The increments of `app-all-guest-traffic-increments-k45` fix them.
