//! The Host CLI and `config.rhai` (ADR-0021, spec "Configuration"), through the real binary.
//!
//! The activation switch is on for a Linux or Windows host and off for a macOS host, so the
//! tests that need a file to count are `cfg(not(target_os = "macos"))`, and the macOS test shows
//! that the file is ignored there. The pure functions are tested with the switch on for any host
//! in `src/host_config.rs`.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_testanyware");

/// Run the CLI with `TESTANYWARE_CONFIG` naming `config`, and a home directory that holds no file.
fn run(home: &Path, config: &str, extra_env: &[(&str, &str)], args: &[&str]) -> Output {
    let mut command = Command::new(BIN);
    command
        .args(args)
        .env("HOME", home)
        .env("TESTANYWARE_CONFIG", config)
        .env_remove("XDG_STATE_HOME")
        .env_remove("XDG_DATA_HOME")
        .env_remove("TESTANYWARE_APP_SOCKET")
        .env_remove("TESTANYWARE_VM_ID");
    for (key, value) in extra_env {
        command.env(key, value);
    }
    command
        .output()
        .unwrap_or_else(|e| panic!("failed to run `{BIN} {}`: {e}", args.join(" ")))
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is no JSON ({e}): {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn write(dir: &TempDir, name: &str, text: &str) -> String {
    let path = dir.path().join(name);
    std::fs::write(&path, text).unwrap();
    path.to_str().unwrap().to_string()
}

#[cfg(not(target_os = "macos"))]
mod active {
    use super::*;

    #[test]
    fn a_file_that_fails_stops_each_command_except_doctor() {
        let dir = TempDir::new().unwrap();
        let bad = write(&dir, "bad.rhai", "let a = 1;\nlet b = ;\n#{}");

        for args in [
            &["vm", "list", "--json"][..],
            &["capabilities", "--json"],
            &["screen", "size", "--vm", "x", "--json"],
        ] {
            let out = run(dir.path(), &bad, &[], args);
            assert_eq!(
                out.status.code(),
                Some(2),
                "{args:?}: {}",
                String::from_utf8_lossy(&out.stdout)
            );
            let body = json(&out);
            assert_eq!(body["code"], "CONFIG_INVALID", "{args:?}");
            assert_eq!(body["ok"], false);
            assert_eq!(body["details"]["file"], bad.as_str());
            assert_eq!(body["details"]["line"], 2);
            assert!(
                body["details"]["message"]
                    .as_str()
                    .unwrap()
                    .starts_with("syntax error"),
                "{body}"
            );
            assert!(body["remediation"]
                .as_str()
                .unwrap()
                .contains("TESTANYWARE_CONFIG=none"));
        }

        // Text mode names the file, the line and the message on stderr.
        let out = run(dir.path(), &bad, &[], &["vm", "list"]);
        assert_eq!(out.status.code(), Some(2));
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains(&format!("{bad}:2: syntax error")),
            "{stderr}"
        );
    }

    #[test]
    fn an_unknown_key_is_refused_with_no_line() {
        let dir = TempDir::new().unwrap();
        let bad = write(&dir, "unknown.rhai", "#{ colour: \"red\" }");
        let out = run(dir.path(), &bad, &[], &["vm", "list", "--json"]);
        assert_eq!(out.status.code(), Some(2));
        let body = json(&out);
        assert_eq!(body["code"], "CONFIG_INVALID");
        assert!(body["details"]["line"].is_null(), "{body}");
        assert!(
            body["message"]
                .as_str()
                .unwrap()
                .contains("unknown key `colour`"),
            "{body}"
        );
    }

    #[test]
    fn doctor_runs_with_a_file_that_fails_and_exits_with_1() {
        let dir = TempDir::new().unwrap();
        let bad = write(&dir, "bad.rhai", "#{ colour: \"red\" }");
        let out = run(dir.path(), &bad, &[], &["doctor", "--json"]);
        assert_eq!(
            out.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let body = json(&out);
        assert_eq!(body["ok"], false);
        let config = &body["checks"]["config"];
        assert_eq!(config["status"], "fail");
        assert_eq!(config["verdict"], "invalid");
        assert_eq!(config["error"]["code"], "CONFIG_INVALID");
        assert_eq!(config["error"]["details"]["file"], bad.as_str());
        // The other checks ran.
        for group in [
            "install_path",
            "bundled_agents",
            "bundled_scripts",
            "host_tools",
            "windows_tpm",
        ] {
            assert!(
                body["checks"][group]["status"].is_string(),
                "{group} missing: {body}"
            );
        }
        // The text report says so too.
        let text = run(dir.path(), &bad, &[], &["doctor"]);
        assert_eq!(text.status.code(), Some(1));
        let stdout = String::from_utf8_lossy(&text.stdout);
        assert!(stdout.contains("Configuration file"), "{stdout}");
        assert!(stdout.contains("CONFIG_INVALID"), "{stdout}");
    }

    #[test]
    fn doctor_reports_a_file_that_loads_a_missing_file_and_none() {
        let dir = TempDir::new().unwrap();
        let good = write(&dir, "good.rhai", "#{}");
        let body = json(&run(dir.path(), &good, &[], &["doctor", "--json"]));
        assert_eq!(body["checks"]["config"]["status"], "pass");
        assert_eq!(body["checks"]["config"]["verdict"], "loaded");
        assert_eq!(body["checks"]["config"]["file"], good.as_str());

        let missing = dir.path().join("missing.rhai");
        let body = json(&run(
            dir.path(),
            missing.to_str().unwrap(),
            &[],
            &["doctor", "--json"],
        ));
        assert_eq!(body["checks"]["config"]["verdict"], "absent");

        let body = json(&run(dir.path(), "none", &[], &["doctor", "--json"]));
        assert_eq!(body["checks"]["config"]["verdict"], "disabled");
        assert!(body["checks"]["config"]["file"].is_null());
    }

    #[test]
    fn none_ignores_a_file_that_would_fail() {
        let dir = TempDir::new().unwrap();
        write(&dir, "unused.rhai", "#{ colour: 1 }");
        let out = run(dir.path(), "none", &[], &["capabilities", "--json"]);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    #[test]
    fn the_state_dir_of_the_file_reaches_the_lookup_of_a_vm() {
        let dir = TempDir::new().unwrap();
        let state = dir.path().join("file-state");
        let file = write(
            &dir,
            "state.rhai",
            &format!("#{{ state_dir: {:?} }}", state.to_str().unwrap()),
        );
        let named = |out: &Output| json(out).to_string();

        // The file beats the host default.
        let out = run(
            dir.path(),
            &file,
            &[],
            &["screen", "size", "--vm", "ghost", "--json"],
        );
        let expected = state.join("vms").join("ghost.json");
        assert!(
            named(&out).contains(&expected.to_string_lossy().replace('\\', "\\\\")),
            "{} should name {}",
            named(&out),
            expected.display()
        );

        // The environment variable beats the file.
        let xdg = dir.path().join("xdg");
        let out = run(
            dir.path(),
            &file,
            &[("XDG_STATE_HOME", xdg.to_str().unwrap())],
            &["screen", "size", "--vm", "ghost", "--json"],
        );
        let expected = xdg.join("testanyware").join("vms").join("ghost.json");
        assert!(
            named(&out).contains(&expected.to_string_lossy().replace('\\', "\\\\")),
            "{} should name {}",
            named(&out),
            expected.display()
        );
    }
}

#[cfg(target_os = "macos")]
mod inactive {
    use super::*;

    /// A macOS host ignores the file until `golden-directory-check-k34` (spec: "Configuration").
    #[test]
    fn a_macos_host_ignores_the_file() {
        let dir = TempDir::new().unwrap();
        let bad = write(&dir, "bad.rhai", "let a = ;");

        let out = run(dir.path(), &bad, &[], &["capabilities", "--json"]);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );

        let out = run(dir.path(), &bad, &[], &["doctor", "--json"]);
        let body = json(&out);
        let config = &body["checks"]["config"];
        assert_eq!(config["status"], "skip", "{body}");
        assert_eq!(config["verdict"], "not_applicable");
        assert!(
            config["reason"]
                .as_str()
                .unwrap()
                .contains("does not apply"),
            "{config}"
        );
        // A skipped check does not fail the report.
        assert!(body["checks"]["config"].get("error").is_none());
    }
}
