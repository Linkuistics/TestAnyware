# Core Architectural Decisions

Running log of decisions made during monorepo unification that diverge
from the original plan/design or are not otherwise derivable from code.

---

## 2026-04-19 — No shared Swift protocol between CLI and agent

**Decision:** `agents/macos/` is a fully self-contained Swift package with
its own `Sources/TestAnywareAgentProtocol/` tree. It does NOT path-depend
on `cli/` for protocol types. `cli/`'s `TestAnywareAgentProtocol` target
and `agents/macos/Sources/TestAnywareAgentProtocol/` are two independent
copies of the same types that happen to share a wire format.

**Why this diverges from design §5:** The design said "`agents/macos/` ...
path-depends on `../../cli/macos` to reuse `TestAnywareAgentProtocol`."
That was predicated on Swift-on-both-sides being a durable architectural
invariant. It isn't — the host CLI will migrate to Rust for cross-platform
support. The true invariant is the wire protocol (JSON-RPC 2.0 over HTTP,
port 8648), not Swift types.

**Also:** Swift Package Manager derives path-dep identity from the last
path component. With `cli/macos/` and `agents/macos/` both ending in
`macos`, a path-dep between them produces an identity collision that SPM
cannot disambiguate. Consolidating would have required either renaming a
directory (contradicting design §4) or extracting the protocol into a
third package (extra directory not in design §4). With the Rust migration
planned, neither was worth doing.

**How to apply:** When editing protocol types, update both copies until
the Rust migration lands. The divergence risk is low (protocol is small;
wire format is the contract) and will be eliminated structurally when the
CLI changes language.

**Small current divergence:** `agents/macos` has `AXWebArea → .webArea`
in `RoleMapper.swift`; `cli/` has the same entry (merged during this
session). Both copies currently agree. If they drift, the agent's copy
is authoritative (it runs accessibility queries inside the guest VM;
the host CLI only consumes role strings via JSON).

---

## 2026-04-19 — No per-platform subdirectories under `cli/`

**Decision:** The Swift host CLI sits at `cli/` directly (Package.swift,
Sources/, Tests/), not at `cli/macos/`. `cli/linux/` placeholder is not
created.

**Why this diverges from design §4:** The design anticipated `cli/macos/`
+ `cli/linux/` siblings under `cli/` to signal multi-platform scope. That
framing assumed per-platform host implementations. The actual direction
is a single cross-platform Rust CLI replacing the Swift one — no
per-platform split needed. The Swift CLI sits at `cli/` transitionally;
the Rust CLI will replace it in place.

**How to apply:** Refer to the host CLI at `cli/` (no platform suffix).
Guest agents keep per-platform subdirectories (`agents/macos/`,
`agents/linux/`, `agents/windows/`) because those legitimately are
different implementations for different guest OSes. Update design §4
and §5 in Milestone 3 docs pass.

**Follow-up:** When writing docs in Milestone 3, rewrite `cli/macos/` →
`cli/` throughout. The flattening affects `swift build --package-path`
invocations, CI, install scripts, and LLM_INSTRUCTIONS.md examples.

---

## 2026-04-19 — Rust migration planned for CLI

**Fact:** The `testanyware` host CLI will be rewritten in Rust to support
Linux hosts (currently macOS-only). Timing is not committed; this note
exists so future sessions don't re-evaluate Swift-specific design
choices without knowing the language change is coming.

**How to apply:** When choosing abstractions or dependencies in the
current Swift CLI, prefer approaches that translate cleanly to Rust
(e.g., argument parsing, subprocess management, HTTP clients are all
direct ports). Avoid Swift-only runtime features (e.g., heavy
dependence on ObjC bridging, KVO, or macro-heavy DSLs) in new code.
The agent stays Swift (macOS guest, needs AppKit / Accessibility APIs).

---

## 2026-04-19 — Vision pytest requires `--import-mode=importlib`

**Fact:** `cd vision && uv run pytest` fails collection under default
import mode because two stage test files share the module path
`tests.test_stage` (one in `stages/drawing-primitives/tests/`, one in
`stages/icon-classification/tests/`). Both stages follow the repo's
convention of naming the stage entry test `test_stage.py`, and pytest's
default `prepend`/`append` import mode uses legacy namespace rules that
collide on duplicate short-paths.

