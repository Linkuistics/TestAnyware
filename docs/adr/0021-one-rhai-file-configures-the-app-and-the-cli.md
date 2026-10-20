# 21. One Rhai file configures the TestAnyware app and the Host CLI

Date: 2026-10-20

## Status

Accepted (grove `gh-issue-8`, leaf `vm-broker-design-k3`). The area's spec is
`docs/specs/testanyware-app.md`.

## Context

Until now TestAnyware had no configuration file. The Host CLI read environment
variables and the XDG directories. The TestAnyware app starts at login, so it
never sees a shell's environment. It still needs the locations of `tart`, QEMU
and swtpm, and the state and data directories.

The owner asked for configuration as code, in the way of Ravel, but in Rhai.
Ravel evaluates an owner policy written in TypeScript. Modaliser, another of the
owner's apps, evaluates `~/.config/modaliser/config.scm` when it starts. There,
a restart reloads the file, and a file that fails leaves the app on its bundled
default. Issues #5 and #7 will later need policy functions, such as the choice of
a host for each guest.

## Decision

**One optional file, `~/.config/testanyware/config.rhai`, configures the
TestAnyware app and the Host CLI. Both evaluate it with one shared evaluator.**

- The script runs with no access to files, processes or the network, and under
  an operation limit. Its value is a map of settings. An unknown key is an error.
- In this delivery the settings are the tool locations, `state_dir` and
  `data_dir`. Policy functions come with the deliveries that need them.
- The app evaluates the file when it starts, and a restart of the app reloads it.
  If the file fails, the app runs with its built-in defaults and reports the
  error. The Host CLI refuses to run with a file that fails.
- In the Host CLI, a flag beats an environment variable, which beats the file,
  which beats the built-in default. So each variable keeps its meaning.
- Each request to the app names the directories that the Host CLI resolved. The
  app refuses a request whose directories differ from its own, and the error
  names both and the remedy.

## Considered options

- **A file for the app alone.** The Host CLI would keep only its flags and its
  variables, and one process would own the file. The owner chose a file for both
  processes. One shared evaluator keeps the two readings of one file the same.
- **A declarative file, such as TOML.** Rejected. The owner asked for code, and
  the policy functions of #5 and #7 need code.
- **The app honours the directories of each request.** Rejected. After a
  restart, adoption would have to find every directory that any request ever
  named.
- **The app ignores `XDG_STATE_HOME` and `XDG_DATA_HOME`.** Rejected. A variable
  that a user sets would then have no effect on macOS, which changes its
  meaning.
- **The app reads the environment of the first client.** Rejected. Docker
  Desktop and VirtualBox both leaked the environment of their first starter into
  a long-lived process.

## Consequences

- The Host CLI and the app gain a dependency on the Rhai crate. It is pure Rust,
  so the cross-builds of the Host CLI need no C toolchain for it.
- A user who sets `XDG_STATE_HOME` on macOS also sets `state_dir` in the file, or
  each command fails with the remedy.
- Golden creation sends no request, so on a macOS host it compares its directories
  with the app's itself, before it starts. It takes the app's directories from the
  file and from the built-in defaults under `HOME`. The app takes its defaults from
  `HOME` alone, so the two agree without a running app.
- An edit of the file reaches the app only after a restart of the app. `doctor`
  reports when the app evaluated an older file.
- A test names another file, or none, through `TESTANYWARE_CONFIG`. The remedy of
  `CONFIG_INVALID` also names `TESTANYWARE_CONFIG=none`, to run one command without
  the file. Only the Host CLI reads the variable.
