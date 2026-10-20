//! Spawn a long-running process in its own session.
//!
//! Port of `DetachedProcess.swift`. The child is immune to SIGHUP on the
//! caller's terminal (`setsid`) and outlives the caller. stdout+stderr go to
//! `log_path` (append); stdin is `/dev/null`.
//!
//! Two rules hold for the TestAnyware app, which runs for days and accepts client
//! sockets on other threads while it spawns:
//!
//! - **The child holds no descriptor of the caller.** On macOS a new socket gets
//!   close-on-exec in a second call, so a `fork` on another thread can hand a
//!   child a client socket that was just accepted. `posix_spawn` with
//!   `POSIX_SPAWN_CLOEXEC_DEFAULT` closes every descriptor that a file action does
//!   not name, whichever thread accepted it.
//! - **A child that exits is reaped.** A thread waits for each child, so no zombie
//!   stays while the caller runs.

use std::path::Path;

use crate::error::VmError;

/// Spawn `program` with `args` detached. Returns the child PID. The child is
/// reparented to init/launchd when the caller exits, and a thread of the caller
/// reaps it if it exits first.
pub fn spawn_detached(program: &str, args: &[String], log_path: &Path) -> Result<i32, VmError> {
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|e| VmError::Io(format!("open {}: {e}", log_path.display())))?;
    let pid =
        spawn_session(program, args, Output::File(&log), Output::File(&log)).map_err(|e| {
            VmError::SpawnFailed {
                detail: format!("{program}: {e}"),
            }
        })?;
    reap_in_background(pid);
    Ok(pid)
}

/// Run `program` in its own session and wait for it to exit, with stdout to
/// `/dev/null` and stderr inherited. For a helper that daemonises itself, such as
/// `swtpm --daemon`: the daemon must not hold a descriptor of the caller either.
/// Returns the exit status in the form of `waitpid`.
pub async fn run_clean(
    program: &str,
    args: &[String],
) -> Result<std::process::ExitStatus, VmError> {
    let program = program.to_string();
    let args = args.to_vec();
    tokio::task::spawn_blocking(move || {
        let pid = spawn_session(&program, &args, Output::Null, Output::Inherit).map_err(|e| {
            VmError::SpawnFailed {
                detail: format!("{program}: {e}"),
            }
        })?;
        wait_for(pid).map_err(|e| VmError::SpawnFailed {
            detail: format!("{program}: wait: {e}"),
        })
    })
    .await
    .map_err(|e| VmError::SpawnFailed {
        detail: format!("the spawn task ended: {e}"),
    })?
}

/// Where a child's stdout or stderr goes.
#[derive(Clone, Copy)]
enum Output<'a> {
    /// The caller's own descriptor.
    Inherit,
    Null,
    File(&'a std::fs::File),
}

/// Wait on a thread for `pid`, so that the child never stays a zombie. `waitpid` fails
/// with `ECHILD` when `child_alive` reaped the child first, and then the thread ends.
#[cfg(unix)]
fn reap_in_background(pid: i32) {
    let spawned = std::thread::Builder::new()
        .name(format!("reap-{pid}"))
        .spawn(move || {
            let _ = wait_for(pid);
        });
    if let Err(e) = spawned {
        // Without a thread, the child is reaped by `child_alive` or not at all.
        eprintln!("testanyware: cannot start a thread to reap {pid}: {e}");
    }
}

#[cfg(not(unix))]
fn reap_in_background(_pid: i32) {}

#[cfg(unix)]
fn wait_for(pid: i32) -> std::io::Result<std::process::ExitStatus> {
    use std::os::unix::process::ExitStatusExt;
    loop {
        let mut status = 0;
        // SAFETY: `status` is a valid out-pointer for the duration of the call.
        let r = unsafe { libc::waitpid(pid, &mut status, 0) };
        if r == pid {
            return Ok(std::process::ExitStatus::from_raw(status));
        }
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::Interrupted {
            return Err(err);
        }
    }
}

#[cfg(not(unix))]
fn wait_for(_pid: i32) -> std::io::Result<std::process::ExitStatus> {
    Err(std::io::Error::other("no process control on this host"))
}

