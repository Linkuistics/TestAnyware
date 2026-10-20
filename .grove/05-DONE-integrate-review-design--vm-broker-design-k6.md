# vm-broker-design-k6

**Integrates:** vm-broker-design-k5

## Goal

Triage the findings of `vm-broker-design-k5` against the agreed design and the
owner's requirements. Apply the findings that hold before planning uses the
design.

## Context

- Read the findings from the committed task record of `vm-broker-design-k5`.
  Find its commit by that handle. Do not treat its proposed corrections as
  requirements.
- The producer is `vm-broker-design-k3`. Its decision log and `plan-k1` hold
  the owner's choices and their rationale.
- The artifact is `docs/specs/testanyware-app.md` and the ADRs it cites.

## Done when

- Each review finding has an evidenced disposition: apply, reject, or record
  an explicit accepted trade-off.
- Accepted changes are reconciled across the design, its ADRs and their
  citations, with appropriate verification and any necessary owner decision.
- `vm-broker-increments-k4` can plan from the resulting coherent design.

## Notes

Keep this a design integration. Implementation and release work belong to the
increments that planning cuts. The owner asked for focus and small increments.

## Decisions (running log)

**Each of the eight findings holds when checked against its source.** The
findings come from commit `4fa6d883`. Each claim was checked against the code,
the lockfile, the vendored reqwest 0.12.28 source, the installed swtpm 0.10.2
and Apple's TN3178 and TN3179. No finding is noise. F2 to F8 are contract gaps
that this leaf corrects. F1 reopens an owner choice, so the owner decided it.

**F1: each release keeps its natural UUID, and the owner accepts a new grant
after an update.** TN3178 says: "Each distinct Mach-O image must have its own
unique build UUID." It also says that no Apple command sets a UUID after the
link. TN3179 says that the main executable's UUID must be unique. So the pinned
UUID broke a rule that Apple states.

The owner chose neither the pinned UUID nor the stable launcher. Their words:

<!-- tc: off because this is the human's message, quoted verbatim -->

> I don't think it's too painful or annoying to have to re-authorise an
> application on update. I already have to do that for other apps and it's not
> a problem.

> However, I'm happy to have a notarisation or signing from Apple for this app
> through developer connection such that it keeps a consistent identity because
> I think that also persists or continues authorisations.

<!-- tc: on -->

The design already signs the app with a Developer ID and notarises it. That
identity keeps one Local Network row in System Settings, as the probe showed.
It also lets Homebrew carry the approval of the first start.

But that identity did not stop a new evaluation of the grant. A new UUID gave a
refusal of about 2 s three times in three, and one alert in three. So an
upgrade can ask a person again. The root brief and the planning leaf now say
so.

**F2: the agent client speaks HTTP/1.1 through hyper on any stream.** reqwest's
connector must return its own `Conn` type, whose fields are private to a sealed
module. So no code outside reqwest can give it a guest stream. hyper 1.9 is
already in the lockfile, and its HTTP/1 client handshake takes any async stream.
The agent client uses it on every host, so the protocol code stays the same on
every host (ADR-0004). No in-VM agent sends a redirect, so the end of reqwest's
redirect handling changes no result.

**F3: an older QEMU guest stops through its own control sockets.** The app
looks for them in its own temporary directory, and then in `/tmp`. An older
Host CLI with no `TMPDIR` used `/tmp`. The monitor speaks HMP, which has `quit`.
The app proves the stop by the clone's disk, which no process may then hold.
When it cannot prove the stop, it deletes nothing and keeps the sidecars.

A new start gives swtpm the `terminate` option, which swtpm 0.10.2 lists. The
meta of a new VM records its session directory.

**F4: golden creation checks its directories against the app's.** It computes
the app's directories from `config.rhai` and the built-in defaults, so it needs
no running app. tart's own storage follows `TART_HOME`, which the tart
executable reads. The app never sees a client's `TART_HOME`. The spec discloses
this as a limit, and adds no check for it.

**F5: a build mismatch gives a notice on stderr.** The `vm start --wait`
amendment of the CLI contract already writes to stderr in both output modes. So
the notice leaves stdout and the JSON envelope unchanged.

**F6: only a dial to an attached network changes the grant observation.** A
dial to loopback, or to an address beyond the host's attached networks, leaves
the observation as it was.

**F7: `doctor` reports what the other commands refuse.** It keeps a
configuration error as the failure of its `config` check. The app answers
`GET /v1/status` for any protocol range and any directories.

**F8: the CLI contract joins the documents that the implementation corrects.**
A sweep for "Local Network" outside the design set found one rule that tells
the user to run the CLI from an app with Local Network access. That rule is the
contract's amendment of 2026-10-19. The code's remedy string is the
implementation's own change.
