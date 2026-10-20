//! The TestAnyware app executable.
//!
//! Without arguments, on macOS, it is the menu bar app: it serves the socket in the user's
//! `~/Library/Application Support/TestAnyware/` and shows the status item. The AppKit loop owns
//! the main thread, and the server runs on the asynchronous runtime's own threads (ADR-0005).
//!
//! With `--test-mode`, it has no status item, no login item and no LaunchServices. It takes
//! every path from its arguments, so a test never touches the user's own directories.

#[cfg(not(unix))]
fn main() {
    eprintln!("testanyware-app: the app runs on macOS hosts only");
    std::process::exit(1);
}

#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::process::ExitCode;
#[cfg(unix)]
use std::sync::Arc;

#[cfg(unix)]
use clap::Parser;
#[cfg(unix)]
use testanyware_app::gate::AppIdentity;
#[cfg(unix)]
use testanyware_app::listener::{bind_socket, BindError};
#[cfg(unix)]
use testanyware_app::{serve, AppContext};
#[cfg(unix)]
use testanyware_app_protocol::version::SUPPORTED_RANGE;
#[cfg(unix)]
use testanyware_app_protocol::Directories;
#[cfg(unix)]
use testanyware_vm::tools::ToolSearch;

#[cfg(unix)]
#[derive(Parser)]
#[command(
    name = "testanyware-app",
    version,
    about = "The TestAnyware app: a menu bar app that serves guest streams on a Unix socket"
)]
struct Args {
    /// Run without a status item, a login item or LaunchServices. Every path comes from an argument.
    #[arg(long)]
    test_mode: bool,
    /// The socket to serve on.
    #[arg(long, requires = "test_mode")]
    socket: Option<PathBuf>,
    /// The state directory.
    #[arg(long, requires = "test_mode")]
    state_dir: Option<PathBuf>,
    /// The data directory.
    #[arg(long, requires = "test_mode")]
    data_dir: Option<PathBuf>,
    /// The configuration path. Nothing reads it yet.
    #[arg(long, requires = "test_mode")]
    config: Option<PathBuf>,
    /// A directory to search for tools such as `tart`, in place of the built-in search. May repeat.
    #[arg(long = "tool-dir", requires = "test_mode")]
    tool_dirs: Vec<PathBuf>,
    /// The port of the In-VM agent in a tart guest, for the probes of a start and for the agent
    /// endpoint of its spec. A fake agent listens on it, or nothing does and each probe is refused.
    #[arg(long = "test-agent-port", requires = "test_mode")]
    agent_port: Option<u16>,
    #[command(subcommand)]
    command: Option<Command>,
}

/// The app's executable also answers the hidden command that a VNC keeper runs. `vm start`
/// spawns the keeper from the executable that starts the VM, so a keeper always matches the
/// app that started it (ADR-0018, spec: "Lifecycle and adoption").
#[cfg(unix)]
#[derive(clap::Subcommand)]
enum Command {
    #[command(name = "_vnc-keeper", hide = true)]
    VncKeeper {
        #[arg(long)]
        id: String,
        #[arg(long)]
        host: String,
        #[arg(long)]
        port: u16,
    },
}

/// Where the app serves and which directories it holds VM records in.
#[cfg(unix)]
struct Paths {
    socket: PathBuf,
    state_dir: PathBuf,
    data_dir: PathBuf,
    config: Option<PathBuf>,
    /// Where the app looks for the tools it runs.
    tools: ToolSearch,
}