/// Start `program` in a new session with stdin from `/dev/null`. The child holds
/// no descriptor except 0, 1 and 2.
#[cfg(target_os = "macos")]
fn spawn_session(
    program: &str,
    args: &[String],
    stdout: Output<'_>,
    stderr: Output<'_>,
) -> std::io::Result<i32> {
    use std::ffi::CString;
    use std::mem::MaybeUninit;
    use std::os::fd::AsRawFd;

    // Apple's <sys/spawn.h>. The libc crate has the second and not the first.
    const POSIX_SPAWN_SETSID: libc::c_int = 0x0400;

    let cstr = |s: &str| {
        CString::new(s).map_err(|_| std::io::Error::other(format!("a NUL byte in {s:?}")))
    };
    let path = cstr(program)?;
    let mut argv_owned = vec![path.clone()];
    for arg in args {
        argv_owned.push(cstr(arg)?);
    }
    let mut argv: Vec<*mut libc::c_char> = argv_owned
        .iter()
        .map(|a| a.as_ptr() as *mut libc::c_char)
        .collect();
    argv.push(std::ptr::null_mut());

    let check = |code: libc::c_int| {
        if code == 0 {
            Ok(())
        } else {
            Err(std::io::Error::from_raw_os_error(code))
        }
    };

    // SAFETY: each `posix_spawn*` call gets a value that the matching `_init` call
    // initialised, and every pointer outlives the call. The structures are destroyed
    // on every path out of the block.
    unsafe {
        let mut actions = MaybeUninit::<libc::posix_spawn_file_actions_t>::uninit();
        check(libc::posix_spawn_file_actions_init(actions.as_mut_ptr()))?;
        let mut attr = MaybeUninit::<libc::posix_spawnattr_t>::uninit();
        if let Err(e) = check(libc::posix_spawnattr_init(attr.as_mut_ptr())) {
            libc::posix_spawn_file_actions_destroy(actions.as_mut_ptr());
            return Err(e);
        }
        let result = (|| {
            let devnull = c"/dev/null";
            check(libc::posix_spawn_file_actions_addopen(
                actions.as_mut_ptr(),
                0,
                devnull.as_ptr(),
                libc::O_RDONLY,
                0,
            ))?;
            for (target, out) in [(1, stdout), (2, stderr)] {
                match out {
                    Output::Inherit => check(libc::posix_spawn_file_actions_adddup2(
                        actions.as_mut_ptr(),
                        target,
                        target,
                    ))?,
                    Output::Null => check(libc::posix_spawn_file_actions_addopen(
                        actions.as_mut_ptr(),
                        target,
                        devnull.as_ptr(),
                        libc::O_WRONLY,
                        0,
                    ))?,
                    Output::File(file) => check(libc::posix_spawn_file_actions_adddup2(
                        actions.as_mut_ptr(),
                        file.as_raw_fd(),
                        target,
                    ))?,
                }
            }
            // The Rust runtime ignores SIGPIPE, and an ignored signal stays ignored across
            // `exec`. `std::process::Command` resets the dispositions and the mask in the child,
            // and so does this: every signal back to its default, and nothing blocked.
            let mut all = MaybeUninit::<libc::sigset_t>::uninit();
            check(libc::sigfillset(all.as_mut_ptr()))?;
            check(libc::posix_spawnattr_setsigdefault(
                attr.as_mut_ptr(),
                all.as_ptr(),
            ))?;
            let mut none = MaybeUninit::<libc::sigset_t>::uninit();
            check(libc::sigemptyset(none.as_mut_ptr()))?;
            check(libc::posix_spawnattr_setsigmask(
                attr.as_mut_ptr(),
                none.as_ptr(),
            ))?;
            check(libc::posix_spawnattr_setflags(
                attr.as_mut_ptr(),
                (POSIX_SPAWN_SETSID
                    | libc::POSIX_SPAWN_CLOEXEC_DEFAULT
                    | libc::POSIX_SPAWN_SETSIGDEF
                    | libc::POSIX_SPAWN_SETSIGMASK) as libc::c_short,
            ))?;
            let mut pid: libc::pid_t = 0;
            check(libc::posix_spawnp(
                &mut pid,
                path.as_ptr(),
                actions.as_ptr(),
                attr.as_ptr(),
                argv.as_ptr(),
                *libc::_NSGetEnviron() as *const *mut libc::c_char,
            ))?;
            Ok(pid)
        })();
        libc::posix_spawn_file_actions_destroy(actions.as_mut_ptr());
        libc::posix_spawnattr_destroy(attr.as_mut_ptr());
        result
    }
}

