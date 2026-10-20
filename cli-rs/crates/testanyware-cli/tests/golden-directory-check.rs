//! `config.rhai` on a macOS host, and the directory check of golden creation (ADR-0021, spec
//! "Configuration" and "Golden creation keeps its direct path"), through the real Host CLI binary
//! and the real app executable in its test mode.
//!
//! The file sits at the default place of the CLI's `HOME`. The app reads it through `--config`,
//! and the CLI finds it with no variable, so the two read one file. No test lets golden creation
//! pass its check into the real pipeline: `HOME` holds no SSH key, so the pipeline stops at once
//! with `GOLDEN_CREATE_FAILED`, and the file names a `tart` that does not exist.

#![cfg(target_os = "macos")]

mod common;

use common::{serial, OutputExt, TestApp};
use serde_json::Value;

/// A `config.rhai` that moves both directories away from the defaults of the harness and from the
/// built-in ones, and names a `tart` that is not there.
fn moved(app: &TestApp) -> String {
    format!(
        r#"#{{ state_dir: {:?}, data_dir: {:?}, tools: #{{ tart: {:?} }} }}"#,
        app.root().join("file-state").display().to_string(),
        app.root().join("file-data").display().to_string(),
        app.root().join("no-such-tart").display().to_string(),
    )
}

const GOLDEN: &[&str] = &["vm", "create-golden", "--platform", "macos", "--json"];

fn assert_directories_mismatch(body: &Value) {
    assert_eq!(body["ok"], false, "{body}");
    assert_eq!(body["code"], "APP_INCOMPATIBLE", "{body}");
    assert_eq!(body["details"]["cause"], "directories", "{body}");
    assert!(body["details"]["app"]["state_dir"].is_string(), "{body}");
    assert!(body["details"]["client"]["data_dir"].is_string(), "{body}");
}

#[test]
fn a_command_succeeds_when_the_cli_and_the_app_read_the_same_file() {
    let _serial = serial();
    let app = TestApp::start_with_config(moved);
    let out = app.cli_without_xdg(
        &["vm", "list", "--json"],
        &[("TESTANYWARE_APP_SOCKET", app.socket().to_str().unwrap())],
    );
    assert_eq!(out.code(), 0, "{}\n{}", out.out(), out.err());
}

#[test]
fn a_command_fails_when_a_variable_moves_a_directory_that_the_app_took_from_the_file() {
    let _serial = serial();
    let app = TestApp::start_with_config(moved);
    let other = app.root().join("other-data");
    let out = app.cli_without_xdg(
        &["vm", "list", "--json"],
        &[
            ("TESTANYWARE_APP_SOCKET", app.socket().to_str().unwrap()),
            ("XDG_DATA_HOME", other.to_str().unwrap()),
        ],
    );
    assert_eq!(out.code(), 1, "{}\n{}", out.out(), out.err());
    let body = out.json();
    assert_directories_mismatch(&body);
    assert_eq!(
        body["details"]["app"]["data_dir"],
        app.root().join("file-data").display().to_string()
    );
    assert_eq!(
        body["details"]["client"]["data_dir"],
        other.join("testanyware").display().to_string()
    );
}

#[test]
fn golden_creation_passes_its_check_with_the_file_and_needs_no_app() {
    let _serial = serial();
    let app = TestApp::not_running_with_config(moved);
    let out = app.cli_without_xdg(GOLDEN, &[]);
    assert_eq!(out.code(), 1, "{}\n{}", out.out(), out.err());
    let body = out.json();
    // It got past the check and started: the pipeline looks for an SSH key first.
    assert_eq!(body["code"], "GOLDEN_CREATE_FAILED", "{body}");
    assert!(
        body["message"].as_str().unwrap().contains("SSH keypair"),
        "{body}"
    );
}

#[test]
fn golden_creation_fails_before_it_starts_when_its_directories_differ() {
    let _serial = serial();
    // The scenario of the spec: XDG_DATA_HOME is set and the file names a data_dir. The app does
    // not run, and the command fails all the same.
    let app = TestApp::not_running_with_config(moved);
    let other = app.root().join("other-data");
    let out = app.cli_without_xdg(GOLDEN, &[("XDG_DATA_HOME", other.to_str().unwrap())]);
    assert_eq!(out.code(), 1, "{}\n{}", out.out(), out.err());
    let body = out.json();
    assert_directories_mismatch(&body);
    // The error names both pairs and the remedy.
    let message = body["message"].as_str().unwrap();
    for pair in [
        app.root().join("file-state"),
        app.root().join("file-data"),
        other.join("testanyware"),
    ] {
        assert!(message.contains(pair.to_str().unwrap()), "{message}");
    }
    let remedy = body["remediation"].as_str().unwrap();
    assert!(remedy.contains("XDG_DATA_HOME"), "{remedy}");
    assert!(remedy.contains("config.rhai"), "{remedy}");
    // It did not start: the pipeline did not run, so it printed nothing of it.
    assert!(!out.err().contains("SSH key"), "{}", out.err());
    assert!(!out.err().contains("Using"), "{}", out.err());
}

