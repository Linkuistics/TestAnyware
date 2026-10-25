//! The own holders that the state directory shows (spec "admission-queue": "The count").
//!
//! A running macOS guest that has a meta holds a slot while the check of the meta's `tart run`
//! finds that process. The check is the one that adoption makes. A meta that records no identity
//! counts, because no check can show that its `tart run` is gone.
//!
//! The other own holders are the starts that the app admitted, and the queue keeps those itself.

use std::collections::BTreeSet;

use testanyware_vm::adoption::{decide, Adoption};
use testanyware_vm::identity::ProcessProbe;
use testanyware_vm::meta::{VmMeta, VmTool};
use testanyware_vm::spec::VmSpec;
use testanyware_vm::VmPaths;

use crate::adoption::meta_ids;

/// The ids of the macOS guests that hold a slot because they have a meta. This reads files and
/// asks `probe` about processes, and it changes nothing.
pub fn macos_guests_with_a_meta(paths: &VmPaths, probe: &impl ProcessProbe) -> BTreeSet<String> {
    meta_ids(paths)
        .into_iter()
        .filter(|id| holds_a_slot(id, paths, probe))
        .collect()
}

fn holds_a_slot(id: &str, paths: &VmPaths, probe: &impl ProcessProbe) -> bool {
    let Ok(meta) = VmMeta::load(&paths.meta_path(id)) else {
        // A torn meta proves nothing about a guest, and adoption does not adopt it either.
        return false;
    };
    // Only tart runs macOS guests, and tart runs Linux guests too. The framework counts
    // macOS guests only. A spec that does not read leaves the platform unknown, and an
    // unknown guest counts, because a count that is too high frees the slot of no live guest.
    if meta.tool != VmTool::Tart {
        return false;
    }
    let other_platform = VmSpec::load(&paths.spec_path(id))
        .ok()
        .is_some_and(|spec| spec.platform != "macos");
    if other_platform {
        return false;
    }
    matches!(decide(&meta, probe), Adoption::Adopt | Adoption::Legacy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use testanyware_vm::identity::ProcessIdentity;
    use testanyware_vm::spec::VncEndpoint;

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

    fn write_vm(
        paths: &VmPaths,
        id: &str,
        tool: VmTool,
        pid: i32,
        identity: Option<ProcessIdentity>,
        platform: Option<&str>,
    ) {
        std::fs::create_dir_all(paths.vms_dir()).unwrap();
        if let Some(platform) = platform {
            let spec = VmSpec {
                vnc: VncEndpoint {
                    host: "127.0.0.1".into(),
                    port: 5900,
                    password: None,
                },
                agent: None,
                platform: platform.into(),
                logical: None,
            };
            spec.write_atomic(&paths.spec_path(id)).unwrap();
        }
        VmMeta {
            identity,
            ..VmMeta::new(id, tool, pid)
        }
        .write_atomic(&paths.meta_path(id))
        .unwrap();
    }

    #[test]
    fn a_running_macos_guest_with_a_meta_counts() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        write_vm(
            &paths,
            "mac",
            VmTool::Tart,
            10,
            Some(identity(1)),
            Some("macos"),
        );
        let host = Host([(10, 1)].into());
        assert_eq!(
            macos_guests_with_a_meta(&paths, &host),
            BTreeSet::from(["mac".to_string()])
        );
    }

    #[test]
    fn a_guest_whose_tart_run_is_gone_or_replaced_does_not_count() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        write_vm(
            &paths,
            "gone",
            VmTool::Tart,
            11,
            Some(identity(1)),
            Some("macos"),
        );
        write_vm(
            &paths,
            "reused",
            VmTool::Tart,
            12,
            Some(identity(1)),
            Some("macos"),
        );
        // pid 11 is gone, and pid 12 started at another time.
        let host = Host([(12, 99)].into());
        assert!(macos_guests_with_a_meta(&paths, &host).is_empty());
    }

    #[test]
    fn a_meta_without_an_identity_counts_because_no_check_can_clear_it() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        write_vm(&paths, "old", VmTool::Tart, 13, None, Some("macos"));
        let host = Host(Default::default());
        assert_eq!(macos_guests_with_a_meta(&paths, &host).len(), 1);
    }

    #[test]
    fn a_linux_guest_on_tart_and_a_qemu_guest_take_no_slot() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        write_vm(
            &paths,
            "lin",
            VmTool::Tart,
            14,
            Some(identity(1)),
            Some("linux"),
        );
        write_vm(
            &paths,
            "win",
            VmTool::Qemu,
            15,
            Some(identity(1)),
            Some("windows"),
        );
        let host = Host([(14, 1), (15, 1)].into());
        assert!(macos_guests_with_a_meta(&paths, &host).is_empty());
    }

    #[test]
    fn a_tart_guest_whose_spec_does_not_read_counts_as_a_macos_guest() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        write_vm(&paths, "nospec", VmTool::Tart, 16, Some(identity(1)), None);
        let host = Host([(16, 1)].into());
        assert_eq!(macos_guests_with_a_meta(&paths, &host).len(), 1);
    }

    #[test]
    fn a_torn_meta_does_not_count() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        std::fs::create_dir_all(paths.vms_dir()).unwrap();
        std::fs::write(paths.meta_path("torn"), b"{ not json").unwrap();
        assert!(macos_guests_with_a_meta(&paths, &Host(Default::default())).is_empty());
    }
}
