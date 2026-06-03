# Error Codes & Shapes

The Host CLI surfaces errors as **stable string codes** carried in the `--json`
error envelope and mapped to process exit codes. The codes are part of the
public surface — `capabilities` lists them and `schema` references them.

- **Live catalogue:** `cli-rs/crates/testanyware-cli/src/surface.rs` —
  `pub const ERROR_CODES: &[&str]` is the single source of truth.
- **Spec:** `docs/architecture/cli-design-contract.md` §4 (codes) and §3.4 /
  the exit-code table (how codes map to exit status). The
  `cli-contract.rs::errors_carry_stable_code_and_correct_exit` test gates this.

## Host CLI error codes

Grouped as in the contract (§4.1–§8.2). This table is a convenience copy; if it
disagrees with `surface.rs`, `surface.rs` wins.

### §4.1 — Connection

| Code | When |
|------|------|
| `AUTH_REQUIRED` | Agent rejected the request: TCC / accessibility permission not granted. |
| `CONNECTION_REFUSED` | TCP/RFB connect refused by the endpoint. |
| `CONNECTION_TIMEOUT` | Endpoint did not accept a connection within the window. |
| `CONNECTION_DROPPED` | Peer dropped mid-request. |
| `INVALID_ENDPOINT` | Malformed `--connect` / `--vnc` / `--agent` endpoint. |
| `NO_CONNECTION_SPECIFIED` | No connection given via flag, env, or spec file. |
| `INVALID_PLATFORM` | `--platform` value is not `macos`, `windows`, or `linux`. |
| `SSH_CONNECT_FAILED` | SSH to the guest failed (golden provisioning paths). |

### §4.2 — VM lifecycle

| Code | When |
|------|------|
| `VM_NOT_FOUND` | No VM with the given id. |
| `VM_BOOT_TIMEOUT` | VNC/agent did not become reachable within the boot window. |
| `VM_STOP_FAILED` | `vm stop` could not terminate the VM cleanly. |
| `VM_BACKEND_UNSUPPORTED` | Neither tart nor QEMU can serve the requested platform. |
| `GOLDEN_NOT_FOUND` | Requested golden image name doesn't exist. |
| `GOLDEN_IN_USE` | `vm delete` refused — clones of the golden are running (use `--force`). |
| `GOLDEN_CREATE_FAILED` | `vm create-golden` aborted. |
| `TART_FAILED` | A `tart` subprocess exited non-zero. |
| `QEMU_FAILED` | `qemu-system-*` failed to launch or a QMP command errored. |
| `KVM_PERMISSION_DENIED` | `/dev/kvm` not accessible (Linux host). |
| `SWTPM_MISSING` | `swtpm` binary not found (TPM-backed guests). |
| `UEFI_NOT_FOUND` | UEFI firmware path missing. |
| `SPAWN_FAILED` | Process spawn failed for a backend executable. |

### §4.3 — VNC / framebuffer

| Code | When |
|------|------|
| `VNC_NOT_CONFIGURED` | RFB used before a connection succeeded. |
| `VNC_FRAMEBUFFER_NOT_READY` | Capture requested before a full framebuffer update arrived. |
| `VNC_CAPTURE_FAILED` | VNC-side capture routine failed. |
| `VNC_ENCODING_FAILED` | PNG/image encoding step failed. |
| `VNC_PIXEL_MISMATCH` | Received byte count doesn't match the advertised size. |
| `VNC_DIMENSIONS_ZERO` | Framebuffer reported zero width or height. |

### §4.4 — Screen record

| Code | When |
|------|------|
| `RECORD_ALREADY_ACTIVE` | Recording start requested while already recording. |
| `RECORD_NOT_ACTIVE` | Frame/stop requested while idle. |
| `RECORD_BUFFER_UNAVAILABLE` | Encoder pixel-buffer pool not ready. |
| `RECORD_BUFFER_CREATE_FAILED` | Pixel-buffer creation returned non-success. |

### §4.5 — Agent actions

| Code | When |
|------|------|
| `ELEMENT_NOT_FOUND` | Element query matched zero elements. |
| `ELEMENT_AMBIGUOUS` | Query matched multiple elements; narrow it or pass `--index`. |
| `WINDOW_NOT_FOUND` | No window matched the `--window` filter. |
| `ACTION_UNSUPPORTED` | Element matched but does not support the requested action. |
| `EXEC_FAILED` | Process failed to spawn (exec itself). |
| `UPLOAD_FAILED` / `DOWNLOAD_FAILED` | File I/O error on the VM. |
| `AGENT_ERROR_UNKNOWN` | Agent returned an error string the host doesn't map. |

### §4.6 — General

| Code | When |
|------|------|
| `USAGE_ERROR` | Invalid arguments / flag combination. |
| `IO_ERROR` | Local I/O failure. |
| `OCR_UNAVAILABLE` | OCR engine cannot be reached/recovered (e.g. EasyOCR bridge down). |
| `OCR_CHILD_CRASHED` | The OCR daemon subprocess exited unexpectedly. |
| `OCR_TIMEOUT` | OCR daemon didn't respond within the timeout. |
| `UNKNOWN_KEY` | Key name not in the supported set (see [key-names.md](key-names.md)). |
| `UNKNOWN_BUTTON` | Mouse button name not in `{left, right, middle, center}`. |
| `INTERNAL` | Unclassified internal error. |

### §4.7 / §8.2 — Discoverability

| Code | When |
|------|------|
| `TEXT_NOT_FOUND` | `screen find-text` matched no text. |
| `SCHEMA_NOT_FOUND` | `schema <command>` requested an unknown command. |

## Agent HTTP error shape

The in-VM agents (macOS, Linux, Windows) return errors as HTTP non-2xx with a
JSON body of shape:

```json
{ "error": "<short machine-parsable key>", "details": "<optional human detail>" }
```

The agent-client crate (`testanyware-agent-client`) decodes that body and maps
the `error` key to the stable host codes above (e.g. `accessibility_unavailable`
→ `AUTH_REQUIRED`, `element_not_found` → `ELEMENT_NOT_FOUND`). The
contract's §4.5 mapping table is authoritative for the translation.

### Common error strings produced by the agents

| `error` string | Emitted by | Meaning |
|----------------|------------|---------|
| `not_found` / `element_not_found` | `/inspect`, `/press`, `/set-value`, `/focus`, `/show-menu` | Element query matched zero elements. |
| `ambiguous` / `multiple_matches` | Same endpoints | Query matched multiple elements; supply `--index N` or a more specific filter. |
| `window_not_found` | `/window-*` endpoints | No window matched the `--window` filter. |
| `action_unsupported` | `/press`, `/set-value`, etc. | Element matched but does not support the requested action. |
| `accessibility_unavailable` | `/health`, `/snapshot`, `/wait` | OS-level accessibility disabled or not yet granted. |
| `exec_failed` | `/exec` | Process failed to spawn (exit codes are returned inside the response, not as an error). |
| `upload_failed` / `download_failed` | `/upload`, `/download` | File I/O error on the VM. |

The agents don't all agree on a single canonical set of error strings — the
host-side mapping compares by HTTP status first, then by `error` string as a
best-effort diagnostic. Clients should tolerate unknown error strings.

Exec output (`/exec`) is **not** an error: it always returns 2xx with
`{success, message, stdout, stderr, exitCode}` so the caller can inspect the
exit code independent of the transport result.
