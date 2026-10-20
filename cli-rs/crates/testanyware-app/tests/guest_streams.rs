//! Guest streams, end to end: the real `testanyware-app` executable in test mode, raw HTTP on
//! its socket, and fake guests on loopback. The In-VM agent is a wiremock server. The RFB
//! server is a synthetic one that speaks the RFB handshake and then moves counted bytes.

#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tempfile::TempDir;
use testanyware_app_protocol::{
    ClientHello, Directories, ProtocolRange, StreamKind, StreamTarget, STREAM_UPGRADE_TOKEN,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Every test holds this lock. macOS has no atomic close-on-exec for a new socket or pipe, so a
/// thread that spawns the app can hand its child a socket that a sibling thread has just
/// opened. Each test also starts its fakes before it spawns the app, and opens no socket in
/// another thread until the app runs.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

const APP: &str = env!("CARGO_BIN_EXE_testanyware-app");
const BUILD: &str = env!("CARGO_PKG_VERSION");
const WAIT: Duration = Duration::from_secs(30);

// ---- the app ----

struct TestApp {
    child: Child,
    socket: PathBuf,
    dirs: Directories,
    _tmp: TempDir,
}

impl Drop for TestApp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl TestApp {
    /// Start the app on a fresh socket and fresh directories.
    fn start() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let socket = tmp.path().join("run").join("testanyware.sock");
        let dirs = Directories {
            state_dir: tmp.path().join("state"),
            data_dir: tmp.path().join("data"),
        };
        std::fs::create_dir_all(dirs.state_dir.join("vms")).unwrap();
        let mut child = Command::new(APP)
            .args(["--test-mode", "--socket"])
            .arg(&socket)
            .arg("--state-dir")
            .arg(&dirs.state_dir)
            .arg("--data-dir")
            .arg(&dirs.data_dir)
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn the app");
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                panic!("the app exited early with {status}");
            }
            if UnixStream::connect(&socket).is_ok() {
                break;
            }
            assert!(Instant::now() < deadline, "the app did not serve in 10 s");
            std::thread::sleep(Duration::from_millis(20));
        }
        TestApp {
            child,
            socket,
            dirs,
            _tmp: tmp,
        }
    }

    fn hello(&self) -> ClientHello {
        ClientHello::new(ProtocolRange::new(1, 1), BUILD, self.dirs.clone())
    }

    /// Write the spec sidecar of a VM whose guest endpoints are on loopback.
    fn add_vm(&self, id: &str, agent: Option<SocketAddr>, vnc: SocketAddr) {
        let mut spec = json!({
            "vnc": { "host": vnc.ip().to_string(), "port": vnc.port() },
            "platform": "macos",
        });
        if let Some(agent) = agent {
            spec["agent"] = json!({ "host": agent.ip().to_string(), "port": agent.port() });
        }
        std::fs::write(
            self.dirs.state_dir.join("vms").join(format!("{id}.json")),
            spec.to_string(),
        )
        .unwrap();
    }

    /// Ask for a guest stream. `Ok` is the connection after the `101`.
    fn open(&self, target: &StreamTarget, kind: StreamKind) -> Result<UnixStream, Reply> {
        self.open_path(&target.path(kind), "GET", true, Some(&self.hello()))
    }

    fn open_path(
        &self,
        path: &str,
        http_method: &str,
        upgrade: bool,
        hello: Option<&ClientHello>,
    ) -> Result<UnixStream, Reply> {
        let mut stream = UnixStream::connect(&self.socket).expect("connect");
        stream.set_read_timeout(Some(WAIT)).unwrap();
        stream.set_write_timeout(Some(WAIT)).unwrap();
        let mut request = format!("{http_method} {path} HTTP/1.1\r\nHost: testanyware\r\n");
        if upgrade {
            request.push_str(&format!(
                "Connection: Upgrade\r\nUpgrade: {STREAM_UPGRADE_TOKEN}\r\n"
            ));
        }
        for (name, value) in hello.map(ClientHello::to_headers).unwrap_or_default() {
            request.push_str(&format!("{name}: {value}\r\n"));
        }
        request.push_str("\r\n");
        stream.write_all(request.as_bytes()).unwrap();
        let reply = read_reply(&mut stream);
        if reply.status == 101 {
            Ok(stream)
        } else {
            Err(reply)
        }
    }
}

