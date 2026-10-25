//! Run a script and check its value.

use std::path::{Path, PathBuf};

use rhai::{Dynamic, Engine, EvalAltResult, Map, Position};

use crate::{Config, ConfigError, ToolLocations, DEFAULT_MACOS_SLOTS, TOOL_NAMES};

/// The operation limit of one script. A settings map takes a few dozen operations, so the
/// limit only stops a script that loops. It stops `loop {}` instead of hanging a command.
pub const MAX_OPERATIONS: u64 = 100_000;

const TOP_LEVEL_KEYS: [&str; 4] = ["tools", "state_dir", "data_dir", "macos_slots"];

/// Evaluate `source`, the text of `file`, to the settings it names.
pub fn evaluate(source: &str, file: &Path) -> Result<Config, ConfigError> {
    let fail = |line: Option<usize>, message: String| ConfigError {
        file: file.to_path_buf(),
        line,
        message,
    };

    let value: Dynamic = engine().eval(source).map_err(|error| {
        let (line, message) = describe_eval_error(&error);
        fail(line, message)
    })?;
    let type_name = value.type_name();
    let Some(map) = value.try_cast::<Map>() else {
        return Err(fail(
            None,
            format!(
                "the script must evaluate to a map of settings, but it evaluated to {type_name}"
            ),
        ));
    };
    settings_from_map(map).map_err(|message| fail(None, message))
}

/// The engine of the spec: no files, no processes, no network, and an operation limit.
///
/// - Files: the `no_module` feature removes `import`, the one way a Rhai script reaches a file
///   (https://rhai.rs/book/rust/modules/ and https://docs.rs/crate/rhai/1.26.1/features).
/// - Processes and network: the standard packages register no function that reaches either.
/// - The limits are in https://docs.rs/rhai/1.26.1/rhai/struct.Engine.html.
fn engine() -> Engine {
    let mut engine = Engine::new();
    engine.set_max_operations(MAX_OPERATIONS);
    engine.set_max_call_levels(32);
    engine.set_max_expr_depths(64, 32);
    engine.set_max_string_size(4096);
    engine.set_max_array_size(1024);
    engine.set_max_map_size(256);
    // `eval` would run a string as a script outside the limits of the text the person wrote.
    engine.disable_symbol("eval");
    // `print` writes to stdout, which carries the `--json` envelope of the Host CLI.
    engine.on_print(|_| {});
    engine.on_debug(|_, _, _| {});
    engine
}

fn describe_eval_error(error: &EvalAltResult) -> (Option<usize>, String) {
    match error {
        EvalAltResult::ErrorParsing(kind, position) => (
            position.line(),
            format!(
                "syntax error: {}",
                without_position(kind.to_string(), *position)
            ),
        ),
        EvalAltResult::ErrorTooManyOperations(position) => (
            position.line(),
            format!("the script exceeds the limit of {MAX_OPERATIONS} operations"),
        ),
        other => {
            let position = other.position();
            (
                position.line(),
                without_position(other.to_string(), position),
            )
        }
    }
}

/// Rhai ends a message with ` (line N, position M)`. The error carries the line itself.
fn without_position(message: String, position: Position) -> String {
    match message.strip_suffix(&format!(" ({position})")) {
        Some(stripped) => stripped.to_string(),
        None => message,
    }
}

fn settings_from_map(map: Map) -> Result<Config, String> {
    let mut config = Config::default();
    for (key, value) in map {
        match key.as_str() {
            "tools" => config.tools = tools_from(value)?,
            "state_dir" => config.state_dir = Some(path_from("state_dir", value)?),
            "data_dir" => config.data_dir = Some(path_from("data_dir", value)?),
            "macos_slots" => config.macos_slots = Some(slots_from(value)?),
            unknown => {
                return Err(format!(
                    "unknown key `{unknown}`; the keys are {}",
                    quoted(&TOP_LEVEL_KEYS)
                ))
            }
        }
    }
    Ok(config)
}

fn tools_from(value: Dynamic) -> Result<ToolLocations, String> {
    let type_name = value.type_name();
    let Some(map) = value.try_cast::<Map>() else {
        return Err(format!("`tools` must be a map, but it is {type_name}"));
    };
    let mut tools = ToolLocations::default();
    for (name, value) in map {
        let slot = match name.as_str() {
            "tart" => &mut tools.tart,
            "qemu-system-aarch64" => &mut tools.qemu_system_aarch64,
            "swtpm" => &mut tools.swtpm,
            unknown => {
                return Err(format!(
                    "unknown tool `{unknown}` in `tools`; the tools are {}",
                    quoted(&TOOL_NAMES)
                ))
            }
        };
        *slot = Some(path_from(&format!("tools.{name}"), value)?);
    }
    Ok(tools)
}

/// An absolute path. A relative one would mean another place for each working directory.
fn path_from(setting: &str, value: Dynamic) -> Result<PathBuf, String> {
    let text = value
        .into_string()
        .map_err(|type_name| format!("`{setting}` must be a string, but it is {type_name}"))?;
    if text.is_empty() {
        return Err(format!("`{setting}` is empty"));
    }
    let path = PathBuf::from(&text);
    if !path.is_absolute() {
        return Err(format!(
            "`{setting}` must be an absolute path, but it is `{text}`"
        ));
    }
    Ok(path)
}

/// The share of the two slots: the integer 0, 1 or 2. Rhai reads `1.0` as a float, so it fails.
fn slots_from(value: Dynamic) -> Result<usize, String> {
    let type_name = value.type_name();
    let Ok(number) = value.as_int() else {
        return Err(format!(
            "`macos_slots` must be an integer, but it is {type_name}"
        ));
    };
    usize::try_from(number)
        .ok()
        .filter(|slots| *slots <= DEFAULT_MACOS_SLOTS)
        .ok_or_else(|| format!("`macos_slots` must be 0, 1 or 2, but it is {number}"))
}

fn quoted(names: &[&str]) -> String {
    names
        .iter()
        .map(|n| format!("`{n}`"))
        .collect::<Vec<_>>()
        .join(", ")
}
