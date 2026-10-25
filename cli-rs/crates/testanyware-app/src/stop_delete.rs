//! `POST /v1/vms/{id}/stop` and `POST /v1/goldens/{name}/delete`: `vm stop` and `vm delete` in the
//! app (spec: "The protocol" and "Lifecycle and adoption").
//!
//! Both run the lifecycle code of the vm crate on the app's directories and with its built-in
//! tool search, and answer with the error object or the result that the direct command prints.
//! Both run on a blocking thread, because they call `tart` and wait for processes to end. That
//! thread runs to its end when the client leaves, so a stop that holds its permit never ends half
//! done. A stop that still waits for the permit when the client leaves does not run.

use std::sync::Arc;

use http_body_util::{BodyExt, Limited};
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use serde_json::{json, Value};
use testanyware_app_protocol::{AppError, DeleteRequest, DeleteResult, StopResult};
use testanyware_vm::legacy_stop::{LegacyStopProbes, SettleWait, SystemProbes};
use testanyware_vm::lifecycle::VmLifecycle;
use testanyware_vm::VmError;
use tokio_util::sync::CancellationToken;

use crate::body::AppBody;
use crate::server::{error_response, json_response};
use crate::start::vm_error_object;
use crate::vms::vm_paths;
use crate::AppContext;

/// The most bytes of a delete request that the app reads. The one option is far smaller.
const DELETE_BODY_LIMIT: usize = 4 * 1024;

/// Serve `POST /v1/vms/{id}/stop`. The stop waits for a start or a stop of the same id that is
/// running now, so two commands never take one VM down together.
///
/// A client that leaves while the stop waits gets no stop: hyper drops this future, the guard
/// fires the token, and the wait ends. Otherwise the stop would run after the start that held
/// the id, and take down the VM that the start made. A stop that holds the permit runs to its end,
/// because the token is read only while the stop waits.
///
/// A QEMU guest whose meta records no identity stops through its control sockets and never by a
/// signal (spec: "Lifecycle and adoption"). Any other guest stops as it does on the direct path.
pub async fn stop_vm(ctx: &Arc<AppContext>, id: String) -> Response<AppBody> {
    stop_vm_with(ctx, id, Arc::new(SystemProbes), SettleWait::DEFAULT).await
}

/// [`stop_vm`], with the sockets and the disk of an older QEMU guest observed by `probes`.
pub(crate) async fn stop_vm_with<P>(
    ctx: &Arc<AppContext>,
    id: String,
    probes: Arc<P>,
    wait: SettleWait,
) -> Response<AppBody>
where
    P: LegacyStopProbes + Send + Sync + 'static,
{
    let cancel = CancellationToken::new();
    let _client_gone = cancel.clone().drop_guard();
    let blocking_ctx = Arc::clone(ctx);
    let blocking_id = id.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        let _permit = blocking_ctx.starts.acquire(&blocking_id, &cancel)?;
        let stopped = VmLifecycle::stop_with_probes(
            &blocking_id,
            &vm_paths(&blocking_ctx),
            &blocking_ctx.tools,
            &*probes,
            wait,
        );
        // A guest that is gone frees its slot, so the queue decides again (spec
        // "admission-queue": "The admission decision": "a holder ends").
        blocking_ctx.admission.holder_ended();
        Some(stopped)
    })
    .await;
    match outcome {
        Ok(Some(Ok(()))) => json_response(
            StatusCode::OK,
            serde_json::to_value(StopResult { id, stopped: true }).expect("the result serialises"),
        ),
        Ok(Some(Err(err))) => vm_error_response(&err),
        // The client left, so nobody reads this answer.
        Ok(None) => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            &AppError::new(
                "INTERNAL",
                "The client left before the stop began.",
                None,
                Value::Null,
            ),
        ),
        Err(join) => panicked("stop the VM", &join),
    }
}

/// Serve `POST /v1/goldens/{name}/delete`.
pub async fn delete_golden(
    ctx: &Arc<AppContext>,
    req: &mut Request<Incoming>,
    name: String,
) -> Response<AppBody> {
    let body = match Limited::new(req.body_mut(), DELETE_BODY_LIMIT)
        .collect()
        .await
    {
        Ok(collected) => collected.to_bytes(),
        Err(e) => {
            return usage_error(format!(
                "The app cannot read the request to delete a golden: {e}."
            ))
        }
    };
    let Ok(request) = serde_json::from_slice::<DeleteRequest>(&body) else {
        return usage_error(
            "The request to delete a golden needs the member force, true or false.".into(),
        );
    };
    let blocking_ctx = Arc::clone(ctx);
    let blocking_name = name.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        VmLifecycle::delete_with(
            &blocking_name,
            request.force,
            &vm_paths(&blocking_ctx),
            &blocking_ctx.tools,
        )
    })
    .await;
    match outcome {
        Ok(Ok(())) => json_response(
            StatusCode::OK,
            serde_json::to_value(DeleteResult {
                name,
                deleted: true,
            })
            .expect("the result serialises"),
        ),
        Ok(Err(err)) => vm_error_response(&err),
        Err(join) => panicked("delete the golden", &join),
    }
}

