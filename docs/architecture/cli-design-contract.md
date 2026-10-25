# CLI Design Contract

**Status:** Authoritative for the `testanyware` host CLI (`cli-rs/`). Every
command must satisfy the shape described below. The `cli-contract.rs`
integration test is the gate. The original macOS-only Swift CLI was retired
2026-06-03. §10 records the audit that drove the port.

**Authority:** This document is referenced by name from the acceptance
criteria of every command's work. Any deviation must be raised as a contract
amendment first.

**Source guidance:** Distilled from
`/Users/antony/Development/Ravel-Lite/defaults/fixed-memory/cli-tool-design.md`
("CLI Design Guidelines for LLM Agents") plus the original TestAnyware
surface (the now-retired Swift CLI) and `docs/reference/error-codes.md`.

---

## 1. Verb/noun convention

**Decision: a subcommand puts its noun first, and a chosen set of top-level
shortcuts put the verb first.**

Most subcommands put the noun first (`<noun> <verb>`):

| Group | Subcommands |
|---|---|
| `vm` | `start`, `stop`, `list`, `delete`, `slots` |
| `agent` | `health`, `snapshot`, `inspect`, `press`, `set-value`, `focus`, `show-menu`, `windows`, `window-focus`, `window-resize`, `window-move`, `window-close`, `window-minimize`, `wait` |
| `input` | `key`, `key-down`, `key-up`, `type`, `click`, `mouse-down`, `mouse-up`, `move`, `scroll`, `drag` |
| `screen` | `capture`, `record`, `size`, `find-text` |
| `file` | `upload`, `download` |

**The top-level shortcuts that put the verb first stay**, for muscle memory and
discoverability. Each one is an alias of the canonical form, which puts the noun
first. None of them is a command of its own:

| Verb-first alias | Canonical |
|---|---|
| `screenshot` | `screen capture` |
| `record` | `screen record` |
| `screen-size` | `screen size` |
| `find-text` | `screen find-text` |
| `upload` | `file upload` |
| `download` | `file download` |
| `exec` | `file exec`. It lives under `file` because it is an action in the VM. The alias stays because many scripts use it |

**Aliases policy.** Tests cover both forms. The `--help` of an alias must say
"Alias of `<canonical>`". Help and schema come from the canonical command, so an
alias does not document the surface again. The removal of an alias is a
breaking change.

**Common synonym aliases.** They cost little, and each one is required:

| Canonical | Required aliases |
|---|---|
| `list` | `ls` |
| `delete` | `rm`, `remove` |
| `inspect` | `show` |

**Internal commands** are not part of the public contract. Their names start
with `_`, such as `_server`. They MUST set `hide = true` in clap, so that they
do not appear in the output of `--help`. No section of this contract binds
them.

---

## 2. Flag vocabulary

**One name for each concept, and no synonyms across commands.** The Rust port
uses these names and no others:

| Concept | Long flag | Short | Notes |
|---|---|---|---|
| Structured output | `--json` | — | A boolean flag. It is the same as `--output json`. |
| Output format selector | `--output <fmt>` | `-o` | Values: `text` (default), `json` and `jsonl`. `-o` is reserved for this flag. A command MUST NOT use `-o` for the path of an output file (see below). |
| Output **file** path | `--out <path>` | — | A command that writes a file uses `--out`, not `--output`. This keeps it apart from the format selector. |
| Suppress non-essential stderr | `--quiet` | `-q` | It acts on stderr only, and it never silences an error. |
| Diagnostic detail | `--verbose` | `-v` | Repeat it (`-vv`) for more detail. It writes to stderr only. |
| Preview without side effects | `--dry-run` | — | Each mutating command requires it (see §9). |
| Non-interactive consent | `--yes` | `-y` | It skips each prompt that asks for consent. |
| Override safety check | `--force` | — | It differs from `--yes`. A command requires it when a check refuses the action. |
| Narrow list output | `--filter <expr>` | — | Pairs of field=value, separated by commas. The help of each command documents its grammar. |
| Field-specific filter | `--<field> <value>` | — | For example `--platform macos` on `vm list`. |
| Page size | `--limit <n>` | — | The default is 100 on list commands. |
| Unbounded results | `--all` | — | It cannot go with `--limit`. |
| Custom format template | `--format <tmpl>` | — | Optional, and only where a template has a meaning. It MUST NOT collide with `--output <fmt>`. |
| Colour | `--color <when>` | — | Values: `auto` (default), `always` and `never`. |
| Keep a failed start's VM | `--keep-on-failure` | — | `vm start` only. It leaves a VM that still runs, with its sidecars, for inspection, and does not remove it. See the §4.2 amendment of 2026-10-17. |
| Wait for a free slot | `--wait[=<seconds>]` | — | `vm start` only. When the macOS guest limit is full, the start waits in the admission queue until the app admits it or the wait runs out. The app serves the waiters in order. If the framework refuses a start, the app puts it back at the head of the queue. It tries the start again after a pause of 3 s. Without the flag, the start fails at once. Bare `--wait` is 600 s. The value is in seconds, as in the other timeout flags. See the §4.2 amendments of 2026-10-19 (`--wait`) and 2026-10-25 (the admission queue). |

**Reserved short flags.** They must stay the same across the whole binary:
`-q`, `-v`, `-y`, `-o` (output format only), `-h` (help) and `-V` (version).

**Flags that MUST NOT appear.** They are deprecated synonyms of the flags above:
`--confirm`, `--noprompt`, `--silent`, `--debug`, `--no-color`, `--pretty` and
`--raw`. A new command MUST reuse the table above, and not invent a flag.

**Connection flags.** They are orthogonal to the vocabulary, and §6 covers
identifiers. They come unchanged from the Swift CLI:

`--connect <path>` | `--vm <id>` | `--vnc <host:port>` |
`--agent <host:port>` | `--platform <macos|linux|windows>`

These flags stay because they identify the target, and they do not configure
behaviour. Their resolution chain is part of the contract: `--connect` →
`--vm` → `--vnc`/`--agent`/`--platform` → `TESTANYWARE_VM_ID` → the other
environment variables. `docs/reference/connection-spec.md` describes it.

---

## 3. JSON schema policy

### 3.1 Coverage

Every **data-producing** command MUST support `--json`, which is the same as
`--output json`. A data-producing command returns structured information that
the caller parses. A confirmation of a mutation counts too.

The current list of the Rust port:

