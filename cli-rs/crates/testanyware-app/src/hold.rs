//! `POST /v1/slots`: the hold of a golden creation (spec "admission-queue": "Golden creation" and
//! "The protocol").
//!
//! A golden creation asks for a slot with the VMs that it boots. The response is NDJSON: any
//! number of `progress` events while the golden creation waits, then one `admitted` event or one
//! `error` event. After `admitted` the app sends nothing more, and the open response is the hold:
//! the thread of the request keeps its slot until the client closes the connection. The client
//! closes it after its setup VM has stopped, and the kernel closes it when the client crashes. In
//! both cases hyper drops the body of the response, and the body cancels the token of the request.
//!
//! The request runs on a thread of its own, as a start does, because it waits on a condition
//! variable of the queue, and a worker of the shared runtime must not block.

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::{Response, StatusCode};
use serde_json::json;
use testanyware_app_protocol::{
    AppError, HoldEvent, HoldRequest, RequestedBy, SLOTS_PATH, START_CONTENT_TYPE,
};
use testanyware_vm::{Progress, VmError};
use tokio::sync::mpsc;

use crate::body::AppBody;
use crate::start::{vm_error_object, Refusal};
use crate::AppContext;

/// How often the hold looks for the close of the connection. The token cannot wake a sleeping
/// thread, and the hold has nothing else to do.
const CLOSE_POLL: Duration = Duration::from_millis(100);

/// Read the request. Every check that needs no process comes first, so a bad request fails
/// before the stream.
pub fn parse_request(body: &[u8]) -> Result<HoldRequest, Refusal> {
    let usage = |message: String| Refusal {
        status: StatusCode::BAD_REQUEST,
        error: AppError::new(
            "USAGE_ERROR",
            message,
            None,
            json!({ "method": "POST", "path": SLOTS_PATH }),
        ),
    };
    let request: HoldRequest = serde_json::from_slice(body)
        .map_err(|e| usage(format!("The request to hold a slot does not read: {e}.")))?;
    request
        .validate()
        .map_err(|reason| usage(format!("The request to hold a slot is invalid: {reason}.")))?;
    Ok(request)
}

/// Join the queue for `request`, and answer with the events of the wait and then the hold.
pub fn hold_response(
    ctx: &Arc<AppContext>,
    request: HoldRequest,
    requested_by: RequestedBy,
) -> Response<AppBody> {
    // A quit that has begun admits nothing. A quit that begins later cancels the wait and ends
    // the hold.
    let Some((quit_guard, cancel)) = ctx.quit.enter() else {
        return crate::start::quitting_response();
    };
    let (sender, receiver) = mpsc::unbounded_channel::<Bytes>();
    // Dropping the body cancels the wait, and it ends the hold.
    let queue = Arc::clone(&ctx.admission);
    let spawned = std::thread::Builder::new().name("golden-hold".into()).spawn({
        let sender = sender.clone();
        let cancel = cancel.clone();
        move || {
            // The quit waits for this thread, the release of the slot included.
            let _quit_guard = quit_guard;
            let progress = Progress::to({
                let sender = sender.clone();
                move |line| {
                    let event = HoldEvent::Progress {
                        line: line.to_string(),
                    };
                    // A closed channel means the client left, and the cancel handles that.
                    let _ = sender.send(Bytes::from(event.to_line()));
                }
            });
            let mut vms = request.vms;
            vms.sort();
            vms.dedup();
            match queue.admit_golden(vms, Some(requested_by), &cancel, &progress) {
                Ok(slot) => {
                    let admitted = HoldEvent::Admitted {
                        request_id: slot.request_id().to_string(),
                    };
                    // A client that left before it read the event holds nothing, and the drop of
                    // the slot below releases it.
                    if sender.send(Bytes::from(admitted.to_line())).is_ok() {
                        while !cancel.is_cancelled() {
                            std::thread::sleep(CLOSE_POLL);
                        }
                    }
                    drop(slot);
                }
                // The client left while it waited. Nobody reads an event.
                Err(VmError::StartCancelled { .. }) => {}
                Err(err) => {
                    let event = HoldEvent::Error {
                        error: vm_error_object(&err),
                    };
                    let _ = sender.send(Bytes::from(event.to_line()));
                }
            }
        }
    });
    if let Err(e) = spawned {
        let event = HoldEvent::Error {
            error: AppError::new(
                "INTERNAL",
                format!("The TestAnyware app could not start a thread for the hold: {e}."),
                None,
                serde_json::Value::Null,
            ),
        };
        let _ = sender.send(Bytes::from(event.to_line()));
    }
    // The thread holds the only sender. The body ends when it is gone.
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

    #[test]
    fn a_request_with_plain_names_reads() {
        let request = parse_request(br#"{"vms":["testanyware-setup-ab12cd34"],"cwd":"/trees/a"}"#)
            .map_err(|r| r.error)
            .unwrap();
        assert_eq!(request.vms, ["testanyware-setup-ab12cd34"]);
        assert_eq!(request.cwd.as_deref(), Some("/trees/a"));
    }

    #[test]
    fn a_name_that_is_no_plain_vm_id_fails_with_a_usage_error_before_the_stream() {
        for body in [
            &br#"{"vms":["../escape"]}"#[..],
            br#"{"vms":["ok","a/b"]}"#,
            br#"{"vms":[]}"#,
            br#"{"vms":".x"}"#,
            b"{",
        ] {
            let Err(refusal) = parse_request(body) else {
                panic!("a refusal");
            };
            assert_eq!(refusal.status, StatusCode::BAD_REQUEST);
            assert_eq!(refusal.error.code, "USAGE_ERROR");
        }
    }
}
