//! `testanyware` CLI binary (Rust port).
//!
//! Surface follows the noun-first canonical layout from
//! `docs/architecture/cli-design-contract.md` §1, with the curated
//! verb-first aliases from §1's alias table announcing themselves per
//! §7.2. The agent-only commands (`agent {health,windows,snapshot,
//! inspect,press}`, `file {exec,upload,download}`) and their verb-first
//! aliases are fully wired; the remaining subcommands are stubs that
//! print `not yet implemented` and exit with status 2 — they land in
//! later port tasks tracked in `LLM_STATE/core/`.

use clap::{Args, Parser, Subcommand};

use testanyware_cli::commands::{
    agent as agent_cmds, file as file_cmds, input as input_cmds, screen as screen_cmds,
};
use testanyware_cli::discoverability::{run_capabilities, run_llm_instructions, run_schema};
use testanyware_cli::output::OutputMode;
use testanyware_cli::resolve::ConnectionOptions as ResolveOptions;

// §7-template "after-help" blocks.

const CAPABILITIES_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: --json (default; this is a machine-only command).
    Schema: capabilities.

EXIT CODES:
    0  success

EXAMPLES:
    # Detect whether the schema subcommand is available
    testanyware capabilities | jq '.features.schema_command'

    # List every error code the binary can emit
    testanyware capabilities | jq -r '.error_codes[]'

    # Diff the surface across two builds
    testanyware capabilities | jq -S . > /tmp/now.json

SEE ALSO:
    testanyware schema, testanyware llm-instructions, testanyware --help
";

const SCHEMA_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: JSON Schema (Draft 2020-12) on stdout for known
    commands. Error envelope on stdout (and exit 3) on miss.

EXIT CODES:
    0  success
    2  USAGE_ERROR (no command argument supplied)
    3  SCHEMA_NOT_FOUND (command unknown or has no declared schema)

EXAMPLES:
    # Print the schema for `vm list --json`
    testanyware schema vm list

    # Pipe into a JSON Schema validator
    testanyware schema agent snapshot | check-jsonschema --schemafile -

SEE ALSO:
    testanyware capabilities, testanyware <command> --help
";

const LLM_INSTRUCTIONS_AFTER_HELP: &str = "\
OUTPUT:
    Plain text on stdout. Not a structured format.

EXIT CODES:
    0  success

EXAMPLES:
    # Prepend as context to an LLM agent prompt
    testanyware llm-instructions

    # Save for offline use
    testanyware llm-instructions > testanyware-llm.txt

SEE ALSO:
    testanyware capabilities, testanyware schema, testanyware --help
";

const AGENT_HEALTH_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (`OK` or `UNHEALTHY: <reason>`), --json
    (schema: agent-health).

EXIT CODES:
    0  success (agent reachable, accessibility granted)
    1  generic agent failure
    2  USAGE_ERROR / NO_CONNECTION_SPECIFIED
    3  VM_NOT_FOUND
    4  AUTH_REQUIRED (agent reachable but accessibility not granted)
    7  CONNECTION_TIMEOUT

EXAMPLES:
    # Quick reachability check via a running VM
    testanyware agent health --vm \"$TESTANYWARE_VM_ID\"

    # Direct endpoint, scriptable
    testanyware agent health --agent 192.168.64.5:8648 --json

SEE ALSO:
    testanyware agent windows, testanyware doctor
";

const AGENT_WINDOWS_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (one window per line), --json (schema:
    agent-windows).

EXIT CODES:
    0  success
    1  generic agent failure
    2  NO_CONNECTION_SPECIFIED
    3  VM_NOT_FOUND
    4  AUTH_REQUIRED
    7  CONNECTION_TIMEOUT

EXAMPLES:
    # List visible windows on a running VM
    testanyware agent windows --vm \"$TESTANYWARE_VM_ID\"

    # JSON for scripting
    testanyware agent windows --vm \"$TESTANYWARE_VM_ID\" --json | jq '.windows[].title'

SEE ALSO:
    testanyware agent snapshot, testanyware agent wait
";