/// Start `program` in a new session with stdin from `/dev/null`. Other Unix hosts
/// create a socket with close-on-exec in one call, so `std` closes it on `exec`.
#[cfg(all(unix, not(target_os = "macos")))]
fn spawn_session(
    program: &str,
    args: &[String],
    stdout: Output<'_>,
    stderr: Output<'_>,
) -> std::io::Result<i32> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let stdio = |out: Output<'_>| -> std::io::Result<Stdio> {
        Ok(match out {
            Output::Inherit => Stdio::inherit(),
            Output::Null => Stdio::null(),
            Output::File(file) => Stdio::from(file.try_clone()?),
        })
    };
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(stdio(stdout)?)
        .stderr(stdio(stderr)?);
    // SAFETY: `setsid` is async-signal-safe and the only post-fork action. It places the
    // child in a fresh session, so it survives the parent's exit and a terminal SIGHUP.
    unsafe {
        cmd.pre_exec(|| {
            nix::unistd::setsid()
                .map(|_| ())
                .map_err(|e| std::io::Error::from_raw_os_error(e as i32))
        });
    }
    let child = cmd.spawn()?;
    let pid = child.id() as i32;
    // The reaper waits by pid, so the `Child` handle is not needed.
    drop(child);
    Ok(pid)
}

