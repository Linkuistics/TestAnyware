//! Prepare and bind the socket (spec: "The socket and access").

use std::fmt;
use std::io;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, PermissionsExt};
use std::path::{Path, PathBuf};

use testanyware_app_protocol::socket::check_socket_dir;
use testanyware_app_protocol::{validate_socket_path, SocketDirError, SocketPathError};
use tokio::net::{UnixListener, UnixStream};

/// Why the app does not serve on its socket.
#[derive(Debug)]
pub enum BindError {
    PathTooLong(SocketPathError),
    UnsafeDirectory {
        dir: PathBuf,
        reason: SocketDirError,
    },
    /// A file that is not a socket sits at the socket path. The app never removes it.
    NotASocket(PathBuf),
    /// Another process answers on the socket.
    AlreadyRunning(PathBuf),
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for BindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathTooLong(e) => e.fmt(f),
            Self::UnsafeDirectory { dir, reason } => write!(f, "{}: {reason}", dir.display()),
            Self::NotASocket(p) => write!(
                f,
                "{} exists and is not a socket. The app does not remove it.",
                p.display()
            ),
            Self::AlreadyRunning(p) => write!(
                f,
                "another TestAnyware app already answers on {}",
                p.display()
            ),
            Self::Io {
                action,
                path,
                source,
            } => write!(f, "cannot {action} {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for BindError {}

fn io_err<'a>(action: &'static str, path: &'a Path) -> impl FnOnce(io::Error) -> BindError + 'a {
    move |source| BindError::Io {
        action,
        path: path.to_path_buf(),
        source,
    }
}

/// Create the socket directory with mode 0700 if it is missing, refuse an unsafe one,
/// remove a stale socket file, and bind. The caller passes its own uid, so a test can
/// state another one.
pub async fn bind_socket(path: &Path, own_uid: u32) -> Result<UnixListener, BindError> {
    validate_socket_path(path).map_err(BindError::PathTooLong)?;
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(io_err("create", dir))?;
    match check_socket_dir(dir, own_uid).map_err(io_err("inspect", dir))? {
        Ok(()) => {}
        Err(reason) => {
            return Err(BindError::UnsafeDirectory {
                dir: dir.to_path_buf(),
                reason,
            })
        }
    }

    remove_stale_socket(path).await?;

    let listener = match UnixListener::bind(path) {
        Ok(l) => l,
        // Two processes that start together both find no file. The second bind loses.
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            return Err(BindError::AlreadyRunning(path.to_path_buf()))
        }
        Err(e) => return Err(io_err("bind", path)(e)),
    };
    // The directory already keeps other users out. The mode of the file is a second guard.
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(io_err("restrict", path))?;
    Ok(listener)
}

/// A socket file that no process answers is stale. Remove it. A file that is not a socket
/// stays, and so does a socket that answers.
async fn remove_stale_socket(path: &Path) -> Result<(), BindError> {
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(io_err("inspect", path)(e)),
    };
    if !meta.file_type().is_socket() {
        return Err(BindError::NotASocket(path.to_path_buf()));
    }
    match UnixStream::connect(path).await {
        Ok(_) => Err(BindError::AlreadyRunning(path.to_path_buf())),
        // Nothing listens on the file: its process is gone.
        Err(e) if e.kind() == io::ErrorKind::ConnectionRefused => {
            std::fs::remove_file(path).map_err(io_err("remove", path))
        }
        Err(e) => Err(io_err("probe", path)(e)),
    }
}
