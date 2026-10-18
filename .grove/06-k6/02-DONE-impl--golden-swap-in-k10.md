# golden-swap-in-k10

## Goal

Put the verified new Windows golden where `vm start --platform windows` finds
it by default, with the human's approval, and run the root brief's final bar
with it: one `vm start --platform windows` on this macOS 27 host reaches a
Healthy agent, with no help over VNC, and says the TPM was attached.

## Context

- `admin-password-never-expires-k8` built the new golden under its own name
  and verified it with `--base <new>`. Its Decisions log has the name, the
  `net user admin` output, and the recipe to rerun.
- `testanyware-golden-windows-11` is the only Windows golden, and is the
  default base for `--platform windows`. A golden is three artefacts in the
  golden directory: `<name>.qcow2`, `<name>-efivars.fd` and a `<name>-tpm/`
  directory. Clones back onto the golden's qcow2 by path, so none may be
  running when it moves.
- The root brief's done line, and issue #3's grove: its
  `start-failure-qemu-k3` seam check e starts the default Windows golden, so
  it needs this swap as well as this grove's code on `main`.

## Done when

- **The human has been asked, and the answer is recorded.** Hand the question
  back in three parts, as `references/execute.md` directs: the trade-off, a
  recommendation, and the evidence.
  - The options: (a) rename the old golden's three artefacts to a backup name
    and give the new golden's artefacts the default name, so the old one is
    kept until the human says to delete it; (b) leave both and have users pass
    `--base <new>`, so the default golden stays the expired one and the root
    bar is met only with a flag; (c) delete the old golden and rebuild under
    the default name with `vm create-golden` (another run of up to 75
    minutes, nothing renamed, and nothing left to roll back to).
  - The recommendation: (a). It meets the bar with the default golden, is
    undone by renaming back, and is cheap (the old golden is a sparse 64 GB
    disk and about 2.2 TiB is free).
  - The evidence: `net user admin` on the new golden and its Healthy-agent
    start from k8, and k3's E1 to E3 on why the old golden cannot reach one.
  - If no human is reachable, stop and leave this leaf live. Do not swap.
- If the human approves a swap: no clone is running (`vm list`), the swap is
  done as approved, and the final check passes with the default golden:
  `task vm:windows-healthy-start` (the recipe k8 added) or its equivalent
  records a Healthy agent, the TPM attached, and a stopped, deleted clone.
  Record the commands and output in the Decisions log. If the human decides
  otherwise, record that and what it means for the root bar.
- While the human is there, ask the separable question k3 left them:
  whether `testanyware doctor` should report the host TPM check (the
  front-end probe and the PPI rule). k3 recommends yes: it is the same 20 ms
  call and would have pointed straight at this fault. If yes, add one `impl`
  leaf for it before retiring (`grove-llm leaf-add . doctor-reports-tpm-check
  --kind impl`), writing its body; if no, record the answer.
- The research note `docs/research/windows-tpm-on-hvf.md` gets a line for the
  final run if it keeps a table of live runs (it does: E1 to E3), so the
  record of the root bar is in one place.
- Closing the root: this is the last leaf, so retiring it leaves no live leaf
  and the driver launches `finish`. Do not tear anything down. In the final
  message, tell the human what is theirs: closing issue #6; filing the
  drafted QEMU report (k3's Notes); and that issue #3's grove can now rebase
  onto `main` once this lands.

## Notes

- **Never replace the golden without the answer above, and never delete the
  old one without a separate yes.** A rename is reversible; a delete is not.
- Guests are driven only through the In-VM agent and VNC.
- This branch's `vm start` warns and exits 0 if the agent never answers;
  the final check must read the agent itself.

## Decisions (running log)

- **Human's answers (2026-10-18).** Swap: option (c), delete the old `testanyware-golden-windows-11` and rebuild under the default name with `vm create-golden`. The option as asked named the deletion and called it irreversible. Doctor: yes, cut an `impl` leaf `doctor-reports-tpm-check`.
- **Order.** `testanyware-golden-windows-11-pwfix` (verified in k8) stays on disk as a fallback until the rebuilt default golden reaches a Healthy agent. It is not deleted without a further yes.
- **Rebuild.** Deleted `testanyware-golden-windows-11` with `vm delete` (dry-run first; no clone running), then `vm create-golden --platform windows` from a frozen copy of the debug CLI with no `--name`: exit 0 in about an hour, `SetupComplete.log` showed `admin password set to never expire (exit 0)` and the build's in-guest check printed `admin's password never expires.`
- **Final bar (2026-10-18).** `task vm:windows-healthy-start` with the default golden: `OK: testanyware-512fb51f agent healthy at localhost:57581, TPM attached`; `{"tpm":{"attached":true,"ppi_off":true}}`; `/health` `{"accessible":true,"platform":"windows"}`; `vm list` shows no running clones afterwards. No help over VNC. The root brief's bar is met.
- **Left for the human.** `testanyware-golden-windows-11-pwfix` (13 GB) is now a duplicate of the default golden. It was not deleted: that needs its own yes. Doctor leaf `doctor-reports-tpm-check-k11` cut at the human's yes.
