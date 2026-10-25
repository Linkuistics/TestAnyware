//! The start record: the file that lets the app account for a start that a crash interrupted
//! (spec "admission-queue": "A quit, a crash and a restart").
//!
//! The app writes `<vms>/<id>.start-record` before the clone of each start that it runs. The record
//! names the VM id and the backend, and the identity of each process that the start runs, once
//! that process exists. The app removes the record when the start ends. A record that the app
//! finds when it starts therefore names a start that a crash of the app, or of the Mac,
//! interrupted.
//!
//! The record holds identities and nothing else about the processes. A process receives a signal
//! only when its identity matches (see [`crate::adoption::check_process`]).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::VmError;
use crate::identity::ProcessIdentity;
use crate::meta::VmTool;
use crate::paths::VmPaths;

/// The suffix of a start record's file name. A spec ends in `.json` and a meta in `.meta.json`.
/// The record has no such ending, because a VM id may hold dots: the spec of the id `a.start` is
/// `a.start.json`, and a record named like that would be taken for it.
pub const SUFFIX: &str = ".start-record";

/// A process that a start runs, as far as the start knew it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedProcess {
    pub pid: i32,
    /// `None` when the host could not name the identity. Such a process is never signalled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<ProcessIdentity>,
}

/// What a start that is running has made so far.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartRecord {
    pub id: String,
    pub backend: VmTool,
    /// The process that is the VM: the `tart run` of the latest try, or QEMU. A try that the
    /// framework refused replaces it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main: Option<RecordedProcess>,
    /// The VNC keeper of a tart start. It ends only when its next connect fails (ADR-0018), so
    /// a keeper that matches proves nothing about the VM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keeper: Option<RecordedProcess>,
    /// The swtpm of a QEMU start that has a TPM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swtpm: Option<RecordedProcess>,
}

impl StartRecord {
    /// A record of a start that has made no process yet.
    pub fn new(id: impl Into<String>, backend: VmTool) -> Self {
        Self {
            id: id.into(),
            backend,
            main: None,
            keeper: None,
            swtpm: None,
        }
    }

    pub fn load(path: &Path) -> Result<Self, VmError> {
        let bytes =
            std::fs::read(path).map_err(|e| VmError::Io(format!("{}: {e}", path.display())))?;
        serde_json::from_slice(&bytes).map_err(|e| VmError::Io(format!("{}: {e}", path.display())))
    }

    pub fn write_atomic(&self, path: &Path) -> Result<(), VmError> {
        let json = serde_json::to_vec_pretty(self)
            .map_err(|e| VmError::Io(format!("serialize start record: {e}")))?;
        crate::spec::write_atomic_0600(path, &json)
    }

    /// Write the record of `id`, replacing the record of an earlier start of the id.
    pub fn write(&self, paths: &VmPaths) -> Result<(), VmError> {
        self.write_atomic(&paths.start_record_path(&self.id))
    }

    /// Change the record of `id` and write it again. A start has one thread, so nothing changes
    /// the file between the read and the write. A record that is gone or unreadable stays so: the
    /// update is a best effort, and the record that the start wrote first still holds.
    pub fn update(paths: &VmPaths, id: &str, change: impl FnOnce(&mut StartRecord)) {
        let path = paths.start_record_path(id);
        if let Ok(mut record) = Self::load(&path) {
            change(&mut record);
            let _ = record.write_atomic(&path);
        }
    }

    /// Remove the record of `id`. A record that is not there is already removed. Returns whether
    /// the file is gone now.
    pub fn remove(paths: &VmPaths, id: &str) -> bool {
        match std::fs::remove_file(paths.start_record_path(id)) {
            Ok(()) => true,
            Err(e) => e.kind() == std::io::ErrorKind::NotFound,
        }
    }
}

/// The ids that have a start record in the state directory, in order.
pub fn ids(paths: &VmPaths) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(paths.vms_dir()) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.strip_suffix(SUFFIX).map(str::to_string)
        })
        .collect();
    ids.sort();
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(dir: &Path) -> VmPaths {
        VmPaths::new(dir.join("state"), dir.join("data"), dir.join("tmp"))
    }

    fn identity(start_time: u64) -> ProcessIdentity {
        ProcessIdentity {
            start_time,
            boot_session: "boot-a".into(),
        }
    }

    #[test]
    fn a_record_round_trips_with_each_process_it_names() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        std::fs::create_dir_all(paths.vms_dir()).unwrap();
        let record = StartRecord {
            main: Some(RecordedProcess {
                pid: 40,
                identity: Some(identity(1)),
            }),
            keeper: Some(RecordedProcess {
                pid: 41,
                identity: None,
            }),
            ..StartRecord::new("vm-1", VmTool::Tart)
        };
        record.write(&paths).unwrap();
        assert_eq!(
            StartRecord::load(&paths.start_record_path("vm-1")).unwrap(),
            record
        );
    }

    #[test]
    fn an_update_adds_a_process_and_leaves_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        std::fs::create_dir_all(paths.vms_dir()).unwrap();
        StartRecord::new("vm-1", VmTool::Tart)
            .write(&paths)
            .unwrap();
        StartRecord::update(&paths, "vm-1", |r| {
            r.main = Some(RecordedProcess {
                pid: 7,
                identity: Some(identity(3)),
            });
        });
        let record = StartRecord::load(&paths.start_record_path("vm-1")).unwrap();
        assert_eq!(record.main.unwrap().pid, 7);
        assert_eq!(record.backend, VmTool::Tart);
    }

    #[test]
    fn an_update_of_a_record_that_is_gone_writes_none() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        std::fs::create_dir_all(paths.vms_dir()).unwrap();
        StartRecord::update(&paths, "vm-1", |r| r.main = None);
        assert!(ids(&paths).is_empty());
    }

    #[test]
    fn removal_reports_a_file_that_is_gone_and_a_record_that_was_never_there_alike() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        std::fs::create_dir_all(paths.vms_dir()).unwrap();
        StartRecord::new("vm-1", VmTool::Qemu)
            .write(&paths)
            .unwrap();
        assert!(StartRecord::remove(&paths, "vm-1"));
        assert!(StartRecord::remove(&paths, "vm-1"));
        assert!(ids(&paths).is_empty());
    }

    #[test]
    fn the_ids_come_from_the_start_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        std::fs::create_dir_all(paths.vms_dir()).unwrap();
        for name in [
            "b.start-record",
            "a.start-record",
            "c.json",
            "c.meta.json",
            "a.start.json",
            "d.start.log",
        ] {
            std::fs::write(paths.vms_dir().join(name), b"{}").unwrap();
        }
        assert_eq!(ids(&paths), ["a", "b"]);
    }
}
