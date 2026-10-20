//! The swtpm of a QEMU guest: its argument list, its pid file and its control socket.
//!
//! swtpm ends when its QEMU ends. QEMU holds a connection to the control socket, and the
//! `terminate` option of `--ctrl` ends swtpm when that connection closes, whether QEMU exits or
//! dies. "The terminate parameter enables the automatic termination of swtpm when the control
//! channel connection has been lost" (swtpm 0.10.2, `swtpm(8)`,
//! <https://github.com/stefanberger/swtpm/blob/master/man/man8/swtpm.pod>).
//!
//! `terminate` ends nothing until a client has connected. A start that fails before QEMU
//! connects stops swtpm with `CMD_SHUTDOWN` on the control socket ([`shutdown`]). The command
//! numbers are in
//! <https://github.com/stefanberger/swtpm/blob/master/include/swtpm/tpm_ioctl.h>.

use std::path::{Path, PathBuf};

/// `CMD_SHUTDOWN` of the control channel: shut the TPM down and end the swtpm process.
#[cfg_attr(not(unix), allow(dead_code))]
const CMD_SHUTDOWN: u32 = 3;

/// The pid file of the swtpm of a session.
pub fn pid_file(session_dir: &Path) -> PathBuf {
    session_dir.join("swtpm.pid")
}

/// The control socket of the swtpm of a session. QEMU connects to it.
pub fn control_socket(session_dir: &Path) -> PathBuf {
    session_dir.join("swtpm-sock")
}

/// The arguments of the swtpm of a guest. The daemon writes its own pid to `pid_file`, because
/// the process that `swtpm --daemon` starts exits at once, and the pid of the daemon is the one
/// that the meta records.
pub fn launch_args(tpm_dir: &Path, socket: &Path, pid_file: &Path) -> Vec<String> {
    vec![
        "socket".into(),
        "--tpmstate".into(),
        format!("dir={}", tpm_dir.display()),
        "--ctrl".into(),
        format!("type=unixio,path={},terminate", socket.display()),
        "--tpm2".into(),
        "--log".into(),
        "level=0".into(),
        "--pid".into(),
        format!("file={}", pid_file.display()),
        "--daemon".into(),
    ]
}

/// The pid that a pid file holds, or `None` when the text is not a positive number.
pub fn parse_pid(text: &str) -> Option<i32> {
    text.trim().parse::<i32>().ok().filter(|pid| *pid > 0)
}

/// The pid in `pid_file`, or `None` when the file is missing or holds no pid.
pub fn read_pid(pid_file: &Path) -> Option<i32> {
    parse_pid(&std::fs::read_to_string(pid_file).ok()?)
}

/// The request for `CMD_SHUTDOWN`: the command number as a big-endian 32-bit integer, and no
/// parameters.
#[cfg_attr(not(unix), allow(dead_code))]
fn shutdown_request() -> [u8; 4] {
    CMD_SHUTDOWN.to_be_bytes()
}

/// True when the 4-byte answer to a control command is `TPM_SUCCESS`.
#[cfg_attr(not(unix), allow(dead_code))]
fn is_success(answer: [u8; 4]) -> bool {
    u32::from_be_bytes(answer) == 0
}

/// Stop the swtpm that listens on `socket`, with `CMD_SHUTDOWN`. Returns `true` when swtpm
/// answered success. A socket that is gone, or that refuses the connection, returns `false`:
/// there is then no swtpm to stop through it. Each step waits at most `timeout`.
#[cfg(unix)]
pub fn shutdown(socket: &Path, timeout: std::time::Duration) -> bool {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let Ok(mut stream) = UnixStream::connect(socket) else {
        return false;
    };
    let _ = stream.set_write_timeout(Some(timeout));
    let _ = stream.set_read_timeout(Some(timeout));
    if stream.write_all(&shutdown_request()).is_err() {
        return false;
    }
    let mut answer = [0u8; 4];
    stream.read_exact(&mut answer).is_ok() && is_success(answer)
}

#[cfg(not(unix))]
pub fn shutdown(_socket: &Path, _timeout: std::time::Duration) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_arguments_end_swtpm_with_its_control_connection_and_name_a_pid_file() {
        let args = launch_args(
            Path::new("/c/v-tpm"),
            Path::new("/s/swtpm-sock"),
            Path::new("/s/swtpm.pid"),
        );
        let after = |flag: &str| {
            let at = args
                .iter()
                .position(|a| a == flag)
                .unwrap_or_else(|| panic!("{flag}: {args:?}"));
            args[at + 1].clone()
        };
        assert_eq!(after("--ctrl"), "type=unixio,path=/s/swtpm-sock,terminate");
        assert_eq!(after("--pid"), "file=/s/swtpm.pid");
        assert_eq!(after("--tpmstate"), "dir=/c/v-tpm");
        assert!(args.contains(&"--tpm2".to_string()));
        assert!(args.contains(&"--daemon".to_string()));
    }

    #[test]
    fn a_session_names_its_pid_file_and_socket() {
        assert_eq!(pid_file(Path::new("/s")), Path::new("/s/swtpm.pid"));
        assert_eq!(control_socket(Path::new("/s")), Path::new("/s/swtpm-sock"));
    }

    #[test]
    fn a_pid_file_holds_a_positive_number() {
        assert_eq!(parse_pid("4242\n"), Some(4242));
        for bad in ["", "0", "-3", "abc", "12 13"] {
            assert_eq!(parse_pid(bad), None, "{bad:?}");
        }
        assert_eq!(read_pid(Path::new("/no/such/pid/file")), None);
    }

    #[test]
    fn the_shutdown_command_is_a_big_endian_3_and_success_is_zero() {
        assert_eq!(shutdown_request(), [0, 0, 0, 3]);
        assert!(is_success([0, 0, 0, 0]));
        assert!(!is_success([0, 0, 0, 1]));
    }

    #[cfg(unix)]
    #[test]
    fn shutdown_sends_the_command_and_reads_the_answer() {
        use std::io::{Read, Write};
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("s");
        let listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0u8; 4];
            stream.read_exact(&mut request).unwrap();
            stream.write_all(&[0, 0, 0, 0]).unwrap();
            request
        });
        assert!(shutdown(&socket, std::time::Duration::from_secs(2)));
        assert_eq!(server.join().unwrap(), [0, 0, 0, 3]);
    }

    #[cfg(unix)]
    #[test]
    fn shutdown_reports_false_when_no_swtpm_listens() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!shutdown(
            &dir.path().join("gone"),
            std::time::Duration::from_millis(200)
        ));
    }
}
