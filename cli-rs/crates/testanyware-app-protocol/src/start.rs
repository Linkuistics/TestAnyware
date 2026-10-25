//! `POST /v1/vms`: `vm start` in the app (spec: "The protocol").
//!
//! The request body is the options that the Host CLI resolved, as JSON. The app reads nothing
//! from the environment of the client, so the Host CLI also states the id and the base that it
//! chose by default. The response is NDJSON, one event per line: any number of `progress`
//! events, then exactly one `result` or `error` event. The result event carries the data that
//! `vm start` prints, and the error event carries the error object of the other routes.

use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// The content type of the response.
pub const START_CONTENT_TYPE: &str = "application/x-ndjson";

/// The options of `vm start`, after the Host CLI parsed them and chose the defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartRequest {
    /// `macos`, `linux` or `windows`.
    pub platform: String,
    /// The golden image to clone.
    pub base: String,
    /// The VM id.
    pub id: String,
    /// The display of the backend, never with `@2x`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    /// The logical size of a HiDPI start (`--display WxH@2x`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logical: Option<(u32, u32)>,
    #[serde(default)]
    pub keep_on_failure: bool,
    /// The seconds that `--wait` waits for a macOS guest slot. `None` is no wait.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait_secs: Option<u64>,
    /// The working directory of the Host CLI, which names the worktree of the grove. `vm slots`
    /// shows it as who asked. An older app ignores it, and an older Host CLI states none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

/// A host and a port of the guest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostPort {
    pub host: String,
    pub port: u16,
}

/// The TPM decision of the start, in the shape that `vm start --json` prints as `tpm`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TpmReport {
    pub attached: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ppi_off: Option<bool>,
    /// Why no TPM is attached: `guest_does_not_need_one` or `host_cannot_supply_one`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// What a successful start made. The Host CLI prints the `vm start` output from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartResult {
    pub id: String,
    pub platform: String,
    pub vnc: HostPort,
    pub agent: Option<HostPort>,
    pub spec_path: String,
    pub meta_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tpm: Option<TpmReport>,
}

/// One line of the response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum StartEvent {
    /// A line for stderr, as `vm start` prints it today.
    Progress { line: String },
    /// The start succeeded. This event ends the stream.
    Result { data: StartResult },
    /// The start failed. This event ends the stream.
    Error { error: AppError },
}

impl StartEvent {
    /// The line of the stream: the event as JSON, and a newline.
    pub fn to_line(&self) -> String {
        let mut line = serde_json::to_string(self).expect("an event serialises");
        line.push('\n');
        line
    }

    /// Whether this event ends the stream.
    pub fn is_final(&self) -> bool {
        !matches!(self, StartEvent::Progress { .. })
    }
}

/// Splits a stream of bytes into events of type `E`. The bytes of one line may arrive in several
/// pieces, and several lines in one piece. The stream of `vm start` and the stream of the hold
/// of a golden creation use the same lines, with their own events.
#[derive(Debug)]
pub struct NdjsonLines<E> {
    pending: Vec<u8>,
    events: std::marker::PhantomData<fn() -> E>,
}

impl<E> Default for NdjsonLines<E> {
    fn default() -> Self {
        Self {
            pending: Vec::new(),
            events: std::marker::PhantomData,
        }
    }
}

/// The lines of the stream of `vm start`.
pub type EventLines = NdjsonLines<StartEvent>;