const AGENT_SNAPSHOT_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (formatted tree), --json (schema:
    agent-snapshot).

EXIT CODES:
    0  success
    1  generic agent failure
    2  NO_CONNECTION_SPECIFIED
    3  VM_NOT_FOUND / WINDOW_NOT_FOUND
    4  AUTH_REQUIRED
    5  ACTION_UNSUPPORTED (e.g. --open-menu without VNC)
    7  CONNECTION_TIMEOUT

EXAMPLES:
    # Interact-mode snapshot of a specific window
    testanyware agent snapshot --vm \"$TESTANYWARE_VM_ID\" --window \"Settings\"

    # Full tree for layout analysis
    testanyware agent snapshot --vm \"$TESTANYWARE_VM_ID\" --mode full --json

SEE ALSO:
    testanyware agent windows, testanyware agent inspect
";

const AGENT_INSPECT_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (one element + bounds + font), --json
    (schema: agent-inspect).

EXIT CODES:
    0  success
    3  ELEMENT_NOT_FOUND / WINDOW_NOT_FOUND
    5  ELEMENT_AMBIGUOUS

EXAMPLES:
    # Inspect a specific button
    testanyware agent inspect --vm \"$TESTANYWARE_VM_ID\" --role button --label \"Save\"

    # Drill into a labelled text field as JSON
    testanyware agent inspect --vm \"$TESTANYWARE_VM_ID\" --role textfield --label \"Email\" --json

SEE ALSO:
    testanyware agent snapshot, testanyware agent press
";

const AGENT_PRESS_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (`OK` / `FAILED: <message>`), --json (schema:
    agent-action).

EXIT CODES:
    0  success
    3  ELEMENT_NOT_FOUND / WINDOW_NOT_FOUND
    5  ELEMENT_AMBIGUOUS / ACTION_UNSUPPORTED

IDEMPOTENCY:
    Not idempotent — pressing twice equals two presses. Retry only when
    the previous attempt's outcome is unknown.

EXAMPLES:
    # Press a button by role + label
    testanyware agent press --vm \"$TESTANYWARE_VM_ID\" --role button --label \"OK\"

    # Plan only — emit the request without performing it
    testanyware agent press --vm \"$TESTANYWARE_VM_ID\" --role button --label \"OK\" --dry-run --json

SEE ALSO:
    testanyware agent inspect, testanyware agent set-value
";

const FILE_EXEC_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: stdout/stderr passthrough in text mode (Swift
    parity); --json emits a single envelope (schema: file-exec). The
    binary's exit code is the in-VM process's exit code in text mode;
    --json mode exits 0 on a clean spawn and surfaces the in-VM exit
    via `details.exit_code` and the top-level `exit_code` field.

EXIT CODES (text mode):
    <in-VM exit code> (0 on success)
    1  EXEC_FAILED, generic agent failure
    2  USAGE_ERROR / NO_CONNECTION_SPECIFIED
    3  VM_NOT_FOUND
    7  CONNECTION_TIMEOUT

IDEMPOTENCY:
    Application-defined.

EXAMPLES:
    # Run a quick command in the guest
    testanyware file exec --vm \"$TESTANYWARE_VM_ID\" \"uname -a\"

    # Capture exit code as JSON
    testanyware file exec --vm \"$TESTANYWARE_VM_ID\" --json \"true\"

    # Plan a longer command without spawning it
    testanyware file exec --vm \"$TESTANYWARE_VM_ID\" --dry-run --timeout 120 \"sleep 60\"

SEE ALSO:
    testanyware file upload, testanyware file download, testanyware doctor
";

const FILE_UPLOAD_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (one-line confirmation), --json (schema:
    file-upload).

EXIT CODES:
    0  success
    1  UPLOAD_FAILED, generic agent failure
    2  USAGE_ERROR / NO_CONNECTION_SPECIFIED
    3  VM_NOT_FOUND
    7  CONNECTION_TIMEOUT

IDEMPOTENCY:
    Idempotent — overwrites the remote file completely. Retry-safe.

