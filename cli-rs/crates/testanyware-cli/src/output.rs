//! Helpers for the §3 JSON envelope and §3.4 error envelope.
//!
//! Every data-producing command supports `--json` per §3.1. Successful
//! output emits a single object with `schema_version: "1.0"`, `ok: true`,
//! and command-specific fields. Failures emit `{schema_version, ok: false,
//! code, message, remediation?, details?}` and exit per §5.

use serde::Serialize;
use serde_json::{json, Value};

pub const SCHEMA_VERSION: &str = "1.0";

/// Output mode selector for data-producing commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Text,
    Json,
}

impl OutputMode {
    pub fn from_flags(json: bool) -> Self {
        if json {
            OutputMode::Json
        } else {
            OutputMode::Text
        }
    }

    pub fn is_json(self) -> bool {
        matches!(self, OutputMode::Json)
    }
}

/// Build a success envelope. Caller-supplied fields are merged on top of
/// `schema_version` and `ok: true`.
pub fn success_envelope(payload: Value) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("schema_version".into(), json!(SCHEMA_VERSION));
    obj.insert("ok".into(), json!(true));
    if let Value::Object(payload_obj) = payload {
        for (k, v) in payload_obj {
            obj.insert(k, v);
        }
    }
    Value::Object(obj)
}

/// Print a success envelope as pretty JSON on stdout, then exit 0.
pub fn print_success<T: Serialize>(payload: T) {
    let value = serde_json::to_value(payload).expect("serialize payload");
    let envelope = success_envelope(value);
    let body = serde_json::to_string_pretty(&envelope).expect("serialize envelope");
    println!("{body}");
}

/// Build the §3.4 error envelope.
pub fn error_envelope(
    code: &str,
    message: impl Into<String>,
    remediation: Option<String>,
    details: Value,
) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("schema_version".into(), json!(SCHEMA_VERSION));
    obj.insert("ok".into(), json!(false));
    obj.insert("code".into(), json!(code));
    obj.insert("message".into(), json!(message.into()));
    if let Some(r) = remediation {
        obj.insert("remediation".into(), json!(r));
    }
    if !details.is_null() {
        obj.insert("details".into(), details);
    }
    Value::Object(obj)
}

/// Print the error envelope (JSON mode) or a plain message (text mode)
/// and exit with the supplied §5 exit code.
pub fn print_error(
    mode: OutputMode,
    code: &str,
    message: &str,
    remediation: Option<&str>,
    details: Value,
    exit_code: i32,
) -> ! {
    match mode {
        OutputMode::Json => {
            let envelope = error_envelope(
                code,
                message,
                remediation.map(|s| s.to_string()),
                details,
            );
            println!(
                "{}",
                serde_json::to_string_pretty(&envelope).expect("serialize error envelope")
            );
        }
        OutputMode::Text => {
            eprintln!("error: {message}");
            if let Some(r) = remediation {
                eprintln!("  hint: {r}");
            }
            // Aggressive discoverability: every text-mode failure points
            // an LLM agent at the full usage guide.
            eprintln!("  → run `testanyware llm-instructions` for the full usage guide");
        }
    }
    std::process::exit(exit_code);
}

/// §5 exit-code lookup keyed by the §4 code string. Catches all currently
/// emitted codes; unknown strings fall through to `1` (generic failure).
pub fn exit_code_for(code: &str) -> i32 {
    match code {
        // §5: 0 success — never reached here.
        "USAGE_ERROR"
        | "NO_CONNECTION_SPECIFIED"
        | "INVALID_PLATFORM"
        | "INVALID_ENDPOINT"
        | "UNKNOWN_KEY"
        | "UNKNOWN_BUTTON" => 2,

        "VM_NOT_FOUND"
        | "WINDOW_NOT_FOUND"
        | "ELEMENT_NOT_FOUND"
        | "GOLDEN_NOT_FOUND"
        | "UEFI_NOT_FOUND"
        | "SCHEMA_NOT_FOUND"
        | "TEXT_NOT_FOUND" => 3,

        "AUTH_REQUIRED" | "KVM_PERMISSION_DENIED" => 4,

        "GOLDEN_IN_USE"
        | "RECORD_ALREADY_ACTIVE"
        | "ELEMENT_AMBIGUOUS"
        | "ACTION_UNSUPPORTED" => 5,

        "VM_BOOT_TIMEOUT" | "CONNECTION_TIMEOUT" | "OCR_TIMEOUT" => 7,

        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_envelope_merges_payload() {
        let env = success_envelope(json!({ "answer": 42 }));
        assert_eq!(env["schema_version"], "1.0");
        assert_eq!(env["ok"], true);
        assert_eq!(env["answer"], 42);
    }

    #[test]
    fn error_envelope_includes_required_fields() {
        let env = error_envelope(
            "VM_NOT_FOUND",
            "no such vm",
            Some("try `testanyware vm list`".into()),
            json!({ "vm_id": "testanyware-deadbeef" }),
        );
        assert_eq!(env["ok"], false);
        assert_eq!(env["code"], "VM_NOT_FOUND");
        assert_eq!(env["message"], "no such vm");
        assert_eq!(env["details"]["vm_id"], "testanyware-deadbeef");
    }

    #[test]
    fn error_envelope_omits_null_details() {
        let env = error_envelope("INTERNAL", "boom", None, Value::Null);
        assert!(env.as_object().unwrap().get("details").is_none());
        assert!(env.as_object().unwrap().get("remediation").is_none());
    }

    #[test]
    fn exit_code_table_matches_contract_section_5() {
        assert_eq!(exit_code_for("USAGE_ERROR"), 2);
        assert_eq!(exit_code_for("VM_NOT_FOUND"), 3);
        assert_eq!(exit_code_for("ELEMENT_NOT_FOUND"), 3);
        assert_eq!(exit_code_for("TEXT_NOT_FOUND"), 3);
        assert_eq!(exit_code_for("AUTH_REQUIRED"), 4);
        assert_eq!(exit_code_for("KVM_PERMISSION_DENIED"), 4);
        assert_eq!(exit_code_for("SWTPM_MISSING"), 1);
        assert_eq!(exit_code_for("ELEMENT_AMBIGUOUS"), 5);
        assert_eq!(exit_code_for("CONNECTION_TIMEOUT"), 7);
        assert_eq!(exit_code_for("INTERNAL"), 1);
    }
}
