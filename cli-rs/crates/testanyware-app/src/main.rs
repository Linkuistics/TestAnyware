//! The TestAnyware app executable.
//!
//! This build runs in test mode only. The status item, the login item and the LaunchServices
//! start come with the bundle leaf. Test mode has none of them and takes every path from its
//! arguments, so a test never touches the user's own directories.

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
#[derive(Parser)]
#[command(
    name = "testanyware-app",
    version,
    about = "The TestAnyware app (test mode only in this build)"
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
}

#[cfg(unix)]
#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let (true, Some(socket), Some(state_dir), Some(data_dir)) =
        (args.test_mode, args.socket, args.state_dir, args.data_dir)
    else {
        eprintln!("testanyware-app: this build runs in test mode only. Pass --test-mode --socket --state-dir --data-dir.");
        return ExitCode::from(2);
    };

    let own_uid = nix::unistd::geteuid().as_raw();
    let listener = match bind_socket(&socket, own_uid).await {
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
                state_dir,
                data_dir,
            },
        },
        own_uid,
        args.config,
    ));
    eprintln!("testanyware-app: serving on {}", socket.display());

    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("install the SIGTERM handler");
    let code = tokio::select! {
        result = serve(listener, ctx) => match result {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => { eprintln!("testanyware-app: {e}"); ExitCode::FAILURE }
        },
        _ = tokio::signal::ctrl_c() => ExitCode::SUCCESS,
        _ = terminate.recv() => ExitCode::SUCCESS,
    };
    // A clean stop leaves no socket file. A killed app leaves one, and the next start removes it.
    let _ = std::fs::remove_file(&socket);
    code
}