EXAMPLES:
    # Upload a build artefact
    testanyware file upload --vm \"$TESTANYWARE_VM_ID\" ./out.bin /tmp/out.bin

    # Plan only
    testanyware file upload --vm \"$TESTANYWARE_VM_ID\" ./out.bin /tmp/out.bin --dry-run

SEE ALSO:
    testanyware file download, testanyware file exec
";

const FILE_DOWNLOAD_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (one-line confirmation), --json (schema:
    file-download).

EXIT CODES:
    0  success
    1  DOWNLOAD_FAILED, generic agent failure
    2  USAGE_ERROR / NO_CONNECTION_SPECIFIED
    3  VM_NOT_FOUND
    7  CONNECTION_TIMEOUT

IDEMPOTENCY:
    Idempotent — overwrites the local file completely. Retry-safe.

EXAMPLES:
    # Pull a log file off the VM
    testanyware file download --vm \"$TESTANYWARE_VM_ID\" /var/log/system.log ./system.log

    # JSON envelope for scripting
    testanyware file download --vm \"$TESTANYWARE_VM_ID\" /tmp/x ./x --json

SEE ALSO:
    testanyware file upload, testanyware file exec
";

#[derive(Parser, Debug)]
#[command(
    name = "testanyware",
    about = "VNC + agent driver for virtual machine automation",
    version,
    propagate_version = true,
    arg_required_else_help = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// Connection-resolution options shared by most subcommands.
///
/// Resolution chain (highest priority first):
///
///   1. `--connect <path>` — explicit spec file
///   2. `--vm <id>` — per-VM spec under `$XDG_STATE_HOME/testanyware/vms/`
///   3. `--vnc` / `--agent` / `--platform` — explicit flags
///   4. `TESTANYWARE_VM_ID` env — resolves like `--vm`
///   5. `TESTANYWARE_VNC` / `TESTANYWARE_AGENT` / `TESTANYWARE_PLATFORM`
///      env — direct env vars
///   6. Error
#[derive(Args, Debug, Clone)]
struct ConnectionOptions {
    #[arg(long, value_name = "PATH", help = "Path to connection spec JSON file")]
    connect: Option<String>,

    #[arg(long, value_name = "ID", env = "TESTANYWARE_VM_ID")]
    vm: Option<String>,

    #[arg(long, value_name = "HOST:PORT", env = "TESTANYWARE_VNC")]
    vnc: Option<String>,

    #[arg(long, value_name = "HOST:PORT", env = "TESTANYWARE_AGENT")]
    agent: Option<String>,

    #[arg(long, value_name = "PLATFORM", env = "TESTANYWARE_PLATFORM")]
    platform: Option<String>,
}

impl From<ConnectionOptions> for ResolveOptions {
    fn from(c: ConnectionOptions) -> Self {
        ResolveOptions {
            connect: c.connect,
            vm: c.vm,
            agent: c.agent,
            vnc: c.vnc,
            platform: c.platform,
        }
    }
}

#[derive(Subcommand, Debug)]
enum Command {
    // ---- Noun-first canonical commands (§1) -----------------------------

    /// Screen capture, recording, OCR, and dimensions
    Screen {
        #[command(subcommand)]
        action: ScreenAction,
    },

    /// File transfer and command execution in the guest
    File {
        #[command(subcommand)]
        action: FileAction,
    },

    /// Send keyboard or mouse input via VNC
    Input {
        #[command(subcommand)]
        action: InputAction,
    },