#[test]
fn golden_creation_names_the_directories_that_the_app_names() {
    let _serial = serial();
    let app = TestApp::start_with_config(moved);
    let socket = app.socket();
    let other = app.root().join("other-data");
    let env = [("XDG_DATA_HOME", other.to_str().unwrap())];
    let with_socket = [
        ("XDG_DATA_HOME", other.to_str().unwrap()),
        ("TESTANYWARE_APP_SOCKET", socket.to_str().unwrap()),
    ];

    // What the app says about its own directories, in a command it refuses.
    let refused = app.cli_without_xdg(&["vm", "list", "--json"], &with_socket).json();
    assert_directories_mismatch(&refused);

    // What golden creation expects of the app, with the app running or not.
    let golden = app.cli_without_xdg(GOLDEN, &env).json();
    assert_directories_mismatch(&golden);
    assert_eq!(golden["details"]["app"], refused["details"]["app"]);
    assert_eq!(golden["details"]["client"], refused["details"]["client"]);
    assert_eq!(golden["remediation"], refused["remediation"]);
}

#[test]
fn golden_creation_expects_the_built_in_directories_when_no_file_names_them() {
    let _serial = serial();
    // The harness sets both XDG variables, and no file moves the app, so the CLI differs from
    // the app's defaults under HOME. The app does not run.
    let app = TestApp::not_running();
    let out = app.cli_with(GOLDEN, &[]);
    assert_eq!(out.code(), 1, "{}\n{}", out.out(), out.err());
    let body = out.json();
    assert_directories_mismatch(&body);
    let home = app.root();
    assert_eq!(
        body["details"]["app"]["state_dir"],
        home.join(".local/state/testanyware").display().to_string()
    );
    assert_eq!(
        body["details"]["app"]["data_dir"],
        home.join(".local/share/testanyware").display().to_string()
    );
}

#[test]
fn a_dry_run_changes_nothing_and_asks_no_directory_check() {
    let _serial = serial();
    let app = TestApp::not_running();
    let out = app.cli_with(
        &["vm", "create-golden", "--platform", "macos", "--dry-run", "--json"],
        &[],
    );
    assert_eq!(out.code(), 0, "{}\n{}", out.out(), out.err());
}

#[test]
fn a_file_that_fails_stops_an_ordinary_command_and_golden_creation_but_not_doctor() {
    let _serial = serial();
    let app = TestApp::start_with_config(|_| "let a = 1;\nlet b = ;\n#{}".to_string());
    let socket = app.socket();
    let file = app.config_file().display().to_string();
    let env = [("TESTANYWARE_APP_SOCKET", socket.to_str().unwrap())];

    for args in [&["vm", "list", "--json"][..], GOLDEN] {
        let out = app.cli_without_xdg(args, &env);
        assert_eq!(out.code(), 2, "{args:?}: {}\n{}", out.out(), out.err());
        let body = out.json();
        assert_eq!(body["code"], "CONFIG_INVALID", "{args:?}: {body}");
        assert_eq!(body["details"]["file"], file.as_str());
        assert_eq!(body["details"]["line"], 2);
    }
    assert!(!app.cli_without_xdg(GOLDEN, &env).err().contains("SSH key"));

    let out = app.cli_without_xdg(&["doctor", "--json"], &env);
    assert_eq!(out.code(), 1, "{}\n{}", out.out(), out.err());
    let body = out.json();
    assert_eq!(body["ok"], false);
    let config = &body["checks"]["config"];
    assert_eq!(config["status"], "fail", "{body}");
    assert_eq!(config["verdict"], "invalid");
    assert_eq!(config["error"]["code"], "CONFIG_INVALID");
    assert_eq!(config["error"]["details"]["file"], file.as_str());
}

#[test]
fn a_macos_host_reads_the_file_in_doctor() {
    let _serial = serial();
    let app = TestApp::not_running_with_config(moved);
    let out = app.cli_without_xdg(&["doctor", "--json"], &[]);
    let body = out.json();
    let config = &body["checks"]["config"];
    assert_eq!(config["status"], "pass", "{body}");
    assert_eq!(config["verdict"], "loaded");
    assert_eq!(config["file"], app.config_file().display().to_string());
}