/// A path whose VM id is no plain id names no VM, and the app has not looked at the disk.
pub fn not_a_vm(id: &str) -> Response<AppBody> {
    vm_error_response(&VmError::VmNotFound { id: id.to_string() })
}

/// A decoded name that could leave the golden directory names no golden.
pub fn not_a_golden(name: &str) -> Response<AppBody> {
    vm_error_response(&VmError::GoldenNotFound {
        name: name.to_string(),
    })
}

/// The error of the direct command, as the object it prints, with the status of its kind.
fn vm_error_response(err: &VmError) -> Response<AppBody> {
    let status = match err {
        VmError::VmNotFound { .. } | VmError::GoldenNotFound { .. } => StatusCode::NOT_FOUND,
        VmError::GoldenInUse { .. } => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    error_response(status, &vm_error_object(err))
}

fn usage_error(message: String) -> Response<AppBody> {
    error_response(
        StatusCode::BAD_REQUEST,
        &AppError::new("USAGE_ERROR", message, None, Value::Null),
    )
}

fn panicked(what: &str, join: &tokio::task::JoinError) -> Response<AppBody> {
    error_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        &AppError::new(
            "INTERNAL",
            format!("The TestAnyware app failed to {what}: {join}."),
            None,
            json!({}),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use http_body_util::BodyExt;
    use testanyware_app_protocol::{Directories, ProtocolRange};

    use crate::gate::AppIdentity;
    use testanyware_vm::legacy_stop::{LegacyStopProbes, SettleWait};

    fn context(dir: &std::path::Path) -> Arc<AppContext> {
        Arc::new(AppContext::new(
            AppIdentity {
                build: "test".into(),
                protocol_range: ProtocolRange::new(1, 1),
                directories: Directories {
                    state_dir: dir.join("state"),
                    data_dir: dir.join("data"),
                },
            },
            // The server compares the uid of each peer with this one.
            nix::unistd::geteuid().as_raw(),
            crate::config::AppConfig::disabled(),
        ))
    }

    async fn code_of(response: Response<AppBody>) -> (StatusCode, Value) {
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&body).unwrap())
    }

    /// A stop of an id that a start is running waits for the start to end. It then reads the
    /// records that the start left, and does not race the start's own cleanup.
    #[tokio::test]
    async fn a_stop_waits_for_the_start_of_its_id() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        let permit_ctx = Arc::clone(&ctx);
        let (held, release) = std::sync::mpsc::channel::<()>();
        let (taken, wait_taken) = std::sync::mpsc::channel::<()>();
        let start = std::thread::spawn(move || {
            let permit = permit_ctx
                .starts
                .acquire("testanyware-ab12", &CancellationToken::new());
            taken.send(()).unwrap();
            release.recv().unwrap();
            drop(permit);
        });
        wait_taken.recv().unwrap();

        let stop = tokio::spawn({
            let ctx = Arc::clone(&ctx);
            async move { stop_vm(&ctx, "testanyware-ab12".into()).await }
        });
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert!(
            !stop.is_finished(),
            "the stop ran while the start held the id"
        );

        // Another id is not held by that start.
        let (status, body) = code_of(stop_vm(&ctx, "testanyware-other".into()).await).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "VM_NOT_FOUND");

        held.send(()).unwrap();
        start.join().unwrap();
        let (status, body) = code_of(stop.await.unwrap()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "VM_NOT_FOUND");
    }

    /// A guest that the probes see. Its monitor answers, and a process holds its disk, until
    /// `quit` arrives and `quit_works` is set.
    struct OlderGuest {
        quit_works: bool,
        alive: std::sync::atomic::AtomicBool,
        quits: std::sync::atomic::AtomicU32,
    }

    impl OlderGuest {
        fn new(quit_works: bool, alive: bool) -> Arc<Self> {
            Arc::new(Self {
                quit_works,
                alive: alive.into(),
                quits: 0.into(),
            })
        }
        fn is_alive(&self) -> bool {
            self.alive.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    impl LegacyStopProbes for OlderGuest {
        fn socket_answers(&self, socket: &std::path::Path) -> bool {
            self.is_alive() && socket.ends_with("monitor.sock")
        }
        fn send_monitor_quit(&self, _socket: &std::path::Path) -> bool {
            self.quits.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.quit_works {
                self.alive.store(false, std::sync::atomic::Ordering::SeqCst);
            }
            true
        }
        fn send_swtpm_shutdown(&self, _socket: &std::path::Path) -> bool {
            true
        }
        fn disk_holders(&self, _clone_dir: &std::path::Path) -> Result<Vec<i32>, String> {
            Ok(if self.is_alive() {
                vec![4242]
            } else {
                Vec::new()
            })
        }
        fn pause(&self, _interval: Duration) {}
    }

    struct OlderVm {
        id: String,
        clone: std::path::PathBuf,
        session: std::path::PathBuf,
        sidecars: [std::path::PathBuf; 2],
    }

    impl Drop for OlderVm {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.session);
        }
    }

    /// An older QEMU meta whose pid names `pid`, with a clone, a session directory in the
    /// temporary directory of the app, and both sidecars.
    fn older_vm(ctx: &AppContext, pid: i32) -> OlderVm {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let id = format!("testanyware-ol{nanos:08x}");
        let paths = vm_paths(ctx);
        std::fs::create_dir_all(paths.vms_dir()).unwrap();
        let clone = paths.clone_dir(&id);
        std::fs::create_dir_all(&clone).unwrap();
        std::fs::write(clone.join(format!("{id}.qcow2")), b"disk").unwrap();
        let session = paths.session_dir(&id);
        std::fs::create_dir_all(&session).unwrap();
        let sidecars = [paths.spec_path(&id), paths.meta_path(&id)];
        std::fs::write(&sidecars[0], b"{}").unwrap();
        // The meta of an older start: no identity and no session directory.
        std::fs::write(
            &sidecars[1],
            json!({ "id": id, "tool": "qemu", "pid": pid, "clone_dir": clone }).to_string(),
        )
        .unwrap();
        OlderVm {
            id,
            clone,
            session,
            sidecars,
        }
    }

    fn stranger() -> std::process::Child {
        std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .unwrap()
    }

    const NO_WAIT: SettleWait = SettleWait::looks(2);

    /// The scenario "An older QEMU guest that does not stop".
    #[tokio::test]
    async fn an_older_qemu_guest_that_does_not_stop_fails_and_keeps_everything() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        let mut process = stranger();
        let vm = older_vm(&ctx, process.id() as i32);
        // The monitor does not answer, and a process holds the disk.
        let guest = OlderGuest::new(false, false);
        let held = Arc::new(OlderGuestHeld(Arc::clone(&guest)));

        let (status, body) = code_of(stop_vm_with(&ctx, vm.id.clone(), held, NO_WAIT).await).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["code"], "VM_STOP_FAILED");
        assert_eq!(body["details"]["disk_holders"], json!([4242]));
        assert!(body["remediation"]
            .as_str()
            .unwrap()
            .contains("Quit that QEMU by hand"));
        assert!(vm.clone.exists());
        assert!(vm.session.exists());
        assert!(vm.sidecars.iter().all(|sidecar| sidecar.exists()));
        assert!(
            process.try_wait().unwrap().is_none(),
            "the process at the recorded pid was signalled"
        );
        let _ = process.kill();
        let _ = process.wait();
    }

    /// The monitor of this guest is silent while a process holds the disk. The probe shows that
    /// state without `quit` being able to end it.
    struct OlderGuestHeld(Arc<OlderGuest>);

    impl LegacyStopProbes for OlderGuestHeld {
        fn socket_answers(&self, _socket: &std::path::Path) -> bool {
            false
        }
        fn send_monitor_quit(&self, socket: &std::path::Path) -> bool {
            self.0.send_monitor_quit(socket)
        }
        fn send_swtpm_shutdown(&self, socket: &std::path::Path) -> bool {
            self.0.send_swtpm_shutdown(socket)
        }
        fn disk_holders(&self, _clone_dir: &std::path::Path) -> Result<Vec<i32>, String> {
            Ok(vec![4242])
        }
        fn pause(&self, _interval: Duration) {}
    }

    #[tokio::test]
    async fn an_older_qemu_guest_that_quits_is_removed_without_a_signal() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        let mut process = stranger();
        let vm = older_vm(&ctx, process.id() as i32);
        let guest = OlderGuest::new(true, true);

        let (status, body) =
            code_of(stop_vm_with(&ctx, vm.id.clone(), Arc::clone(&guest), NO_WAIT).await).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["stopped"], true);
        assert_eq!(guest.quits.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(!vm.clone.exists());
        assert!(!vm.session.exists());
        assert!(vm.sidecars.iter().all(|sidecar| !sidecar.exists()));
        assert!(
            process.try_wait().unwrap().is_none(),
            "the process at the recorded pid was signalled"
        );
        let _ = process.kill();
        let _ = process.wait();
    }

    /// A guest whose monitor answers but ignores `quit` is not removed, and `quit` is all it gets.
    #[tokio::test]
    async fn an_older_qemu_guest_that_ignores_quit_fails_and_names_its_monitor() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        let vm = older_vm(&ctx, 0);
        let guest = OlderGuest::new(false, true);

        let (status, body) =
            code_of(stop_vm_with(&ctx, vm.id.clone(), Arc::clone(&guest), NO_WAIT).await).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["code"], "VM_STOP_FAILED");
        assert!(body["details"]["monitor_socket"]
            .as_str()
            .unwrap()
            .ends_with("monitor.sock"));
        assert_eq!(guest.quits.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(vm.clone.exists());
        assert!(vm.sidecars.iter().all(|sidecar| sidecar.exists()));
    }

    #[tokio::test]
    async fn two_stops_of_one_id_never_overlap() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        let first = ctx
            .starts
            .acquire("testanyware-cd34", &CancellationToken::new())
            .expect("the first permit");
        let second = tokio::spawn({
            let ctx = Arc::clone(&ctx);
            async move { stop_vm(&ctx, "testanyware-cd34".into()).await }
        });
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(!second.is_finished());
        drop(first);
        let (status, _) = code_of(second.await.unwrap()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    /// Serve the app in this process, hold the permit of `id`, send a stop of `id`, and release the
    /// permit. With `client_leaves`, the client closes its connection while the stop waits.
    /// Returns whether the stop ran, which it shows by removing the VM's records.
    async fn stop_after_wait(client_leaves: bool) -> bool {
        use testanyware_app_protocol::ClientHello;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let id = "testanyware-ee55";
        let dir = tempfile::tempdir().unwrap();
        let ctx = context(dir.path());
        let vms = dir.path().join("state").join("vms");
        std::fs::create_dir_all(&vms).unwrap();
        let sidecars = [
            vms.join(format!("{id}.json")),
            vms.join(format!("{id}.meta.json")),
        ];
        std::fs::write(&sidecars[0], b"{}").unwrap();
        std::fs::write(
            &sidecars[1],
            // A meta with an identity keeps the stop of the direct path, which fails here for
            // want of a clone directory and removes the sidecars.
            json!({
                "id": id, "tool": "qemu", "pid": 0,
                "identity": { "start_time": 1, "boot_session": "b" },
            })
            .to_string(),
        )
        .unwrap();

        let socket = dir.path().join("s.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let server = tokio::spawn(crate::server::serve(listener, Arc::clone(&ctx)));

        let start = ctx
            .starts
            .acquire(id, &CancellationToken::new())
            .expect("the start's permit");
        let hello = ClientHello::new(
            ProtocolRange::new(1, 1),
            "test",
            ctx.identity.directories.clone(),
        );
        let headers: String = hello
            .to_headers()
            .into_iter()
            .map(|(name, value)| format!("{name}: {value}\r\n"))
            .collect();
        let mut client = tokio::net::UnixStream::connect(&socket).await.unwrap();
        client
            .write_all(
                format!(
                    "POST /v1/vms/{id}/stop HTTP/1.1\r\nHost: t\r\nContent-Length: 0\r\n{headers}\r\n"
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        // The stop is now parked on the permit.
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(sidecars[1].exists());
        let mut kept = None;
        if client_leaves {
            drop(client);
            // Time for hyper to see the close, and for the wait to see the cancel.
            tokio::time::sleep(Duration::from_millis(400)).await;
        } else {
            kept = Some(client);
        }
        drop(start);
        tokio::time::sleep(Duration::from_millis(600)).await;
        if let Some(mut client) = kept {
            let mut answer = vec![0u8; 4096];
            let n = client.read(&mut answer).await.unwrap();
            assert!(String::from_utf8_lossy(&answer[..n]).contains("VM_STOP_FAILED"));
        }
        server.abort();
        !sidecars[1].exists()
    }

    #[tokio::test]
    async fn a_stop_that_waits_runs_when_the_client_stays() {
        assert!(
            stop_after_wait(false).await,
            "the control: the stop never ran"
        );
    }

    /// A client that gives up while its stop waits must not get the stop later, after the start
    /// that held the id has ended: that stop would take down the VM that the start made.
    #[tokio::test]
    async fn a_stop_that_waits_does_not_run_when_the_client_leaves() {
        assert!(
            !stop_after_wait(true).await,
            "the stop ran after its client left"
        );
    }
}
