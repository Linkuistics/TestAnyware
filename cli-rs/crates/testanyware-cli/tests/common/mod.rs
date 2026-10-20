//! A harness that drives the real Host CLI binary against the real TestAnyware app executable.
//!
//! The app runs in its test mode, on a temporary socket and in temporary directories. The
//! guests are fakes on loopback: a wiremock In-VM agent here, and a synthetic RFB server when
//! the VNC routing tests need one (`cli-app-vnc-routing-k25`). The test writes the spec file
//! that names a fake, as `vm start` writes it for a real VM.
//!
//! The rules that keep it sound:
//!
//! - A test holds [`serial`] for its whole body. macOS has no atomic close-on-exec for a new
//!   socket or pipe, so a thread that spawns a process can hand its child a socket that a
//!   sibling thread has just opened. The child then keeps a listener alive, and tests hang or
//!   read stale sockets (`crates/testanyware-app/tests/protocol_core.rs` has the history).
//! - A test starts its fakes first and spawns the app once. Nothing opens a socket in another
//!   thread while a process spawns.
//! - The harness builds the app executable itself, because `cargo test -p testanyware-cli`
//!   does not.

#![allow(dead_code)] // Each test file uses part of the harness.
#![cfg(unix)]

use std::net::SocketAddr;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// The Host CLI under test.
pub const CLI: &str = env!("CARGO_BIN_EXE_testanyware");

static SERIAL: Mutex<()> = Mutex::new(());

/// Hold this for the whole body of a test.
pub fn serial() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The path of the app executable, built once for the test process. The executable sits in
/// the directory of the CLI, and a profile other than `debug` is named by that directory.
fn app_binary() -> &'static Path {
    static APP: OnceLock<PathBuf> = OnceLock::new();
    APP.get_or_init(|| {
        let cli = Path::new(CLI);
        let dir = cli.parent().expect("the CLI has a directory");
        let mut build = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
        build
            .args(["build", "--quiet", "-p", "testanyware-app"])
            .current_dir(env!("CARGO_MANIFEST_DIR"));
        match dir.file_name().and_then(|n| n.to_str()) {
            Some("debug") => {}
            Some("release") => {
                build.arg("--release");
            }
            Some(profile) => {
                build.args(["--profile", profile]);
            }
            None => {}
        }
        let status = build.status().expect("run cargo to build the app");
        assert!(status.success(), "cargo could not build testanyware-app");
        let app = dir.join("testanyware-app");
        assert!(app.is_file(), "no app executable at {}", app.display());
        app
    })
}

// ---- the fake In-VM agent ----

/// A wiremock In-VM agent on loopback. It owns the runtime that serves it.
pub struct FakeAgent {
    runtime: tokio::runtime::Runtime,
    server: MockServer,
}

/// A request that the fake agent received.
#[derive(Debug)]
pub struct Received {
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    /// The `Host` header: the authority that the client names the agent by.
    pub host: Option<String>,
    pub body: Vec<u8>,
}

impl FakeAgent {
    pub fn start() -> Self {
        let runtime = tokio::runtime::Runtime::new().expect("a tokio runtime");
        let server = runtime.block_on(MockServer::start());
        FakeAgent { runtime, server }
    }

    pub fn addr(&self) -> SocketAddr {
        *self.server.address()
    }

    /// Answer `method` `route` with `response`.
    pub fn mount(&self, http_method: &str, route: &str, response: ResponseTemplate) {
        self.runtime.block_on(
            Mock::given(method(http_method))
                .and(path(route))
                .respond_with(response)
                .mount(&self.server),
        );
    }

    /// Every request that arrived, oldest first.
    pub fn received(&self) -> Vec<Received> {
        self.runtime
            .block_on(self.server.received_requests())
            .unwrap_or_default()
            .into_iter()
            .map(|r| Received {
                method: r.method.to_string(),
                path: r.url.path().to_string(),
                query: r.url.query().map(str::to_string),
                host: r
                    .headers
                    .get("host")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string),
                body: r.body,
            })
            .collect()
    }

    pub fn request_count(&self) -> usize {
        self.received().len()
    }
}

// ---- the app and the directories ----

/// The directories of one test, and the app that serves them when it runs.
///
/// The layout under the temporary directory:
///
/// - `run/testanyware.sock`: the socket. The app creates `run` with mode 0700.
/// - `xdg-state/testanyware/vms/`: the VM spec files. The CLI reads them as `$XDG_STATE_HOME`,
///   and the app as its `--state-dir`.
/// - `xdg-data/testanyware/`: the data directory.
pub struct TestApp {
    child: Option<Child>,
    tmp: TempDir,
}

