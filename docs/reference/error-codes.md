# Error Codes & Shapes

Catalogue of every error type the CLI or driver can surface, their
cases, and when they are thrown. Swift errors surface as non-zero exit
codes with a human-readable stderr message via `LocalizedError`; agent
errors surface as HTTP non-2xx with an `ErrorResponse` JSON body.

## Host CLI / Driver errors

Source files all live under `cli/Sources/TestAnywareDriver/`.

### `ConnectionSpecError` (`Connection/ConnectionSpec.swift`)

| Case | When thrown |
|------|-------------|
| `invalidPlatform(String)` | `--platform` value is not `macos`, `windows`, or `linux`, or the env var / spec file contains an unknown value. |
| `emptyHost` | `--vnc` or `--agent` endpoint parsed to an empty host. |
| `invalidPort(String)` | Port segment of an endpoint isn't a number in `1..=65535`. |

### `VNCCaptureError` (`VNC/VNCCaptureError.swift`)

| Case | When thrown |
|------|-------------|
| `notConfigured` | `VNCCapture` used before `connect()` succeeded. |
| `connectionFailed(String)` | Socket / RFB handshake failure; message carries detail. |
| `disconnected` | Peer dropped during a request. |
| `framebufferNotReady` | Screenshot requested before a full framebuffer update was received. |
| `captureFailed` | VNC-side capture routine failed (e.g. encoding rejected). |
| `encodingFailed` | PNG/HEVC encoding step failed. |
| `timeout` | Generic operation timeout. |

### `FramebufferConverterError` (`VNC/FramebufferConverter.swift`)

| Case | When thrown |
|------|-------------|
| `zeroDimensions` | Framebuffer has zero width or height. |
| `pixelCountMismatch(expected:got:)` | Received byte count doesn't match the advertised size. |
| `pngEncodingFailed` | CoreGraphics PNG encode returned nil. |
| `cgImageCreationFailed` | `CGImage` construction from the pixel buffer failed. |

### `AgentTCPClientError` (`Agent/AgentTCPClient.swift`)