/// A response head and its body, as the raw client reads them.
#[derive(Debug)]
struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|e| panic!("body is not JSON ({e}): {:?}", self.body))
    }
}

/// Read the head one byte at a time, so no byte of the stream that follows a `101` is consumed.
fn read_reply(stream: &mut UnixStream) -> Reply {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        stream
            .read_exact(&mut byte)
            .expect("read the response head");
        head.push(byte[0]);
    }
    let head = String::from_utf8(head).expect("utf-8 head");
    let mut lines = head.lines();
    let status = lines
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect();
    let length = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .map(|(_, v)| v.parse::<usize>().unwrap())
        .unwrap_or(0);
    let mut body = vec![0u8; length];
    if status != 101 {
        stream
            .read_exact(&mut body)
            .expect("read the response body");
    }
    Reply {
        status,
        headers,
        body,
    }
}

fn error_of(result: Result<UnixStream, Reply>) -> Reply {
    match result {
        Ok(_) => panic!("the app upgraded, and an error was expected"),
        Err(reply) => reply,
    }
}

// ---- the fake guests ----

/// A wiremock In-VM agent on loopback. It owns the runtime that serves it.
struct FakeAgent {
    _runtime: tokio::runtime::Runtime,
    server: MockServer,
}

impl FakeAgent {
    fn start() -> Self {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let server = runtime.block_on(async {
            let server = MockServer::start().await;
            Mock::given(method("GET"))
                .and(path("/health"))
                .respond_with(ResponseTemplate::new(200).set_body_string("healthy-from-the-agent"))
                .mount(&server)
                .await;
            server
        });
        FakeAgent {
            _runtime: runtime,
            server,
        }
    }

    fn addr(&self) -> SocketAddr {
        *self.server.address()
    }

    fn requests(&self) -> usize {
        self._runtime
            .block_on(self.server.received_requests())
            .unwrap_or_default()
            .len()
    }
}

/// Bytes of a stream that both sides can regenerate: xorshift64 from a seed.
struct Pattern {
    state: u64,
}

impl Pattern {
    fn new(seed: u64) -> Self {
        Pattern { state: seed }
    }

    fn fill(&mut self, buf: &mut [u8]) {
        for b in buf {
            self.state ^= self.state << 13;
            self.state ^= self.state >> 7;
            self.state ^= self.state << 17;
            *b = (self.state >> 24) as u8;
        }
    }
}

/// FNV-1a over a byte stream.
struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(0xcbf29ce484222325)
    }

    fn update(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ u64::from(*b)).wrapping_mul(0x100000001b3);
        }
    }
}

fn pattern_hash(seed: u64, len: u64) -> u64 {
    let (mut pattern, mut hash) = (Pattern::new(seed), Fnv::new());
    let mut buf = vec![0u8; 64 * 1024];
    let mut left = len;
    while left > 0 {
        let n = left.min(buf.len() as u64) as usize;
        pattern.fill(&mut buf[..n]);
        hash.update(&buf[..n]);
        left -= n as u64;
    }
    hash.0
}

/// What the server tells a client in `ServerInit`.
fn server_init() -> Vec<u8> {
    let mut init = Vec::new();
    init.extend_from_slice(&16u16.to_be_bytes()); // width
    init.extend_from_slice(&16u16.to_be_bytes()); // height
    init.extend_from_slice(&[32, 24, 0, 1]); // bits per pixel, depth, big-endian, true colour
    init.extend_from_slice(&[0, 255, 0, 255, 0, 255]); // maxima
    init.extend_from_slice(&[16, 8, 0]); // shifts
    init.extend_from_slice(&[0, 0, 0]); // padding
    init.extend_from_slice(&9u32.to_be_bytes());
    init.extend_from_slice(b"synthetic");
    init
}

const DOWN_SEED: u64 = 0x5eed_0001;
const UP_SEED: u64 = 0x5eed_0002;

