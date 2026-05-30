//! `agent {health|windows|snapshot|inspect|press}` handlers.

use serde_json::json;

use testanyware_protocol::{
    ActionResponse, AgentFormatter, ElementQuery, HealthResponse, InspectResponse, SetValueRequest,
    SnapshotRequest, SnapshotResponse, WaitRequest, WindowMoveRequest, WindowResizeRequest,
    WindowTarget,
};

use crate::commands::{build_agent_client, exit_agent_error};
use crate::output::{print_error, print_success, OutputMode};
use crate::resolve::ConnectionOptions;

pub async fn run_health(opts: ConnectionOptions, mode: OutputMode) {
    let client = build_agent_client(&opts, mode);
    match client.health().await {
        Ok(response) => emit_health(&response, mode),
        Err(err) => exit_agent_error(err, mode),
    }
}

fn emit_health(response: &HealthResponse, mode: OutputMode) {
    match mode {
        OutputMode::Json => {
            print_success(json!({
                "reachable": true,
                "accessibility_status": if response.accessible { "granted" } else { "denied" },
                "platform": response.platform,
            }));
        }
        OutputMode::Text => {
            // Mirrors Swift's "OK"/"UNHEALTHY" parity. The agent
            // distinction is "reachable but AX denied" → exit 4 so scripts
            // can detect the half-healthy state without parsing text.
            if response.accessible {
                println!("OK");
            } else {
                println!("UNHEALTHY: accessibility not granted");
            }
        }
    }
    if !response.accessible {
        std::process::exit(crate::output::exit_code_for("AUTH_REQUIRED"));
    }
}

pub async fn run_windows(opts: ConnectionOptions, mode: OutputMode) {
    let client = build_agent_client(&opts, mode);
    match client.windows().await {
        Ok(response) => emit_snapshot(&response, mode, /* flat windows-only */ true),
        Err(err) => exit_agent_error(err, mode),
    }
}

pub struct SnapshotArgs {
    pub mode_arg: Option<String>,
    pub window: Option<String>,
    pub role: Option<String>,
    pub label: Option<String>,
    pub depth: Option<i64>,
    pub open_menu: Option<String>,
}

pub async fn run_snapshot(opts: ConnectionOptions, args: SnapshotArgs, mode: OutputMode) {
    if args.open_menu.is_some() {
        // `--open-menu` requires VNC mouse clicks (see Swift's
        // `openMenuBarPath` in AgentCommand.swift). Until the RFB client
        // crate lands, the Rust port can't satisfy this flag. Surface a
        // clear error rather than silently ignoring it.
        print_error(
            mode,
            "ACTION_UNSUPPORTED",
            "--open-menu requires VNC support, which is not yet ported to the Rust CLI",
            Some("Use the Swift CLI for `--open-menu` for now, or use the canonical noun-first command tree once the RFB port lands."),
            json!({ "missing_capability": "vnc" }),
            crate::output::exit_code_for("ACTION_UNSUPPORTED"),
        );
    }
    let client = build_agent_client(&opts, mode);
    let request = SnapshotRequest {
        mode: args.mode_arg,
        window: args.window,
        role: args.role,
        label: args.label,
        depth: args.depth,
    };
    match client.snapshot(&request).await {
        Ok(response) => emit_snapshot(&response, mode, false),
        Err(err) => exit_agent_error(err, mode),
    }
}

fn emit_snapshot(response: &SnapshotResponse, mode: OutputMode, windows_only: bool) {
    match mode {
        OutputMode::Json => {
            // Use the wire-format JSON value directly so the output matches
            // the agent-snapshot / agent-windows schema byte-for-byte.
            let inner = serde_json::to_value(response).expect("serialize snapshot");
            print_success(json!({
                "windows": inner["windows"],
            }));
        }
        OutputMode::Text => {
            let body = if windows_only {
                AgentFormatter::format_windows(response)
            } else {
                AgentFormatter::format_snapshot(response)
            };
            println!("{body}");
        }
    }
}

