# vm-broker-design-k5

**Reviews:** vm-broker-design-k3

## Goal

Read the agreed design of the TestAnyware app adversarially, in fresh context,
before `vm-broker-increments-k4` cuts increments from it. Produce findings, not
fixes.

## Context

- The design: `docs/specs/testanyware-app.md`, ADR-0004, ADR-0019, ADR-0020 and
  ADR-0021. Find the producer's commit by the handle `vm-broker-design-k3`.
- The evidence: `docs/research/testanyware-app-local-network-grant.md` and
  `docs/research/vm-broker-prior-art-a.md`.
- The decision logs of `plan-k1` and `vm-broker-design-k3`. They hold the
  owner's words and each rejected option. A finding that reopens an owner's
  choice says so, and it names the evidence.

## Done when

- Each doubt below has a finding, or a sentence that says why it holds.
- The ADR set is checked as a minimum coherent set, and each citation of the
  four ADRs is checked across `docs/`, the code comments and `CONTEXT.md`.
- The spec is checked against each line of the root brief's done-when.

## Notes

The producer could not settle these doubts in its own session:

- **The pinned UUID.** One row of the measurements supports it: new code with
  the old UUID, on a build that was not notarised. Check whether a pinned
  `LC_UUID` is safe with `codesign`, notarisation and the crash reporter. Check
  also whether the launcher split, which the ADR rejects, is the safer default.
- **Explicit endpoints.** The app relays a stream to any TCP address that a
  process of the same user names. Judge whether that defeats the purpose of
  Local Network privacy more than the requirements need.
- **The guest stream under the agent client.** The agent client uses `reqwest`.
  Check that an HTTP client can run on a stream after an HTTP/1.1 upgrade on a
  Unix socket. If none can, name the change of client that the spec omits.
- **Golden creation and the directories.** Golden creation keeps the direct path
  and resolves its own data directory. The app's `vm start` reads goldens from
  the app's data directory. Nothing checks that the two agree.
- **The Koine start model.** Requirement 1 of `plan-k1` changed. Look for any
  sentence, in the spec or elsewhere, that still says a client starts the app.
- **Adoption of an older sidecar.** It stops a guest by name only. Check that a
  stop by name covers tart, QEMU, swtpm and the VNC keeper.

## Findings

The reviewed producer is commit `fc6071181f93cf140678ea25d22cb113c31d77d8`.
P1 means a design decision needs resolution before implementation is planned;
P2 means a concrete contract gap needs correction. These are findings and
proposed corrections for triage, not changes to the agreed design.

### F1 — P1: Reusing a build UUID for changed code violates Apple's image identity rule

Location: `docs/specs/testanyware-app.md:163` and ADR-0020, Decision and
Consequences.

