# admin-password-never-expires-k8

## Goal

Fix the second fault, from `windows-tpm-on-hvf-k3`'s R3. The Windows golden's
`admin` account inherits Windows' 42-day maximum password age, the answer
file's auto-logon has no logon count to bypass it, and so a golden older than
42 days stops at "Your password has expired" and never starts its logon-
triggered agent. Make `vm create-golden --platform windows` leave `admin`'s
password never expiring, then build a new golden with it and prove it.

## Context

- `windows-tpm-on-hvf-k3` F4 (the evidence: the guest clock read 141 days
  after the build, `net accounts` showed "Maximum password age 42 days",
  the answer file's `<AutoLogon>` has no `LogonCount`, and the agent is a
  logon-triggered scheduled task registered by `SetupComplete.cmd`), F6 (the
  golden has no licence clock: Windows 11 Pro with the generic KMS client key),
  and R3, which rejected repairing the golden in place, driving the password
  change over VNC at every start, pinning the guest clock, and an agent that
  needs no logon.
- `provisioner/helpers/SetupComplete.cmd` runs as SYSTEM after OOBE, before
  the first logon, so it is elevated; `admin` already exists by then. It is
  embedded into the CLI by `include_str!` and copied onto the autounattend
  media (`golden_windows.rs`), and `embedded_media_is_present` there is the
  test that guards the embedded files.
- `build_and_provision` already runs commands in the guest through the agent
  (`agent_exec`) after it comes up and prints `SetupComplete.log`;
  `desktop_setup_done` is a pure predicate over an `ExecResult` with a unit
  test, the model for the check below.
- Stage 1 (`windows-start-on-hvf-k5`) has landed: golden creation attaches
  the TPM with PPI off under HVF, so a build can now run on this Mac.

## Done when

- `SetupComplete.cmd` makes `admin`'s password never expire. k3 knows of no
  never-expires setting in the answer file's `<LocalAccount>` schema, so it is
  a command: `net accounts /maxpwage:unlimited` (machine-wide, no module
  dependency) or `Set-LocalUser -Name admin -PasswordNeverExpires $true` (one
  account). Prefer the narrower unless it fails in the guest. Avoid `wmic`
  (deprecated by Microsoft; not checked in this guest). Log the command's
  outcome to `SetupComplete.log` like its neighbours.
- A unit test pins the command in the embedded media.
- Golden creation checks the result in the guest, so a golden that would
  expire cannot be produced silently: after provisioning, run `net user admin`
  through the agent and fail the build (with the output in the error) unless
  it shows `Password expires` as `Never`. The decision of pass or fail is a
  pure predicate over the `ExecResult`, unit-tested on a passing and a failing
  `net user` output.
- A new golden is built on this Mac under a new `--name` that still contains
  `windows` (the platform is classified from the name; for example
  `testanyware-golden-windows-11-pwfix`), with `vm create-golden --platform
  windows --name <new>`. The Windows ISO and the VirtIO ISO are cached in
  `~/.local/share/testanyware/cache/`, so no `--iso` is needed.
