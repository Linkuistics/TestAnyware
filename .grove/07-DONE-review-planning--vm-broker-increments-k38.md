# vm-broker-increments-k38

**Reviews:** vm-broker-increments-k4

## Goal

Read the planned subtree of `vm-broker-increments-k4` with fresh context. Find the defects that would make an implementation session fail or build the wrong thing. Report findings and fix nothing.

## Context

- The subtree is every node and leaf from `agent-client-hyper-k7` to `release-publish-k37`, and the root brief.
- The design is `docs/specs/testanyware-app.md` and the ADRs 0004, 0019, 0020 and 0021.
- The owner asked for small increments and no rabbit holes. Agents under herdr are blocked today, so an early unblock matters.

## Done when

- Each finding names the leaf or node, the defect and the session that it would hurt.
- The review checks that each increment leaves the product working and has a test or a live check that proves it.
- The review checks that the dependency order holds, and that no requirement of the spec lacks a leaf.
- The review checks that each leaf fits one session.

## Decisions (running log)

**The review found five planning gaps that need triage before implementation.**
The evidence is the producer's committed subtree, the agreed spec and the
existing source cited below. The gaps concern the boundary between adoption
and legacy teardown, activation of configuration before its consumers, the
diagnostic exception, release candidate preparation and non-macOS runtime
verification. This review changes none of the implementation tasks or design.

## Findings

Reviewed producer: `vm-broker-increments-k4`, commit
`15985c4de06b9d3611c57ec7452a8b7dac587d1c`. Locations below name paths at that
commit, before insertion of the integration leaf renumbers the siblings.
Handles remain stable. P1 identifies a boundary that can endanger running VM
state; P2 identifies a working-increment or verification gap.

### F1 — P1: The signal guard precedes the safe stop of an older QEMU guest

Location: `app-adoption-k30`,
`.grove/12-k14/02-impl--app-adoption-k30.md:17`, and its note at line 24;
`qemu-legacy-stop-k31`.

The adoption leaf makes proof of process identity a condition of every signal,
but deliberately leaves stopping an older QEMU guest to the next leaf. That
guest has no recorded identity. This is an expected input: guests started
before `vm-process-identity-k15`, including those started by the earlier app
lifecycle increment, have older metas.

The existing QEMU stop is not a safe interim implementation. In
`cli-rs/crates/testanyware-vm/src/lifecycle.rs:519` it calls QEMU teardown and
then removes the sidecars at line 525. Teardown in
`cli-rs/crates/testanyware-vm/src/qemu.rs:437` signals QEMU, finds and signals
swtpm, and removes the clone and session directories. Keeping that path
violates the new guard. Merely suppressing its unverified signals still leaves
destructive cleanup without proof that QEMU stopped. Refusing the operation
and retaining everything avoids that danger, but leaves a previously stoppable
legacy guest without its normal stop path until a sibling lands.

The affected session is `app-adoption-k30`: it cannot deliver its guard and
preserve legacy QEMU stop behaviour using only its assigned work. The
spec's older-QEMU failure scenario requires preservation of the clone and
sidecars, not just absence of signals. Put the safe legacy stop and its failure
preservation before, or in the same working slice as, activation of the guard.
The boundary needs a legacy-meta stop check as well as the reused-pid check.

### F2 — P2: Configuration becomes active in the CLI before the app can consume it

Location: `config-evaluator-cli-k18`,
`.grove/14-k17/01-impl--config-evaluator-cli-k18.md:16`, followed by
`config-in-app-k33` and `golden-directory-check-k34`.

The first configuration leaf makes the Host CLI honour the file on every
host. By then macOS routes through the app by default, and the app refuses
different directories. However, the app cannot evaluate the file until k33.
For example, a valid file naming a new `state_dir` makes the newly configured
CLI incompatible with its app. Restarting the app cannot fix that mismatch at
the end of k18: that app still has no file evaluator.

The split also exposes the problem that the agreed golden-directory check was
intended to prevent. The CLI can create a golden in a configured `data_dir`
while the app still uses its defaults; k34 owns the creation-side check. The
creation can report success in a location the app does not read. The spec at
`docs/specs/testanyware-app.md:580` explicitly closes that gap.

