# config-evaluator-cli-k18

## Goal

Build the shared Rhai evaluator. Make the Host CLI honour `config.rhai` on Linux and Windows hosts, with the `doctor` exception and its `config` check. A macOS host ignores the file until `golden-directory-check-k34`.

## Context

- `docs/specs/testanyware-app.md`, section "Configuration", and the requirement "doctor reports the failures that it diagnoses".
- ADR-0021.
- The code in the CLI and the vm crate that reads the directory and tool variables today.
- The doctor code of the CLI and its report schema in `docs/reference/cli-schemas`.

## Done when

- A crate holds the evaluator. It limits operations, gives the script no access to files, processes or the network, and rejects an unknown key.
- One activation switch decides whether a host reads the file. It is on for Linux and Windows hosts. It is off for a macOS host. A parameter of the pure functions carries it, so that a test can turn it on for any host.
- With the switch on, the Host CLI resolves `state_dir`, `data_dir` and the tool locations with the order flag, environment variable, file, default.
- With the switch on, the Host CLI takes the tool locations where it runs the tools itself, golden creation included.
- With the switch on, a file that fails gives `CONFIG_INVALID` with exit 2, the file, the line and the message. Each command except `doctor` refuses to run.
- `doctor` runs with a file that fails. Its `config` check fails with the `CONFIG_INVALID` error object, and `doctor` exits with 1, not 2. Its other checks run with the values that the CLI resolves without the file.
- With the switch off, the `config` check reports that it does not apply to the host.
- The `--json` schema of `doctor` documents the `config` check.
- `TESTANYWARE_CONFIG` names another file or `none`.
- The crate builds in the Linux and Windows cross-builds with no C toolchain.
- Pure-function tests cover the order of sources, every error, and the report of `doctor` with a file that fails.

## Notes

- Without the file, every command behaves as before.
- A macOS host stays as it is until `golden-directory-check-k34` turns the switch on. The app cannot read the file before `config-in-app-k33`, and a CLI that read it earlier would disagree with its app about directories.
- `doctor-app-check-k20` adds the `app` check. It does not change the `config` check.

## Decisions (running log)

- **The evaluator is a new crate, `testanyware-config`, with no dependency on the VM crate.** It reads and checks the file. It does not order the sources, because the Host CLI and the app order them differently. The app (`config-in-app-k33`) depends on the same crate. It depends on Rhai 1.26.1 with `no_module` and `no_time`. `no_module` removes `import`, the one way a Rhai script reaches a file, so the "no files" limit holds by the build and no runtime setting can undo it. `no_time` removes the clock. Source: https://docs.rs/crate/rhai/1.26.1/features and the crate's `src/api/limits.rs`.
- **The limits.** 100 000 operations, 32 call levels, expression depth 64 and 32, a string of 4096 bytes, an array of 1024 and a map of 256. The engine disables `eval` and drops `print` and `debug`, because `print` writes to stdout and that carries the `--json` envelope. A test shows each of `import`, `eval`, `timestamp` and `loop {}` failing for its own reason.
- **The keys.** `tools` (a map of `tart`, `"qemu-system-aarch64"`, `swtpm`), `state_dir` and `data_dir`. `state_dir` and `data_dir` are the directories themselves, the ones the request carries as `Directories`. They are not a replacement of `XDG_STATE_HOME`, to which `testanyware` is appended. Every path must be a non-empty absolute path, because a relative one means another place in each working directory. There is no `~` expansion. An empty file is an error, because its value is `()` and not a map.
- **The line.** Rhai places a syntax error and a runtime error, so those carry a line. A wrong type or an unknown key is found after the script finished, so it carries no line (`null`). I did not guess a line from the text.
- **The location.** `TESTANYWARE_CONFIG` names another file, or `none`. An empty value counts as unset. The default is `~/.config/testanyware/config.rhai` from `HOME`, else `USERPROFILE`, on every host. A file that does not exist is not an error. A file that cannot be read is.
- **Where the pure function lives.** `testanyware_cli::host_config::resolve(activation, env, read)` returns the status of the file (`NotApplicable`, `Disabled`, `Absent`, `Loaded`, `Failed`) with the `VmPaths` and the tool locations that follow. A file that fails does not make it fail: it gives `Failed` and the values without the file, because `doctor` runs on those. `Activation` is in the config crate, and `Activation::for_this_host()` is the one switch. `golden-directory-check-k34` deletes its `macos` arm.
- **Two process-wide installs, and why.** The Host CLI resolves once at its start. The directories go to `host_config::vm_paths()`, which replaced `VmPaths::from_process_env()` at each CLI call site, and to `EnvProvider::process()` for the lookup of a `--vm` spec. The tool locations go to `testanyware_vm::tools::install_configured`, a `OnceLock` that every `ToolSearch::from_process_path()` reads. The vm crate has about thirty such calls, golden creation among them (`qemu_profile::which` is one), so a parameter through all of them was too large for this leaf. The app and the tests install nothing, so they are unaffected. A test of the install has a binary of its own in the vm crate, because the install is once per process.
- **A configured tool is found at its location or not at all.** `find` does not fall back to the directories for a tool that the file names. Falling back would hide a wrong path. The message of the existing miss still says "not found on PATH". I left that text.
- **Which commands refuse.** All of them except `doctor`, as the leaf says. That includes `capabilities`, `schema` and `llm-instructions`, so the remedy carries its own way out (`TESTANYWARE_CONFIG=none`). The hidden `_vnc-keeper` is no command of the contract and uses no setting, so it ignores the file: a later edit must not stop a keeper of a running guest. The refusal reads `--json` from the arguments, up to a `--`, because it comes before the dispatch.
- **`CONFIG_INVALID` is in the protocol crate.** `AppError::config_invalid(file, line, message)` builds the object. The Host CLI uses it for the envelope and for the `doctor` check, and `config-in-app-k33` uses it for the status of the app. The exit code of the code is 2 in `exit_code_for`, and `ERROR_CODES` lists it.
- **The `config` check.** `status` is `pass` (`loaded`, `absent` or `disabled`), `skip` (`not_applicable`) or `fail` (`invalid`, with `error`). The `doctor.json` schema requires it and documents each field, and a unit test holds the renderer to the schema. A failing check makes `ok` false, so `doctor` exits with 1.
- **What a macOS host shows.** Its `config` check is `skip`, `not_applicable`. I ran the tests of the active host on this Mac by turning the switch on for one run (the six tests of `tests/config-file.rs`), then restored the switch and the test gates. The integration tests of an active host are `cfg(not(target_os = "macos"))`, so they run on the Linux and Windows harnesses.
- **The cross-builds.** `cargo check -p testanyware-config` passes for `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-pc-windows-gnu` and `aarch64-pc-windows-gnullvm`. The tree of the crate holds no `cc`, no `-sys` crate and no build script that compiles C. Checking the vm crate on those targets needs a C compiler, but the cause is an older dependency, not this leaf.
- **Left alone.** `resolve.rs` computes the Windows state directory from `USERPROFILE\.local\state`, but `VmPaths` uses `%LOCALAPPDATA%`. The two disagree already. I did not touch it, and no setting here depends on it.
