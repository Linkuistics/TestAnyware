# 19. The TestAnyware app holds the Local Network grant, and a denial is diagnosed where the connection is made

Date: 2026-10-20

## Status

Accepted. First recorded in grove `gh-issue-3`, leaf `reliable-macos-start-k4`.
Reworked in grove `gh-issue-8`, leaf `vm-broker-design-k3`. The evidence is
`docs/research/macos-local-network-platform-binary-exemption.md` and
`docs/research/testanyware-app-local-network-grant.md`. The area's spec is
`docs/specs/testanyware-app.md`.

## Context

Issue #3 reported that `vm start --platform macos` waited its whole agent
window and then failed, although the guest's agent answered `/health`. The
probe's error was `No route to host` (errno 65) every time.

On a macOS 27 host, the system refuses a process a route to the guest subnet
(`192.168.64.0/24`). Only an exempt process, or one that the user allows, gets
it.
Platform-signed tools such as `/usr/bin/nc` are exempt. Every other executable
can fail, the Host CLI included. A copy of `nc` that is re-signed ad hoc fails,
so the signature decides it. Errno 65 alone is ambiguous: a booting guest that
has not answered ARP gives the same errno.

An agent harness such as herdr runs under a login service. Nothing in that
context is an app that the user can allow, so there the refusal is permanent.
The user's view is to give the context its access, not to engineer around the
denial in product code.

Issue #8 gives that access a holder. On 2026-10-20, a signed app that `open`
started from a herdr shell received the alert, and the user allowed it. The app
then connected to the guest, while the shell stayed denied.

## Decision

**On a macOS host, the user grants Local Network access to the TestAnyware app
once. Clients reach the app on a Unix socket, which Local Network privacy does
not cover. The process that makes a guest connection diagnoses its denial, and
the remedy names the process that needs the grant.**

The TestAnyware app makes every guest connection on a macOS host, except in
golden creation (ADR-0004). So it diagnoses each denial:

- **In the health wait of `vm start`**, after five `host_unreachable` probes in
  a row, the app asks the platform `/usr/bin/nc` to connect to the same
  endpoint. Success proves that the refusal is about the app. The start then
  ends with `VM_BOOT_TIMEOUT`, cause `local_network_denied`. If `nc` does not
  connect, the wait goes on and asks again after each further five.
- **For a guest stream**, the app retries a `host_unreachable` dial for 5 s at
  most, and then asks `nc` in the same way. The error is `CONNECTION_REFUSED`,
  with cause `local_network_denied` or `host_unreachable`. A new executable UUID
  gave a refusal of about 2 s in the measurements, and the retry covers it.
- **The remedy** says to allow TestAnyware in System Settings, Privacy &
  Security, Local Network. A person does this once.

A QEMU guest is on loopback and never asks. Golden creation keeps the direct
path, so it needs a context with Local Network access, such as Terminal.

The decision is a function of what each wait observed. The `nc` call sits behind
an injected oracle, so unit tests drive the rules with scripted probe results.

## Considered options

- **Relay every agent call through `/usr/bin/nc`**, or through a platform `perl`
  forwarder. Rejected. It hides a permission problem that the user can fix, and
  it adds a process to every command. It also depends on tools that Apple may
  remove.
- **Report errno 65 and leave the reader to guess.** Rejected. That was the
  state that made issue #3 look like a flaky agent.
- **Fail on the first `host_unreachable`.** Rejected. A booting guest gives it
  before its network is up.
- **Give the grant to the Host CLI itself.** Rejected. Under herdr the CLI is its
  own responsible process, and it has no bundle for System Settings to list. Its
  executable also gets a new UUID with each build, and a new UUID makes macOS
  evaluate the grant again.
- **A `launchd` daemon or a root helper.** TN3179 exempts both. Rejected: the
  user chose an app that the user grants, in grove `gh-issue-8`.

## Consequences

- After a person allows the alert once, an agent under herdr reaches each guest
  through the app. ADR-0020 keeps that grant across upgrades.
- Before the grant, a command fails about 8 s after the guest's address appears,
  and the error names the app. The command does not wait for a person.
- The diagnosis spawns `nc` only on a failing connection. It needs no new
  dependency.
