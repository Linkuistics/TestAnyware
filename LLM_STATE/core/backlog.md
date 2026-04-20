# Core Backlog

Catch-all plan for bugs and backlog items that do not belong to any
focused plan in this project. Items here are mutually unrelated — the
work phase should treat each task as an isolated slice.

Category tags are generic: `bug`, `feature`, `chore`, `docs`. Tag items
as they land.

## Tasks

### 12. QEMURunner swtpm socket path exceeds macOS sun_path limit [bug]

**Status:** done
**Dependencies:** none
**Description:** `QEMURunner.start` composes the swtpm control socket as
`<HOME>/.local/share/testanyware/clones/<id>/<id>-tpm/swtpm-sock`. For the
integration-test clones named `testanyware-test-<hex8>` under a typical
`$HOME`, the absolute path is ~108–110 bytes, which exceeds macOS's
104-byte `struct sockaddr_un.sun_path` limit. swtpm refuses to create
the socket with "Path for UnioIO socket is too long" and `QEMURunner.start`
throws `commandFailed`. Verified to reproduce against pristine main on
2026-04-19 during P5 verification.

Fix options (in order of preference):
1. Stage the swtpm + monitor sockets under `$TMPDIR` (typically
   `/var/folders/.../T/testanyware-<id>/`, which is longer than `/tmp` but
   still well under 104 chars in practice) and pass QEMU the short path;
   clone the overlay qcow2 / EFI / TPM state in the original clones dir
   as today.
2. Use `/tmp/testanyware-<id>/` unconditionally — shortest possible host
   path, simplest fix, but does not respect the per-user isolation that
   `$TMPDIR` provides.
3. Shorten the inner filename and omit the `<id>-tpm` directory layer —
   e.g. collapse to `<cloneDir>/tpm.sock`. Saves ~35 chars without
   moving the socket; may be enough on short `$HOME` paths but still
   fragile.

Option 1 gives the most headroom; option 3 is the smallest diff. Blocks
`startStopRoundTripWindows` and `qemuMonitorDiscoversAgentPort` integration
tests (270/272 pass baseline — these are the 2 failing). Surfaced
2026-04-19 during Task 3 P5 verification.
**Results:** Resolved 2026-04-20 by `fix(qemu): stage swtpm+monitor sockets under $TMPDIR (sun_path limit)`
— option 1 (TMPDIR staging) implemented, plus shared teardown helper for
start-failure parity with `stop()`, plus a CRLF parser fix surfaced once
the swtpm wall came down. Both previously-failing integration tests pass;
manual `vm-start.sh --platform windows` smoke produced a running VM with
both sockets in `${TMPDIR}/testanyware-<id>/`, a non-zero screenshot, and
clean `vm-stop` teardown (no qemu/swtpm orphans). See decisions.md for
the full rationale.

### 4. Disable SSH in macOS and Linux golden images [feature]

**Status:** not_started
**Dependencies:** none (Task 3 P5 landed 2026-04-19)
**Description:** The macOS and Linux golden images ship with SSH enabled for the
`admin` user. Consumers have started reaching into VMs via SSH instead of using
`testanyware exec`, which teaches anti-patterns: the Windows golden has no SSH at
all, so any SSH-based workflow is non-portable. Bake SSH-off into the goldens
after the agent is known-working: `systemsetup -f -setremotelogin off` for macOS,
`systemctl disable --now ssh` for Linux. Downstream cleanup:
- `testanyware vm start` (Swift): drop `--no-ssh`, the SSH-wait loop,
  and the `ssh` field from the spec file.
- `ConnectionSpec.ssh` in Swift: remove (or accept-on-decode, never-encode, if
  old spec files from running VMs need tolerance).
- README: drop SSH mentions from golden-image descriptions and the spec-file
  schema.
Requires a golden rebuild for macOS and Linux. Surfaced 2026-04-18 during the
`guivision vm` refactor brainstorm. SSH wait (~11 s of ~26 s macOS start) is the
primary motivation for doing this promptly after Task 3 P5 lands.
**Results:** _pending_

### 2. recordingTask cancel-sleep pattern is a latent SIGABRT [bug]

**Status:** not_started
**Dependencies:** none
**Description:** `recordingTask` in `TestAnywareServer` cancels a sleeping `Task` from
a different executor — the exact pattern that caused the idle-timer `swift_task_dealloc`
LIFO-violation SIGABRT fixed in Task 7. Cancelling `Task.sleep(for: Duration)` from a
different executor in optimised builds triggers the crash. The fix pattern is already
established: replace cancel-then-recreate with a monotonic epoch counter — bump it to
invalidate in-flight sleeps; each fire-and-forget task checks epoch on wake and no-ops
if stale. Apply this pattern to `recordingTask` before enabling recording under load.
Surfaced as a latent risk during the idle-timer fix investigation on 2026-04-18.
**Results:** _pending_

### 13. `input click --window` reports VNC click ~40px below target on Tahoe [bug]