The affected session is k18. Its Linux and Windows delivery can be useful on
its own, but unconditional activation on macOS depends on later siblings to
work. Define an expand step that leaves those macOS settings inactive, or
activate the shared evaluation and required directory safeguards together.
Include a real CLI/app check with a non-default directory at that activation
boundary; the proposed precedence-only unit tests cannot detect the split.

### F3 — P2: The configuration leaf postpones its own doctor exception

Location: `config-evaluator-cli-k18`,
`.grove/14-k17/01-impl--config-evaluator-cli-k18.md:18` and line 26;
`doctor-app-check-k20`.

The done-when says every command except `doctor` refuses an invalid file. The
note assigns the doctor exception to k20, after the rest of the configuration
increment. These instructions disagree at the k18 handoff. Implementing the
note leaves doctor blocked by the same configuration error it must diagnose;
implementing the exception alone leaves no assigned diagnostic check to report
the error and produce the required failing report.

The spec at `docs/specs/testanyware-app.md:770` requires a `config` check that
carries the error and exit 1, rather than the ordinary `CONFIG_INVALID` error
envelope with exit 2. Its other checks must still run with values resolved
without the file. This behaviour is needed as soon as invalid configuration
can block commands, including on Linux and Windows, not only when app
diagnostics arrive.

The affected session is k18; k20 currently owns a prerequisite of k18's stated
completion. Assign the config check, fallback values and invalid-file CLI test
to the activation of configuration. The later leaf can still add the app
status, login item and adoption diagnostics independently.

### F4 — P2: Release candidate preparation has an unresolved dependency on the final tag

Location: `app-release-bundle-k22`,
`.grove/16-k21/01-impl--app-release-bundle-k22.md:25`;
`upgrade-identity-check-k36`; `release-publish-k37`,
`.grove/16-k21/03-impl--release-publish-k37.md:15`.

The release-bundle leaf must run a local release build. The upgrade leaf then
needs two locally installable cask versions, without publication. Only after
the upgrade and release acceptance checks does k37 ask the owner to approve
the tag and publication. The existing build entry point does not support that
order: `scripts/release-build.sh:130` requires a clean, exactly tagged tree,
and its main function calls that check at line 439 before deriving the version
from the tag. `Taskfile.yml` wraps this same entry point.

No leaf assigns a way to prepare the two candidate versions before that tag
step, or explicitly settles an earlier local-tag step. Merely rendering a cask
template does not provide the two versioned formula/app artifact sets needed
for the upgrade check. The affected sessions are k22 and k36: they must invent
release preparation outside the planned order or reach k37's approval step
before its evidence exists.

Specify candidate version inputs and local artifacts in k22, or explicitly
place the necessary local tags before the checks. State how k37 publishes the
candidate that passed acceptance, so approval does not cause a different,
unverified build to replace it. This finding concerns the repository's current
build contract; it makes no claim that local tags themselves publish anything.

### F5 — P2: No leaf owns runtime verification of the changed Linux and Windows clients

Location: `agent-client-hyper-k7`, `config-evaluator-cli-k18`, and
`release-publish-k37`; root brief, Linux and Windows compatibility obligation.

The shared HTTP transport changes on every host in k7, and configuration
changes command startup on every host in k18. The assigned checks are the
ordinary workspace suite, cross-build compatibility and the macOS herdr run.
After reading every planned leaf, neither the migration nor the release has an
assigned Linux or Windows runtime run.

The existing runtime harnesses are explicitly ignored tests:
`cli-rs/crates/testanyware-cli/tests/linux-host-harness.rs:1246` and
`cli-rs/crates/testanyware-cli/tests/windows-host-harness.rs:1141`.
`task cli:test` uses `cargo test --workspace`, so it does not run them. The
release build and publish main functions also do not invoke them. Consequently
the listed checks can all pass without executing the changed direct-path
agent client on either supported non-macOS host.

ADR-0009, Decision and Consequences, requires the native aarch64 host-pass and
harness before distribution of that OS; it separately accepts build-only
coverage for x86_64. The affected session is k37: it can meet its own listed
conditions and publish despite missing the established runtime evidence.
Assign the existing Linux and Windows harness runs before publication, using
the candidate binaries, and carry forward the explicit x86_64 limitation.
This needs no new fake backend or test seam.

