# Homebrew Binary Distribution — Plan

> Output of the `brainstorm-and-plan-local-homebrew-binary-distribution`
> backlog task. Intended as input to the `writing-plans` skill before
> execution of `local-homebrew-binary-distribution-tap-no-client-builds`.

## Goal

Mirror Ravel-Lite's local-build pattern: end users install `testanyware`
via Homebrew, and golden-image creation consumes pre-built in-VM agent
binaries that ship inside the same Homebrew formula. Neither path
requires the user to run `swift build`, `dotnet build`, or any compiler
on their machine.

## Reference implementation

`~/Development/Ravel-Lite/scripts/{release-build.sh,release-publish.sh}`
plus `~/Development/Ravel-Lite/scripts/templates/ravel-lite.rb.tmpl`.
Pure-local: tag → `release-build.sh` produces per-target tarballs +
rendered formula → `release-publish.sh` does `gh release create` and
copies the formula into `~/Development/homebrew-taps`. No CI.

## Decisions (resolved during brainstorm)

| Question | Decision | Rationale |
|---|---|---|
| Apple Developer ID + notarisation? | **No.** Ad-hoc signing only. | Brew-fetched CLI binaries don't carry the quarantine attribute, so Gatekeeper won't block them. The macOS in-VM agent's TCC grant is computed on the VM at golden-build time from whatever csreq the agent binary actually has — ad-hoc CDHash csreq is what currently works, and a release-channel binary preserves it. |
| Cross-arch macOS builds? | **arm64 only.** No universal2, no x86_64. | TestAnyware's host requirement is macOS 14+ on Apple Silicon (tart needs Apple virtualisation). x86_64 hosts cannot run the integration suite anyway. Universal2 deferred until user demand surfaces. |
| Linux host CLI build? | **No.** | Host CLI links AVFoundation / Vision / CoreGraphics / CoreMedia. macOS-only by design. |
| One bundle tarball or per-artifact? | **One bundle.** Single tarball containing CLI + all three agents + golden scripts + helpers. | TestAnyware has no users who don't use VMs — the per-artifact "save bandwidth for CLI-only users" defense is hollow. CLI and agents already ship under the same tag (wire protocol is shared), so splitting distribution channels for version-coupled artifacts is busywork. Bundling matches the Homebrew idiom for "tool plus auxiliary artifacts" (`qemu` ships firmware blobs in `share/qemu/`, `git` ships templates similarly). Eliminates `gh release download` from the golden-script preflight. Bundle size is well under 100MB compressed — a non-issue for brew. |
| Linux agent in the bundle? | **Yes — ships under `share/testanyware/agents/linux/`.** | Treats all three agents symmetrically: golden scripts read from `$(brew --prefix testanyware)/share/...` regardless of platform. Avoids the asymmetry of "macOS/Windows agents come from brew, Linux agent comes from repo." The Python source is small (a single package) so bundling cost is negligible. |
| Vision pipeline distribution? | **Stay as `uv sync`.** Out of scope. | Multi-package Python workspace; uv handles its own distribution model. |
| Version coupling: CLI vs agents? | **Same tag for both.** | Agent ↔ CLI share the wire protocol (`UnifiedRole.swift` is duplicated; both copies must stay in sync per existing memory). Releasing them together keeps the contract well-defined. |
| Golden script's agent-version source? | `git describe --tags --abbrev=0` of the checkout. | A checkout at v0.1.0 always produces a v0.1.0 golden. Determinism beats "always pull latest". |
| Tap repo? | **Reuse `Linkuistics/homebrew-taps`.** | Already used by Ravel-Lite. Env override `TESTANYWARE_TAP_DIR` mirrors `RAVEL_TAP_DIR`. |
| Release trigger? | **Manual.** `git tag -a v<x.y.z>` then `scripts/release-build.sh` then `scripts/release-publish.sh`. | No GitHub Actions; matches Ravel-Lite. |
| How does a brew-installed user invoke golden creation? | **Open follow-up — pick during P5.** Three candidates: (A) new `testanyware vm create-golden --platform <p>` subcommand that exec's the bundled script; (B) wrapper binaries at `bin/testanyware-create-golden-{macos,linux,windows}` shipping in the formula; (C) just document `bash $(brew --prefix testanyware)/share/testanyware/scripts/vm-create-golden-<p>.sh`. | (A) is most user-friendly and consistent with the existing `testanyware vm {start,stop,list,delete}` subcommands but adds a Swift-side change; (B) keeps formula plumbing minimal; (C) requires zero formula work but is least discoverable. Not a blocker for P1–P4 since brew users don't strictly need this until they create their own goldens (most use a pre-shared image). Resolve before P5 README rewrite. |