    /// In-VM agent commands
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },

    /// VM lifecycle
    Vm {
        #[command(subcommand)]
        action: VmAction,
    },

    /// Diagnose the local install
    Doctor,

    /// Print stable JSON describing the binary's surface
    #[command(
        long_about = "Print stable JSON describing the binary's surface.\n\n\
                      Emits a single JSON document on stdout enumerating the canonical \
                      command tree, alias maps, output formats, supported platforms, \
                      and the full error-code catalogue from contract §4. Agents poll \
                      this to detect feature availability without parsing --help.\n\n\
                      Default output is JSON — this is a machine-only command. The \
                      --json flag is accepted for symmetry but is a no-op.",
        after_long_help = CAPABILITIES_AFTER_HELP,
    )]
    Capabilities {
        /// Accepted for symmetry with other commands; output is always JSON.
        #[arg(long)]
        json: bool,
    },

    /// Emit a JSON schema for a command's `--json` output
    #[command(
        long_about = "Emit the JSON Schema for a command's --json output.\n\n\
                      Reads from docs/reference/cli-schemas/<schema-id>.json (embedded \
                      at build time via include_str!). The argument is the canonical \
                      noun-first command path, e.g. `schema vm list` or \
                      `schema screen capture`. Verb-first aliases are not accepted; \
                      pass the canonical form.",
        after_long_help = SCHEMA_AFTER_HELP,
    )]
    Schema {
        /// Command path tokens, e.g. `vm start` or `screen capture`
        #[arg(value_name = "COMMAND")]
        command: Vec<String>,
    },

    /// Emit a focused LLM-oriented manual
    #[command(
        long_about = "Emit a focused LLM-oriented manual for the binary.\n\n\
                      Plain-text manual covering the mental model (noun-first \
                      commands, verb-first aliases, connection resolution chain), \
                      two or three end-to-end workflows, common mistakes, and \
                      pointers to --json / exit codes / per-command schemas. Capped \
                      at ~3000 tokens so an LLM can prepend it as context.",
        after_long_help = LLM_INSTRUCTIONS_AFTER_HELP,
    )]
    LlmInstructions,

    /// Run as an agent server (development helper)
    Server {
        #[arg(long, default_value_t = 8648)]
        port: u16,
    },

    // ---- Verb-first aliases (§1, §7.2) ----------------------------------

    /// Alias of `testanyware screen capture`. Run that for full help.
    Screenshot(ScreenCaptureArgs),

    /// Alias of `testanyware screen record`. Run that for full help.
    Record(ScreenRecordArgs),

    /// Alias of `testanyware screen size`. Run that for full help.
    ScreenSize(ConnectionArgs),

    /// Alias of `testanyware screen find-text`. Run that for full help.
    FindText(ScreenFindTextArgs),

    /// Alias of `testanyware file upload`. Run that for full help.
    Upload(FileUploadArgs),

    /// Alias of `testanyware file download`. Run that for full help.
    Download(FileDownloadArgs),

    /// Alias of `testanyware file exec`. Run that for full help.
    Exec(FileExecArgs),
}

// ---- Shared args structs (used by both canonical and alias variants) ----

#[derive(Args, Debug, Clone)]
struct ConnectionArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    #[arg(long, help = "Emit JSON envelope on stdout")]
    json: bool,
}

#[derive(Args, Debug, Clone)]
struct ScreenCaptureArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    #[arg(short = 'o', value_name = "FILE")]
    output: Option<String>,
    #[arg(long, value_name = "X,Y,W,H")]
    region: Option<String>,
    #[arg(long, help = "Emit JSON envelope on stdout")]
    json: bool,
}

#[derive(Args, Debug, Clone)]
struct ScreenRecordArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    #[arg(short = 'o', value_name = "FILE")]
    output: String,
    #[arg(long)]
    fps: Option<u32>,
    #[arg(long)]
    duration: Option<u32>,
}

#[derive(Args, Debug, Clone)]
struct ScreenFindTextArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    text: Option<String>,
    #[arg(long)]
    timeout: Option<u32>,
}

#[derive(Args, Debug, Clone)]
struct FileUploadArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    local: String,
    remote: String,
    #[arg(long, help = "Emit JSON envelope on stdout")]
    json: bool,
    #[arg(long, help = "Plan the upload but do not perform it")]
    dry_run: bool,
}

#[derive(Args, Debug, Clone)]
struct FileDownloadArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    remote: String,
    local: String,
    #[arg(long, help = "Emit JSON envelope on stdout")]
    json: bool,
    #[arg(long, help = "Plan the download but do not perform it")]
    dry_run: bool,
}

