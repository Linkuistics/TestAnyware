//! `vm {start|stop|list|delete}` command handlers.
//!
//! Bridges clap-parsed args to the `testanyware-vm` crate and emits the
//! contract §3 JSON envelope (or text). Ports the surface of
//! `cli/Sources/testanyware/VMCommand.swift`.

use serde_json::{json, Value};

use testanyware_vm::display_request::parse_display_request;
use testanyware_vm::lifecycle::{Platform, VmLifecycle, VmStartOptions};
use testanyware_vm::listing::{build_view, parse_filter, ListItem, ListView};
use testanyware_vm::tpm::{OmitReason, TpmDecision};
use testanyware_vm::{VmError, VmMeta, VmPaths};

use testanyware_app_client::StartEnd;
use testanyware_app_protocol::{
    is_plain_vm_id, AppError, ListRequest, StartRequest, StartResult, TpmReport,
    GOLDEN_NAME_MAX_BYTES,
};

use crate::app_route;
use crate::commands::exit_app_error;
use crate::output::{exit_code_for, print_error, print_success, OutputMode};
use crate::resolve::{app_socket_with_env, ConnectionOptions, EnvProvider};

// ---- handlers -----------------------------------------------------------

/// The hidden `_vnc-keeper` command (ADR-0018): hold one silent connection to
/// a tart guest's VNC port until the port goes away. `vm start` spawns it
/// detached, with its output in `<id>.keeper.log`. Returns the exit code: 0
/// when it held a connection and the port then went away, 1 otherwise.
pub async fn run_vnc_keeper(id: String, host: String, port: u16) -> i32 {
    testanyware_vm::keeper::run(&id, &host, port).await
}

/// `testanyware vm start`.
#[allow(clippy::too_many_arguments)]
pub async fn run_vm_start(
    platform: String,
    base: Option<String>,
    id: Option<String>,
    display: Option<String>,
    viewer: bool,
    keep_on_failure: bool,
    wait: Option<u64>,
    mode: OutputMode,
    dry_run: bool,
) {
    let parsed = match Platform::parse(&platform) {
        Ok(p) => p,
        Err(err) => exit_vm_error(err, mode),
    };
    // Parse the `--display` flag, splitting any `@2x` HiDPI opt-in from the
    // backend value (ADR-0016 D3). A bad scale / malformed @2x is a usage error.
    let request = match parse_display_request(display.as_deref()) {
        Ok(r) => r,
        Err(err) => print_error(
            mode,
            "USAGE_ERROR",
            &err.to_string(),
            Some(err.remediation()),
            json!({ "value": display.unwrap_or_default() }),
            2,
        ),
    };
    // HiDPI is a macOS-VF concept: there is no Retina mode for Linux/Windows
    // guests, and the `pt` translation would confuse their backends. Reject it
    // here rather than silently producing a broken 1× session.
    if request.logical.is_some() && parsed != Platform::Macos {
        print_error(
            mode,
            "USAGE_ERROR",
            "HiDPI (@2x) is only supported for macOS guests",
            Some("Use --platform macos for HiDPI, or drop the @2x suffix for a 1× display."),
            json!({ "platform": parsed.as_str() }),
            2,
        );
    }
    let mut opts = VmStartOptions::new(parsed, base, id, request, viewer);
    opts.keep_on_failure = keep_on_failure;
    opts.wait = wait.map(std::time::Duration::from_secs);
    let paths = VmPaths::from_process_env();

    if dry_run {
        // Validate without side effects: the golden must exist in the
        // backend `start` would route to (tart on macOS, else QEMU), plus
        // the QEMU host preflight. Routing lives in the vm crate.
        let plan = match VmLifecycle::dry_run_validate_start(&opts, &paths) {
            Ok(plan) => plan,
            Err(err) => exit_vm_error(err, mode),
        };
        emit_start_plan(&opts, plan.tpm, mode);
        return;
    }

    // On a macOS host, and with `TESTANYWARE_APP_SOCKET` set, the TestAnyware app starts the VM:
    // it holds the Local Network grant, and the guest processes outlive the call. Elsewhere the
    // Host CLI starts it. Both print the same lines, and then the same output.
    let started = match app_socket_with_env(&EnvProvider::process()) {
        Some(socket) => {
            let request = StartRequest {
                platform: opts.platform.as_str().to_string(),
                base: opts.base.clone(),
                id: opts.id.clone(),
                display: opts.display.clone(),
                logical: opts.logical,
                keep_on_failure: opts.keep_on_failure,
                wait_secs: opts.wait.map(|wait| wait.as_secs()),
            };
            match app_route::start_vm(socket, &request, |line| eprintln!("{line}")).await {
                Ok(StartEnd::Started(result)) => Started::from_app(result, &opts.base),
                // The app sends the code, the message, the remedy and the details of the
                // direct command's error, so the output is the direct output.
                Ok(StartEnd::Failed(error)) => exit_answered_error(error, mode),
                Err(error) => exit_app_error(error, mode),
            }
        }
        None => match VmLifecycle::start(&opts, &paths).await {
            Ok(result) => Started::from_direct(&result, &opts.base),
            Err(err) => exit_vm_error(err, mode),
        },
    };

    // Text mode keeps stdout to the id alone, so the TPM decision goes
    // to stderr; `--json` carries it in the envelope (`tpm`).
    if let (OutputMode::Text, Some(tpm)) = (mode, started.tpm) {
        eprintln!("{}", tpm_text(tpm));
    }
    // The viewer resolves through `--connect` against the spec file `start` just
    // wrote — the same chain `--vm` uses.
    let viewer_connect = viewer.then(|| started.spec_path.clone());
    match mode {
        OutputMode::Text => println!("{}", started.id),
        OutputMode::Json => print_success(started.json()),
    }
    // Sugar (ADR-0005 Q2): the start envelope is already emitted, so
    // an `--json` consumer has its data before this window blocks.
    // Opening it inline (blocking-until-close) is acceptable for an
    // explicit window request. Dry-run returns earlier, never here.
    if let Some(connect) = viewer_connect {
        let conn = ConnectionOptions { connect: Some(connect), ..Default::default() };
        crate::commands::viewer::run_viewer(conn);
    }
}