## Artifact inventory

All artifacts ship in a single Homebrew formula. The formula's `install`
block lays them out under the brew prefix as follows:

| Artifact | Language | Where it runs | Install path under `<prefix>` |
|---|---|---|---|
| `testanyware` (CLI) | Swift | macOS host (arm64) | `bin/testanyware` |
| `testanyware-agent` (macOS) | Swift | In-VM (macOS golden) | `share/testanyware/agents/macos/testanyware-agent` |
| `testanyware-agent.exe` (Windows) | C# .NET 9 self-contained | In-VM (Windows golden) | `share/testanyware/agents/windows/testanyware-agent.exe` |
| `testanyware-agent` (Linux, Python) | Python source | In-VM (Linux golden) | `share/testanyware/agents/linux/testanyware_agent/` (package directory) |
| Golden scripts | bash | macOS host | `share/testanyware/scripts/vm-create-golden-{macos,linux,windows}.sh` (+ `_testanyware-paths.sh`, `vm-{start,stop,list,delete}.sh`) |
| Helpers | mixed | macOS host + in-VM | `share/testanyware/helpers/` (`set-wallpaper.swift`, `com.linkuistics.testanyware.agent.plist`, `autounattend.xml`, `desktop-setup.ps1`, `set-wallpaper.ps1`, `SetupComplete.cmd`) |
| Vision pipeline | Python (uv workspace) | Host machine | **Not bundled.** `uv sync` from repo (no change) |

`share/testanyware/` over `libexec/testanyware/` because the agent
binaries are not host-side executables — they are payloads `scp`'d into
VMs. Homebrew convention (per `qemu`, `git`) puts non-host-executable
auxiliary files in `share/<formula>/`.

### Release tarball

A single tarball per release:

```
testanyware-v<ver>-aarch64-apple-darwin.tar.xz
```

Layout inside the tarball mirrors the brew install layout — the formula
just `install`s it to the prefix wholesale (or with per-path moves if
the formula prefers fine-grained control). One archive, one SHA, one
formula stanza.

Target triple matches Rust's (`aarch64-apple-darwin`) for parity with
Ravel-Lite's naming and to clarify "this artifact runs on Apple Silicon
macOS" at-a-glance, even though the bundle's *contents* include
non-arm64-Mac payloads (Windows .exe, Linux Python source). The triple
labels the *host* the bundle is consumed on, not the targets the
payloads run on.

## Constraints surfaced by memory

- **`agent-hot-swap-does-not-re-grant-tcc-accessibility`** — replacing
  `/usr/local/bin/testanyware-agent` and reloading its LaunchAgent
  leaves `/health` returning `accessible: false` because TCC pins the
  grant to the original binary's csreq. **Implication:** the brew-shipped
  agent binary must be the one consumed at golden-build time so the TCC
  grant is computed against it. The script flow is unchanged (read agent
  → scp → install → grant); only the *source path* switches from
  `swift build --show-bin-path` to
  `$(brew --prefix testanyware)/share/testanyware/agents/macos/testanyware-agent`.
- **`vm-create-golden-macos.sh` currently does `swift build` inside the
  same script** (lines 262–283 in the `install_agent` function). The
  replacement is a `cp` from the brew prefix.
- **`vm-create-golden-windows.sh` currently does `dotnet publish`**
  (around line 193). The replacement is a `cp` from
  `$(brew --prefix testanyware)/share/testanyware/agents/windows/testanyware-agent.exe`
  into the autounattend tmp dir.
- **`vm-create-golden-linux.sh` currently tars `agents/linux/testanyware_agent/`
  from the repo** (around lines 327–336). The replacement is a tar of
  `$(brew --prefix testanyware)/share/testanyware/agents/linux/testanyware_agent/`.
- **The host CLI itself is also brew-installed.** The macOS golden
  script's recovery-mode VNC commands need a host-side `testanyware`
  binary (`$_TESTANYWARE_BIN` in `_recovery_boot_csrutil`); after the
  switch, `_TESTANYWARE_BIN` resolves to whatever `command -v
  testanyware` returns, i.e. the brew-installed CLI. No more
  in-script `swift build` for the host binary either.
- **`UnifiedRole.swift` is duplicated; both copies must stay in sync.**
  Releasing CLI + agents in one bundle under one tag gives a single
  point at which divergence would surface (the existing
  `TestAnywareAgentProtocolTests` byte-equality test catches it pre-tag).

## Files to create / modify

### New