#[derive(Args, Debug, Clone)]
struct FileExecArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    /// Command to execute in the guest
    command: String,
    /// Per-command timeout in seconds (the agent enforces; HTTP layer
    /// gets a +10s buffer).
    #[arg(long, default_value_t = 30)]
    timeout: i64,
    /// Spawn detached and return immediately without waiting.
    #[arg(long)]
    detach: bool,
    #[arg(long, help = "Emit JSON envelope on stdout (no stdout/stderr passthrough)")]
    json: bool,
    #[arg(long, help = "Plan the exec but do not run it")]
    dry_run: bool,
}

// ---- Subcommand groups --------------------------------------------------

#[derive(Subcommand, Debug)]
enum ScreenAction {
    /// Capture a screenshot via VNC
    Capture(ScreenCaptureArgs),
    /// Record VNC framebuffer to MP4
    Record(ScreenRecordArgs),
    /// Print VNC display dimensions ("WxH")
    Size(ConnectionArgs),
    /// OCR the screen and find text
    FindText(ScreenFindTextArgs),
}

#[derive(Subcommand, Debug)]
enum FileAction {
    /// Upload a file from host to guest
    #[command(after_long_help = FILE_UPLOAD_AFTER_HELP)]
    Upload(FileUploadArgs),
    /// Download a file from guest to host
    #[command(after_long_help = FILE_DOWNLOAD_AFTER_HELP)]
    Download(FileDownloadArgs),
    /// Run a command in the guest, capture stdout/stderr/exit
    #[command(after_long_help = FILE_EXEC_AFTER_HELP)]
    Exec(FileExecArgs),
}

#[derive(Subcommand, Debug)]
enum InputAction {
    /// Press and release a key
    Key {
        #[command(flatten)]
        conn: ConnectionOptions,
        key: String,
        #[arg(long, value_delimiter = ',')]
        modifiers: Vec<String>,
        #[arg(long, help = "Emit JSON envelope on stdout")]
        json: bool,
    },
    /// Press a key (no release)
    KeyDown {
        #[command(flatten)]
        conn: ConnectionOptions,
        key: String,
        #[arg(long, help = "Emit JSON envelope on stdout")]
        json: bool,
    },
    /// Release a key
    KeyUp {
        #[command(flatten)]
        conn: ConnectionOptions,
        key: String,
        #[arg(long, help = "Emit JSON envelope on stdout")]
        json: bool,
    },
    /// Type a string
    Type {
        #[command(flatten)]
        conn: ConnectionOptions,
        text: String,
        #[arg(long, help = "Emit JSON envelope on stdout")]
        json: bool,
    },
    /// Click at a point
    Click {
        #[command(flatten)]
        conn: ConnectionOptions,
        x: i32,
        y: i32,
        #[arg(long, default_value = "left")]
        button: String,
        #[arg(long, default_value_t = 1)]
        count: u32,
        #[arg(long, help = "Emit JSON envelope on stdout")]
        json: bool,
    },
    /// Press a mouse button (no release)
    MouseDown {
        #[command(flatten)]
        conn: ConnectionOptions,
        x: i32,
        y: i32,
        #[arg(long, default_value = "left")]
        button: String,
        #[arg(long, help = "Emit JSON envelope on stdout")]
        json: bool,
    },
    /// Release a mouse button
    MouseUp {
        #[command(flatten)]
        conn: ConnectionOptions,
        x: i32,
        y: i32,
        #[arg(long, default_value = "left")]
        button: String,
        #[arg(long, help = "Emit JSON envelope on stdout")]
        json: bool,
    },
    /// Move the mouse cursor
    Move {
        #[command(flatten)]
        conn: ConnectionOptions,
        x: i32,
        y: i32,
        #[arg(long, help = "Emit JSON envelope on stdout")]
        json: bool,
    },
    /// Scroll at a point
    Scroll {
        #[command(flatten)]
        conn: ConnectionOptions,
        x: i32,
        y: i32,
        #[arg(long)]
        dx: Option<i32>,
        #[arg(long)]
        dy: Option<i32>,
        #[arg(long, help = "Emit JSON envelope on stdout")]
        json: bool,
    },
    /// Drag from one point to another
    Drag {
        #[command(flatten)]
        conn: ConnectionOptions,
        from_x: i32,
        from_y: i32,
        to_x: i32,
        to_y: i32,
        #[arg(long, default_value = "left")]
        button: String,
        #[arg(long, default_value_t = 10)]
        steps: u32,
        #[arg(long, help = "Emit JSON envelope on stdout")]
        json: bool,
    },
}

