//! Spawn a long-running process in its own session.
//!
//! Port of `DetachedProcess.swift`. The child is immune to SIGHUP on the
//! caller's terminal (`setsid`) and outlives the CLI. stdout+stderr go to
//! `log_path` (append); stdin is `/dev/null`.

use std::path::Path;
use std::process::Stdio;

use crate::error::VmError;

/// Spawn `program` with `args` detached. Returns the child PID. The
/// `tokio::process::Child` is dropped without waiting — `kill_on_drop`
/// defaults to `false`, so the process keeps running and is reparented to
/// init/launchd when the short-lived CLI exits.
pub fn spawn_detached(program: &str, args: &[String], log_path: &Path) -> Result<i32, VmError> {
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|e| VmError::Io(format!("open {}: {e}", log_path.display())))?;
    let log_err = log
        .try_clone()
        .map_err(|e| VmError::Io(format!("dup log fd: {e}")))?;

    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args)
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .kill_on_drop(false);

    #[cfg(unix)]
    {
        let devnull = std::fs::File::open("/dev/null")
            .map_err(|e| VmError::Io(format!("open /dev/null: {e}")))?;
        cmd.stdin(Stdio::from(devnull));
        // SAFETY: `setsid` is async-signal-safe and the only post-fork
        // action; it places the child in a fresh session so it survives
        // the parent's exit and a terminal SIGHUP.
        unsafe {
            cmd.pre_exec(|| {
                nix::unistd::setsid()
                    .map(|_| ())
                    .map_err(|e| std::io::Error::from_raw_os_error(e as i32))
            });
        }
    }
    #[cfg(not(unix))]
    {
        cmd.stdin(Stdio::null());
    }

    let child = cmd
        .spawn()
        .map_err(|e| VmError::SpawnFailed { detail: format!("{program}: {e}") })?;
    let pid = child
        .id()
        .ok_or_else(|| VmError::SpawnFailed { detail: format!("{program}: exited before id") })?;
    // Detach: drop without waiting. kill_on_drop is false.
    drop(child);
    Ok(pid as i32)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn spawn_detached_runs_child_in_its_own_session() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("child.log");
        // `sh -c 'echo hi; sleep 30'` — long enough to inspect, writes to log.
        let pid = spawn_detached(
            "/bin/sh",
            &["-c".into(), "echo hi; sleep 30".into()],
            &log,
        )
        .expect("spawn");

        assert!(crate::process::process_alive(pid), "detached child should be running");

        // A `setsid` child is a session leader: its SID equals its PID.
        let sid = nix::unistd::getsid(Some(nix::unistd::Pid::from_raw(pid)))
            .expect("getsid");
        assert_eq!(sid.as_raw(), pid, "detached child must be its own session leader");

        // stdout was redirected to the log file.
        std::thread::sleep(Duration::from_millis(200));
        let logged = std::fs::read_to_string(&log).unwrap_or_default();
        assert!(logged.contains("hi"), "child stdout should land in the log: {logged:?}");

        crate::process::terminate(pid, Duration::from_millis(100), 10);
    }

    #[tokio::test]
    async fn spawn_detached_reports_a_missing_executable() {
        let dir = tempfile::tempdir().unwrap();
        let err = spawn_detached("/no/such/binary-xyzzy", &[], &dir.path().join("l.log"));
        assert!(err.is_err(), "missing executable must be an error");
    }
}
