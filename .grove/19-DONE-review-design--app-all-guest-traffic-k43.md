# app-all-guest-traffic-k43

**Reviews:** app-all-guest-traffic-k42

## Goal

Read the design of `app-all-guest-traffic-k42` adversarially, in fresh context, before a `planning` session cuts increments from it. Produce findings, not fixes.

## Context

- The design: `docs/specs/testanyware-app.md`, ADR-0004 and ADR-0019, as `app-all-guest-traffic-k42` reworks them. Find the producer's commit by the handle `app-all-guest-traffic-k42`.
- The producer's charter: its body states the owner's rule and the owner's narrowing. A finding that reopens either says so, and it names the evidence.
- The decision log of `non-macos-runtime-check-k40`. It holds the failure that caused this design and the owner's words.
- The root brief of the grove and the node brief of `app-release-k21`.

## Done when

- Each doubt below has a finding, or a sentence that says why it holds.
- The ADR set is checked as a minimum coherent set. Each citation of ADR-0004 and ADR-0019 is checked across `docs/`, the code comments and `CONTEXT.md`.
- The spec is checked against each line of the root brief's done-when, in particular "Each command, flag, environment variable, VM id, exit code and `--json` field keeps its meaning" and "Only the user who runs the app can reach it".

## Notes

Doubts that the producer could not settle in its own session:

- A stream to an arbitrary guest port may widen what a process on loopback can reach. Check that the app limits a request to the ports and guests that it owns.
- The harness test processes may need a library layer that hides the app protocol. Check that this layer does not change a flag, an exit code or a `--json` field of the Host CLI.
- Linux and Windows hosts keep the direct path and operate identically. Check that no wording of the spec changes their behaviour. Check that no design element is specific to a Windows host. The owner treats driving from a Windows host as future work, through WSL. Windows guests stay in scope.
- `vm create-golden` stays direct. Check that the design names it as the one direct traffic and says why.
- The design voids the candidate `v2.4.0`. Check that it says which leaf rebuilds the candidate.
- Check that each increment is small, ordered, and demonstrable on its own.

Doubts that the producer `app-all-guest-traffic-k42` could not settle alone:

- The design answers "a stream to port 22" with "no client needs one". The Linux HUT becomes the Linux golden, reached through its agent. Check that this keeps what ADR-0009 needs from the harness. The library check stands in for the stock image. Check that the check covers what the stock image covered, and that the open risk names the rest.
- The route for an explicit endpoint still relays to any address of the same user. The owner's rule says that the app alone knows where a VM runs. Check that keeping `--agent`, `--vnc` and `--connect` with their meaning does not break that rule, or name the conflict for the owner.
- The forward of the Windows harness listens on loopback, where another local user can connect during a run. Check that the spec states this exposure plainly enough for the owner to accept it.
- The increments are in the planning leaf `app-all-guest-traffic-increments-k45`, not in the spec. Check them there.

## Decisions (running log)

- **Review boundary.** The reviewed producer is commit `880100aa018707d9a96a217ba2a133b54966b9fb`, `app-all-guest-traffic-k42`. Its parent is `2cfabbc4`. The working copy was empty at the start. This session inspects the design and its source evidence only; it runs no test, build, lint or format command and changes no production or test code.
- **Source evidence replaces the unavailable graph.** No indexed project matches this workspace. `index_repository` failed with `CBM index worker could not start: a pre-coordination or unverified CBM generation is active`. The subsequent coverage request reported `project not found or not indexed`. There is no graph generation or clean coverage result to cite. The review therefore reads the relevant source directly, and makes no claim that the graph proves complete coverage.
- **The app library and the golden exception hold.** Reusing `testanyware_cli::app_route` keeps the protocol headers, version check and error translation below the harness. A caller-selected agent timeout is necessary for provisioning, and does not change the command surface. Golden creation and its live tests remain direct for the owner's stated Terminal context. Neither choice needs another module or ADR.
- **The Linux HUT choice needs an explicit backend bound.** `vm start --platform linux` can choose QEMU when a same-named qcow2 exists, while the first planned increment assumes a tart gateway and peer address. This is a concrete failing configuration, recorded as F1 below.
- **Two owner rules remain unresolved.** The later rule that only the app knows VM placement conflicts with the retained explicit-endpoint route unless its compatibility exception is stated. The harness loopback forward also serves requests from another local user through an authorised harness process. The spec describes that exposure plainly, but the recorded owner decisions do not accept an exception to the one-user requirement. These are F2 and F3 below; this review does not make either decision for the owner.
- **The replacement library check proves less than the stock-image run.** It checks the Host CLI and its bundled libraries at load time. The existing OCR band also installs and imports EasyOCR and its native dependencies in the HUT. Moving that band to a desktop can hide a system-library dependency of a Python wheel, which the proposed check never reads. The open risk names the viewer but does not account for this exercised path. This is F4 below; no current failing candidate dependency is asserted.
- **The related viewer record needs a current-state correction.** Its claim to be the only long-lived RFB consumer contradicts ADR-0006 and the glossary. This is F5. The record still owns a distinct thread-ownership decision and does not need merging or replacement.