- **The D1 bar against the new golden:** `vm start --platform windows --base
  <new>` reaches a Healthy agent with no manual logon and no help over VNC,
  and its output says the TPM was attached. The check reads the agent itself
  (this branch's start exits 0 with a warning when the agent never answers):
  add it as a script and a Taskfile recipe (see Notes).
- **The fix is proved by `net user admin`, not by the Healthy agent.** A
  golden built today would reach a Healthy agent for the next 42 days even
  without the fix, so a green start is not evidence of it. The build's own
  in-guest check, repeated by hand on a clone of the new golden (through
  `file exec`), shows `Password expires  Never`. Record the output in the
  Decisions log.
- The docs that describe the Windows golden's provisioning state it:
  `CONTEXT.md`'s *Autounattend provisioning*, and a grep of the claim (the
  provisioning flow, the golden's account, the logon-triggered agent) across
  `docs/` and the hand-synced `website/docs/` copies, for example
  `user/golden-images.md` and `components/provisioner.md`.
- `task cli:test` passes, and the new recipe's script is shellcheck- and
  `shfmt -i 2 -ci`-clean.

## Notes

- **The build is long and unattended**: up to about 75 minutes. Run it in the
  background with output to a log file, and wait on it with a monitor rather
  than polling. Nothing else may touch `testanyware-golden-windows-11` or its
  `-tpm` directory in the meantime. About 2.2 TiB is free.
- **Never modify or replace `testanyware-golden-windows-11`.** The new golden
  lives beside it under its own name; replacing it is the next leaf
  (`golden-swap-in`) and needs the human.
- **The live-check recipe.** A script under `scripts/` (house style: `set
  -euo pipefail`, `shellcheck`, `shfmt -i 2 -ci`) and a root `Taskfile.yml`
  recipe that starts a Windows clone with `--json` (optional `--base`), fails
  unless the agent endpoint is non-null *and* `GET /health` answers (the
  glossary counts both `accessible` states as healthy), fails unless the
  start's JSON says the TPM was attached, and always stops and deletes the
  clone. Use the debug CLI (`task cli:build`). Name the recipe
  `vm:windows-healthy-start` (the next leaf runs it by that name), taking an
  optional `BASE`. Do not call it `vm:qemu-start`: issue #3's Taskfile uses
  that name for a different recipe. If you add a
  `scripts:check` task, keep issue #3's name and shape (shellcheck and shfmt
  over the scripts this repo's tasks run); the later merge then needs only to
  union the file lists. A Windows start's agent wait is 10 to 14 minutes if
  the agent never answers, and about 2.5 minutes to the lock screen when it
  does.
- **Optional, if cheap: reproduce the actual failure mode.** Boot a clone of
  the new golden with the guest RTC set about 60 days ahead (a one-off
  `-rtc base=` on QEMU's argv through a PATH shim, as k3 shimmed `ppi=off`;
  the shim and its output stay out of the repo). A Healthy agent there is the
  behavioural proof that the fix works; the old golden stops at the expired-
  password prompt under the same shift.
- If the build fails, diagnose before retrying; a failure that is not a small
  fix is a reason to decompose this leaf rather than to run long.
- Guests are driven only through the In-VM agent and VNC.

## Decisions (running log)

- **Fix form.** `SetupComplete.cmd` step 9 runs `powershell -NoProfile -Command "Set-LocalUser -Name admin -PasswordNeverExpires $true"`, logged to `SetupComplete.log` with its exit code. The narrower per-account form was tried first, per the brief; `net accounts /maxpwage:unlimited` stays the fallback if the live build shows the cmdlet failing at SYSTEM/SetupComplete time.
- **In-guest check.** `admin_password_never_expires` (pure, over `ExecResult`) requires exit 0 and a line whose `Password expires` value is exactly `Never` ("Account expires  Never" does not count). It runs right after the agent comes up and `SetupComplete.log` is printed, before the 60 s settle and the reboot, so a failing golden fails in minutes rather than after the whole build. The error carries `net user admin`'s output.
- **Live check.** `scripts/windows-healthy-start.sh` + `task vm:windows-healthy-start [BASE=…]`; also `task scripts:check` (shellcheck + `shfmt -i 2 -ci -d` over the two scripts the tasks run). Failure path seen once with a nonexistent base: exit 1, no clone left behind.
- **Build.** Started with a frozen copy of the debug CLI (so a later rebuild cannot move the subject) as `vm create-golden --platform windows --name testanyware-golden-windows-11-pwfix`, agent from the brew-installed 2.1.0 bundle, log in the session scratchpad `build.log`.
- **Live build (2026-10-18).** `vm create-golden --platform windows --name testanyware-golden-windows-11-pwfix` ran about 25 minutes and exited 0. `SetupComplete.log` showed `admin password set to never expire (exit 0)`: `Set-LocalUser` works at SYSTEM/SetupComplete time, so the `net accounts` fallback was not needed. The build's own in-guest check printed `admin's password never expires.` The golden is `~/.local/share/testanyware/golden/testanyware-golden-windows-11-pwfix.*`; `testanyware-golden-windows-11` was not touched.
- **D1 bar.** `task vm:windows-healthy-start BASE=testanyware-golden-windows-11-pwfix` passed in 21 s: Healthy agent at `localhost:55280`, `{"attached":true,"ppi_off":true}`, `/health` `{"accessible":true,"platform":"windows"}`, no manual logon, clone removed.
- **The fix, proved by `net user admin` on a fresh clone** (via `file exec`), not by the Healthy agent:
  ```
  Account expires              Never
  Password last set            10/18/2026 6:12:00 AM
  Password expires             Never
  ```
  `net accounts` on the same clone still shows `Maximum password age (days): 42`, so it is the per-account `Set-LocalUser` flag that protects `admin`; the machine-wide policy is unchanged.
- **Behavioural proof (optional step, done).** A PATH shim in the session scratchpad (outside the repo) added `-rtc base=2026-12-17T17:19:36` to QEMU's argv (60 days after the build; the 42-day expiry would have fallen on 2026-11-29). The same `vm:windows-healthy-start` run was Healthy with the TPM attached. Not run: the old golden under the same shift as a control; k3 already showed it stopping at the expired-password prompt, and today (141 days after its build) it is past expiry without any shift.
- **Docs.** The never-expiring password is stated in `CONTEXT.md` (*Autounattend provisioning*), `docs/user/golden-images.md`, `README.md` and `docs/components/provisioner.md` plus the two `website/docs/` copies. The grep for the provisioning claim (auto-logon, logon task, SetupComplete, admin password) found no other statement of the golden's account state. ADR-0017 records the cause and the remedy already; no ADR change.
- **Checks.** `task cli:test` passes; `task scripts:check` clean (the three `release-*.sh` scripts were not formatted under `shfmt -i 2 -ci` before this leaf and are not in the task's file list).
