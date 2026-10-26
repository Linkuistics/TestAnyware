//! The VNC commands routed through the TestAnyware app (spec: "The protocol").
//!
//! Each test runs the real `testanyware` binary against the real `testanyware-app` executable
//! in its test mode, with a synthetic RFB server on loopback as the guest. A macOS host routes
//! each command through the app, by `TESTANYWARE_APP_SOCKET` or by the default socket. A command
//! through the app must print and do what the same command did on the direct path, and it must
//! dial nothing itself.
//!
//! A macOS host has no direct path any more, so what it printed is frozen in this file as the
//! oracle. A Linux or Windows host keeps the direct path, and there each routed run is compared
//! with a direct run too.

#![cfg(unix)]

mod common;

use common::{serial, FakeRfb, OutputExt, RfbEvent, TestApp, HOST_GOES_DIRECT};
use serde_json::json;

const VM: &str = "testanyware-feed0002";
const WIDTH: u16 = 64;
const HEIGHT: u16 = 48;

/// A synthetic RFB server, and an app that serves its VM.
struct Fixture {
    rfb: FakeRfb,
    app: TestApp,
}

impl Fixture {
    /// The fakes start first, and the app spawns once (see the rules in `common`).
    fn new(password: Option<&str>, logical: Option<(u32, u32)>) -> Self {
        Self::with(FakeRfb::start(WIDTH, HEIGHT, password), logical)
    }

    fn with(rfb: FakeRfb, logical: Option<(u32, u32)>) -> Self {
        let app = TestApp::start();
        app.add_vnc_vm(VM, &rfb, logical);
        Fixture { rfb, app }
    }
}

/// `screen size` as the direct path printed it.
fn size_text() -> String {
    format!("{WIDTH}x{HEIGHT}\n")
}

/// A `--json` output: sorted keys, two-space indent and a final newline.
fn pretty(value: serde_json::Value) -> String {
    format!("{}\n", serde_json::to_string_pretty(&value).unwrap())
}

fn size_json() -> String {
    pretty(json!({ "height": HEIGHT, "ok": true, "schema_version": "1.0", "width": WIDTH }))
}

/// Run `args` through the app and require `expected` on stdout and nothing on stderr. The
/// guest must accept a connection, so the app dialed it.
///
/// `expected` is the output that the direct path printed. Where a host still has the direct
/// path, a direct run must print it too.
fn routed_prints(fixture: &Fixture, args: &[&str], expected: &str) {
    let before = fixture.rfb.accepted();
    let routed = fixture.app.cli(args);
    let after_routed = fixture.rfb.accepted();
    assert_eq!(routed.code(), 0, "routed {args:?}: {}", routed.err());
    assert_eq!(routed.out(), expected, "stdout of {args:?}");
    assert_eq!(routed.err(), "", "stderr of {args:?}");
    assert!(
        after_routed > before,
        "the routed run did not reach the fake RFB server"
    );

    if HOST_GOES_DIRECT {
        let direct = fixture.app.cli_default(args);
        assert_eq!(direct.code(), 0, "direct {args:?}: {}", direct.err());
        assert_eq!(direct.out(), expected, "direct stdout of {args:?}");
        assert_eq!(direct.err(), "", "direct stderr of {args:?}");
        assert!(
            fixture.rfb.accepted() > after_routed,
            "the direct run did not reach the fake RFB server"
        );
    }
}

#[test]
fn screen_size_through_the_app_prints_the_direct_output() {
    let _lock = serial();
    let fixture = Fixture::new(None, None);
    routed_prints(&fixture, &["screen", "size", "--vm", VM], &size_text());
    routed_prints(
        &fixture,
        &["screen", "size", "--vm", VM, "--json"],
        &size_json(),
    );
}

#[test]
fn the_hidpi_logical_target_of_the_spec_still_applies() {
    // The Host CLI routes by the id, and still reads the spec for the logical target
    // (ADR-0016). A 64x48 wire frame with a 32x24 target is a 2x frame.
    let _lock = serial();
    let fixture = Fixture::new(None, Some((32, 24)));
    routed_prints(&fixture, &["screen", "size", "--vm", VM], "32x24\n");

    // A click is in logical pixels, and the wire carries physical ones.
    let routed = fixture.app.cli(&["input", "click", "10", "5", "--vm", VM]);
    assert_eq!(routed.code(), 0, "{}", routed.err());
    let events = fixture.rfb.wait_for(|e| {
        e.iter()
            .any(|e| matches!(e, RfbEvent::Pointer { buttons: 0, .. }))
    });
    assert!(
        events.contains(&RfbEvent::Pointer {
            buttons: 1,
            x: 20,
            y: 10
        }),
        "{events:?}"
    );
}

