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
    agent as agent_cmds, doctor as doctor_cmds, file as file_cmds, input as input_cmds,
    screen as screen_cmds, vm as vm_cmds,
};
use testanyware_cli::discoverability::{run_capabilities, run_llm_instructions, run_schema};
use testanyware_cli::output::OutputMode;
use testanyware_cli::resolve::ConnectionOptions as ResolveOptions;

// §7-template "after-help" blocks.

const WINDOW_FLAG_HELP: &str = "Window name for relative coordinates. \
    The macOS Tahoe drop-shadow inset (~40 px in y) is compensated automatically; \
    override the inset via `TESTANYWARE_WINDOW_TOP_INSET=<int>` if your macOS version differs.";

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

const AGENT_SET_VALUE_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (`OK` / `FAILED: <message>`), --json (schema:
    agent-action).

EXIT CODES:
    0  success
    3  ELEMENT_NOT_FOUND / WINDOW_NOT_FOUND
    5  ELEMENT_AMBIGUOUS / ACTION_UNSUPPORTED

IDEMPOTENCY:
    Not idempotent — setting twice replaces the value twice. Retry only when
    the previous attempt's outcome is unknown.

EXAMPLES:
    # Set the value of a text field
    testanyware agent set-value --vm \"$TESTANYWARE_VM_ID\" --role textfield --label \"Email\" --value me@example.com

    # Plan only — emit the request without performing it
    testanyware agent set-value --vm \"$TESTANYWARE_VM_ID\" --role textfield --label \"Email\" --value x --dry-run --json

SEE ALSO:
    testanyware agent inspect, testanyware agent focus
";

const AGENT_FOCUS_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (`OK` / `FAILED: <message>`), --json (schema:
    agent-action).

EXIT CODES:
    0  success
    3  ELEMENT_NOT_FOUND / WINDOW_NOT_FOUND
    5  ELEMENT_AMBIGUOUS / ACTION_UNSUPPORTED

IDEMPOTENCY:
    Idempotent in the focused state. Retry-safe.

EXAMPLES:
    # Focus a text field before typing into it
    testanyware agent focus --vm \"$TESTANYWARE_VM_ID\" --role textfield --label \"Search\"

    # Plan only
    testanyware agent focus --vm \"$TESTANYWARE_VM_ID\" --role textfield --label \"Search\" --dry-run --json

SEE ALSO:
    testanyware agent set-value, testanyware input type
";

const AGENT_WAIT_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (`OK` / `FAILED: <message>`), --json (schema:
    agent-action). Read-only: no --dry-run.

EXIT CODES:
    0  success (accessibility became ready)
    3  WINDOW_NOT_FOUND
    4  AUTH_REQUIRED
    7  CONNECTION_TIMEOUT

EXAMPLES:
    # Wait for the agent's accessibility tree to be ready
    testanyware agent wait --vm \"$TESTANYWARE_VM_ID\"

    # Wait, scoped to a window, with an explicit timeout
    testanyware agent wait --vm \"$TESTANYWARE_VM_ID\" --window \"Settings\" --timeout 30 --json

SEE ALSO:
    testanyware agent health, testanyware agent windows
";

const AGENT_WINDOW_FOCUS_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (`OK` / `FAILED: <message>`), --json (schema:
    agent-window-action).

EXIT CODES:
    0  success
    3  WINDOW_NOT_FOUND
    4  AUTH_REQUIRED

IDEMPOTENCY:
    Idempotent in the focused state. Retry-safe.

EXAMPLES:
    # Bring a window to the front
    testanyware agent window-focus --vm \"$TESTANYWARE_VM_ID\" --window \"Settings\"

    # Plan only
    testanyware agent window-focus --vm \"$TESTANYWARE_VM_ID\" --window \"Settings\" --dry-run --json

SEE ALSO:
    testanyware agent windows, testanyware agent window-minimize
";

const AGENT_WINDOW_RESIZE_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (`OK` / `FAILED: <message>`), --json (schema:
    agent-window-action).

EXIT CODES:
    0  success
    3  WINDOW_NOT_FOUND
    4  AUTH_REQUIRED

IDEMPOTENCY:
    Idempotent in the target size. Retry-safe.

