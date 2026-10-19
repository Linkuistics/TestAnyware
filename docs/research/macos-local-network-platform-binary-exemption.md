# macOS Local Network privacy and platform binaries

Evidence for ADR-0019. Measured on 2026-10-19, macOS 27.0.1, tart 2.40.1, from a
Claude/Grove harness session. The harness chain is `claude`, `ravel`, `grove`,
`zsh`, `herdr`, and `herdr` is a login service.

## The question

Why does the Host CLI get `No route to host` (errno 65) from a macOS guest at
`192.168.64.x:8648`, while the guest's agent answers `/health` with 200?

## The measurements

The target was a running guest whose agent was healthy. Each tool ran from the
same shell at the same moment.

| Executable | Signature | Result |
|---|---|---|
| `/usr/bin/curl` | platform (`Platform identifier=26`) | HTTP 200 |
| `/usr/bin/nc` | platform | connected |
| `/usr/bin/perl` | platform | connected |
| Homebrew `curl` | ad hoc | errno 65 |
| Xcode `python3` (what `/usr/bin/python3` execs) | Apple team ID, not platform | errno 65 |
| `testanyware` (debug build and installed) | ad hoc | errno 65 |
| a copy of `/usr/bin/nc`, re-signed ad hoc | ad hoc | errno 65 |

The last row is the control. It is the same code as the working `nc`, and only the
signature changed. The same copy connects to a loopback listener and to the host's
own LAN address, so the copy is not broken. A copy that keeps the platform
signature dies with exit 137 (the kernel kills it), so the copy must be re-signed
to run.

`/usr/bin/python3` is itself a platform-signed shim. It execs the Xcode
interpreter, and the interpreter is what connects. The shim looked like a
counter-example until the target's signature was read.

## What it shows

The property is whether the connecting executable is a platform binary. The
parent chain is not the cause: every process above shares one `zsh` ancestry. The
behaviour fits macOS Local Network privacy. It is not proven that a particular
terminal app holds the permission, because this harness cannot test one.

## Reproduce

With a running guest at `$IP`:

```sh
/usr/bin/nc -v -z -G 5 "$IP" 8648            # platform: connects
cp /usr/bin/nc ./nc-adhoc && codesign --force -s - ./nc-adhoc
./nc-adhoc -v -z -G 5 "$IP" 8648             # same code, ad hoc: No route to host
```

Run both from the shell that runs `testanyware`. If the first connects and the
second fails, the system denies that shell.

## What the product does about it

`vm start` asks `/usr/bin/nc` after five unreachable probes and fails fast with
`local_network_denied` when it connects. ADR-0019 records the decision and the
relay it rejected.
