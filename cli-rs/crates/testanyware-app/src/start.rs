//! `POST /v1/vms`: `vm start` in the app (spec: "The protocol" and "Lifecycle and adoption").
//!
//! The app runs the lifecycle code of the vm crate for a client that may go away. The
//! response is the stream of the events of the start. The response body owns the receiving
//! end of the channel, so when hyper drops the body, the start learns that nobody listens.
//!
//! The start runs on a thread of its own, on a runtime of its own. It calls `tart clone` and
//! other tools that block, and a worker of the shared runtime must not wait for them. The
//! thread is also the one place that starts the detached children.

use std::panic::AssertUnwindSafe;

use bytes::Bytes;
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::{Response, StatusCode};
use serde_json::json;
use testanyware_app_protocol::{
    is_plain_vm_id, AppError, HostPort, StartEvent, StartRequest, StartResult, TpmReport,
    START_CONTENT_TYPE,
};
use testanyware_vm::display_request::DisplayRequest;
use testanyware_vm::lifecycle::{Platform, VmLifecycle, VmStartOptions, VmStartResult};
use testanyware_vm::tpm::{OmitReason, TpmDecision};
use testanyware_vm::{Progress, StartContext, VmError};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::body::AppBody;
use crate::vms::vm_paths;
use crate::AppContext;
use std::sync::Arc;

/// Why a request cannot start a VM. The app answers with the error object and a status, and
/// no stream.
pub struct Refusal {
    pub status: StatusCode,
    pub error: AppError,
}

/// Read the options of a request. Every check that needs no tool comes first, so a bad
/// request fails before any process runs.
pub fn parse_request(body: &[u8]) -> Result<(StartRequest, VmStartOptions), Refusal> {
    let usage = |message: String| Refusal {
        status: StatusCode::BAD_REQUEST,
        error: AppError::new("USAGE_ERROR", message, None, json!({ "path": "/v1/vms" })),
    };
    let request: StartRequest = serde_json::from_slice(body)
        .map_err(|e| usage(format!("The request to start a VM does not read: {e}.")))?;
    let platform = Platform::parse(&request.platform).map_err(|e| Refusal {
        status: StatusCode::BAD_REQUEST,
        error: vm_error_object(&e),
    })?;
    // The id names files in the state directory, so it must be a plain name.
    if !is_plain_vm_id(&request.id) {
        return Err(usage(format!(
            "The VM id '{}' has characters other than letters, digits, '.', '-' and '_', or starts with a dot.",
            request.id
        )));
    }
    if request.base.is_empty() || request.base.contains('/') {
        return Err(usage(format!(
            "The base '{}' is not the name of a golden image.",
            request.base
        )));
    }
    let mut options = VmStartOptions::new(
        platform,
        Some(request.base.clone()),
        Some(request.id.clone()),
        DisplayRequest {
            backend_display: request.display.clone(),
            logical: request.logical,
        },
        false,
    );
    options.keep_on_failure = request.keep_on_failure;
    options.wait = request.wait_secs.map(std::time::Duration::from_secs);
    Ok((request, options))
}

/// A start error in the Host CLI's vocabulary: the code, the message, the remedy and the
/// details that the direct command prints.
pub fn vm_error_object(err: &VmError) -> AppError {
    AppError::new(
        err.code(),
        err.to_string(),
        err.remediation(),
        err.details(),
    )
}

/// The data of a successful start, as `vm start` prints it.
pub fn result_of(result: &VmStartResult) -> StartResult {
    StartResult {
        id: result.id.clone(),
        platform: result.platform.as_str().to_string(),
        vnc: HostPort {
            host: result.spec.vnc.host.clone(),
            port: result.spec.vnc.port,
        },
        agent: result.spec.agent.as_ref().map(|a| HostPort {
            host: a.host.clone(),
            port: a.port,
        }),
        spec_path: result.spec_path.display().to_string(),
        meta_path: result.meta_path.display().to_string(),
        tpm: result.tpm.map(tpm_report),
    }
}

