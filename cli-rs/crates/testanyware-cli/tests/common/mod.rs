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

use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
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
        self.write_spec(id, Some(agent), None);
    }

    /// Write the spec file of a VM whose VNC server is on loopback. The VM has no agent.
    /// `password` and `logical` are what the spec holds for them, as `vm start` writes it.
    pub fn add_vnc_vm(&self, id: &str, vnc: &FakeRfb, logical: Option<(u32, u32)>) {
        self.write_spec(id, None, Some((vnc, logical)));
    }

    /// The path that `--connect` takes: a spec file outside the state directory.
    pub fn connect_spec(&self, name: &str, vnc: &FakeRfb) -> PathBuf {
        let path = self.root().join(name);
        std::fs::write(&path, spec_json(None, Some((vnc, None))).to_string()).unwrap();
        path
    }

    fn write_spec(
        &self,
        id: &str,
        agent: Option<SocketAddr>,
        vnc: Option<(&FakeRfb, Option<(u32, u32)>)>,
    ) {
        std::fs::write(
            self.state_dir().join("vms").join(format!("{id}.json")),
            spec_json(agent, vnc).to_string(),
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

/// The JSON of a spec file. The app reads it as `testanyware_vm::spec::VmSpec`, which requires
/// a `vnc` section, so a VM with no VNC server gets a placeholder that nothing dials.
fn spec_json(agent: Option<SocketAddr>, vnc: Option<(&FakeRfb, Option<(u32, u32)>)>) -> Value {
    let mut spec = json!({ "platform": "macos" });
    if let Some(agent) = agent {
        spec["agent"] = json!({ "host": agent.ip().to_string(), "port": agent.port() });
    }
    match vnc {
        Some((rfb, logical)) => {
            spec["vnc"] = json!({ "host": rfb.addr.ip().to_string(), "port": rfb.addr.port() });
            if let Some(password) = &rfb.password {
                spec["vnc"]["password"] = json!(password);
            }
            if let Some((width, height)) = logical {
                spec["logical"] = json!({ "width": width, "height": height });
            }
        }
        None => spec["vnc"] = json!({ "host": "127.0.0.1", "port": 1 }),
    }
    spec
}

// ---- the synthetic RFB server ----

/// What a guest received from an RFB client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RfbEvent {
    Key { down: bool, keysym: u32 },
    Pointer { buttons: u8, x: u16, y: u16 },
    UpdateRequest { incremental: bool, w: u16, h: u16 },
}

/// A synthetic RFB 3.8 server on loopback, for the VNC routing tests.
///
/// It offers security type None, or VNC authentication when it has a password. It answers
/// every framebuffer update request with the whole frame as one Raw rectangle, and it records
/// the key and pointer events it receives. The frame is the same on every connection, so a
/// capture through the app and a capture on the direct path print the same bytes.
pub struct FakeRfb {
    pub addr: SocketAddr,
    pub width: u16,
    pub height: u16,
    pub password: Option<String>,
    accepted: Arc<AtomicUsize>,
    events: Arc<Mutex<Vec<RfbEvent>>>,
}

impl FakeRfb {
    pub fn start(width: u16, height: u16, password: Option<&str>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind the fake RFB server");
        let addr = listener.local_addr().unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let events = Arc::new(Mutex::new(Vec::new()));
        let password = password.map(str::to_string);
        let (count, log, secret) = (Arc::clone(&accepted), Arc::clone(&events), password.clone());
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                let Ok(conn) = conn else { continue };
                count.fetch_add(1, Ordering::SeqCst);
                let (log, secret) = (Arc::clone(&log), secret.clone());
                std::thread::spawn(move || {
                    let _ = serve_rfb(conn, width, height, secret.as_deref(), &log);
                });
            }
        });
        FakeRfb {
            addr,
            width,
            height,
            password,
            accepted,
            events,
        }
    }

    /// How many connections the server accepted.
    pub fn accepted(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }

    /// The events received so far, oldest first.
    pub fn events(&self) -> Vec<RfbEvent> {
        self.events.lock().unwrap().clone()
    }

    /// Wait until `wanted` events satisfy `done`, because the CLI may exit before the server
    /// thread has read its last bytes.
    pub fn wait_for(&self, done: impl Fn(&[RfbEvent]) -> bool) -> Vec<RfbEvent> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let events = self.events();
            if done(&events) || Instant::now() > deadline {
                return events;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// One pixel of the frame, in the BGRX layout that the client asks for.
fn frame_pixel(x: u16, y: u16) -> [u8; 4] {
    [(x * 4) as u8, (y * 5) as u8, ((x + y) * 3) as u8, 0]
}

fn serve_rfb(
    mut conn: TcpStream,
    width: u16,
    height: u16,
    password: Option<&str>,
    log: &Mutex<Vec<RfbEvent>>,
) -> std::io::Result<()> {
    conn.set_read_timeout(Some(Duration::from_secs(10)))?;
    conn.write_all(b"RFB 003.008\n")?;
    let mut version = [0u8; 12];
    conn.read_exact(&mut version)?;
    let mut byte = [0u8; 1];
    match password {
        None => {
            conn.write_all(&[1, 1])?; // one security type: None
            conn.read_exact(&mut byte)?;
        }
        Some(password) => {
            conn.write_all(&[1, 2])?; // one security type: VNC authentication
            conn.read_exact(&mut byte)?;
            let challenge: [u8; 16] = std::array::from_fn(|i| i as u8 * 7 + 1);
            conn.write_all(&challenge)?;
            let mut response = [0u8; 16];
            conn.read_exact(&mut response)?;
            let expected = testanyware_rfb::auth::vnc_authenticate(password.as_bytes(), &challenge);
            if response != expected {
                conn.write_all(&1u32.to_be_bytes())?;
                return Ok(());
            }
        }
    }
    conn.write_all(&0u32.to_be_bytes())?; // SecurityResult: OK
    conn.read_exact(&mut byte)?; // ClientInit

    let mut init = Vec::new();
    init.extend_from_slice(&width.to_be_bytes());
    init.extend_from_slice(&height.to_be_bytes());
    init.extend_from_slice(&testanyware_rfb::PixelFormat::rgba32_le().encode());
    init.extend_from_slice(&4u32.to_be_bytes());
    init.extend_from_slice(b"fake");
    conn.write_all(&init)?;

    // The frame is one Raw rectangle, in the layout the client set.
    let mut update = vec![0u8, 0, 0, 1];
    for value in [0u16, 0, width, height] {
        update.extend_from_slice(&value.to_be_bytes());
    }
    update.extend_from_slice(&0i32.to_be_bytes()); // Raw
    for y in 0..height {
        for x in 0..width {
            update.extend_from_slice(&frame_pixel(x, y));
        }
    }

    let skip = |conn: &mut TcpStream, n: usize| -> std::io::Result<()> {
        let mut sink = vec![0u8; n];
        conn.read_exact(&mut sink)
    };
    loop {
        if conn.read(&mut byte)? == 0 {
            return Ok(());
        }
        match byte[0] {
            0 => skip(&mut conn, 19)?, // SetPixelFormat
            2 => {
                // SetEncodings
                let mut head = [0u8; 3];
                conn.read_exact(&mut head)?;
                skip(
                    &mut conn,
                    4 * u16::from_be_bytes([head[1], head[2]]) as usize,
                )?;
            }
            3 => {
                let mut body = [0u8; 9];
                conn.read_exact(&mut body)?;
                log.lock().unwrap().push(RfbEvent::UpdateRequest {
                    incremental: body[0] != 0,
                    w: u16::from_be_bytes([body[5], body[6]]),
                    h: u16::from_be_bytes([body[7], body[8]]),
                });
                conn.write_all(&update)?;
            }
            4 => {
                let mut body = [0u8; 7];
                conn.read_exact(&mut body)?;
                log.lock().unwrap().push(RfbEvent::Key {
                    down: body[0] != 0,
                    keysym: u32::from_be_bytes([body[3], body[4], body[5], body[6]]),
                });
            }
            5 => {
                let mut body = [0u8; 5];
                conn.read_exact(&mut body)?;
                log.lock().unwrap().push(RfbEvent::Pointer {
                    buttons: body[0],
                    x: u16::from_be_bytes([body[1], body[2]]),
                    y: u16::from_be_bytes([body[3], body[4]]),
                });
            }
            6 => {
                // ClientCutText
                let mut head = [0u8; 7];
                conn.read_exact(&mut head)?;
                skip(
                    &mut conn,
                    u32::from_be_bytes([head[3], head[4], head[5], head[6]]) as usize,
                )?;
            }
            other => {
                let _ = conn.shutdown(Shutdown::Both);
                return Err(std::io::Error::other(format!(
                    "unknown client message {other}"
                )));
            }
        }
    }
}