/// What a successful `vm start` prints, whoever started the VM.
struct Started {
    id: String,
    platform: String,
    base: String,
    vnc: (String, u16),
    agent: Option<(String, u16)>,
    spec_path: String,
    meta_path: String,
    tpm: Option<TpmDecision>,
}

impl Started {
    fn from_direct(result: &testanyware_vm::lifecycle::VmStartResult, base: &str) -> Self {
        Self {
            id: result.id.clone(),
            platform: result.platform.as_str().to_string(),
            base: base.to_string(),
            vnc: (result.spec.vnc.host.clone(), result.spec.vnc.port),
            agent: result.spec.agent.as_ref().map(|a| (a.host.clone(), a.port)),
            spec_path: result.spec_path.display().to_string(),
            meta_path: result.meta_path.display().to_string(),
            tpm: result.tpm,
        }
    }

    fn from_app(result: StartResult, base: &str) -> Self {
        Self {
            id: result.id,
            platform: result.platform,
            base: base.to_string(),
            vnc: (result.vnc.host, result.vnc.port),
            agent: result.agent.map(|a| (a.host, a.port)),
            spec_path: result.spec_path,
            meta_path: result.meta_path,
            tpm: result.tpm.map(tpm_decision),
        }
    }

    /// The `data` of the `--json` envelope.
    fn json(&self) -> Value {
        let agent = self
            .agent
            .as_ref()
            .map(|(host, port)| json!({ "host": host, "port": port }))
            .unwrap_or(Value::Null);
        let mut data = json!({
            "id": self.id,
            "platform": self.platform,
            "base": self.base,
            "vnc": { "host": self.vnc.0, "port": self.vnc.1 },
            "agent": agent,
            "spec_path": self.spec_path,
            "meta_path": self.meta_path,
        });
        if let Some(tpm) = self.tpm {
            data["tpm"] = tpm_json(tpm);
        }
        data
    }
}

/// The TPM decision that the app reported, as the vm crate states it.
fn tpm_decision(report: TpmReport) -> TpmDecision {
    if report.attached {
        TpmDecision::Attach { ppi_off: report.ppi_off.unwrap_or(false) }
    } else {
        match report.reason.as_deref() {
            Some("host_cannot_supply_one") => TpmDecision::Omit(OmitReason::HostCannotSupplyOne),
            _ => TpmDecision::Omit(OmitReason::GuestDoesNotNeedOne),
        }
    }
}

/// Print an error that the app sent for a lifecycle command, and exit with the code that the
/// error has today. The app sends exactly what the direct command prints, so no default remedy
/// is added here.
fn exit_answered_error(error: AppError, mode: OutputMode) -> ! {
    let exit = exit_code_for(&error.code);
    print_error(mode, &error.code, &error.message, error.remediation.as_deref(), error.details, exit);
}