#[cfg(unix)]
fn main() -> ExitCode {
    let args = Args::parse();
    if let Some(Command::VncKeeper { id, host, port }) = &args.command {
        return run_keeper(id, host, *port);
    }
    let paths = match (args.test_mode, args.socket, args.state_dir, args.data_dir) {
        (true, Some(socket), Some(state_dir), Some(data_dir)) => Paths {
            socket,
            state_dir,
            data_dir,
            config: args.config,
            tools: if args.tool_dirs.is_empty() {
                ToolSearch::builtin()
            } else {
                ToolSearch::new(args.tool_dirs)
            },
        },
        (true, ..) => {
            eprintln!("testanyware-app: test mode needs --socket, --state-dir and --data-dir.");
            return ExitCode::from(2);
        }
        (false, ..) => match app_paths() {
            Some(paths) => paths,
            None => return ExitCode::from(2),
        },
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build the asynchronous runtime");
    let own_uid = nix::unistd::geteuid().as_raw();
    let listener = match runtime.block_on(bind_socket(&paths.socket, own_uid)) {
        Ok(l) => l,
        // The login item can start a second app. That is no failure.
        Err(e @ BindError::AlreadyRunning(_)) => {
            eprintln!("testanyware-app: {e}");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("testanyware-app: {e}");
            return ExitCode::FAILURE;
        }
    };

    let ctx = Arc::new(AppContext::new(
        AppIdentity {
            build: env!("CARGO_PKG_VERSION").to_string(),
            protocol_range: SUPPORTED_RANGE,
            directories: Directories {
                state_dir: paths.state_dir,
                data_dir: paths.data_dir,
            },
        },
        own_uid,
        paths.config,
    ).with_tools(paths.tools).with_agent_port(args.agent_port.unwrap_or(testanyware_vm::start_context::AGENT_PORT)));
    eprintln!("testanyware-app: serving on {}", paths.socket.display());

    if args.test_mode {
        run_headless(runtime, listener, ctx, paths.socket)
    } else {
        run_menu_bar_app(runtime, listener, ctx, paths.socket)
    }
}

/// Hold one silent connection to a guest's VNC port until the port goes away. No status item,
/// no socket and no server: the keeper is a plain process of the app's executable.
#[cfg(unix)]
fn run_keeper(id: &str, host: &str, port: u16) -> ExitCode {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build the asynchronous runtime");
    let code = runtime.block_on(testanyware_vm::keeper::run(id, host, port));
    ExitCode::from(code as u8)
}

/// Serve until a signal or a fatal accept error. Test mode runs here.
#[cfg(unix)]
fn run_headless(
    runtime: tokio::runtime::Runtime,
    listener: tokio::net::UnixListener,
    ctx: Arc<AppContext>,
    socket: PathBuf,
) -> ExitCode {
    let code = runtime.block_on(async {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("install the SIGTERM handler");
        tokio::select! {
            result = serve(listener, ctx) => match result {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => { eprintln!("testanyware-app: {e}"); ExitCode::FAILURE }
            },
            _ = tokio::signal::ctrl_c() => ExitCode::SUCCESS,
            _ = terminate.recv() => ExitCode::SUCCESS,
        }
    });
    // A clean stop leaves no socket file. A killed app leaves one, and the next start removes it.
    let _ = std::fs::remove_file(&socket);
    code
}

/// The user's own directories: the socket in Application Support, and the VM records where the
/// Host CLI reads them from, since the handshake compares the two.
#[cfg(target_os = "macos")]
fn app_paths() -> Option<Paths> {
    use testanyware_app_protocol::default_socket_path;
    use testanyware_vm::paths::VmPaths;

    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        eprintln!("testanyware-app: HOME is not set.");
        return None;
    };
    let vm_paths = VmPaths::from_process_env();
    Some(Paths {
        socket: default_socket_path(&home),
        state_dir: vm_paths.state_dir().to_path_buf(),
        data_dir: vm_paths.data_dir().to_path_buf(),
        config: None,
        // The app has no shell environment, so it reads no `PATH`.
        tools: ToolSearch::builtin(),
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn app_paths() -> Option<Paths> {
    eprintln!("testanyware-app: the menu bar app runs on macOS hosts only. Pass --test-mode to serve without it.");
    None
}

/// The server runs on the runtime's threads, and AppKit takes this thread until the app quits.
#[cfg(target_os = "macos")]
fn run_menu_bar_app(
    runtime: tokio::runtime::Runtime,
    listener: tokio::net::UnixListener,
    ctx: Arc<AppContext>,
    socket: PathBuf,
) -> ExitCode {
    use std::sync::atomic::{AtomicBool, Ordering};

    let serving = Arc::new(AtomicBool::new(true));
    runtime.spawn({
        let ctx = Arc::clone(&ctx);
        let serving = Arc::clone(&serving);
        async move {
            if let Err(e) = serve(listener, ctx).await {
                eprintln!("testanyware-app: {e}");
            }
            // The menu says so. The app stays up, so a person can see it and quit.
            serving.store(false, Ordering::SeqCst);
        }
    });
    testanyware_app::shell::run(testanyware_app::shell::ShellConfig {
        ctx,
        serving,
        socket,
    });
    // `run` returns after `terminate:`, which runs `applicationWillTerminate:` first.
    ExitCode::SUCCESS
}

#[cfg(all(unix, not(target_os = "macos")))]
fn run_menu_bar_app(
    _runtime: tokio::runtime::Runtime,
    _listener: tokio::net::UnixListener,
    _ctx: Arc<AppContext>,
    _socket: PathBuf,
) -> ExitCode {
    unreachable!("app_paths refuses a run without test mode on this host")
}