## Findings

The design needs integration before `app-all-guest-traffic-increments-k45` cuts its implementation leaves. F1 is a failing configuration of the proposed harness. F2 and F3 need the owner's rules reconciled, without silently changing the command surface or accepting a new access boundary. F4 concerns the evidence that a passing harness would supply. F5 is a small inconsistency in the related ADR set.

### F1 — P2: Bound the Linux HUT to the backend that its forward supports

**Location:** planning charter `app-all-guest-traffic-increments-k45`, lines 26–28 at the reviewed producer commit; `docs/specs/testanyware-app.md:908` and line 928. Resolve the planning handle for its current path after the integration is inserted.

The first increment starts the HUT with `vm start --platform linux --json`, then binds the forward to the HUT's default gateway and accepts only its route's `src`. That is the tart case. The start does not guarantee tart: `wants_tart` in `cli-rs/crates/testanyware-vm/src/lifecycle.rs:954` gives a same-named qcow2 priority over the tart golden. This is an existing, supported Linux-on-qcow2 configuration, described in `CONTEXT.md` too.

With a QEMU HUT, the default gateway is the guest-visible `10.0.2.2`. It is not the address that the host should bind; the spec itself requires `127.0.0.1` for that case. The guest's route source is also not the loopback peer that the host forward sees. The planned first increment therefore fails before its endpoint-driven bands in this configuration. The spec only calls out the other-user exposure for the Windows harness, although a QEMU Linux HUT would have the same exposure.

**Smallest useful correction:** state and enforce that this harness uses the kept-built tart Linux golden, and fail clearly when the selected golden is QEMU. Alternatively, specify the backend-dependent binding and peer policy for Linux too, with F3 resolved for its QEMU branch. Do not change the general `vm start` backend selection to make the test pass. The reversal condition is a deliberately supported QEMU HUT with its own demonstrated forward policy.

### F2 — P2: Reconcile placement ownership with explicit endpoints

**Location:** `docs/specs/testanyware-app.md:262` and line 291; `docs/adr/0004-each-command-opens-its-own-guest-connections.md:101`.

The owner said that the app is the only point that knows where a VM runs and how to reach it. The producer rejects an explicit endpoint for a harness VM for exactly that reason in ADR-0004. Yet the retained `--agent`, `--vnc` and `--connect` route lets a client supply any address and port. `vm start` and `vm list` also continue to publish those endpoints. The design preserves the owner-required compatibility, but it does not state how this fits the later placement rule.

This is also a limit on the claimed port restriction. An endpoint request for `<guest>:22` with stream kind `agent` still produces a raw byte stream to port 22. The relay validates neither the guest protocol nor ownership of that endpoint. Refusing an arbitrary port on the VM-id route is therefore not a restriction on all app requests. The unrestricted endpoint route was accepted on 2026-10-20; the broader placement rule was recorded on 2026-10-21 in `non-macos-runtime-check-k40`. This review does not infer that either owner instruction silently overrides the other.

**Smallest useful correction:** put the conflict to the owner and record the resulting scope. The least disruptive interpretation is that the app owns placement resolution for VM-id requests, while explicit endpoints remain an explicit compatibility exception. If that is accepted, state that distinction in the spec and ADR-0004 and qualify the port-ownership guarantee. If the owner requires placement to be hidden even for explicit endpoints, return that compatibility conflict for a decision before planning; do not remove or repurpose the flags as an incidental implementation fix.

### F3 — P2: The harness forward bypasses the one-user guarantee

**Location:** `docs/specs/testanyware-app.md:925`, especially lines 937–941, against the root brief's “Only the user who runs the app can reach it” and the requirement at spec line 1080.

The spec states the Windows loopback exposure plainly enough to understand it. It has not, however, recorded owner acceptance of that exception. During a manual Windows harness run, a process of another local user can connect to the forward and send agent requests to the macOS golden. The harness then opens the app stream as its own authorised uid. The app's kernel peer check sees the harness, so the other user's request reaches the app indirectly and can invoke the golden agent's process and file operations. Restricting the listener to loopback and to one run narrows the exposure but does not preserve the requirement.