fn emit_start_plan(opts: &VmStartOptions, tpm: Option<TpmDecision>, mode: OutputMode) {
    match mode {
        OutputMode::Text => {
            println!("dry-run: would start {} (platform {}, base {})",
                opts.id, opts.platform.as_str(), opts.base);
            if let Some(tpm) = tpm {
                println!("dry-run: {}", tpm_text(tpm));
            }
        }
        OutputMode::Json => {
            let mut data = json!({
                "dry_run": true,
                "id": opts.id,
                "platform": opts.platform.as_str(),
                "base": opts.base,
                "vnc": { "host": "localhost", "port": 0 },
            });
            if let Some(tpm) = tpm {
                data["tpm"] = tpm_json(tpm);
            }
            print_success(data);
        }
    }
}

/// The `tpm` value of the `vm start` envelope: `{"attached": true, "ppi_off":
/// bool}` or `{"attached": false, "reason": ...}`. Pure, so every answer of the
/// TPM decision is unit-tested; the schema (`vm-start.json`) describes it.
/// `doctor` renders its Windows TPM check from the same function.
pub(crate) fn tpm_json(decision: TpmDecision) -> Value {
    match decision {
        TpmDecision::Attach { ppi_off } => json!({ "attached": true, "ppi_off": ppi_off }),
        TpmDecision::Omit(reason) => json!({ "attached": false, "reason": omit_reason_slug(reason) }),
    }
}

/// The same answer in words, for stderr in text mode and the dry-run plan (and
/// `doctor`'s text line).
pub(crate) fn tpm_text(decision: TpmDecision) -> String {
    match decision {
        TpmDecision::Attach { ppi_off: false } => "TPM attached".to_string(),
        TpmDecision::Attach { ppi_off: true } => "TPM attached (PPI off)".to_string(),
        TpmDecision::Omit(OmitReason::GuestDoesNotNeedOne) => {
            "TPM omitted: the guest does not need one".to_string()
        }
        TpmDecision::Omit(OmitReason::HostCannotSupplyOne) => {
            "TPM omitted: this host's QEMU cannot supply one (it has no tpm-tis-device)".to_string()
        }
    }
}

fn omit_reason_slug(reason: OmitReason) -> &'static str {
    match reason {
        OmitReason::GuestDoesNotNeedOne => "guest_does_not_need_one",
        OmitReason::HostCannotSupplyOne => "host_cannot_supply_one",
    }
}

/// `testanyware vm stop`.
pub async fn run_vm_stop(id: Option<String>, mode: OutputMode, dry_run: bool) {
    let Some(id) = id.filter(|s| !s.is_empty()) else {
        print_error(
            mode,
            "USAGE_ERROR",
            "VM id required: pass it as an argument or set TESTANYWARE_VM_ID",
            Some("Run `testanyware vm list` to see running VM ids."),
            json!({}),
            2,
        );
    };
    let paths = VmPaths::from_process_env();
    let meta_path = paths.meta_path(&id);

    if dry_run {
        if !meta_path.is_file() {
            exit_vm_error(VmError::VmNotFound { id }, mode);
        }
        let pid = VmMeta::load(&meta_path).ok().map(|m| m.pid);
        let pid_str = pid.map_or_else(|| "unknown".to_string(), |p| p.to_string());
        match mode {
            OutputMode::Text => println!("dry-run: would stop {id} (pid {pid_str})"),
            OutputMode::Json => {
                print_success(json!({ "dry_run": true, "id": id, "stopped": false }));
            }
        }
        return;
    }

    // On a macOS host, and with `TESTANYWARE_APP_SOCKET` set, the TestAnyware app stops the VM,
    // because it started the VM's processes. Elsewhere the Host CLI stops it. Both print the same.
    let outcome = match app_socket_with_env(&EnvProvider::process()) {
        // An id that is no plain id names no VM, which is what the app would answer.
        Some(_) if !is_plain_vm_id(&id) => Err(VmError::VmNotFound { id: id.clone() }),
        Some(socket) => match app_route::stop_vm(socket, &id).await {
            Ok(_) => Ok(()),
            Err(error) => exit_answered_error(error, mode),
        },
        None => VmLifecycle::stop(&id, &paths),
    };
    match outcome {
        Ok(()) => match mode {
            OutputMode::Text => println!("stopped {id}"),
            OutputMode::Json => print_success(json!({ "id": id, "stopped": true })),
        },
        Err(err) => exit_vm_error(err, mode),
    }
}

