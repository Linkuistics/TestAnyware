//! Adoption at the start of the app (spec: "Lifecycle and adoption").
//!
//! The app keeps no VM state across a restart except the sidecars in the state directory. At
//! its start it reads each meta, decides with [`testanyware_vm::adoption::decide`], and then
//! probes the agent and the VNC banner of each survivor with a deadline. The app serves its
//! socket before any of this runs, so a survivor that answers nothing never delays the app.
//!
//! Adoption never stops, kills or starts a VM, and it sends no signal. A VM that it does not
//! adopt keeps running, and `GET /v1/status` and the menu report it with the reason.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use testanyware_app_protocol::{
    AdoptedVm, AdoptionStatus, NotAdoptedVm, ProbeAnswer, StreamKind, UnfinishedStart,
};
use testanyware_vm::adoption::{decide, Adoption};
use testanyware_vm::identity::{ProcessProbe, SystemProbe};
use testanyware_vm::meta::VmMeta;
use testanyware_vm::spec::VmSpec;
use testanyware_vm::VmPaths;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::streams::{open_guest_stream, GuestEndpoint};
use crate::vms::vm_paths;
use crate::AppContext;

/// How long one probe of a survivor may take, dial included. A guest that answers slower is
/// reported as not answering. That is a statement about the answer and not about the guest.
pub const PROBE_DEADLINE: Duration = Duration::from_secs(8);

/// The length of an RFB protocol version banner, such as `RFB 003.008\n`.
const RFB_BANNER_BYTES: usize = 12;

/// The results of the current run, shared with `GET /v1/status` and with the menu.
#[derive(Debug, Default)]
pub struct AdoptionLog {
    status: Mutex<AdoptionStatus>,
}

impl AdoptionLog {
    /// The results now.
    pub fn snapshot(&self) -> AdoptionStatus {
        self.lock().clone()
    }

    fn replace(&self, status: AdoptionStatus) {
        *self.lock() = status;
    }

    /// Note the results for the start records that the app found, before it serves its socket
    /// (see [`crate::unfinished`]). The adoption that follows keeps them.
    pub fn set_unfinished_starts(&self, starts: Vec<UnfinishedStart>) {
        self.lock().unfinished_starts = starts;
    }

    /// Note a new result for a start record that the app decided again (spec "admission-queue":
    /// "A record that tart did not decide"). It replaces the result of the same VM id.
    pub fn update_unfinished_start(&self, start: UnfinishedStart) {
        let mut status = self.lock();
        match status
            .unfinished_starts
            .iter_mut()
            .find(|s| s.id == start.id)
        {
            Some(old) => *old = start,
            None => status.unfinished_starts.push(start),
        }
    }

    fn update(&self, change: impl FnOnce(&mut AdoptionStatus)) {
        change(&mut self.lock());
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, AdoptionStatus> {
        self.status.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// A VM the app adopts, with what the probes need to know about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Survivor {
    pub id: String,
    pub legacy: bool,
    pub agent: Option<GuestEndpoint>,
    pub vnc: Option<GuestEndpoint>,
}

/// What the app found in the state directory.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Scan {
    pub survivors: Vec<Survivor>,
    pub not_adopted: Vec<NotAdoptedVm>,
}

/// Read each meta in the state directory and decide what to do with its VM. This reads files
/// and asks `probe` about processes, and it changes nothing.
pub fn scan(paths: &VmPaths, probe: &impl ProcessProbe) -> Scan {
    let mut scan = Scan::default();
    for id in meta_ids(paths) {
        let meta = match VmMeta::load(&paths.meta_path(&id)) {
            Ok(meta) => meta,
            Err(e) => {
                scan.not_adopted.push(NotAdoptedVm {
                    id,
                    reason: "meta_unreadable".to_string(),
                    detail: e.to_string(),
                });
                continue;
            }
        };
        match decide(&meta, probe) {
            Adoption::Adopt => scan.survivors.push(survivor(&id, false, paths)),
            Adoption::Legacy => scan.survivors.push(survivor(&id, true, paths)),
            Adoption::Stopped => scan.not_adopted.push(NotAdoptedVm {
                id,
                reason: "process_gone".to_string(),
                detail: format!(
                    "process {} of the VM is gone. `testanyware vm stop` removes its records.",
                    meta.pid
                ),
            }),
            Adoption::Doubt { pid, mismatch } => scan.not_adopted.push(NotAdoptedVm {
                id,
                reason: mismatch.code().to_string(),
                detail: format!(
                    "pid {pid} of the VM names another process: {}. The app signals nothing.",
                    mismatch.describe()
                ),
            }),
        }
    }
    scan
}

/// The ids that have a meta in the state directory, in order.
pub(crate) fn meta_ids(paths: &VmPaths) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(paths.vms_dir()) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".meta.json").map(str::to_string)
        })
        .collect();
    ids.sort();
    ids
}

