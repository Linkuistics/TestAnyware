//! The `app` check of `doctor` (spec: "What the commands report about the app"), through the real
//! Host CLI binary and the real app executable in its test mode.
//!
//! The report is judged by `checks.app` and by the exit code of a failing item. A test does not
//! judge `ok` of a healthy app, because the other checks of `doctor` look at the developer's own
//! Homebrew install and the machine may fail them.

#![cfg(unix)]

mod common;

use common::{serial, OutputExt, TestApp};
use serde_json::Value;

/// `doctor --json` against `app`, with the CLI pointed at the app's socket and directories.
fn doctor(app: &TestApp) -> (i32, Value) {
    let out = app.cli(&["doctor", "--json"]);
    (out.code(), out.json())
}

fn app_check(body: &Value) -> &Value {
    &body["checks"]["app"]
}

#[cfg(target_os = "macos")]
mod on_a_macos_host {
    use super::*;

    const BAD_FILE: &str = "let a = 1;\nlet b = ;\n#{}";

    #[test]
    fn a_running_app_is_reported_with_its_build_range_directories_and_configuration() {
        let _serial = serial();
        let app = TestApp::start();
        let (_, body) = doctor(&app);
        let check = app_check(&body);
        assert_ne!(check["status"], "fail", "{check}");
        assert_eq!(check["socket"], app.socket().display().to_string().as_str());

        let running = &check["running"];
        assert_eq!(running["status"], "pass", "{check}");
        assert_eq!(running["build"], env!("CARGO_PKG_VERSION"));
        assert_eq!(running["protocol_range"]["min"], 1);
        assert_eq!(running["protocol_range"]["max"], 1);
        assert!(running["executable"].as_str().unwrap().ends_with("testanyware-app"));

        assert_eq!(check["directories"]["verdict"], "match", "{check}");
        assert_eq!(check["versions"]["verdict"], "match", "{check}");
        assert_eq!(check["configuration"]["verdict"], "disabled", "{check}");
        // The test mode has no login item, and no guest has shown the grant yet.
        assert_eq!(check["login_item"]["verdict"], "unobserved", "{check}");
        assert_eq!(check["local_network"]["verdict"], "not_yet_seen", "{check}");
        assert!(check["local_network"]["remedy"].is_string());
    }

    #[test]
    fn other_directories_are_a_failing_check_and_not_an_error_envelope() {
        let _serial = serial();
        let app = TestApp::start();
        let other = app.root().join("other-data");
        let out = app.cli_with(
            &["doctor", "--json"],
            &[
                ("TESTANYWARE_APP_SOCKET", app.socket().to_str().unwrap()),
                ("XDG_DATA_HOME", other.to_str().unwrap()),
            ],
        );
        assert_eq!(out.code(), 1, "{}\n{}", out.out(), out.err());
        let body = out.json();
        assert_eq!(body["ok"], false);
        assert!(body["checks"].is_object(), "a report, not an error envelope: {body}");
        let check = app_check(&body);
        assert_eq!(check["status"], "fail");
        let directories = &check["directories"];
        assert_eq!(directories["status"], "fail", "{check}");
        assert_eq!(directories["verdict"], "mismatch");
        assert_eq!(directories["app"]["data_dir"], app.data_dir().display().to_string().as_str());
        assert_eq!(
            directories["cli"]["data_dir"],
            other.join("testanyware").display().to_string().as_str()
        );
        assert!(directories["remedy"].as_str().unwrap().contains("XDG_DATA_HOME"), "{directories}");
        // The other items still report.
        assert_eq!(check["running"]["status"], "pass");
        assert_eq!(check["versions"]["verdict"], "match");
    }

    #[test]
    fn a_mismatch_of_directories_in_the_text_report_names_the_item_and_the_remedy() {
        let _serial = serial();
        let app = TestApp::start();
        let other = app.root().join("other-state");
        let out = app.cli_with(
            &["doctor"],
            &[
                ("TESTANYWARE_APP_SOCKET", app.socket().to_str().unwrap()),
                ("XDG_STATE_HOME", other.to_str().unwrap()),
            ],
        );
        assert_eq!(out.code(), 1);
        let text = out.out();
        assert!(text.contains("TestAnyware app"), "{text}");
        assert!(text.contains("✗ directories:"), "{text}");
        assert!(text.contains("remedy:") && text.contains("XDG_STATE_HOME"), "{text}");
    }

    fn protocol_mismatch(app_args: &[&str], remedy_word: &str, remedy: &str) {
        let args: Vec<String> = app_args.iter().map(|a| a.to_string()).collect();
        let app = TestApp::start_after_with_args(|_| {}, &args);
        let (code, body) = doctor(&app);
        assert_eq!(code, 1, "{body}");
        assert_eq!(body["ok"], false);
        let check = app_check(&body);
        assert_eq!(check["status"], "fail");
        let versions = &check["versions"];
        assert_eq!(versions["status"], "fail", "{check}");
        assert_eq!(versions["verdict"], "no_common_version");
        assert_eq!(versions["remedy_word"], remedy_word);
        assert_eq!(versions["remedy"], remedy);
        // The status request is answered for any range, so the app still reports itself.
        assert_eq!(check["running"]["status"], "pass");
    }