/// `testanyware vm list`.
pub async fn run_vm_list(
    mode: OutputMode,
    limit: usize,
    all: bool,
    filter: Option<String>,
) {
    let filters = match filter.as_deref().map(parse_filter) {
        Some(Ok(f)) => f,
        Some(Err(msg)) => print_error(
            mode,
            "USAGE_ERROR",
            &format!("invalid --filter: {msg}"),
            Some("Expected comma-separated field=value pairs, e.g. --filter platform=windows."),
            json!({ "value": filter.unwrap_or_default() }),
            2,
        ),
        None => Vec::new(),
    };
    // On a macOS host, and with `TESTANYWARE_APP_SOCKET` set, the TestAnyware app builds the
    // view from the VM records. Elsewhere the Host CLI builds it from the same code.
    let view = match app_socket_with_env(&EnvProvider::process()) {
        Some(socket) => {
            let request = ListRequest { limit, all, filters };
            match app_route::list_vms(socket, &request).await {
                Ok(view) => view,
                Err(error) => exit_app_error(error, mode),
            }
        }
        None => {
            let listing = VmLifecycle::list(&VmPaths::from_process_env());
            build_view(&listing, &filters, limit, all)
        }
    };

    match mode {
        OutputMode::Text => render_list_text(&view),
        OutputMode::Json => print_success(list_json(&view)),
    }
}

/// `testanyware vm delete`.
pub async fn run_vm_delete(name: String, force: bool, mode: OutputMode, dry_run: bool) {
    let paths = VmPaths::from_process_env();

    if dry_run {
        if let Err(err) = VmLifecycle::dry_run_validate_delete(&name, &paths) {
            exit_vm_error(err, mode);
        }
        match mode {
            OutputMode::Text => println!("dry-run: would delete golden {name}"),
            OutputMode::Json => {
                print_success(json!({ "dry_run": true, "name": name, "deleted": false }));
            }
        }
        return;
    }

    // On a macOS host, and with `TESTANYWARE_APP_SOCKET` set, the TestAnyware app deletes the
    // golden. Elsewhere the Host CLI deletes it. Both print the same.
    let outcome = match app_socket_with_env(&EnvProvider::process()) {
        // A name longer than a file name names no golden, which is what the app would answer.
        // The path of such a request would be too long for an HTTP request line.
        Some(_) if name.len() > GOLDEN_NAME_MAX_BYTES => {
            Err(VmError::GoldenNotFound { name: name.clone() })
        }
        Some(socket) => match app_route::delete_golden(socket, &name, force).await {
            Ok(_) => Ok(()),
            Err(error) => exit_answered_error(error, mode),
        },
        None => VmLifecycle::delete(&name, force, &paths),
    };
    match outcome {
        Ok(()) => match mode {
            OutputMode::Text => println!("deleted {name}"),
            OutputMode::Json => print_success(json!({ "name": name, "deleted": true })),
        },
        Err(err) => exit_vm_error(err, mode),
    }
}

/// `testanyware vm create-golden` (ADR-0007/0008; grove node
/// `110-vm-create-golden-macos`).
///
/// `--dry-run` validates and prints the 5-boot plan, mutating nothing (both
/// text and `--json`). A real invocation drives the full pipeline end-to-end
/// ([`testanyware_vm::finalize::create_golden`]): boot-1 normal-mode
/// provisioning (pubkey, defaults, wallpaper, CLT, Homebrew, agent + plist) →
/// the SIP/TCC cycle (disable SIP via Recovery, grant TCC, re-enable SIP) →
/// the agent-health gate, clean shutdown, and `tart clone` to the golden. On
/// success it reports the created golden's name; the throwaway setup VM is
/// consumed. Mirrors the `run_vm_start` dry-run-validate → emit-plan shape.
pub async fn run_vm_create_golden(
    platform: String,
    version: Option<String>,
    name: Option<String>,
    iso: Option<String>,
    mode: OutputMode,
    dry_run: bool,
) {
    // macOS, Windows, and Linux goldens are all supported — all three are
    // macOS-host work (macOS/Linux via tart, Windows via QEMU+swtpm). An unknown
    // platform string is a usage error (INVALID_PLATFORM, exit 2).
    let parsed = match Platform::parse(&platform) {
        Ok(p) => p,
        Err(err) => exit_vm_error(err, mode),
    };
    // Per-platform default version (macOS: 27; Windows: 11; Linux: 24.04). The
    // macOS default must name the golden `Platform::default_base` starts.
    let version = version.unwrap_or_else(|| match parsed {
        Platform::Windows => "11".into(),
        Platform::Linux => "24.04".into(),
        Platform::Macos => "27".into(),
    });
    let name = name.unwrap_or_else(|| format!("testanyware-golden-{}-{version}", parsed.as_str()));

    match parsed {
        Platform::Windows => run_vm_create_golden_windows(version, name, iso, mode, dry_run).await,
        Platform::Linux => run_vm_create_golden_linux(version, name, mode, dry_run).await,
        Platform::Macos => run_vm_create_golden_macos(version, name, mode, dry_run).await,
    }
}

