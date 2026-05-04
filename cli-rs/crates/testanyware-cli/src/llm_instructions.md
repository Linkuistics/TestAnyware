# testanyware — LLM-oriented manual

`testanyware` drives a virtual machine over VNC (capture, input) and an
in-VM HTTP agent on port 8648 (accessibility tree, exec, file transfer).
It is *not* a general remote-shell tool, *not* a screen recorder for the
host, and *not* an OCR library — it is glue between LLM agents and a
running guest OS. If you need a host-side screenshot, use a host tool;
if you need to run a process on the host, do not pipe it through here.

## Mental model

The command tree is **noun-first** with a small set of **verb-first
shortcut aliases**. Both forms are stable.

- Noun groups: `vm`, `agent`, `input`, `screen`, `file`.
- Top-level utilities: `doctor`, `capabilities`, `schema`,
  `llm-instructions`.
- Verb-first aliases (kept for muscle memory, *not* independent
  commands): `screenshot` → `screen capture`, `record` → `screen record`,
  `screen-size` → `screen size`, `find-text` → `screen find-text`,
  `upload`/`download`/`exec` → `file upload`/`download`/`exec`.

Connection target resolves through this chain (highest priority first):

1. `--connect <path>` — explicit JSON spec.
2. `--vm <id>` — per-VM spec at `$XDG_STATE_HOME/testanyware/vms/<id>.json`.
3. `--vnc` / `--agent` / `--platform` — direct flags.
4. `TESTANYWARE_VM_ID` env — resolves like `--vm`.
5. `TESTANYWARE_VNC` / `TESTANYWARE_AGENT` / `TESTANYWARE_PLATFORM` env.
6. Error.

The typical pattern is `vmid=$(testanyware vm start ...); export
TESTANYWARE_VM_ID="$vmid"`, then every subsequent command auto-resolves
to that VM with no flags.

## Workflows

### 1. Boot a VM, screenshot, shut down

```
vmid=$(testanyware vm start --platform macos --display 1920x1080)
export TESTANYWARE_VM_ID="$vmid"
testanyware screen capture --out screen.png
testanyware vm stop "$vmid"
```

### 2. Drive a UI through the agent

```
testanyware agent snapshot --window "Settings" --json
testanyware agent press --role button --label "Save"
testanyware screen find-text "Saved" --timeout 10 --json
```

`agent snapshot` is the primary discovery tool — read the tree, decide
what to act on, then call `agent press` / `agent set-value` / `agent
focus`. Use `screen find-text` to confirm the post-action state visually.

### 3. Record a session and exec inside the guest

```
testanyware screen record --out session.mp4 --fps 30 --duration 30 &
testanyware file exec "uname -a"
testanyware file upload local.txt /tmp/remote.txt
testanyware file download /tmp/remote.txt local.txt
wait
```

## Common mistakes

- **Don't grep `vm list` text output.** Use `--json | jq` — text output
  is not a parsing target.
- **Don't pipe `find-text` output through `awk`.** Use `--json` and
  parse the detection array directly.
- **Don't poll `agent windows` in a tight loop.** Use `agent wait`
  (data-producing, blocks until the element is visible).
- **Don't reuse a screenshot's coordinates after a window move.** The
  agent returns absolute screen coordinates; if the window moves, your
  cached coordinates are stale. Re-snapshot or use `--window <name>`
  filters that resolve at click time.
- **Don't mix `--vm` and `--vnc`/`--agent` on the same command.** The
  resolution chain takes the first match; explicit endpoints will be
  used silently if `--vm` resolves to a missing spec.

## Authentication and state

The in-VM agent must hold OS-level accessibility permission (TCC on
macOS, AT-SPI2 on Linux, UIA on Windows). Golden images grant this at
build time. Hot-swapping the agent binary breaks the TCC grant on
macOS — re-build the golden, do not replace the binary in a running VM.

VNC has no auth model beyond the password in the spec file. Specs are
written with mode `0600`. Treat them as secrets.

## JSON, exit codes, idempotency

- Every data-producing command supports `--json` (or `--output json`).
  Streaming commands use `--output jsonl`.
- All JSON envelopes carry `schema_version` and `ok`.
- Error envelopes (when `--json` is set and the command fails) carry
  `code` from a stable catalogue. Run `testanyware capabilities` for the
  full list, and `testanyware schema <command>` for per-command schema.
- Exit codes: 0 success; 1 generic; 2 usage; 3 not-found; 4 auth; 5
  conflict; 7 timeout. Sub-process exit (`file exec`) is propagated.
- Mutating commands accept `--dry-run`. The JSON envelope sets
  `dry_run: true` and no side effect runs.

## Where to read more

- `testanyware capabilities` — machine-readable surface (subcommands,
  aliases, error codes, features).
- `testanyware schema <command>` — JSON Schema for a command's output.
- `testanyware <command> --help` — full §7 help: USAGE, OUTPUT, EXIT
  CODES, EXAMPLES, SEE ALSO.
- `docs/architecture/cli-design-contract.md` — the underlying contract.
