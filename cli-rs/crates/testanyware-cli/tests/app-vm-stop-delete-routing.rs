//! `vm stop` and `vm delete` through the TestAnyware app (spec: "The protocol", "Lifecycle and
//! adoption").
//!
//! The real Host CLI binary runs against the real app executable in test mode. The processes of
//! a VM are `sleep` commands that the test orphans, so that they have no parent to reap them, as
//! the processes of a VM that the app started before it restarted. The tart backend is a fake
//! script in the one directory where the app looks for tools. It keeps its VMs as files and logs
//! each call, so a test sees which commands the app ran.
//!
//! Where a host keeps the direct path (Linux, Windows), each test also runs the CLI with no
//! socket on an identical fixture and requires the same output. A macOS host has no direct path,
//! so there the expected output is written out below, and the routed run must print it.

#![cfg(unix)]

mod common;

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use common::{serial, OutputExt, TestApp, HOST_GOES_DIRECT};
use serde_json::json;

/// A process that outlives the test's interest in it, orphaned so that nothing has to reap it.
/// Dropping it kills what is left.
struct Orphan(i32);

impl Orphan {
    fn start() -> Self {
        let out = Command::new("sh")
            .args(["-c", "sleep 600 </dev/null >/dev/null 2>&1 & echo $!"])
            .output()
            .expect("run sh");
        let pid = String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse()
            .expect("a pid");
        Orphan(pid)
    }

