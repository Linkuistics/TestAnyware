# diagnostic-routes-k19

## Goal

Establish the baseline public guest routes or a concrete unavailable
prerequisite before the causal leaves start, without moving setup into their
first diagnostic sample.

## Context

Read F2 and F8 and the golden-name point in F9 of review
`diagnostic-foundation-k15` in commit `4bb47c05`, integration
`diagnostic-foundation-k16`, and the current diagnostic-foundation report.
Complete the reviewed diagnostic-evidence and disposal successors first.

The installed 2.7.0 start help contradicts itself about macOS support. Checkout
source does not prove that installed binary's route. Neither installed macOS
nor Linux has been probed; absence of a probe is not a missing prerequisite.

## Done when

- Freeze finite probe plans, boot/slot/export/identity/health bounds and inputs
  before any live run. Use at most one unchanged owned clone per affected OS;
  failed samples remain failed and are not replaced. No causal workload here.
- Resolve installed start support with public evidence, then minimally verify
  answering host/app/agent identity and health per OS or name the actual missing
  prerequisite. Preserve installed app and retained goldens. No private
  connection files, direct guest routes or app quit without owner authority.
- Supply and hash a dynamic inventory checker that verifies exact identity
  absence in both public VM and hypervisor inventories, rejecting malformed
  responses and inspection errors. Demonstrate fake-peer failure controls.
- Demonstrate already-absent disposal and failed acquisition: recognize only a
  proved VM_NOT_FOUND result plus independent absence; do not accept arbitrary
  exit 3 or conceal cleanup failure. Supply an actionable remedy.
- If retaining golden creation recipes as the documented installation path,
  add and test refusal of existing names before any create operation, using
  fixture inventories, including malformed/inaccessible inventory. The current
  Taskfile NAME-nonempty check does not protect a retained golden.
- Report each raw result, unavailable route, residual resource and concrete
  remedy under a new evidence condition. No acceptance or production repair
  claim. Insert material implementation review ahead of the causal consumer.

## Notes

Use the root brief's common investigation contract. Do not invoke
`agent:macos:test`. A development-app handover still needs the owner's separate
interval and Local Network grant; this leaf cannot infer that authorization.

## Decisions (running log)

- **Installed version moved.** The installed Host CLI is now 2.8.0
  (`/opt/homebrew/Cellar/testanyware/2.8.0_1`), not the 2.7.0 that the
  preflight recorded. The probe records the 2.8.0 identity and reads its help
  again. No conclusion about 2.7.0 carries over.
- **Installed macOS start support, public evidence.** `vm start --help` still
  says that `--platform` takes "linux or windows". The same help and
  `llm-instructions` give macOS examples, and `llm-instructions` says that the
  app starts every guest on a macOS host. The probe resolves the contradiction
  with the public non-mutating `vm start --platform macos --dry-run --json`, and
  then with the one owned macOS clone. Source of tag v2.8.0 agrees:
  `cli-rs/crates/testanyware-cli/src/resolve.rs:377-386` selects the default
  app socket on a macOS host, and `commands/vm.rs:96-123` sends the start to
  the app.
- **Boot bound 420 s per tart start.** v2.8.0
  `cli-rs/crates/testanyware-vm/src/lifecycle.rs:1076-1080` polls VNC 60 x 1 s,
  the guest IP 30 x 2 s and the agent 60 x 2 s. Each agent request has a 2 s
  timeout (`health.rs:235`). The worst stage sum is 360 s. The bound adds 60 s for the
  clone, the spawn and the app queue hand-off. It is frozen before any run and
  is not raised after a run.
- **Slot bound.** The probe never passes `--wait`. A full macOS slot table ends
  the start with `VM_LIMIT_REACHED` (exit 6), and the probe reports it as the
  missing prerequisite. It does not retry.
