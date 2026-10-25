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
use testanyware_app::config::{stat_file, AppConfig};
#[cfg(all(unix, target_os = "macos"))]
use testanyware_app::default_file::plan;
#[cfg(unix)]
use testanyware_app::default_file::{write_if_absent, Written};
#[cfg(unix)]
use testanyware_app::gate::AppIdentity;
#[cfg(unix)]
use testanyware_app::listener::{bind_socket, BindError};
#[cfg(unix)]
use testanyware_app::{serve, AppContext};
#[cfg(unix)]
use testanyware_app_protocol::version::SUPPORTED_RANGE;
#[cfg(unix)]
use testanyware_app_protocol::{Directories, ProtocolRange};
#[cfg(unix)]
use testanyware_config::{locate, ConfigLocation};
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
    /// The `config.rhai` to evaluate. Without it, a test mode run reads no file, so that a
    /// developer's own file never reaches a test. The file beats `--state-dir` and `--data-dir`,
    /// as it beats the built-in defaults of the app.
    #[arg(long, requires = "test_mode")]
    config: Option<PathBuf>,
    /// A directory to search for tools such as `tart`, in place of the built-in search. May repeat.
    #[arg(long = "tool-dir", requires = "test_mode")]
    tool_dirs: Vec<PathBuf>,
    /// The port of the In-VM agent in a tart guest, for the probes of a start and for the agent
    /// endpoint of its spec. A fake agent listens on it, or nothing does and each probe is refused.
    #[arg(long = "test-agent-port", requires = "test_mode")]
    agent_port: Option<u16>,
    /// The deadline of each probe of an adopted VM, in milliseconds, in place of the default.
    #[arg(long = "test-probe-deadline-ms", requires = "test_mode")]
    probe_deadline_ms: Option<u64>,
    /// The build that the app states in place of its own, so that a test forges a skew.
    #[arg(long = "test-build", requires = "test_mode")]
    build: Option<String>,
    /// The protocol range that the app states in place of its own, as `MIN-MAX`.
    #[arg(long = "test-protocol-range", requires = "test_mode")]
    protocol_range: Option<ProtocolRange>,
    /// The build of the bundle on disk, in place of the one that the executable's own bundle
    /// holds. A test mode run has no bundle, so without this the app states none.
    #[arg(long = "test-bundle-build", requires = "test_mode")]
    bundle_build: Option<String>,
    /// How long a quit waits for the cleanup of the starts that it cancelled, in milliseconds, in
    /// place of 10 s.
    #[arg(long = "test-quit-wait-ms", requires = "test_mode")]
    quit_wait_ms: Option<u64>,
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
    /// The built-in directories. `config.rhai` beats them.
    state_dir: PathBuf,
    data_dir: PathBuf,
    /// The file the app evaluates, if any.
    config: ConfigLocation,
    /// The default file and its text, when this run may write it before it evaluates.
    default_file: Option<(PathBuf, String)>,
    /// Where the app looks for the tools it runs, before `config.rhai` locates any.
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
            config: args
                .config
                .map_or(ConfigLocation::Disabled, ConfigLocation::File),
            // A test-mode run writes nothing, so a test keeps its file or its absent file.
            default_file: None,
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

    // The default file is written first, so the app evaluates the file it wrote and reports no
    // change. A failed write leaves the app as with no file.
    if let Some((file, text)) = &paths.default_file {
        match write_if_absent(file, text) {
            Ok(Written::Created) => {
                eprintln!("testanyware-app: wrote the default {}", file.display());
            }
            Ok(Written::EntryThere) => {}
            Err(error) => eprintln!(
                "testanyware-app: cannot write the default {}: {error}",
                file.display()
            ),
        }
    }
    // The app evaluates the file once, here. A restart reloads it. A file that fails leaves the
    // built-in directories and tools, and the menu and the status show the error.
    let config = AppConfig::evaluate_now(&paths.config);
    if let Some(error) = config.status(&stat_file).error {
        eprintln!("testanyware-app: {}", error.message);
    }
    let directories = config.directories(Directories {
        state_dir: paths.state_dir,
        data_dir: paths.data_dir,
    });
    let tools = config.tool_search(paths.tools);

    let mut ctx = AppContext::new(
        AppIdentity {
            build: args
                .build
                .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string()),
            protocol_range: args.protocol_range.unwrap_or(SUPPORTED_RANGE),
            directories,
        },
        own_uid,
        config,
    )
    .with_bundle(match args.bundle_build {
        Some(build) => testanyware_app::bundle::BundleSource::Fixed(Some(build)),
        None => testanyware_app::bundle::BundleSource::this_process(),
    })
    .with_tools(tools)
    .with_agent_port(
        args.agent_port
            .unwrap_or(testanyware_vm::start_context::AGENT_PORT),
    )
    .with_probe_deadline(args.probe_deadline_ms.map_or(
        testanyware_app::adoption::PROBE_DEADLINE,
        std::time::Duration::from_millis,
    ))
    .with_quit_wait(args.quit_wait_ms.map_or(
        testanyware_app::quit::QUIT_WAIT,
        std::time::Duration::from_millis,
    ));
    // The test mode has no login item to ask. It states `unobserved`.
    #[cfg(target_os = "macos")]
    if !args.test_mode {
        ctx = ctx.with_login_item(testanyware_app::shell::login_item_state);
    }
    let ctx = Arc::new(ctx);
    // A start record names a start that a crash interrupted. The app decides for each one before
    // it serves its socket, and before adoption reads the sidecars that the decision removes.
    // A client that connects meanwhile waits in the backlog of the socket.
    let (unfinished, undecided) = testanyware_app::unfinished::run_for_app(&ctx);
    for start in &unfinished {
        eprintln!("testanyware-app: {}", start.detail);
    }
    ctx.adoption.set_unfinished_starts(unfinished);
    // The records that tart did not decide wait for an observation whose list answers.
    testanyware_app::unfinished::watch(&ctx, undecided);
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
    // The socket is served by the select below, so adoption starts after the app can answer.
    runtime.spawn(testanyware_app::adoption::run(Arc::clone(&ctx)));
    let quit_ctx = Arc::clone(&ctx);
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
    // The server runs on the runtime's own threads, so the streams of the cancelled starts end
    // while this thread waits.
    quit_ctx.quit.finish_and_say();
    // A clean stop leaves no socket file. A killed app leaves one, and the next start removes it.
    let _ = std::fs::remove_file(&socket);
    // A cleanup that outlasted the limit may still hold a thread of the blocking pool. The app
    // exits at the limit and does not wait for it.
    runtime.shutdown_background();
    code
}