EXAMPLES:
    # Resize a window to 1280x800
    testanyware agent window-resize --vm \"$TESTANYWARE_VM_ID\" --window \"Settings\" --width 1280 --height 800

    # Plan only
    testanyware agent window-resize --vm \"$TESTANYWARE_VM_ID\" --window \"Settings\" --width 1280 --height 800 --dry-run --json

SEE ALSO:
    testanyware agent window-move, testanyware agent windows
";

const AGENT_WINDOW_MOVE_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (`OK` / `FAILED: <message>`), --json (schema:
    agent-window-action).

EXIT CODES:
    0  success
    3  WINDOW_NOT_FOUND
    4  AUTH_REQUIRED

IDEMPOTENCY:
    Idempotent in the target position. Retry-safe.

EXAMPLES:
    # Move a window to (100, 80)
    testanyware agent window-move --vm \"$TESTANYWARE_VM_ID\" --window \"Settings\" --x 100 --y 80

    # Plan only
    testanyware agent window-move --vm \"$TESTANYWARE_VM_ID\" --window \"Settings\" --x 100 --y 80 --dry-run --json

SEE ALSO:
    testanyware agent window-resize, testanyware agent windows
";

const AGENT_WINDOW_CLOSE_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (`OK` / `FAILED: <message>`), --json (schema:
    agent-window-action).

EXIT CODES:
    0  success
    3  WINDOW_NOT_FOUND
    4  AUTH_REQUIRED

IDEMPOTENCY:
    Idempotent in the closed state. Retry-safe.

EXAMPLES:
    # Close a window
    testanyware agent window-close --vm \"$TESTANYWARE_VM_ID\" --window \"Settings\"

    # Plan only
    testanyware agent window-close --vm \"$TESTANYWARE_VM_ID\" --window \"Settings\" --dry-run --json

SEE ALSO:
    testanyware agent window-minimize, testanyware agent windows
";

const AGENT_WINDOW_MINIMIZE_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (`OK` / `FAILED: <message>`), --json (schema:
    agent-window-action).

EXIT CODES:
    0  success
    3  WINDOW_NOT_FOUND
    4  AUTH_REQUIRED

IDEMPOTENCY:
    Idempotent in the minimized state. Retry-safe.

EXAMPLES:
    # Minimize a window
    testanyware agent window-minimize --vm \"$TESTANYWARE_VM_ID\" --window \"Settings\"

    # Plan only
    testanyware agent window-minimize --vm \"$TESTANYWARE_VM_ID\" --window \"Settings\" --dry-run --json

SEE ALSO:
    testanyware agent window-focus, testanyware agent windows
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

const VM_START_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (the VM id on one line), --json (schema: vm-start).

EXIT CODES:
    0  success
    1  QEMU_FAILED, SWTPM_MISSING, VM_BACKEND_UNSUPPORTED, SPAWN_FAILED
    2  USAGE_ERROR / INVALID_PLATFORM
    3  GOLDEN_NOT_FOUND / UEFI_NOT_FOUND
    4  KVM_PERMISSION_DENIED

IDEMPOTENCY:
    Not idempotent — each call without --id clones a fresh VM. Retry-safe:
    a failed start tears its own partial clone down.

EXAMPLES:
    # Start a Windows guest at 1080p
    testanyware vm start --platform windows --display 1920x1080

    # Start a Linux guest, capture the id as JSON
    testanyware vm start --platform linux --json

    # Plan a start without performing it
    testanyware vm start --platform windows --dry-run

SEE ALSO:
    testanyware vm stop, testanyware vm list, testanyware doctor
";

const VM_STOP_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (`stopped <id>`), --json (schema: vm-stop).

EXIT CODES:
    0  success
    1  VM_STOP_FAILED, VM_BACKEND_UNSUPPORTED
    2  USAGE_ERROR (no id and TESTANYWARE_VM_ID unset)
    3  VM_NOT_FOUND

IDEMPOTENCY:
    Idempotent — stopping an already-stopped VM is a VM_NOT_FOUND, not a
    crash. Retry-safe.

