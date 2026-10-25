# 21. One Rhai file configures the TestAnyware app and the Host CLI

Date: 2026-10-20

## Status

Accepted (grove `gh-issue-8`, leaf `vm-broker-design-k3`). The leaf
`default-config-file-k58` added the file that the app writes. The leaf
`admission-queue-k2` of grove `gh-issue-7-vm-queue` added `macos_slots`. The
area's spec is `docs/specs/testanyware-app.md`, and `docs/specs/admission-queue.md`
uses the slot count.

## Context

Until now TestAnyware had no configuration file. The Host CLI read environment
variables and the XDG directories. The TestAnyware app starts at login, so it
never sees a shell's environment. It still needs the locations of `tart`, QEMU
and swtpm, and the state and data directories.

The owner asked for configuration as code, in the way of Ravel, but in Rhai.
Ravel evaluates an owner policy written in TypeScript. Modaliser, another of the
owner's apps, evaluates `~/.config/modaliser/config.scm` when it starts. There,
a restart reloads the file, and a file that fails leaves the app on its bundled
default. Issue #5 will later need policy functions, such as the choice of a host
for each guest. The admission queue of issue #7 needed only a number, the slot
count.

On 2026-10-22 the owner asked for a minimal file when TestAnyware finds none. A
person who sees the file learns what it can hold, and the app's menu then offers
the file.

## Decision

**One optional file, `~/.config/testanyware/config.rhai`, configures the
TestAnyware app and the Host CLI. Both evaluate it with one shared evaluator.**

- The script runs with no access to files, processes or the network, and under
  an operation limit. Its value is a map of settings. An unknown key is an error.
- The settings are the tool locations, `state_dir`, `data_dir` and
  `macos_slots`. `macos_slots` is the number of the two macOS guest slots that
  the app may hold: 0, 1 or 2 (ADR-0022). Policy functions come with the
  deliveries that need them.
- The app evaluates the file when it starts, and a restart of the app reloads it.
  If the file fails, the app runs with its built-in defaults and reports the
  error. The Host CLI refuses to run with a file that fails.
- In the Host CLI, a flag beats an environment variable, which beats the file,
  which beats the built-in default. So each variable keeps its meaning.
- Each request to the app names the directories that the Host CLI resolved. The
  app refuses a request whose directories differ from its own, and the error
  names both and the remedy.
- When the app starts and finds no file at the default place, it writes one,
  then evaluates it. The file holds comments that name each setting, and an
  empty map, so it changes no setting. The app never replaces an entry at the
  path. The Host CLI never writes the file.

## Considered options

- **A file for the app alone.** The Host CLI would keep only its flags and its
  variables, and one process would own the file. The owner chose a file for both
  processes. One shared evaluator keeps the two readings of one file the same.
- **A declarative file, such as TOML.** Rejected. The owner asked for code, and
  the policy functions of #5 need code.
- **The app honours the directories of each request.** Rejected. After a
  restart, adoption would have to find every directory that any request ever
  named.
- **The app ignores `XDG_STATE_HOME` and `XDG_DATA_HOME`.** Rejected. A variable
  that a user sets would then have no effect on macOS, which changes its
  meaning.
- **The app reads the environment of the first client.** Rejected. Docker
  Desktop and VirtualBox both leaked the environment of their first starter into
  a long-lived process.
- **The Host CLI writes the missing file.** Rejected. The CLI also runs inside
  guests, in tests and in agent sandboxes that may refuse the write. A file that
  a command writes after the app started also makes the app report a changed
  file and ask for a restart.
- **The formula's `post_install` writes the file.** Rejected. Homebrew runs that
  step in a sandbox that allows no write to `~/.config`.
- **The cask writes the file in a `postflight`.** Rejected. An install from the
  archive or from the source would get no file, and only a live install could
  test the step.
- **`doctor`, or a new command, writes the file.** Rejected. `doctor` changes
  nothing, and a new command adds to the CLI surface. Either one needs a person
  who knows about the step.

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
  the file. In normal use only the Host CLI reads the variable, because the
  login item gives the app no shell environment.
- On a macOS host the file exists after the first start of the app, and the app
  writes it again after a person deletes it. A Linux or Windows host has no app,
  so it gets no written file. A test-mode run of the app writes nothing.
- The file stays optional. A missing file and the written file give the same
  settings, so each command behaves the same with either.
- A Host CLI or an app of an older release reads a new key, such as
  `macos_slots`, as an unknown key, and it fails. So the written file names a
  new key only in a comment.
