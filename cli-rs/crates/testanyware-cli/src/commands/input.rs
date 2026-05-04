//! `input` command handlers — keyboard, mouse, scroll, and drag.
//!
//! Each handler opens one short-lived RFB connection, runs the
//! handshake, sends the input message(s), then disconnects. This
//! mirrors the Swift CLI's per-invocation lifecycle. Long-lived sessions
//! land later, with the embedded viewer.

use serde_json::json;
use testanyware_rfb::{InputError, KeymapError, Platform, RfbConnection, RfbError};

use crate::output::{print_error, print_success, OutputMode};
use crate::resolve::{resolve_vnc, ConnectionOptions, ResolveError, ResolvedVnc};

/// Connect to the resolved VNC endpoint, or print a typed error and exit.
async fn connect_or_exit(
    opts: &ConnectionOptions,
    mode: OutputMode,
) -> RfbConnection<tokio::io::BufReader<tokio::net::TcpStream>> {
    let endpoint: ResolvedVnc = match resolve_vnc(opts) {
        Ok(e) => e,
        Err(err) => exit_resolve_error(err, mode),
    };
    match RfbConnection::connect(
        &endpoint.host,
        endpoint.port,
        endpoint.password.as_deref().map(str::as_bytes),
    )
    .await
    {
        Ok(c) => c,
        Err(err) => exit_rfb_error(err, mode),
    }
}

/// Resolve the platform from the connection options. Defaults to macOS
/// when neither `--platform` nor `TESTANYWARE_PLATFORM` is set, matching
/// the Swift CLI's `Platform.macos` default.
fn resolve_platform(opts: &ConnectionOptions, mode: OutputMode) -> Platform {
    match opts.platform.as_deref() {
        None => Platform::Macos,
        Some(name) => match Platform::from_name(name) {
            Some(p) => p,
            None => print_error(
                mode,
                "INVALID_PLATFORM",
                &format!("unknown platform '{name}'; expected macos|linux|windows"),
                Some("Pass --platform macos, --platform linux, or --platform windows."),
                json!({ "value": name }),
                2,
            ),
        },
    }
}

// ---- Keyboard --------------------------------------------------------------

pub async fn run_key(
    opts: ConnectionOptions,
    key: String,
    modifiers: Vec<String>,
    mode: OutputMode,
) {
    let platform = resolve_platform(&opts, mode);
    let mut conn = connect_or_exit(&opts, mode).await;
    let mod_refs: Vec<&str> = modifiers.iter().map(String::as_str).collect();
    if let Err(err) = conn.press_key(&key, &mod_refs, platform).await {
        exit_input_error(err, mode);
    }
    emit_text_or_json(
        mode,
        || {
            if modifiers.is_empty() {
                println!("Key pressed: {key}");
            } else {
                println!("Key pressed: {key} + {}", modifiers.join("+"));
            }
        },
        json!({ "key": key, "modifiers": modifiers }),
    );
}

pub async fn run_key_down(opts: ConnectionOptions, key: String, mode: OutputMode) {
    let mut conn = connect_or_exit(&opts, mode).await;
    if let Err(err) = conn.key_down_named(&key).await {
        exit_input_error(err, mode);
    }
    emit_text_or_json(mode, || println!("Key down: {key}"), json!({ "key": key }));
}

pub async fn run_key_up(opts: ConnectionOptions, key: String, mode: OutputMode) {
    let mut conn = connect_or_exit(&opts, mode).await;
    if let Err(err) = conn.key_up_named(&key).await {
        exit_input_error(err, mode);
    }
    emit_text_or_json(mode, || println!("Key up: {key}"), json!({ "key": key }));
}

pub async fn run_type(opts: ConnectionOptions, text: String, mode: OutputMode) {
    let mut conn = connect_or_exit(&opts, mode).await;
    if let Err(err) = conn.type_text(&text).await {
        exit_input_error(err, mode);
    }
    emit_text_or_json(
        mode,
        || println!("Typed: {text}"),
        json!({ "text": text, "chars": text.chars().count() }),
    );
}

// ---- Mouse -----------------------------------------------------------------

pub async fn run_click(
    opts: ConnectionOptions,
    x: i32,
    y: i32,
    button: String,
    count: u32,
    mode: OutputMode,
) {
    let (x, y) = match clamp_coords(x, y, mode) {
        Some(p) => p,
        None => return,
    };
    let mut conn = connect_or_exit(&opts, mode).await;
    if let Err(err) = conn.click(x, y, &button, count).await {
        exit_input_error(err, mode);
    }
    emit_text_or_json(
        mode,
        || println!("Clicked at ({x}, {y}) button={button} count={count}"),
        json!({ "x": x, "y": y, "button": button, "count": count }),
    );
}

pub async fn run_mouse_down(
    opts: ConnectionOptions,
    x: i32,
    y: i32,
    button: String,
    mode: OutputMode,
) {
    let (x, y) = match clamp_coords(x, y, mode) {
        Some(p) => p,
        None => return,
    };
    let mut conn = connect_or_exit(&opts, mode).await;
    if let Err(err) = conn.mouse_down(x, y, &button).await {
        exit_input_error(err, mode);
    }
    emit_text_or_json(
        mode,
        || println!("Mouse down at ({x}, {y}) button={button}"),
        json!({ "x": x, "y": y, "button": button }),
    );
}