## Coverage and slice assessment

This table accounts for the planned increments and their assigned checks. It
assesses the plan; it does not claim that any implementation or check passed.

| Increment and leaves | Assessment |
|---|---|
| `agent-client-hyper-k7` | An independently testable transport refactor: existing TCP assertions, a non-TCP stream and streamed-body checks. F5 adds the missing non-macOS runtime obligation before distribution. |
| `app-streams-k8`: k9, k23, k24, k25 | The raw status/socket tests make k9 independently verifiable; fake guest relay tests do the same for k23. Agent routing then introduces the reusable real-CLI harness, which VNC routing consumes. Opt-in routing preserves the earlier product default. UID, path, directory, half-close, transfer and explicit-endpoint obligations have owners. |
| `app-bundle-k10`: k11, k26 | The shell and signed development bundle can be opened and queried before the default changes. k26 owns migration of existing macOS CLI tests and the first herdr drive. Keeping lifecycle direct here is an explicit transitional choice, not a missing final route. |
| `app-lifecycle-k12`: k13, k27, k28, k29 | Listing, starting and stopping have separately observable outputs. The plan assigns fake/injected checks for list and stop, request progress and cancellation checks for start, a live start, and then the full herdr run. Shared sidecars support the partially moved command set. |
| `app-adoption-k14`: k15, k30, k31, k32 | Identity recording can expand the meta compatibly, and the final live check proves survival/adoption. F1 concerns the unsafe dependency boundary between the intervening guard and legacy stop leaves. |
| `app-version-skew-k16` | Headers and range refusal already exist. This leaf owns compatible-skew notices, each remedy and forged-build harness checks. No later feature is needed to observe them. |
| `app-configuration-k17`: k18, k33, k34 | Evaluator restrictions, precedence, app fallback and golden directory checks all have final owners. F2 and F3 concern activation order and the diagnostic prerequisite, not an absent final configuration feature. |
| `app-diagnostics-k19`: k20, k35 | App status, remedies, JSON schema and required document corrections have owners. The basic invalid-config doctor path must arrive earlier, as F3 describes. |
| `app-release-k21`: k22, k36, k37 | Bundle identity, notarisation, cask contents, local upgrade, release acceptance and publication are assigned. F4 leaves candidate preparation unresolved; F5 leaves non-macOS release validation unassigned. |

The final feature coverage matches the spec's requirement headings: routing
and no client startup; the owning-user boundary; survival and adoption;
version skew; denial diagnosis; upgrade identity; configuration; doctor; and
the non-macOS direct path. The findings above distinguish missing verification
or prerequisite behaviour from features that merely land later by choice.
The accepted upgrade reauthorisation cost and the disclosed `TART_HOME` limit
are not reopened.

The leaf sizes were also inspected. Most leaves have one observable command,
one decision or one live run. k11 explicitly allows a split of bundle tooling
from the shell. k27 and k22 have the greatest sizing uncertainty: k27 combines
resident-process child handling, request progress and cancellation with the
existing lifecycle; k22 combines signing/notarisation and release tooling.
No measured session bound establishes that either must be split now. Their
implementers retain Grove's decomposition procedure. The concrete dependency
failures above do need resolution before those sessions consume this plan.

## Inspection limits

This session inspected the producer's committed change list, every node and
leaf of the planned subtree, the root brief, the glossary, the spec, its four
decision records and ADRs 0009, 0010 and 0018. Targeted direct source reads
checked the teardown, paths/meta, transport, lifecycle and release assumptions.
The existing `Taskfile.yml` was read; no reusable development task changed.

There is no usable code graph generation for this checkout. `list_projects`
had no matching project. `index_repository` failed with "a pre-coordination
or unverified CBM generation is active"; graph search and a coverage request
for the evidence paths and bounded plan/release/test scopes returned "project
not found or not indexed". Evidence therefore comes from direct source reads
and document/literal searches. No graph completeness claim is made. F5's
absence claim is bounded to the inspected planned subtree and release entry
points, not all possible external release procedures.

No tests, builds, lint, formatting, GUI or live checks ran. This review owns
findings only. The adjacent planning integration owns their disposition and
every correction to the subtree.