/// How late the guest of the next tests reads. The app resets the guest connection within a few
/// milliseconds of the client's end, so a guest that reads this late sees the reset first.
const LATE_READ: std::time::Duration = std::time::Duration::from_millis(100);

/// The input events of a log, without the requests for a framebuffer update.
fn input_events(events: Vec<RfbEvent>) -> Vec<RfbEvent> {
    events
        .into_iter()
        .filter(|e| !matches!(e, RfbEvent::UpdateRequest { .. }))
        .collect()
}

#[test]
fn a_click_has_reached_a_guest_that_reads_late_when_the_command_returns() {
    // Issue 12. tart's VNC server drops the input that a reset follows before it has read it
    // (docs/research/tart-vnc-reset-drops-input-macos-27.md). The app resets the guest connection
    // when the client has gone, so a command that closes at once loses its input.
    let _lock = serial();
    let fixture = Fixture::with(FakeRfb::start_reading_late(WIDTH, HEIGHT, LATE_READ), None);
    let run = fixture.app.cli(&["input", "click", "10", "5", "--vm", VM]);
    assert_eq!(run.code(), 0, "{}", run.err());
    // The log is read at once. The guarantee is that the guest has the input when the command
    // returns, not that it has it a moment later.
    assert_eq!(
        input_events(fixture.rfb.events()),
        vec![
            RfbEvent::Pointer {
                buttons: 1,
                x: 10,
                y: 5
            },
            RfbEvent::Pointer {
                buttons: 0,
                x: 10,
                y: 5
            },
        ]
    );
}

#[test]
fn typed_text_has_reached_a_guest_that_reads_late_when_the_command_returns() {
    let _lock = serial();
    let fixture = Fixture::with(FakeRfb::start_reading_late(WIDTH, HEIGHT, LATE_READ), None);
    let run = fixture.app.cli(&["input", "type", "ab", "--vm", VM]);
    assert_eq!(run.code(), 0, "{}", run.err());
    let keys: Vec<(bool, u32)> = input_events(fixture.rfb.events())
        .into_iter()
        .filter_map(|e| match e {
            RfbEvent::Key { down, keysym } => Some((down, keysym)),
            _ => None,
        })
        .collect();
    let (a, b) = ('a' as u32, 'b' as u32);
    assert_eq!(keys, vec![(true, a), (false, a), (true, b), (false, b)]);
}

#[test]
fn the_password_of_the_spec_reaches_the_handshake() {
    let _lock = serial();
    let fixture = Fixture::new(Some("s3cret"), None);
    routed_prints(&fixture, &["screen", "size", "--vm", VM], &size_text());
}

#[test]
fn screen_capture_through_the_app_writes_the_direct_png() {
    let _lock = serial();
    let fixture = Fixture::new(None, None);
    let png = fixture.app.root().join("capture.png");
    let png = png.to_str().unwrap();
    let args = ["screen", "capture", "--vm", VM, "-o", png];

    let routed = fixture.app.cli(&args);
    assert_eq!(routed.code(), 0, "{}", routed.err());
    assert_eq!(routed.out(), format!("wrote {png} ({WIDTH}x{HEIGHT})\n"));
    let routed_png = std::fs::read(png).unwrap();

    // The oracle: the PNG that the direct path wrote shows the frame of the guest, so each
    // pixel is the guest's own, with its bytes reversed (BGRX on the wire, RGBA in the file).
    let image = image::load_from_memory(&routed_png).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (u32::from(WIDTH), u32::from(HEIGHT)));
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let wire = [(x * 4) as u8, (y * 5) as u8, ((x + y) * 3) as u8];
            assert_eq!(
                image.get_pixel(u32::from(x), u32::from(y)).0,
                [wire[2], wire[1], wire[0], 255],
                "pixel ({x}, {y})"
            );
        }
    }
    // The PNG header holds the size: 8 bytes of signature, then IHDR.
    assert_eq!(&routed_png[1..4], b"PNG");

    if HOST_GOES_DIRECT {
        std::fs::remove_file(png).unwrap();
        let direct = fixture.app.cli_default(&args);
        assert_eq!(direct.code(), 0, "{}", direct.err());
        assert_eq!(direct.out(), routed.out());
        assert_eq!(
            std::fs::read(png).unwrap(),
            routed_png,
            "the two PNG files differ"
        );
    }
}

