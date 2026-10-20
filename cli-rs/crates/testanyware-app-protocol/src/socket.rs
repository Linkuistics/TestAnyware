//! The rules of the socket and its directory (spec: "The socket and access").
//!
//! The app and the Host CLI share these rules. The decisions are pure functions, so a
//! test can drive them without a socket.

use std::fmt;
use std::path::{Path, PathBuf};

/// The most bytes a socket path may have. The `sun_path` field of a macOS socket address
/// holds 104 bytes, and the last one is the terminating NUL.
pub const SOCKET_PATH_MAX_BYTES: usize = 103;

/// The file name of the socket.
pub const SOCKET_FILE_NAME: &str = "testanyware.sock";

/// `~/Library/Application Support/TestAnyware/testanyware.sock`, for a given home directory.
pub fn default_socket_path(home: &Path) -> PathBuf {
    home.join("Library")
        .join("Application Support")
        .join("TestAnyware")
        .join(SOCKET_FILE_NAME)
}

/// A socket path that is too long.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketPathError {
    pub path: PathBuf,
    pub bytes: usize,
}

impl fmt::Display for SocketPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "socket path {} has {} bytes. A socket path has {} bytes at most.",
            self.path.display(),
            self.bytes,
            SOCKET_PATH_MAX_BYTES
        )
    }
}

impl std::error::Error for SocketPathError {}

/// Refuse a socket path of more than [`SOCKET_PATH_MAX_BYTES`]. Nothing truncates a path.
pub fn validate_socket_path(path: &Path) -> Result<(), SocketPathError> {
    let bytes = path_bytes(path);
    if bytes > SOCKET_PATH_MAX_BYTES {
        return Err(SocketPathError {
            path: path.to_path_buf(),
            bytes,
        });
    }
    Ok(())
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> usize {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().len()
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> usize {
    path.to_string_lossy().len()
}

/// Why a socket directory is unsafe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocketDirError {
    NotADirectory,
    /// Another user owns the directory.
    NotOwned {
        owner_uid: u32,
        own_uid: u32,
    },
    /// The mode admits the group or others.
    ModeTooOpen {
        mode: u32,
    },
}

impl fmt::Display for SocketDirError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotADirectory => write!(f, "the socket directory is not a directory"),
            Self::NotOwned { owner_uid, own_uid } => {
                write!(
                    f,
                    "the socket directory belongs to uid {owner_uid}, not to uid {own_uid}"
                )
            }
            Self::ModeTooOpen { mode } => {
                write!(f, "the socket directory has mode {:04o}, which admits other users. Its mode must be 0700.", mode & 0o7777)
            }
        }
    }
}

impl std::error::Error for SocketDirError {}

/// Judge the facts of a socket directory. The directory is safe when the user owns it and
/// its mode admits nobody else.
pub fn judge_socket_dir(
    is_dir: bool,
    owner_uid: u32,
    mode: u32,
    own_uid: u32,
) -> Result<(), SocketDirError> {
    if !is_dir {
        return Err(SocketDirError::NotADirectory);
    }
    if owner_uid != own_uid {
        return Err(SocketDirError::NotOwned { owner_uid, own_uid });
    }
    if mode & 0o077 != 0 {
        return Err(SocketDirError::ModeTooOpen { mode });
    }
    Ok(())
}

/// Read the owner and the mode of `dir` and judge them. A symlink is not a directory
/// here, so the check does not follow it.
#[cfg(unix)]
pub fn check_socket_dir(dir: &Path, own_uid: u32) -> std::io::Result<Result<(), SocketDirError>> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(dir)?;
    Ok(judge_socket_dir(
        meta.is_dir(),
        meta.uid(),
        meta.mode(),
        own_uid,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_path_sits_under_application_support() {
        let p = default_socket_path(Path::new("/Users/me"));
        assert_eq!(
            p,
            Path::new("/Users/me/Library/Application Support/TestAnyware/testanyware.sock")
        );
    }

    #[test]
    fn a_path_of_103_bytes_is_accepted() {
        let path = PathBuf::from(format!("/{}", "a".repeat(102)));
        assert_eq!(path_bytes(&path), 103);
        assert_eq!(validate_socket_path(&path), Ok(()));
    }

    #[test]
    fn a_path_of_104_bytes_is_refused_and_not_truncated() {
        let path = PathBuf::from(format!("/{}", "a".repeat(103)));
        let err = validate_socket_path(&path).unwrap_err();
        assert_eq!(err.bytes, 104);
        assert_eq!(err.path, path);
    }

    #[test]
    fn length_counts_bytes_not_characters() {
        // 52 two-byte characters plus the slash give 105 bytes and 53 characters.
        let path = PathBuf::from(format!("/{}", "é".repeat(52)));
        assert!(validate_socket_path(&path).is_err());
    }

    #[test]
    fn own_directory_with_mode_0700_is_safe() {
        assert_eq!(judge_socket_dir(true, 501, 0o040700, 501), Ok(()));
        assert_eq!(judge_socket_dir(true, 501, 0o040500, 501), Ok(()));
    }

    #[test]
    fn another_owner_is_refused() {
        assert_eq!(
            judge_socket_dir(true, 0, 0o040700, 501),
            Err(SocketDirError::NotOwned {
                owner_uid: 0,
                own_uid: 501
            })
        );
    }

    #[test]
    fn any_group_or_other_bit_is_refused() {
        for mode in [0o040750, 0o040705, 0o040770, 0o040701, 0o040710, 0o040755] {
            assert_eq!(
                judge_socket_dir(true, 501, mode, 501),
                Err(SocketDirError::ModeTooOpen { mode }),
                "mode {mode:o}"
            );
        }
    }

    #[test]
    fn a_non_directory_is_refused() {
        assert_eq!(
            judge_socket_dir(false, 501, 0o100600, 501),
            Err(SocketDirError::NotADirectory)
        );
    }

    #[cfg(unix)]
    #[test]
    fn check_reads_the_real_mode_and_does_not_follow_a_symlink() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let tmp = std::env::temp_dir().join(format!("tap-sock-dir-{}", std::process::id()));
        let real = tmp.join("real");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o700)).unwrap();
        let uid = std::os::unix::fs::MetadataExt::uid(&std::fs::metadata(&real).unwrap());
        assert_eq!(check_socket_dir(&real, uid).unwrap(), Ok(()));

        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(
            check_socket_dir(&real, uid).unwrap(),
            Err(SocketDirError::ModeTooOpen { .. })
        ));

        let link = tmp.join("link");
        symlink(&real, &link).unwrap();
        assert_eq!(
            check_socket_dir(&link, uid).unwrap(),
            Err(SocketDirError::NotADirectory)
        );
        std::fs::remove_dir_all(&tmp).unwrap();
    }
}