#[derive(Args, Debug, Clone)]
struct AgentElementArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    #[arg(long)]
    role: String,
    #[arg(long)]
    label: Option<String>,
    #[arg(long)]
    window: Option<String>,
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    index: Option<i64>,
    #[arg(long, help = "Emit JSON envelope on stdout")]
    json: bool,
}

#[derive(Args, Debug, Clone)]
struct AgentPressArgs {
    #[command(flatten)]
    common: AgentElementArgs,
    #[arg(long, help = "Plan the press but do not perform it")]
    dry_run: bool,
}

#[derive(Args, Debug, Clone)]
struct AgentSetValueArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    #[arg(long)]
    role: String,
    #[arg(long)]
    label: Option<String>,
    #[arg(long)]
    value: String,
}

#[derive(Args, Debug, Clone)]
struct AgentWindowArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    #[arg(long)]
    window: String,
}

#[derive(Args, Debug, Clone)]
struct AgentWindowMoveArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    #[arg(long)]
    window: String,
    x: i32,
    y: i32,
}

#[derive(Args, Debug, Clone)]
struct AgentWindowResizeArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    #[arg(long)]
    window: String,
    width: u32,
    height: u32,
}

#[derive(Subcommand, Debug)]
enum AgentAction {
    /// Check the agent is reachable
    #[command(after_long_help = AGENT_HEALTH_AFTER_HELP)]
    Health {
        #[command(flatten)]
        conn: ConnectionOptions,
        #[arg(long, help = "Emit JSON envelope on stdout")]
        json: bool,
    },
    /// List visible windows
    #[command(after_long_help = AGENT_WINDOWS_AFTER_HELP)]
    Windows {
        #[command(flatten)]
        conn: ConnectionOptions,
        #[arg(long, help = "Emit JSON envelope on stdout")]
        json: bool,
    },
    /// Snapshot the accessibility tree
    #[command(after_long_help = AGENT_SNAPSHOT_AFTER_HELP)]
    Snapshot {
        #[command(flatten)]
        conn: ConnectionOptions,
        #[arg(long, default_value = "interact")]
        mode: String,
        #[arg(long)]
        window: Option<String>,
        #[arg(long)]
        role: Option<String>,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        depth: Option<i64>,
        #[arg(long, value_name = "MENU")]
        open_menu: Option<String>,
        #[arg(long, help = "Emit JSON envelope on stdout")]
        json: bool,
    },
    /// Inspect a single element
    #[command(aliases = ["show"], after_long_help = AGENT_INSPECT_AFTER_HELP)]
    Inspect(AgentElementArgs),
    /// Wait for an element to become available
    Wait(AgentElementArgs),
    /// Press an element by role and label
    #[command(after_long_help = AGENT_PRESS_AFTER_HELP)]
    Press(AgentPressArgs),
    /// Set the value of an element
    SetValue(AgentSetValueArgs),
    /// Focus an element
    Focus(AgentElementArgs),
    /// Show a menu (open menu, then snapshot)
    ShowMenu {
        #[command(flatten)]
        conn: ConnectionOptions,
        #[arg(long)]
        menu: String,
    },
    /// Focus a window
    WindowFocus(AgentWindowArgs),
    /// Resize a window
    WindowResize(AgentWindowResizeArgs),
    /// Move a window
    WindowMove(AgentWindowMoveArgs),
    /// Close a window
    WindowClose(AgentWindowArgs),
    /// Minimize a window
    WindowMinimize(AgentWindowArgs),
}