fn survivor(id: &str, legacy: bool, paths: &VmPaths) -> Survivor {
    let spec = VmSpec::load(&paths.spec_path(id)).ok();
    let vnc = spec.as_ref().map(|s| GuestEndpoint {
        host: s.vnc.host.clone(),
        port: s.vnc.port,
    });
    let agent = spec.as_ref().and_then(|s| {
        s.agent.as_ref().map(|a| GuestEndpoint {
            host: a.host.clone(),
            port: a.port,
        })
    });
    Survivor {
        id: id.to_string(),
        legacy,
        agent,
        vnc,
    }
}

/// Probe the agent: connect, ask `GET /health`, and read a status line.
pub async fn probe_agent(
    ctx: &AppContext,
    endpoint: &GuestEndpoint,
    deadline: Duration,
) -> ProbeAnswer {
    let probe = async {
        let mut stream = open_guest_stream(&ctx.grant, endpoint).await.ok()?;
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: testanyware\r\nConnection: close\r\n\r\n")
            .await
            .ok()?;
        let mut head = [0u8; 5];
        stream.read_exact(&mut head).await.ok()?;
        (&head == b"HTTP/").then_some(())
    };
    answer(tokio::time::timeout(deadline, probe).await.ok().flatten())
}

/// Probe the VNC port: connect and read the protocol banner, then close. The app reads the whole
/// banner and sends nothing, because the banner is the answer.
pub async fn probe_vnc_banner(
    ctx: &AppContext,
    endpoint: &GuestEndpoint,
    deadline: Duration,
) -> ProbeAnswer {
    let probe = async {
        let mut stream = open_guest_stream(&ctx.grant, endpoint).await.ok()?;
        let mut banner = [0u8; RFB_BANNER_BYTES];
        stream.read_exact(&mut banner).await.ok()?;
        banner.starts_with(b"RFB ").then_some(())
    };
    answer(tokio::time::timeout(deadline, probe).await.ok().flatten())
}

fn answer(found: Option<()>) -> ProbeAnswer {
    match found {
        Some(()) => ProbeAnswer::Answering,
        None => ProbeAnswer::NotAnswering,
    }
}

/// Adopt the VMs in the state directory: read the sidecars, publish the decisions, and probe the
/// survivors. The caller starts this after the app serves its socket, and the probes of all
/// survivors run at once.
pub async fn run(ctx: Arc<AppContext>) {
    let blocking_ctx = Arc::clone(&ctx);
    let scanned = tokio::task::spawn_blocking(move || {
        scan(&vm_paths(&blocking_ctx), &SystemProbe)
    })
    .await
    .unwrap_or_default();
    run_scanned(&ctx, scanned, ctx.probe_deadline).await;
}

/// The part of [`run`] that follows the scan.
pub async fn run_scanned(ctx: &Arc<AppContext>, scanned: Scan, deadline: Duration) {
    let pending = |survivor: &Survivor| AdoptedVm {
        id: survivor.id.clone(),
        legacy: survivor.legacy,
        agent: if survivor.agent.is_some() {
            ProbeAnswer::Pending
        } else {
            ProbeAnswer::NotProbed
        },
        vnc: if survivor.vnc.is_some() {
            ProbeAnswer::Pending
        } else {
            ProbeAnswer::NotProbed
        },
    };
    ctx.adoption.replace(AdoptionStatus {
        finished: false,
        adopted: scanned.survivors.iter().map(pending).collect(),
        not_adopted: scanned.not_adopted,
        unfinished_starts: ctx.adoption.snapshot().unfinished_starts,
    });

    let mut tasks = Vec::new();
    for survivor in scanned.survivors {
        for kind in [StreamKind::Agent, StreamKind::Vnc] {
            let endpoint = match kind {
                StreamKind::Agent => survivor.agent.clone(),
                StreamKind::Vnc => survivor.vnc.clone(),
            };
            let Some(endpoint) = endpoint else {
                continue;
            };
            let ctx = Arc::clone(ctx);
            let id = survivor.id.clone();
            tasks.push(tokio::spawn(async move {
                let found = match kind {
                    StreamKind::Agent => probe_agent(&ctx, &endpoint, deadline).await,
                    StreamKind::Vnc => probe_vnc_banner(&ctx, &endpoint, deadline).await,
                };
                ctx.adoption.update(|status| {
                    if let Some(vm) = status.adopted.iter_mut().find(|vm| vm.id == id) {
                        match kind {
                            StreamKind::Agent => vm.agent = found,
                            StreamKind::Vnc => vm.vnc = found,
                        }
                    }
                });
            }));
        }
    }
    for task in tasks {
        let _ = task.await;
    }
    ctx.adoption.update(|status| status.finished = true);
}