fn tpm_report(decision: TpmDecision) -> TpmReport {
    match decision {
        TpmDecision::Attach { ppi_off } => TpmReport {
            attached: true,
            ppi_off: Some(ppi_off),
            reason: None,
        },
        TpmDecision::Omit(reason) => TpmReport {
            attached: false,
            ppi_off: None,
            reason: Some(
                match reason {
                    OmitReason::GuestDoesNotNeedOne => "guest_does_not_need_one",
                    OmitReason::HostCannotSupplyOne => "host_cannot_supply_one",
                }
                .to_string(),
            ),
        },
    }
}

/// The final event of a start, or `None` for a start that the client cancelled: nobody reads
/// it.
fn final_event(result: Result<VmStartResult, VmError>) -> Option<StartEvent> {
    match result {
        Ok(result) => Some(StartEvent::Result {
            data: result_of(&result),
        }),
        Err(VmError::StartCancelled { .. }) => None,
        Err(err) => Some(StartEvent::Error {
            error: vm_error_object(&err),
        }),
    }
}

/// Start the VM of `options` and answer with the stream of its events.
pub fn start_response(ctx: &Arc<AppContext>, options: VmStartOptions) -> Response<AppBody> {
    let (sender, receiver) = mpsc::unbounded_channel::<Bytes>();
    // Dropping the body cancels the start.
    let cancel = CancellationToken::new();

    let start_ctx = StartContext::for_app(
        vm_paths(ctx),
        ctx.tools.clone(),
        {
            let sender = sender.clone();
            Progress::to(move |line| {
                let event = StartEvent::Progress {
                    line: line.to_string(),
                };
                // A closed channel means the client left, and the cancel handles that.
                let _ = sender.send(Bytes::from(event.to_line()));
            })
        },
        cancel.clone(),
    )
    .with_agent_port(ctx.agent_port);

    let id = options.id.clone();
    let gate_ctx = Arc::clone(ctx);
    let spawned = std::thread::Builder::new()
        .name(format!("start-{id}"))
        .spawn({
            let sender = sender.clone();
            move || {
                // An earlier start of this id may still clean up. Its cleanup must end before
                // this start reclaims the id, and a client that left while it waited starts nothing.
                let Some(_permit) = gate_ctx.starts.acquire(&options.id, &start_ctx.cancel) else {
                    return;
                };
                let outcome = std::panic::catch_unwind(AssertUnwindSafe(|| {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| VmError::Io(format!("build the runtime of the start: {e}")))?;
                    runtime.block_on(VmLifecycle::start_in(&options, &start_ctx))
                }));
                let event = match outcome {
                    Ok(result) => {
                        let started = result.as_ref().ok().map(|r| r.id.clone());
                        let event = final_event(result);
                        // A client that left after the start ended was told nothing. The start
                        // checked the cancel before it returned, and this covers the rest: the
                        // runtime that is dropped and the event that is sent. A VM that nobody
                        // learns of is stopped again.
                        let delivered = match &event {
                            Some(event) => {
                                sender.send(Bytes::from(event.to_line())).is_ok()
                                    && !start_ctx.cancel.is_cancelled()
                            }
                            None => true,
                        };
                        if let (false, Some(vm)) = (delivered, started) {
                            let _ = VmLifecycle::stop_with(&vm, &start_ctx.paths, &start_ctx.tools);
                        }
                        return;
                    }
                    Err(_) => {
                        // A panic skipped every cleanup of the start. Remove what the id left.
                        let _ = VmLifecycle::stop_with(&id, &start_ctx.paths, &start_ctx.tools);
                        #[cfg(target_os = "macos")]
                        testanyware_vm::tart::remove_existing_in(&start_ctx.tools, &id);
                        Some(StartEvent::Error {
                            error: AppError::new(
                                "INTERNAL",
                                "The start failed inside the TestAnyware app.",
                                None,
                                json!({ "vm_id": id }),
                            ),
                        })
                    }
                };
                if let Some(event) = event {
                    let _ = sender.send(Bytes::from(event.to_line()));
                }
            }
        });
    if let Err(e) = spawned {
        let event = StartEvent::Error {
            error: AppError::new(
                "INTERNAL",
                format!("The TestAnyware app could not start a thread for the start: {e}."),
                None,
                serde_json::Value::Null,
            ),
        };
        let _ = sender.send(Bytes::from(event.to_line()));
    }
    // The threads hold the only senders. The body ends when the last one is gone.
    drop(sender);

    let mut response = Response::new(AppBody::events(receiver, cancel));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(START_CONTENT_TYPE));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(extra: serde_json::Value) -> Vec<u8> {
        let mut base = json!({
            "platform": "linux",
            "base": "testanyware-golden-linux-24.04",
            "id": "vm-1",
        });
        base.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        serde_json::to_vec(&base).unwrap()
    }

    #[test]
    fn a_request_becomes_the_options_of_the_direct_command() {
        let (request, options) = parse_request(&body(json!({
            "display": "1920x1080px",
            "keep_on_failure": true,
            "wait_secs": 30,
        })))
        .map_err(|r| r.error)
        .unwrap();
        assert_eq!(request.id, "vm-1");
        assert_eq!(options.platform, Platform::Linux);
        assert_eq!(options.base, "testanyware-golden-linux-24.04");
        assert_eq!(options.id, "vm-1");
        assert_eq!(options.display.as_deref(), Some("1920x1080px"));
        assert!(options.keep_on_failure);
        assert_eq!(options.wait, Some(std::time::Duration::from_secs(30)));
        assert!(!options.open_viewer, "the viewer runs in the Host CLI");
    }

    #[test]
    fn a_platform_that_is_unknown_is_the_error_of_the_direct_command() {
        let refusal = parse_request(&body(json!({ "platform": "beos" })))
            .err()
            .unwrap();
        assert_eq!(refusal.status, StatusCode::BAD_REQUEST);
        assert_eq!(refusal.error.code, "INVALID_PLATFORM");
        assert_eq!(refusal.error.details["platform"], "beos");
    }

    #[test]
    fn an_id_that_could_leave_the_state_directory_is_refused() {
        for id in ["../escape", "a/b", ".hidden", ""] {
            let refusal = parse_request(&body(json!({ "id": id }))).err().unwrap();
            assert_eq!(refusal.error.code, "USAGE_ERROR", "{id:?}");
        }
    }

    #[test]
    fn a_base_that_is_a_path_is_refused() {
        let refusal = parse_request(&body(json!({ "base": "../golden" })))
            .err()
            .unwrap();
        assert_eq!(refusal.error.code, "USAGE_ERROR");
    }

    #[test]
    fn a_body_that_is_no_request_is_a_usage_error() {
        let refusal = parse_request(b"{").err().unwrap();
        assert_eq!(refusal.error.code, "USAGE_ERROR");
        let refusal = parse_request(br#"{"platform":"linux"}"#).err().unwrap();
        assert_eq!(refusal.error.code, "USAGE_ERROR");
    }

    #[test]
    fn a_start_error_keeps_the_code_the_message_the_remedy_and_the_details() {
        let err = VmError::VmNotFound { id: "x".into() };
        let object = vm_error_object(&err);
        assert_eq!(object.code, "VM_NOT_FOUND");
        assert_eq!(object.message, err.to_string());
        assert_eq!(object.remediation, err.remediation());
        assert_eq!(object.details, err.details());
    }

    #[test]
    fn a_cancelled_start_has_no_final_event() {
        assert!(final_event(Err(VmError::StartCancelled { id: "x".into() })).is_none());
        assert!(final_event(Err(VmError::SwtpmMissing)).is_some());
    }

    #[test]
    fn the_tpm_decision_is_reported_in_the_shape_of_the_json_output() {
        let attached = tpm_report(TpmDecision::Attach { ppi_off: true });
        assert_eq!(
            serde_json::to_value(attached).unwrap(),
            json!({ "attached": true, "ppi_off": true })
        );
        let omitted = tpm_report(TpmDecision::Omit(OmitReason::GuestDoesNotNeedOne));
        assert_eq!(
            serde_json::to_value(omitted).unwrap(),
            json!({ "attached": false, "reason": "guest_does_not_need_one" })
        );
    }
}