#[test]
fn a_key_press_through_the_app_reaches_the_guest() {
    let _lock = serial();
    let fixture = Fixture::new(None, None);
    let routed = fixture.app.cli(&["input", "key", "a", "--vm", VM]);
    assert_eq!(routed.code(), 0, "{}", routed.err());

    let events = fixture.rfb.wait_for(|e| {
        e.iter()
            .filter(|e| matches!(e, RfbEvent::Key { .. }))
            .count()
            >= 2
    });
    let keys: Vec<_> = events
        .iter()
        .filter(|e| matches!(e, RfbEvent::Key { .. }))
        .cloned()
        .collect();
    assert_eq!(
        keys,
        vec![
            RfbEvent::Key {
                down: true,
                keysym: 0x61
            },
            RfbEvent::Key {
                down: false,
                keysym: 0x61
            },
        ]
    );
}

#[test]
fn the_pointer_events_of_a_command_are_those_of_the_direct_path() {
    let _lock = serial();
    let fixture = Fixture::new(None, None);
    let args = ["input", "click", "7", "9", "--vm", VM];
    let pointers = |events: Vec<RfbEvent>| -> Vec<RfbEvent> {
        events
            .into_iter()
            .filter(|e| matches!(e, RfbEvent::Pointer { .. }))
            .collect()
    };
    // The oracle: a click is a press and a release at the point.
    let click = vec![
        RfbEvent::Pointer {
            buttons: 1,
            x: 7,
            y: 9,
        },
        RfbEvent::Pointer {
            buttons: 0,
            x: 7,
            y: 9,
        },
    ];

    let routed = fixture.app.cli(&args);
    assert_eq!(routed.code(), 0, "{}", routed.err());
    assert_eq!(routed.out(), "Clicked at (7, 9) button=left count=1\n");
    let routed_events = pointers(fixture.rfb.wait_for(|e| {
        e.iter()
            .any(|e| matches!(e, RfbEvent::Pointer { buttons: 0, .. }))
    }));
    assert_eq!(routed_events, click);

    if HOST_GOES_DIRECT {
        let direct = fixture.app.cli_default(&args);
        assert_eq!(direct.code(), 0, "{}", direct.err());
        assert_eq!(direct.out(), routed.out());
        let both = pointers(fixture.rfb.wait_for(|e| {
            e.iter()
                .filter(|e| matches!(e, RfbEvent::Pointer { .. }))
                .count()
                >= 2 * click.len()
        }));
        assert_eq!(both[click.len()..], click[..]);
    }
}

#[test]
fn an_explicit_vnc_endpoint_goes_through_the_app() {
    let _lock = serial();
    let fixture = Fixture::new(None, None);
    let endpoint = fixture.rfb.addr.to_string();
    routed_prints(
        &fixture,
        &["screen", "size", "--vnc", &endpoint],
        &size_text(),
    );
}

#[test]
fn an_explicit_vnc_endpoint_carries_its_password_from_the_environment() {
    let _lock = serial();
    let fixture = Fixture::new(Some("s3cret"), None);
    let endpoint = fixture.rfb.addr.to_string();
    let routed = fixture.app.cli_with(
        &["screen", "size", "--vnc", &endpoint],
        &[
            (
                "TESTANYWARE_APP_SOCKET",
                fixture.app.socket().to_str().unwrap(),
            ),
            ("TESTANYWARE_VNC_PASSWORD", "s3cret"),
        ],
    );
    assert_eq!(routed.code(), 0, "{}", routed.err());
    assert_eq!(routed.out().trim(), format!("{WIDTH}x{HEIGHT}"));
}

#[test]
fn a_connect_spec_goes_through_the_app_as_an_endpoint() {
    let _lock = serial();
    let fixture = Fixture::new(None, None);
    let spec = fixture.app.connect_spec("spec.json", &fixture.rfb);
    routed_prints(
        &fixture,
        &["screen", "size", "--connect", spec.to_str().unwrap()],
        &size_text(),
    );
}

