# Prior art for a CLI that reaches VMs through a signed local app

**Status:** survey, 2026-10-20. Leaf `vm-broker-prior-art-k2` of the issue #8 grove.

**Audience:** leaf `vm-broker-design-k3`, which writes the spec of the VM broker and
reworks ADR-0004 and ADR-0019.

**Bottom line:** the prior art supports a signed app that holds the Local Network grant.
It does not support a channel to that app that is more complex than the fallback in the
brief. A Unix socket in a 0700 directory, with a peer uid check and a version check on
each connection, held up best in real use. Two risks need the design's attention first:

- macOS can lose the grant when the path of the app changes on an upgrade. TestAnyware
  ships today as a Homebrew formula, whose path changes with each version.
- A broker that forwards guest traffic fails most often by a silent stall of every
  stream, while its process stays alive.

## How to read this survey

Six research passes ran on 2026-10-20, one for each question of the brief. Each pass
fetched every page that it cites. I checked a sample of the citations again, through the
GitHub API, the CVE records and Apple's own copy of TN3179. The last section lists them.

A primary source is an issue, a forum thread, a security bulletin or CVE record, release
notes, official documentation, a commit or source code. A user report is primary, but it
carries no statement from the vendor. The survey says so where it matters.

Each date is the date of the cited post, commit or release. For a page with no date, the
survey gives no date. Quotes from source code, commits and GitHub issues are verbatim.
Quotes from forum and documentation pages came through a fetch tool that summarises, so
their wording can differ in small details.

Each section ends with what the evidence means for the design. It compares the design
with the fallback of the brief. That fallback is a plain Unix socket in a directory with
mode 0700, with a version check on each connection.

Three repositories moved. GitHub redirects `cirruslabs/tart` to `openai/tart` and
`cirruslabs/orchard` to `openai/orchard`. Podman issues now live under
`podman-container-tools/podman`. The survey cites the current names.

## The answers in short

| Question | What the prior art shows | Against the fallback |
|---|---|---|
| 1. Version skew | Pairs that compare build strings only warn, and users stay on the old daemon. Docker compares a range, and it broke clients when it raised its floor. macOS does not restart a running helper after an update. | Same handshake in both. The broker has a longer skew window, because the CLI cannot replace a running app. |
| 2. Start on demand | Tools connect, take a lock, connect again, and only then start the server. `open` needs a GUI session. A single-instance check in an app left no instance running. | Same lock in both. The fallback can start from SSH with no GUI login. The app cannot. |
| 3. Grant persistence | macOS ties the Local Network grant to the code signature, the main executable UUID and, in practice, the path. A new path asks the user again. | No difference. The app holds the grant in both designs, and the path rule binds both. |
| 4. Access control | A Unix socket with a peer uid check held up. Localhost TCP listeners failed again and again. A code-signature check of the CLI adds little. | The fallback with a uid check equals the app at the boundary of one user. |
| 5. Guest traffic | Byte forwarders stall silently, lose data in large transfers and leave stale sockets. Multiplexed streams failed in ways that one connection per stream avoids. | One connection per stream is the fallback's natural shape. The broker adds one relay hop. |
| 6. Adoption | Tools that find a survivor through an object that dies with it adopt reliably. Tools that trust a stored pid killed or kept the wrong process. | No difference. Both need the same identity record. |

## 1. Version skew

**The question.** After an upgrade, a new CLI can meet an old app that still runs. How do
CLI and daemon pairs find and settle a mismatch?

### What the pairs do