#[derive(Subcommand, Debug)]
enum VmAction {
    /// Start a clone of a golden image
    Start {
        #[arg(long, default_value = "macos")]
        platform: String,
        #[arg(long)]
        display: Option<String>,
        #[arg(long)]
        viewer: bool,
    },
    /// Stop a running VM by id
    Stop { id: String },
    /// List running clones and golden images
    #[command(aliases = ["ls"])]
    List,
    /// Delete a golden image by name
    #[command(aliases = ["rm", "remove"])]
    Delete {
        name: String,
        #[arg(long)]
        force: bool,
    },
}

fn unimplemented(name: &str) -> ! {
    eprintln!("testanyware {name}: not yet implemented in the Rust port");
    eprintln!("This subcommand is still served by the Swift CLI under cli/.");
    std::process::exit(2);
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli.command {
        Command::Screen { action } => match action {
            ScreenAction::Capture(args) => run_screen_capture(args).await,
            ScreenAction::Record(_) => unimplemented("screen record"),
            ScreenAction::Size(args) => run_screen_size(args).await,
            ScreenAction::FindText(_) => unimplemented("screen find-text"),
        },
        Command::File { action } => match action {
            FileAction::Upload(args) => run_upload(args).await,
            FileAction::Download(args) => run_download(args).await,
            FileAction::Exec(args) => run_exec(args).await,
        },
        Command::Input { action } => match action {
            InputAction::Key {
                conn,
                key,
                modifiers,
                json,
            } => {
                input_cmds::run_key(conn.into(), key, modifiers, OutputMode::from_flags(json))
                    .await
            }
            InputAction::KeyDown { conn, key, json } => {
                input_cmds::run_key_down(conn.into(), key, OutputMode::from_flags(json)).await
            }
            InputAction::KeyUp { conn, key, json } => {
                input_cmds::run_key_up(conn.into(), key, OutputMode::from_flags(json)).await
            }
            InputAction::Type { conn, text, json } => {
                input_cmds::run_type(conn.into(), text, OutputMode::from_flags(json)).await
            }
            InputAction::Click {
                conn,
                x,
                y,
                button,
                count,
                json,
            } => {
                input_cmds::run_click(
                    conn.into(),
                    x,
                    y,
                    button,
                    count,
                    OutputMode::from_flags(json),
                )
                .await
            }
            InputAction::MouseDown {
                conn,
                x,
                y,
                button,
                json,
            } => {
                input_cmds::run_mouse_down(
                    conn.into(),
                    x,
                    y,
                    button,
                    OutputMode::from_flags(json),
                )
                .await
            }
            InputAction::MouseUp {
                conn,
                x,
                y,
                button,
                json,
            } => {
                input_cmds::run_mouse_up(
                    conn.into(),
                    x,
                    y,
                    button,
                    OutputMode::from_flags(json),
                )
                .await
            }
            InputAction::Move { conn, x, y, json } => {
                input_cmds::run_move(conn.into(), x, y, OutputMode::from_flags(json)).await
            }
            InputAction::Scroll {
                conn,
                x,
                y,
                dx,
                dy,
                json,
            } => {
                input_cmds::run_scroll(
                    conn.into(),
                    x,
                    y,
                    dx.unwrap_or(0),
                    dy.unwrap_or(0),
                    OutputMode::from_flags(json),
                )
                .await
            }
            InputAction::Drag {
                conn,
                from_x,
                from_y,
                to_x,
                to_y,
                button,
                steps,
                json,
            } => {
                input_cmds::run_drag(
                    conn.into(),
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                    button,
                    steps,
                    OutputMode::from_flags(json),
                )
                .await
            }
        },
        Command::Agent { action } => match action {
            AgentAction::Health { conn, json } => {
                agent_cmds::run_health(conn.into(), OutputMode::from_flags(json)).await
            }
            AgentAction::Windows { conn, json } => {
                agent_cmds::run_windows(conn.into(), OutputMode::from_flags(json)).await
            }
            AgentAction::Snapshot {
                conn,
                mode,
                window,
                role,
                label,
                depth,
                open_menu,
                json,
            } => {
                agent_cmds::run_snapshot(
                    conn.into(),
                    agent_cmds::SnapshotArgs {
                        mode_arg: Some(mode),
                        window,
                        role,
                        label,
                        depth,
                        open_menu,
                    },
                    OutputMode::from_flags(json),
                )
                .await
            }
            AgentAction::Inspect(args) => {
                agent_cmds::run_inspect(
                    args.conn.clone().into(),
                    element_args_to_query(args.clone()),
                    OutputMode::from_flags(args.json),
                )
                .await
            }
            AgentAction::Wait(_) => unimplemented("agent wait"),
            AgentAction::Press(args) => {
                let mode = OutputMode::from_flags(args.common.json);
                let dry_run = args.dry_run;
                agent_cmds::run_press(
                    args.common.conn.clone().into(),
                    element_args_to_query(args.common),
                    mode,
                    dry_run,
                )
                .await
            }
            AgentAction::SetValue(_) => unimplemented("agent set-value"),
            AgentAction::Focus(_) => unimplemented("agent focus"),
            AgentAction::ShowMenu { .. } => unimplemented("agent show-menu"),
            AgentAction::WindowFocus(_) => unimplemented("agent window-focus"),
            AgentAction::WindowResize(_) => unimplemented("agent window-resize"),
            AgentAction::WindowMove(_) => unimplemented("agent window-move"),
            AgentAction::WindowClose(_) => unimplemented("agent window-close"),
            AgentAction::WindowMinimize(_) => unimplemented("agent window-minimize"),
        },
        Command::Vm { action } => match action {
            VmAction::Start { .. } => unimplemented("vm start"),
            VmAction::Stop { .. } => unimplemented("vm stop"),
            VmAction::List => unimplemented("vm list"),
            VmAction::Delete { .. } => unimplemented("vm delete"),
        },
        Command::Doctor => unimplemented("doctor"),
        Command::Capabilities { json: _ } => run_capabilities(),
        Command::Schema { command } => run_schema(&command),
        Command::LlmInstructions => run_llm_instructions(),
        Command::Server { .. } => unimplemented("server"),
        // Verb-first aliases dispatch to the same handler as the canonical.
        Command::Screenshot(args) => run_screen_capture(args).await,
        Command::Record(_) => unimplemented("screen record"),
        Command::ScreenSize(args) => run_screen_size(args).await,
        Command::FindText(_) => unimplemented("screen find-text"),
        Command::Upload(args) => run_upload(args).await,
        Command::Download(args) => run_download(args).await,
        Command::Exec(args) => run_exec(args).await,
    }
}

