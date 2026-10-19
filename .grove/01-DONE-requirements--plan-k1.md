# plan-k1

## Goal

Establish what the `gh-issue-8` grove builds, in the human's own words. Issue #8
asks that all access to TestAnyware VMs goes through one local server. Settle the
open questions that the issue and its sub-issues (#4, #5 and #7) raise, and cut
the next leaf.

## Context

- GitHub issue #8 and its sub-issues #4, #5 and #7. Read each with
  `gh issue view <n> --repo Linkuistics/TestAnyware`.
- ADR-0004: the Rust CLI has no persistent server. Issue #8 reverses its premise.
- ADR-0019: the CLI diagnoses a Local Network denial and does not route around
  it. With issue #8, no client meets the denial.
- ADR-0009, ADR-0010 and ADR-0018: the verification harness, the host-side
  framebuffer gate and the VNC keeper.
- `docs/research/macos-local-network-platform-binary-exemption.md`: the herdr
  evidence.
- Apple TN3179, "Understanding local network privacy", section "macOS
  considerations".

## Done when

- The decision log below answers each open question, or names the leaf that
  answers it.
- The root brief states the goal of the grove, its done-when roll-up and its
  pointers.
- `CONTEXT.md` defines each term that the decisions resolved.
- The next leaf exists, and its body names what it must deliver.

## Notes

Issue #8, in the human's words:

<!-- tc: off because this is the human's issue text, quoted verbatim -->

> Not only does this cover all of the sub-issues that are linked, it also solves
> the problem where LLM's running under herdr, which don't have local networks
> permission, can't access TestAnyware. If instead TestAnyware was a localhost
> server and it abstracted over communication with the VMs, it would solve this
> problem.

<!-- tc: on -->

## Decisions (running log)

**The server is an app that the user can grant permissions to.** The human said:
"I was thinking the localhost server would be an app that could be granted
permissions." TN3179 calls the app bundle the responsible code. macOS records
the Local Network choice of the user against it, for the whole app.

A client such as a CLI under herdr then connects only to loopback. The research
document measured that an ad hoc binary reaches a loopback listener while it is
denied the guest subnet.

Two requirements follow. A grant must survive each update, so the app needs a
stable signing identity. A `Developer ID Application` identity exists on this
Mac. TN3179 also says that macOS keeps Local Network state for each user
account. So one app serves the login session of one user.

**This grove delivers the server and closes issue #8, and nothing more.** The
human said: "I worry that a lot of work means many rabbit holes and loss of
focus. Let's do the server as a first deliverable, because it's blocking other
agents. Then we can continue with the other issues, with a delivery for each."

Issues #7, #4 and #5 follow later, each as its own delivery. The design leaves
room for them, but this grove builds none of them. The urgency is that an agent
under herdr cannot reach a macOS guest today.

**Every command that reaches a guest goes through the server, except golden
creation.** The human chose this from three options. The server carries
`vm start`, `vm stop`, `vm list` and `vm delete`. It also carries every `agent`,
`input`, `screen` and `file` command, `screen record` and `viewer` included.
`vm create-golden` stays a direct CLI operation in this delivery.

So an agent under herdr still cannot build a golden image. A human runs that
once for each platform, from a context that has Local Network access, such as
Terminal.

The human rejected two other options. "Everything" also moved golden creation,
with its long jobs. "Only what herdr needs" left VNC as a direct loopback
connection, which later issues would have to move anyway.

**The server exists on macOS hosts only, and on macOS it is the only path.** The
human chose this from three options. On a macOS host the CLI never connects to a
guest itself. This includes explicit `--agent`, `--vnc` and `--connect`
endpoints. If the app is not reachable, the command fails and says why.

Linux and Windows hosts keep the direct path of today, so the CLI has two ways
to reach a guest. A later delivery can bring a server to those hosts. The human
rejected a silent fallback to direct access on macOS. Such a fallback would
bring back the herdr failure with no reason given.

**The component is the VM broker, and it runs in the TestAnyware app.** The
human chose these two terms, and `CONTEXT.md` now defines both. The glossary
already tells sessions to avoid "the server", because the agent and the
hypervisor VNC endpoint are servers too. From here on, this log uses the two
terms.

**VMs outlive the TestAnyware app.** The human chose this from three options. A
quit, a crash or an update of the app leaves each running VM running. When the
VM broker starts again, it adopts each VM it started and serves it again. While
the broker is down, a command fails and says why, and no VM is lost.

This fits the code of today. `vm start` already detaches `tart run`, QEMU and
the VNC keeper, and `vm list` already rebuilds its view from `tart list` and the
sidecar files. The human rejected two options. One stopped the VMs with the app.
The other asked the user on a deliberate quit.

**The CLI surface stays compatible, and the spec file stops being a client
contract.** The human chose this from three options, and added: "TestAnyware is
used by many projects". Each command, flag, environment variable, VM id, exit
code and `--json` field keeps its meaning. A client project needs one new step:
it installs the TestAnyware app and allows it once.

The on-disk `<id>.json` spec is no longer documented for direct readers. On
macOS, a client that reads it and connects itself goes around the VM broker.
The human rejected two options. One kept the spec file as a contract. The other
let a new major version break the surface.

A sweep on 2026-10-19 supports the choice. It read every project under
`~/Development` that mentions TestAnyware, the workspace copies included. No
code reads the spec file, takes an endpoint from `--json` output, or calls a
guest itself. Each match was a record of `vm start --json` output, or a document.
The APIAnyware Racket developer guide names the spec file path, but it tells
readers to prefer `--vm <id>`. Koine also runs `tart list` for liveness, which
the VM broker leaves unchanged.

**The delivery is a release through the tap, proved from herdr.** The human
chose this from three options. The grove ends with a release leaf, as the
earlier groves on this repo did. That leaf publishes a tagged version to
`linkuistics/taps`, and the version installs the signed TestAnyware app with the
CLI. Other projects then get the VM broker with `brew upgrade`.

The acceptance run happens on this Mac, from a herdr session, after the
one-time grant. A macOS guest starts, an agent, screen, input and file command
each act on it, and it stops, all through the VM broker. A session of this
grove runs under herdr, so it can make that run. The human rejected two
options. One added a run of a consumer project. The other ended at a merge to
`main`.

**Tests reach the work through three seams.** The human agreed to this proposal.
The one new seam is the protocol of the VM broker. A test starts a real VM
broker on a temporary socket and state directory, and drives it with the real
CLI binary. The guests behind it are the existing fakes on loopback: a wiremock
In-VM agent and a synthetic RFB server.

Decisions inside the VM broker, such as adoption after a restart, use the
existing seams of pure functions and injected probes. The existing Taskfile
live checks carry two runs. One is the herdr acceptance run. The other checks
that the Local Network grant survives an upgrade of the app. The human rejected
a fake VM backend, which was a second new seam, and live checks alone.

**The human agreed the shared understanding, and added a research leaf.** It
covers the decisions above and four derived requirements:

1. After the one-time grant, an agent never waits for a human. When the app is
   not running, the CLI starts it.
2. Only the user who runs the app can reach its VM broker. The agent's `/exec`
   runs any command in a guest, so other local users and the network get no
   access.
3. `testanyware doctor` reports whether the app is installed, running and
   allowed, each with a remedy. The `llm-instructions` of the CLI describe the
   app. The `using-testanyware` skill in the Grove repo needs no change in this
   delivery, because it already sends agents to `doctor` first.
4. A version mismatch between the CLI and the running VM broker is never
   silent. The CLI restarts the VM broker, or fails with a remedy.

The human chose one prior-art research leaf ahead of the design leaf. The
rejected proposal let the design leaf read prior art as it goes. So the tree runs
`vm-broker-prior-art` (research), then `vm-broker-design` (design), then
`vm-broker-increments` (planning).
