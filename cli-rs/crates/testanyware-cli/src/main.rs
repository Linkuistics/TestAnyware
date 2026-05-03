//! `testanyware` CLI binary (Rust port).
//!
//! All subcommands are stubs that print `not yet implemented` to stderr
//! and exit with status 2 when invoked. The bootstrap task that created
//! this binary only requires `testanyware --help` to list the same
//! top-level surface as the existing Swift CLI under `cli/`. Per-command
//! behaviour is added by the per-feature port tasks tracked in
//! `LLM_STATE/core/`.

use clap::{Args, Parser, Subcommand};

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
    /// Capture a screenshot via VNC
    Screenshot {
        #[command(flatten)]
        conn: ConnectionOptions,
        #[arg(short = 'o', value_name = "FILE")]
        output: Option<String>,
        #[arg(long, value_name = "X,Y,W,H")]
        region: Option<String>,
    },

    /// Print VNC display dimensions ("WxH")
    ScreenSize {
        #[command(flatten)]
        conn: ConnectionOptions,
    },

    /// Send keyboard or mouse input via VNC
    Input {
        #[command(subcommand)]
        action: InputAction,
    },

    /// Run a command in the guest via the agent
    Exec {
        #[command(flatten)]
        conn: ConnectionOptions,
        command: String,
    },

    /// Upload a file from host to guest
    Upload {
        #[command(flatten)]
        conn: ConnectionOptions,
        local: String,
        remote: String,
    },

    /// Download a file from guest to host
    Download {
        #[command(flatten)]
        conn: ConnectionOptions,
        remote: String,
        local: String,
    },

    /// Record VNC framebuffer to MP4
    Record {
        #[command(flatten)]
        conn: ConnectionOptions,
        #[arg(short = 'o', value_name = "FILE")]
        output: String,
        #[arg(long)]
        fps: Option<u32>,
        #[arg(long)]
        duration: Option<u32>,
    },

    /// OCR the screen and find text
    FindText {
        #[command(flatten)]
        conn: ConnectionOptions,
        text: Option<String>,
        #[arg(long)]
        timeout: Option<u32>,
    },

    /// Run as an agent server (development helper)
    Server {
        #[arg(long, default_value_t = 8648)]
        port: u16,
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
    Inspect {
        #[command(flatten)]
        conn: ConnectionOptions,
        #[arg(long)]
        role: String,
        #[arg(long)]
        label: Option<String>,
    },
    /// Press an element by role and label
    Press {
        #[command(flatten)]
        conn: ConnectionOptions,
        #[arg(long)]
        role: String,
        #[arg(long)]
        label: Option<String>,
    },
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
    List,
    /// Delete a golden image by name
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
        Command::Screenshot { .. } => unimplemented("screenshot"),
        Command::ScreenSize { .. } => unimplemented("screen-size"),
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
        Command::Exec { .. } => unimplemented("exec"),
        Command::Upload { .. } => unimplemented("upload"),
        Command::Download { .. } => unimplemented("download"),
        Command::Record { .. } => unimplemented("record"),
        Command::FindText { .. } => unimplemented("find-text"),
        Command::Server { .. } => unimplemented("server"),
        Command::Agent { action } => match action {
            AgentAction::Health { .. } => unimplemented("agent health"),
            AgentAction::Windows { .. } => unimplemented("agent windows"),
            AgentAction::Snapshot { .. } => unimplemented("agent snapshot"),
            AgentAction::Inspect { .. } => unimplemented("agent inspect"),
            AgentAction::Press { .. } => unimplemented("agent press"),
        },
        Command::Vm { action } => match action {
            VmAction::Start { .. } => unimplemented("vm start"),
            VmAction::Stop { .. } => unimplemented("vm stop"),
            VmAction::List => unimplemented("vm list"),
            VmAction::Delete { .. } => unimplemented("vm delete"),
        },
        Command::Doctor => unimplemented("doctor"),
    }
}