#[cfg(not(unix))]
fn spawn_session(
    program: &str,
    args: &[String],
    stdout: Output<'_>,
    stderr: Output<'_>,
) -> std::io::Result<i32> {
    use std::process::{Command, Stdio};

    let stdio = |out: Output<'_>| -> std::io::Result<Stdio> {
        Ok(match out {
            Output::Inherit => Stdio::inherit(),
            Output::Null => Stdio::null(),
            Output::File(file) => Stdio::from(file.try_clone()?),
        })
    };
    let child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(stdio(stdout)?)
        .stderr(stdio(stderr)?)
        .spawn()?;
    Ok(child.id() as i32)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[tokio::test]
    async fn spawn_detached_runs_child_in_its_own_session() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("child.log");
        // `sh -c 'echo hi; sleep 30'` — long enough to inspect, writes to log.
        let pid = spawn_detached("/bin/sh", &["-c".into(), "echo hi; sleep 30".into()], &log)
            .expect("spawn");

        assert!(
            crate::process::process_alive(pid),
            "detached child should be running"
        );

        // A `setsid` child is a session leader: its SID equals its PID.
        let sid = nix::unistd::getsid(Some(nix::unistd::Pid::from_raw(pid))).expect("getsid");
        assert_eq!(
            sid.as_raw(),
            pid,
            "detached child must be its own session leader"
        );

        // stdout was redirected to the log file.
        std::thread::sleep(Duration::from_millis(200));
        let logged = std::fs::read_to_string(&log).unwrap_or_default();
        assert!(
            logged.contains("hi"),
            "child stdout should land in the log: {logged:?}"
        );

        crate::process::terminate(pid, Duration::from_millis(100), 10);
    }

    #[tokio::test]
    async fn spawn_detached_reports_a_missing_executable() {
        let dir = tempfile::tempdir().unwrap();
        let err = spawn_detached("/no/such/binary-xyzzy", &[], &dir.path().join("l.log"));
        assert!(err.is_err(), "missing executable must be an error");
    }

    /// Whether `pid` is a zombie, by the state column of `ps`. A pid that no longer exists
    /// has no row, and it is not a zombie.
    fn is_zombie(pid: i32) -> bool {
        let out = std::process::Command::new("ps")
            .args(["-o", "stat=", "-p", &pid.to_string()])
            .output()
            .expect("run ps");
        String::from_utf8_lossy(&out.stdout).trim().starts_with('Z')
    }

    #[tokio::test]
    async fn a_detached_child_that_exits_is_reaped_while_the_caller_runs() {
        let dir = tempfile::tempdir().unwrap();
        let pid = spawn_detached(
            "/bin/sh",
            &["-c".into(), "exit 3".into()],
            &dir.path().join("exit.log"),
        )
        .expect("spawn");

        // The caller never waits for the child itself. The reaper does.
        let deadline = Instant::now() + Duration::from_secs(5);
        while crate::process::process_alive(pid) {
            assert!(
                Instant::now() < deadline,
                "pid {pid} is still there: zombie={}",
                is_zombie(pid)
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!is_zombie(pid));
    }

    /// The descriptor numbers that a child lists in `/dev/fd`.
    fn fds_seen_by(spawn: impl FnOnce(&Path) -> i32, dir: &Path) -> Vec<i32> {
        let log = dir.join("fds.log");
        let pid = spawn(&log);
        let deadline = Instant::now() + Duration::from_secs(5);
        while crate::process::process_alive(pid) {
            assert!(Instant::now() < deadline, "the lister does not end");
            std::thread::sleep(Duration::from_millis(20));
        }
        std::fs::read_to_string(&log)
            .expect("the child's log")
            .split_whitespace()
            .filter_map(|n| n.parse().ok())
            .collect()
    }

    /// A descriptor with a high number and without close-on-exec: the state of a client
    /// socket in the gap between `accept` and the second call that sets the flag.
    #[cfg(target_os = "macos")]
    fn leaked_descriptor() -> std::fs::File {
        use std::os::fd::{FromRawFd, OwnedFd};
        let file = std::fs::File::open("/dev/null").unwrap();
        // SAFETY: F_DUPFD returns a new descriptor that this function owns.
        let raw = unsafe { libc::fcntl(std::os::fd::AsRawFd::as_raw_fd(&file), libc::F_DUPFD, 40) };
        assert!(
            raw >= 40,
            "fcntl F_DUPFD: {}",
            std::io::Error::last_os_error()
        );
        std::fs::File::from(unsafe { OwnedFd::from_raw_fd(raw) })
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn a_detached_child_holds_no_descriptor_of_the_caller() {
        use std::os::fd::AsRawFd;
        let dir = tempfile::tempdir().unwrap();
        let leaked = leaked_descriptor();
        let number = leaked.as_raw_fd();
        let lister = ["-c".to_string(), "ls /dev/fd".to_string()];

        // The control: a plain spawn, as the code did before, hands the descriptor over. A
        // check that cannot fail proves nothing.
        let inherited = fds_seen_by(
            |log| {
                use std::os::unix::process::CommandExt;
                let out = std::fs::File::create(log).unwrap();
                let mut cmd = std::process::Command::new("/bin/sh");
                cmd.args(&lister).stdout(out);
                unsafe {
                    cmd.pre_exec(|| {
                        nix::unistd::setsid()
                            .map(|_| ())
                            .map_err(std::io::Error::from)
                    })
                };
                let mut child = cmd.spawn().unwrap();
                let pid = child.id() as i32;
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                pid
            },
            dir.path(),
        );
        assert!(
            inherited.contains(&number),
            "the control spawn should leak descriptor {number}, it saw {inherited:?}"
        );

        let held = fds_seen_by(
            |log| {
                // `spawn_detached` appends to the log, which `ls` fills before it ends.
                let _ = std::fs::remove_file(log);
                spawn_detached("/bin/sh", &lister, log).unwrap()
            },
            dir.path(),
        );
        assert!(
            !held.contains(&number),
            "the child holds descriptor {number}: {held:?}"
        );
        // 0, 1 and 2, and the few that `ls` opens to read the directory. The leaked one is 40.
        assert!(held.iter().all(|fd| *fd < 10), "the child holds {held:?}");
    }

    /// A child must not inherit an ignored SIGPIPE from the Rust runtime: an ignored signal
    /// stays ignored across `exec`. Perl reports `IGNORE` for a signal that it inherited ignored.
    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn a_detached_child_gets_the_default_sigpipe() {
        use nix::sys::signal::{signal, SigHandler, Signal};
        // The caller ignores SIGPIPE, as every Rust program does.
        unsafe { signal(Signal::SIGPIPE, SigHandler::SigIgn).unwrap() };
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("sigpipe.log");
        let pid = spawn_detached(
            "/bin/sh",
            &[
                "-c".into(),
                r#"/usr/bin/perl -e 'print defined $SIG{PIPE} ? $SIG{PIPE} : "default"'"#.into(),
            ],
            &log,
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while crate::process::process_alive(pid) {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
        let logged = std::fs::read_to_string(&log).unwrap();
        assert_eq!(logged.trim(), "default", "SIGPIPE in the child");
    }

    #[tokio::test]
    async fn run_clean_leaves_stderr_open_and_stdout_on_dev_null() {
        // A write to a closed descriptor fails with "Bad file descriptor", and the shell then
        // exits non-zero. So a zero exit shows that both descriptors are open.
        let both = run_clean("/bin/sh", &["-c".into(), "echo out; echo err >&2".into()])
            .await
            .unwrap();
        assert!(both.success(), "{both:?}");
    }

    #[tokio::test]
    async fn run_clean_waits_for_the_exit_status() {
        let ok = run_clean("/bin/sh", &["-c".into(), "exit 0".into()])
            .await
            .unwrap();
        assert!(ok.success());
        let failed = run_clean("/bin/sh", &["-c".into(), "exit 7".into()])
            .await
            .unwrap();
        assert_eq!(failed.code(), Some(7));
    }
}
