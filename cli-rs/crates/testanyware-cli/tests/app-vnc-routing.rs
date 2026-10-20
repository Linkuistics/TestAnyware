//! The VNC commands routed through the TestAnyware app (spec: "The protocol").
//!
//! Each test runs the real `testanyware` binary against the real `testanyware-app` executable
//! in its test mode, with a synthetic RFB server on loopback as the guest. `TESTANYWARE_APP_SOCKET`
//! turns the routing on. A command through the app must print and do what the same command does
//! on the direct path, and it must dial nothing itself.

#![cfg(unix)]

mod common;

use common::{serial, FakeRfb, OutputExt, RfbEvent, TestApp};

const VM: &str = "testanyware-feed0002";
const WIDTH: u16 = 64;
const HEIGHT: u16 = 48;
/// The side of the square frame of the recording test.
const RECORD_SIDE: u16 = 16;

/// A synthetic RFB server, and an app that serves its VM.
struct Fixture {
    rfb: FakeRfb,
    app: TestApp,
}

impl Fixture {
    /// The fakes start first, and the app spawns once (see the rules in `common`).
    fn new(password: Option<&str>, logical: Option<(u32, u32)>) -> Self {
        Self::sized(WIDTH, HEIGHT, password, logical)
    }

    fn sized(width: u16, height: u16, password: Option<&str>, logical: Option<(u32, u32)>) -> Self {
        let rfb = FakeRfb::start(width, height, password);
        let app = TestApp::start();
        app.add_vnc_vm(VM, &rfb, logical);
        Fixture { rfb, app }
    }
}

/// Run `args` on both paths and require the same output. Returns the stdout.
fn same_on_both_paths(fixture: &Fixture, args: &[&str]) -> String {
    let before = fixture.rfb.accepted();
    let direct = fixture.app.cli_direct(args);
    let after_direct = fixture.rfb.accepted();
    let routed = fixture.app.cli(args);
    let after_routed = fixture.rfb.accepted();

    assert_eq!(direct.code(), 0, "direct {args:?}: {}", direct.err());
    assert_eq!(routed.code(), 0, "routed {args:?}: {}", routed.err());
    assert_eq!(routed.out(), direct.out(), "stdout of {args:?}");
    assert_eq!(routed.err(), direct.err(), "stderr of {args:?}");
    assert!(
        !direct.out().is_empty(),
        "{args:?} printed nothing, so the comparison is empty"
    );
    assert!(
        after_direct > before,
        "the direct run did not reach the fake RFB server"
    );
    assert!(
        after_routed > after_direct,
        "the routed run did not reach the fake RFB server"
    );
    direct.out()
}

#[test]
fn screen_size_through_the_app_prints_the_direct_output() {
    let _lock = serial();
    let fixture = Fixture::new(None, None);
    let text = same_on_both_paths(&fixture, &["screen", "size", "--vm", VM]);
    assert_eq!(text.trim(), format!("{WIDTH}x{HEIGHT}"));
    same_on_both_paths(&fixture, &["screen", "size", "--vm", VM, "--json"]);
}