EXAMPLES:
    # Stop a VM by id
    testanyware vm stop testanyware-deadbeef

    # Stop the VM named by $TESTANYWARE_VM_ID
    testanyware vm stop

    # Plan the stop as JSON
    testanyware vm stop testanyware-deadbeef --dry-run --json

SEE ALSO:
    testanyware vm start, testanyware vm list
";

const VM_LIST_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: --json (schema: vm-list; envelope carries
    returned/total/truncated per contract §3.5). Text is a two-section
    summary and is not a parsing target.

EXIT CODES:
    0  success

EXAMPLES:
    # List golden images and running clones
    testanyware vm list

    # JSON for scripting, unbounded
    testanyware vm list --json --all

    # Only Windows entries
    testanyware vm list --filter platform=windows

SEE ALSO:
    testanyware vm start, testanyware vm delete
";

const VM_DELETE_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (`deleted <name>`), --json (schema: vm-delete).

EXIT CODES:
    0  success
    1  generic failure
    3  GOLDEN_NOT_FOUND
    5  GOLDEN_IN_USE (running clones depend on the image; use --force)

IDEMPOTENCY:
    Idempotent in the deleted state. --force overrides the running-clones
    safety check.

EXAMPLES:
    # Delete a golden image
    testanyware vm delete testanyware-golden-linux-24.04

    # Delete even though clones depend on it
    testanyware vm delete testanyware-golden-windows-11 --force

    # Plan the delete as JSON
    testanyware vm delete testanyware-golden-linux-24.04 --dry-run --json

SEE ALSO:
    testanyware vm list, testanyware vm start
";

const DOCTOR_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: --json (schema: doctor). Text output is a readable
    per-check report (pass ✓ / warn ! / fail ✗) and is not a parsing target.

EXIT CODES:
    0  healthy — every blocking check passed
    1  unhealthy — a blocking check failed (install path, bundled agents,
       or bundled scripts). Host-tool and script-floor checks are advisory
       and never flip the exit code.

EXAMPLES:
    # Human-readable diagnosis of the local install
    testanyware doctor

    # Machine-readable report for scripting / CI gating
    testanyware doctor --json | jq '.ok'

    # Show only the checks that did not pass
    testanyware doctor --json | jq '.checks | map_values(select(.status != \"pass\"))'

SEE ALSO:
    testanyware capabilities, testanyware vm list, testanyware --help
";

// Root-level help banners — make the LLM usage guide impossible to miss
// for an agent that runs bare `testanyware` or `testanyware --help`.
const ROOT_BEFORE_HELP: &str =
    ">> AI AGENTS: run `testanyware llm-instructions` for the full LLM usage guide. <<";

const ROOT_AFTER_HELP: &str = "\
AI AGENTS / LLMs:
    Run `testanyware llm-instructions` first. It prints a complete usage
    guide written for you — mental model, command reference, end-to-end
    workflows, JSON output, exit codes, and common mistakes — runnable
    with only this binary.
";