/// The user's own directories: the socket in Application Support, and the built-in directories of
/// the VM records, which `config.rhai` can replace.
#[cfg(target_os = "macos")]
fn app_paths() -> Option<Paths> {
    use testanyware_app_protocol::default_socket_path;
    use testanyware_vm::paths::VmPaths;

    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        eprintln!("testanyware-app: HOME is not set.");
        return None;
    };
    // The built-in directories come from HOME alone, as the directory check of golden creation
    // assumes. `config.rhai` is the one way to move them.
    let vm_paths = VmPaths::builtin_for_home(&home);
    let config = locate(&|key| std::env::var(key).ok());
    let default_file = plan(&|key| std::env::var(key).ok(), &config, Some(&home));
    Some(Paths {
        socket: default_socket_path(&home),
        state_dir: vm_paths.state_dir().to_path_buf(),
        data_dir: vm_paths.data_dir().to_path_buf(),
        // The app's environment is the one the login item gives it. `TESTANYWARE_CONFIG` is the
        // variable for tests that names another file, or none.
        config,
        default_file,
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
    runtime.spawn(testanyware_app::adoption::run(Arc::clone(&ctx)));
    // Homebrew quits the app for an upgrade with SIGTERM, and a shell quits it with Ctrl-C. Neither
    // runs `applicationWillTerminate:`, so the signal runs the quit and then exits.
    runtime.spawn({
        let ctx = Arc::clone(&ctx);
        let socket = socket.clone();
        async move {
            let mut terminate =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("install the SIGTERM handler");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = terminate.recv() => {}
            }
            let _ = tokio::task::spawn_blocking(move || ctx.quit.finish_and_say()).await;
            let _ = std::fs::remove_file(&socket);
            std::process::exit(0);
        }
    });
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