/// A synthetic RFB server. Each connection runs the RFB handshake, sends `down` pattern bytes,
/// reads until the client ends its half, and then sends a trailer: the count and the FNV hash
/// of what it received, each as 8 bytes big-endian. Then it closes.
struct FakeRfb {
    addr: SocketAddr,
    accepted: Arc<AtomicUsize>,
    closed: Arc<AtomicUsize>,
}

impl FakeRfb {
    fn start(down: u64) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let accepted = Arc::new(AtomicUsize::new(0));
        let closed = Arc::new(AtomicUsize::new(0));
        let (a, c) = (Arc::clone(&accepted), Arc::clone(&closed));
        std::thread::spawn(move || {
            for conn in listener.incoming() {
                let Ok(conn) = conn else { continue };
                a.fetch_add(1, Ordering::SeqCst);
                let c = Arc::clone(&c);
                std::thread::spawn(move || {
                    let _ = serve_rfb(conn, down);
                    c.fetch_add(1, Ordering::SeqCst);
                });
            }
        });
        FakeRfb {
            addr,
            accepted,
            closed,
        }
    }

    fn accepted(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }

    fn closed(&self) -> usize {
        self.closed.load(Ordering::SeqCst)
    }
}

fn serve_rfb(mut conn: TcpStream, down: u64) -> std::io::Result<()> {
    conn.set_read_timeout(Some(WAIT))?;
    conn.write_all(b"RFB 003.008\n")?;
    let mut version = [0u8; 12];
    conn.read_exact(&mut version)?;
    conn.write_all(&[1, 1])?; // one security type: None
    let mut chosen = [0u8; 1];
    conn.read_exact(&mut chosen)?;
    conn.write_all(&0u32.to_be_bytes())?; // SecurityResult: OK
    conn.read_exact(&mut chosen)?; // ClientInit
    conn.write_all(&server_init())?;

    let mut sender = conn.try_clone()?;
    let writer = std::thread::spawn(move || -> std::io::Result<()> {
        let mut pattern = Pattern::new(DOWN_SEED);
        let mut buf = vec![0u8; 64 * 1024];
        let mut left = down;
        while left > 0 {
            let n = left.min(buf.len() as u64) as usize;
            pattern.fill(&mut buf[..n]);
            sender.write_all(&buf[..n])?;
            left -= n as u64;
        }
        Ok(())
    });

    let (mut count, mut hash) = (0u64, Fnv::new());
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = match conn.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => break,
        };
        count += n as u64;
        hash.update(&buf[..n]);
    }
    let _ = writer.join();
    conn.write_all(&count.to_be_bytes())?;
    conn.write_all(&hash.0.to_be_bytes())?;
    conn.shutdown(Shutdown::Write)
}

/// The client half of the handshake. Returns the `ServerInit` it read.
fn rfb_client_handshake(stream: &mut UnixStream) -> Vec<u8> {
    let mut banner = [0u8; 12];
    stream.read_exact(&mut banner).unwrap();
    assert_eq!(&banner, b"RFB 003.008\n");
    stream.write_all(&banner).unwrap();
    let mut types = [0u8; 2];
    stream.read_exact(&mut types).unwrap();
    assert_eq!(types, [1, 1]);
    stream.write_all(&[1]).unwrap();
    let mut result = [0u8; 4];
    stream.read_exact(&mut result).unwrap();
    assert_eq!(result, [0, 0, 0, 0]);
    stream.write_all(&[1]).unwrap(); // ClientInit: shared
    let mut init = vec![0u8; server_init().len()];
    stream.read_exact(&mut init).unwrap();
    init
}

fn vm(id: &str) -> StreamTarget {
    StreamTarget::Vm { id: id.to_string() }
}

