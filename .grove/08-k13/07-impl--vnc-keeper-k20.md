# vnc-keeper-k20

## Goal

Build the VNC keeper that ADR-0018 decides. The result: a `screen` or `input` command after
any pause no longer kills a tart guest on this macOS 27 host.

## Context

- **The design is ADR-0018.** Read `docs/adr/0018-tart-guests-get-a-vnc-keeper.md` first. It
  says what holds the connection, how the keeper starts and stops, and why. Build what it says.
  If the code shows that a part of it cannot work, stop and change the ADR before you build.
- **The evidence** is `docs/research/tart-vnc-reconnect-crash-macos-27.md`. ADR-0004 explains
  why each command opens its own connection.
- **Where the code goes.** These are pointers, not a plan. Read the code before you decide.
  - The internal command `_vnc-keeper` goes in `testanyware-cli` (`main.rs`). The contract
    hides each `_` command from `--help` (`docs/architecture/cli-design-contract.md`, section
    1). It is not in `surface.rs::CANONICAL_COMMANDS`.
  - The keeper loop (connect, read and discard, connect again, exit) needs a seam. Give it the
    host and port as values, so that a test can run it against a local `TcpListener`.
  - The spawn goes in `lifecycle.rs::start_tart`, or in `TartRunner::start` after
    `poll_vnc_url`. Both points come before any VNC client. Use `detached::spawn_detached`
    with `std::env::current_exe()` as the program. Pass the program in, so that a test can
    replace it. Clear an old `<id>.keeper.log` first, as `tart.rs::fresh_log_path` does.
  - `meta.rs::VmMeta` gains `keeper_pid: Option<i32>`, with `serde(default)` and
    `skip_serializing_if`.
  - `vm stop` (`lifecycle.rs::stop`, the tart arm) signals the keeper after `TartRunner::stop`.
    It signals only if the command line of that pid still names `_vnc-keeper` and this id.
    Keep that check pure (a function of the command line and the id), with a thin `ps` reader
    in `process.rs`.
  - `vm start --dry-run` starts no keeper. QEMU guests get none.
- **Docs that describe the workaround or the start and stop flow.** Search for more. Do not
  trust this list.
  - `docs/user/troubleshooting.md`, the crash entry. `vm start` now holds the connection. The
    recipe stays for a tart guest that `vm start` did not start.
  - `LLM_INSTRUCTIONS.md` (around line 221) and `README.md`.
  - `docs/architecture/vm-lifecycle.md`, the start flow and the stop flow.
  - The research note, at "Until the keeper is built".
  - `CONTEXT.md`, the term **VNC keeper**: say that it is built.
  - The copies under `website/docs/`. A document that fails `scripts/ste-lint.py` (the
  technical-communication checker) gets converted first, as its own change.
- **Running a clone from this harness.** `vm start --platform macos` works here. It waits about 4
  minutes, prints `warning: agent did not reach health`, and exits 0 with a running clone. VNC
  on loopback works. Use the branch binary `cli-rs/target/debug/testanyware`.

## Done when

- `task cli:test` passes, with unit tests at these seams:
  1. The keeper loop, against a local `TcpListener` stand-in. It holds the connection and writes
     nothing. It reads and discards what the server sends. It connects again after the server
     closes the connection. It exits when the listener is gone. It exits, and does not loop,
     when connections keep ending at once.
  2. The keeper's argument list, and the identity check that decides whether `vm stop` signals
     a pid.
  3. The meta, with and without `keeper_pid`. A meta written before this change still loads.
- `testanyware _vnc-keeper` does not appear in `--help`, and the contract tests pass.
- A Taskfile recipe runs the live checks below. Each one passes on this Mac (macOS 27.0.1,
  tart 2.40.1):
  1. A macOS clone from `vm start`: `screen size`, 5 minutes with no other client, `screen size`.
     The second command works and the clone still runs.
  2. The negative control, on a clone with a keeper. Kill the keeper, then run `screen size` at
     once: it works, so the keeper's close did not crash tart. Wait 70 s and run `screen size`:
     tart crashes. This proves that the keeper is what protects the guest.
  3. A Linux clone (`--platform linux`, `testanyware-golden-linux-24.04`): `screen size`, 70 s,
     `screen size`. The second command works.
  4. After `vm stop`, `pgrep -f _vnc-keeper` finds nothing, and `vm list` shows no running clone.
- The docs above describe the keeper, and each passes the checker.

## Notes

- Guests are driven only through the In-VM agent and VNC, never a host-specific channel.
- Never change a golden. Run each experiment on a clone. One macOS VM at a time.
- A crash costs a clone start, about 4 minutes. Plan the live checks so that each clone answers
  as much as it can.
- No macOS 26 host exists here. The keeper runs there too (ADR-0018), untested.
