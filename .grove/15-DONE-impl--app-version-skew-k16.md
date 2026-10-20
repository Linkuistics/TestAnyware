# app-version-skew-k16

## Goal

Never let a version mismatch between the Host CLI and the running app pass silently.

## Context

- `docs/specs/testanyware-app.md`, section "Version skew", and the requirement "A version mismatch is never silent".
- `vm-broker-design-k6`, finding F5, which fixes the notice on stderr.
- `app-protocol-core-k9`, which already carries the ranges and the builds in headers.

## Done when

- A pure function takes the two ranges, the two builds and the build of the bundle on disk. It returns a decision: serve, serve with a notice, or fail with one of three remedies.
- When the ranges do not overlap, the command fails with `APP_INCOMPATIBLE`, cause `protocol`. The error names both builds and both ranges, and one remedy from the table of the spec.
- When the builds differ and the ranges overlap, the command proceeds. The CLI writes one notice line to stderr in both output modes. The line names both builds and the remedy.
- The notice changes neither stdout nor the JSON envelope. `--quiet` does not suppress it.
- The status response says whether the running app is older than its bundle on disk.
- Tests in the harness use a test mode that reports forged builds and ranges. They cover each row of the table.

## Notes

- A client never restarts the app. The remedies tell the person what to do.
- The report of this state in `doctor` comes in `app-diagnostics-k19`.

## Decisions (running log)

- **One pure function, in the shared protocol crate.** `testanyware_app_protocol::skew::decide(cli, app, bundle_build)` takes two `Peer`s (each a build and a range, both optional) and the bundle build. It returns `Serve`, `ServeWithNotice(Notice)` or `Fail(Remedy)`. `Remedy` is the table of the spec as a type, with a sentence (`text`) and a stable word (`word`). The app's gate calls it to refuse a request, and the Host CLI calls it on each response, so the table lives in one place. The crate gains one dependency, `semver`, which was already in the lock file.
- **Who is older.** For ranges that do not overlap, the lower range is the older side. For ranges that overlap, the lower build is the older side. A side that states no range is the older one. A build that is missing, or is no version, never counts as older, so the app is then the side to change. These rules are spec text now.
- **An unknown bundle build gives the cask upgrade.** A bundle build that is missing or unreadable never counts as newer than the running app. `brew upgrade --cask` is the safe remedy, because a cask upgrade quits and opens the app again.
- **A third response header.** The Host CLI needs the bundle build to word the notice, and a notice is a response of the app that serves the request. So every response carries `X-Testanyware-Bundle-Build` when the app can read the bundle, and the app reads the `Info.plist` again for each response. The alternative was a second request to `GET /v1/status` after a mismatch. I took the header because it costs no round trip and has no ordering problem. The spec names the header now, as `k9` did for the other headers.
- **The app words the failure, the Host CLI words the notice.** The app's gate knows both ranges, both builds and its own bundle, so it puts the remedy into the `APP_INCOMPATIBLE` error (`remediation`, and `details.remedy` as a stable word). The Host CLI does not recompute a failure. Its observer acts on `ServeWithNotice` only.
- **The notice.** One line on stderr in both output modes: `testanyware: notice: the Host CLI is build X and the running TestAnyware app is build Y. <remedy>`. A process writes it once, through an `AtomicBool`, however many connections the command opens. The CLI has no `--quiet` flag today, so no switch can suppress it, and the code has no such switch. The notice is also written when the command then fails for another reason, because a skew can be the cause.
- **Where the Host CLI sees a response.** `AppClient::with_response_observer` calls a function with the `ServerHello` of every response: a document, a refusal or the `101`. `app_route::app_client(socket)` builds each client of the Host CLI with the observer, and the seven places that built `AppClient::new(.., client_hello())` now call it.
- **Status.** `StatusResponse` gains `bundle_build` and `older_than_bundle`. Both have `serde(default)`, so a reader of an older body still works.
- **The bundle reader.** `bundle::BundleSource` reads `CFBundleShortVersionString` from the XML `Info.plist` of the `.app` that holds the executable. `scripts/build-app.sh` writes that XML, and the cask copies the bundle unchanged. A binary property list gives no build, and the remedy is then the cask upgrade.
- **Test mode forges the app side only.** `--test-build`, `--test-protocol-range` and `--test-bundle-build` all need `--test-mode`. The Host CLI under test is real (the workspace build, range 1-1), so no test hook enters the shipped CLI. Unit tests of the pure function cover a forged CLI side. The harness file `app-version-skew.rs` has a run for each row of the table, in both output modes. It also checks that a refused run never reaches the guest.
- **A control seen to fail.** With the observer removed from `app_client`, the four notice tests failed and the eight others passed. I restored the line and all 12 pass.
- **A test of mine was wrong, not the code.** I expected exit code 1 for `VM_NOT_FOUND`, and the CLI's code for it is 3. I corrected the test.
- **Review.** I spent no in-session reviewer. The one claim that the compiler cannot check is the `expect` in the gate, which holds because `decide` fails exactly when the ranges do not overlap. A test covers a matching request, and the harness covers every disjoint case.
- **Source for the API.** `semver::Version::parse` and its `Ord` are in the registry copy of `semver 1.0.28`. No framework API of a changing version enters this leaf. The hyper calls reuse the ones of `k9` and `k24`.
- **Not run on a Linux or Windows host.** The shared crates build for `aarch64-unknown-linux-gnu` and `x86_64-pc-windows-gnu` with `cargo zigbuild`. The `testanyware-cli` crate does not cross-build here for lack of `pkg-config`, and the new harness file ran on this Mac only. The routed commands there run through `TESTANYWARE_APP_SOCKET`, the same path as the other routing tests.
- **An unrelated flake.** `ready_then_die_on_first_call_surfaces_child_crashed` in `testanyware-ocr-client` failed once under load and passed alone and in the next full run. I did not touch that crate.