#[derive(Parser, Debug)]
#[command(
    name = "testanyware",
    about = "VNC + agent driver for virtual machine automation",
    version,
    propagate_version = true,
    arg_required_else_help = true,
    before_help = ROOT_BEFORE_HELP,
    after_help = ROOT_AFTER_HELP,
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
    #[command(after_long_help = DOCTOR_AFTER_HELP)]
    Doctor {
        /// Emit JSON envelope on stdout.
        #[arg(long)]
        json: bool,
    },

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

    /// Print the full LLM usage guide for this tool (LLM agents: read this first)
    #[command(
        long_about = "Print the full LLM usage guide for this tool.\n\n\
                      Emits LLM_INSTRUCTIONS.md as plain text on stdout — the \
                      complete reference for driving TestAnyware: the noun-first \
                      command tree and verb-first aliases, the connection \
                      resolution chain, the agent/input/screen/file command \
                      reference, end-to-end workflows, JSON output and exit \
                      codes, and common mistakes. Embedded in the binary at \
                      build time, so it is runnable with only the installed \
                      CLI — an LLM agent can read it or prepend it as context.",
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
        #[arg(long, help = WINDOW_FLAG_HELP)]
        window: Option<String>,
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
        #[arg(long, help = WINDOW_FLAG_HELP)]
        window: Option<String>,
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
        #[arg(long, help = WINDOW_FLAG_HELP)]
        window: Option<String>,
        #[arg(long, help = "Emit JSON envelope on stdout")]
        json: bool,
    },
    /// Move the mouse cursor
    Move {
        #[command(flatten)]
        conn: ConnectionOptions,
        x: i32,
        y: i32,
        #[arg(long, help = WINDOW_FLAG_HELP)]
        window: Option<String>,
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
        #[arg(long, help = WINDOW_FLAG_HELP)]
        window: Option<String>,
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
        #[arg(long, help = WINDOW_FLAG_HELP)]
        window: Option<String>,
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

/// Element-targeting action with `--dry-run` (shared by `press` and `focus`).
#[derive(Args, Debug, Clone)]
struct AgentActionArgs {
    #[command(flatten)]
    common: AgentElementArgs,
    #[arg(long, help = "Plan the action but do not perform it")]
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
    window: Option<String>,
    #[arg(long)]
    id: Option<String>,
    #[arg(long)]
    index: Option<i64>,
    #[arg(long)]
    value: String,
    #[arg(long, help = "Emit JSON envelope on stdout")]
    json: bool,
    #[arg(long, help = "Plan the action but do not perform it")]
    dry_run: bool,
}

#[derive(Args, Debug, Clone)]
struct AgentWaitArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    #[arg(long)]
    window: Option<String>,
    #[arg(long)]
    timeout: Option<i64>,
    #[arg(long, help = "Emit JSON envelope on stdout")]
    json: bool,
}

#[derive(Args, Debug, Clone)]
struct AgentWindowArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    #[arg(long)]
    window: String,
    #[arg(long, help = "Emit JSON envelope on stdout")]
    json: bool,
    #[arg(long, help = "Plan the action but do not perform it")]
    dry_run: bool,
}

#[derive(Args, Debug, Clone)]
struct AgentWindowMoveArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    #[arg(long)]
    window: String,
    #[arg(long)]
    x: i64,
    #[arg(long)]
    y: i64,
    #[arg(long, help = "Emit JSON envelope on stdout")]
    json: bool,
    #[arg(long, help = "Plan the action but do not perform it")]
    dry_run: bool,
}

#[derive(Args, Debug, Clone)]
struct AgentWindowResizeArgs {
    #[command(flatten)]
    conn: ConnectionOptions,
    #[arg(long)]
    window: String,
    #[arg(long)]
    width: i64,
    #[arg(long)]
    height: i64,
    #[arg(long, help = "Emit JSON envelope on stdout")]
    json: bool,
    #[arg(long, help = "Plan the action but do not perform it")]
    dry_run: bool,
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
    /// Wait for the agent's accessibility tree to be ready
    #[command(after_long_help = AGENT_WAIT_AFTER_HELP)]
    Wait(AgentWaitArgs),
    /// Press an element by role and label
    #[command(after_long_help = AGENT_PRESS_AFTER_HELP)]
    Press(AgentActionArgs),
    /// Set the value of an element
    #[command(after_long_help = AGENT_SET_VALUE_AFTER_HELP)]
    SetValue(AgentSetValueArgs),
    /// Focus an element
    #[command(after_long_help = AGENT_FOCUS_AFTER_HELP)]
    Focus(AgentActionArgs),
    /// Show a menu (open menu, then snapshot)
    ShowMenu {
        #[command(flatten)]
        conn: ConnectionOptions,
        #[arg(long)]
        menu: String,
    },
    /// Focus a window
    #[command(after_long_help = AGENT_WINDOW_FOCUS_AFTER_HELP)]
    WindowFocus(AgentWindowArgs),
    /// Resize a window
    #[command(after_long_help = AGENT_WINDOW_RESIZE_AFTER_HELP)]
    WindowResize(AgentWindowResizeArgs),
    /// Move a window
    #[command(after_long_help = AGENT_WINDOW_MOVE_AFTER_HELP)]
    WindowMove(AgentWindowMoveArgs),
    /// Close a window
    #[command(after_long_help = AGENT_WINDOW_CLOSE_AFTER_HELP)]
    WindowClose(AgentWindowArgs),
    /// Minimize a window
    #[command(after_long_help = AGENT_WINDOW_MINIMIZE_AFTER_HELP)]
    WindowMinimize(AgentWindowArgs),
}

