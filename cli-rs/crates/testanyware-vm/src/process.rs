//! Process-tree control: liveness checks, graceful-then-forced
//! termination, and `pgrep`-based discovery.
//!
//! Ports the kill helpers from `QEMURunner.swift`. Unix-only; Windows
//! host support (`CREATE_NEW_PROCESS_GROUP` / `GenerateConsoleCtrlEvent`)
//! is backlog task 14.

#[cfg(unix)]
use std::time::Duration;

/// True if `pid` names a live process. Ports the Swift `kill(pid, 0) == 0`
/// idiom: signal 0 performs error checking without delivering a signal.
#[cfg(unix)]
pub fn process_alive(pid: i32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    if pid <= 0 {
        return false;
    }
    // Ok => exists; Err(EPERM) => exists but not ours; Err(ESRCH) => gone.
    !matches!(kill(Pid::from_raw(pid), None), Err(nix::errno::Errno::ESRCH))
}

#[cfg(not(unix))]
pub fn process_alive(_pid: i32) -> bool {
    false
}

/// Terminate `pid`: SIGTERM, poll up to `attempts` times spaced by
/// `poll_interval`, then SIGKILL if still alive. Idempotent and
/// best-effort — a dead or stale pid is a silent no-op. Ports the qemu
/// branch of `QEMURunner.teardown`.
///
/// After signalling, attempts a best-effort `waitpid(WNOHANG)` to reap
/// the child so it does not remain a zombie — `kill(pid,0)` returns
/// success for zombies, which would defeat the `process_alive` check.
#[cfg(unix)]
pub fn terminate(pid: i32, poll_interval: Duration, attempts: u32) {
    use nix::sys::signal::{kill, Signal};
    use nix::sys::wait::{waitpid, WaitPidFlag};
    use nix::unistd::Pid;
    if pid <= 0 || !process_alive(pid) {
        return;
    }
    let target = Pid::from_raw(pid);
    let _ = kill(target, Signal::SIGTERM);
    for _ in 0..attempts {
        // Try to reap so the process does not linger as a zombie.
        let _ = waitpid(target, Some(WaitPidFlag::WNOHANG));
        if !process_alive(pid) {
            return;
        }
        std::thread::sleep(poll_interval);
    }
    if process_alive(pid) {
        let _ = kill(target, Signal::SIGKILL);
        // Final reap attempt after SIGKILL.
        let _ = waitpid(target, Some(WaitPidFlag::WNOHANG));
    }
}

#[cfg(not(unix))]
pub fn terminate(_pid: i32, _poll_interval: std::time::Duration, _attempts: u32) {}

/// First PID whose command line matches `pattern`, via `pgrep -f`.
/// Returns `None` on no match or if `pgrep` is unavailable. Ports
/// `QEMURunner.pgrepFirst`.
#[cfg(unix)]
pub fn pgrep_first(pattern: &str) -> Option<i32> {
    let output = std::process::Command::new("pgrep")
        .args(["-f", pattern])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .and_then(|line| line.trim().parse::<i32>().ok())
}

#[cfg(not(unix))]
pub fn pgrep_first(_pattern: &str) -> Option<i32> {
    None
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::Duration;

    // The child is intentionally not waited on — terminate() is responsible
    // for reaping it, which is exactly what the tests exercise.
    #[allow(clippy::zombie_processes)]
    fn spawn_sleep(secs: u32) -> i32 {
        let child = std::process::Command::new("sleep")
            .arg(secs.to_string())
            .spawn()
            .expect("spawn sleep");
        child.id() as i32
    }

    #[test]
    fn process_alive_tracks_a_real_child() {
        let pid = spawn_sleep(30);
        assert!(process_alive(pid), "freshly spawned child should be alive");
        terminate(pid, Duration::from_millis(100), 10);
        assert!(!process_alive(pid), "child should be dead after terminate");
    }

    #[test]
    fn process_alive_is_false_for_unused_pid() {
        // PID 2^31-1 is effectively never allocated.
        assert!(!process_alive(i32::MAX));
    }

    #[test]
    fn terminate_is_a_noop_for_dead_pid() {
        // Must not panic / must not signal an unrelated process.
        terminate(i32::MAX, Duration::from_millis(10), 2);
    }

    #[test]
    fn pgrep_first_finds_a_running_process() {
        // A deliberately odd duration: `pgrep -f` matches process-wide, so
        // the pattern must not collide with other tests' `sleep` children
        // (which use `sleep 30`) or any ambient `sleep`. The child is
        // terminated immediately, so the long duration never elapses.
        let pid = spawn_sleep(28_931);
        let found = pgrep_first("sleep 28931");
        terminate(pid, Duration::from_millis(100), 10);
        assert_eq!(found, Some(pid), "pgrep should locate the sleep child");
    }

    #[test]
    fn pgrep_first_returns_none_on_no_match() {
        assert_eq!(pgrep_first("a-pattern-that-matches-nothing-xyzzy-42"), None);
    }
}
