# vm-broker-prior-art-k2

## Goal

Survey the prior art for one design. A CLI reaches local VMs only through a
long-lived, signed macOS app, and the app holds the permissions that the user
grants. Write the survey to `docs/research/vm-broker-prior-art-a.md`. Structure
it around the six questions below.

## Context

- The audience is `vm-broker-design-k3`. That leaf writes the spec of the VM
  broker and reworks ADR-0004 and ADR-0019 from this survey.
- The decision log of `plan-k1` holds the requirements and their reasons.
- `docs/research/macos-local-network-platform-binary-exemption.md` and Apple
  TN3179 hold what this repo already knows about Local Network privacy.

## Questions the design needs answered

1. **Version skew.** After an upgrade, a new CLI can meet an old app that still
   runs. How do CLI and daemon pairs find and settle a mismatch? Candidates:
   the API version negotiation of the Docker CLI, and the Tailscale CLI with its
   macOS app.
2. **Start on demand.** How does a CLI start its app when the app is not
   running? What races occur when two clients start it at the same time?
   Candidates: VBoxSVC of VirtualBox, Docker Desktop, and the 1Password CLI
   with its app.
3. **Grant persistence.** What makes macOS drop a privacy grant across an
   update of an app, the Local Network grant in particular? Candidates: a new
   path, a new signature, a new version, and a Homebrew formula against a cask.
   Find reports of lost grants.
4. **Access control.** How do these tools keep a local socket or port to the
   user who owns it? What went wrong where a tool did not?
5. **Guest traffic through a broker.** How do Cirrus Labs Orchard, Lima and UTM
   carry agent, SSH or VNC traffic to a guest? Orchard forwards ports and VNC
   through its controller. Lima uses a host agent, and UTM offers `utmctl`.
   What failed after real use?
6. **Adoption.** When a broker restarts, how does it adopt VMs or processes that
   outlived it? How does it guard against PID reuse?

## Done when

- `docs/research/vm-broker-prior-art-a.md` answers each question above, or
  records that the search found no primary source for it.
- Each claim about a failure cites a primary source: an issue, a thread or a
  post-mortem.
- Each system in the survey has a walk-away check.

## Notes

- Search for what went wrong after years of real use, not for feature lists.
- The fallback to beat is a plain Unix socket in a directory with mode 0700,
  with a version check on each connection.
- Stay inside the six questions. The human asked for focus and no rabbit holes.

## Decisions (running log)

**The configuration requirement goes to the design leaf.** During this session the human
asked that the configuration be code, as in Ravel, but in Rhai. It is a design input,
not a question of this survey. So the Notes of `vm-broker-design-k3` now quote it and
list the points that its spec settles.

**The survey names two routes that macOS offers, and recommends neither.** TN3179
exempts `launchd` daemons and root from Local Network privacy. Since macOS 15.5, two
system defaults can also exempt a network. The human chose the app in `plan-k1`. The
design still needs to know of both routes, so section 3 of the survey states them.
