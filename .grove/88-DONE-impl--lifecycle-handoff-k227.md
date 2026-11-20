# lifecycle-handoff-k227

## Goal

Ravel-Strategies has a durable handoff for the released lifecycle contracts.
A resource audit shows that the grove left no owned resource behind.

## Context

- Root brief: section "Done when".
- The trial evidence of leaf k225 and the release of leaf k226.
- Spec: sections "Out of scope" and "Bounds".

## Done when

- Write a handoff document in `docs/`. It states:
  - the release version,
  - the outcome kinds, the JSON fields and the bounds,
  - the explicit cancellation and CLI interruption contracts,
  - the escaped-descendant limit,
  - the exclusions of the spec,
  - the evidence index.
- Map each item of the root "Done when" to its evidence.
- Audit the resources. No owned VM, clone, slot, export, temporary file,
  process or workspace remains.
- Record that each retained golden is unchanged.
- Comment on issue #14 with a summary and a link to the handoff.

## Notes

Close issue #14 only on the human's confirmation.

## Decisions (running log)

- 2026-11-20: the audit before disposal is
  `experiments/exec-reliability/evidence/lifecycle-handoff-k227/audit-before.txt`.
  No clone runs, `vm slots` shows no holder and no waiter, and the TestAnyware
  clone directory is empty.
- 2026-11-20: `diagnostic:acceptance-images` hashed six Tart images, and the
  compare with the k224 record `images-final.json` passed for each one. The
  four retained images and the two k224 goldens kept their digests and were
  not booted. A scratch copy with one changed disk digest failed the same
  compare (exit 201), so the instrument can fail.
- 2026-11-20: the two Windows goldens are QEMU images with no k224 record.
  Their files last changed on 2026-10-18, before this grove started. That is
  a modification-time check, not a digest check.
- 2026-11-20: the owned resources are the k224 goldens, the k224 export
  archives, the jj workspace `k224-build`, five orphan test processes of
  leaves k146 and k188, the scratch directories of earlier sessions of this
  workspace, SwiftPM lock files in `$TMPDIR`, and VM logs named after leaf
  keys. The root brief tells the grove to dispose of temporary baselines and
  archives after it keeps the evidence. The k224 goldens hold the trial
  agent, not the release agent, so a consumer gains nothing from them.
- 2026-11-20: the five orphan processes passed an identity check by PID,
  start time, command and a single-member group. `xctest` 70224 ended on
  SIGTERM. The four `pytest` processes ignored SIGTERM for 11 s and ended on
  SIGKILL. The record is `process-disposal.txt`.
- 2026-11-20: `vm delete` removed both k224 goldens after a dry run. The two
  archives matched their k224 byte counts, and the session removed them and
  the directory `TestAnyware.k224-exports`.
- 2026-11-20: `jj workspace forget k224-build` removed the jj and git
  registrations. The session then removed the 12 GB directory.
- 2026-11-20: the session removed 91 scratch directories of earlier sessions
  (about 17 GB) and 33 empty SwiftPM lock files. It kept its own scratch
  directory.
- 2026-11-20: 27 VM names in the app's log directory appear in this
  repository's records, and the session removed their 54 log files. 24
  generic names appear in no record, so they stay as unclassified residuals.
- 2026-11-20: the three failed-run directories of k226 lived only in the
  ignored `target/candidates/`. The session copied them, about 150 KB, to
  `experiments/exec-reliability/evidence/lifecycle-release-k226/`, and the
  digests match.
- 2026-11-20: the audit after disposal is `audit-after.txt`. The ignored
  candidate directories (688 MB) stay with this workspace until the finish.
  The Local Network entry of the private app stays, because macOS 27.0.1
  cannot remove an entry.
- 2026-11-20: the handoff is `docs/experiments/exec-reliability-handoff.md`,
  beside the reports that it indexes. It cites commits, reports, specs and
  ADRs, not `.grove/` paths, because the finish removes `.grove/`. The spec
  section "Release" links it in a new subsection "Handoff".
- 2026-11-20: k238 changed the open flags of the macOS download after k221
  measured its stress reads. The handoff names the k221 agent for that result
  and adds the Linux FIFO limit of k238.
- 2026-11-20: no product code changed after `v2.9.0`. The commits after the
  tag hold only the k226 log, the spec section "Release", this handoff and
  its evidence. So no release is missing.
- 2026-11-20: the link of the issue comment works only after a push of
  `main`. The push and the comment are public, and no instruction approves
  the push, so the session asks the human before either.
- 2026-11-20: **the human approved** the fast-forward of `origin/main` from
  `55af0aa8` by the k226 log commit and this commit. The human also approved
  the issue comment. The comment links the handoff on `main`.
- 2026-11-20: **the human chose to close issue #14** after the comment, on
  three conditions: the release exists, the local Homebrew install is
  updated, and `main` is pushed. `gh` shows release `v2.9.0` published with
  4 assets. Homebrew has the formula and the cask at 2.9.0, and `doctor`
  shows the Host CLI and the running app at build 2.9.0. Its one failure is
  the login item, as for releases 2.7.0 and 2.8.0. After this commit, the
  session pushes `main`, posts the comment and closes the issue.
- 2026-11-20: no review leaf follows this leaf. It changed no code, and each
  claim of the handoff cites a record that an earlier review or integration
  checked. No framework API of a changing version entered this leaf.