impl<E: serde::de::DeserializeOwned> NdjsonLines<E> {
    /// Add `bytes` and return the events they complete. A line that is no event is an error.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<E>, String> {
        self.pending.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(end) = self.pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            let line = &line[..line.len() - 1];
            if line.is_empty() {
                continue;
            }
            let event = serde_json::from_slice(line).map_err(|e| {
                format!(
                    "{e} in the line {:?}",
                    String::from_utf8_lossy(line)
                        .chars()
                        .take(200)
                        .collect::<String>()
                )
            })?;
            events.push(event);
        }
        Ok(events)
    }

    /// Whether the bytes after the last newline are not an event yet.
    pub fn has_partial_line(&self) -> bool {
        !self.pending.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request() -> StartRequest {
        StartRequest {
            platform: "macos".into(),
            base: "testanyware-golden-macos-27".into(),
            id: "testanyware-ab12cd34".into(),
            display: Some("1920x1080px".into()),
            logical: None,
            keep_on_failure: true,
            wait_secs: Some(600),
            cwd: Some("/trees/one".into()),
        }
    }

    #[test]
    fn a_request_round_trips_and_omits_what_is_not_set() {
        let mut r = request();
        r.display = None;
        r.wait_secs = None;
        r.cwd = None;
        let text = serde_json::to_string(&r).unwrap();
        assert!(!text.contains("display"), "{text}");
        assert!(!text.contains("wait_secs"), "{text}");
        assert!(!text.contains("cwd"), "{text}");
        assert_eq!(serde_json::from_str::<StartRequest>(&text).unwrap(), r);
        let full = serde_json::to_string(&request()).unwrap();
        assert_eq!(
            serde_json::from_str::<StartRequest>(&full).unwrap(),
            request()
        );
    }

    #[test]
    fn a_request_of_an_older_host_cli_states_no_cwd_and_reads() {
        let older = r#"{"platform":"macos","base":"b","id":"i"}"#;
        assert_eq!(serde_json::from_str::<StartRequest>(older).unwrap().cwd, None);
    }

    #[test]
    fn a_request_needs_the_platform_the_base_and_the_id() {
        for missing in ["platform", "base", "id"] {
            let mut value = serde_json::to_value(request()).unwrap();
            value.as_object_mut().unwrap().remove(missing);
            assert!(
                serde_json::from_value::<StartRequest>(value).is_err(),
                "{missing}"
            );
        }
    }

    #[test]
    fn the_events_have_the_names_that_the_spec_gives() {
        let progress = serde_json::to_value(StartEvent::Progress { line: "hi".into() }).unwrap();
        assert_eq!(progress, json!({ "event": "progress", "line": "hi" }));
        let error = serde_json::to_value(StartEvent::Error {
            error: AppError::new("VM_LIMIT_REACHED", "m", None, json!({ "limit": 2 })),
        })
        .unwrap();
        assert_eq!(error["event"], "error");
        assert_eq!(error["error"]["code"], "VM_LIMIT_REACHED");
        let result = serde_json::to_value(StartEvent::Result {
            data: StartResult {
                id: "i".into(),
                platform: "macos".into(),
                vnc: HostPort {
                    host: "127.0.0.1".into(),
                    port: 5900,
                },
                agent: None,
                spec_path: "/s".into(),
                meta_path: "/m".into(),
                tpm: Some(TpmReport {
                    attached: false,
                    ppi_off: None,
                    reason: Some("x".into()),
                }),
            },
        })
        .unwrap();
        assert_eq!(result["event"], "result");
        assert_eq!(
            result["data"]["tpm"],
            json!({ "attached": false, "reason": "x" })
        );
        assert_eq!(result["data"]["agent"], serde_json::Value::Null);
    }

    #[test]
    fn only_progress_does_not_end_the_stream() {
        assert!(!StartEvent::Progress { line: "x".into() }.is_final());
        assert!(StartEvent::Error {
            error: AppError::new("X", "m", None, json!(null))
        }
        .is_final());
    }

    #[test]
    fn lines_are_found_whatever_the_size_of_the_pieces() {
        let text = format!(
            "{}{}",
            StartEvent::Progress { line: "one".into() }.to_line(),
            StartEvent::Progress { line: "two".into() }.to_line()
        );
        let bytes = text.as_bytes();
        // One piece: both lines. Single bytes: the same two events.
        let mut whole = EventLines::default();
        assert_eq!(whole.push(bytes).unwrap().len(), 2);
        let mut bytewise = EventLines::default();
        let mut events = Vec::new();
        for b in bytes {
            events.extend(bytewise.push(std::slice::from_ref(b)).unwrap());
        }
        assert_eq!(events.len(), 2);
        assert_eq!(events[1], StartEvent::Progress { line: "two".into() });
        assert!(!bytewise.has_partial_line());
    }

    #[test]
    fn a_partial_line_waits_for_its_newline() {
        let mut lines = EventLines::default();
        assert!(lines
            .push(br#"{"event":"progress","li"#)
            .unwrap()
            .is_empty());
        assert!(lines.has_partial_line());
        let events = lines.push(b"ne\":\"x\"}\n").unwrap();
        assert_eq!(events, vec![StartEvent::Progress { line: "x".into() }]);
    }

    #[test]
    fn a_line_that_is_no_event_is_an_error_that_quotes_it() {
        let err = EventLines::default().push(b"not json\n").unwrap_err();
        assert!(err.contains("not json"), "{err}");
    }
}