fn element_args_to_query(args: AgentElementArgs) -> agent_cmds::ElementQueryArgs {
    agent_cmds::ElementQueryArgs {
        role: args.role,
        label: args.label,
        window: args.window,
        id: args.id,
        index: args.index,
    }
}

async fn run_screen_size(args: ConnectionArgs) {
    let mode = OutputMode::from_flags(args.json);
    screen_cmds::run_screen_size(args.conn.into(), mode).await
}

async fn run_screen_capture(args: ScreenCaptureArgs) {
    let mode = OutputMode::from_flags(args.json);
    screen_cmds::run_screen_capture(args.conn.into(), args.output, args.region, mode).await
}

async fn run_upload(args: FileUploadArgs) {
    let mode = OutputMode::from_flags(args.json);
    file_cmds::run_upload(
        args.conn.into(),
        args.local,
        args.remote,
        mode,
        args.dry_run,
    )
    .await
}

async fn run_download(args: FileDownloadArgs) {
    let mode = OutputMode::from_flags(args.json);
    file_cmds::run_download(
        args.conn.into(),
        args.remote,
        args.local,
        mode,
        args.dry_run,
    )
    .await
}

async fn run_exec(args: FileExecArgs) {
    let mode = OutputMode::from_flags(args.json);
    file_cmds::run_exec(
        args.conn.into(),
        file_cmds::ExecArgs {
            command: args.command,
            timeout: args.timeout,
            detach: args.detach,
        },
        mode,
        args.dry_run,
    )
    .await
}