#[cfg(target_os = "macos")]
#[test]
fn a_recording_through_the_app_reports_what_the_direct_recording_reported() {
    // The encoder is the native one of macOS (ADR-0006); other hosts report that they lack it.
    let _lock = serial();
    let fixture = Fixture::new(None, None);
    let output = fixture.app.root().join("rec.mp4");
    let output = output.to_str().unwrap();
    let args = [
        "screen",
        "record",
        "--vm",
        VM,
        "-o",
        output,
        "--fps",
        "5",
        "--duration",
        "1",
        "--json",
    ];

    let before = fixture.rfb.accepted();
    let routed = fixture.app.cli(&args);
    assert_eq!(routed.code(), 0, "{}\n{}", routed.out(), routed.err());
    assert!(
        fixture.rfb.accepted() > before,
        "the routed run reached no guest"
    );
    let routed_size = std::fs::metadata(output).unwrap().len();
    let routed = routed.json();

    // The oracle: the fields that the direct recording reported, with the values of the
    // arguments and of the frame.
    assert_eq!(routed["ok"], true, "{routed}");
    assert_eq!(routed["output"], output);
    assert_eq!(routed["fps"], 5);
    assert_eq!(routed["duration"], 1);
    assert_eq!(routed["width"], u32::from(WIDTH));
    assert_eq!(routed["height"], u32::from(HEIGHT));
    assert!(!routed["codec"].is_null(), "no codec in {routed}");
    assert!(routed["frames"].as_u64().unwrap() > 0, "{routed}");
    assert!(routed_size > 0);

    if HOST_GOES_DIRECT {
        std::fs::remove_file(output).unwrap();
        let direct = fixture.app.cli_default(&args);
        assert_eq!(direct.code(), 0, "{}\n{}", direct.out(), direct.err());
        let direct = direct.json();
        for field in ["output", "fps", "duration", "codec", "width", "height"] {
            assert_eq!(routed[field], direct[field], "field {field}");
        }
    }
}

// ---- the routing is real, and the refusals hold ----

#[test]
fn with_no_app_running_a_vnc_command_fails_and_dials_nothing() {
    // The guest would answer a direct dial, so a pass here means the CLI did not dial.
    let _lock = serial();
    let rfb = FakeRfb::start(WIDTH, HEIGHT, None);
    let app = TestApp::not_running();
    app.add_vnc_vm(VM, &rfb, None);

    for args in [
        vec!["screen", "size", "--vm", VM, "--json"],
        vec!["input", "key", "a", "--vm", VM, "--json"],
    ] {
        let out = app.cli(&args);
        assert_eq!(out.code(), 1, "{args:?}: {}", out.out());
        let envelope = out.json();
        assert_eq!(envelope["code"], "APP_UNREACHABLE", "{envelope}");
        assert_eq!(envelope["details"]["cause"], "not_running", "{envelope}");
    }
    let endpoint = rfb.addr.to_string();
    let out = app.cli(&["screen", "size", "--vnc", &endpoint, "--json"]);
    assert_eq!(out.json()["code"], "APP_UNREACHABLE");

    assert_eq!(rfb.accepted(), 0, "the CLI dialed the guest itself");
    // Positive control: the guest answers a dial, so a pass above means the CLI did not dial.
    drop(std::net::TcpStream::connect(rfb.addr).expect("the guest accepts a dial"));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while rfb.accepted() == 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "the guest never accepted"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(rfb.accepted(), 1);
}

#[test]
fn an_unknown_vm_fails_as_it_does_on_the_direct_path() {
    let _lock = serial();
    let fixture = Fixture::new(None, None);
    let args = ["screen", "size", "--vm", "testanyware-nope", "--json"];
    let routed = fixture.app.cli(&args);
    assert_eq!(routed.code(), 3);
    assert_eq!(routed.json()["code"], "VM_NOT_FOUND");
    if HOST_GOES_DIRECT {
        let direct = fixture.app.cli_default(&args);
        assert_eq!(direct.code(), 3);
        assert_eq!(routed.json()["code"], direct.json()["code"]);
    }
    assert_eq!(fixture.rfb.accepted(), 0);
}

#[test]
fn a_guest_that_refuses_the_connection_is_reported_with_the_code_it_has_today() {
    let _lock = serial();
    // Find a port that nothing listens on.
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let app = TestApp::start();
    let endpoint = format!("127.0.0.1:{port}");
    let args = ["screen", "size", "--vnc", &endpoint, "--json"];
    let routed = app.cli(&args);
    assert_eq!(routed.code(), 1);
    assert_eq!(routed.json()["code"], "CONNECTION_REFUSED");
    if HOST_GOES_DIRECT {
        let direct = app.cli_default(&args);
        assert_eq!(direct.code(), 1);
        assert_eq!(direct.json()["code"], "CONNECTION_REFUSED");
    }
}

#[test]
fn a_wrong_password_is_an_authentication_failure() {
    let _lock = serial();
    let fixture = Fixture::new(Some("s3cret"), None);
    let endpoint = fixture.rfb.addr.to_string();
    let wrong = [("TESTANYWARE_VNC_PASSWORD", "wrong")];
    let routed = fixture.app.cli_with(
        &["screen", "size", "--vnc", &endpoint, "--json"],
        &[
            (
                "TESTANYWARE_APP_SOCKET",
                fixture.app.socket().to_str().unwrap(),
            ),
            wrong[0],
        ],
    );
    assert_eq!(routed.code(), 4, "{}", routed.out());
    assert_eq!(routed.json()["code"], "AUTH_REQUIRED");
    if HOST_GOES_DIRECT {
        let direct = fixture
            .app
            .cli_with(&["screen", "size", "--vnc", &endpoint, "--json"], &wrong);
        assert_eq!(direct.code(), 4, "{}", direct.out());
    }
}