- `scripts/release-build.sh` — preflight, build CLI + macOS agent +
  Windows agent, gather Linux agent source + scripts + helpers, stage
  the unified layout, package as a single `.tar.xz`, render formula.
- `scripts/release-publish.sh` — `gh release create`, copy rendered
  formula to `$TESTANYWARE_TAP_DIR/Formula/testanyware.rb`, commit, push.
- `scripts/release-doctor.sh` — preflight (gh auth, clean tagged tree,
  required toolchains: swift, dotnet).
- `scripts/templates/testanyware.rb.tmpl` — formula template with
  `@VERSION@` and `@SHA_AARCH64_APPLE_DARWIN@` placeholders. The
  `install` block lays out `bin/`, `share/testanyware/agents/{macos,linux,windows}/`,
  `share/testanyware/scripts/`, `share/testanyware/helpers/`.

### Modify

- `provisioner/scripts/vm-create-golden-macos.sh` — replace the
  `swift build` block at lines 262–283 (`install_agent` body) with a
  resolution of the agent source path: `cp
  "$(brew --prefix testanyware)/share/testanyware/agents/macos/testanyware-agent"`
  into a temp file, then scp into the VM. `_TESTANYWARE_BIN` (used by
  the recovery-mode VNC driver) likewise becomes
  `$(command -v testanyware)`. Add an `BREW_PREFIX_OVERRIDE`-style env
  var so contributors building from source can override
  (`TESTANYWARE_AGENT_BIN_OVERRIDE` and `TESTANYWARE_CLI_BIN_OVERRIDE`,
  defaulting to brew). The TCC-grant flow is unchanged — `csreq` runs
  on the installed binary regardless of source.
- `provisioner/scripts/vm-create-golden-windows.sh` — replace the
  `dotnet publish` invocation around line 193 with a
  `cp "$(brew --prefix testanyware)/share/testanyware/agents/windows/testanyware-agent.exe"`
  into the autounattend tmp dir. Same env-var override pattern.
- `provisioner/scripts/vm-create-golden-linux.sh` — replace the
  `_AGENT_DIR="$(...)/agents/linux"` resolution at lines 327–336 with
  `_AGENT_DIR="$(brew --prefix testanyware)/share/testanyware/agents/linux"`.
  Same env-var override.
- `README.md` — add a "Quick install" section at the top of CLI usage:
  `brew install linkuistics/taps/testanyware`. Demote the existing
  "Building from Source" section to a contributor sub-heading. Mention
  that `testanyware vm create-golden ...` (or running the bundled
  `vm-create-golden-*.sh` from `$(brew --prefix testanyware)/share/...`)
  is the supported golden-creation path for brew-installed users; see
  open question P0.bonus below.
- `.gitignore` — add `/target/dist/` (release-artifact staging).

### Untouched

- `cli/Package.swift`, `agents/macos/Package.swift`,
  `agents/windows/TestAnywareAgent.csproj`,
  `agents/linux/testanyware_agent/` — build invocations and source
  layouts stay identical; only their callers move from in-script
  `swift build`/`dotnet publish` to `cp` from brew.
- `vision/` — out of scope.

## Phases & ordering

```
P0 (decisions)  ──► P1 (release-build.sh)  ──┬──► P2 (release-publish.sh)  ──► P6 (smoke test brew install)
                                              │
                                              ├──► P3 (formula template)
                                              │
P4 (golden scripts) ◄── needs first published release ──┘
                                              │
P5 (README + .gitignore) ── independent ──────┘
```

- **P0** Lock the decisions in this document. *(Done — this file.)*
- **P1** `release-build.sh` (and the `release-doctor.sh` preflight it
  invokes). Builds CLI + macOS agent + Windows agent, gathers Linux
  agent source + scripts + helpers into a staging tree mirroring the
  brew install layout, packages as one `.tar.xz`, computes SHA256,
  renders the formula via sed substitution. Output to `target/dist/`.
- **P2** `release-publish.sh`. Creates a GitHub release for the current
  tag, uploads the single tarball, copies the rendered formula into the
  tap, commits, pushes.
- **P3** `scripts/templates/testanyware.rb.tmpl`. macOS-only block,
  arm64-only branch. Single tarball URL → `bin/testanyware` plus
  `share/testanyware/{agents,scripts,helpers}/` install. Use the
  formula's `bin.install`, `pkgshare.install`, and friends; lay
  everything out idiomatically.
