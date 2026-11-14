# isolated-helper-capture-k51

## Goal

Move the live controller's blocking helper creation into a private, session-owning child so a delayed helper launch cannot consume the controller's fixed inspection/reap allowance.

## Context

First independently useful slice of worker-scheduler-k48. Preserve bounded-helper-capture-k47 and reconciliation k50 as the worker-local helper engine, including their exact ownership and capture controls. This slice deliberately uses one worker per query; it does not satisfy the persistent worker/shared scheduler contract. complete-worker-scheduler-k52 retains every remaining ancestor/spec obligation.

## Done when

- The live controller uses the isolated path, with bounded nonblocking parent reads, capped result frames, exact-child nonblocking worker reap and explicit residual obligations on incomplete creation or unreaped ownership.
- Before the worker fork enforce native/Python single-thread and default SIGCHLD admission; block signals around fork, restore the parent's mask, reset child handlers/mask/wakeup descriptor and inherited descriptors, and exit child paths via os._exit.
- Real delayed helper creation returns before the original fixed deadline and leaves no worker; healthy capture and inherited handler/mask/wakeup/evidence descriptor controls pass. Existing local helper tests remain labeled direct-engine controls, not proof of the worker protocol.
- Freeze the added sources, caps, controls and evidence; extend runner/type subjects and report limitations without claiming the complete worker/scheduler or guest acceptance.

## Notes

- Decision: a per-query worker is a temporary useful behavior boundary, not the required per-run worker. No group signal is allowed before the child announces its own session; worker reap closes all signal authority. Interrupted creation remains unknown even if the worker was reaped.
- Initial RED: two real-process tests fail with ModuleNotFoundError: No module named 'worker_capture'; original JUnit retained in the k51 evidence directory.
- Delivered: controller imports the isolated capture path and freezes worker_capture.py; manifest includes the worker and its tests, and self_check.py's existing Python-source discovery freezes both without a runner change. Parent channel limits are 4096 control bytes, 1 MiB frame, 256 KiB decoded output and <=64 KiB reads per turn; shutdown reserve stays inside the original deadline. Child reset lives in inspection.reset_after_fork. Direct helper capture retains the k47/k50 engine.
- Verification: k51-check-01 retains 137 passing tests plus strict-type failure; k51-check-02 passes all checkers and 141 tests. The real-close/injected-EIO regression fails in k51-close-red-01 before the guarded independent channel-close fix. k51-final-01 passes lint/format/types and 142 tests with matching frozen before/after subjects. Earlier targeted REDs lack a full runner hash envelope; initial Darwin pipe2 failure has no separate artifact. Preserve those limits rather than claiming complete historical evidence.
- Independent audit: k51-audit-01 stops before process checks/cleanup on an old receipt schema mismatch. k51-audit-02 freezes that failed audit too, validates historical schemas, final frozen subjects/artifacts/JUnit and real worker receipts; live/mixed/invalid/missing controls challenge its absence predicate. It finds all 1256 retained exact PIDs absent and accounts for 285 argv/JUnit/session-scoped fixture directories, retaining marker/preflight bytes and cleanup hashes. This later observation does not repair unknown creation in original samples. The close-error RED lost its receipt, so its worker PID was unavailable. No unrecorded escaped-child claim.
- Completion boundary: k52 and every ancestor stay open. The per-query worker, synchronous raw worker fork, unavailable helper-publication identities on interruption, direct-engine adapters and incomplete protocol controls are explicit limits. Full every-helper reset, strict protocol, sticky persistent-worker failure and shared scheduler remain k52, which must still schedule whole-runtime bounded-inspection-k30 review before k21. Ten-clone acceptance remains 0/10 unavailable.
- Review provenance: GROVE_PROVIDER_EVIDENCE is absent from this session's own environment; no Creator line can be recorded. Follow Grove's owner-declaration remedy if caller review policy requires it. All raw oversized retained evidence files are explicitly tracked with jj file track --include-ignored; no evidence or runtime cap was raised.