pub struct ElementQueryArgs {
    pub role: String,
    pub label: Option<String>,
    pub window: Option<String>,
    pub id: Option<String>,
    pub index: Option<i64>,
}

impl From<ElementQueryArgs> for ElementQuery {
    fn from(a: ElementQueryArgs) -> Self {
        ElementQuery {
            role: Some(a.role),
            label: a.label,
            window: a.window,
            id: a.id,
            index: a.index,
        }
    }
}

pub async fn run_inspect(opts: ConnectionOptions, args: ElementQueryArgs, mode: OutputMode) {
    let client = build_agent_client(&opts, mode);
    let query: ElementQuery = args.into();
    match client.inspect(&query).await {
        Ok(response) => emit_inspect(&response, mode),
        Err(err) => exit_agent_error(err, mode),
    }
}

fn emit_inspect(response: &InspectResponse, mode: OutputMode) {
    match mode {
        OutputMode::Json => {
            let inner = serde_json::to_value(response).expect("serialize inspect");
            print_success(inner);
        }
        OutputMode::Text => {
            println!("{}", AgentFormatter::format_inspect(response));
        }
    }
}

pub async fn run_press(
    opts: ConnectionOptions,
    args: ElementQueryArgs,
    mode: OutputMode,
    dry_run: bool,
) {
    let query: ElementQuery = args.into();
    if dry_run {
        emit_dry_run_action(&query, "press", mode);
        return;
    }
    let client = build_agent_client(&opts, mode);
    match client.press(&query).await {
        Ok(response) => emit_action(&response, "press", mode),
        Err(err) => exit_agent_error(err, mode),
    }
}

fn emit_action(response: &ActionResponse, action: &str, mode: OutputMode) {
    match mode {
        OutputMode::Json => {
            print_success(json!({
                "action": action,
                "success": response.success,
                "message": response.message,
            }));
        }
        OutputMode::Text => {
            println!("{}", AgentFormatter::format_action(response));
        }
    }
    if !response.success {
        std::process::exit(crate::output::exit_code_for("ACTION_UNSUPPORTED"));
    }
}

fn emit_dry_run_action(query: &ElementQuery, action: &str, mode: OutputMode) {
    match mode {
        OutputMode::Json => {
            print_success(json!({
                "action": action,
                "dry_run": true,
                "query": query,
            }));
        }
        OutputMode::Text => {
            println!("DRY-RUN: would {action} on {query:?}");
        }
    }
}

// -------------------------------------------------------------------------
// Agent action parity (port leaf 010): focus, set-value, wait, window-*.
// Each mirrors the Swift `agent` subcommand: dispatch one HTTP call and emit
// the shared action receipt (`agent-action` / `agent-window-action` schema).
// -------------------------------------------------------------------------

pub async fn run_focus(
    opts: ConnectionOptions,
    args: ElementQueryArgs,
    mode: OutputMode,
    dry_run: bool,
) {
    let query: ElementQuery = args.into();
    if dry_run {
        emit_dry_run_action(&query, "focus", mode);
        return;
    }
    let client = build_agent_client(&opts, mode);
    match client.focus(&query).await {
        Ok(response) => emit_action(&response, "focus", mode),
        Err(err) => exit_agent_error(err, mode),
    }
}

pub struct SetValueCmdArgs {
    pub query: ElementQueryArgs,
    pub value: String,
}

pub async fn run_set_value(
    opts: ConnectionOptions,
    args: SetValueCmdArgs,
    mode: OutputMode,
    dry_run: bool,
) {
    let query: ElementQuery = args.query.into();
    let value = args.value;
    if dry_run {
        match mode {
            OutputMode::Json => print_success(json!({
                "action": "set-value",
                "dry_run": true,
                "query": query,
                "value": value,
            })),
            OutputMode::Text => println!("DRY-RUN: would set-value on {query:?} = {value:?}"),
        }
        return;
    }
    let client = build_agent_client(&opts, mode);
    let request = SetValueRequest { query, value };
    match client.set_value(&request).await {
        Ok(response) => emit_action(&response, "set-value", mode),
        Err(err) => exit_agent_error(err, mode),
    }
}