- **P4** Golden scripts. Switch all three from in-script
  `swift build`/`dotnet publish`/repo-tar to `cp` from
  `$(brew --prefix testanyware)/share/testanyware/...`. Add the
  `TESTANYWARE_AGENT_BIN_OVERRIDE` / `TESTANYWARE_CLI_BIN_OVERRIDE`
  env-var escape hatches for contributors building from source. Replace
  the `swift build` toolchain preflight in each script with a
  `brew --prefix testanyware` reachability check. The `gh` dependency
  introduced by the per-artifact split is *not* needed in this design.
- **P5** README + `.gitignore` + (resolve open follow-up: how brew-
  installed users invoke golden creation, then document the chosen
  path). Move install instructions to brew; reframe the existing build
  section as "for contributors".
- **P6** Cut a `v0.0.1-rc1` pre-release tag and run the full pipeline
  end-to-end: `release-build.sh` → `release-publish.sh` → fresh shell
  `brew install linkuistics/taps/testanyware && testanyware --version`
  → run `vm-create-golden-macos.sh` (resolved per P5 follow-up) →
  integration tests pass against the resulting golden. Promote to
  v0.1.0 on success.

## Risks & mitigations

| Risk | Mitigation |
|---|---|
| Agent binary shipped via brew has different csreq each release → TCC grant looks unstable | Already addressed by the existing flow: `grant_tcc_permissions` computes csreq from the actual installed binary at golden-build time, so any binary works. The csreq stays stable for a given binary; new releases simply produce new csreq blobs that flow through unchanged. |
| Brew-installed user has no working `vm-create-golden-*.sh` invocation path | Resolved by the P5 open follow-up — bundled scripts under `share/testanyware/scripts/` are reachable via either a wrapper binary, a CLI subcommand, or a documented `bash $(brew --prefix testanyware)/...` invocation. |
| Tap clone may be missing on a contributor's machine | `release-publish.sh` errors with a helpful message + path; matches Ravel-Lite's `RAVEL_TAP_DIR` pattern via `TESTANYWARE_TAP_DIR`. |
| Cross-version brew/agent mismatch (user's brew CLI talks to a v0.2.0 golden built from an older brew install) | Bundling CLI + agents under one tag eliminates this for fresh brew installs — the agent inside `share/...` is exactly the version pinned to the CLI in `bin/`. The remaining cross-version risk is a user upgrading the CLI via brew but keeping an old golden; document that goldens should be rebuilt on `brew upgrade testanyware`, and consider adding `testanyware --version` checks against `agent /health` version output long-term. |
| Apple-silicon-only formula breaks contributors on x86_64 macOS | Already broken — tart needs Apple Silicon, and the agent uses Apple-only frameworks. Formula's `on_arm do` block makes the constraint explicit instead of implicit. |
| Contributors who build from source need a way to override the brew-shipped agent | `TESTANYWARE_AGENT_BIN_OVERRIDE` / `TESTANYWARE_CLI_BIN_OVERRIDE` env vars in the golden scripts let a `swift build`-of-the-day binary be substituted without touching brew. Falls back to brew prefix when unset. |

## Out of scope (for the sibling feature task)

- Notarisation / Developer ID signing.
- Universal2 binaries.
- x86_64 host support.
- Vision pipeline brew formula.
- GitHub Actions CI for releases.
- Per-artifact tap formulas (`testanyware-agent-macos` etc.) for
  installing an agent on bare-metal macOS without the rest of the
  bundle. Possible later if a non-VM use case emerges; not needed for
  the VM-driven flow.

## Implementation hand-off

After P0 (this document), the implementer should pick up the sibling
backlog task `local-homebrew-binary-distribution-tap-no-client-builds`
and run the phases above. The first concrete code-writing step is
`scripts/release-build.sh`: model the structure byte-for-byte on
Ravel-Lite's equivalent, but replace the per-target build loop with a
single staging-tree assembly:

```
target/dist/staging/testanyware-v<ver>-aarch64-apple-darwin/
├── bin/testanyware                                        ← swift build cli
└── share/testanyware/
    ├── agents/
    │   ├── macos/testanyware-agent                        ← swift build agents/macos
    │   ├── windows/testanyware-agent.exe                  ← dotnet publish agents/windows
    │   └── linux/testanyware_agent/                       ← cp -r agents/linux/testanyware_agent
    ├── scripts/                                           ← cp provisioner/scripts/*.sh
    └── helpers/                                           ← cp provisioner/helpers/*
```

Then `tar -cJf` the whole staging dir.

The pre-release smoke test in P6 is the gating quality bar — do not
promote to v0.1.0 until a fresh `brew install` produces a working CLI
*and* the bundled `vm-create-golden-macos.sh` (invoked per the P5
follow-up resolution) produces a Tahoe golden that passes the
integration suite.