#[derive(Subcommand, Debug)]
enum VmAction {
    /// Start a clone of a golden image
    #[command(after_long_help = VM_START_AFTER_HELP)]
    Start {
        /// Target platform: linux or windows. (macos uses the tart
        /// backend, which is not yet ported to the Rust CLI.)
        #[arg(long, value_name = "PLATFORM")]
        platform: String,
        /// Golden image to clone [default: the platform's golden].
        #[arg(long, value_name = "NAME")]
        base: Option<String>,
        /// VM instance id [default: testanyware-<hex8>].
        #[arg(long, value_name = "ID")]
        id: Option<String>,
        /// Display resolution, e.g. 1920x1080.
        #[arg(long, value_name = "WxH")]
        display: Option<String>,
        /// Open a VNC viewer after boot (not yet ported — backlog task 8).
        #[arg(long)]
        viewer: bool,
        /// Emit JSON envelope on stdout.
        #[arg(long)]
        json: bool,
        /// Plan the start but do not perform it.
        #[arg(long)]
        dry_run: bool,
    },
    /// Stop a running VM by id
    #[command(after_long_help = VM_STOP_AFTER_HELP)]
    Stop {
        /// VM instance id (falls back to TESTANYWARE_VM_ID).
        #[arg(value_name = "ID", env = "TESTANYWARE_VM_ID")]
        id: Option<String>,
        /// Emit JSON envelope on stdout.
        #[arg(long)]
        json: bool,
        /// Plan the stop but do not perform it.
        #[arg(long)]
        dry_run: bool,
    },
    /// List running clones and golden images
    #[command(aliases = ["ls"], after_long_help = VM_LIST_AFTER_HELP)]
    List {
        /// Emit JSON envelope on stdout.
        #[arg(long)]
        json: bool,
        /// Maximum rows to show.
        #[arg(long, value_name = "N", default_value_t = 100)]
        limit: usize,
        /// Show all rows (overrides --limit).
        #[arg(long, conflicts_with = "limit")]
        all: bool,
        /// Comma-separated field=value filters (fields: kind, platform,
        /// backend, name).
        #[arg(long, value_name = "EXPR")]
        filter: Option<String>,
    },
    /// Delete a golden image by name
    #[command(aliases = ["rm", "remove"], after_long_help = VM_DELETE_AFTER_HELP)]
    Delete {
        /// Golden image name (run `testanyware vm list` to see images).
        name: String,
        /// Delete even if running clones appear to depend on the image.
        #[arg(long)]
        force: bool,
        /// Emit JSON envelope on stdout.
        #[arg(long)]
        json: bool,
        /// Plan the delete but do not perform it.
        #[arg(long)]
        dry_run: bool,
    },
}

fn unimplemented(name: &str) -> ! {
    eprintln!("testanyware {name}: not yet implemented in the Rust port");
    eprintln!("This subcommand is still served by the Swift CLI under cli/.");
    std::process::exit(2);
}