| Case | When thrown |
|------|-------------|
| `connectionFailed(String)` | TCP connect to the agent failed. |
| `httpError(Int, String)` | Agent returned a non-2xx status. String is the parsed `ErrorResponse.error` field (or raw body if it doesn't decode). |
| `decodingFailed(String)` | Agent response body didn't decode to the expected type. |

### `VMLifecycleError` (`VM/VMLifecycle.swift`)

| Case | When thrown |
|------|-------------|
| `vncTimeout` | VNC did not become reachable within the boot window. |
| `unsupportedBackend(String)` | Neither tart nor QEMU can serve the requested platform. |
| `stopFailed(id:)` | `testanyware vm stop` couldn't terminate the VM cleanly. |
| `goldenNotFound(String)` | Requested golden image name doesn't exist. |
| `runningClonesPresent(name:pids:)` | `testanyware vm delete` refused because clones of the golden are running; use `--force` to override. |
| `tartDeleteFailed(String)` | tart CLI returned an error during golden deletion. |

### `TartRunnerError` (`VM/TartRunner.swift`)

| Case | When thrown |
|------|-------------|
| `vncURLMalformed(String)` | `tart ip` / `tart vnc` returned an unexpected URL format. |
| `commandFailed(String)` | A `tart` subprocess exited non-zero. |

### `QEMURunnerError` (`VM/QEMURunner.swift`)

| Case | When thrown |
|------|-------------|
| `uefiNotFound(String)` | UEFI firmware path (`QEMU_EFI.fd`) missing at expected location. |
| `qemuFailedToStart` | `qemu-system-aarch64` failed to launch or exited immediately. |
| `monitorDiscoveryFailed` | QEMU monitor socket didn't come up in time. |
| `commandFailed(String)` | A QMP command returned an error. |

### `DetachedProcessError` (`VM/DetachedProcess.swift`)

| Case | When thrown |
|------|-------------|
| `spawnFailed(errno:executable:)` | `posix_spawn` failed for the given executable. |

### `StreamingCaptureError` (`Capture/StreamingCapture.swift`)

| Case | When thrown |
|------|-------------|
| `alreadyRecording` | `start()` called while state is already `recording`. |
| `notRecording` | `appendFrame()` or `stop()` called while state is `idle`. |
| `pixelBufferPoolUnavailable` | `AVAssetWriterInputPixelBufferAdaptor` pool not ready (usually means the writer never started). |
| `pixelBufferCreationFailed` | `CVPixelBufferCreate` returned non-success. |

### `OCRBridgeError` (`OCR/OCRChildBridge.swift`)

| Case | When thrown |
|------|-------------|
| `permanentlyUnavailable(reason:)` | EasyOCR bridge cannot be recovered (e.g. Python interpreter missing). CLI hard-fails unless `TESTANYWARE_OCR_FALLBACK=1`. |
| `childCrashed` | The EasyOCR daemon subprocess exited unexpectedly. |
| `responseTimeout` | Daemon didn't respond within the timeout. |

### `PlatformKeymapError` (`Input/PlatformKeymap.swift`)

| Case | When thrown |
|------|-------------|
| `unknownKey(String)` | Key name not in the supported set (see [key-names.md](key-names.md)). |
| `unknownButton(String)` | Mouse button name not in `{left, right, middle, center}`. |

### `ServerClientError` (`Server/ServerClient.swift`)

| Case | When thrown |
|------|-------------|
| `socketCreateFailed(Int32)` | UNIX-domain socket creation failed (carries `errno`). |
| `connectFailed(Int32)` | Connect to the internal `_server` socket failed. |
| `serverStartTimeout` | `_server` didn't print its `ready` line within the timeout. |
| `serverStartFailed(String)` | `_server` aborted startup (carries its stderr message). |
| `httpError(Int, String)` | Internal `_server` returned non-2xx. |

`WireParseError` (private to `ServerClient`) — internal HTTP framing
parse failures (`missingHeaderBlock`, `malformedStatusLine`,
`bodyIncomplete`). These should only ever surface if the `_server`
process is corrupt.

## Agent HTTP error shape

All three agents (macOS Swift, Linux Python, Windows C#) return errors
as HTTP non-2xx with a JSON body of shape:

```json
{ "error": "<short machine-parsable key>", "details": "<optional human detail>" }
```

Source (host side): `cli/Sources/TestAnywareAgentProtocol/AgentResponses.swift` —
```swift
public struct ErrorResponse: Codable, Sendable, Equatable {
    public var error: String
    public var details: String?
}
```

When `AgentTCPClient` sees a non-2xx response it decodes `ErrorResponse`
and wraps it as `AgentTCPClientError.httpError(status, error.error)`.

### Common error strings produced by the agents

| `error` string | Emitted by | Meaning |
|----------------|------------|---------|
| `not_found` / `element_not_found` | `/inspect`, `/press`, `/set-value`, `/focus`, `/show-menu` | Element query matched zero elements. |
| `ambiguous` / `multiple_matches` | Same endpoints | Query matched multiple elements; supply `--index N` or a more specific filter. |
| `window_not_found` | `/window-*` endpoints | No window matched the `--window` filter. |
| `action_unsupported` | `/press`, `/set-value`, etc. | Element matched but does not support the requested action. |
| `accessibility_unavailable` | `/health`, `/snapshot`, `/wait` | OS-level accessibility disabled or not yet granted. |
| `exec_failed` | `/exec` | Process failed to spawn (exec itself; exit codes are returned inside `ActionResponse`). |
| `upload_failed` / `download_failed` | `/upload`, `/download` | File I/O error on the VM. |

The agents don't currently agree on a single canonical set of error
strings — the host-side wrapper compares by `status` first, then by
`error` string as a best-effort diagnostic. Clients should tolerate
unknown error strings.

Exec output (`/exec`) is **not** an error: it always returns 2xx with
`{success, message, stdout, stderr, exitCode}` so the caller can
inspect the exit code independent of the transport result.
