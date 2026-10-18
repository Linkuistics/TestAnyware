# windows-golden-password-k6 — brief

## Goal

Stage 2 of the remedy: k3's R3. Windows golden creation makes `admin`'s
password never expire, so auto-logon, and with it the logon-triggered agent,
keeps working however old the golden is. A new golden is built and verified,
and with the human's approval it replaces `testanyware-golden-windows-11`.
That meets the root brief's bar: one `vm start --platform windows` on this
host reaches a Healthy agent, unaided.

## Done when

- `vm create-golden --platform windows` leaves `admin` with "Password expires
  Never" (checked in-guest: `net user admin`), and a unit test pins the
  command in the embedded media.
- A new golden, built under a new `--name` on this Mac with stage 1's code,
  reaches a Healthy agent from `vm start --platform windows --base <new>`
  with no manual logon, and says the TPM was attached.
- The human has been asked about replacing `testanyware-golden-windows-11`
  and the outcome is recorded. If it was replaced, the final D1 check passes
  with the default golden: one `vm start --platform windows`, a Healthy agent,
  no help over VNC.
- The docs that describe the Windows golden (`CONTEXT.md`'s *Autounattend
  provisioning*, `docs/user/golden-images.md`, `docs/components/provisioner.md`
  and their `website/docs/` copies) state the never-expiring password.
- The root `Taskfile.yml` carries the live check these leaves ran.

## Decomposition

1. `admin-password-never-expires` (impl): the one-line fix, then the live
   build and verification under a new name (up to about 75 minutes,
   unattended). It follows stage 1 because golden creation aborts at the TPM
   on this host until `windows-start-on-hvf-k5` lands.
2. `golden-swap-in` (impl): the human-gated replacement of the only Windows
   golden, then the final check with the default golden. It is its own leaf
   because it needs a decision only the human can make, and because the build
   session should end on a committed, verified new golden.

## Pointers

- Retired `windows-tpm-on-hvf-k3`: F4 and F6 (the expired password and the
  golden having no licence clock) and R3 (the remedy and what it rejected).
- Retired `plan-k1`: D1 (the bar), D4 (golden creation and the live build).
- `provisioner/helpers/SetupComplete.cmd` (runs as SYSTEM after OOBE) and
  `provisioner/helpers/autounattend.xml`: the golden's provisioning. Both are
  embedded in the CLI by `include_str!`.
- Glossary: Autounattend provisioning, Golden image, Healthy agent
  (`CONTEXT.md`).
- Seam 3, live (root brief).

## Notes

- **Never modify `testanyware-golden-windows-11`, and never replace it
  without asking.** It is the only Windows golden. The ISO and the VirtIO ISO
  are cached in `~/.local/share/testanyware/cache/`.
- Issue #3's grove (`start-failure-qemu-k3`, seam check e) starts the default
  Windows golden, so it needs the swapped-in golden as well as this grove's
  code on `main`.
- On this branch `vm start` warns and exits 0 if the agent never answers, so
  every live check must look for the Healthy agent itself.