    #[test]
    fn a_protocol_mismatch_with_an_older_host_cli_is_a_failing_check_with_the_cli_upgrade() {
        let _serial = serial();
        protocol_mismatch(
            &["--test-build", "9.0.0", "--test-protocol-range", "2-3"],
            "upgrade_host_cli",
            "Run `brew upgrade testanyware`.",
        );
    }

    #[test]
    fn a_protocol_mismatch_with_an_older_app_is_a_failing_check_with_the_remedy_of_the_table() {
        let _serial = serial();
        protocol_mismatch(
            &["--test-build", "0.0.0", "--test-protocol-range", "0-0", "--test-bundle-build", "9.9.9"],
            "quit_and_open_app",
            "Quit TestAnyware from its menu, and open it again.",
        );
        protocol_mismatch(
            &["--test-build", "0.0.0", "--test-protocol-range", "0-0", "--test-bundle-build", "0.0.0"],
            "upgrade_cask",
            "Run `brew upgrade --cask testanyware-app`.",
        );
    }

    #[test]
    fn an_app_older_than_its_bundle_is_reported_with_the_quit_and_open_remedy() {
        let _serial = serial();
        let app = TestApp::start_after_with_args(
            |_| {},
            &["--test-bundle-build".to_string(), "9.9.9".to_string()],
        );
        let (_, body) = doctor(&app);
        let bundle = &app_check(&body)["bundle"];
        assert_eq!(bundle["status"], "warn", "{body}");
        assert_eq!(bundle["verdict"], "older_than_bundle");
        assert_eq!(bundle["bundle_build"], "9.9.9");
        assert_eq!(bundle["remedy"], "Quit TestAnyware from its menu, and open it again.");
        assert_ne!(app_check(&body)["status"], "fail", "a warning does not flip the check");
    }

    #[test]
    fn a_file_that_fails_fails_the_config_check_and_the_app_check_still_reports_the_app() {
        let _serial = serial();
        let app = TestApp::start_with_config(|_| BAD_FILE.to_string());
        // With the XDG variables set, the CLI's directories without the file are the app's.
        let (code, body) = doctor(&app);
        assert_eq!(code, 1, "{body}");
        assert_eq!(body["ok"], false);

        let config = &body["checks"]["config"];
        assert_eq!(config["status"], "fail", "{body}");
        assert_eq!(config["error"]["code"], "CONFIG_INVALID");

        let check = app_check(&body);
        assert_eq!(check["running"]["status"], "pass", "the app still reports: {check}");
        assert_eq!(check["running"]["build"], env!("CARGO_PKG_VERSION"));
        let configuration = &check["configuration"];
        assert_eq!(configuration["status"], "fail", "{check}");
        assert_eq!(configuration["verdict"], "invalid");
        assert_eq!(configuration["error"]["code"], "CONFIG_INVALID");
        assert_eq!(
            configuration["file"],
            app.config_file().display().to_string().as_str()
        );
        assert!(configuration["remedy"].as_str().unwrap().contains("open it again"));
        assert_eq!(check["directories"]["verdict"], "match", "{check}");
    }

    #[test]
    fn a_silent_app_fails_running_with_the_remedy_and_skips_the_items_that_need_it() {
        let _serial = serial();
        let app = TestApp::not_running();
        let (code, body) = doctor(&app);
        assert_eq!(code, 1, "{body}");
        let check = app_check(&body);
        assert_eq!(check["status"], "fail");
        let running = &check["running"];
        assert_eq!(running["status"], "fail", "{check}");
        assert_eq!(running["verdict"], "not_running");
        assert_eq!(running["error"]["code"], "APP_UNREACHABLE");
        assert_eq!(running["error"]["details"]["cause"], "not_running");
        for name in ["login_item", "local_network", "configuration", "adoption", "directories", "versions", "bundle"] {
            assert_eq!(check[name]["status"], "skip", "{name}: {check}");
            assert_eq!(check[name]["verdict"], "app_not_running");
        }
        // `installed` judges the places of the host, whatever the machine holds.
        assert!(["installed", "not_found"].contains(&check["installed"]["verdict"].as_str().unwrap()));
        assert!(check["installed"]["searched"].is_array() || check["installed"]["path"].is_string());
    }

    #[test]
    fn doctor_changes_nothing_in_the_app() {
        let _serial = serial();
        let app = TestApp::start();
        let before = app.cli(&["vm", "list", "--json"]).out();
        let _ = doctor(&app);
        let _ = doctor(&app);
        assert_eq!(app.cli(&["vm", "list", "--json"]).out(), before);
    }
}

#[cfg(not(target_os = "macos"))]
mod on_a_linux_or_windows_host {
    use super::*;

    #[test]
    fn the_check_does_not_apply_even_when_a_socket_is_named() {
        let _serial = serial();
        let app = TestApp::start();
        let (_, body) = doctor(&app);
        let check = app_check(&body);
        assert_eq!(check["status"], "skip", "{body}");
        assert_eq!(check["verdict"], "not_applicable");
        assert!(check["reason"].as_str().unwrap().contains("macOS"));
        assert!(check.get("running").is_none());
    }
}
