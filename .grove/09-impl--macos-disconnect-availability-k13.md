# macos-disconnect-availability-k13


## Goal
Account for macOS guest work and independent request responsiveness after
abrupt Host CLI interruption, separately from guest timeout and file consistency.


## Context
- Issue #14 failure C says both OSes left a known child alive. Failure D's
  `macos-candidate-02/after-abrupt-03-*` artifacts are later transfer evidence;
  they do not establish macOS disconnect cleanup or channel responsiveness.
- Reuse k11's controller/routes and k3/k4's safe process fixtures and reports.
- Match `linux-disconnect-availability-k6` comparison semantics and distinguish
  macOS `handleExec`/`ProcessRunner` from app/client close or half-close.
## Done when
- Freeze one unchanged macOS clone's exact issue #14 fork/wait payload with a
  20 s guest timeout, a no-interruption control and a finite concurrent-request
  control. Interrupt only the command CLI with SIGTERM at 2 s, allow <=2 s,
  then SIGKILL with <=2 s more if necessary. Preserve host signals/status and
  partial evidence while the outer controller survives.
- Immediately request known-child inspection, health and the immutable partial
  file through the public VM-id path, independently bounded at ordinary 30 s
  and transfer 150 s. Predeclare the controlled responsiveness oracle; record
  request acceptance/response and original-handler completion. A blocked channel
  is a defect even if it later responds within the outer ceiling.
- Measure child survival separately from handler/relay behavior using recorded
  identities. Failed inspection is unknown; failed downloads remain failures.
  Do not diagnose mutable-file corruption here: retain any such sample and
  hand it to `download-consistency-k7` rather than retrying it to success.
- Use deterministic HTTP/client controls and at most one instrumented macOS
  clone to distinguish continued execution, request availability, close/half-close
  propagation and deadlines. No clean timeout or Linux result substitutes for
  this inquiry. State a non-reproduction or unavailable phase explicitly.
- Deliver verified Taskfile diagnostics and
  `docs/experiments/exec-reliability-macos-disconnect-availability.md` with raw
  receipts, causal limits, design inputs and resource audit. Bound exact-child
  cleanup at 30 s and VM disposal independently at 45 s in `finally`, preserving
  uncertain identities/remedies. Reuse the common finite-run/isolation contract.
## Notes
Do not choose cancel-on-disconnect or continued execution here. Explicit guest
cancellation is a later independent design/acceptance case. File consistency,
ordinary completion and post-timeout inspection retain their own inquiries.