**Status:** not_started
**Dependencies:** none
**Description:** The `--window` option translates window-relative coordinates to
screen-absolute using the AX-reported window origin. On Tahoe, the AX window
frame includes the window's drop-shadow inset in its origin, so all clicks land
~40px below the intended target. Workaround documented in `--window` help text
and README Gotchas: obtain screen-absolute coordinates from a full-screen
screenshot and pass them directly to `input click` without `--window`. Fix:
subtract the drop-shadow inset from the AX window origin before converting
coordinates. Surfaced 2026-04-19 during Racket sample app VM testing.
**Results:** _pending_

### 5. Consolidate ConnectionSpec.namedSpecPath with VMPaths [chore]

**Status:** not_started
**Dependencies:** none (Task 3 P5 landed 2026-04-19)
**Description:** `ConnectionSpec.namedSpecPath(for:)` and `VMPaths` both compute
named-spec paths under the XDG state directory (`$XDG_STATE_HOME/testanyware/vms/<id>/`).
The duplication was flagged during P1 Task 3 (VMSpec) and deferred. Changing one
without the other will create path drift — if XDG logic evolves in `VMPaths`,
`namedSpecPath` will silently return a different location and `--vm <id>` resolution
will break. Fix: remove `ConnectionSpec.namedSpecPath(for:)` (or make it delegate to
`VMPaths`) so there is one authoritative path computation. Audit callers of
`namedSpecPath` before deleting — the `--vm` resolution path in
`ConnectionOptions.resolve()` is the primary consumer. Best done in a focused PR
after Task 3 (vm subcommands) P5 lands, since P5 touches the spec-file surface.
**Results:** _pending_

### 10. Audit testanyware-agent entitlements for Documents folder access on Tahoe [bug]

**Status:** not_started
**Dependencies:** none
**Description:** During Note Editor VM verification on 2026-04-18, running
`testanyware exec ls /Users/admin/Documents/` on the Tahoe VM triggered a macOS TCC
dialog (Transparency, Consent, and Control) on the guest. The agent binary lacks the
Documents folder (or full-disk access) privacy entitlement or TCC grant on Tahoe.
This can block automated test scripts that inspect file-save results.

Remediation options (evaluate in order):
1. **TCC grant** — grant `testanyware-agent` Documents/full-disk access via `tccutil` or
   the Privacy & Security pane during the Tahoe golden-image build, the same way
   Accessibility is granted.
2. **Entitlement** — if ad-hoc signing allows it, add `com.apple.security.files.user-selected.read-write`
   or the Documents/Downloads entitlements to the agent binary's entitlement set and
   rebuild the golden.
3. **Alternate path** — redirect file-inspection commands to paths outside the TCC
   sandbox (e.g. `/tmp`, the agent's own working directory) when Documents inspection
   is not strictly required.

Requires a golden rebuild for Tahoe once the correct grant is identified. Surfaced
2026-04-18 during Note Editor verification.
**Results:** _pending_

### 11. Deduplicate UnifiedRole.swift across CLI and agent targets [chore]

**Status:** not_started
**Dependencies:** none
**Description:** `UnifiedRole.swift` exists in both the CLI target and the agent-side
target and must be kept byte-for-byte identical. Adding or removing a `UnifiedRole`
case requires updating both copies; `UnifiedRoleTests` checks `allCases.count` and
will fail with a stale expected value if only one copy is updated. The test acts as a
guard but the duplication remains a maintenance burden — divergence can go undetected
between case additions. Fix options:
- Move `UnifiedRole.swift` into a shared framework/module target both targets link.
- Generate the file once and copy as a pre-build step.
- At minimum, add a lint rule / CI check that asserts the two files are identical.
The shared-module option aligns best with the existing library structure but requires
a target restructure. Surfaced 2026-04-19 during P4 code review.
**Results:** _pending_

### 6. Menu bar drill-down: snapshot only surfaces top-level items [feature]

**Status:** not_started
**Dependencies:** none
**Description:** `testanyware agent snapshot --window "Menu Bar"` only returns
top-level menu bar items (Apple, app-name). Submenus are not in the accessibility
tree until the menu is open, so `press --role menu-item --label "..."` fails with
"No element found" for any submenu item. Verifying full menu structure currently
requires: (1) VNC click on the menu title to open it, (2) screenshot of the region.
A `testanyware menu-drill` command, a `snapshot --open-menu <title>` flag, or at
minimum a documented multi-step workflow would reduce friction in app-validation
sessions that need to inspect or exercise menu contents. Surfaced from Racket sample
app validation sessions on 2026-04-17.
**Results:** _pending_

### 7. VirtioFS serves stale cached files after host update [bug]

**Status:** not_started
**Dependencies:** none
**Description:** VirtioFS shared filesystem can serve cached (stale) file content
to the VM even after the host has written an updated version. Current workaround:
use base64 transfer (`testanyware scp` or inline base64) or restart the VM. A
`testanyware sync` command or explicit cache-bust mechanism (e.g. `testanyware scp
--no-virtio-cache`) would eliminate this class of silent transfer error. Affects
any workflow that edits a source file on the host and then recompiles or re-runs it
inside the VM. Surfaced on 2026-04-17 during Racket sample app validation.
**Results:** _pending_

<!--
### N. Title [category]

**Status:** not_started
**Dependencies:** none
**Description:** What and why.
**Results:** _pending_
-->