/// macOS golden: the 5-boot SIP/TCC pipeline (node `110`, ADR-0007/0008).
async fn run_vm_create_golden_macos(
    version: String,
    name: String,
    mode: OutputMode,
    dry_run: bool,
) {
    if dry_run {
        emit_golden_plan(&version, &name, mode);
        return;
    }

    // tart is macOS-only, so a non-macOS host cannot serve the command.
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (&version, &name);
        exit_vm_error(
            VmError::BackendUnsupported {
                platform: "macos (vm create-golden requires a macOS host)".into(),
            },
            mode,
        );
    }

    #[cfg(target_os = "macos")]
    {
        use testanyware_vm::finalize::create_golden;
        use testanyware_vm::golden::GoldenOptions;
        use testanyware_vm::VmPaths;

        let opts = GoldenOptions { version: version.clone(), name: name.clone() };
        let paths = VmPaths::from_process_env();
        match create_golden(&opts, &paths).await {
            Ok(golden) => match mode {
                OutputMode::Text => {
                    println!("Golden image '{golden}' created successfully.");
                    println!("Use it with: testanyware vm start --platform macos --base {golden}");
                }
                OutputMode::Json => print_success(json!({
                    "platform": "macos",
                    "version": version,
                    "name": golden,
                    "created": true,
                })),
            },
            Err(err) => exit_vm_error(err, mode),
        }
    }
}

/// Windows golden: unattended ISO install + agent provisioning (leaf `220/020`,
/// ADR-0009). macOS-host work (FAT32 media built with `hdiutil`).
async fn run_vm_create_golden_windows(
    version: String,
    name: String,
    iso: Option<String>,
    mode: OutputMode,
    dry_run: bool,
) {
    if dry_run {
        emit_windows_golden_plan(&version, &name, iso.as_deref(), mode);
        return;
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (&version, &name, &iso);
        exit_vm_error(
            VmError::BackendUnsupported {
                platform: "macos (vm create-golden --platform windows requires a macOS host)".into(),
            },
            mode,
        );
    }

    #[cfg(target_os = "macos")]
    {
        use std::path::Path;
        use testanyware_vm::golden::GoldenOptions;
        use testanyware_vm::golden_windows::create_golden_windows;
        use testanyware_vm::VmPaths;

        let opts = GoldenOptions { version: version.clone(), name: name.clone() };
        let paths = VmPaths::from_process_env();
        let iso_path = iso.as_deref().map(Path::new);
        match create_golden_windows(&opts, iso_path, &paths).await {
            Ok(golden) => match mode {
                OutputMode::Text => {
                    println!("Golden image '{golden}' created successfully.");
                    println!("Use it with: testanyware vm start --platform windows --base {golden}");
                }
                OutputMode::Json => print_success(json!({
                    "platform": "windows",
                    "version": version,
                    "name": golden,
                    "created": true,
                })),
            },
            Err(err) => exit_vm_error(err, mode),
        }
    }
}

/// Linux golden: a tart-based SSH provisioning pass + one apply-settings reboot
/// (leaf `230`, ADR-0007). macOS-host work (built on this Mac via tart), no
/// SIP/TCC/recovery cycle.
async fn run_vm_create_golden_linux(
    version: String,
    name: String,
    mode: OutputMode,
    dry_run: bool,
) {
    if dry_run {
        emit_linux_golden_plan(&version, &name, mode);
        return;
    }

    // tart is macOS-only, so a non-macOS host cannot serve the command.
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (&version, &name);
        exit_vm_error(
            VmError::BackendUnsupported {
                platform: "macos (vm create-golden --platform linux requires a macOS host)".into(),
            },
            mode,
        );
    }

    #[cfg(target_os = "macos")]
    {
        use testanyware_vm::golden::GoldenOptions;
        use testanyware_vm::golden_linux::create_golden_linux;
        use testanyware_vm::VmPaths;

        let opts = GoldenOptions { version: version.clone(), name: name.clone() };
        let paths = VmPaths::from_process_env();
        match create_golden_linux(&opts, &paths).await {
            Ok(golden) => match mode {
                OutputMode::Text => {
                    println!("Golden image '{golden}' created successfully.");
                    println!("Use it with: testanyware vm start --platform linux --base {golden}");
                }
                OutputMode::Json => print_success(json!({
                    "platform": "linux",
                    "version": version,
                    "name": golden,
                    "created": true,
                })),
            },
            Err(err) => exit_vm_error(err, mode),
        }
    }
}