#[tokio::main]
async fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            // `--help` / `--version` arrive here as "errors" too; print
            // those untouched. Genuine usage errors get an LLM pointer so
            // an agent that mis-invokes the CLI is steered to the guide.
            let is_help_or_version = matches!(
                err.kind(),
                clap::error::ErrorKind::DisplayHelp
                    | clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
                    | clap::error::ErrorKind::DisplayVersion
            );
            if is_help_or_version {
                err.exit();
            }
            let _ = err.print();
            eprintln!(
                "\nLLM agents: run `testanyware llm-instructions` for the full usage guide."
            );
            std::process::exit(err.exit_code());
        }
    };
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
                window,
                json,
            } => {
                input_cmds::run_click(
                    conn.into(),
                    x,
                    y,
                    button,
                    count,
                    window,
                    OutputMode::from_flags(json),
                )
                .await
            }
            InputAction::MouseDown {
                conn,
                x,
                y,
                button,
                window,
                json,
            } => {
                input_cmds::run_mouse_down(
                    conn.into(),
                    x,
                    y,
                    button,
                    window,
                    OutputMode::from_flags(json),
                )
                .await
            }
            InputAction::MouseUp {
                conn,
                x,
                y,
                button,
                window,
                json,
            } => {
                input_cmds::run_mouse_up(
                    conn.into(),
                    x,
                    y,
                    button,
                    window,
                    OutputMode::from_flags(json),
                )
                .await
            }
            InputAction::Move {
                conn,
                x,
                y,
                window,
                json,
            } => {
                input_cmds::run_move(conn.into(), x, y, window, OutputMode::from_flags(json)).await
            }
            InputAction::Scroll {
                conn,
                x,
                y,
                dx,
                dy,
                window,
                json,
            } => {
                input_cmds::run_scroll(
                    conn.into(),
                    x,
                    y,
                    dx.unwrap_or(0),
                    dy.unwrap_or(0),
                    window,
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
                window,
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
                    window,
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
            AgentAction::Wait(args) => {
                agent_cmds::run_wait(
                    args.conn.into(),
                    agent_cmds::WaitCmdArgs {
                        window: args.window,
                        timeout: args.timeout,
                    },
                    OutputMode::from_flags(args.json),
                )
                .await
            }
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
            AgentAction::SetValue(args) => {
                let mode = OutputMode::from_flags(args.json);
                let dry_run = args.dry_run;
                agent_cmds::run_set_value(
                    args.conn.into(),
                    agent_cmds::SetValueCmdArgs {
                        query: agent_cmds::ElementQueryArgs {
                            role: args.role,
                            label: args.label,
                            window: args.window,
                            id: args.id,
                            index: args.index,
                        },
                        value: args.value,
                    },
                    mode,
                    dry_run,
                )
                .await
            }
            AgentAction::Focus(args) => {
                let mode = OutputMode::from_flags(args.common.json);
                let dry_run = args.dry_run;
                agent_cmds::run_focus(
                    args.common.conn.clone().into(),
                    element_args_to_query(args.common),
                    mode,
                    dry_run,
                )
                .await
            }
            AgentAction::ShowMenu { .. } => unimplemented("agent show-menu"),
            AgentAction::WindowFocus(args) => {
                agent_cmds::run_window_focus(
                    args.conn.into(),
                    args.window,
                    OutputMode::from_flags(args.json),
                    args.dry_run,
                )
                .await
            }
            AgentAction::WindowResize(args) => {
                agent_cmds::run_window_resize(
                    args.conn.into(),
                    args.window,
                    args.width,
                    args.height,
                    OutputMode::from_flags(args.json),
                    args.dry_run,
                )
                .await
            }
            AgentAction::WindowMove(args) => {
                agent_cmds::run_window_move(
                    args.conn.into(),
                    args.window,
                    args.x,
                    args.y,
                    OutputMode::from_flags(args.json),
                    args.dry_run,
                )
                .await
            }
            AgentAction::WindowClose(args) => {
                agent_cmds::run_window_close(
                    args.conn.into(),
                    args.window,
                    OutputMode::from_flags(args.json),
                    args.dry_run,
                )
                .await
            }
            AgentAction::WindowMinimize(args) => {
                agent_cmds::run_window_minimize(
                    args.conn.into(),
                    args.window,
                    OutputMode::from_flags(args.json),
                    args.dry_run,
                )
                .await
            }
        },
        Command::Vm { action } => match action {
            VmAction::Start { platform, base, id, display, viewer, json, dry_run } => {
                vm_cmds::run_vm_start(
                    platform, base, id, display, viewer,
                    OutputMode::from_flags(json), dry_run,
                )
                .await
            }
            VmAction::Stop { id, json, dry_run } => {
                vm_cmds::run_vm_stop(id, OutputMode::from_flags(json), dry_run).await
            }
            VmAction::List { json, limit, all, filter } => {
                vm_cmds::run_vm_list(OutputMode::from_flags(json), limit, all, filter).await
            }
            VmAction::Delete { name, force, json, dry_run } => {
                vm_cmds::run_vm_delete(name, force, OutputMode::from_flags(json), dry_run).await
            }
        },
        Command::Doctor { json } => doctor_cmds::run_doctor(OutputMode::from_flags(json)),
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