#[test]
fn the_hidpi_logical_target_of_the_spec_still_applies() {
    // The Host CLI routes by the id, and still reads the spec for the logical target
    // (ADR-0016). A 64x48 wire frame with a 32x24 target is a 2x frame.
    let _lock = serial();
    let fixture = Fixture::new(None, Some((32, 24)));
    let text = same_on_both_paths(&fixture, &["screen", "size", "--vm", VM]);
    assert_eq!(text.trim(), "32x24");

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

#[test]
fn the_password_of_the_spec_reaches_the_handshake() {
    let _lock = serial();
    let fixture = Fixture::new(Some("s3cret"), None);
    let text = same_on_both_paths(&fixture, &["screen", "size", "--vm", VM]);
    assert_eq!(text.trim(), format!("{WIDTH}x{HEIGHT}"));
}

#[test]
fn screen_capture_through_the_app_writes_the_direct_png() {
    let _lock = serial();
    let fixture = Fixture::new(None, None);
    let png = fixture.app.root().join("capture.png");
    let png = png.to_str().unwrap();
    let args = ["screen", "capture", "--vm", VM, "-o", png];

    let direct = fixture.app.cli_direct(&args);
    assert_eq!(direct.code(), 0, "{}", direct.err());
    let direct_png = std::fs::read(png).unwrap();
    std::fs::remove_file(png).unwrap();

    let routed = fixture.app.cli(&args);
    assert_eq!(routed.code(), 0, "{}", routed.err());
    let routed_png = std::fs::read(png).unwrap();

    assert_eq!(routed.out(), direct.out());
    assert_eq!(routed_png, direct_png, "the two PNG files differ");
    // The PNG header holds the size: 8 bytes of signature, then IHDR.
    assert_eq!(&routed_png[1..4], b"PNG");
    assert_eq!(
        u32::from_be_bytes(routed_png[16..20].try_into().unwrap()),
        u32::from(WIDTH)
    );
    assert_eq!(
        u32::from_be_bytes(routed_png[20..24].try_into().unwrap()),
        u32::from(HEIGHT)
    );
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
fn the_pointer_events_of_a_command_are_the_same_on_both_paths() {
    let _lock = serial();
    let fixture = Fixture::new(None, None);
    let args = ["input", "click", "7", "9", "--vm", VM];
    let pointers = |events: Vec<RfbEvent>| -> Vec<RfbEvent> {
        events
            .into_iter()
            .filter(|e| matches!(e, RfbEvent::Pointer { .. }))
            .collect()
    };

    let direct = fixture.app.cli_direct(&args);
    assert_eq!(direct.code(), 0, "{}", direct.err());
    let direct_events = pointers(fixture.rfb.wait_for(|e| {
        e.iter()
            .any(|e| matches!(e, RfbEvent::Pointer { buttons: 0, .. }))
    }));
    assert!(!direct_events.is_empty());

    let routed = fixture.app.cli(&args);
    assert_eq!(routed.code(), 0, "{}", routed.err());
    let both = pointers(fixture.rfb.wait_for(|e| {
        e.iter()
            .filter(|e| matches!(e, RfbEvent::Pointer { .. }))
            .count()
            >= 2 * direct_events.len()
    }));

    assert_eq!(routed.out(), direct.out());
    assert_eq!(both.len(), 2 * direct_events.len(), "{both:?}");
    assert_eq!(both[direct_events.len()..], direct_events[..]);
}

#[test]
fn an_explicit_vnc_endpoint_goes_through_the_app() {
    let _lock = serial();
    let fixture = Fixture::new(None, None);
    let endpoint = fixture.rfb.addr.to_string();
    let text = same_on_both_paths(&fixture, &["screen", "size", "--vnc", &endpoint]);
    assert_eq!(text.trim(), format!("{WIDTH}x{HEIGHT}"));
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
    let text = same_on_both_paths(
        &fixture,
        &["screen", "size", "--connect", spec.to_str().unwrap()],
    );
    assert_eq!(text.trim(), format!("{WIDTH}x{HEIGHT}"));
}

#[cfg(target_os = "macos")]
#[test]
fn a_recording_through_the_app_matches_the_direct_recording() {
    // The encoder is the native one of macOS (ADR-0006); other hosts report that they lack it.
    //
    // The frame is small on purpose. The recorder reads server messages inside a `select!`
    // that also fires its timers, and a read that a timer cancels halfway loses its bytes. A
    // slow server breaks the direct path the same way, so the defect is not the app's.
    // An update of this size crosses each hop in one piece. See `rfb-cancel-safe-reads-k41`.
    let _lock = serial();
    let fixture = Fixture::sized(RECORD_SIDE, RECORD_SIDE, None, None);
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

    let direct = fixture.app.cli_direct(&args);
    assert_eq!(direct.code(), 0, "{}\n{}", direct.out(), direct.err());
    let direct_size = std::fs::metadata(output).unwrap().len();
    std::fs::remove_file(output).unwrap();
    let before = fixture.rfb.accepted();

    let routed = fixture.app.cli(&args);
    assert_eq!(routed.code(), 0, "{}\n{}", routed.out(), routed.err());
    assert!(
        fixture.rfb.accepted() > before,
        "the routed run reached no guest"
    );
    let routed_size = std::fs::metadata(output).unwrap().len();

    let (direct, routed) = (direct.json(), routed.json());
    for field in ["output", "fps", "duration", "codec", "width", "height"] {
        assert!(!direct[field].is_null(), "no field {field} in {direct}");
        assert_eq!(routed[field], direct[field], "field {field}");
    }
    assert_eq!(routed["width"], u32::from(RECORD_SIDE));
    assert!(routed["frames"].as_u64().unwrap() > 0, "{routed}");
    assert!(direct_size > 0 && routed_size > 0);
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
    // And it is the variable that does it: without it, the guest answers.
    let direct = app.cli_direct(&["screen", "size", "--vm", VM]);
    assert_eq!(direct.code(), 0, "{}", direct.err());
    assert_eq!(rfb.accepted(), 1);
}

#[test]
fn an_unknown_vm_fails_as_it_does_on_the_direct_path() {
    let _lock = serial();
    let fixture = Fixture::new(None, None);
    let args = ["screen", "size", "--vm", "testanyware-nope", "--json"];
    let direct = fixture.app.cli_direct(&args);
    let routed = fixture.app.cli(&args);
    assert_eq!(direct.code(), 3);
    assert_eq!(routed.code(), 3);
    assert_eq!(routed.json()["code"], "VM_NOT_FOUND");
    assert_eq!(routed.json()["code"], direct.json()["code"]);
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
    let direct = app.cli_direct(&args);
    let routed = app.cli(&args);
    assert_eq!(direct.code(), 1);
    assert_eq!(routed.code(), 1);
    assert_eq!(direct.json()["code"], "CONNECTION_REFUSED");
    assert_eq!(routed.json()["code"], "CONNECTION_REFUSED");
}

#[test]
fn a_wrong_password_is_an_authentication_failure_on_both_paths() {
    let _lock = serial();
    let fixture = Fixture::new(Some("s3cret"), None);
    let endpoint = fixture.rfb.addr.to_string();
    let wrong = [("TESTANYWARE_VNC_PASSWORD", "wrong")];
    let direct = fixture
        .app
        .cli_with(&["screen", "size", "--vnc", &endpoint, "--json"], &wrong);
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
    assert_eq!(direct.code(), 4, "{}", direct.out());
    assert_eq!(routed.code(), 4, "{}", routed.out());
    assert_eq!(routed.json()["code"], "AUTH_REQUIRED");
}

#[test]
fn without_the_variable_nothing_needs_the_app() {
    let _lock = serial();
    let rfb = FakeRfb::start(WIDTH, HEIGHT, None);
    let app = TestApp::not_running();
    app.add_vnc_vm(VM, &rfb, None);
    let out = app.cli_direct(&["screen", "size", "--vm", VM]);
    assert_eq!(out.code(), 0, "{}", out.err());
    assert_eq!(out.out().trim(), format!("{WIDTH}x{HEIGHT}"));
}