#[cfg(test)]
mod tests {
    use super::*;
    use testanyware_vm::identity::ProcessIdentity;
    use testanyware_vm::spec::{AgentEndpoint, VncEndpoint};
    use testanyware_vm::meta::VmTool;

    /// Processes by pid: a start time each. The boot session is fixed.
    struct Host(std::collections::HashMap<i32, u64>);

    impl ProcessProbe for Host {
        fn start_time(&self, pid: i32) -> Option<u64> {
            self.0.get(&pid).copied()
        }
        fn boot_session(&self) -> Option<String> {
            Some("boot-a".into())
        }
    }

    fn identity(start_time: u64) -> ProcessIdentity {
        ProcessIdentity {
            start_time,
            boot_session: "boot-a".into(),
        }
    }

    fn paths(dir: &std::path::Path) -> VmPaths {
        VmPaths::new(dir.join("state"), dir.join("data"), dir.join("tmp"))
    }

    fn write_vm(paths: &VmPaths, meta: VmMeta, agent_port: Option<u16>) {
        std::fs::create_dir_all(paths.vms_dir()).unwrap();
        let spec = VmSpec {
            vnc: VncEndpoint {
                host: "127.0.0.1".into(),
                port: 5900,
                password: None,
            },
            agent: agent_port.map(|port| AgentEndpoint {
                host: "127.0.0.1".into(),
                port,
            }),
            platform: "linux".into(),
            logical: None,
        };
        spec.write_atomic(&paths.spec_path(&meta.id)).unwrap();
        meta.write_atomic(&paths.meta_path(&meta.id)).unwrap();
    }

    fn meta(id: &str, tool: VmTool, pid: i32, identity: Option<ProcessIdentity>) -> VmMeta {
        VmMeta {
            identity,
            ..VmMeta::new(id, tool, pid)
        }
    }

    #[test]
    fn each_meta_gets_the_outcome_that_its_process_earns() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        write_vm(&paths, meta("vm-adopt", VmTool::Qemu, 10, Some(identity(1))), Some(8648));
        write_vm(&paths, meta("vm-gone", VmTool::Qemu, 11, Some(identity(1))), None);
        write_vm(&paths, meta("vm-reused", VmTool::Qemu, 12, Some(identity(1))), None);
        write_vm(&paths, meta("vm-legacy", VmTool::Qemu, 13, None), Some(8648));
        std::fs::write(paths.meta_path("vm-torn"), b"{ not json").unwrap();
        // pid 11 is gone, and pid 12 started at another time.
        let host = Host([(10, 1), (12, 99), (13, 5)].into());

        let scanned = scan(&paths, &host);

        let survivors: Vec<_> = scanned
            .survivors
            .iter()
            .map(|s| (s.id.as_str(), s.legacy))
            .collect();
        assert_eq!(survivors, [("vm-adopt", false), ("vm-legacy", true)]);
        let reasons: Vec<_> = scanned
            .not_adopted
            .iter()
            .map(|n| (n.id.as_str(), n.reason.as_str()))
            .collect();
        assert_eq!(
            reasons,
            [
                ("vm-gone", "process_gone"),
                ("vm-reused", "start_time_differs"),
                ("vm-torn", "meta_unreadable"),
            ]
        );
        assert!(scanned.not_adopted[1].detail.contains("signals nothing"));
    }

    #[test]
    fn a_directory_with_no_vms_gives_an_empty_scan() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(scan(&paths(dir.path()), &Host(Default::default())), Scan::default());
    }
}