Stopping Windows host distribution does not authorise this exception: the second increment deliberately retains and changes the runnable Windows harness. F1 also determines whether this case can occur in the Linux release harness.

**Smallest useful correction:** the integration must either obtain and record an explicit, bounded owner exception for these manual harness runs, with the root requirement and ADR rationale reconciled, or require a mechanism that authenticates the HUT before any guest stream opens. An exception is the smaller change if the owner accepts it. A document saying “this design accepts” is not evidence that the owner accepted it. This finding reopens only the proposed harness exposure, not the already recorded risk of the hypervisors' own listeners.

### F4 — P2: The library check does not preserve the stock image's OCR dependency check

**Location:** `docs/specs/testanyware-app.md:943` and lines 1302–1306; `docs/adr/0009-self-hosted-host-cli-verification-harness.md:128`; planning charter `app-all-guest-traffic-increments-k45`, line 31 at the reviewed producer commit.

The proposed check reads only the Host CLI and its bundled shared libraries. That can check their load-time dependency closure. The old stock-image harness also exercised another native loading path: `provision_ocr` installs EasyOCR into a fresh venv and constructs its reader (`linux-host-harness.rs:1127` and line 1174), and `check_find_text` drives the daemon at line 1226. `vision/stages/text-ocr/pyproject.toml:6` identifies torch, OpenCV, Pillow and NumPy among the transitive dependencies. The Linux golden adds `ubuntu-desktop-minimal` (`golden_linux.rs:454`).

A native OCR wheel can therefore acquire a desktop-library dependency, be satisfied by the golden, and pass the OCR band without appearing in the proposed binary-and-ffmpeg check. The stock-image run exercised that import on the smaller system. This differs from the untested viewer's Vulkan loading. The risk's statement that later loading was outside the stock-image check does not account for the OCR path that the old harness actually ran.

**Smallest useful correction:** preserve a check of the native dependency boundary exercised by the OCR band, or explicitly record and seek acceptance of this narrower proof in ADR-0009 and the open risk. Do not claim that inspecting the candidate executable's libraries replaces all useful stock-image coverage. No current candidate wheel is alleged to have a missing dependency; this is a lost verification boundary.

### F5 — P3: The related viewer ADR still contradicts the current consumer set

**Location:** `docs/adr/0005-embedded-viewer-eframe-and-dedicated-rfb-thread.md:15`.

The viewer record says that it is the “only long-lived RFB consumer” and that every other command is short-lived. ADR-0006's current decision explicitly makes `screen record` the second long-lived consumer, and the glossary agrees. This predates the producer, but the review's required minimum-coherent-set and citation check exposes it: a reader following ADR-0004 through the related records receives incompatible current claims.

**Smallest useful correction:** state the current two-consumer relationship in the existing viewer record. Keep its thread ownership decision; no merged ADR, new record, or history chain is needed.

## Charter checks

These checks assess the committed design, not an implemented or tested result.

| Review doubt | Result |
|---|---|
| Requests are limited to owned ports and guests | The VM-id route has the stated bound. Explicit endpoints are unrestricted; F2 qualifies the wider claim. |
| A library hides the app protocol without changing the CLI | Supported. `app_route.rs:101`, line 190 and line 235 already own stream opening, the version observer and the agent connector. Exposing the stream operation and allowing a provisioning timeout extends that existing boundary. The harness need not construct headers or paths. |
| Linux and Windows host behaviour remains identical | The host command paths remain direct, with the existing test-socket override. The new bindings concern guest backends on a macOS host, not a Windows-host design. Windows host distribution removal belongs to the already-live `release-without-windows-host-k44`, which also owns its ADR and brief corrections. |
| Golden creation is the one direct traffic | Supported by the traffic table and the dedicated section at spec line 970. The owner explicitly includes its live tests and Terminal context. No helper is introduced. |
| The invalid candidate has a rebuild owner | Supported by `app-all-guest-traffic-increments-k45`: it must cut the rebuild in `app-release-k21`, after the traffic increments and `release-without-windows-host-k44`, before `non-macos-runtime-check-k40`. The planner has not run, so the rebuild leaf does not yet exist. |
| Increments are ordered and independently demonstrable | The Linux migration includes its library entry points and a live demonstration; the remaining dials form the next complete migration; candidate rebuilding follows both. F1 and F4 must be reflected in the first slice's contract. |
| Removing SSH preserves ADR-0009's runtime proof | The agent can stage and run the same three smoke bands, and no cross-binary SSH band is removed: guest lifecycle stays compile-only. The new dependency on a healthy Linux golden is stated. F4 limits the claimed replacement for stock-image evidence. |
| Explicit endpoints fit the owner's placement rule | Unresolved; F2. Preserving the flags is deliberate and must not be silently withdrawn. |
| The Windows forward's exposure is stated plainly | Yes. Owner acceptance and its effect on the root requirement remain unresolved; F3. |
| Increments are checked in the planning charter | Read in `app-all-guest-traffic-increments-k45`, not inferred from the spec. |