| Command | Mode | Schema id |
|---|---|---|
| `screen capture` | data (file path + bytes) | `screen-capture` |
| `screen record` | data (file path + frames + duration) | `screen-record` |
| `screen size` | data | `screen-size` |
| `screen find-text` | data (array of detections) | `screen-find-text` |
| `vm start` | data | `vm-start` |
| `vm stop` | data (mutation receipt) | `vm-stop` |
| `vm list` | data | `vm-list` |
| `vm slots` | data | `vm-slots` |
| `vm delete` | data (mutation receipt) | `vm-delete` |
| `agent health` | data | `agent-health` |
| `agent snapshot` | data | `agent-snapshot` |
| `agent inspect` | data | `agent-inspect` |
| `agent windows` | data | `agent-windows` |
| `agent press` / `focus` / `set-value` / `show-menu` / `wait` | data (action receipt) | `agent-action` |
| `agent window-*` | data (action receipt) | `agent-window-action` |
| `input *` | data (action receipt) | `input-action` |
| `file exec` | data | `file-exec` |
| `file upload` | data (mutation receipt) | `file-upload` |
| `file download` | data (mutation receipt) | `file-download` |
| `doctor` | data | `doctor` |
| `capabilities` | data | `capabilities` |

**`vm start` reports its TPM decision.** A start on QEMU (Windows, and Linux on
a qcow2 golden) runs under one TPM decision (ADR-0017), and it says what that
decision was. In `--json` the envelope carries an optional `tpm` object:
`{"attached": true, "ppi_off": <bool>}` or `{"attached": false, "reason":
"guest_does_not_need_one" | "host_cannot_supply_one"}`. In text mode, stdout
stays the id alone. A line on stderr says the same in words (`TPM attached`,
`TPM omitted: ...`).

`--dry-run` reports the decision in its plan, in both modes. A text plan prints
it as a second `dry-run:` line. So a refusal (`SWTPM_MISSING`, §4.2, raised
before anything is cloned) and the answer show without a start. A start on tart
(macOS, and Linux on a tart golden) has no TPM decision, and it omits `tpm`. The
field is optional in `vm-start.json`, so it is an additive change under §3.2.

**`doctor` reports the same decision.** The `doctor` report carries a sixth
check group, `checks.windows_tpm`. It asks the Windows question of the TPM
decision (ADR-0017) without a start. It is the `tpm` object above with a
`status`: `pass` (attached), `warn` or `fail`. `warn` means that the decision
omits the TPM because the host cannot supply one, or that `doctor` could not
observe the host. `fail` means that the decision refuses with `SWTPM_MISSING`
(§4.2).

A refusal also carries an `error` object (`code`, `message`, `remediation`).
Only `fail` flips the `ok` of `doctor` and its exit code to `1`, and a `warn`
never does. The group is required in `doctor.json`, which is an additive change
under §3.2.

<!-- tc: pending because the conversion has not reached sections 3.2 to 4.1 -->

### 3.2 Stability

Schemas are versioned by the binary's major version. **Adding** a field
or a non-required enum variant is permitted in any release. **Removing**
or **renaming** a field, or changing its type, requires a major version
bump and a documented migration path.

Every JSON object MUST include `"schema_version"` (string, semver of the
schema). Streaming output (`--output jsonl`) emits `schema_version` on
each line.

### 3.3 Schema directory