The release would deliberately give different Mach-O images the same UUID.
[Apple TN3178](https://developer.apple.com/documentation/technotes/tn3178-checking-for-and-resolving-build-uuid-problems)
requires a unique build UUID for each distinct image. The UUID connects a
binary to its debug symbols; stripping the shipped binary does not remove the
need to identify the right symbols or image when investigating a crash. The
ADR records the crash-report cost, but does not address the conflict with this
platform rule. Signing after the edit covers the edited UUID; it does not make
the collision a supported identity scheme. This review found no evidence that
notarisation itself will reject it, and makes no such claim.

The local measurement proves one successful connection with changed code and
the old UUID, on an unnotarised probe. It does not establish the safety of the
release policy. The same evidence already demonstrates a stable launcher with
a changing helper. That arrangement can keep the launcher's image unchanged
and give each changed helper its normal UUID.

Smallest useful correction: reopen the pinned-UUID decision and compare that
launcher arrangement against Apple's requirement before planning the release
build. Record any remaining release validation explicitly. This finding
**reopens an owner-approved choice**, recorded in `vm-broker-design-k3`, on new
primary evidence rather than on a preference for another process layout.

### F2 — P1: The agent client's proposed stream substitution has no specified implementation

Location: `docs/specs/testanyware-app.md:249` and ADR-0004, Decision.

`cli-rs/crates/testanyware-agent-client/src/lib.rs:195–206` owns a `reqwest::Client` and
constructs it with `Client::builder()`. The lockfile selects reqwest 0.12.28.
That client's public builder does not take an already upgraded byte stream.
Its Unix-socket option selects a socket path; it does not perform the app's
upgrade before each guest HTTP connection. Its
[connector layer](https://docs.rs/reqwest/0.12.28/reqwest/struct.ClientBuilder.html#method.connector_layer)
wraps the existing connector and retains reqwest's sealed connection type.
It is not a replacement connector that can return the proposed guest stream.
Sending the agent request straight to the app's socket would send it before
the upgrade and to the wrong HTTP server.

HTTP on the upgraded stream is possible:
[Hyper's HTTP/1 handshake](https://docs.rs/hyper/latest/hyper/client/conn/http1/fn.handshake.html)
accepts an I/O object. The missing decision is how the agent client uses that
capability, and how it retains streamed uploads/downloads, timeout and error
semantics, and any redirect behaviour. RFB already has a generic handshake
over `AsyncRead + AsyncWrite`
(`cli-rs/crates/testanyware-rfb/src/connection.rs:85–89`); reqwest does not
offer the equivalent entry point used by the spec's argument.

Smallest useful correction: specify a viable HTTP transport inside the existing
agent-client module and account for its migration in the spec. Preserve the
agreed raw-stream boundary and test through the real CLI and fake HTTP agent;
this does not require another public test seam or moving commands into the app.

### F3 — P2: Legacy QEMU teardown has no recoverable failure contract

Location: `docs/specs/testanyware-app.md:349–351`.

For an old meta, the app must signal none of its processes and stop QEMU through
its monitor. Today's `qemu.rs:437–454` teardown instead terminates the QEMU pid,
finds swtpm by its state-directory path and terminates that pid, then deletes
the clone and session directories. The old meta (`meta.rs:22–34`) has neither
a swtpm identity nor a monitor path. `paths.rs:67–68` reconstructs that path
from the current process's `TMPDIR`, which need not equal the old starter's.
Thus reusing teardown violates the new signal rule, while replacing just the
QEMU termination with monitor `quit` leaves the swtpm cleanup and failure path
unspecified.

An orderly QEMU exit can cover the normal swtpm case: upstream
[QEMU sends `CMD_SHUTDOWN`](https://github.com/qemu/qemu/blob/master/backends/tpm/tpm_emulator.c),
and [swtpm's handler terminates the process](https://github.com/stefanberger/swtpm/blob/master/src/swtpm/ctrlchannel.c).
That does not cover an unavailable monitor or an already crashed QEMU with a
surviving helper. QEMU launches swtpm without `--ctrl ...,terminate`
(`qemu.rs:305–320`), the
[documented option](https://raw.githubusercontent.com/stefanberger/swtpm/master/man/man8/swtpm.pod)
for termination when the control connection is lost. Deleting state or
forgetting the sidecar before confirming teardown can leave a live process with
no reclaimable record.

Smallest useful correction: specify how legacy monitor/control endpoints are
located, how each backend and helper is stopped without unverified pid signals,
and what remains recorded when that cannot be done. The normal tart case can
use `tart stop`/delete; its keeper normally exits when VNC closes
(`keeper.rs:163–215`). That does not supply the missing QEMU/swtpm contract.

### F4 — P2: Direct golden creation can succeed in a directory the app never reads

Location: `docs/specs/testanyware-app.md:455–459` and `:501–506`.

Directory agreement is checked on requests to the app, but golden creation
explicitly sends none. For example, create a Windows golden in Terminal with
`XDG_DATA_HOME=/Volumes/vms`, leave the app on its defaults, and later start
that golden from a shell with no override. Creation reports success in
`/Volumes/vms/testanyware/golden`; the app looks in its default golden
directory. The later request's directories agree with the app, so the designed
directory check never identifies the mismatch.

`commands/vm.rs:448` resolves the creation process's paths, and
`paths.rs:44–54` puts goldens under its resolved data directory. This is a
concrete gap in the agreed choice to refuse directory disagreement, not a
reason to move golden creation into the app.

Smallest useful correction: define how creation validates or communicates the
shared golden location, including when the app is absent, and add that case to
the configuration scenarios. Keep golden guest traffic direct as the owner
chose.

### F5 — P2: Compatible build skew is silent during the command that encounters it

Location: `docs/specs/testanyware-app.md:373–381` and `:594–604`.

The root brief requires that a version mismatch is never silent. The design
proceeds when builds differ but protocol ranges overlap, and reports the
versions only if someone separately runs `doctor`. With the initial 1–1 range
on both sides, a newer CLI and an older running app can therefore serve ordinary
commands with no indication of the mismatch. Merely putting the versions in
internal HTTP headers does not inform the person or agent using the command.

Smallest useful correction: define an observable notice on the affected
invocation, including its text/JSON channel without changing existing JSON
fields, and add an overlapping-range, different-build scenario. Alternatively
obtain an explicit change to the owner's never-silent requirement.

### F6 — P2: Loopback success is incorrectly treated as proof of a Local Network grant

Location: `docs/specs/testanyware-app.md:424–426`.

Any successful guest connection marks the app allowed. QEMU's agent and the
host-side VNC endpoints are on loopback, which the spec itself says needs no
grant. Adoption probes of those endpoints can therefore report allowed before
the user has granted anything, or overwrite a proven denial with allowed.
`doctor` and the menu then give the wrong answer for the exact tart-subnet
failure this delivery must diagnose.

Smallest useful correction: qualify observations by whether the connection
actually exercises Local Network privacy. A loopback connection must leave the
grant observation unknown or preserve the previous relevant observation; retain
the destination and time needed to explain that observation. Add the sequence
denied tart connection → successful loopback connection to the existing pure
decision tests.

### F7 — P2: The diagnostic command is blocked by the failures it must diagnose

Location: `docs/specs/testanyware-app.md:449–459`, `:486–492` and `:631–643`.

The Host CLI refuses to run when the file fails, and every request with
different directories is refused. The scenario strengthens this to each
command failing with `CONFIG_INVALID`. Yet `doctor` must report that very
configuration failure and fetch the running app's configuration and directory
values. No diagnostic exception is defined. With an invalid file, a client
cannot reach the report; with mismatched directories, its status request is
itself incompatible. The output promised for these recovery cases cannot be
obtained under the general request rules.

Smallest useful correction: define how `doctor` retains a configuration error
as diagnostic data and obtains read-only status without requiring successful
configuration or matching directories. State its exit/envelope behaviour and
exercise invalid-file and differing-directory cases through the agreed CLI/app
test seam. This keeps ordinary commands' refusal semantics.

### F8 — P2: The binding CLI contract still prescribes the old Local Network remedy

Location: `docs/architecture/cli-design-contract.md:314–333`, especially
`:325–328`; compare ADR-0019 and the spec's Local Network diagnosis.

The CLI contract still says remediation tells the user to run testanyware
from an app with Local Network access. On macOS, running the Host CLI from
Terminal will no longer help an already denied TestAnyware app: the connection
is made by the app in every such context. The new decision correctly tells the
user to allow TestAnyware itself, but the producer neither reconciles this
citation nor lists this contract among the implementation's document updates.
The two binding documents prescribe different externally visible behaviour.

Smallest useful correction: reconcile that amendment with ADR-0019 and include
the binding contract in the migration obligations. Preserve the historical
measurement documents as evidence, clearly distinguished from current product
instructions.

## The six requested doubts

| Doubt | Outcome |
|---|---|
| Pinned UUID, signing, notarisation and crash reporting | F1. Editing before signing avoids invalidating an existing signature, but reuse conflicts with TN3178. Notarisation acceptance remains unproved; no rejection is asserted. The measured stable-launcher option warrants reconsideration. |
| Explicit endpoints and Local Network privacy | Supported as an explicit trust choice. The owner requires existing explicit endpoints to work under herdr. Raw relay bytes cannot enforce an agent/VNC-only protocol, so the meaningful boundary is the owning uid. Restricting this to known VMs would break the approved endpoint requirement. The 0700 directory, kernel peer uid check and absence of a TCP listener confine access to that user. This deliberately delegates that user's app grant to their clients; it does not isolate processes of the same user. |
| Guest stream below the HTTP client | F2. HTTP over the stream is viable, but the current reqwest client cannot simply accept it. |
| Golden creation and directories | F4. The request check cannot protect an operation that makes no request. |
| Koine start model | Supported by the current spec's startup section, error remedy, scenario and out-of-scope crash-restart statement, plus `CONTEXT.md`. The earlier requirement and producer question still mention client startup, but the later decision explicitly replaces them. The prior-art survey's startup recommendations are research, and its adoption bridge explicitly records their rejection. Homebrew reopening an upgraded app is the separately agreed packaging operation. |
| Adoption of older sidecars | F3 for QEMU endpoint discovery and failed teardown. The normal tart stop closes VNC and lets the keeper end itself; normal QEMU shutdown can also terminate swtpm. Neither justifies signalling an unverified legacy pid. |

## Check against the root brief

This is a check of what the design specifies, not proof that an implementation
or a release passes acceptance.

| Root done-when obligation | Design evidence and result |
|---|---|
| Every macOS guest command, explicit endpoints included, uses the app; golden creation alone is direct | Solution, protocol and first requirement cover the scope. F2 leaves a material implementation boundary unresolved. |
| App unavailable means an explained failure, with no direct fallback | Startup and unavailable-app scenario specify `APP_UNREACHABLE` and no fallback. Supported. |
| Linux and Windows hosts retain their direct path | Dedicated section and requirement state this, with configuration added separately. Supported as a design obligation. |
| VMs outlive the app and are adopted on restart | Detached lifecycle, atomic sidecars, identity checks and live survivor check cover the main path. F3 leaves legacy recovery incomplete. |
| Commands, flags, environment, VM ids, exit codes and JSON fields retain their meaning | Lifecycle carries parsed options; stream bytes retain guest protocols; existing JSON endpoint fields remain unchanged; directory refusal is owner-approved. F2 must account for the HTTP-client migration. No requirement to preserve direct sidecar readers remains. |
| After setup, no human wait while the app runs; login starts it; clients never start it | Startup, bounded denial diagnosis, login-item menu and explicit no-start requirement cover this. Supported, subject to F1's upgrade identity issue. |
| Only the app's user reaches it | Filesystem access and kernel peer credentials enforce the stated uid boundary. Explicit endpoint delegation is addressed above. The spec discloses existing guest listener exposure; that is distinct from access to the app. |
| One optional Rhai file configures app and CLI | Shared evaluator, limits, precedence and reload policy are specified. F4 and F7 identify directory and diagnostic gaps. |
| Doctor reports installed, running, allowed and login status with remedies; llm-instructions describes the app | Reporting section supplies these obligations. F6 and F7 undermine diagnostic accuracy/access, and F8 leaves a conflicting remedy in the binding contract. |
| Version mismatch is never silent | F5: the compatible-skew branch does not meet this obligation. |
| Tagged tap release installs signed app and CLI | Cask depends on the existing formula; one tag versions both, with Developer ID, hardened runtime, notarisation and stapling specified. F1 challenges the executable identity policy. The cask's one-time installation is the owner's accepted migration. |
| Herdr acceptance succeeds and the grant survives an upgrade on this Mac | Both live runs are explicitly required in Test seams. Release notarisation/quarantine and Homebrew approval carry-over remain disclosed risks; the probe alone is not acceptance evidence. F1 must be resolved before treating the upgrade identity as settled. |

## ADR set and citation check

The four decisions have separate reasons to exist: per-command transport and
ownership (ADR-0004), grant ownership and diagnosis (ADR-0019), release identity
and installation (ADR-0020), and shared configuration (ADR-0021). They replace
the two earlier decisions in place rather than leaving a second current record
of either. Their cross-references form a coherent explanation of the proposed
app; F1 challenges the content of one decision, not a need for another ADR.
ADR-0018 remains necessary because the detached keeper outlives the resident
app and avoids the measured tart crash. No additional ADR is proposed by this
review.

The citation sweep covered `docs/`, `cli-rs/`, `agents/` and `CONTEXT.md`, using
numeric ADR references and both old and new filename slugs. The relevant
matches were read in context:

- `CONTEXT.md`, the new spec, the four ADRs and ADR-0018 reflect the new grant
  owner and per-command connection rule.
- `docs/components/cli.md`, ADR-0005 and ADR-0006 cite the per-command RFB rule
  or retirement of the old shared-VNC process. A byte relay does not reverse
  those claims. ADR-0005's earlier "only long-lived" wording predates record
  and is not a new consequence of this design.
- The code comments in `commands/record.rs`, VM `spec.rs`, `lifecycle.rs`,
  `lib.rs` and `local_network.rs` describe per-command connections or a
  diagnosis made by the connecting process. Those properties survive the
  proposed relocation into the app.
- The earlier Local Network experiment and the prior-art survey are dated
  evidence. Their added bridges point to the new decisions; their original
  measured/rejected alternatives do not prescribe current client startup.
- The remaining binding mismatch is the CLI design contract, F8. Historical
  plans that use ADR-0004 for a different, earlier numbering scheme are not
  current references to the reviewed connection decision.

## Inspection limits

This review ran no tests, builds, lint, formatting, GUI experiments or signing
experiments. It inspected the producer's committed documents, the decision
logs, relevant source and upstream documentation/source. The existing
`Taskfile.yml` was inspected; no reusable development task changed.

Tier 2 graph verification could not be established. `list_projects` had no
project for this checkout; `index_repository` failed with "a pre-coordination
or unverified CBM generation is active". A coverage request for the evidence
paths and the document/source scopes returned "project not found or not
indexed". Consequently there is no usable graph generation, pagination or
coverage assertion. Source evidence above comes from direct reads, with
literal/document searches for citation discovery. No graph-derived negative
or whole-codebase structural claim is made.

The live probe's limitations remain those in its evidence document. Upstream
QEMU/swtpm source supports the orderly-shutdown reasoning, but does not prove
the installed binaries' failure behaviour. The integration owns triage, any
design corrections, and the decision about further verification.

## Decisions (running log)

**F1–F6 identify actionable design gaps.** They were recorded as
their supporting requirement, source or primary documentation became clear.
The review changes no spec, ADR, production code or tests. F1 explicitly names
the owner decision it asks the integration to reconsider.

**The legacy teardown finding is about recovery, not ordinary QEMU exit.**
Upstream QEMU and swtpm source support graceful helper shutdown. F3 records
that support and narrows the gap to endpoint discovery and failed teardown.

**Diagnostic access and the remaining binding citation also need triage.**
F7 and F8 record those contradictions without changing the design or the CLI
contract.