/// The 5-boot provisioning sequence (3 normal + 2 recovery) that golden
/// creation drives, as `(step, mode, actions)`. Parity with the boot
/// sequence in `provisioner/scripts/vm-create-golden-macos.sh`. Pure, so
/// the plan is unit-tested and shared by the text and JSON renderers.
fn golden_boot_plan() -> [(u32, &'static str, &'static str); 5] {
    [
        (1, "normal",
         "SSH pubkey install, macOS defaults, solid wallpaper, hide widgets, \
          Xcode CLT, Homebrew, agent binary + LaunchAgent plist"),
        (2, "recovery", "disable SIP (csrutil disable)"),
        (3, "normal",
         "grant TCC: kTCCServiceAccessibility + kTCCServiceSystemPolicyAllFiles"),
        (4, "recovery", "re-enable SIP (csrutil enable)"),
        (5, "normal",
         "verify agent health on port 8648, disable Remote Login, clean shutdown, \
          clone to golden"),
    ]
}

/// Where the macOS golden's base VM comes from, in words, for the dry-run
/// plan: a Cirrus vanilla image, or a base built from Apple's IPSW on first use.
/// Pure (no `tart`, no network), shared by the text and JSON renderers.
fn golden_base_note(version: &str) -> String {
    use testanyware_vm::base_source::{base_source, base_vm_name, BaseSource};
    match base_source(version) {
        BaseSource::Vanilla(image) => format!("Cirrus Labs vanilla image {image}"),
        BaseSource::Ipsw { major } => format!(
            "{}, built from Apple's IPSW for macOS {major} on first use (about 25 GB download)",
            base_vm_name(major)
        ),
    }
}

fn emit_golden_plan(version: &str, name: &str, mode: OutputMode) {
    let plan = golden_boot_plan();
    let base = golden_base_note(version);
    match mode {
        OutputMode::Text => {
            println!(
                "dry-run: would create golden {name} (platform macos, version {version})"
            );
            println!("base: {base}");
            println!("boot plan (3 normal + 2 recovery = 5 boots):");
            for (step, boot_mode, actions) in plan {
                println!("  {step}. [{boot_mode}] {actions}");
            }
        }
        OutputMode::Json => {
            let steps: Vec<Value> = plan
                .iter()
                .map(|(step, boot_mode, actions)| {
                    json!({ "step": step, "mode": boot_mode, "actions": actions })
                })
                .collect();
            print_success(json!({
                "dry_run": true,
                "platform": "macos",
                "version": version,
                "name": name,
                "base": base,
                "boot_plan": steps,
            }));
        }
    }
}

/// The Linux golden boot sequence (2 normal boots — no SIP/TCC/recovery
/// cycle), as `(step, mode, actions)`. Parity with the boot sequence in
/// `provisioner/scripts/vm-create-golden-linux.sh`. Pure, so the plan is
/// unit-tested and shared by the text and JSON renderers. Reuses the macOS
/// `boot_plan` schema shape (both Linux boots are `normal`).
fn linux_boot_plan() -> [(u32, &'static str, &'static str); 2] {
    [
        (1, "normal",
         "SSH pubkey install, Ubuntu Desktop (minimal), NetworkManager via netplan, \
          Firefox, GDM autologin forced to X11, solid-gray locked-down desktop, \
          system updates, silent boot, testanyware-agent (systemd user service + AT-SPI2)"),
        (2, "normal",
         "reboot to apply (GDM autologin starts the agent), verify agent health on \
          port 8648, disable + mask sshd, clean shutdown, clone to golden"),
    ]
}

fn emit_linux_golden_plan(version: &str, name: &str, mode: OutputMode) {
    let plan = linux_boot_plan();
    match mode {
        OutputMode::Text => {
            println!("dry-run: would create golden {name} (platform linux, version {version})");
            println!("boot plan (2 normal boots — no SIP/TCC/recovery cycle):");
            for (step, boot_mode, actions) in plan {
                println!("  {step}. [{boot_mode}] {actions}");
            }
        }
        OutputMode::Json => {
            let steps: Vec<Value> = plan
                .iter()
                .map(|(step, boot_mode, actions)| {
                    json!({ "step": step, "mode": boot_mode, "actions": actions })
                })
                .collect();
            print_success(json!({
                "dry_run": true,
                "platform": "linux",
                "version": version,
                "name": name,
                "boot_plan": steps,
            }));
        }
    }
}

/// The Windows golden phases (single-boot unattended install + agent
/// provisioning), as `(step, phase, actions)`. Parity with the boot sequence
/// in `provisioner/scripts/vm-create-golden-windows.sh`. Pure, so the plan is
/// unit-tested and shared by the text and JSON renderers.
fn windows_install_plan() -> [(u32, &'static str, &'static str); 4] {
    [
        (1, "media",
         "build autounattend USB (answer file + agent + VirtIO ARM64 drivers), \
          blank 64GB NVMe disk, blank UEFI vars, swtpm TPM 2.0 (when the host \
          QEMU can supply one)"),
        (2, "install",
         "boot from ISO + USB; unattended Setup partitions, installs Windows, \
          creates admin/autologin, installs agent logon task + Chocolatey (20–40 min)"),
        (3, "provision",
         "wait for the in-VM agent on :8648, run desktop-setup, reboot to finalize, \
          settle, clean the desktop"),
        (4, "finalize",
         "agent shutdown, then move the setup disk/efivars/tpm into the golden"),
    ]
}