JSON schemas live at `docs/reference/cli-schemas/<schema-id>.json` in
[JSON Schema 2020-12](https://json-schema.org/draft/2020-12/release-notes)
format. The directory is canonical: a command's actual `--json` output
MUST validate against its declared schema in CI.

Stub schemas (objects with only `schema_version` and `$comment: "TODO"`)
are acceptable for commands that have not yet been ported, so the tree
of schema files stays parallel to the command tree from day one.

### 3.4 Errors in JSON mode

When `--json` (or `--output json|jsonl`) is set and a command fails, the
**only** thing on stdout is a single JSON error object. Stderr is empty
or carries diagnostic detail under `--verbose`, never user-facing prose.

```json
{
  "schema_version": "1.0",
  "ok": false,
  "code": "VM_NOT_FOUND",
  "message": "No spec found for VM id 'testanyware-deadbeef'",
  "remediation": "Start a VM with `testanyware vm start` or check the id with `testanyware vm list --ls --json`.",
  "details": { "vm_id": "testanyware-deadbeef", "spec_path": "/Users/.../vms/testanyware-deadbeef.json" }
}
```

`code` is one of the stable strings catalogued in §4. `details` is a
free-form object; its keys are documented per error code where useful.

### 3.5 Truncation

List commands MUST signal truncation in the JSON envelope:

```json
{
  "schema_version": "1.0",
  "ok": true,
  "items": [...],
  "returned": 100,
  "total": 1438,
  "truncated": true
}
```

Human output prints a final line: `Showing 100 of 1,438. Use --limit N
or --all to see more.`

### 3.6 Streaming

For commands that emit a long stream (currently `screen find-text
--watch`, future `agent snapshot --watch`), use `--output jsonl`. One
JSON object per line, each with `schema_version`. Do **not** use a
giant top-level array — agents and downstream tools should be able to
process incrementally.

---

## 4. Error codes

**Codes are stable strings.** Renaming or removing one is a major
version bump.

### 4.1 Authentication, connection, target resolution

| Code | When |
|---|---|
| `AUTH_REQUIRED` | The agent rejected the request because TCC / accessibility permission is not granted. |
| `CONNECTION_REFUSED` | TCP connect to VNC or agent failed; service likely not listening. |
| `CONNECTION_TIMEOUT` | TCP connect or RFB handshake exceeded the timeout. |
| `CONNECTION_DROPPED` | Peer dropped during a request. |
| `INVALID_ENDPOINT` | `--vnc`/`--agent` value parsed empty or out-of-range. |
| `NO_CONNECTION_SPECIFIED` | None of `--connect`, `--vm`, `--vnc`, `TESTANYWARE_VM_ID`, `TESTANYWARE_VNC` resolved. |
| `INVALID_PLATFORM` | `--platform` not in `{macos, linux, windows}`. |
| `SSH_CONNECT_FAILED` | SSH connect or auth to a provisioning target (the throwaway setup VM) failed. `details.detail` carries the cause. `vm create-golden` only. |

<!-- tc: on -->

### 4.2 VM lifecycle

| Code | When |
|---|---|
| `VM_NOT_FOUND` | `--vm <id>` resolved to a spec that does not exist. |
| `VM_BOOT_TIMEOUT` | `vm start`: the VM is alive, but a stage of the start never completed. The stage is the VNC endpoint, the guest IP or, in the usual case, the agent, which never became a Healthy agent. `details` names the `stage` (`vnc`, `guest-ip`, `agent`), the polled `endpoint`, the `last_error` (kind, OS errno, text), the `attempts` and the `elapsed_secs`. It also names the proven `cause` (`null` or `local_network_denied`, see the §4.2 amendment of 2026-10-19), and whether the VM was `kept`. |
| `VM_LIMIT_REACHED` | `vm start` or `vm create-golden` was refused because the platform's limit of concurrent guests is full: Apple's macOS guest limit, two for each Mac. It is a *try again later* condition, exit `6`. `details`: `platform`, `limit` (the limit that refused), `holders` (the VMs holding the slots), `tart_error` (the backend's own line, or an empty string when no `tart run` ran), `request_id`, `place`, `waited_secs` and `refused_by` (`app` or `framework`). A `--wait` start that ran out returns the same error. If the framework refuses a start while the app counts a free slot, or contention holds a try back, the remedy names two causes. One cause is a guest of another user or tool that the app cannot see. The other is a stuck count of the framework, which only a restart of the Mac clears. See the amendment of 2026-10-25. |
| `VM_STOP_FAILED` | `vm stop` could not stop the VM cleanly. The app's stop of a QEMU guest that an older release started signals no process. It fails when a socket still answers or a process still holds the clone's disk, and `details` names `monitor_socket`, `swtpm_socket`, `disk_holders` and `disk_check_error`. The app then removes nothing, and the remedy says to quit that QEMU by hand and run `vm stop` again. |
| `VM_BACKEND_UNSUPPORTED` | Neither tart nor QEMU can serve the requested platform on this host. |
| `VM_HOST_UNSUPPORTED` | `vm start` ran on a Windows host. That host cannot run a local QEMU VM, because the binary has no QEMU monitor and no process control for Windows. `details.detail` holds the text. Start VMs from a Linux or macOS host. |
| `GOLDEN_NOT_FOUND` | Requested golden image name doesn't exist. |
| `GOLDEN_IN_USE` | `vm delete` refused: running clones depend on the image. Use `--force`. |
| `GOLDEN_CREATE_FAILED` | `vm create-golden` could not produce the golden image (a provisioning boot, TCC/SIP step, or the final health gate failed). `details.detail` carries the cause. |
| `TART_FAILED` | tart subprocess returned non-zero, or `tart run` exited before the guest was ready. `details.tart_error` carries its stderr / the tail of its log. |
| `QEMU_FAILED` | QEMU subprocess failed (start, monitor, or QMP), or the QEMU process exited before the guest was ready. `details.qemu_error` carries the tail of its log. |
| `KVM_PERMISSION_DENIED` | `/dev/kvm` is missing or not readable+writable (Linux host). `details.path` carries `/dev/kvm`. |
| `SWTPM_MISSING` | swtpm is not installed. A Windows guest needs it when the host's QEMU can supply a TPM (TPM 2.0 socket). The TPM decision (ADR-0017) raises it before anything is cloned, also under `vm start --dry-run` and before `vm create-golden --platform windows` boots. |
| `UEFI_NOT_FOUND` | UEFI firmware path missing for the requested QEMU configuration. |
| `SPAWN_FAILED` | `posix_spawn` (or platform equivalent) returned an error. |

<!-- tc: pending because the conversion has not reached the amendments of section 4.2 from 2026-05-22 to 2026-10-17 -->

> **Amendment 2026-05-22** (`port-qemu-runner-and-vm-lifecycle-to-rust`):
> `KVM_PERMISSION_DENIED` and `SWTPM_MISSING` added to support the QEMU
> runner's host preflight. `KVM_PERMISSION_DENIED` maps to exit code `4`
> (permission family, §5); `SWTPM_MISSING` maps to exit code `1` (generic
> — a missing optional dependency, recoverable by installing it).

> **Amendment 2026-06-03** (`110-vm-create-golden-macos`, ADR-0007):
> `SSH_CONNECT_FAILED` (§4.1) and `GOLDEN_CREATE_FAILED` (§4.2) added for the
> `vm create-golden` golden-provisioning command. Both are generic failures
> mapping to exit code `1` (§5) and carry their cause in `details.detail`.

> **Amendment 2026-10-17** (`start-failure-tart-k2`, `start-failure-qemu-k3`,
> issue #3): `vm start` succeeds only when its guest has a **Healthy agent**,
> on every backend — tart (macOS, Linux-on-tart) and QEMU (Windows,
> Linux-on-qcow2) alike. It never writes an agent-less spec. When it cannot,
> it exits non-zero with the reason and removes its VM (clone, process and
> sidecars):
>
> - `VM_BOOT_TIMEOUT` (exit `7`) also covers the agent stage — the VM is
>   alive but its agent never answered `/health`. The error names the stage
>   and the last probe's error, including its OS errno (refused vs. no route
>   to host vs. timeout). A QEMU guest gets 120 probes, 5 s apart (ten to
>   fourteen minutes), to bring its agent up because Windows boots slowly; a
>   tart guest gets 60 probes, 2 s apart.
> - `TART_FAILED` / `QEMU_FAILED` is returned as soon as the backend process
>   (`tart run` / QEMU) exits, with its log tail in `details.tart_error` /
>   `details.qemu_error`, rather than after the boot window.
> - `VM_LIMIT_REACHED` (new; exit `6`, the previously reserved bucket) is
>   returned when tart reports the macOS guest limit — Apple's, so tart only.
>   Drivers can wait on exit `6` without parsing text.
> - `--keep-on-failure` (new) leaves a failed start's VM running, with its
>   sidecars, when the backend process is still alive, so it can be inspected
>   and then reclaimed with `vm stop`. A VM whose backend has exited is always
>   removed. The kept VM's spec names the agent endpoint if it is known (the
>   guest IP for tart, the loopback forward for QEMU), so commands against it
>   report the real connection error.

<!-- tc: on -->

> **Amendment 2026-10-19** (`reliable-macos-start-k4`, issue #3, ADR-0019):
> `VM_BOOT_TIMEOUT` at the agent stage gains `details.cause`. A tart guest is
> on a host-only subnet. macOS Local Network privacy can refuse this process a
> route to it while the agent is healthy. The refusal is errno `65`
> (`host_unreachable`), the same errno as a guest that has not answered ARP.
>
> - After five `host_unreachable` probes in a row, `vm start` asks the platform
>   `/usr/bin/nc` to connect to the same endpoint. The platform tool is exempt
>   from the privacy check.
> - If `nc` connects, the refusal is about this process. The start ends at once
>   with `VM_BOOT_TIMEOUT` (exit `7`), `details.cause` is
>   `local_network_denied`, and `remediation` says to run testanyware from an
>   app that has Local Network access. On a macOS host, `remediation` names the
>   TestAnyware app instead (amendment 2026-10-20 below and §4.9).
> - If `nc` does not connect, the wait carries on unchanged and asks again
>   after each further five. `details.cause` stays `null`.
> - A QEMU guest is on loopback and never asks.
>
> `vm start` does not route any agent call through `nc`.

> **Amendment 2026-10-19** (`start-wait-for-slot-k6`, issue #3): `vm start`
> gains `--wait[=<seconds>]` to wait out a full macOS guest limit.
>
> - Without `--wait` nothing changes: a refused start fails at once with
>   `VM_LIMIT_REACHED` (exit `6`).
> - With `--wait`, the start waits for a slot until it succeeds or the wait runs
>   out. Bare `--wait` waits 600 s, so an unattended driver cannot hang for ever.
>   `--wait=<seconds>` sets the bound. A wait that runs out fails with the same
>   `VM_LIMIT_REACHED`, exit `6`, and details that say that it waited.
> - Virtualization.framework's refusal is the only authoritative signal about a
>   slot. So each retry is a real `tart run` on the one clone that the first
>   attempt made. A wait leaves one clone, never one per retry, and removes it
>   when it fails. On a macOS host, the app decides when a retry runs
>   (amendment of 2026-10-25 below).
> - While waiting, stderr names the place of the start and the VMs holding the
>   slots. It names them when the start joins, when they change, and every 30 s
>   otherwise. The `--json` envelope is still the final result only.
> - SIGINT or SIGTERM during a waiting start removes the clone and ends the
>   process as the signal would have (130 / 143).
> - On a macOS host the waiters wait in the admission queue of the TestAnyware
>   app, which serves them in order. The queue blocks only the starts that the
>   limit blocks. A refusal by the framework sends the start back to the head
>   of the queue, with a pause of 3 s. The help text no longer says that the
>   waiters are not served in order (amendment of 2026-10-25 below).
> - `--wait` changes nothing else. Any failure other than the limit, such as
>   `TART_FAILED` or `VM_BOOT_TIMEOUT`, ends the start at once. The wait is
>   spent only on refusals, and boot time after the slot is won does not count
>   against it.

<!-- tc: pending because the conversion has not reached the amendment of 2026-10-20 in section 4.2 -->

> **Amendment 2026-10-20** (`app-vm-start-k27`, issue #8): on a macOS host,
> `vm start` runs in the TestAnyware app, and the Host CLI sends it the options.
> The app holds the Local Network grant, so an agent under a harness that macOS
> refuses can start a guest.
>
> - The output does not change. The Host CLI prints the progress lines that the
>   app sends to stderr, in both output modes, and then the same stdout, the same
>   `--json` fields and the same exit codes.
> - A `VM_BOOT_TIMEOUT` with `details.cause` `local_network_denied` has a remedy
>   that names the app: allow TestAnyware in System Settings, Privacy & Security,
>   Local Network. The direct path of a Linux or Windows host keeps its old text.
> - SIGINT or SIGTERM of a waiting `vm start` ends the Host CLI as the signal
>   would. The app sees the closed connection, cancels the start and removes the
>   VM, its keeper and its sidecars. The removal ends a moment after the Host CLI
>   has gone, not before. This holds for every start, not for `--wait` alone.
> - A cancelled start removes the VM even under `--keep-on-failure`: nobody is
>   left to reclaim a kept VM.

<!-- tc: on -->

> **Amendment 2026-10-25** (`admission-queue-k2`, issue #7, ADR-0022): on a
> macOS host, the TestAnyware app admits macOS guests through one ordered
> queue. `docs/specs/admission-queue.md` describes it.
>
> - `vm start --wait` of a macOS guest waits in the queue, and the app serves
>   the waiters in order. A start without `--wait` fails at once with
>   `VM_LIMIT_REACHED` when the count is full or the queue holds a waiter. So
>   it never passes a waiter.
> - Linux and Windows guests take no slot, and they never wait.
> - When the framework refuses an admitted start, the app puts the start back at
>   the place of its join. The start goes ahead of each waiter that joined after
>   it. The app then admits nothing for 3 s. After the pause, the app tries the
>   head alone, and only while no admitted start is unproven. So a later start
>   never takes a freed slot before an earlier one. A guest that the app cannot
>   see costs one retry every 3 s. A start without `--wait` never waits for the
>   pause. It fails at once while contention holds back an admission.
> - If the framework refuses a start while the app counts a free slot, the
>   remedy names two causes. A guest of another user or tool holds a slot that
>   the app cannot see, or the count of the framework is stuck. Only a restart
>   of the Mac clears a stuck count.
> - One user for each Mac is the supported deployment. The queue counts the tart
>   VMs of that user only.
> - `VM_LIMIT_REACHED` keeps exit `6` and the type of each member. `limit` is the
>   limit that refused: `macos_slots` of `config.rhai` when the app's share is
>   full, and 2 otherwise. `tart_error` is an empty string when the start ran no
>   `tart run`.
> - The details gain `request_id`, `place`, `waited_secs` and `refused_by`
>   (`app` or `framework`). Without an app, `request_id` and `place` are `null`.
> - When the app runs, `vm create-golden` of a macOS golden waits in the same
>   queue, with no bound. When the framework refuses one of its boots for 30 s,
>   it fails with `VM_LIMIT_REACHED`, with the app or without it.
> - The new read-only command `vm slots` shows the slot count, each holder, the
>   queue and the last refusal. Its schema id is `vm-slots`.
> - `config.rhai` gains `macos_slots`: the number of the two slots that the app
>   may hold, 0, 1 or 2.
> - No command, flag, environment variable, exit code or `--json` field changes
>   its meaning, and the catalogue gains no code.

<!-- tc: pending because the conversion has not reached sections 4.3 to 4.7 -->

### 4.3 VNC capture

| Code | When |
|---|---|
| `VNC_NOT_CONFIGURED` | Capture used before `connect()` succeeded. |
| `VNC_FRAMEBUFFER_NOT_READY` | Screenshot requested before a full framebuffer update arrived. |
| `VNC_CAPTURE_FAILED` | RFB-side capture failed (server-rejected encoding, etc.). |
| `VNC_ENCODING_FAILED` | Output encoding (PNG/H.264/HEVC) failed. |
| `VNC_PIXEL_MISMATCH` | Received byte count doesn't match advertised framebuffer size. |
| `VNC_DIMENSIONS_ZERO` | Server advertised a zero-dimension framebuffer. |

### 4.4 Recording

| Code | When |
|---|---|
| `RECORD_ALREADY_ACTIVE` | `record` invoked while a recording is already running on this VM. |
| `RECORD_NOT_ACTIVE` | Stop / append called outside a recording. |
| `RECORD_BUFFER_UNAVAILABLE` | AVAssetWriter pixel-buffer pool not ready. |
| `RECORD_BUFFER_CREATE_FAILED` | `CVPixelBufferCreate` (or libav equivalent) returned non-success. |

### 4.5 Agent

The three agent implementations historically use different error
strings (per `docs/reference/error-codes.md`). The contract closes this:
**agents must emit `error` strings drawn from the table below**, and
the host CLI maps them 1:1 to a `code`.

| Wire `error` (agent → host) | CLI `code` | Meaning |
|---|---|---|
| `not_found` | `ELEMENT_NOT_FOUND` | Element query matched zero elements. |
| `ambiguous` | `ELEMENT_AMBIGUOUS` | Multiple matches; supply `--index N` or narrow filter. |
| `window_not_found` | `WINDOW_NOT_FOUND` | No window matched the `--window` filter. |
| `action_unsupported` | `ACTION_UNSUPPORTED` | Element matched but does not support the requested action. |
| `accessibility_unavailable` | `AUTH_REQUIRED` | OS-level accessibility disabled or not yet granted. |
| `exec_failed` | `EXEC_FAILED` | Process failed to spawn (exit codes are returned in the receipt, not via this error). |
| `upload_failed` | `UPLOAD_FAILED` | File I/O error on the VM during upload. |
| `download_failed` | `DOWNLOAD_FAILED` | File I/O error on the VM during download. |
| `invalid_json` | `AGENT_INVALID_JSON` | Agent could not parse the request body as JSON. Indicates a CLI bug — agents do not generate this for valid client traffic. |

The Linux and Windows agent ports MUST be brought into line with the
table above as part of the per-platform agent backlog work; the host
CLI MUST NOT carry a translation map. Today's wrapper logic in
`AgentTCPClient` (which carries the wire `error` string verbatim into
the displayed message) is replaced by a strict mapping; unknown wire
strings surface as `code: AGENT_ERROR_UNKNOWN` with the wire string in
`details.wire_error`.

### 4.6 Generic / fall-through

| Code | When |
|---|---|
| `USAGE_ERROR` | clap-level: bad flag, missing arg, invalid combination. |
| `IO_ERROR` | Local filesystem error (permission denied, disk full, etc.). `details.path` carries the offending path. |
| `OCR_UNAVAILABLE` | EasyOCR daemon cannot be recovered. Override with `TESTANYWARE_OCR_FALLBACK=1`. |
| `OCR_CHILD_CRASHED` | EasyOCR daemon subprocess crashed. |
| `OCR_TIMEOUT` | EasyOCR daemon did not respond within the timeout. |
| `UNKNOWN_KEY` | Key name not in the supported set. |
| `UNKNOWN_BUTTON` | Mouse button name not in `{left, right, middle, center}`. |
| `INTERNAL` | The CLI itself failed in a way that should not happen. Should always be a bug; `details.trace_id` for log correlation. |

### 4.7 Discoverability errors

| Code | When |
|---|---|
| `SCHEMA_NOT_FOUND` | `testanyware schema <command>` invoked with an unknown command, or with a command that has no declared schema. Exit code `3` (per §5). |
| `TEXT_NOT_FOUND` | `screen find-text <query> --require-match` completed (including any `--timeout` poll) without the query appearing on screen. Exit code `3` (per §5). Only emitted under `--require-match`; the default is exit `0` with an empty result (see §10.1). |

> **Amendment 2026-05-30** (`030-screen-find-text`): `TEXT_NOT_FOUND` added
> to catalogue the `--require-match` failure mode §10.1 already referenced.
> It joins the `*_NOT_FOUND` family at exit code `3` (§5).

<!-- tc: on -->

### 4.8 Catalogue exposure

`testanyware capabilities --json` (see §8) MUST emit the full code
catalogue in `error_codes`. Adding a new code in a release means the
catalogue grows by one entry. Agents can poll for additions.

### 4.9 The TestAnyware app and configuration

> **Amendment 2026-10-21** (`llm-instructions-and-docs-k35`, issue #8, ADR-0019,
> ADR-0021): on a macOS host, every guest connection goes through the
> TestAnyware app. One optional file, `config.rhai`, configures the app and the
> Host CLI. The catalogue gains four codes. Each error carries a `remediation`.

| Code | Exit | When |
|---|---|---|
| `APP_NOT_INSTALLED` | `1` | Nothing answers on the default socket, and `/Applications` and `~/Applications` hold no `TestAnyware.app`. `details.searched` lists both paths. The remedy is the one-time setup. A socket that `TESTANYWARE_APP_SOCKET` names never gives this error. |
| `APP_UNREACHABLE` | `1` | The Host CLI cannot use the socket of the app. `details.cause` is `not_running`, `unsafe_socket_directory`, `quitting`, `socket_path_too_long` or `socket_error`. |
| `APP_INCOMPATIBLE` | `1` | `details.cause` is `protocol` (the two protocol ranges share no version) or `directories` (the Host CLI and the app resolved other directories). |
| `CONFIG_INVALID` | `2` | `config.rhai` fails to evaluate, or it names an unknown key. `details` holds `file`, `line` and `message`. |

- A client never starts the app. When the app is not running, the command fails
  with `APP_NOT_INSTALLED` or `APP_UNREACHABLE`, and `remediation` names the fix.
- A Local Network denial keeps its codes. `CONNECTION_REFUSED` and
  `VM_BOOT_TIMEOUT` carry `details.cause` `local_network_denied`. On a macOS
  host their `remediation` says to allow TestAnyware in System Settings,
  Privacy & Security, Local Network.
- `vm create-golden` keeps its direct path. It needs a context with Local
  Network access, such as Terminal.
- Each existing code keeps its meaning and its exit code.

---

## 5. Exit code scheme

| Code | Meaning |
|---|---|
| `0` | Success. |
| `1` | Generic failure (no more specific bucket fits). The `APP_*` codes use it. |
| `2` | Usage error (`USAGE_ERROR` family — bad flags, missing args, invalid combinations). `CONFIG_INVALID` also uses it. |
| `3` | Not found (`*_NOT_FOUND` family: `VM_NOT_FOUND`, `WINDOW_NOT_FOUND`, `ELEMENT_NOT_FOUND`, `GOLDEN_NOT_FOUND`, `UEFI_NOT_FOUND`, `SCHEMA_NOT_FOUND`, `TEXT_NOT_FOUND`). |
| `4` | Auth/permission required (`AUTH_REQUIRED`, `KVM_PERMISSION_DENIED`). |
| `5` | Conflict / precondition failed (`GOLDEN_IN_USE`, `RECORD_ALREADY_ACTIVE`, `ELEMENT_AMBIGUOUS`, `ACTION_UNSUPPORTED`). |
| `6` | Rate limited / try again later (`VM_LIMIT_REACHED`). |
| `7` | Boot/startup timeout (`VM_BOOT_TIMEOUT`, `CONNECTION_TIMEOUT`, `OCR_TIMEOUT`, recording-related timeouts). |

**Sub-process exit codes.** `file exec` propagates the in-VM process's
exit code through `details.exit_code` in JSON mode and via the binary's
own exit code in text mode (today's behaviour, kept). The binary's exit
code is only `0` on a clean spawn AND a clean in-VM exit.

**Documented in help.** Every subcommand's `--help` lists the exit
codes it can produce (see §7).

---

<!-- tc: pending because the conversion has not reached sections 6 to 8.2 -->

## 6. Identifier conventions

| Identifier | Format | Round-trippable? |
|---|---|---|
| VM instance id | `testanyware-<8 hex>` by default; `vm start --id` accepts any id | Yes — accepted by `--vm`, returned by `vm start` / `vm list --json`. |
| Golden image name | `testanyware-golden-<platform>-<release>` | Yes — accepted by `vm delete` / passed via `--base`. |
| AX element id | Agent-issued opaque string | Yes — accepted by `agent --id <id>`, returned by `agent inspect --json`. |
| Window id | Agent-issued opaque string | Yes — accepted by `agent --window <filter>` (currently substring-matched on title/app, see Gap §G.4). |

### 6.1 Round-trip rule

Whatever JSON output a command emits, every `id`, `vm_id`,
`golden_name`, etc. field MUST be accepted unchanged as input to a
related command. Tests in CI MUST exercise this round-trip for every
identifier-bearing command pair.

### 6.2 Stable across calls

VM ids and golden names persist across invocations. AX element ids and
window ids are session-scoped and MAY change between snapshots — agents
that need persistence across snapshots MUST resolve by `(role, label,
window)` rather than by id.

### 6.3 No bare integers

Every identifier in JSON output is a string with a typed prefix. Bare
integer ids are forbidden for new commands.

---

## 7. Help-text template

Every subcommand's `--help` MUST emit, in order:

1. **One-line summary** (≤ 80 chars). This is the `about`.
2. **Description** — one paragraph: when to use it, when not to, key
   caveats.
3. **Synopsis** — the canonical invocation.
4. **Arguments and flags** — grouped: required, common options,
   target-resolution (`--connect`/`--vm`/...), output options
   (`--json`/`--output`/`--out`), advanced.
5. **Output formats** — explicit list of stable outputs: e.g. "Stable:
   `--json`. Human output is not a parsing target."
6. **Exit codes** — list every code this subcommand can produce.
7. **Examples** — **at least 2 concrete invocations**, ideally 3,
   covering the most common real uses. Hard requirement; PR review
   blocks on this.
8. **See also** — sibling subcommands and related commands.

### 7.1 Example template skeleton

```
Capture a screenshot from the VNC server.

Connects to the VNC channel of the target VM, captures the current
framebuffer, and writes a PNG. Use --region for crop, --window to crop
to a named window's bounds, or neither for the full screen.

USAGE:
    testanyware screen capture [OPTIONS]

TARGET (one required):
    --connect <PATH>     Connection spec JSON file
    --vm <ID>            VM instance id (testanyware-<hex8>)
    --vnc <HOST:PORT>    VNC endpoint
    --agent <HOST:PORT>  Agent endpoint (host:port, default port 8648)

OPTIONS:
    --out <PATH>         Output PNG path [default: screenshot.png]
    --region <X,Y,W,H>   Crop region in pixels
    --window <NAME>      Crop to the bounds of the named window (resolved via agent)
    -o, --output <FMT>   Output format: text|json [default: text]
        --json           Shortcut for --output json
    -q, --quiet
    -v, --verbose

OUTPUT:
    Stable formats: --json (schema: screen-capture).
    Text output is not a parsing target.

EXIT CODES:
    0  success
    1  generic failure (e.g. VNC_CAPTURE_FAILED)
    2  usage error
    3  VM_NOT_FOUND, WINDOW_NOT_FOUND
    4  AUTH_REQUIRED (accessibility permission needed for --window)
    7  CONNECTION_TIMEOUT

EXAMPLES:
    # Capture full screen of a running VM
    testanyware screen capture --vm "$TESTANYWARE_VM_ID" --out screen.png

    # Capture only the Settings window
    testanyware screen capture --vm "$TESTANYWARE_VM_ID" --window Settings --out settings.png

    # JSON receipt (path + bytes) for scripting
    testanyware screen capture --vm "$TESTANYWARE_VM_ID" --out screen.png --json

SEE ALSO:
    testanyware screen size, testanyware screen record, testanyware screen find-text
```

### 7.2 Aliases

The alias's `--help` emits a single line `Alias of \`testanyware screen
capture\`. Run that for full help.` and clap's defaults for short help.
This avoids duplication.

---

## 8. Discoverability commands

### 8.1 `testanyware capabilities`

**Required.** Stable JSON describing the binary's surface. Agents poll
this to detect feature availability without parsing help.

```json
{
  "schema_version": "1.0",
  "version": "0.7.2",
  "git_revision": "9d8400d",
  "subcommands": ["vm", "agent", "input", "screen", "file", "doctor", "capabilities", "schema", "llm-instructions"],
  "aliases": { "screenshot": "screen capture", "record": "screen record", "find-text": "screen find-text", "screen-size": "screen size", "upload": "file upload", "download": "file download", "exec": "file exec" },
  "output_formats": ["text", "json", "jsonl"],
  "features": {
    "idempotency_keys": false,
    "streaming": true,
    "dry_run": true,
    "schema_command": true
  },
  "platforms": { "host": ["macos", "linux"], "guest": ["macos", "linux", "windows"] },
  "error_codes": ["AUTH_REQUIRED", "CONNECTION_REFUSED", "VM_NOT_FOUND", ...]
}
```

Default output is JSON (this is a machine-only command). `--output
text` may pretty-print a summary; humans typically use `--help`
instead.

### 8.2 `testanyware schema <command>`

**Required.** Emits the JSON Schema for `<command> --json` output.
Reads from `docs/reference/cli-schemas/<schema-id>.json` (embedded at
build time via `include_str!`).

```
testanyware schema vm list
testanyware schema agent snapshot
```

Exit `3` with `code: SCHEMA_NOT_FOUND` (catalogued in §4.7) when the
command is unknown or has no declared schema.

<!-- tc: on -->

### 8.3 `testanyware llm-instructions`

**Required.** The full LLM usage guide for the binary, emitted as plain
text on stdout. This command is the single source of truth. It embeds
the repo-root `LLM_INSTRUCTIONS.md` at build time (`include_str!`). So an
LLM agent that has only the installed binary — e.g. a Homebrew install
with no source checkout — can read or prepend the complete reference.

It covers:

- One-paragraph "what this tool is and isn't."
- Mental model: noun-first command tree, verb-first aliases, the
  resolution chain (`--connect` → `--vm` → ...).
- Command reference for `vm`, `agent`, `input`, `screen`, `file`.
- End-to-end workflow recipes (discover-then-act, visual verification).
- Common mistakes:
  - "Don't parse text output. Use `--json`."
  - "Don't click stale pixel coordinates. Re-snapshot or filter by window."
  - "Don't poll `agent windows` in a tight loop. Use `agent wait`."
- Authentication and state assumptions.
- JSON output, exit codes, and idempotency conventions.
- On a macOS host, the TestAnyware app. This covers the one-time setup by a
  person and the rule that a client never starts the app. It also covers each
  error of §4.9 with its remedy, and golden creation from Terminal.

Every instruction MUST be runnable with only the installed binary — no
source checkout, no repo-relative paths. Keep the guide lean enough to
prepend as LLM context (well under 3000 tokens). If it must grow
substantially, add `--topic <name>` / `--section <name>` flags rather
than introducing subcommands.

---

<!-- tc: pending because the conversion has not reached sections 9 to 9.4 -->

## 9. Behaviour invariants

These are enforced by the contract on top of the structural rules above.

### 9.1 TTY adaptation

Every command MUST detect whether stdout is a TTY and adapt:

- No pagers, no spinners, no carriage-return animations when piped or
  captured.
- No ANSI color unless `--color always` or stdout is a TTY and
  `--color` defaults to `auto`.
- No interactive prompts. Destructive operations must accept `--yes` /
  `--force` and MUST refuse to prompt when stdin is non-TTY (exit `2`
  with `USAGE_ERROR`, message naming the right flag).

### 9.2 Idempotency and retry safety

Every mutating command's `--help` documents:

- Whether running it twice equals running it once.
- Whether it is safe to retry on transient failure.
- Whether it has partial-failure semantics.

| Command | Idempotent? | Retry-safe? |
|---|---|---|
| `vm start` | Yes (returns existing id if `--id` matches a running VM; otherwise creates a new one). | Yes — a failed start removes its own VM unless `--keep-on-failure` kept a still-running one. |
| `vm stop` | Yes (no-op if already stopped). | Yes. |
| `vm delete` | Yes (no-op if absent). | Yes. |
| `screen capture` | Yes. | Yes. |
| `screen record` | **No** (creates a fresh recording each call). | **No** — concurrent recordings against the same VM produce `RECORD_ALREADY_ACTIVE`. |
| `agent press` / `set-value` / `focus` / `show-menu` | **No** (intent-level UI actions; double-press has app-defined effect). | **Application-defined.** Help must say "retry only if the previous attempt's outcome is unknown." |
| `agent window-*` | Yes (idempotent in target state). | Yes. |
| `input *` | **No.** Pressing a key twice is two presses. | **No.** |
| `file exec` | Application-defined. | Application-defined. |
| `file upload` / `download` | Yes (full overwrite). | Yes. |

Idempotency keys are reserved (`--idempotency-key <uuid>`) but not
implemented in v1.

### 9.3 `--dry-run` coverage

Required on every mutating command:

`vm start`, `vm stop`, `vm delete`, `screen record`, `agent press`,
`agent set-value`, `agent focus`, `agent show-menu`, `agent window-*`,
`input *`, `file exec`, `file upload`, `file download`.

Read-only commands (`vm list`, `screen capture`, `screen size`,
`screen find-text`, `agent health`, `agent snapshot`, `agent inspect`,
`agent windows`, `agent wait`, `doctor`, `capabilities`, `schema`,
`llm-instructions`) do not need `--dry-run` and MUST NOT advertise it.

Dry-run output validates inputs, resolves the connection, and emits the
intended action without performing it. Exit code `0` on a successful
plan; the same JSON envelope as the real run with `"dry_run": true` set.

### 9.4 Default list limits

`vm list`, `agent windows`, `agent snapshot` (when emitting flat
element lists), and `screen find-text` (when no query is supplied)
default to `--limit 100`. `--all` is required to opt into unbounded
output. Truncation is signalled per §3.5.

<!-- tc: on -->

### 9.5 Hidden state

Any environment variable that influences behaviour MUST be listed in
`docs/reference/env-vars.md` AND surfaced in `capabilities --json`
under `env_vars`. Errors that result from a missing env var MUST name
that env var in `remediation`.

---

<!-- tc: pending because the conversion has not reached sections 10 to 12 -->

## 10. Gap report — Swift CLI vs. this contract (historical)

This was the audit deliverable that drove the port. Each row recorded what the
Rust port had to do differently from the now-retired Swift original — these
were never bugs to fix in `cli/`, but specifications the Rust port has since
satisfied. Retained as a design record of *why* each command is shaped as it
is.

### 10.1 Output

| Swift command | Gap | Rust target |
|---|---|---|
| `screenshot` | No `--json`. Confirmation prose only. | Add `--json` with `screen-capture` schema (path, bytes, region). Drop "Screenshot saved to X (N bytes)" prose; emit only on text mode. |
| `screen-size` | No `--json`. Emits `1920x1080` only. | Add `--json` (object with `width`, `height`). Keep `WxH` text shorthand for humans. |
| `record` | No `--json`. Emits status prose. | Add `--json` (path, fps, region, duration, frame count). |
| `find-text` | **Always emits JSON** even in text mode. No human alternative. Throws `ValidationError` when text not found rather than emitting an empty result. | Default to text mode (one detection per line); JSON is opt-in via `--json`. Empty result is exit `0` with empty array (text mode: silent), not an error — except when `--require-match` is set, which then exits `3` with `TEXT_NOT_FOUND`. |
| `exec` | Mixes stdout/stderr capture with prose; exit code via `ExitCode(result.exitCode)`. | Keep stdout/stderr passthrough in text mode. In `--json` mode emit a single object; do not interleave stdout. |
| `upload` / `download` | Confirmation prose only. | Add `--json` with `file-upload` / `file-download` schemas. |
| `vm list` | Hand-formatted via `VMListFormatter.render`. No `--json`. | Add `--json` (envelope per §3.5). Add `--filter platform=macos`, `--limit`, `--all`. |
| `agent windows` | Formatted text only. | Add `--json`. Truncation envelope per §3.5. |
| `agent press` / `set-value` / `focus` / `show-menu` / `wait` / `window-*` | Formatted text only. | Add `--json` with `agent-action` / `agent-window-action` schemas. |
| `agent health` | Prints `OK`/`UNHEALTHY`, throws `ExitCode.failure` on bad. | Keep text behaviour; add `--json` returning `{ok, agent_version, accessibility_status, uptime_s}`. |
| `doctor` | Decorated multi-line prose with ✓/✗/!. | Add `--json`. The check groups become structured records (five from the Swift command, plus `windows_tpm`, below). Keep the pretty text mode; have it render the same data. |

### 10.2 Help

| Swift command | Gap | Rust target |
|---|---|---|
| **All subcommands** | One-line `abstract` only; no examples, no exit codes section, no output-format declaration, no see-also. | Apply the §7 template uniformly. CI gate: every public subcommand's help text contains `EXAMPLES:` and at least 2 example invocations. |
| `vm delete` | No mention of `--force` semantics in help. | Help describes when `--force` is required (`GOLDEN_IN_USE`) and links to `vm list`. |
| `agent snapshot --open-menu` | Long inline help paragraph in `InputCommand.swift`. | Move the long-form explanation to `docs/reference/cli-commands.md`; help summarises and points there. |

### 10.3 Errors

| Swift command | Gap | Rust target |
|---|---|---|
| **Whole CLI** | No structured `code` field. Errors surface as `LocalizedError` text only. | Every error is a `(code, message, remediation, details)` tuple. JSON mode emits the structure; text mode renders it. |
| Connection resolution | Error mentions only `scripts/macos/vm-start.sh`. | `NO_CONNECTION_SPECIFIED` remediation lists every accepted form (flag, env var) and points to `docs/reference/connection-spec.md`. |
| `vm delete` (running clones) | `runningClonesPresent` doesn't expose the clone PIDs in a structured form. | `GOLDEN_IN_USE` JSON `details` includes `clone_ids` and `clone_pids`. |
| Agent errors | Three implementations emit different `error` strings (per `error-codes.md`). | Linux and Windows agents brought into line with §4.5 table; host CLI maps strictly. |

### 10.4 Conventions

| Swift | Gap | Rust target |
|---|---|---|
| Verb/noun mixed: `screenshot`, `record`, `find-text`, `screen-size`, `exec`, `upload`, `download` are top-level verbs; `vm`, `agent`, `input` are noun-first. | Inconsistent. | Canonical = noun-first per §1; verb-first names retained as aliases. |
| No aliases (`vm list` ≠ `vm ls`, `vm delete` ≠ `vm rm`). | Pure miss. | Add `ls`, `rm`/`remove`, `show` aliases per §1. |
| Flag inconsistency: `--detach`, `--viewer`, `--force`, ad-hoc per command. | No consolidation. | Reduce to the §2 table. `--detach` on `exec` stays (concept-specific); `--viewer` on `vm start` stays (concept-specific). New flags must reuse §2. |
| VM ids are already `testanyware-<hex>` prefix-typed. | Already compliant. | Keep. Document round-trip explicitly. |
| Window matching is substring case-insensitive across title and app — easy to over-match. | Loose. | Document explicitly in `--window` help; add `--window-regex` for exact-match scripts. (Optional v1; track in backlog.) |

### 10.5 Behaviour

| Swift | Gap | Rust target |
|---|---|---|
| No TTY detection. `print()` always. | Pure miss. | TTY-aware output, color, and progress per §9.1. |
| `--dry-run` not present anywhere. | Pure miss. | Add to all mutating commands per §9.3. |
| `--yes` not present (only `--force` on `vm delete`). | Partial. | `--yes` separate from `--force` per §2; required where prompts would otherwise appear. |
| `vm list` has no limit. | Unbounded. | `--limit 100` default per §9.4. |
| `record --duration 0` quietly maps to 300s. | Surprising. | Emit a `--verbose` notice; document explicitly; consider `--duration max` instead. |

### 10.6 Discoverability

| Swift | Gap | Rust target |
|---|---|---|
| No `capabilities` command. | Pure miss. | Add per §8.1. |
| No `schema <cmd>`. | Pure miss. | Add per §8.2. |
| No `llm-instructions`. | Pure miss. | Add per §8.3. |
| `--version` exists (via clap/ArgumentParser). | Compliant. | Keep; also expose under `capabilities --json`. |

---

## 11. Acceptance for downstream port tasks

Each Rust-port backlog task that adds or moves a command MUST verify
in its acceptance criteria:

1. The command's `--help` follows §7 (one-line, description, synopsis,
   flags, output formats, exit codes, ≥ 2 examples, see-also).
2. The command supports `--json` if it produces data, and the JSON
   output validates against `docs/reference/cli-schemas/<schema-id>.json`.
3. Errors surface a `code` from §4 and an exit code from §5.
4. Mutating commands have `--dry-run` (§9.3) and document idempotency
   (§9.2).
5. Identifiers in JSON output round-trip back as input to a sibling
   command (§6.1).
6. The command appears in `capabilities --json` (§8.1) and has a schema
   file at `docs/reference/cli-schemas/<schema-id>.json` even if
   stubbed.

The port-time CI gate runs `tests/cli-contract.rs` (to be added by the
first port task) which walks every public subcommand and asserts each
of these.

---

## 12. Out of scope

Listed here so they do not get added later by accident:

- Internationalisation. Help text is English-only.
- Localisation of error messages. Error `code` is the stable surface;
  `message` is a developer-language string.
- Hand-written shell completions. clap generates them from the same
  definitions.
- A REPL or interactive shell mode. The `llm-instructions` command
  covers the workflow guidance need.
- Plugin architecture. The contract assumes a single static binary.