| Root done-when | Design evidence and limit |
|---|---|
| Each guest-reaching command, including explicit endpoints, uses the app; only golden creation stays direct | Traffic table, protocol routes, the two migration slices and the dedicated golden section. F2 concerns placement knowledge, not who performs the dial. |
| An unreachable app causes failure with a reason and no direct fallback | Start and socket sections; `APP_NOT_INSTALLED` / `APP_UNREACHABLE`; stream failures remain app failures. |
| Linux and Windows hosts retain the direct path | “Linux and Windows hosts”, and the existing explicit socket override for tests. Distribution narrowing is separately owned by k44. |
| VMs outlive the app and are adopted | “Lifecycle and adoption” retains detached processes, identity checks and bounded probes. This design changes none of that state ownership. |
| Commands, flags, variables, VM ids, exit codes and JSON fields keep their meaning | Existing command handlers and errors remain above the byte-stream boundary; endpoint fields remain guest endpoints. F2 requires the placement rule to be reconciled without breaking this compatibility. |
| After setup no agent waits for a person; login starts the app; clients never start it | Startup, bounded Local Network diagnosis and upgrade sections keep failures and remedies instead of waiting or restarting the app. |
| Only the user who runs the app can reach it | Direct socket access is bounded by directory permissions and kernel uid. Indirect harness access is unresolved in F3. |
| One optional Rhai file configures both processes | Shared evaluator and precedence are retained in “Configuration”; no additional configuration source is proposed. |
| doctor reports install, running, grant and login state with remedies; llm-instructions explains the app | The status protocol and command-reporting sections retain each item, the failure rules and setup instructions. |
| Version mismatch is never silent | The existing protocol handshake, failure and one-notice-per-command rules remain below the harness library operation. |
| One tagged release installs the signed app with the CLI through the tap | Packaging section and `app-release-k21`; k45 explicitly sequences a new candidate before its checks. No release was built or published by this review. |
| Acceptance after the grant and upgrade identity checks pass on this Mac | The Taskfile live seams and later release leaves require these runs on the rebuilt candidate. This review supplies no new runtime or grant evidence. |

## ADR and citation check

The related records have distinct responsibilities: connection ownership in ADR-0004, target-host runtime evidence in ADR-0009, denial attribution in ADR-0019, release identity in ADR-0020 and configuration ownership in ADR-0021. Reworking the first, second and third in place was appropriate; this design needs no additional ADR. Their unresolved guarantees are F2–F4. F5 is the adjacent viewer record's remaining current-state contradiction. The numbering and older record templates are existing repository conventions; this review proposes no set-wide formatting migration.

The citation check enumerated references from the tracked file list with `jj file list`, excluding `.grove/`, and searched the ADR numbers and filename stems. It covered `docs/`, source comments, manifests, scripts, `Taskfile.yml`, `CONTEXT.md` and the website's duplicate component page. The older migration plan's `0004-center-distance-matching` is a different historical record, not a broken reference to the guest-connections ADR. Dated research remains evidence. The app transport comments, denial oracle comments and amended CLI contract support the respective decisions. The viewer record's incompatible claim is F5. The website component page omits the app explanation but does not require a direct TCP connection. No cited ADR was deleted or renamed by the producer.

## Verification limits and handoff

This is an inspection-only review. The producer recorded an `objdump -p` reading of candidate `v2.4.0`; that evidence belongs to the old candidate and was not re-run here. The failed candidate harness in `non-macos-runtime-check-k40` never staged or ran the candidate. It supplies the denial's provenance, not runtime success. All implementation and post-fix tests belong to the paired integration and later implementation leaves.

The graph index and coverage calls failed as recorded above. Direct source reads establish the specific paths and counterexamples in these findings. The dial enumeration required of the implementation remains future work; this review does not claim to have proved the absence of every possible direct dial.