/// On a Linux or Windows host, a command without the variable dials the guest itself and needs
/// no app (spec: "Linux and Windows hosts keep the direct path").
#[cfg(not(target_os = "macos"))]
#[test]
fn a_linux_or_windows_host_without_the_variable_needs_no_app() {
    let _lock = serial();
    let rfb = FakeRfb::start(WIDTH, HEIGHT, None);
    let app = TestApp::not_running();
    app.add_vnc_vm(VM, &rfb, None);
    let out = app.cli_default(&["screen", "size", "--vm", VM]);
    assert_eq!(out.code(), 0, "{}", out.err());
    assert_eq!(out.out(), size_text());
}

/// On a macOS host, a command without the variable goes to the default socket of the app, and
/// the app dials the guest. With no app on that socket, it fails and dials nothing.
#[cfg(target_os = "macos")]
#[test]
fn a_macos_host_without_the_variable_goes_through_the_app_on_the_default_socket() {
    let _lock = serial();
    let rfb = FakeRfb::start(WIDTH, HEIGHT, None);
    let app = TestApp::start_on_the_default_socket();
    app.add_vnc_vm(VM, &rfb, None);
    let endpoint = rfb.addr.to_string();
    for args in [
        vec!["screen", "size", "--vm", VM],
        vec!["screen", "size", "--vnc", &endpoint],
    ] {
        let out = app.cli_default(&args);
        assert_eq!(out.code(), 0, "{args:?}: {}", out.err());
        assert_eq!(out.out(), size_text());
    }
    assert_eq!(rfb.accepted(), 2);
    drop(app);

    // The app is gone. The guest still answers, and the command does not reach it.
    let app = TestApp::not_running_on_the_default_socket();
    app.add_vnc_vm(VM, &rfb, None);
    let out = app.cli_default(&["screen", "size", "--vm", VM, "--json"]);
    assert_eq!(out.code(), 1, "{}", out.out());
    let code = out.json()["code"].clone();
    let installed = std::path::Path::new("/Applications/TestAnyware.app").exists();
    assert_eq!(
        code,
        if installed {
            "APP_UNREACHABLE"
        } else {
            "APP_NOT_INSTALLED"
        }
    );
    assert_eq!(rfb.accepted(), 2, "the CLI dialed the guest itself");
}

#[test]
fn many_vnc_commands_leave_the_descriptors_of_the_app_flat() {
    // tart's VNC server keeps a connection open after the client's end and sends nothing more.
    // A relay that waits for the guest to end its half then keeps both descriptors of the
    // stream, and an app at the default limit of 256 fails after about 120 commands.
    let _lock = serial();
    let rfb = FakeRfb::start_holding(WIDTH, HEIGHT);
    let app = TestApp::start();
    app.add_vnc_vm(VM, &rfb, None);

    // The first command warms the app up. The count after it is the baseline.
    let first = app.cli(&["screen", "size", "--vm", VM]);
    assert_eq!(first.code(), 0, "{}", first.err());
    settle(&rfb, 1);
    let baseline = app.open_descriptors();

    const COMMANDS: usize = 40;
    for _ in 0..COMMANDS {
        let run = app.cli(&["screen", "size", "--vm", VM]);
        assert_eq!(run.code(), 0, "{}", run.err());
    }
    settle(&rfb, COMMANDS + 1);

    // Count before the probe below: a byte that reaches a stuck relay makes it fail and free
    // its descriptors, which would hide the leak.
    let after = app.open_descriptors();
    assert!(
        after <= baseline + 2,
        "the app holds {after} descriptors after {COMMANDS} commands, and held {baseline} after one"
    );

    // The app closes each guest connection once its client has gone. The guest has no sign
    // of that but a reset, which a closed socket sends for the byte that the test writes.
    let closed = rfb.connections_the_peer_closed();
    assert_eq!(
        closed,
        COMMANDS + 1,
        "the guest holds {} connections that the app has not closed",
        COMMANDS + 1 - closed
    );
}

/// Wait until the holding guest has read the end of `clients` clients.
fn settle(rfb: &FakeRfb, clients: usize) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while rfb.client_ends() < clients {
        assert!(
            std::time::Instant::now() < deadline,
            "the guest read the end of {} clients, and {clients} were expected",
            rfb.client_ends()
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
