//! `testanyware` CLI binary (Rust port).
//!
//! Surface follows the noun-first canonical layout from
//! `docs/architecture/cli-design-contract.md` §1, with the curated
//! verb-first aliases from §1's alias table announcing themselves per
//! §7.2. All subcommands are stubs that print `not yet implemented` to
//! stderr and exit with status 2 when invoked. Per-command behaviour is
//! added by the per-feature port tasks tracked in `LLM_STATE/core/`.

use clap::{Args, Parser, Subcommand};
use testanyware_cli::discoverability::{run_capabilities, run_llm_instructions, run_schema};

// §7-template "after-help" blocks for the three §8 discoverability
// commands. Clap renders these after the auto-generated USAGE/OPTIONS,
// completing the OUTPUT/EXIT CODES/EXAMPLES/SEE ALSO sections.

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
    //
    // The doc comment on each variant is rendered into the alias's
    // `--help` per §7.2 so the alias announces itself rather than
    // re-documenting the canonical command.

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

#[derive(Args, Debug)]
struct ConnectionArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
}

#[derive(Args, Debug)]
struct ScreenCaptureArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    #[arg(short = 'o', value_name = "FILE")]
    output: Option<String>,
    #[arg(long, value_name = "X,Y,W,H")]
    region: Option<String>,
}

#[derive(Args, Debug)]
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

#[derive(Args, Debug)]
struct ScreenFindTextArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    text: Option<String>,
    #[arg(long)]
    timeout: Option<u32>,
}

#[derive(Args, Debug)]
struct FileUploadArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    local: String,
    remote: String,
}

#[derive(Args, Debug)]
struct FileDownloadArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    remote: String,
    local: String,
}

#[derive(Args, Debug)]
struct FileExecArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    command: String,
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
    Upload(FileUploadArgs),
    /// Download a file from guest to host
    Download(FileDownloadArgs),
    /// Run a command in the guest, capture stdout/stderr/exit
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
    },
    /// Press a key (no release)
    KeyDown {
        #[command(flatten)]
        conn: ConnectionOptions,
        key: String,
    },
    /// Release a key
    KeyUp {
        #[command(flatten)]
        conn: ConnectionOptions,
        key: String,
    },
    /// Type a string
    Type {
        #[command(flatten)]
        conn: ConnectionOptions,
        text: String,
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
    },
    /// Press a mouse button (no release)
    MouseDown {
        #[command(flatten)]
        conn: ConnectionOptions,
        x: i32,
        y: i32,
        #[arg(long, default_value = "left")]
        button: String,
    },
    /// Release a mouse button
    MouseUp {
        #[command(flatten)]
        conn: ConnectionOptions,
        x: i32,
        y: i32,
        #[arg(long, default_value = "left")]
        button: String,
    },
    /// Move the mouse cursor
    Move {
        #[command(flatten)]
        conn: ConnectionOptions,
        x: i32,
        y: i32,
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
    },
    /// Drag from one point to another
    Drag {
        #[command(flatten)]
        conn: ConnectionOptions,
        from_x: i32,
        from_y: i32,
        to_x: i32,
        to_y: i32,
    },
}

#[derive(Args, Debug)]
struct AgentElementArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    #[arg(long)]
    role: String,
    #[arg(long)]
    label: Option<String>,
}

#[derive(Args, Debug)]
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

#[derive(Args, Debug)]
struct AgentWindowArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    #[arg(long)]
    window: String,
}

#[derive(Args, Debug)]
struct AgentWindowMoveArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    #[arg(long)]
    window: String,
    x: i32,
    y: i32,
}

#[derive(Args, Debug)]
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
    Health {
        #[command(flatten)]
        conn: ConnectionOptions,
    },
    /// List visible windows
    Windows {
        #[command(flatten)]
        conn: ConnectionOptions,
    },
    /// Snapshot the accessibility tree
    Snapshot {
        #[command(flatten)]
        conn: ConnectionOptions,
        #[arg(long, default_value = "interact")]
        mode: String,
        #[arg(long)]
        window: Option<String>,
        #[arg(long, value_name = "MENU")]
        open_menu: Option<String>,
    },
    /// Inspect a single element
    #[command(aliases = ["show"])]
    Inspect(AgentElementArgs),
    /// Wait for an element to become available
    Wait(AgentElementArgs),
    /// Press an element by role and label
    Press(AgentElementArgs),
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
            ScreenAction::Capture(_) => unimplemented("screen capture"),
            ScreenAction::Record(_) => unimplemented("screen record"),
            ScreenAction::Size(_) => unimplemented("screen size"),
            ScreenAction::FindText(_) => unimplemented("screen find-text"),
        },
        Command::File { action } => match action {
            FileAction::Upload(_) => unimplemented("file upload"),
            FileAction::Download(_) => unimplemented("file download"),
            FileAction::Exec(_) => unimplemented("file exec"),
        },
        Command::Input { action } => match action {
            InputAction::Key { .. } => unimplemented("input key"),
            InputAction::KeyDown { .. } => unimplemented("input key-down"),
            InputAction::KeyUp { .. } => unimplemented("input key-up"),
            InputAction::Type { .. } => unimplemented("input type"),
            InputAction::Click { .. } => unimplemented("input click"),
            InputAction::MouseDown { .. } => unimplemented("input mouse-down"),
            InputAction::MouseUp { .. } => unimplemented("input mouse-up"),
            InputAction::Move { .. } => unimplemented("input move"),
            InputAction::Scroll { .. } => unimplemented("input scroll"),
            InputAction::Drag { .. } => unimplemented("input drag"),
        },
        Command::Agent { action } => match action {
            AgentAction::Health { .. } => unimplemented("agent health"),
            AgentAction::Windows { .. } => unimplemented("agent windows"),
            AgentAction::Snapshot { .. } => unimplemented("agent snapshot"),
            AgentAction::Inspect(_) => unimplemented("agent inspect"),
            AgentAction::Wait(_) => unimplemented("agent wait"),
            AgentAction::Press(_) => unimplemented("agent press"),
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
        Command::Screenshot(_) => unimplemented("screen capture"),
        Command::Record(_) => unimplemented("screen record"),
        Command::ScreenSize(_) => unimplemented("screen size"),
        Command::FindText(_) => unimplemented("screen find-text"),
        Command::Upload(_) => unimplemented("file upload"),
        Command::Download(_) => unimplemented("file download"),
        Command::Exec(_) => unimplemented("file exec"),
    }
}