pub struct WaitCmdArgs {
    pub window: Option<String>,
    pub timeout: Option<i64>,
}

/// `agent wait` is read-only (it polls until accessibility is ready), so it
/// has no `--dry-run` per contract §9.3.
pub async fn run_wait(opts: ConnectionOptions, args: WaitCmdArgs, mode: OutputMode) {
    let client = build_agent_client(&opts, mode);
    let request = WaitRequest {
        window: args.window,
        timeout: args.timeout,
    };
    match client.wait(&request).await {
        Ok(response) => emit_action(&response, "wait", mode),
        Err(err) => exit_agent_error(err, mode),
    }
}

pub async fn run_window_focus(
    opts: ConnectionOptions,
    window: String,
    mode: OutputMode,
    dry_run: bool,
) {
    if dry_run {
        emit_window_dry_run("window-focus", json!({ "window": window }), mode);
        return;
    }
    let client = build_agent_client(&opts, mode);
    match client.window_focus(&WindowTarget { window }).await {
        Ok(response) => emit_action(&response, "window-focus", mode),
        Err(err) => exit_agent_error(err, mode),
    }
}

pub async fn run_window_close(
    opts: ConnectionOptions,
    window: String,
    mode: OutputMode,
    dry_run: bool,
) {
    if dry_run {
        emit_window_dry_run("window-close", json!({ "window": window }), mode);
        return;
    }
    let client = build_agent_client(&opts, mode);
    match client.window_close(&WindowTarget { window }).await {
        Ok(response) => emit_action(&response, "window-close", mode),
        Err(err) => exit_agent_error(err, mode),
    }
}

pub async fn run_window_minimize(
    opts: ConnectionOptions,
    window: String,
    mode: OutputMode,
    dry_run: bool,
) {
    if dry_run {
        emit_window_dry_run("window-minimize", json!({ "window": window }), mode);
        return;
    }
    let client = build_agent_client(&opts, mode);
    match client.window_minimize(&WindowTarget { window }).await {
        Ok(response) => emit_action(&response, "window-minimize", mode),
        Err(err) => exit_agent_error(err, mode),
    }
}

pub async fn run_window_resize(
    opts: ConnectionOptions,
    window: String,
    width: i64,
    height: i64,
    mode: OutputMode,
    dry_run: bool,
) {
    if dry_run {
        emit_window_dry_run(
            "window-resize",
            json!({ "window": window, "width": width, "height": height }),
            mode,
        );
        return;
    }
    let client = build_agent_client(&opts, mode);
    match client
        .window_resize(&WindowResizeRequest {
            window,
            width,
            height,
        })
        .await
    {
        Ok(response) => emit_action(&response, "window-resize", mode),
        Err(err) => exit_agent_error(err, mode),
    }
}

pub async fn run_window_move(
    opts: ConnectionOptions,
    window: String,
    x: i64,
    y: i64,
    mode: OutputMode,
    dry_run: bool,
) {
    if dry_run {
        emit_window_dry_run("window-move", json!({ "window": window, "x": x, "y": y }), mode);
        return;
    }
    let client = build_agent_client(&opts, mode);
    match client.window_move(&WindowMoveRequest { window, x, y }).await {
        Ok(response) => emit_action(&response, "window-move", mode),
        Err(err) => exit_agent_error(err, mode),
    }
}

fn emit_window_dry_run(action: &str, target: serde_json::Value, mode: OutputMode) {
    match mode {
        OutputMode::Json => {
            print_success(json!({
                "action": action,
                "dry_run": true,
                "target": target,
            }));
        }
        OutputMode::Text => {
            println!("DRY-RUN: would {action} {target}");
        }
    }
}