fn emit_windows_golden_plan(version: &str, name: &str, iso: Option<&str>, mode: OutputMode) {
    let plan = windows_install_plan();
    match mode {
        OutputMode::Text => {
            println!("dry-run: would create golden {name} (platform windows, version {version})");
            match iso {
                Some(p) => println!("  install ISO: {p}"),
                None => println!("  install ISO: cached (pass --iso <path> on first run)"),
            }
            println!("plan (single-boot unattended install + agent provisioning):");
            for (step, phase, actions) in plan {
                println!("  {step}. [{phase}] {actions}");
            }
        }
        OutputMode::Json => {
            let steps: Vec<Value> = plan
                .iter()
                .map(|(step, phase, actions)| {
                    json!({ "step": step, "phase": phase, "actions": actions })
                })
                .collect();
            print_success(json!({
                "dry_run": true,
                "platform": "windows",
                "version": version,
                "name": name,
                "iso": iso,
                "plan": steps,
            }));
        }
    }
}

// ---- pure helpers (unit-tested) -----------------------------------------

fn item_to_json(item: &ListItem) -> Value {
    json!({
        "kind": item.kind,
        "name": item.name,
        "platform": item.platform,
        "backend": item.backend,
        "pid": item.pid,
        "vnc": item.vnc,
        "agent": item.agent,
    })
}

/// The `--json` payload of `vm list`.
fn list_json(view: &ListView) -> Value {
    let items: Vec<Value> = view.items.iter().map(item_to_json).collect();
    json!({
        "items": items,
        "returned": view.returned,
        "total": view.total,
        "truncated": view.truncated,
    })
}

fn render_list_text(view: &ListView) {
    let (returned, total, truncated) = (view.returned, view.total, view.truncated);
    let goldens: Vec<&ListItem> = view.items.iter().filter(|i| i.kind == "golden").collect();
    let running: Vec<&ListItem> = view.items.iter().filter(|i| i.kind == "running").collect();
    println!("Golden images:");
    if goldens.is_empty() {
        println!("  (none)");
    } else {
        for g in goldens {
            println!("  {:<8} {:<40} {}", g.platform, g.name, g.backend);
        }
    }
    println!();
    println!("Running clones:");
    if running.is_empty() {
        println!("  (none)");
    } else {
        for r in running {
            println!(
                "  {:<24} {:<8} vnc={} agent={} pid={}",
                r.name,
                r.platform,
                r.vnc.as_deref().unwrap_or("-"),
                r.agent.as_deref().unwrap_or("-"),
                r.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into()),
            );
        }
    }
    if truncated {
        println!("\nShowing {returned} of {total}. Use --limit N or --all to see more.");
    }
}

