# 19. The CLI diagnoses a Local Network denial with a platform tool and never routes around it

Date: 2026-10-19

## Status

Accepted (grove `gh-issue-3`, leaf `reliable-macos-start-k4`). The evidence is
`docs/research/macos-local-network-platform-binary-exemption.md`.

## Context

Issue #3 reported that `vm start --platform macos` waited its whole agent
window and then failed, although the guest's agent answered `/health`. The
probe's error was `No route to host` (errno 65) every time.

On a macOS 27 host, the system refuses a process a route to the guest subnet
(`192.168.64.0/24`) unless it exempts that process. Platform-signed tools are
exempt. `/usr/bin/curl`, `/usr/bin/nc` and `/usr/bin/perl` connect. Every other
executable fails, including the Host CLI, Homebrew `curl` and the Xcode
`python3`. A copy of `nc` that is re-signed ad hoc fails too, so the signature
decides it.

Terminal apps may hold the Local Network permission. An agent harness runs under a
background login service, so it has no app that the user can allow. For such a process
the refusal is permanent.

Errno 65 alone is ambiguous. A booting guest that has not answered ARP gives the
same errno.

## Decision

**`vm start` diagnoses the denial and fails fast. It does not route agent calls
around it.**

After five `host_unreachable` probes in a row, the agent wait asks the platform
`/usr/bin/nc` to connect to the same endpoint. The platform tool is exempt, so
its success proves the refusal is about this process. The start then ends with
`VM_BOOT_TIMEOUT`, cause `local_network_denied`, and a remedy that names the
permission. If `nc` does not connect, the wait carries on and asks again after
each further five. A QEMU guest is on loopback and never asks.

The decision is a function of what the wait observed (`wait_with_probe` in
`health.rs`). The `nc` call sits behind an injected oracle, so the rules are
unit-tested with scripted probe results.

## Considered options

- **Relay every agent call through `/usr/bin/nc`**, or a platform `perl`
  forwarder, so the CLI works in a denied context. Rejected. It would work.
  It hides a permission problem that the user can fix. It adds a process to
  every command. It also depends on tools that Apple may remove (`perl` is
  already deprecated). The user's standing position is to give the harness its
  access, not to engineer around the denial in product code.
- **Report errno 65 and leave the reader to guess.** Rejected. That was the state
  that made issue #3 look like a flaky agent.
- **Fail on the first `host_unreachable`.** Rejected. A booting guest gives it
  before its network is up, and the wait would fail healthy starts.
- **Sign the Host CLI with a Local Network usage description and prompt.**
  Not pursued here. It needs a stable signing identity and a human to answer a
  dialog, and a harness cannot answer one.

## Consequences

A denied start fails in about 8 s after the guest's address appears, not after
118 s, and the error says what to change. The CLI still cannot start a macOS
guest from a context that has no Local Network access. The fix for that
context is in the environment.

The diagnosis spawns `nc` only on a failing start. It needs no new dependency.
