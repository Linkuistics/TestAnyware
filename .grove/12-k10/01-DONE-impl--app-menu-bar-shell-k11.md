# app-menu-bar-shell-k11

## Goal

Put the app server of `app-streams-k8` inside a menu bar app, and build it as a signed development bundle.

## Context

- `docs/specs/testanyware-app.md`, sections "The TestAnyware app" and "Packaging and identity".
- ADR-0003, ADR-0005 and ADR-0020.
- Koine's `scripts/build-app.sh`, `scripts/signing-env.sh` and `Sources/KoineApp/LoginLaunchView.swift`.
- `docs/research/testanyware-app-local-network-grant.md`, for the probe app that the owner already measured.

## Done when

- The AppKit run loop runs on the main thread and shows one status item. The asynchronous runtime runs the server on its own threads.
- The menu shows the version, whether the app serves its socket, and the Local Network state. The Local Network state has an item that opens the Local Network pane. The menu also has the Start at Login switch. The switch uses `SMAppService.mainApp` and reports the status `requiresApproval`.
- The menu has a Quit item. Its text says that each VM keeps running.
- A pure model builds the menu content from the state of the app. Unit tests cover it.
- A script builds the bundle for a channel. It writes the Info.plist, sets `LSUIElement`, and signs with the hardened runtime and team `TA43A4RUP3`. It checks the bundle id and the team, and it refuses any other.
- The development channel uses the id `dev.antony.TestAnyware.dev`. The release channel uses `dev.antony.TestAnyware`.
- A Taskfile task builds the development bundle. A live check opens it, and `GET /v1/status` answers on the socket with the bundle's build.

## Notes

- Test mode from `app-streams-k8` stays available in the same executable.
- Keep the shell thin. The server code must not depend on AppKit.
- The leaf may decompose if the bundle script and the shell do not fit one session. Cut the script first.

- `app-menu-bar-shell-k11` left the shell in the crate `testanyware-app`. `app-default-routing-k26` can start the bundle with `task app:bundle-dev` and `open -n -g`. The bundle takes no arguments, so it serves the real socket in `~/Library/Application Support/TestAnyware/` and the VM records of `VmPaths::from_process_env()`. The live check `task app:bundle-live` refuses to run while another app answers on that socket.
- The accessibility grant of the terminal lets a script read and click the status item through System Events. `scripts/app-bundle-live.sh` does so, and its menu step reports SKIP without the grant. The herdr acceptance run in the same increment can use the same method to click Start at Login.
- The release channel of `scripts/build-app.sh` builds and signs with a secure timestamp and does not notarise. `app-release-k21` adds notarisation.

## Decisions (running log)

- **Where the AppKit code lives.** `src/shell.rs` is the only module that imports AppKit, and `lib.rs` compiles it on macOS only. `src/menu.rs` is a pure function from `MenuState` to `Vec<MenuEntry>`, with seven unit tests. The shell turns entries into `NSMenuItem`s and clicks into `MenuAction`s, and it decides nothing. The server modules did not change. The test mode runs the same code as before, and all 53 + 15 + 15 + 18 tests of the app, the client, the protocol crate and the CLI routing still pass.
- **One executable, two modes.** `main` builds the multi-thread runtime by hand. In both modes it binds the socket first, so a second app exits quietly before it shows a status item. Test mode then blocks on the server. App mode spawns the server on the runtime and gives the main thread to `shell::run`. A fatal accept error clears an `AtomicBool` that the menu reads as "Not serving its socket", and the app stays up so a person can see that and quit. `applicationWillTerminate:` removes the socket file, and a menu Quit leaves no stale file (the live check proves it).
- **The menu is rebuilt each time it opens.** The delegate implements `menuNeedsUpdate:`, so the menu shows the state of that moment, including the `SMAppService` status read back from macOS. The switch calls `register()` or `unregister()` and keeps only the error text. `NotFound` counts as not registered, as Koine found.
- **Verified by hand on this Mac (macOS 27.0.1).** The bundle opens through LaunchServices and the status item appears (a screenshot shows it). The menu reads as the model says. Start at Login registered the dev bundle (macOS showed "Login Item Added") and unregistered it again, and the check mark followed the status each time. Quit exits the process and removes the socket. **Not seen:** the `requiresApproval` status, because this Mac approved the item at once. Only the unit tests cover that menu.
- **Gap: the Local Network item cannot reach the Local Network row.** The `?Privacy_LocalNetwork` anchor opens Privacy & Security, and I tried seven other forms. The Privacy extension of System Settings has anchors for Camera and others (`?Privacy_Camera` reaches the Camera pane) and none for Local Network. I stopped there, to avoid a rabbit hole. The item keeps its spec name, opens Privacy & Security, and the spec's open risks say so. The decision for the human: keep this, or rename the item to say "Privacy & Security".
- **Bundle script.** `scripts/build-app.sh dev|release` fixes the id by channel (`dev.antony.TestAnyware.dev`, `dev.antony.TestAnyware`). It refuses any signing identity whose name does not end in `(TA43A4RUP3)` before it builds, and it reads the id, the team and the hardened-runtime flag back from the signed bundle. I ran the refusals: no channel, a wrong channel, a wrong team, a suffix after the team and an Apple Development identity all exit 1 with a message. Koine's script was the model, and I did not copy its provider and framework steps.
- **Sources.** `SMAppService` calls cite the Apple documentation pages that Koine's `LoginLaunchView.swift` cites, and `NSStatusBar.statusItem(withLength:)` is cited beside the call. The `objc2-service-management` 0.3.2 and `objc2-app-kit` 0.3.2 signatures come from the crate sources in the cargo registry. The Local Network URL has no Apple documentation, and the code says so and gives the measurement.
- **Controls seen to fail.** Two mutations of `menu.rs` (a login status that counts `RequiresApproval` as not registered, and a Quit title that drops the VM sentence) each failed a test, and I restored the file. The live check failed with exit 1 when an app already held the socket.
- **Not verified.** I could not run `cargo check` for a Linux target, because the toolchain here has no Linux standard library installed. The macOS-only code sits behind `cfg(target_os = "macos")` for both the modules and the dependencies, as ADR-0003 asks. `app-release-k21` runs the Linux and Windows runtime check on the candidate.
- **Review.** I spent no in-session reviewer. The compiler, the unit tests and the live check cover the decisions of this leaf.