- **Identity and health bounds.** `agent health --vm <id> --json` has 30 s.
  The agent identity probe is one `file exec --timeout 2` that hashes the
  installed agent. Its controller budget is 2 + 20 s (raw host exec rule).
- **One checker, two uses.** `scripts/vm_inventory.py` decides exact-name
  absence from both `vm list --json --all` and `tart list --format json`. The
  disposal oracle and the golden-name guard both call it. A malformed listing,
  a nonzero exit, a timeout or an unreadable executable is an inspection
  failure, never absence.
- **Disposal wrapper.** `vm_inventory.py dispose` accepts only exit 0 with a
  success envelope, or exit 3 with an error envelope whose `code` is
  `VM_NOT_FOUND`. Then it requires independent absence. Any other exit 3,
  for example `GOLDEN_NOT_FOUND`, fails. Its stop budget is 30 s and each
  listing has 5 s. The controller call that runs it has 41 s. The
  controller adds TERM 2 s and KILL 2 s to reach the 45 s ceiling. (This
  supersedes the earlier figures of 6 s and 45 s, which the controller's
  ceiling check refuses.)
- **Golden guard.** The golden recipes stay the documented provisioning path.
  Both `vm:create-golden-*` recipes now refuse a name that either inventory
  lists. They also refuse when an inventory cannot be read.
- **QEMU scope.** Both affected OSes start on tart. The QEMU hypervisor has no
  public inventory, so a QEMU identity is out of scope for the checker.
- **Frozen probe plan.** `experiments/exec-reliability/routes_k19.py` writes
  every plan, bound and subject hash to `OUT/inventory.json` before its first
  live call. Each OS runs five plans once, in this order: `host` (dry run,
  `doctor`, app signature, absence of the missing golden name),
  `never-created`, `failed-acquisition`, `route` and `after-route`. Only
  `route` creates a clone, so each OS has at most one owned clone. `route`
  runs only when the dry run exited 0. The evidence keeps a failed plan, and
  the probe does not repeat it. The OS order is Linux first, then macOS, because the installed
  help claims Linux support and contradicts itself only about macOS.
- **Golden preservation evidence.** Each live directory holds `tart list
  --format json` and `vm list --json --all` before and after the probe. It
  also holds the pid of the installed app before and after. A retained golden must
  appear in both listings with the same size.
- **Self-check timing finding.** The full suite without the k19 test file
  took 124 s and 127 s on this host (load average about 5.5). That exceeds
  the frozen 120 s pytest budget of `diagnostic:self-check`
  (`k19-self-check-01` timed out). The second baseline run also failed
  `test_close_acknowledgement_read_at_p_is_late[exited]` once. This leaf does
  not raise the budget. The report hands both findings to a later leaf, and
  the k19 files have their own checker run.
- **Linux run and one plan revision.** `k19-route-linux-01` ran once. The
  Linux route works: one owned tart clone started in 19.4 s, and agent health
  answered. The 9 guest agent files match the installed 2.8.0 bundle.
  The host plan stopped at `doctor` (exit 1, only `login_item` failed), so
  its app signature and missing-golden calls never ran. That failed sample
  stays as it is. Before the macOS run, `doctor` moves to the end of the host
  plan. No bound or oracle changes.
- **macOS run.** `k19-route-macos-01` ran once with the revised order. The
  macOS route works: one owned clone started in 16.9 s, and agent health
  answered. The guest agent hash `1282f4cf…` differs from the installed
  2.8.0 bundle agent `22dd56ce…`. Every host call before `doctor` passed.
- **Failed acquisition result.** On both OSes a missing `--base` gave exit 1
  (`QEMU_FAILED` on Linux, `TART_FAILED` on macOS), not the documented exit 3.
  Each disposal still proved `VM_NOT_FOUND` and absence. The report hands the
  exit code to a product leaf.
- **Review.** This leaf inserts a `review-impl` leaf ahead of k3, because k3
  is the first causal consumer of this route and checker.