fn endpoint(addr: SocketAddr) -> StreamTarget {
    StreamTarget::Endpoint {
        host: addr.ip().to_string(),
        port: addr.port(),
    }
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// An HTTP request through a stream, answered to the end of the connection.
fn http_get_through(stream: &mut UnixStream, target: &str) -> String {
    stream
        .write_all(
            format!("GET {target} HTTP/1.1\r\nHost: guest\r\nConnection: close\r\n\r\n").as_bytes(),
        )
        .unwrap();
    let mut text = String::new();
    stream.read_to_string(&mut text).unwrap();
    text
}

// ---- the agent through a stream ----

#[test]
fn a_vm_agent_stream_carries_http_to_the_guests_agent() {
    let _serial = serial();
    let agent = FakeAgent::start();
    let rfb = FakeRfb::start(0);
    let app = TestApp::start();
    app.add_vm("testanyware-aabbccdd", Some(agent.addr()), rfb.addr);

    let mut stream = app
        .open(&vm("testanyware-aabbccdd"), StreamKind::Agent)
        .unwrap_or_else(|reply| panic!("no upgrade: {reply:?}"));
    let answer = http_get_through(&mut stream, "/health");

    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    assert!(answer.ends_with("healthy-from-the-agent"), "{answer}");
    assert_eq!(agent.requests(), 1);
    assert_eq!(
        rfb.accepted(),
        0,
        "an agent stream dials the agent and nothing else"
    );
}

#[test]
fn the_upgrade_answer_names_the_token_and_the_app() {
    let _serial = serial();
    let agent = FakeAgent::start();
    let app = TestApp::start();

    let mut stream = UnixStream::connect(&app.socket).unwrap();
    stream.set_read_timeout(Some(WAIT)).unwrap();
    let path = endpoint(agent.addr()).path(StreamKind::Agent);
    let mut request = format!(
        "GET {path} HTTP/1.1\r\nHost: t\r\nConnection: Upgrade\r\nUpgrade: {STREAM_UPGRADE_TOKEN}\r\n"
    );
    for (name, value) in app.hello().to_headers() {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    request.push_str("\r\n");
    stream.write_all(request.as_bytes()).unwrap();
    let reply = read_reply(&mut stream);

    assert_eq!(reply.status, 101);
    assert_eq!(reply.header("upgrade"), Some(STREAM_UPGRADE_TOKEN));
    assert!(reply
        .header("connection")
        .unwrap()
        .eq_ignore_ascii_case("upgrade"));
    assert_eq!(reply.header("x-testanyware-protocol"), Some("1-1"));
    assert_eq!(reply.header("x-testanyware-build"), Some(BUILD));
}

#[test]
fn an_endpoint_stream_uses_the_endpoint_that_the_request_names() {
    let _serial = serial();
    let agent = FakeAgent::start();
    let app = TestApp::start();
    // No VM exists. The request carries the whole address.
    let mut stream = app
        .open(&endpoint(agent.addr()), StreamKind::Agent)
        .unwrap_or_else(|reply| panic!("no upgrade: {reply:?}"));
    let answer = http_get_through(&mut stream, "/health");
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    assert_eq!(agent.requests(), 1);
}

// ---- RFB through a stream ----

#[test]
fn a_vnc_stream_passes_the_rfb_handshake_unchanged() {
    let _serial = serial();
    let rfb = FakeRfb::start(0);
    let app = TestApp::start();
    app.add_vm("vm-1", None, rfb.addr);

    let mut stream = app
        .open(&vm("vm-1"), StreamKind::Vnc)
        .unwrap_or_else(|reply| panic!("no upgrade: {reply:?}"));
    assert_eq!(rfb_client_handshake(&mut stream), server_init());
}

#[test]
fn a_vnc_endpoint_stream_reaches_the_endpoint() {
    let _serial = serial();
    let rfb = FakeRfb::start(0);
    let app = TestApp::start();
    let mut stream = app
        .open(&endpoint(rfb.addr), StreamKind::Vnc)
        .unwrap_or_else(|reply| panic!("no upgrade: {reply:?}"));
    assert_eq!(rfb_client_handshake(&mut stream), server_init());
}

#[test]
fn a_half_close_passes_through_to_the_guest_and_the_answer_comes_back() {
    let _serial = serial();
    let down = 4096;
    let rfb = FakeRfb::start(down);
    let app = TestApp::start();
    app.add_vm("vm-1", None, rfb.addr);
    let mut stream = app.open(&vm("vm-1"), StreamKind::Vnc).unwrap();
    rfb_client_handshake(&mut stream);

    // The client sends 1000 bytes and ends its half. The guest answers only after it has read
    // the end of the stream, so the answer proves that the end reached the guest.
    let mut up = vec![0u8; 1000];
    Pattern::new(UP_SEED).fill(&mut up);
    stream.write_all(&up).unwrap();
    stream.shutdown(Shutdown::Write).unwrap();

    let mut answer = Vec::new();
    stream.read_to_end(&mut answer).unwrap();
    assert_eq!(
        answer.len() as u64,
        down + 16,
        "the pattern, then the 16-byte trailer"
    );
    let (pattern, trailer) = answer.split_at(down as usize);
    let mut expected = vec![0u8; down as usize];
    Pattern::new(DOWN_SEED).fill(&mut expected);
    assert_eq!(
        pattern,
        expected.as_slice(),
        "the guest's bytes changed in the relay"
    );
    assert_eq!(u64::from_be_bytes(trailer[..8].try_into().unwrap()), 1000);
    assert_eq!(
        u64::from_be_bytes(trailer[8..].try_into().unwrap()),
        pattern_hash(UP_SEED, 1000),
        "the client's bytes changed in the relay"
    );
}

#[test]
fn a_large_transfer_passes_in_both_directions_unchanged() {
    let _serial = serial();
    const MIB: u64 = 1024 * 1024;
    let (down, up) = (48 * MIB, 32 * MIB);
    let rfb = FakeRfb::start(down);
    let app = TestApp::start();
    app.add_vm("vm-1", None, rfb.addr);
    let mut stream = app.open(&vm("vm-1"), StreamKind::Vnc).unwrap();
    rfb_client_handshake(&mut stream);

    // Both directions run at once, as they do for a framebuffer and its input events.
    let mut sender = stream.try_clone().unwrap();
    let writer = std::thread::spawn(move || {
        let mut pattern = Pattern::new(UP_SEED);
        let mut buf = vec![0u8; 64 * 1024];
        let mut left = up;
        while left > 0 {
            let n = left.min(buf.len() as u64) as usize;
            pattern.fill(&mut buf[..n]);
            sender.write_all(&buf[..n]).unwrap();
            left -= n as u64;
        }
        sender.shutdown(Shutdown::Write).unwrap();
    });

    let (mut pattern, mut hash) = (Pattern::new(DOWN_SEED), Fnv::new());
    let (mut expected, mut got) = (vec![0u8; 64 * 1024], vec![0u8; 64 * 1024]);
    let mut left = down;
    while left > 0 {
        let n = left.min(got.len() as u64) as usize;
        stream.read_exact(&mut got[..n]).unwrap();
        pattern.fill(&mut expected[..n]);
        assert!(
            got[..n] == expected[..n],
            "the guest's bytes changed, {left} bytes from the end"
        );
        hash.update(&got[..n]);
        left -= n as u64;
    }
    let mut trailer = [0u8; 16];
    stream.read_exact(&mut trailer).unwrap();
    writer.join().unwrap();

    assert_eq!(u64::from_be_bytes(trailer[..8].try_into().unwrap()), up);
    assert_eq!(
        u64::from_be_bytes(trailer[8..].try_into().unwrap()),
        pattern_hash(UP_SEED, up),
        "the client's bytes changed in the relay"
    );
    assert_ne!(hash.0, 0);
}

// ---- one guest connection for each stream ----

#[test]
fn each_stream_has_its_own_guest_connection_and_the_app_closes_it_with_the_client() {
    let _serial = serial();
    let rfb = FakeRfb::start(0);
    let app = TestApp::start();
    app.add_vm("vm-1", None, rfb.addr);

    let mut first = app.open(&vm("vm-1"), StreamKind::Vnc).unwrap();
    rfb_client_handshake(&mut first);
    let mut second = app.open(&vm("vm-1"), StreamKind::Vnc).unwrap();
    rfb_client_handshake(&mut second);
    assert_eq!(rfb.accepted(), 2, "two streams, two guest connections");
    assert_eq!(rfb.closed(), 0);

    // The first client closes. Only its guest connection closes.
    drop(first);
    wait_until("the first guest connection to close", || rfb.closed() == 1);
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(rfb.closed(), 1, "the second stream must stay open");

    // A stream that opens later dials again. The app reuses nothing.
    let mut third = app.open(&vm("vm-1"), StreamKind::Vnc).unwrap();
    rfb_client_handshake(&mut third);
    assert_eq!(rfb.accepted(), 3);

    drop(second);
    drop(third);
    wait_until("both guest connections to close", || rfb.closed() == 3);
}

#[test]
fn an_open_stream_has_no_timeout_of_its_own() {
    let _serial = serial();
    let rfb = FakeRfb::start(0);
    let app = TestApp::start();
    app.add_vm("vm-1", None, rfb.addr);
    let mut stream = app.open(&vm("vm-1"), StreamKind::Vnc).unwrap();
    rfb_client_handshake(&mut stream);
    // A quiet stream stays open past any short timer. The app has none to hit, and the fake
    // guest waits for the client's end.
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(rfb.closed(), 0);
    stream.shutdown(Shutdown::Write).unwrap();
    let mut trailer = Vec::new();
    stream.read_to_end(&mut trailer).unwrap();
    assert_eq!(trailer.len(), 16);
}

// ---- what fails before the upgrade ----

#[test]
fn a_refused_dial_gives_the_error_object_with_the_errno_and_no_upgrade() {
    let _serial = serial();
    // A port that nothing listens on.
    let closed_port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let app = TestApp::start();

    let reply = error_of(app.open(&endpoint(closed_port), StreamKind::Agent));

    assert_eq!(reply.status, 502);
    let body = reply.json();
    assert_eq!(body["code"], "CONNECTION_REFUSED");
    assert!(body["message"]
        .as_str()
        .unwrap()
        .contains(&closed_port.port().to_string()));
    assert_eq!(body["details"]["errno"], nix::libc::ECONNREFUSED);
    assert_eq!(body["details"]["cause"], "connection_refused");
    assert_eq!(body["details"]["port"], closed_port.port());
    // A refusal is not a denial: the app does not retry it or ask `nc`.
    assert!(body.get("remediation").is_none());
    assert_eq!(reply.header("x-testanyware-protocol"), Some("1-1"));
}

#[test]
fn an_unknown_vm_is_vm_not_found() {
    let _serial = serial();
    let app = TestApp::start();
    let reply = error_of(app.open(&vm("testanyware-00000000"), StreamKind::Agent));
    assert_eq!(reply.status, 404);
    let body = reply.json();
    assert_eq!(body["code"], "VM_NOT_FOUND");
    assert!(body["details"]["spec_path"]
        .as_str()
        .unwrap()
        .ends_with("vms/testanyware-00000000.json"));
}

#[test]
fn a_vm_without_an_agent_in_its_spec_cannot_open_an_agent_stream() {
    let _serial = serial();
    let rfb = FakeRfb::start(0);
    let app = TestApp::start();
    app.add_vm("vm-1", None, rfb.addr);
    let reply = error_of(app.open(&vm("vm-1"), StreamKind::Agent));
    assert_eq!(reply.status, 409);
    assert_eq!(reply.json()["code"], "NO_CONNECTION_SPECIFIED");
    assert_eq!(rfb.accepted(), 0);
}

#[test]
fn an_id_that_climbs_out_of_the_state_directory_reads_no_file_and_dials_nothing() {
    let _serial = serial();
    let rfb = FakeRfb::start(0);
    let app = TestApp::start();
    // A decoy spec one level above `vms/`. A path join of `..` would read it.
    std::fs::write(
        app.dirs.state_dir.join("evil.json"),
        json!({
            "vnc": { "host": "127.0.0.1", "port": rfb.addr.port() },
            "agent": { "host": "127.0.0.1", "port": rfb.addr.port() },
            "platform": "macos",
        })
        .to_string(),
    )
    .unwrap();

    for id in ["..%2Fevil", "%2E%2E%2Fevil", "..", ".hidden", "a%2Fb"] {
        let path = format!("/v1/vms/{id}/vnc");
        let reply = error_of(app.open_path(&path, "GET", true, Some(&app.hello())));
        assert_eq!(reply.status, 404, "{id}");
        let body = reply.json();
        assert_eq!(body["code"], "VM_NOT_FOUND", "{id}");
        assert!(
            body["details"].get("spec_path").is_none(),
            "the app looked for {id}"
        );
    }
    assert_eq!(rfb.accepted(), 0, "the decoy was dialled");
}

#[test]
fn an_endpoint_that_is_no_host_and_port_is_invalid() {
    let _serial = serial();
    let app = TestApp::start();
    for (target, needle) in [
        ("host:0", "1..=65535"),
        ("no-port", "host:port"),
        ("%2Ftmp%2Fsock:80", "not a host name"),
        ("::1:80", "[address]:port"),
    ] {
        let path = format!("/v1/endpoints/{target}/agent");
        let reply = error_of(app.open_path(&path, "GET", true, Some(&app.hello())));
        assert_eq!(reply.status, 400, "{target}");
        let body = reply.json();
        assert_eq!(body["code"], "INVALID_ENDPOINT", "{target}");
        assert!(
            body["details"]["reason"].as_str().unwrap().contains(needle),
            "{target}"
        );
    }
}

#[test]
fn a_request_without_the_upgrade_headers_dials_nothing() {
    let _serial = serial();
    let rfb = FakeRfb::start(0);
    let app = TestApp::start();
    app.add_vm("vm-1", None, rfb.addr);
    let path = vm("vm-1").path(StreamKind::Vnc);

    let plain = error_of(app.open_path(&path, "GET", false, Some(&app.hello())));
    assert_eq!(plain.status, 400);
    assert_eq!(plain.json()["code"], "USAGE_ERROR");

    let post = error_of(app.open_path(&path, "POST", true, Some(&app.hello())));
    assert_eq!(post.status, 405);
    assert_eq!(post.json()["code"], "USAGE_ERROR");

    assert_eq!(rfb.accepted(), 0);
}

#[test]
fn a_stream_request_still_passes_the_handshake_before_it_dials() {
    let _serial = serial();
    let rfb = FakeRfb::start(0);
    let app = TestApp::start();
    app.add_vm("vm-1", None, rfb.addr);
    let path = vm("vm-1").path(StreamKind::Vnc);

    // Other directories.
    let other = ClientHello::new(
        ProtocolRange::new(1, 1),
        "9.9.9",
        Directories {
            state_dir: "/elsewhere/state".into(),
            data_dir: app.dirs.data_dir.clone(),
        },
    );
    let reply = error_of(app.open_path(&path, "GET", true, Some(&other)));
    assert_eq!(reply.status, 409);
    assert_eq!(reply.json()["details"]["cause"], "directories");

    // A range with no common version, and a silent client.
    let ahead = ClientHello::new(ProtocolRange::new(7, 8), "9.9.9", app.dirs.clone());
    let reply = error_of(app.open_path(&path, "GET", true, Some(&ahead)));
    assert_eq!(reply.json()["details"]["cause"], "protocol");
    let reply = error_of(app.open_path(&path, "GET", true, None));
    assert_eq!(reply.json()["details"]["cause"], "protocol");

    assert_eq!(rfb.accepted(), 0);
}

#[test]
fn the_app_keeps_serving_after_streams_end_and_fail() {
    let _serial = serial();
    let agent = FakeAgent::start();
    let app = TestApp::start();
    let closed_port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let _ = error_of(app.open(&endpoint(closed_port), StreamKind::Agent));
    for _ in 0..3 {
        let mut stream = app
            .open(&endpoint(agent.addr()), StreamKind::Agent)
            .unwrap();
        assert!(http_get_through(&mut stream, "/health").starts_with("HTTP/1.1 200"));
    }
    assert_eq!(agent.requests(), 3);
    let status = error_of(app.open_path("/v1/status", "GET", false, None));
    assert_eq!(status.status, 200, "{status:?}");
    assert_eq!(status.json()["build"], BUILD);
}

#[test]
fn the_socket_directory_of_a_test_is_the_only_path_the_app_opens() {
    // The app reads a VM spec by id and dials TCP. A request cannot name a path of its own.
    let _serial = serial();
    let app = TestApp::start();
    let outside: &Path = app.socket.parent().unwrap();
    let path = format!("/v1/endpoints/{}:80/agent", outside.display());
    let reply = error_of(app.open_path(&path, "GET", true, Some(&app.hello())));
    // A raw slash splits the path into more segments, so it is no route at all.
    assert_eq!(reply.status, 404);
    assert_eq!(reply.json()["code"], "USAGE_ERROR");
}