fn exit_vm_error(err: VmError, mode: OutputMode) -> ! {
    let code = err.code();
    let exit = err.exit_code();
    let remediation = err.remediation();
    print_error(mode, code, &err.to_string(), remediation.as_deref(), err.details(), exit);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The app sends the code of a `VmError`, and the Host CLI maps the code back to an exit
    /// code with its own table. The two must agree for every error that a stop or a delete can
    /// make, or the routed command would exit with another code than the direct command.
    #[test]
    fn the_exit_code_of_a_code_matches_the_error_for_stop_and_delete() {
        let errors = [
            VmError::VmNotFound { id: "v".into() },
            VmError::VmStopFailed { id: "v".into() },
            VmError::GoldenNotFound { name: "g".into() },
            VmError::GoldenInUse { name: "g".into(), clone_pids: vec![1] },
            VmError::TartFailed { detail: "d".into() },
            VmError::BackendUnsupported { platform: "macos (tart)".into() },
            VmError::Io("m".into()),
        ];
        for err in errors {
            assert_eq!(exit_code_for(err.code()), err.exit_code(), "{}", err.code());
        }
    }

    #[test]
    fn tpm_json_covers_every_answer() {
        assert_eq!(
            tpm_json(TpmDecision::Attach { ppi_off: true }),
            json!({ "attached": true, "ppi_off": true })
        );
        assert_eq!(
            tpm_json(TpmDecision::Attach { ppi_off: false }),
            json!({ "attached": true, "ppi_off": false })
        );
        assert_eq!(
            tpm_json(TpmDecision::Omit(OmitReason::GuestDoesNotNeedOne)),
            json!({ "attached": false, "reason": "guest_does_not_need_one" })
        );
        assert_eq!(
            tpm_json(TpmDecision::Omit(OmitReason::HostCannotSupplyOne)),
            json!({ "attached": false, "reason": "host_cannot_supply_one" })
        );
    }

    #[test]
    fn tpm_text_tells_attached_from_each_omit() {
        assert_eq!(tpm_text(TpmDecision::Attach { ppi_off: false }), "TPM attached");
        assert_eq!(tpm_text(TpmDecision::Attach { ppi_off: true }), "TPM attached (PPI off)");
        let guest = tpm_text(TpmDecision::Omit(OmitReason::GuestDoesNotNeedOne));
        let host = tpm_text(TpmDecision::Omit(OmitReason::HostCannotSupplyOne));
        assert!(guest.starts_with("TPM omitted") && guest.contains("guest"));
        assert!(host.starts_with("TPM omitted") && host.contains("host"));
        assert_ne!(guest, host);
    }

    /// The renderer's output stays inside what `vm-start.json` declares: the
    /// `tpm` property exists, is optional, and enumerates the reasons emitted.
    #[test]
    fn tpm_json_fits_the_vm_start_schema() {
        let schema: Value = serde_json::from_str(include_str!(
            "../../../../../docs/reference/cli-schemas/vm-start.json"
        ))
        .unwrap();
        let tpm = &schema["properties"]["tpm"];
        assert_eq!(tpm["type"], "object", "vm-start.json must describe `tpm`");
        let required = schema["required"].as_array().unwrap();
        assert!(!required.iter().any(|r| r == "tpm"), "tpm stays optional (additive)");
        let reasons: Vec<&str> = tpm["properties"]["reason"]["enum"]
            .as_array()
            .expect("reason enum")
            .iter()
            .map(|r| r.as_str().unwrap())
            .collect();
        for d in [
            TpmDecision::Omit(OmitReason::GuestDoesNotNeedOne),
            TpmDecision::Omit(OmitReason::HostCannotSupplyOne),
        ] {
            let slug = tpm_json(d)["reason"].as_str().unwrap().to_string();
            assert!(reasons.contains(&slug.as_str()), "{slug} missing from schema enum");
        }
        for d in [TpmDecision::Attach { ppi_off: true }, TpmDecision::Attach { ppi_off: false }] {
            for key in tpm_json(d).as_object().unwrap().keys() {
                assert!(tpm["properties"].get(key).is_some(), "{key} not in schema");
            }
        }
    }

    #[test]
    fn golden_base_note_names_a_vanilla_image_or_the_ipsw_built_base() {
        assert_eq!(
            golden_base_note("tahoe"),
            "Cirrus Labs vanilla image ghcr.io/cirruslabs/macos-tahoe-vanilla:latest"
        );
        let ipsw = golden_base_note("27");
        assert!(ipsw.starts_with("testanyware-base-macos-27, built from Apple's IPSW"), "{ipsw}");
    }

    #[test]
    fn golden_boot_plan_is_three_normal_two_recovery() {
        let plan = golden_boot_plan();
        assert_eq!(plan.len(), 5);
        let normal = plan.iter().filter(|(_, m, _)| *m == "normal").count();
        let recovery = plan.iter().filter(|(_, m, _)| *m == "recovery").count();
        assert_eq!(normal, 3, "script parity: 3 normal boots");
        assert_eq!(recovery, 2, "script parity: 2 recovery boots");
        // Steps are 1-based and contiguous.
        for (i, (step, _, _)) in plan.iter().enumerate() {
            assert_eq!(*step as usize, i + 1);
        }
        // The SIP cycle brackets the TCC grant: disable (2) → grant (3) →
        // enable (4), matching the script's ordering.
        assert!(plan[1].2.contains("disable SIP"));
        assert!(plan[2].2.contains("TCC"));
        assert!(plan[3].2.contains("re-enable SIP"));
    }

    #[test]
    fn linux_boot_plan_is_two_normal_boots() {
        let plan = linux_boot_plan();
        assert_eq!(plan.len(), 2);
        // No recovery cycle on Linux — both boots are normal.
        assert!(plan.iter().all(|(_, m, _)| *m == "normal"));
        for (i, (step, _, _)) in plan.iter().enumerate() {
            assert_eq!(*step as usize, i + 1);
        }
        // Boot 1 provisions; boot 2 applies + clones.
        assert!(plan[0].2.contains("Ubuntu Desktop"));
        assert!(plan[0].2.contains("testanyware-agent"));
        assert!(plan[1].2.contains("verify agent health"));
        assert!(plan[1].2.contains("clone to golden"));
    }
}