impl Drop for TestApp {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl TestApp {
    /// Directories and a socket path, and no app. A command routed through the app finds
    /// nothing on the socket.
    pub fn not_running() -> Self {
        let tmp = tempfile::tempdir().expect("a temporary directory");
        let app = TestApp { child: None, tmp };
        std::fs::create_dir_all(app.state_dir().join("vms")).unwrap();
        std::fs::create_dir_all(app.data_dir()).unwrap();
        std::fs::create_dir_all(app.socket_dir()).unwrap();
        std::fs::set_permissions(app.socket_dir(), std::fs::Permissions::from_mode(0o700)).unwrap();
        app
    }

    /// Start the app on a fresh socket and fresh directories.
    pub fn start() -> Self {
        let mut app = Self::not_running();
        let mut child = Command::new(app_binary())
            .args(["--test-mode", "--socket"])
            .arg(app.socket())
            .arg("--state-dir")
            .arg(app.state_dir())
            .arg("--data-dir")
            .arg(app.data_dir())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn the app");
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                panic!("the app exited early with {status}");
            }
            if UnixStream::connect(app.socket()).is_ok() {
                break;
            }
            assert!(Instant::now() < deadline, "the app did not serve in 10 s");
            std::thread::sleep(Duration::from_millis(20));
        }
        app.child = Some(child);
        app
    }

    pub fn root(&self) -> &Path {
        self.tmp.path()
    }

    pub fn socket_dir(&self) -> PathBuf {
        self.root().join("run")
    }

    pub fn socket(&self) -> PathBuf {
        self.socket_dir().join("testanyware.sock")
    }

    /// `$XDG_STATE_HOME` of the CLI.
    pub fn xdg_state_home(&self) -> PathBuf {
        self.root().join("xdg-state")
    }

    /// `$XDG_DATA_HOME` of the CLI.
    pub fn xdg_data_home(&self) -> PathBuf {
        self.root().join("xdg-data")
    }

    /// The `--state-dir` of the app: the directory that holds `vms/`.
    pub fn state_dir(&self) -> PathBuf {
        self.xdg_state_home().join("testanyware")
    }

    pub fn data_dir(&self) -> PathBuf {
        self.xdg_data_home().join("testanyware")
    }

    /// Write the spec file of a VM whose In-VM agent is on loopback.
    pub fn add_vm(&self, id: &str, agent: SocketAddr) {
        // The app reads the spec as `testanyware_vm::spec::VmSpec`, which requires a `vnc`
        // section. Nothing listens on this one: the agent commands never use it.
        let spec = json!({
            "agent": { "host": agent.ip().to_string(), "port": agent.port() },
            "vnc": { "host": "127.0.0.1", "port": 1 },
            "platform": "macos",
        });
        std::fs::write(
            self.state_dir().join("vms").join(format!("{id}.json")),
            spec.to_string(),
        )
        .unwrap();
    }

    /// Run the CLI with the app's socket in `TESTANYWARE_APP_SOCKET`.
    pub fn cli(&self, args: &[&str]) -> Output {
        self.cli_with(
            args,
            &[("TESTANYWARE_APP_SOCKET", self.socket().to_str().unwrap())],
        )
    }

    /// Run the CLI as before the app: the variable is unset, so it dials the guest itself.
    pub fn cli_direct(&self, args: &[&str]) -> Output {
        self.cli_with(args, &[])
    }

    /// Run the CLI with `extra` in its environment. The CLI sees none of the environment of
    /// the test, so no `TESTANYWARE_*` variable of the developer leaks in.
    pub fn cli_with(&self, args: &[&str], extra: &[(&str, &str)]) -> Output {
        let mut command = Command::new(CLI);
        command
            .args(args)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", self.root())
            .env("XDG_STATE_HOME", self.xdg_state_home())
            .env("XDG_DATA_HOME", self.xdg_data_home());
        for (key, value) in extra {
            command.env(key, value);
        }
        command
            .output()
            .unwrap_or_else(|e| panic!("run `{CLI} {}`: {e}", args.join(" ")))
    }
}

// ---- reading an outcome ----

pub trait OutputExt {
    fn out(&self) -> String;
    fn err(&self) -> String;
    /// The stdout as the JSON envelope of `--json`.
    fn json(&self) -> Value;
    fn code(&self) -> i32;
}

impl OutputExt for Output {
    fn out(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    fn err(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }

    fn json(&self) -> Value {
        serde_json::from_slice(&self.stdout).unwrap_or_else(|e| {
            panic!(
                "stdout is not JSON ({e}): {}\nstderr: {}",
                self.out(),
                self.err()
            )
        })
    }

    fn code(&self) -> i32 {
        self.status.code().unwrap_or(-1)
    }
}

/// Bytes that both sides can regenerate: xorshift64 from a seed.
pub fn pattern(seed: u64, len: usize) -> Vec<u8> {
    let mut state = seed;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 24) as u8
        })
        .collect()
}