**Docker compares an API version range on each request.** The client pings once and
negotiates down to the daemon's version. The daemon rejects a version below its floor or
above its ceiling
([version.go](https://github.com/moby/moby/blob/master/daemon/server/middleware/version.go)).
`DOCKER_API_VERSION` turns negotiation off
([Engine API docs](https://docs.docker.com/reference/api/engine/)).

**Tailscale and GnuPG compare build strings and only warn.** Every LocalAPI response
carries the daemon's version in a header. The CLI prints a warning once and goes on
([cli.go](https://github.com/tailscale/tailscale/blob/main/cmd/tailscale/cli/cli.go)).
gpg warns only when the agent is older, and it names `gpgconf --kill all` as the remedy
([asshelp.c](https://github.com/gpg/gnupg/blob/master/common/asshelp.c)).

**Bazel and Flow compare a build identity and replace the old server.** Bazel kills a
server of another version and starts its own
([client-server docs](https://bazel.build/run/client-server)). Since 0.98.0, Flow keeps
whichever side is newer
([Changelog](https://github.com/facebook/flow/blob/main/Changelog.md), 2019-04-26).

**Gradle and Lima do not settle a mismatch.** A Gradle client only talks to a daemon of
its own version, and it starts a new one
([Gradle daemon docs](https://docs.gradle.org/current/userguide/gradle_daemon.html)). The
Lima host agent stays at the version that started the instance. Its client has a
`TODO(AkihiroSuda): negotiate the version`
([client.go](https://github.com/lima-vm/lima/blob/master/pkg/hostagent/api/client/client.go)).

**VirtualBox prevents skew at install time.** Its Debian package refuses to upgrade while
VBoxSVC runs
([preinst](https://github.com/VirtualBox/virtualbox/blob/main/src/VBox/Installer/linux/debian/preinst)).

**Koine, a sibling project of this owner, demands an exact contract string.** Its
`endpoint.json` and its API both state a `contractVersion`. A client that does not target
that exact string must report an incompatible service, not an unavailable one
(`docs/client-guide.md` in [Koine](https://github.com/Linkuistics/Koine), commit
`a5eac76d`).

### What went wrong

- **Docker raised its floor, and clients broke the next day.** Engine 29.0.0 required API
  1.44 ([release notes](https://docs.docker.com/engine/release-notes/29/), 2025-11-10).
  Traefik, Portainer, Testcontainers in CI and others failed with "client version 1.24 is
  too old"
  ([Docker forum](https://forums.docker.com/t/docker-29-increased-minimum-api-version-breaks-traefik-reverse-proxy/150384),
  2025-11-11). Engine 29.3.0 lowered the floor to 1.40 again (same release notes,
  2026-03-05).
- **A pinned client did not negotiate.** Traefik had hard-coded API 1.24
  ([traefik PR 12256](https://github.com/traefik/traefik/pull/12256), 2025-11-11).
- **A discarded negotiation error gave a misleading message.** The 29.0 CLI told users of
  an older daemon "client version 1.52 is too new". Kubernetes release CI pinned the CLI
  back to 24.0
  ([kubernetes/release PR 4183](https://github.com/kubernetes/release/pull/4183),
  2025-11-13).
- **A warning leaves users on the old daemon.** On Windows the Tailscale service stayed on
  1.20.0 while the UI ran 1.20.1, and the remedy was a restart or a reboot
  ([Tailscale forum](https://forum.tailscale.com/t/client-version-1-20-1-tailscaled-server-version-1-20-0/1609),
  2022-01-15). A systemd socket unit kept starting an old gpg-agent binary after the user
  killed it
  ([gnupg-users](https://lists.gnupg.org/pipermail/gnupg-users/2019-July/062346.html),
  2019-07-11).
- **A string comparison gave false alarms.** Tailscale warned that "1.96.5" differs from
  "1.96.5-t4ee448d3a-g74ffbefc2"
  ([tailscale issue 19708](https://github.com/tailscale/tailscale/issues/19708),
  2026-05-12). A Podman maintainer raised the floor by mistake, and it rejected Podman's own release
  candidates
  ([podman issue 10578](https://github.com/podman-container-tools/podman/issues/10578),
  2021-06-06).
- **A kill on mismatch killed a server in use.** An inner Bazel of another version tried
  to kill the outer server that ran it, and deadlocked
  ([setup-bazel issue 108](https://github.com/bazel-contrib/setup-bazel/issues/108),
  2025-11-19). An older Bazel commit connected to a server after it sent that server
  SIGKILL
  ([commit 3e66b6e](https://bazel.googlesource.com/bazel/+/3e66b6e8163bdf49a65a02d422d109ffba170d26),
  2015-10-22).
- **After `brew upgrade lima`, the instance broke.** The error was "vz driver is running
  but host agent is not", and the fix was `limactl stop -f`
  ([lima issue 3081](https://github.com/lima-vm/lima/issues/3081), 2025-01-03).
- **macOS does not restart a running helper after an app update.** Apple DTS: "Basically,
  no, the system doesn't do that." The old helper kept running from deleted files
  ([Apple forum 763268](https://developer.apple.com/forums/thread/763268), September
  2024).
- **A Tailscale network extension stayed old after an auto-update.** The app said to
  reboot, and a reboot did not help. The issue is open
  ([tailscale issue 21468](https://github.com/tailscale/tailscale/issues/21468),
  2026-09-24).
- **SMAppService kept starting the version that registered first**, whichever version
  registered again later (FB13570494,
  [Apple forum 745412](https://developer.apple.com/forums/thread/745412), January 2024).

### For the design

1. **Compare a protocol range, not build strings.** Each side states a minimum and a
   maximum protocol version, and the pair uses the overlap. String and semver compares
   gave false alarms and hard failures. The fallback needs the same handshake, so this
   point favours neither design.
2. **Refuse with a remedy, and never only warn.** The error names the running version,
   the range that the CLI accepts and the exact command that restarts the app. Both
   designs can do this.
3. **Decide who replaces the old side, and keep the newer side.** With the fallback, the
   CLI can stop its own daemon and start a newer one. With the broker, the CLI must ask a
   running signed app to quit, and macOS will not restart it. This is a real cost of the
   broker.
4. **Expect a longer skew window, and keep the app's floor low and stable.** A Homebrew
   upgrade changes the CLI at once. The app changes only when it restarts. Docker's floor
   raise and its reversal show the cost of a floor that moves.

## 2. Start on demand

**The question.** How does a CLI start its app when the app is not running? What races
occur when two clients start it at the same time?

### What the tools do

**Most daemon CLIs connect first, start the server when the connect fails, then poll to a
deadline.** VirtualBox, gpg, tmux, Bazel, Gradle and sccache all do this. VirtualBox
waits for a five-byte `READY` on a pipe, then polls every 100 ms for up to 30 s
([server_module.cpp](https://github.com/VirtualBox/virtualbox/blob/main/src/VBox/Main/src-server/xpcom/server_module.cpp)).

**tmux closes the spawn race with a lock and a second connect.** It connects, takes
`flock` on `<socket>.lock`, and connects again before it starts a server. Its comment
says why: "another client could have taken the lock, started the server and released the
lock between our connect() and flock()"
([client.c](https://github.com/tmux/tmux/blob/master/client.c)). gpg uses a `link()`
lock in the same place
([dotlock.c](https://github.com/gpg/gnupg/blob/master/common/dotlock.c)).

**The server refuses to be the second.** A second VBoxSVC exits when it cannot register
its name
([server.cpp](https://github.com/VirtualBox/virtualbox/blob/main/src/VBox/Main/src-server/xpcom/server.cpp)).
The OpenSSH master binds a temporary path and links it into place. The loser falls back
to a connection without multiplexing
([mux.c](https://github.com/openssh/openssh-portable/blob/master/mux.c)).

**The Docker CLI does not start Docker Desktop.** It fails to connect, and the user or
`docker desktop start` must start the app
([moby issue 27102](https://github.com/moby/moby/issues/27102), maintainer comment
2021-08-30). By default `docker desktop start` waits with no timeout
([CLI reference](https://docs.docker.com/reference/cli/docker/desktop/start/)).

**The 1Password docs give the start to the user.** They tell the user to open and unlock
the app and to allow it in the background
([app integration](https://www.1password.dev/cli/app-integration/)). No official source
says that `op` starts the app. Koine also states that a client never starts it.

**`launchd` can hold the socket and start the job on the first connect.** `launchd` holds the
client's request until the job starts and answers
([Apple, Creating `launchd` Jobs](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html),
updated 2016-09-13). `launchd` throttles a respawn to once in 10 s by default
(`launchd.plist(5)`).

**LaunchServices keeps one instance for each bundle id in one GUI session.** By default
NSWorkspace "open[s] the already running app when present"
([createsNewApplicationInstance](https://developer.apple.com/documentation/appkit/nsworkspace/openconfiguration/createsnewapplicationinstance)).
`open -n` starts a second instance anyway (`open(1)`). The guard covers only starts that
go through `open` or NSWorkspace.

### What went wrong

- **A single-instance check left no instance.** Two login items started Docker Desktop
  twice. Each copy saw the other, and neither stayed
  ([for-mac issue 4832](https://github.com/docker/for-mac/issues/4832), 2020-08-05,
  user report).
- **`docker desktop start` hangs at random.** "Starting the application from the UI vs
  the cli works 100% of the time"
  ([for-mac issue 7709](https://github.com/docker/for-mac/issues/7709), 2025-06-18).
- **The starter's environment leaked into the app.** Docker Desktop 4.70.0 fixed an
  inherited CLI plugin variable that broke one of its daemons
  ([release notes](https://docs.docker.com/desktop/release-notes/), 2026-04-20). The
  first starter of VBoxSVC also sets its `VBOX_USER_HOME`
  ([VirtualBox forum](https://forums.virtualbox.org/viewtopic.php?t=108421),
  2023-01-25).
- **From SSH with no GUI login, an app start fails or does nothing.** `open` failed with
  -10810, and Docker staff said that the app cannot start without the GUI
  ([for-mac issue 3567](https://github.com/docker/for-mac/issues/3567), 2019-03-06). In
  another case `open` exited 0 and did nothing
  ([for-mac issue 6504](https://github.com/docker/for-mac/issues/6504), 2022-10-01).
  Apple DTS says that `open` "isn't rated for use in a non-GUI context"
  ([Apple forum 756081](https://developer.apple.com/forums/thread/756081), May 2024).
- **Fixed start deadlines fail under load.** gpg tries five times at one-second
  intervals, and its developer calls the start "an inherent race"
  ([pkg-gnupg-maint](https://alioth-lists.debian.net/pipermail/pkg-gnupg-maint/2017-August/005877.html),
  2017-08-17). FreeBSD raised the sccache start timeout from 10 s to 30 s and called it "a
  workaround, not a real fix"
  ([freebsd-ports commit 47e7d26](https://gitlab.com/FreeBSD/freebsd-ports/commit/47e7d26def939bee65ef9b4ddb39fdda01fb19e9),
  2024-06-07).
- **An idle exit raced the next request.** Bazel's server could time out between `Ping()`
  and `Run()`. The fix was "not an principled fix"
  ([commit 949c876](https://bazel.googlesource.com/bazel/+/949c876fadae5a23635756265447082b9cdf44c7),
  2016-07-08). VirtualBox loops back when the server exits between lookup and request
  (server_module.cpp, above).
- **Socket activation kept a registration for the old binary.** After an upgrade, Watchman
  commands hung, and Watchman stopped using socket activation
  ([watchman issue 358](https://github.com/facebook/watchman/issues/358), 2016-09-13 to
  2016-11-01).

### What macOS allows from a context with no app

Apple DTS endorses one route to GUI code from an SSH session. Load an agent into the
`gui/<uid>` domain, and start the program from that agent
([Apple forum 765060](https://developer.apple.com/forums/thread/765060), October 2024).
`launchd` loads Aqua agents only after a GUI login (`launchctl(1)`). The user can revoke an
SMAppService agent in System Settings, and its status then reads `requiresApproval`
([SMAppService docs](https://developer.apple.com/documentation/servicemanagement/smappservice/status-swift.enum/requiresapproval)).

The herdr harness runs under a login service. The design leaf
lists "`open`, run from a herdr session, starts the app" as a fact to prove. No source in
this survey covers that exact context.

### For the design

1. **Arbitrate the start with a lock, not with LaunchServices.** Connect, take `flock`
   on a lock file beside the socket, connect again, and start only while holding the lock.
   Inside the app, refuse a second server at bind time. The code is the same with the
   fallback.
2. **Use the connect and its version check as the readiness test.** An app that
   LaunchServices starts receives no pipe from the CLI. This is my inference, and nobody has
   tested it. The CLI holds the lock and polls the socket to a deadline that it states.
3. **Do not let the broker quit on idle.** If it ever quits, it unlinks its socket before
   it refuses work, and the CLI retries once.
4. **Report the GUI session as a precondition.** The CLI needs separate errors for "no GUI
   session", "app not approved" and "app did not answer". The fallback can start from SSH
   with no GUI login. It then runs in the context of its first client.
5. **Start the app with a fixed environment.** Per-call settings travel in the protocol,
   never in the environment of the first starter.

## 3. Grant persistence

**The question.** What makes macOS drop a privacy grant when an app updates? The Local
Network grant matters most.

### What Apple says

TN3179 names two inputs
([TN3179](https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy),
read 2026-10-20):

- "Local network privacy tracks the identity of your program using its code signature."
  It adds that unsigned and ad hoc code makes that tracking unreliable.
- "Local network privacy uses your main executable UUID as part of its implementation."
  A missing or shared UUID makes it "behave weirdly".

An Apple DTS engineer lists at least four factors. They are the bundle id, the path of the
app, the designated requirement and the main executable UUID
([Apple forum 766270](https://developer.apple.com/forums/thread/766270), October 2024). The
engineer also says that Local Network is not a TCC privilege, so `tccutil` does not reset it
([Apple forum 757949](https://developer.apple.com/forums/thread/757949), June 2024).

TN3179 also states these facts:

- macOS keeps Local Network state for each user account.
- When an app spawns a helper, the app is the responsible code, and the choice covers the
  whole app.
- A `launchd` agent that SMAppService did not install names its app with
  `AssociatedBundleIdentifiers`.
- macOS fails to show the alert for a process with a very short life (FB16131937).
- On macOS 27.2 and later, a minus (−) button in System Settings resets a row. Before 27.2,
  "there's no way to reset your program's Local Network privilege" (FB14944392).

This Mac runs macOS 27.0.1, so it has no reset.

### What went wrong

- **A new path drops the grant, even with a stable Developer ID identity.** Each Claude
  Code update installs its helper app at a new versioned path. Each time, the user loses
  the grant ([claude-code issue 90412](https://github.com/anthropics/claude-code/issues/90412),
  2026-08-28, open, user reports). The store held one rule for each versioned path. A
  move to a new path made `nehelper` log "Found new path … allowing a new prompt"
  ([comment](https://github.com/anthropics/claude-code/issues/90412#issuecomment-5859659591),
  2026-09-27).
- **A Homebrew formula breaks the grant on each upgrade.** Apple's `container` formula put
  its helper at a new Cellar path for each version. System Settings showed one row for
  each version, and the toggle changed only the first row
  ([homebrew-core issue 312547](https://github.com/Homebrew/homebrew-core/issues/312547),
  2026-09-21, open).
- **Two notarised builds with the same bundle id and designated requirement prompted
  separately.** Their paths and UUIDs differed. The developer filed FB15568200
  (Apple forum 766270, October 2024). TN3179 now lists that bug.
- **A shared Mach-O UUID merged two apps.** Two bundles of one vendor shared one
  executable UUID, and macOS logged "found bundle id … by UUID" for the wrong one
  ([claude-code issue 95444](https://github.com/anthropics/claude-code/issues/95444),
  2026-09-18, user report).
- **After an update, macOS can deny with no prompt and no new row.** Every switch in
  System Settings was already on
  ([comment](https://github.com/anthropics/claude-code/issues/90412#issuecomment-6142964381),
  2026-10-13).
- **A background-only app never receives the prompt.** The denial is silent, and no row
  appears ([claude-code issue 91373](https://github.com/anthropics/claude-code/issues/91373),
  2026-09-02, user report).
- **macOS updates reset grants.** An update to 15.2 cleared all Local Network grants
  ([Apple forum 769037](https://developer.apple.com/forums/thread/769037), December 2024,
  user report). TN3179 says that macOS 15.1 fixed "a number of" Local Network bugs.
- **After a reboot, an IPC failure can deny access and cache the denial.** Apple DTS
  describes how the system "fails secure"
  ([Apple forum 809211](https://developer.apple.com/forums/thread/809211), February 2026).

TCC grants such as Accessibility and Screen Recording behave differently. TCC stores the
designated requirement and checks each new version against it, so a new version with the
same signing identity keeps the grant
([TN3127](https://developer.apple.com/documentation/technotes/tn3127-inside-code-signing-requirements)).
An ad hoc signature ties that requirement to one build. A Homebrew formula with an ad hoc
binary lost Accessibility and Screen Recording on each `brew upgrade`
([moomux issue 304](https://github.com/erickgnclvs/moomux/issues/304), 2026-09-27).

### The candidates, one by one

| Candidate cause | Local Network grant | TCC grant |
|---|---|---|
| A new path | Drops it in every report found | Drops it for a bare executable, which TCC keys by path |
| A new code signature, same team and identifier | No report found | Kept, because the designated requirement still holds |
| A new version at the same path | No Apple statement. Reports of apps in /Applications show no loss. | Kept |
| An ad hoc signature | Unreliable, by TN3179 | Dropped on each build |
| A formula (versioned Cellar path) | Dropped, because the path changes | Dropped for the same reason |
| A cask with the `app` stanza (one path in /Applications) | The best case in the evidence | Kept |

### This repository today

TestAnyware ships as a formula. On this Mac, `/opt/homebrew/bin/testanyware` links to
`../Cellar/testanyware/2.3.0/bin/testanyware`. An app bundle inside that keg would move
on each upgrade.

The same tap already ships an app the other way. The `koine` cask installs
`/Applications/Koine.app` with the `app` stanza. That app is signed with Developer ID,
notarised and stapled, and it registers a login item with `SMAppService.mainApp`
(`Casks/koine.rb` in the `linkuistics/taps` tap).

### Two other routes that macOS offers

The survey found two routes to the guest subnet that need no app. The design should know
of them, though the human chose the app in the decision log of `plan-k1`.

- **TN3179 exempts "any daemon started by `launchd`" and "any program running as root".**
  It also says that "the exception for `launchd` daemons doesn't apply to `launchd` agents".
  This fits the herdr measurement, because herdr runs as a login service.
- **Since macOS 15.5, two system defaults can exempt a network.** These are
  `AllowedEthernetLocalNetworkAddresses` and `AllowedWiFiLocalNetworkAddresses` in the
  `com.apple.network.local-network` domain. "Every program can access that address,
  regardless of its Local Network privilege state." They need `sudo` and a restart.
  Nobody tested whether the bridge of the tart subnet counts as an Ethernet interface.

### For the design

1. **Keep the app at one path, with one identity.** Ship it through a cask `app` stanza
   into /Applications, as Koine does. Sign it with Developer ID, and never change its
   team or signing identifier. Put no code that reaches the guest subnet in a formula keg.
2. **Expect the fact to prove to fail as worded.** The design leaf lists "the grant
   survives an upgrade of the app to a new Homebrew path". Every report in this survey
   says that a new path asks again. The experiment should test an upgrade at one path.
3. **Give each Mach-O its own UUID.** Do not copy one executable into both the CLI and the
   app. `dwarfdump --uuid` shows the UUID, so each release can check it.
4. **Make the app the responsible code.** The app connects to guests itself, or it spawns
   the helpers that do. A user report says that a background-only app never sees the
   alert. TN3179 says the same of a process with a very short life. So the app is not
   background-only, and it does not exit at once after a failed first connect.
5. **Report the grant state through the channel.** macOS gives no API that reads the
   state. The app sees `localNetworkDenied` on a waiting connection (TN3179) and can return
   it to the CLI with a remedy. The fallback's version check can carry the same field.
6. **Keep one installed copy, and test the grant in a VM.** Several copies confuse Local
   Network privacy, and macOS 27.0.1 has no reset. A development build needs its own bundle
   id. A test of an upgrade can run in a guest from a snapshot.

None of these points depend on the channel, so the app and the fallback tie here.

## 4. Access control

**The question.** How do these tools keep a local socket or port to the user who owns it?
What went wrong where a tool did not?

### What held up

**A Unix socket with a peer uid check.** ssh-agent refuses a peer whose effective uid is
neither its own nor root
([ssh-agent.c](https://github.com/openssh/openssh-portable/blob/master/ssh-agent.c)).
tailscaled reads the peer's uid with `LOCAL_PEERCRED`
([ipnauth.go](https://github.com/tailscale/tailscale/blob/main/ipn/ipnauth/ipnauth.go)).
The macOS manual page says that `getpeereid` is reliable: the kernel records the peer's
credentials at `connect(2)`
([getpeereid(3)](https://developer.apple.com/library/archive/documentation/System/Conceptual/ManPages_iPhoneOS/man3/getpeereid.3.html)).

**A socket directory that the user owns, checked at each start.** GnuPG refuses a socket
directory that another user owns or that others can open
([homedir.c](https://github.com/gpg/gnupg/blob/master/common/homedir.c)). Colima
creates its `docker.sock` with mode 0600 and gives each user an instance of their own
([colima issue 1395](https://github.com/abiosoft/colima/issues/1395), 2025-09-12).

**XPC with a requirement on each message.** On macOS, the 1Password CLI and app talk over
NSXPCConnection, and each side checks the other's code signature
([app integration security](https://www.1password.dev/cli/app-integration-security/)).
`xpc_connection_set_peer_code_signing_requirement` checks the peer "every time it sends
a message"
([Apple docs](https://developer.apple.com/documentation/xpc/xpc_connection_set_peer_code_signing_requirement(_:_:))).
A named XPC listener must run as a `launchd` agent or daemon
([NSXPCListener docs](https://developer.apple.com/documentation/foundation/nsxpclistener/init(machservicename:))).

### What went wrong

- **Websites reached localhost TCP listeners.** Zoom's server on localhost let "any web
  site" control it, and it stayed after the user uninstalled the app
  ([CVE-2019-13450](https://cveawg.mitre.org/api/cve/CVE-2019-13450), 2019-07-09).
  Tailscale's Windows LocalAPI had "no Host header verification", and DNS rebinding led
  to code execution
  ([CVE-2022-41924](https://cveawg.mitre.org/api/cve/CVE-2022-41924), 2022-11-23). The
  Node.js inspector
  ([March 2018 security releases](https://nodejs.org/en/blog/vulnerability/march-2018-security-releases))
  and Ollama ([CVE-2024-28224](https://cveawg.mitre.org/api/cve/CVE-2024-28224),
  2024-04-08) had the same fault.
- **The internet reached Docker's unauthenticated port 2375.** "People keep doing this
  and getting pwned because they accidentally left it exposed to the internet"
  ([moby PR 41285](https://github.com/moby/moby/pull/41285), merged 2020-09-25). Docker
  had earlier moved its API from localhost TCP to a Unix socket, because of cross-site
  request forgery
  ([Docker security docs](https://docs.docker.com/engine/security/)).
- **An internal TCP endpoint had no authentication.** Containers in Docker Desktop
  reached the Engine API at 192.168.65.7:2375, whatever the user's settings
  ([CVE-2025-9074](https://cveawg.mitre.org/api/cve/CVE-2025-9074), 2025-08-20).
- **A second, unfiltered socket went around the filter.** Docker Desktop's `docker.raw.sock`
  let a user bypass Enhanced Container Isolation
  ([CVE-2023-0629](https://cveawg.mitre.org/api/cve/CVE-2023-0629), 2023-03-13).
- **A sound socket check still served a confused deputy.** Tailscale Serve let users
  publish directories that they could not read (TS-2024-001, 2024-01-08). It also proxied
  Unix sockets for a non-root operator (TS-2026-005, 2026-06-03,
  [bulletins](https://tailscale.com/security-bulletins)). Docker Desktop let code in its
  VM request Unix socket forwards to host paths (4.93.0, 2026-09-28,
  [release notes](https://docs.docker.com/desktop/release-notes/)).
- **A guest-side socket was open to every user in the guest.** With the QEMU driver, any
  user in a Lima VM could reach root daemons through the guest agent socket
  ([GHSA-2j9v-p4xj-cjw2](https://github.com/lima-vm/lima/security/advisories/GHSA-2j9v-p4xj-cjw2),
  CVE-2026-53657, 2026-06-19).
- **Another user pre-created the socket directory.** VirtualBox kept its IPC socket in
  `/tmp/vbox-$USER-ipc`, and another local user could create it first
  ([oss-security](https://seclists.org/oss-sec/2008/q4/225), CVE-2008-5256, 2008-11-24).
- **A check by pid let a reused pid pass as a trusted client.** Three macOS privileged
  helpers checked their XPC clients by pid in 2026
  ([CVE-2026-24067](https://cveawg.mitre.org/api/cve/CVE-2026-24067), 2026-06-10, and
  CVE-2026-24071 and CVE-2026-24065). Apple DTS advises against a code-signature check
  that an app writes itself, because of time-of-check problems
  ([Apple forum 773573](https://developer.apple.com/forums/thread/773573), February 2025).
- **The macOS socket path limit broke tools.** `sun_path` holds 104 bytes
  ([un.h](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/un.h)). Lima
  keeps its home in `~/.lima` for this reason
  ([Lima internals](https://lima-vm.io/docs/dev/internals/)). Orchard and tart both work
  around it in code (section 5).

On macOS, a TCP connection gives the server no peer credentials. tailscaled therefore
reads a random token from a "sameuserproof" file for its TCP path
([safesocket_darwin.go](https://github.com/tailscale/tailscale/blob/main/safesocket/safesocket_darwin.go)).
Koine uses loopback TCP with a bearer credential. It refuses any request with an `Origin`
header and any `Host` other than `127.0.0.1:<port>`, which blocks DNS rebinding
(`docs/client-guide.md` in Koine). Its credential needs a human approval, which a
TestAnyware agent cannot wait for.

### What a signed app can check on a Rust CLI

| Check | Ad hoc CLI (a `cargo build`) | CLI signed with Developer ID |
|---|---|---|
| Peer uid with `getpeereid` | Works | Works |
| Peer pid, then a code check by pid | Unsafe (pid reuse) | Unsafe (pid reuse) |
| Audit token from `LOCAL_PEERTOKEN`, then a requirement | Matches one build only (its cdhash) | Matches the team, but the source and Apple DTS disagree on when the token is taken |
| XPC requirement on each message | Matches one build only | Matches the team on each message |

Each row comes from the sources above and from
[Apple forum 850777](https://developer.apple.com/forums/thread/850777), about
2026-10-15. The linker signs every Apple silicon binary ad hoc, so a plain `cargo build`
has no stable identity
([macOS 11 release notes](https://developer.apple.com/documentation/macos-release-notes/macos-big-sur-11_0_1-universal-apps-release-notes)).

### For the design

1. **Use a Unix socket, not localhost TCP.** Most failures in this section came from TCP
   on loopback. The fallback already avoids it.
2. **Check the peer uid on each connection, in both directions.** The CLI also checks the
   owner and mode of the socket directory. That defeats a directory that another user
   created first. With this check, the fallback equals the app at the boundary of one user.
3. **Do not count a code-signature check of the CLI as a gain.** Any process of the user
   can run a signed CLI with any arguments. The check adds parts and a dependency on a
   stable CLI signature.
4. **Treat the socket path as a contract.** Keep it under 104 bytes, and refuse a longer
   path rather than truncate it. Avoid `/tmp` and a global symlink.
5. **Limit what a reachable broker does.** The broker refuses any request that reaches a
   host path or a host socket. Code in a guest never controls the host side. This work is
   the same in both designs.

## 5. Guest traffic through a broker

**The question.** How do Orchard, Lima and UTM carry agent, SSH or VNC traffic to a guest?
What failed after real use?

### How the tools carry traffic

| Tool | What the host process forwards | Transport |
|---|---|---|
| Orchard | Bytes. The controller joins the client's WebSocket to a worker connection ([api_vms_portforward.go](https://github.com/openai/orchard/blob/main/internal/controller/api_vms_portforward.go)). | WebSocket to the controller, then TCP from the worker to the VM's IP ([rpcv2.go](https://github.com/openai/orchard/blob/main/internal/worker/rpcv2.go)) |
| Lima | Bytes. The host agent forwards guest ports. | SSH `-L` through a ControlMaster, or gRPC tunnels over vsock ([port docs](https://lima-vm.io/docs/config/port/)) |
| UTM | Commands. `utmctl` wraps the AppleScript interface of UTM.app ([scripting docs](https://docs.getutm.app/scripting/scripting/)). | Apple Events, then the QEMU guest agent |
| tart | Bytes. `tart run` listens on `control.sock` in the VM directory ([ControlSocket.swift](https://github.com/openai/tart/blob/main/Sources/tart/ControlSocket.swift)). | Unix socket to vsock port 8080 |
| Docker Desktop | Bytes. `com.docker.backend` owns the published ports ([networking docs](https://docs.docker.com/desktop/features/networking/)). | Host socket to vsock |
| Multipass | Nothing. The daemon hands the CLI an address and a key, and the CLI dials SSH itself ([shell.cpp](https://github.com/canonical/multipass/blob/main/src/client/cli/cmd/shell.cpp)). | Direct SSH |

### What went wrong

**The most frequent failure is a silent stall of every stream.** The host process stays
alive, logs nothing, and only a restart clears it.

- Lima: "every forwarded port broke at once and stayed broken until manual intervention
  (44 and 13 minutes)"
  ([lima issue 5545](https://github.com/lima-vm/lima/issues/5545), 2026-09-26).
- Orchard: the worker logged a running stream, but the controller received nothing until
  a restart ([orchard issue 152](https://github.com/openai/orchard/issues/152),
  2024-01-24). After an nginx reload, `orchard vnc` and `orchard ssh` stayed broken until
  the worker restarted ([orchard issue 98](https://github.com/openai/orchard/issues/98),
  2023-06-29).
- gvproxy: after a short loss of the host network, every published port was dead, while
  `podman machine list` still said `Running`
  ([gvisor-tap-vsock issue 648](https://github.com/containers/gvisor-tap-vsock/issues/648),
  2026-04-27).
- Docker Desktop: each day the user could reach no container, although the containers ran
  and the logs showed no error ([for-mac issue 6654](https://github.com/docker/for-mac/issues/6654), 2022-12-30).

**Multiplexed streams failed in ways that one connection per stream avoids.**

- Lima's gRPC forwarder leaked one tunnel for each keep-alive request. At about 2048
  tunnels, the `grpc.MaxConcurrentStreams(2048)` limit stopped all forwarding
  ([lima issue 5042](https://github.com/lima-vm/lima/issues/5042), 2026-05-26).
- It treated a half-close as a full close, so a server's response was lost
  ([lima issue 5329](https://github.com/lima-vm/lima/issues/5329), 2026-07-24).
- A synchronous log write stalled its event loop for 10 to 11 seconds
  ([lima issue 5410](https://github.com/lima-vm/lima/issues/5410), 2026-08-11).
- Lima made gRPC the default, reverted it two days later "for the stability reason"
  ([v1.0.1](https://github.com/lima-vm/lima/releases/tag/v1.0.1), 2024-11-08), and made it
  the default again in v1.1.0 (2025-05-21).
- Orchard moved its controller-to-worker channel from gRPC to WebSocket over HTTP/1.1, to
  "avoid multiplexing issues with nginx" and other proxies
  ([0.27.0](https://github.com/openai/orchard/releases/tag/0.27.0), 2025-02-06).

**Sleep broke TCP links, but vsock links survived.** After a sleep, Lima's SSH
ControlMaster was dead, and Lima never restored its socket forwards. On the `vz` driver
the failure was silent, because the guest-agent link rides vsock and survived
([lima issue 5420](https://github.com/lima-vm/lima/issues/5420), 2026-08-17).

**Large transfers lost data or memory.**

- tart truncated output over a few hundred KiB from some guests, and `tart exec` still
  returned 0 ([tart issue 1347](https://github.com/openai/tart/issues/1347), 2026-09-24).
- `tart exec -i` kept all of standard input in an unbounded buffer. A 4 GiB input reached
  9.35 GiB of resident memory
  ([tart issue 1348](https://github.com/openai/tart/issues/1348), 2026-09-24).
- gvproxy exited with ENOBUFS when it pulled large images
  ([gvisor-tap-vsock issue 367](https://github.com/containers/gvisor-tap-vsock/issues/367),
  2024-06-28).

**A dead or restarted host process left state that blocked recovery.**

- One badly timed disconnect stopped tart's control socket for the life of the VM
  ([tart issue 1346](https://github.com/openai/tart/issues/1346), 2026-09-24).
- A stale `control.sock` after a VM restart made `exec` hang for about 10 seconds
  ([tart issue 1220](https://github.com/openai/tart/issues/1220), 2026-03-24).
- A stale socket file outlived Lima's SSH master after any unclean exit
  ([lima issue 4913](https://github.com/lima-vm/lima/issues/4913), 2026-04-30).
- With the host agent dead, `limactl` could neither connect to the instance nor stop it
  ([lima issue 357](https://github.com/lima-vm/lima/issues/357), 2021-10-21).

**UTM's command route fails outside a GUI session and hides errors.** Its maintainer: "The
way apple events works depends on the GUI app running under some logged in user. So no,
you can't do it from just a ssh session"
([UTM PR 5081](https://github.com/utmapp/UTM/pull/5081), 2023-03-02). Earlier, UTM dropped a
socket interface in favour of Apple Events (same PR). Recent reports:

- A failed Apple Event exits 0
  ([UTM issue 7933](https://github.com/utmapp/UTM/issues/7933), 2026-10-01).
- `utmctl exec` returns before the guest process ends
  ([UTM issue 7932](https://github.com/utmapp/UTM/issues/7932), 2026-10-01).
- After a login, `utmctl list` shows no VMs until someone opens the main window
  ([UTM issue 7949](https://github.com/utmapp/UTM/issues/7949), 2026-10-08).

**Local Network blocked host processes that reach into the guest.**

- On Sequoia, an Orchard worker that `launchd` started received "no route to host"
  ([orchard issue 221](https://github.com/openai/orchard/issues/221), 2024-11-20). Go
  binaries then had no `LC_UUID`
  ([golang issue 68678](https://github.com/golang/go/issues/68678), 2024-07-31).
- tart over SSH on a headless host met the same denial. tart fixed it with a usage
  description in the Info.plist of `Tart.app`
  ([tart issue 1056](https://github.com/openai/tart/issues/1056)).
- The tart FAQ: "This is not caused by Tart itself, but by the host-side process that
  needs network access into the guest" ([tart FAQ](https://tart.run/faq/)).
- Multipass, whose CLI dials SSH itself, failed only in `shell` and `exec` on Sequoia
  ([multipass issue 3864](https://github.com/canonical/multipass/issues/3864),
  2024-12-31).

**The hypervisor's VNC server has its own faults.** The plain Virtualization.framework
VNC path drops every second keystroke in Terminal. The `--vnc-experimental` server maps
non-US keyboard layouts wrongly
([tart issue 1317](https://github.com/openai/tart/issues/1317), September 2026). These
faults are not in a broker, but a broker passes them through.

### For the design

1. **Probe the guest path, not the process.** A live process said nothing about the
   streams in every stall above. A health check of the broker runs an agent request and
   an RFB handshake for each VM. A version check on the fallback socket would not catch
   these stalls either.
2. **Give each guest stream its own connection, and pass a half-close through.** A Unix
   socket connection for each stream gives this with no extra work. The broker loses this
   if its socket multiplexes VNC, recordings and transfers on one connection.
3. **Bound every buffer, and check the length of each transfer and recording.** The broker
   adds one relay hop, so it adds one buffer to bound.
4. **Make the broker restartable, and let no stale socket file block a restart.** A dead
   broker stops every VM's traffic at once, so the CLI reconnects with no human action.
5. **Hold the grant in the one signed bundle, and depend on no Apple Events or window.**
   UTM shows the cost of a GUI-app route from SSH and before a login. A `launchd` daemon is
   exempt from Local Network privacy (section 3), but its costs as root were outside this
   survey.

## 6. Adoption

**The question.** When a broker restarts, how does it adopt VMs or processes that outlived
it? How does it guard against pid reuse?

### What the tools do

**Containerd finds each shim through its socket address, not a pid.** It reads
`bootstrap.json` from the bundle and dials the shim. It deletes the bundle when the load
fails
([shim_load.go](https://github.com/containerd/containerd/blob/main/core/runtime/v2/shim_load.go)).

**tart finds a running VM through a lock that the kernel holds.** `tart list` asks
`F_GETLK` on `config.json` for the holder's pid. tart writes no pid file
([VMDirectory.swift](https://github.com/openai/tart/blob/main/Sources/tart/VMDirectory.swift)).
QEMU holds an `fcntl` write lock on its `-pidfile`, and it compares inodes after it takes
the lock
([QEMU commit 9e6bdef](https://github.com/qemu/qemu/commit/9e6bdef224f700c057462a7d5e9b4a2770e04569),
2018-08-31). libvirt kills from a pidfile only when the lock is held, that is, when a live
holder exists
([libvirt commit a8db4dc](https://github.com/libvirt/libvirt/commit/a8db4dc31f70798a81611d5ea2d18bd235fd467d),
2020-03-13).

**Lima signals the recorded pid with signal 0.** It now also records the boot session
UUID beside its pid files
([lima commit e529ce0](https://github.com/lima-vm/lima/commit/e529ce0874f8f220deb3056334e1bafae54b8476),
2026-08-23). It does not adopt an orphaned VM driver. `limactl start` force-stops it
([lima commit e1268fd](https://github.com/lima-vm/lima/commit/e1268fd1011c3d151400f082591421d667e2af96),
2026-09-12).

**VirtualBox has nothing to adopt.** Its headless frontend exits when VBoxSVC goes away,
and its GUI powers the VM off
([VBoxHeadless.cpp](https://github.com/VirtualBox/virtualbox/blob/main/src/VBox/Frontends/VBoxHeadless/VBoxHeadless.cpp)).

**Koine names a process by its pid and its start time, never by the pid alone.** The
identity is `{ pid, startedAt }`, from `proc_pidinfo(PROC_PIDTBSDINFO)`, compared
exactly. The guide notes that no command-line tool reports the start time at that
precision. It also tells clients to ignore the pid in `endpoint.json`. A client "cannot
take Koine's instance lock, so it can never know whether `pid` is still Koine's"
(`docs/client-guide.md` in Koine).

**TestAnyware today trusts signal 0 for the VM pid.** `process_alive` sends signal 0
(`cli-rs/crates/testanyware-vm/src/process.rs:14`). Only the VNC keeper's stop also reads
the command line with `ps` before it signals
(`cli-rs/crates/testanyware-vm/src/keeper.rs:98`).

### What went wrong

- **A reused pid looked alive after a reboot.** Lima "then reports the instance as
  `Broken`" (lima commit e529ce0). A user found `vz.pid` naming an unrelated process
  ([Roots forum](https://discourse.roots.io/t/trellis-vm-wont-start-after-reboot-vz-driver-is-running-but-host-agent-is-not/30350),
  2026-05-24). Lima still lists "PID reuse within a single boot" as open (lima commit
  e1268fd).
- **The fix for pid reuse added a race.** A boot marker that is not written atomically can
  make Lima delete the pid files of the current boot
  ([lima issue 5646](https://github.com/lima-vm/lima/issues/5646), 2026-10-18, open).
- **A reused pid received a signal meant for another process.** Rancher Desktop adopted
  orphaned Lima host agents and killed an unrelated process
  ([rancher-desktop-2 issue 364](https://github.com/rancher-sandbox/rancher-desktop-2/issues/364),
  2026-05-22). `podman rm` once killed itself
  ([podman issue 25104](https://github.com/podman-container-tools/podman/issues/25104),
  2025-01-23). On macOS, Claude Code sent SIGKILL to an unrelated process
  ([claude-code issue 96394](https://github.com/anthropics/claude-code/issues/96394),
  2026-09-23).
- **A start-time check alone is still racy.** A Podman maintainer: "the pid might have
  exited between our check and when we kill it" (podman issue 25104). Podman now stores a
  pidfd handle or the start time with the pid
  ([podman PR 25906](https://github.com/podman-container-tools/podman/pull/25906),
  2025-05-08).
- **A failed reconnect killed a running guest.** libvirt killed QEMU when a reconnect step
  failed ([libvirt-devel](https://lists.libvirt.org/archives/list/devel@lists.libvirt.org/thread/RI5TMRXHFFP62PHCYFN7MVILUL6ERLRJ/),
  2015-12-17). It did so again in 2017: "Nothing … could justify unceremoniously killing
  the qemu process, but that's what we were doing"
  ([libvirt commit 29b57a2](https://github.com/libvirt/libvirt/commit/29b57a2f15b8993935947e12692693a01efeaefb),
  2017-04-25).
- **A wrong adoption duplicated a workload.** Containerd took a live shim for a leaked
  one, it orphaned the container, and a replacement started beside it
  ([containerd issue 13784](https://github.com/containerd/containerd/issues/13784),
  2026-07-13).
- **A survivor that answers nothing stalled the restart.** An old shim accepted the
  connection and never answered, so containerd never finished starting
  ([containerd commit fd29ff1](https://github.com/containerd/containerd/commit/fd29ff1073de96f6c50ee2bdf92ae23edac2b567),
  2026-08-13).
- **Adoption does not cross versions.** Docker's live restore is "only supported when
  installing patch releases"
  ([live-restore docs](https://docs.docker.com/engine/daemon/live-restore/)). Processes
  that survive an upgrade keep running the old code
  ([GHSA-36xw-fx78-c5r4](https://github.com/containerd/containerd/security/advisories/GHSA-36xw-fx78-c5r4),
  2020-11-30).
- **A lock on an inode does not survive a replaced path.** A clone over a running tart VM
  made tart lose track of it. The VM still held a slot of the macOS guest limit
  ([tart PR 1321](https://github.com/openai/tart/pull/1321), 2026-08-28). Closing any
  descriptor of the file drops an `fcntl` lock. QEMU once lost its pidfile lock that way
  ([QEMU commit 93dd748](https://github.com/qemu/qemu/commit/93dd748b789202af4f5be75412c58ee1ed481b29),
  2012-01-27).

### What macOS offers against pid reuse

- **Pids wrap at 99999 and start again at 100** (`PID_MAX` in XNU
  `bsd/sys/proc_internal.h`).
- **The start time is public.** `proc_pidinfo` with `PROC_PIDTBSDINFO` returns
  `pbi_start_tvsec` and `pbi_start_tvusec`. Apple recommends that a tool combine "the PID
  with the process's start time"
  ([Apple forum 824760](https://developer.apple.com/forums/thread/824760), April 2026).
  The 64-bit unique id of a process is private API (same thread).
- **The boot session UUID is public** as the `kern.bootsessionuuid` sysctl, which Lima now
  records (lima commit e529ce0).
- **`EVFILT_PROC` binds to a process when it attaches,** so a later exit cannot pass to a
  reused pid (XNU `kqueue(2)`).
- **The kernel drops `fcntl` locks when the process ends,** and also on any close of the
  file (XNU `fcntl(2)`).
- **An audit token carries a pid version.** Apple DTS warns that a token gives no
  protection when the tool starts from a pid
  ([Apple forum 652363](https://developer.apple.com/forums/thread/652363), July 2020).

### For the design

1. **Record identity, not only a pid.** Each sidecar records the pid, the start time and
   the boot session UUID. The broker checks all three before it adopts or signals a
   process. It writes the sidecar atomically.
2. **Prefer a lock that the kernel holds as the record of liveness.** tart already locks
   `config.json`, and QEMU can lock its `-pidfile`. The VNC keeper can lock a file of its
   own. A signal goes only to a lock holder. The CLI can also read these locks while the
   app is down, which keeps `vm list` working.
3. **A failed adoption never kills or duplicates a VM.** The broker marks the VM "not
   adopted", leaves it running and reports it. The app has more ways to fail than the
   fallback, so this rule matters more for the app.
4. **Close the window between the check and the signal.** Attach `EVFILT_PROC` first,
   then check the start time, then signal. A check followed by `kill` is racy.
5. **Probe each survivor with a deadline.** An old process that answers nothing must not
   stall the broker's start. A timeout means "not adopted", not "dead".

None of these points depend on the channel, so the app and the fallback tie here.

## Walk-away checks

For each system, the table answers one question: with the tool uninstalled, what stays on
disk, and can a person read it?

| System | What stays on disk | Readable without the tool |
|---|---|---|
| Docker Engine | `/var/lib/docker`, `daemon.json`, the root-owned socket | `daemon.json` is JSON. The image store was not checked. |
| Docker Desktop | `~/.docker`, `~/Library/Group Containers/group.com.docker`, the `/var/run/docker.sock` symlink, helpers in `/Library/PrivilegedHelperTools` ([uninstall docs](https://docs.docker.com/desktop/uninstall/)) | JSON and symlinks, yes. The VM disk was not checked. |
| Tailscale (macOS) | Preferences and containers under `~/Library`, Keychain items, `/Library/Tailscale` with `ipnport` and `sameuserproof-<port>` | `ipnport` is a symlink and the proof file holds hex. Keychain items need Keychain Access. |
| GnuPG | The home directory with keys and sockets, and stale `.#lk.<host>.<pid>` lock files | Sockets and locks, yes. Key files were not checked. |
| Bazel | `<output_base>/server/` with `server.pid.txt`, `command_port` and the `install` symlink | Text files and a symlink, yes |
| Gradle | `~/.gradle/daemon/<version>/` with `registry.bin` and logs | Logs, yes. `registry.bin` appears binary (not checked). |
| VirtualBox | `VirtualBox.xml`, the `.vbox` files, and `/tmp/.vbox-<user>-ipc` | XML, yes. The `.vbox` machine element records an `aborted` state. |
| Lima | `~/.lima/<instance>/` with `lima.yaml`, `lima-version`, `ha.pid`, the driver pid file, `boot-session.tmp`, logs and dead sockets | Yes, with `cat`. Only the boot marker shows that a pid is stale. |
| Colima | `~/.colima/default/docker.sock`, recreated with mode 0600 at each start | `ls -l` shows it |
| Podman | Machine identity under `~/.local/share/containers/podman/machine/`, and exec-session pids in a BoltDB or SQLite database | Files, yes. The database needs `sqlite3` or a Bolt tool. |
| containerd | `bootstrap.json` under `/run/containerd/…/<id>/` | JSON, yes. `/run` is cleared at reboot. |
| QEMU | The `-pidfile` | A decimal pid. After a crash it can name a reused pid. |
| libvirt | A status XML and a pid file under its run directory (paths not checked) | Yes |
| tart | `~/.tart/vms/<name>/` with `config.json`, `disk.img`, `nvram.bin` and a stale `control.sock` after a crash | `config.json`, yes. The "running" state lives only in the kernel's lock table, which `lsof` shows. |
| Orchard | `~/.orchard/orchard.yml` (mode 0600) and `~/.orchard/controller/` with certificates and a Badger database | YAML and PEM, yes. The database needs Badger tools (not checked). |
| UTM | No `utmctl` state. VM bundles under `~/Library/Containers/com.utmapp.UTM/Data/Documents/`, each with a `config.plist` | Yes, with `plutil` |
| gvproxy | Transient sockets under `$TMPDIR/podman/` | Dead sockets only |
| Multipass | `/var/root/Library/Application Support/multipassd`, unless the user asks for deletion | JSON that root can read and edit |
| 1Password CLI | No socket file, because the channel is XPC to a helper in the app | Nothing to read |
| tmux | The server socket and `<socket>.lock` | `ls` shows both, and neither holds data |
| OpenSSH | `~/.ssh/agent` sockets (since 10.1) and dead ControlMaster sockets | `ls` shows them |
| Watchman (`launchd`) | The plist in `~/Library/LaunchAgents` | XML, yes. In the Watchman issue it still named the old binary. |
| SMAppService agent | The approval record in the background task database | Only `sfltool dumpbtm` prints it |
| Zoom (2019) | The web server in `~/.zoomus`, which kept running after uninstall | `ls` shows it |
| Koine | `~/Library/Application Support/Koine` with `endpoint.json` | JSON, yes |
| Local Network store | `/Library/Preferences/com.apple.networkextension.plist`, with rules that name paths which no longer exist | A binary plist that `plutil -p` reads. Apple calls the format an implementation detail. |
| TCC | `/Library/Application Support/com.apple.TCC/TCC.db` | SQLite. `tccutil` clears rows only by service and bundle id. |
| The fallback (0700 socket) | One directory and one socket file, which stays after the socket closes | `ls -l` shows both. The server must delete a stale file when it starts. |

## Where the search found silence

Each item below had no primary source. The query lists are in the research passes, and the
main queries follow each item.

- **The record format of a Local Network grant, and which field decides a match.** Apple
  documents neither. Query: `"local network" privacy macOS "executable UUID" forum thread Quinn`.
- **What an in-place update at the same path does to a Local Network grant.** No Apple
  statement. The evidence is indirect: apps in /Applications that keep their grants.
- **A grant lost after a Developer ID certificate renewal.** No report found.
- **A grant lost after a cask upgrade of an app in /Applications.** No report found.
- **What `op` does when the 1Password app is not running.** No official statement, and one
  user report only suggests that `op` brings the app forward.
- **Two `open` calls for one bundle id at the same moment.** No source on how
  LaunchServices orders them.
- **A start of an app from a herdr-like login service.** No source covers that context.
- **A version check between VBoxManage and VBoxSVC.** The source shows only a wire-format
  version in the IPC header.
- **Orchard across host sleep, and Orchard throughput for long VNC sessions or large
  transfers.**
- **A long VNC session or a long screen recording that failed through any broker.** Queries
  covered tart, Orchard and UTM.
- **The Virtualization.framework VNC server in long sessions or on disconnect.** The
  reconnect crash of ADR-0018 is the only known fault.
- **When `LOCAL_PEERTOKEN` takes the audit token.** Apple DTS says at connect. The
  published XNU source reads the token of the socket's most recent user. The survey could
  not settle this.
- **A bypass of the 1Password CLI signature check.** No report found.
- **Bugs in macOS `EVFILT_PROC` with pid reuse.** None found for macOS.
- **A wrong kill from a QEMU pidfile.** None found.
- **gpg start races on dev.gnupg.org.** Snippets from a web search named tasks T5069,
  T6838 and T5884. Every fetch of that site failed, so the survey cites none of them.

## Citations checked again

I fetched these again myself on 2026-10-20 and compared them with the research passes.
Each one matched.

- TN3179, from Apple's JSON copy of the page:
  - the sentences on the code signature and the executable UUID
  - the per-user state, and the sentences on `launchd` daemons and agents
  - the responsible code, and FB16131937
  - the reset buttons of macOS 27.2, and the two system defaults of macOS 15.5.
- Through the GitHub API:
  - lima commits e529ce0 and e1268fd, and lima issue 5646
  - containerd issue 13784
  - claude-code issues 90412 (with comment 5859659591) and 96394
  - homebrew-core issue 312547, and moomux issue 304
  - moby PR 41285, and for-mac issue 7709
  - watchman issue 358, with the comment of 2016-11-01.
- The tmux `client.c` comment on the lock and second connect.
- The CVE records CVE-2022-41924, CVE-2025-9074 and CVE-2026-24067.
- The redirects of `cirruslabs/tart` to `openai/tart` and of `cirruslabs/orchard` to
  `openai/orchard`.
- In this repository and on this Mac: the formula path of TestAnyware 2.3.0, the
  `koine` and `modaliser` casks of the tap, Koine's `docs/client-guide.md`, and
  `process.rs` and `keeper.rs`.

## Where the findings landed

The design leaf `vm-broker-design-k3` adopted these findings:

- **Version skew (section 1)** in `docs/specs/testanyware-app.md`. The pair
  compares a protocol range, refuses with a remedy, and keeps the floor low. The
  owner chose the Koine model, in which no client starts the app, so no client
  replaces an old app either.
- **Start on demand (section 2)** in the same spec, in reverse. The owner chose
  a login item, as Koine has, so a client never starts the app.
- **Grant persistence (section 3)** in ADR-0020. A measurement on this Mac
  separated the path from the executable UUID, and the UUID decided
  (`docs/research/testanyware-app-local-network-grant.md`).
- **Access control (section 4)** in the spec: a Unix socket in a 0700
  directory, with a peer uid check and no TCP listener.
- **Guest traffic (section 5)** in ADR-0004: one connection for each guest
  stream, no multiplexing, and a half-close passed through.
- **Adoption (section 6)** in the spec: an identity of pid, start time and boot
  session, and an adoption that fails never stops a VM.
