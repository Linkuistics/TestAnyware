# app-vm-list-k13

## Goal

Link the lifecycle code of the vm crate into the app, and serve `vm list` through `GET /v1/vms`.

## Context

- `docs/specs/testanyware-app.md`, sections "The protocol" and "Lifecycle and adoption".
- The vm crate lifecycle module and the code of `vm list` in the CLI.
- The harness of `cli-app-agent-routing-k24`.

## Done when

- The app executable links the vm crate. It resolves tools with the built-in search, and it reads the state directory of its own configuration.
- `GET /v1/vms` returns the listing that `vm list` prints today, in text and in `--json`.
- With the routing on, `vm list` asks the app. Without it, the command is direct as before.
- A test with sidecars in a temporary state directory and an injected `tart list` shows the same output through the app as the direct output.
- The status item menu lists the running VMs.

## Notes

- A VM that the direct CLI started has the same sidecars. The app lists it, because the list is rebuilt at each request.
- Stop at the listing. The start and the stop have their own leaves.

## Decisions (running log)

- The app builds the whole view and sends it as JSON: `items`, `returned`, `total`, `truncated`. The Host CLI prints text and `--json` from that view, on both paths, so the output cannot drift between them. The filter and the limit run in the app, because the spec says a lifecycle request carries the parsed options. The rejected option was to send every row and cut in the CLI.
- The options travel in the query of `GET /v1/vms`: `limit`, `all` and one `filter=field%3Dvalue` per pair. The request builder and the parser sit together in `testanyware-app-protocol` (`list.rs`). A request with no `limit` fails with `USAGE_ERROR`. The CLI always states one.
- `ListItem`, `parse_filter`, `filter_matches`, `apply_limit` and the new `build_view` moved from the CLI to `testanyware-vm` (`listing.rs`), with owned `String` fields so the view can be read back from JSON. Both the app and the direct path now call the one function.
- The tool search is a value, `testanyware_vm::tools::ToolSearch`. The Host CLI keeps its `PATH` search through `ToolSearch::from_process_path()`, and `qemu_profile::which` now calls it. The app uses `ToolSearch::builtin()`: `/opt/homebrew/bin`, `/usr/local/bin`, `/usr/bin`. Only the listing takes the search so far. The start and stop leaves thread it further.
- The injected `tart list` is a fake script in a directory that the app takes from `--tool-dir` in test mode. The harness always passes one, empty by default, so no test runs the developer's `tart`. The first run of `protocol_core.rs` without it listed the real tart goldens of this Mac.
- The QEMU session directories come from `std::env::temp_dir()` of the app's own process. The harness gives the app and the CLI the same `TMPDIR`.
- The menu lists the running VMs from the same listing. It waits two seconds on its own thread and says "no answer from the listing" after that, so a hung `tart` cannot freeze the status item.
- `cli-contract.rs::vm_list_json_emits_truncation_envelope` now runs through a test app on Unix, because a macOS host has no direct `vm list`.
- No in-session reviewer was spent. Nothing here is hard to reverse, and the new seam has executable tests, one mutation-checked.
