# plan-k1

## Goal

Establish, in the owner's words, what the `gh-issue-13` grove delivers for
[issue #13](https://github.com/Linkuistics/TestAnyware/issues/13). Record the
settled decisions, then fill the root brief and cut the next leaves.

## Context

- Issue #13 quotes Koine's reviewed consumer contract,
  `docs/specs/settings-capture-resource-contract.md` at Koine revision
  `e1900d49668c2ec13244c8fb189d0d45f167eab7` (SHA-256
  `cf26eab09b8abab7bdc9dc4dae5ae8f95573316cfe8fa61654245a8b4868aa4a`).
- ADR-0018 (`docs/adr/0018-tart-guests-get-a-vnc-keeper.md`) describes the VNC
  keeper and the test for its removal.
- Koine's live grove is in `../Koine.gh-issue-14`, beside this working tree.
  The Koine node that needs this issue is `settings-keepalive-owner-k96`, and
  its next leaf is `settings-keepalive-profile-reconciliation-k126`. The
  keepalive rationale is in
  `docs/verification/settings-activation/KEEPALIVE-DESIGN.md` there.

## Done when

- The owner settles the deliverable for issue #13 with the keeper finding in
  view.
- The root brief states the goal and the done criteria that follow from that
  decision.
- The next leaves exist, or this session does the work directly when the
  decision needs no tree.

## Notes

- **Keeper finding.** Koine's harness takes a screenshot about every 20
  seconds, which Koine calls a keepalive capture. Koine's legacy source
  documents "an observed VZ assertion after roughly 60 seconds without a VNC
  client". The keepalive captures are the reason for Koine's serialised command
  owner. The resource admission of that owner is the reason for issue #13.
- That crash is the macOS 27 `_VZVNCServer` fault in ADR-0018. Since v2.2.0
  (2026-10-19), each tart guest that `vm start` starts gets a VNC keeper,
  through the TestAnyware app too. Its live test kept a macOS clone alive for 5
  minutes with no other client. Nothing restarts a dead keeper, and nobody
  measured a hold longer than 300 seconds.
- Koine's design rejected a persistent VNC connection, because "Neither is a
  passive liveness interface". The keeper is such an interface. Koine's
  keepalive design and its resource contract never mention it, although
  Koine's producer basis is TestAnyware 2.7.0, which contains it. One finished
  Koine review task, `settings-nonexec-evidence-k63`, names "keeper processes"
  once. It notes only that they do not inherit Koine's writer lock. No Koine
  document links the keeper to liveness. Only Koine can confirm whether
  another reason for the limits remains.
- **OS upgrade.** The owner stopped the grove on 2026-11-21 to upgrade macOS.
  The owner said that the new version may fix the fault that the keeper
  prevents. ADR-0018 says TestAnyware can remove the keeper when
  `task vm:tart-vnc-idle-repro` passes without `HOLD=1` on every supported
  host. If Apple fixed the fault, Koine's keepalive captures lose their reason
  even without the keeper. After the upgrade, the host runs macOS 27.2.1
  (26B131). ADR-0018 recorded the fault on 27.0.1 (26A434).
- **Reproduction result.** On 2026-11-21, on this host with tart 2.42.0,
  `task vm:tart-vnc-idle-repro` passed without `HOLD=1` at a 70-second gap
  and at a 300-second gap. Each second connection completed the handshake,
  and tart did not crash. Neither run left a clone. The crash-report folder
  held no tart report before or after the runs, so its silence only
  corroborates the result. On 27.0.1 with tart 2.40.1, every tested gap of 60
  seconds or more crashed tart.
- **Attribution result.** On 2026-11-21, the same reproduction with the tart
  2.40.1 release passed at a 70-second gap on this macOS 27.2.1 host. The
  release digest matched GitHub's published SHA-256, and its signature named
  the same Cirrus Labs team as the installed tart. The same tart release
  crashed in every tested run on 27.0.1, so macOS removed the fault.
- **Resume here.** The owner saw only a short version of the keeper finding
  before the stop. Explain the finding in full first. Then ask the deliverable
  question again. The recommendation is a short reply on #13. It directs Koine
  to the keeper, and it declines the per-allocation contract as out of
  proportion for a test driver. The alternatives are the keeper pointer alone,
  or the full design response that the decisions below assumed. Any GitHub
  comment needs the owner's approval of its exact text first.

## Decisions (running log)

- 2026-11-21, deliverable. The grove delivers the design response only: the
  full contract assessment as a TestAnyware spec, reviewed, then returned on
  issue #13. Implementation and qualification are a later scope that the owner
  agrees separately. The owner chose this option.
- 2026-11-21, platforms. The assessment covers the macOS host path through the
  TestAnyware app, with both macOS OCR engines: in-process Vision and the
  EasyOCR daemon child. Linux and Windows hosts are out of scope, and the spec
  says so. Koine's scripts run only on a macOS host. The owner chose this option.
- 2026-11-21, evidence. The design sessions read TestAnyware's code and the
  exact dependency sources that `Cargo.lock` pins. They may also run small
  in-process Rust probes, such as a counting allocator over finite fixtures, to
  confirm where a dependency allocates. They run no VM, no Vision call and no
  input action. Probe output stays in scratch, not in the repository. The owner
  chose this option over the source-only recommendation. Consequence: the spec
  describes the method and result of each probe that it relies on. It never
  presents a probe result as enforcement evidence, because the contract says a
  successful sample proves no bound.
- 2026-11-21, artifacts. The response has two durable parts. A spec in
  `docs/specs/` holds the behavioural design: the interface, the unit schema,
  the enforcement owners, the refusals, the controls and the unavailable bounds.
  A research document in `docs/research/` holds the per-site assessment: source
  paths, crate versions, features, probe methods and results, and the missing
  evidence. An ADR is written only where the three-part test passes. This
  follows the issue #14 pattern of a spec, design views and ADRs. The owner
  chose this option.
- 2026-11-21, issue state. Issue #13 stays open after the response is posted.
  The response answers the design request. Only a later implementation can
  supply the enforcement evidence that the title also requests. That later
  scope closes the issue under the owner's usual close rule. Koine's documents
  continue to cite one live thread. The owner chose this option.
- 2026-11-21, premise questioned. At the review question, the owner wrote the
  words below instead of an answer. The doubt puts the earlier choices in
  question. The next session confirms the deliverable with the owner before it
  grows the tree.

<!-- tc: off because this quotes the owner verbatim -->

> I'm not actually sure what we are doing in this issue, and what Koine wants
> and why. It seems a very intrusive request for something Koine just uses to
> run tests.

<!-- tc: on -->

- 2026-11-21, stop. The owner stopped the grove to upgrade macOS. This session
  ends with a force-stop, so the leaf stays live. Rerunning Grove resumes it.
- 2026-11-21, deliverable revised. After the full keeper finding, the owner
  agreed with the recommendation. The grove delivers a short reply on #13. The
  reply directs Koine to the VNC keeper and declines the per-allocation
  contract as out of proportion for a test driver. It invites Koine to name
  any other reason for the limits. This decision replaces the five earlier
  decisions on the deliverable, platforms, evidence, artifacts and issue
  state, because each one assumed the full design response.
- 2026-11-21, reproduction first. Before the reply, this session runs
  `task vm:tart-vnc-idle-repro` without `HOLD=1` on this macOS 27.2.1 host.
  The owner wants to see whether the new macOS fixed the fault, and whether
  the keeper is still required. The owner chose this.
- 2026-11-21, latest versions. The owner wants the result from the latest
  tart and macOS, and not from release notes. The installed tart, 2.42.0, is
  the latest release on Homebrew and on GitHub, so the runs already use it. No
  release note is evidence for the result. The owner gave this rule.
- 2026-11-21, reply posted. The owner approved the exact reply text. This
  session posted it unchanged as
  https://github.com/Linkuistics/TestAnyware/issues/13#issuecomment-7060910300.
- 2026-11-21, host requirement. The owner set a requirement: a TestAnyware
  host on macOS needs the latest release of macOS 27 and the latest release of
  tart. A guest is a different situation. Where this work belongs is still
  open. Consequence: every supported macOS host would then run the versions
  that passed the reproduction. That meets the condition in ADR-0018 for
  removing the keeper.
- 2026-11-21, attribution. The owner wants to know whether macOS or tart
  removed the fault. This session runs the reproduction with the tart 2.40.1
  release on this macOS 27.2.1 host.
- 2026-11-21, issue #2. The owner asked this session to update and close
  issue #2, because it reports this crash. The comment needs the owner's
  approval of its exact text first.
- 2026-11-21, issue #2 closed. The owner approved the exact text. This
  session posted it unchanged as
  https://github.com/Linkuistics/TestAnyware/issues/2#issuecomment-7061254055
  and closed #2 as completed.
- 2026-11-21, issue #13 closed. The owner chose to close #13. This session
  closed it as not planned, because TestAnyware declined the request.
- 2026-11-21, docs and workarounds. The host requirement goes in the docs.
  TestAnyware removes its workarounds for the fault that the latest macOS 27
  fixed, and the keeper is one of them. The owner chose this.
- 2026-11-21, goldens. The owner wants the macOS golden updated to macOS
  27.2.1. The general rule: keep each golden at the latest release of its
  major.
- 2026-11-21, grove scope. The owner confirmed that this grove takes on the
  follow-through as one workstream that ends with a release. The new leaves
  are flat siblings under the root: a design of the keeper's removal, a
  design of golden refresh, and a release. This confirmation is the explicit
  scope change that the grove's rules require.
- 2026-11-21, no backward compatibility. The owner wrote the words below. This
  session reads them as follows. The work keeps nothing working for state
  that 2.9.0 created: guests, metas, start records, the old base VM and the
  old goldens. That state is discarded and rebuilt, not migrated.

<!-- tc: off because this quotes the owner verbatim -->

> We are the only users of TestAnyware, so we don't need to worry about any
> kind of backwards-compatibility. We should burn the disk packs.

<!-- tc: on -->

- 2026-11-21, test seams. The owner agreed the seams in the root brief. For
  the keeper, they are its live task in a new form, the unchanged idle
  reproduction and the existing code suites without keeper cases. For issue
  #12, they are the input control script and the input live task. For
  goldens, they are the dry run and the live gate. For the release, they are
  the facts of the close rule. With that agreement, the owner and this
  session share one understanding, and the tree can grow.
