---
name: using-testanyware
description: Drives GUI / desktop / windowed apps inside isolated macOS / Linux / Windows VMs. Use when you need to run, launch, test, drive, screenshot, or screen-record a GUI / desktop / windowed application — especially when its windows would grab focus, move the mouse, or otherwise interfere with your own machine, or when the app needs a Windows or Linux environment you are not running. Also covers experimenting with a UI, or capturing screenshots / videos of a running app for documentation. Not for headless services or pure-logic / unit tests with no UI.
harnesses: [any]
---

# Using TestAnyware

## Overview

`testanyware` drives a **GUI application inside an isolated guest VM** — macOS,
Linux, or Windows — over two channels: an in-VM accessibility **agent** (semantic
UI tree, act on elements by role + label, exec, file transfer) and **VNC**
(screenshots, keyboard / mouse, video, OCR). Reach for it instead of launching a
GUI app on your own machine: the app's windows, focus-stealing, and input never
touch your host, and you can drive a Windows or Linux app while on macOS.

The binary is already on your `PATH` and self-documents. **This skill is the
trigger and the workflow spine; the command reference lives in the tool itself**
(see Command detail) — don't reproduce it here.

## When to use

- The app's UI would **interfere with your machine** — grabs focus, moves the
  mouse, or takes over the screen.
- The app needs a **Windows or Linux** environment and you are on macOS (or vice
  versa).
- You want to **experiment** with a UI without it landing on your own desktop.
- You need **screenshots or screen recordings** of a running app for docs.

**Not for:** headless services, APIs, or pure-logic / unit tests with no GUI —
run those directly.

## Workflow spine

1. `testanyware doctor` — confirm the local install and host prerequisites (on a
   fresh machine, or whenever behaviour looks off).
2. `testanyware vm list` — check a golden image exists for your platform. If not,
   `testanyware vm create-golden --platform <os>` (slow, one-time).
3. Start the VM and pin its id so later commands need no connection flags:
   ```bash
   vmid=$(testanyware vm start --platform macos --wait)   # macos | linux | windows
   export TESTANYWARE_VM_ID="$vmid"
   ```
   Pass `--wait` to a macOS start only. It can wait a long time — see
   **Waiting for a macOS slot** before you run it in a foreground tool call.
4. Drive it — prefer the **agent** (semantic, survives layout changes) over
   pixel coordinates:
   ```bash
   testanyware agent snapshot --mode interact            # see the UI
   testanyware agent press --role button --label "Save"  # act by role + label
   testanyware input type "hello"                        # raw keyboard / mouse when needed
   ```
5. Observe — `testanyware screen capture -o out.png`, `screen record -o out.mp4`,
   or `screen find-text "..."` for visual / OCR checks.
6. `testanyware vm stop "$vmid"` — always stop the clone when finished.

## Waiting for a macOS slot

A Mac runs two macOS guests at most, and other agents and groves on the same Mac
share those slots.

- **Pass `--wait` to every macOS `vm start`.** Without it, a full limit fails at
  once with `VM_LIMIT_REACHED` (exit 6).
- **Run a long wait in the background.** A bare `--wait` gives up after 600 s,
  and a shell tool call often ends sooner, which cancels the start. Start it in
  the background, do the next step of your work, and read the result when the
  start ends:
  ```bash
  testanyware vm start --platform macos --wait --json > start.json 2> start.err &
  ```
- **While a start waits, read `testanyware vm slots --json`.** It lists each
  holder and each waiter, with who asked. An older release has no `vm slots`.
  There, `start.err` names the holders.
- **After a wait that ends with `VM_LIMIT_REACHED`, run `vm slots`.** A holder
  with `kind` `external` is a guest that TestAnyware did not start.
- A macOS `vm create-golden` also waits for a slot when the TestAnyware app runs.
  Run it in the background too.

## Command detail

Don't memorise or duplicate the surface — discover it live:

- `testanyware llm-instructions` — the full LLM playbook (mental model, every
  command, connection resolution, workflow patterns, gotchas).
- `testanyware capabilities --json` — machine-readable surface: every subcommand,
  alias, flag, and error code.
- `testanyware schema <command>` — JSON Schema for a command's `--json` output.
- `testanyware <command> --help` — per-command usage, examples, exit codes.

Parse `--json`, never the human-readable text output.

## Host-side gotchas

Host-environment facts the CLI cannot self-report:

- **Run `testanyware doctor` first** on a new machine — it catches missing host
  prerequisites before a confusing downstream failure.
- **Golden images must already exist.** `vm start` clones a pre-built golden;
  `vm create-golden` builds one and is slow. Check `vm list` before assuming a
  platform is ready.
- **Don't trust `tart ip`** (macOS backend) — it returns a cached address. Read
  the **state column of `tart list`** to tell whether a VM is running or stopped.
- **Keep golden images minimal.** Don't bake test-specific tooling into an image;
  install what a test needs at runtime with `testanyware file exec`.