pub async fn run_mouse_up(
    opts: ConnectionOptions,
    x: i32,
    y: i32,
    button: String,
    mode: OutputMode,
) {
    let (x, y) = match clamp_coords(x, y, mode) {
        Some(p) => p,
        None => return,
    };
    let mut conn = connect_or_exit(&opts, mode).await;
    if let Err(err) = conn.mouse_up(x, y, &button).await {
        exit_input_error(err, mode);
    }
    emit_text_or_json(
        mode,
        || println!("Mouse up at ({x}, {y}) button={button}"),
        json!({ "x": x, "y": y, "button": button }),
    );
}

pub async fn run_move(opts: ConnectionOptions, x: i32, y: i32, mode: OutputMode) {
    let (x, y) = match clamp_coords(x, y, mode) {
        Some(p) => p,
        None => return,
    };
    let mut conn = connect_or_exit(&opts, mode).await;
    if let Err(err) = conn.mouse_move(x, y).await {
        exit_input_error(err, mode);
    }
    emit_text_or_json(
        mode,
        || println!("Mouse moved to ({x}, {y})"),
        json!({ "x": x, "y": y }),
    );
}

pub async fn run_scroll(
    opts: ConnectionOptions,
    x: i32,
    y: i32,
    dx: i32,
    dy: i32,
    mode: OutputMode,
) {
    let (x, y) = match clamp_coords(x, y, mode) {
        Some(p) => p,
        None => return,
    };
    let mut conn = connect_or_exit(&opts, mode).await;
    if let Err(err) = conn.scroll(x, y, dx, dy).await {
        exit_input_error(err, mode);
    }
    emit_text_or_json(
        mode,
        || println!("Scrolled at ({x}, {y}) dx={dx} dy={dy}"),
        json!({ "x": x, "y": y, "dx": dx, "dy": dy }),
    );
}

pub async fn run_drag(
    opts: ConnectionOptions,
    from_x: i32,
    from_y: i32,
    to_x: i32,
    to_y: i32,
    button: String,
    steps: u32,
    mode: OutputMode,
) {
    let (from_x, from_y) = match clamp_coords(from_x, from_y, mode) {
        Some(p) => p,
        None => return,
    };
    let (to_x, to_y) = match clamp_coords(to_x, to_y, mode) {
        Some(p) => p,
        None => return,
    };
    let mut conn = connect_or_exit(&opts, mode).await;
    if let Err(err) = conn.drag(from_x, from_y, to_x, to_y, &button, steps).await {
        exit_input_error(err, mode);
    }
    emit_text_or_json(
        mode,
        || println!("Dragged from ({from_x},{from_y}) to ({to_x},{to_y})"),
        json!({
            "from": { "x": from_x, "y": from_y },
            "to":   { "x": to_x,   "y": to_y },
            "button": button,
            "steps": steps,
        }),
    );
}

// ---- Helpers --------------------------------------------------------------

/// Coordinates arrive as `i32` from clap so users get a clean error on
/// `--500`, but the RFB wire format is `u16`. We reject negatives and
/// >65535 with USAGE_ERROR rather than silently wrapping.
fn clamp_coords(x: i32, y: i32, mode: OutputMode) -> Option<(u16, u16)> {
    if x < 0 || y < 0 || x > u16::MAX as i32 || y > u16::MAX as i32 {
        print_error(
            mode,
            "USAGE_ERROR",
            &format!("coordinate out of range: ({x}, {y})"),
            Some("RFB framebuffer coordinates are 0..=65535."),
            json!({ "x": x, "y": y }),
            2,
        );
    }
    Some((x as u16, y as u16))
}

fn emit_text_or_json(mode: OutputMode, on_text: impl FnOnce(), json_payload: serde_json::Value) {
    match mode {
        OutputMode::Text => on_text(),
        OutputMode::Json => print_success(json_payload),
    }
}

fn exit_resolve_error(err: ResolveError, mode: OutputMode) -> ! {
    let code = err.code();
    let message = err.to_string();
    print_error(mode, code, &message, None, err.details(), err.exit_code());
}

fn exit_rfb_error(err: RfbError, mode: OutputMode) -> ! {
    let (code, exit_code) = match &err {
        RfbError::Io(_) => ("CONNECTION_REFUSED", 1),
        RfbError::UnsupportedProtocolVersion(_) => ("INTERNAL", 1),
        RfbError::SecurityNegotiationFailed(_)
        | RfbError::NoMutualSecurityType(_)
        | RfbError::AuthFailed(_)
        | RfbError::PasswordRequired => ("AUTH_REQUIRED", 4),
        RfbError::InvalidFramebufferSize { .. }
        | RfbError::UnexpectedMessageType(_)
        | RfbError::UnsupportedEncoding(_)
        | RfbError::Protocol(_) => ("INTERNAL", 1),
    };
    print_error(mode, code, &err.to_string(), None, json!({}), exit_code);
}

fn exit_input_error(err: InputError, mode: OutputMode) -> ! {
    match err {
        InputError::Keymap(KeymapError::UnknownKey(name)) => print_error(
            mode,
            "UNKNOWN_KEY",
            &format!("unknown key: '{name}'"),
            Some("See `testanyware --help` for the list of accepted key names."),
            json!({ "value": name }),
            2,
        ),
        InputError::Keymap(KeymapError::UnknownButton(name)) => print_error(
            mode,
            "UNKNOWN_BUTTON",
            &format!("unknown mouse button: '{name}'"),
            Some("Accepted: left, right, middle (alias center)."),
            json!({ "value": name }),
            2,
        ),
        InputError::Rfb(rfb) => exit_rfb_error(rfb, mode),
    }
}

// `clamp_coords` and the per-handler exits all terminate via
// `print_error` → `std::process::exit`, which makes them un-unit-testable
// without spawning a subprocess. Their behaviour is exercised by the
// process-driven cli-contract tests.