    fn alive(&self) -> bool {
        Command::new("kill")
            .args(["-0", &self.0.to_string()])
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Wait for the process to end. The app signals it, and the kernel then takes a moment.
    fn gone_within(&self, limit: Duration) -> bool {
        let deadline = Instant::now() + limit;
        while self.alive() {
            if Instant::now() > deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        true
    }
}

impl Drop for Orphan {
    fn drop(&mut self) {
        let _ = Command::new("kill")
            .args(["-9", &self.0.to_string()])
            .stderr(Stdio::null())
            .status();
    }
}

fn vms_dir(app: &TestApp) -> PathBuf {
    app.state_dir().join("vms")
}

fn sidecars(app: &TestApp, id: &str) -> [PathBuf; 2] {
    [
        vms_dir(app).join(format!("{id}.json")),
        vms_dir(app).join(format!("{id}.meta.json")),
    ]
}

fn clone_dir(app: &TestApp, id: &str) -> PathBuf {
    app.data_dir().join("clones").join(id)
}

fn session_dir(app: &TestApp, id: &str) -> PathBuf {
    app.tmp_dir().join(format!("testanyware-{id}"))
}

/// A running QEMU VM whose process is `orphan`: its sidecars, its clone directory and its
/// session directory. The meta names the orphan, so that a stop signals it and nothing else.
fn add_qemu_vm(app: &TestApp, id: &str, orphan: &Orphan) {
    app.add_running_qemu_clone(id, "linux", 5901);
    let meta = json!({
        "id": id,
        "tool": "qemu",
        "pid": orphan.0,
        "clone_dir": clone_dir(app, id),
    });
    std::fs::write(&sidecars(app, id)[1], meta.to_string()).unwrap();
}

/// A fake `tart` that keeps each VM as a file in `state/` whose content is the VM's state, and
/// logs each call to `calls.log`. `delete` fails while `delete-fails` exists.
fn add_fake_tart(app: &TestApp) {
    let state = app.root().join("tart-state");
    std::fs::create_dir_all(&state).unwrap();
    let state = state.display();
    app.add_tool(
        "tart",
        &format!(
            r#"D="{state}"
echo "$@" >> "$D/../calls.log"
case "$1" in
  list)
    first=1
    printf '['
    for f in "$D"/*; do
      [ -e "$f" ] || continue
      [ "$first" = 1 ] || printf ','
      first=0
      printf '{{"Name":"%s","State":"%s"}}' "$(basename "$f")" "$(cat "$f")"
    done
    printf ']\n'
    ;;
  stop) ;;
  delete)
    [ -e "$D/../delete-fails" ] && exit 1
    rm -f "$D/$2"
    ;;
esac
"#
        ),
    );
}

fn add_tart_vm(app: &TestApp, name: &str, state: &str) {
    std::fs::write(app.root().join("tart-state").join(name), state).unwrap();
}

fn tart_has(app: &TestApp, name: &str) -> bool {
    app.root().join("tart-state").join(name).exists()
}

fn tart_calls(app: &TestApp) -> Vec<String> {
    std::fs::read_to_string(app.root().join("calls.log"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

/// Run `args` through the app on `app`. Where the host keeps the direct path, run them directly
/// on `twin`, an identical fixture, and require the same output and exit code.
fn routed_and_direct(app: &TestApp, twin: impl FnOnce() -> TestApp, args: &[&str]) -> Output {
    let routed = app.cli(args);
    if HOST_GOES_DIRECT {
        let twin = twin();
        let direct = twin.cli_default(args);
        assert_eq!(direct.code(), routed.code(), "exit code of {args:?}");
        assert_eq!(direct.out(), routed.out(), "stdout of {args:?}");
        assert_eq!(direct.err(), routed.err(), "stderr of {args:?}");
    }
    routed
}

// ---- vm stop ----------------------------------------------------------------------------

/// A QEMU VM `id` and the process that runs it.
fn qemu_fixture(id: &str) -> (TestApp, Orphan) {
    let orphan = Orphan::start();
    let app = TestApp::start_after(|app| add_qemu_vm(app, id, &orphan));
    (app, orphan)
}

#[test]
fn a_stop_signals_the_vm_and_removes_what_it_left() {
    let _lock = serial();
    let id = "testanyware-aaaa1111";
    let (app, orphan) = qemu_fixture(id);
    let bystander = Orphan::start();
    assert!(orphan.alive());

    let out = routed_and_direct(&app, || qemu_fixture(id).0, &["vm", "stop", id]);

    assert_eq!(out.code(), 0, "{}", out.err());
    assert_eq!(out.out(), format!("stopped {id}\n"));
    assert_eq!(out.err(), "");
    assert!(
        orphan.gone_within(Duration::from_secs(5)),
        "the VM's process is still alive"
    );
    assert!(
        bystander.alive(),
        "a stop signalled a process that is not the VM's"
    );
    for sidecar in sidecars(&app, id) {
        assert!(!sidecar.exists(), "{} remains", sidecar.display());
    }
    assert!(!clone_dir(&app, id).exists());
    assert!(!session_dir(&app, id).exists());
}

#[test]
fn a_json_stop_prints_the_id_and_stopped() {
    let _lock = serial();
    let id = "testanyware-bbbb2222";
    let (app, orphan) = qemu_fixture(id);

    let out = routed_and_direct(&app, || qemu_fixture(id).0, &["vm", "stop", id, "--json"]);

    assert_eq!(out.code(), 0, "{}", out.err());
    let envelope = out.json();
    assert_eq!(envelope["ok"], true, "{envelope}");
    assert_eq!(envelope["id"], id);
    assert_eq!(envelope["stopped"], true);
    assert!(orphan.gone_within(Duration::from_secs(5)));
}

#[test]
fn the_id_may_come_from_the_environment() {
    let _lock = serial();
    let id = "testanyware-cccc3333";
    let (app, orphan) = qemu_fixture(id);
    let socket = app.socket();

    let out = app.cli_with(
        &["vm", "stop"],
        &[
            ("TESTANYWARE_APP_SOCKET", socket.to_str().unwrap()),
            ("TESTANYWARE_VM_ID", id),
        ],
    );

    assert_eq!(out.code(), 0, "{}", out.err());
    assert_eq!(out.out(), format!("stopped {id}\n"));
    assert!(orphan.gone_within(Duration::from_secs(5)));
}

#[test]
fn a_stop_of_a_missing_vm_fails_with_vm_not_found() {
    let _lock = serial();
    let app = TestApp::start();

    let out = routed_and_direct(&app, TestApp::start, &["vm", "stop", "nope", "--json"]);

    assert_eq!(out.code(), 3, "{}", out.err());
    let envelope = out.json();
    assert_eq!(envelope["ok"], false, "{envelope}");
    assert_eq!(envelope["code"], "VM_NOT_FOUND");
    assert_eq!(envelope["message"], "no VM found for id 'nope'");
    assert_eq!(
        envelope["remediation"],
        "Run `testanyware vm list` to see running VMs."
    );
    assert_eq!(envelope["details"]["vm_id"], "nope");
}

#[test]
fn a_text_stop_of_a_missing_vm_prints_the_error_on_stderr() {
    let _lock = serial();
    let app = TestApp::start();

    let out = routed_and_direct(&app, TestApp::start, &["vm", "stop", "nope"]);

    assert_eq!(out.code(), 3);
    assert_eq!(out.out(), "");
    assert!(
        out.err().contains("no VM found for id 'nope'"),
        "{}",
        out.err()
    );
}

#[test]
fn an_id_that_is_no_plain_id_names_no_vm() {
    let _lock = serial();
    let app = TestApp::start();
    let outside = app.state_dir().join("outside.meta.json");
    std::fs::write(&outside, b"{}").unwrap();

    for id in ["../outside", "a b", "a%2Fb", ".hidden", "a?b"] {
        let out = app.cli(&["vm", "stop", id, "--json"]);
        assert_eq!(out.code(), 3, "{id}: {}", out.err());
        assert_eq!(out.json()["code"], "VM_NOT_FOUND", "{id}");
    }
    assert!(
        outside.exists(),
        "a stop reached a file outside the VM records"
    );
}

#[test]
fn a_qemu_meta_without_a_clone_directory_fails_the_stop_and_still_clears_the_records() {
    let _lock = serial();
    let id = "testanyware-dddd4444";
    let prepare = |app: &TestApp| {
        app.add_sidecars(id, "linux", "qemu", 5901);
        // The meta of a pid that is no process: a corrupt record that names nothing to signal.
        std::fs::write(
            &sidecars(app, id)[1],
            json!({ "id": id, "tool": "qemu", "pid": 0 }).to_string(),
        )
        .unwrap();
    };
    let app = TestApp::start_after(prepare);

    let out = routed_and_direct(
        &app,
        || TestApp::start_after(prepare),
        &["vm", "stop", id, "--json"],
    );

    assert_eq!(out.code(), 1, "{}", out.err());
    let envelope = out.json();
    assert_eq!(envelope["code"], "VM_STOP_FAILED");
    assert_eq!(
        envelope["message"],
        format!("VM '{id}' did not stop cleanly")
    );
    assert_eq!(envelope["details"]["vm_id"], id);
    for sidecar in sidecars(&app, id) {
        assert!(!sidecar.exists(), "{} remains", sidecar.display());
    }
}

#[test]
fn a_meta_that_does_not_read_fails_the_stop_with_io_error() {
    let _lock = serial();
    let id = "testanyware-eeee5555";
    let app = TestApp::start_after(|app| {
        app.add_sidecars(id, "linux", "qemu", 5901);
        std::fs::write(&sidecars(app, id)[1], b"not json").unwrap();
    });

    let out = app.cli(&["vm", "stop", id, "--json"]);

    assert_eq!(out.code(), 1, "{}", out.err());
    let envelope = out.json();
    assert_eq!(envelope["code"], "IO_ERROR");
    // The direct command prints no remedy for this error, and the app adds none.
    assert!(envelope["remediation"].is_null(), "{envelope}");
}

#[test]
fn a_dry_run_asks_the_app_nothing() {
    let _lock = serial();
    let id = "testanyware-ffff6666";
    let orphan = Orphan::start();
    let app = TestApp::not_running();
    add_qemu_vm(&app, id, &orphan);

    let out = app.cli(&["vm", "stop", id, "--dry-run"]);

    assert_eq!(out.code(), 0, "{}", out.err());
    assert_eq!(
        out.out(),
        format!("dry-run: would stop {id} (pid {})\n", orphan.0)
    );
    assert!(orphan.alive());
    assert!(sidecars(&app, id)[1].exists());
}

#[test]
fn a_dry_run_of_a_missing_vm_fails_as_before() {
    let _lock = serial();
    let app = TestApp::not_running();

    let out = app.cli(&["vm", "stop", "nope", "--dry-run", "--json"]);

    assert_eq!(out.code(), 3);
    assert_eq!(out.json()["code"], "VM_NOT_FOUND");
}

#[test]
fn a_stop_without_an_app_fails_and_names_why() {
    let _lock = serial();
    let id = "testanyware-aaaa7777";
    let orphan = Orphan::start();
    let app = TestApp::not_running();
    add_qemu_vm(&app, id, &orphan);

    let out = app.cli(&["vm", "stop", id, "--json"]);

    assert_eq!(out.code(), 1, "{}", out.err());
    assert_eq!(out.json()["code"], "APP_UNREACHABLE");
    assert!(
        orphan.alive(),
        "a stop that never reached the app signalled the VM"
    );
    assert!(sidecars(&app, id)[1].exists());
}

#[test]
fn a_stop_with_no_id_is_a_usage_error_before_any_request() {
    let _lock = serial();
    let app = TestApp::not_running();

    let out = app.cli(&["vm", "stop", "--json"]);

    assert_eq!(out.code(), 2);
    assert_eq!(out.json()["code"], "USAGE_ERROR");
}

#[cfg(target_os = "macos")]
mod tart_stop {
    use super::*;

    const ID: &str = "testanyware-1111aaaa";

    fn tart_fixture() -> TestApp {
        TestApp::start_after(|app| {
            add_fake_tart(app);
            add_tart_vm(app, ID, "running");
            app.add_sidecars(ID, "macos", "tart", 5902);
            // The meta of a tart VM names no process that the test could signal.
            std::fs::write(
                &sidecars(app, ID)[1],
                json!({ "id": ID, "tool": "tart", "pid": 0 }).to_string(),
            )
            .unwrap();
        })
    }

    #[test]
    fn a_stop_of_a_tart_guest_stops_and_deletes_it_by_name() {
        let _lock = serial();
        let app = tart_fixture();

        let out = app.cli(&["vm", "stop", ID]);

        assert_eq!(out.code(), 0, "{}", out.err());
        assert_eq!(out.out(), format!("stopped {ID}\n"));
        assert!(!tart_has(&app, ID));
        let calls = tart_calls(&app);
        assert!(calls.contains(&format!("stop {ID}")), "{calls:?}");
        assert!(calls.contains(&format!("delete {ID}")), "{calls:?}");
        for sidecar in sidecars(&app, ID) {
            assert!(!sidecar.exists(), "{} remains", sidecar.display());
        }
    }

    #[test]
    fn a_tart_guest_that_no_longer_exists_fails_the_stop_and_clears_the_records() {
        let _lock = serial();
        let app = TestApp::start_after(|app| {
            add_fake_tart(app);
            app.add_sidecars(ID, "macos", "tart", 5902);
            std::fs::write(
                &sidecars(app, ID)[1],
                json!({ "id": ID, "tool": "tart", "pid": 0 }).to_string(),
            )
            .unwrap();
        });

        let out = app.cli(&["vm", "stop", ID, "--json"]);

        assert_eq!(out.code(), 1, "{}", out.err());
        assert_eq!(out.json()["code"], "VM_STOP_FAILED");
        for sidecar in sidecars(&app, ID) {
            assert!(!sidecar.exists(), "{} remains", sidecar.display());
        }
    }

    #[test]
    fn a_stop_signals_the_recorded_tart_process_only() {
        let _lock = serial();
        let orphan = Orphan::start();
        let bystander = Orphan::start();
        let app = TestApp::start_after(|app| {
            add_fake_tart(app);
            add_tart_vm(app, ID, "running");
            app.add_sidecars(ID, "macos", "tart", 5902);
            std::fs::write(
                &sidecars(app, ID)[1],
                json!({ "id": ID, "tool": "tart", "pid": orphan.0 }).to_string(),
            )
            .unwrap();
        });

        let out = app.cli(&["vm", "stop", ID]);

        assert_eq!(out.code(), 0, "{}", out.err());
        assert!(
            orphan.gone_within(Duration::from_secs(5)),
            "`tart run` still runs"
        );
        assert!(
            bystander.alive(),
            "a stop signalled a process that is not the VM's"
        );
    }
}

// ---- vm delete --------------------------------------------------------------------------

const QEMU_GOLDEN: &str = "testanyware-golden-windows-11";

fn golden_files(app: &TestApp, name: &str) -> PathBuf {
    app.data_dir().join("golden").join(format!("{name}.qcow2"))
}

#[test]
fn a_delete_removes_the_golden_image() {
    let _lock = serial();
    let prepare = |app: &TestApp| app.add_qemu_golden(QEMU_GOLDEN);
    let app = TestApp::start_after(prepare);

    let out = routed_and_direct(
        &app,
        || TestApp::start_after(prepare),
        &["vm", "delete", QEMU_GOLDEN],
    );

    assert_eq!(out.code(), 0, "{}", out.err());
    assert_eq!(out.out(), format!("deleted {QEMU_GOLDEN}\n"));
    assert_eq!(out.err(), "");
    assert!(!golden_files(&app, QEMU_GOLDEN).exists());
}

#[test]
fn a_json_delete_prints_the_name_and_deleted() {
    let _lock = serial();
    let prepare = |app: &TestApp| app.add_qemu_golden(QEMU_GOLDEN);
    let app = TestApp::start_after(prepare);

    let out = routed_and_direct(
        &app,
        || TestApp::start_after(prepare),
        &["vm", "delete", QEMU_GOLDEN, "--json"],
    );

    assert_eq!(out.code(), 0, "{}", out.err());
    let envelope = out.json();
    assert_eq!(envelope["ok"], true, "{envelope}");
    assert_eq!(envelope["name"], QEMU_GOLDEN);
    assert_eq!(envelope["deleted"], true);
}

#[test]
fn a_delete_of_a_missing_golden_fails_with_golden_not_found() {
    let _lock = serial();
    let app = TestApp::start();

    let out = routed_and_direct(&app, TestApp::start, &["vm", "delete", "nope", "--json"]);

    assert_eq!(out.code(), 3, "{}", out.err());
    let envelope = out.json();
    assert_eq!(envelope["code"], "GOLDEN_NOT_FOUND");
    assert_eq!(envelope["details"]["golden_name"], "nope");
    assert_eq!(
        envelope["remediation"],
        "Run `testanyware vm list` to see available golden images."
    );
}

#[test]
fn a_golden_name_with_characters_beyond_a_plain_id_reaches_the_app_whole() {
    let _lock = serial();
    let name = "my golden: 100%";
    let app = TestApp::start_after(|app| app.add_qemu_golden(name));

    let out = app.cli(&["vm", "delete", name]);

    assert_eq!(out.code(), 0, "{}", out.err());
    assert_eq!(out.out(), format!("deleted {name}\n"));
    assert!(!golden_files(&app, name).exists());
}

#[test]
fn a_name_that_could_leave_the_golden_directory_names_no_golden() {
    let _lock = serial();
    let app = TestApp::start_after(|app| app.add_qemu_golden(QEMU_GOLDEN));
    // The file that `sub/../../outside` would name, one level above the golden directory. `sub`
    // exists, so only the rule on the name stands between the name and the file.
    let outside = app.data_dir().join("outside.qcow2");
    std::fs::write(&outside, b"x").unwrap();
    std::fs::create_dir(app.data_dir().join("golden").join("sub")).unwrap();

    for name in [
        "sub/../../outside",
        "../outside",
        "..",
        ".hidden",
        "a/b",
        "",
    ] {
        let out = app.cli(&["vm", "delete", name, "--force", "--json"]);
        assert_eq!(out.code(), 3, "{name:?}: {}", out.err());
        assert_eq!(out.json()["code"], "GOLDEN_NOT_FOUND", "{name:?}");
    }
    assert!(
        outside.exists(),
        "a delete reached a file outside the golden directory"
    );
    assert!(golden_files(&app, QEMU_GOLDEN).exists());
}

#[test]
fn a_golden_name_that_starts_with_a_dot_is_deleted_like_any_other() {
    let _lock = serial();
    let name = ".hand-placed";
    let app = TestApp::start_after(|app| app.add_qemu_golden(name));

    let out = app.cli(&["vm", "delete", name]);

    assert_eq!(out.code(), 0, "{}", out.err());
    assert_eq!(out.out(), format!("deleted {name}\n"));
    assert!(!golden_files(&app, name).exists());
}

#[test]
fn a_name_longer_than_a_file_name_is_not_found_whatever_its_length() {
    let _lock = serial();
    let app = TestApp::start();

    // 30,000 bytes of characters that a path encodes to three bytes each: more than a request
    // line holds. The direct command finds no such file, and so does the routed one.
    for len in [256, 3_000, 30_000] {
        let name = "%".repeat(len);
        let out = app.cli(&["vm", "delete", &name, "--json"]);
        assert_eq!(out.code(), 3, "{len}: {}", out.err());
        assert_eq!(out.json()["code"], "GOLDEN_NOT_FOUND", "{len}");
    }
}

#[test]
fn a_dry_run_delete_asks_the_app_nothing() {
    let _lock = serial();
    let app = TestApp::not_running();
    app.add_qemu_golden(QEMU_GOLDEN);

    let out = app.cli(&["vm", "delete", QEMU_GOLDEN, "--dry-run"]);

    assert_eq!(out.code(), 0, "{}", out.err());
    assert_eq!(
        out.out(),
        format!("dry-run: would delete golden {QEMU_GOLDEN}\n")
    );
    assert!(golden_files(&app, QEMU_GOLDEN).exists());
}

#[test]
fn a_delete_without_an_app_fails_and_names_why() {
    let _lock = serial();
    let app = TestApp::not_running();
    app.add_qemu_golden(QEMU_GOLDEN);

    let out = app.cli(&["vm", "delete", QEMU_GOLDEN, "--json"]);

    assert_eq!(out.code(), 1, "{}", out.err());
    assert_eq!(out.json()["code"], "APP_UNREACHABLE");
    assert!(golden_files(&app, QEMU_GOLDEN).exists());
}

#[cfg(target_os = "macos")]
mod tart_delete {
    use super::*;

    const GOLDEN: &str = "testanyware-golden-macos-27";
    const CLONE: &str = "testanyware-2222bbbb";

    fn fixture(with_running_clone: bool) -> TestApp {
        TestApp::start_after(|app| {
            add_fake_tart(app);
            add_tart_vm(app, GOLDEN, "stopped");
            if with_running_clone {
                add_tart_vm(app, CLONE, "running");
            }
        })
    }

    #[test]
    fn a_tart_golden_is_deleted_by_name() {
        let _lock = serial();
        let app = fixture(false);

        let out = app.cli(&["vm", "delete", GOLDEN]);

        assert_eq!(out.code(), 0, "{}", out.err());
        assert_eq!(out.out(), format!("deleted {GOLDEN}\n"));
        assert!(!tart_has(&app, GOLDEN));
        assert!(tart_calls(&app).contains(&format!("delete {GOLDEN}")));
    }

    #[test]
    fn a_golden_with_a_running_clone_is_kept_unless_forced() {
        let _lock = serial();
        let app = fixture(true);

        let refused = app.cli(&["vm", "delete", GOLDEN, "--json"]);

        assert_eq!(refused.code(), 5, "{}", refused.err());
        let envelope = refused.json();
        assert_eq!(envelope["code"], "GOLDEN_IN_USE");
        assert_eq!(
            envelope["remediation"],
            "Stop the running clones first, or re-run with --force."
        );
        assert!(
            tart_has(&app, GOLDEN),
            "a refused delete removed the golden"
        );

        let forced = app.cli(&["vm", "delete", GOLDEN, "--force"]);

        assert_eq!(forced.code(), 0, "{}", forced.err());
        assert_eq!(forced.out(), format!("deleted {GOLDEN}\n"));
        assert!(!tart_has(&app, GOLDEN));
        assert!(
            tart_has(&app, CLONE),
            "the clone is not the golden's to delete"
        );
    }

    #[test]
    fn a_tart_that_cannot_delete_fails_with_tart_failed() {
        let _lock = serial();
        let app = fixture(false);
        std::fs::write(app.root().join("delete-fails"), b"").unwrap();

        let out = app.cli(&["vm", "delete", GOLDEN, "--json"]);

        assert_eq!(out.code(), 1, "{}", out.err());
        let envelope = out.json();
        assert_eq!(envelope["code"], "TART_FAILED");
        assert!(tart_has(&app, GOLDEN));
    }
}