**Workaround:** Run pytest with `--import-mode=importlib`. Expected to
be added as a default via `[tool.pytest.ini_options] addopts =
"--import-mode=importlib ..."` in `vision/pyproject.toml` during
Milestone 3 (docs + config polish).

**How to apply:** When running the vision test suite from any script,
CI, or command-line invocation, pass `--import-mode=importlib` (or rely
on the addopts once set).

---

## 2026-04-19 — Historical `guivision` mentions preserved in plan docs

**Fact:** `vision/docs/superpowers/plans/2026-04-03-*-window-detection.md`
contain historical `GUIVision*` and `guivision_*` references. These are
dated plan documents (2026-04-03) describing past implementation work
when the project was still named GUIVisionPipeline. Left as-is per the
"don't rewrite history" rule.

**How to apply:** Grep sweeps for stale references should exclude
these files (they're historical by design). Same applies to dated
session logs elsewhere in LLM_STATE.

---

## 2026-04-19 — `provisioner/helpers/` (not `autounattend/`)

**Decision:** The provisioner auxiliary directory is `provisioner/helpers/`
(matching the source archive's `scripts/helpers/`). NOT
`provisioner/autounattend/` as the plan Task 2d.1 named it.

**Why:** `autounattend/` implies Windows-only install-time assets, but
the directory actually contains a mix:
- Windows install assets: `autounattend.xml`, `SetupComplete.cmd`,
  `desktop-setup.ps1`, `set-wallpaper.ps1`
- macOS runtime assets: `com.linkuistics.testanyware.agent.plist`
  (LaunchAgent), `set-wallpaper.swift`

The scripts (both macOS and Windows builders) reference `helpers/`, not
`autounattend/`. Changing the scripts would be mislabeling the wrong way.
Keeping the original name matches what the scripts expect and matches
the archive.

**How to apply:** Refer to `provisioner/helpers/` wherever plan/design
said `provisioner/autounattend/`. Update design §4 accordingly during
docs refresh.

---

## 2026-04-20 — Server self-respawn ignores PATH lookup

**Issue surfaced by:** Task 6.1 (macOS smoke). `testanyware screenshot`
failed with `Server failed to start: The file "testanyware" doesn't
exist.` immediately after `vm-start.sh` succeeded.

**Root cause:** `cli/Sources/TestAnywareDriver/Server/ServerClient.swift`
lines 151-156 use `CommandLine.arguments[0]` to spawn the helper
`_server` subprocess. When the binary is invoked by bare name from
`$PATH` (e.g. `/usr/local/bin/testanyware` resolved via shell PATH
search), `argv[0]` is just `"testanyware"` — not absolute. The fallback
joins it with `FileManager.default.currentDirectoryPath`, producing
`<cwd>/testanyware`, which doesn't exist. `Process.run()` then throws
the localized "file doesn't exist" error and the CLI surfaces it via
`ServerClientError.serverStartFailed`.

**Repro:** From any CWD that doesn't contain a `testanyware` file,
run `testanyware screenshot --vm <id> -o /tmp/x.png` against a started
VM. Fails before contacting the VM.

**Workaround pending fix:** Invoke via the absolute symlink target,
e.g. `/usr/local/bin/testanyware screenshot ...` — that makes
`argv[0]` absolute and the spawn path resolves correctly.

**How to apply:** Fix-forward should resolve `argv[0]` against `$PATH`
when it lacks a slash (or use `/proc/self/exe` equivalent — on macOS,
`_NSGetExecutablePath`). Until then, the smoke recipes in plan §6
need `/usr/local/bin/testanyware` rather than bare `testanyware`,
or the user's shell must invoke from a directory containing the
binary.

**Resolved by:** commit `fix(cli): resolve executable path via Bundle.main.executablePath` (2026-04-20) — added
`cli/Sources/TestAnywareDriver/ExecutablePath.swift` exposing
`currentExecutablePath()` (backed by `Bundle.main.executablePath`,
which calls `_NSGetExecutablePath`); both call sites in
`ServerClient.swift` and `TestAnywareServer.swift` now use it.
Site 2's single-level symlink resolution (Cellar layout discovery)
is preserved. Smoke verified: `testanyware screenshot` from `/tmp`
against a fresh VM produced a 30,416-byte PNG.

---

## 2026-04-20 — QEMU sockets staged under `$TMPDIR`, not the clone tree

**Issue surfaced by:** Backlog item 12 / Task 6.3 (Windows integration
smoke). `swtpm` failed to create its control socket at
`<HOME>/.local/share/testanyware/clones/<id>/<id>-tpm/swtpm-sock` with
`Path for UnioIO socket is too long` — for a typical `$HOME` and an
integration-test id `testanyware-test-<hex8>`, the absolute path is
~108–110 bytes, exceeding macOS's 104-byte `struct sockaddr_un.sun_path`
limit. With swtpm refusing to start, `QEMURunner.start` threw
`commandFailed` and Windows VMs could not be brought up at all.

**Decision:** Split the per-VM artefacts across two locations:

| Artefact | Location | Why |
|---|---|---|
| qcow2 overlay | `$XDG_DATA_HOME/testanyware/clones/<id>/` | Persistent state; long path is fine for regular files. |
| EFI vars copy | `$XDG_DATA_HOME/testanyware/clones/<id>/` | Same. |
| TPM state dir | `$XDG_DATA_HOME/testanyware/clones/<id>/<id>-tpm/` | Same. |
| **swtpm control socket** | **`$TMPDIR/testanyware-<id>/swtpm-sock`** | AF_UNIX path, must fit `sun_path`. |
| **QEMU monitor socket** | **`$TMPDIR/testanyware-<id>/monitor.sock`** | Same. |

`$TMPDIR` on Apple platforms is per-user (e.g.
`/var/folders/.../T/`) which gives us isolation without depending on
`/tmp` semantics; we fall back to `/tmp` when the env var is unset.
The session-dir name convention `testanyware-<id>/` makes cleanup
deterministic from just the VM id — no sentinel file needed.

For the longest VM id we generate (`testanyware-test-deadbeef`) and a
typical `$TMPDIR`, the socket path is ~92–94 bytes — well under the
104-byte limit, with margin for `$TMPDIR` variance.

**Why both sockets, not just swtpm:** monitor.sock under `cloneDir/`
was within budget for typical IDs but not by much, and a runner that
keeps one socket near the limit is fragile against any reasonable
`$HOME` lengthening. Moving both preserves the property that
QEMURunner is robust against any reasonable user setup.

**Implications captured in the same fix:**

- `scanClonesDir` (the "is running" detector) now keys off
  `$TMPDIR/testanyware-<id>/monitor.sock` rather than
  `cloneDir/monitor.sock`, with the clone subdirectory name as the id.
- `QEMURunner.stop(pid:cloneDir:)` now derives the session dir from
  the clone dir basename and removes both directories.
- A shared `internal teardown(pid:cloneDir:sessionDir:)` helper backs
  both `stop()` and the start-failure recovery paths. Pre-fix, the
  start-failure recovery path was bare (`kill SIGTERM` + `removeItem`
  on cloneDir) and left orphan qemu processes that the implementer of
  Task 6.3 had to `kill -9` by hand. The shared helper does the full
  wait + SIGKILL escalate + swtpm pgrep + rmdir flow.

**Latent parser bug surfaced by the same smoke:** With the swtpm
wall removed, `QEMUMonitorClient.parseAgentPort` started returning
nil from real `info usernet` responses. Root cause: QEMU's monitor
sends CRLF line endings; Swift's `Character` is a grapheme cluster,
so `\r\n` is a single `Character` and `String.split(separator: "\n")`
matches nothing — the whole response collapses into one logical line
and the field-index parse fails. Fixed by switching to
`split(whereSeparator: { $0.isNewline })`. Could not have surfaced
earlier because the swtpm path-length error blocked us from ever
reaching this code path with a real response. Regression-guarded by
`parseAgentPortHandlesCRLFLineEndings`.

**Resolved by:** commit `fix(qemu): stage swtpm+monitor sockets under $TMPDIR (sun_path limit)`
(2026-04-20). Smoke verified: `vm-start.sh --platform windows`
produced a running VM, `${TMPDIR}/testanyware-<id>/` contained both
sockets, `vm list` showed it as running, `screenshot` produced a
non-zero PNG, `vm-stop.sh` cleared the session dir, and no qemu/swtpm
orphans were left behind.


