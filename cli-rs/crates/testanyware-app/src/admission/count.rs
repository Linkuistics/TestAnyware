//! The own holders that the state directory shows (spec "admission-queue": "The count").
//!
//! A running macOS guest that has a meta holds a slot while the check of the meta's `tart run`
//! finds that process. The check is the one that adoption makes. A meta that records no identity
//! counts, because no check can show that its `tart run` is gone.
//!
//! The other own holders are the starts that the app admitted, and the queue keeps those itself.
//!
//! The seen holders are the other macOS guests that the last answer of tart reports running.
//! [`holders`] puts both together, and a guest that two holders name counts once.

use std::collections::BTreeSet;
use std::time::Duration;

use testanyware_vm::adoption::{decide, Adoption};
use testanyware_vm::identity::ProcessProbe;
use testanyware_vm::meta::{VmMeta, VmTool};
use testanyware_vm::spec::VmSpec;
use testanyware_vm::VmPaths;

use super::observation::ReadFailed;
use crate::adoption::meta_ids;

/// The ids of the macOS guests that hold a slot because they have a meta. This reads files and
/// asks `probe` about processes, and it changes nothing.
pub fn macos_guests_with_a_meta(paths: &VmPaths, probe: &impl ProcessProbe) -> BTreeSet<String> {
    meta_ids(paths)
        .into_iter()
        .filter(|id| holds_a_slot(id, paths, probe))
        .collect()
}

/// The own holders and the seen holders of one moment.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Holders {
    /// The guests that a meta holds and the starts that the app admitted. A guest that a golden
    /// creation names is not among them: the hold of the golden creation counts it.
    pub own: BTreeSet<String>,
    /// The golden creations that the app admitted, each with the names of the VMs that it boots.
    /// Each one is one own holder, whatever the number of its names.
    pub golden: Vec<Vec<String>>,
    /// The macOS guests of the last answer that no own holder names.
    pub seen: BTreeSet<String>,
}

impl Holders {
    /// The number of own holders: the guests, and the golden creations. This is the number that
    /// counts against `macos_slots` and against the two slots of Apple.
    pub fn own_count(&self) -> usize {
        self.own.len() + self.golden.len()
    }

    /// The name of each holder, own and seen, in order.
    pub fn names(&self) -> Vec<String> {
        let mut names: BTreeSet<String> = self.own.union(&self.seen).cloned().collect();
        names.extend(self.golden.iter().flatten().cloned());
        names.into_iter().collect()
    }
}

/// Put the own holders and the last answer of tart together. A guest that two holders name counts
/// once, as an own holder: an admitted start names its VM id, and so does the meta of the guest.
/// A golden creation counts as one own holder and names each VM that it boots (spec
/// "admission-queue": "The count"). A guest that it names is its own, so the answer of tart makes
/// it no seen holder, and no meta makes it a second own holder.
///
/// The exit of a `tart run` removes no name from the answer, so a guest whose own hold has ended
/// stays in the count as a seen holder until a later answer drops it. Before the first answer,
/// `answer` is `None` and no guest is seen.
pub fn holders(
    own: BTreeSet<String>,
    goldens: &[Vec<String>],
    answer: Option<&BTreeSet<String>>,
) -> Holders {
    let named_by_a_golden: BTreeSet<&String> = goldens.iter().flatten().collect();
    let own: BTreeSet<String> = own
        .into_iter()
        .filter(|name| !named_by_a_golden.contains(name))
        .collect();
    let seen = answer
        .map(|macos| {
            macos
                .iter()
                .filter(|name| !own.contains(*name) && !named_by_a_golden.contains(name))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    Holders {
        own,
        golden: goldens.to_vec(),
        seen,
    }
}

/// Whether the read of a stop removes `vm` from the last answer (spec "admission-queue": "The
/// count": "When tart does not answer").
///
/// The answer ended at `answered_at`, when its last call to tart returned. The read is a `tart
/// list` that started at `read_started`, after the stop. It removes the VM only when it started
/// after the answer ended, and it answered, and it does not report the VM running. A read that
/// started earlier can predate the guest that the answer shows, and a read that failed proves
/// nothing. The read of a stop is no observation, so the caller changes neither the time of the
/// answer nor the time since tart did not answer.
pub fn stop_read_removes(
    answered_at: Duration,
    read_started: Duration,
    read: &Result<BTreeSet<String>, ReadFailed>,
    vm: &str,
) -> bool {
    read_started > answered_at && read.as_ref().is_ok_and(|running| !running.contains(vm))
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

    fn names(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn a_name_that_two_holders_share_counts_once_as_an_own_holder() {
        // The meta of `a` and the answer of tart both name it. An admitted start `b` is named
        // by the answer too, once its `tart run` has started.
        let held = holders(names(&["a", "b"]), &[], Some(&names(&["a", "b", "c"])));
        assert_eq!(held.own, names(&["a", "b"]));
        assert_eq!(held.seen, names(&["c"]));
        assert_eq!(held.names(), ["a", "b", "c"]);
    }

    #[test]
    fn a_golden_creation_is_one_own_holder_whatever_the_number_of_its_names() {
        let goldens = [vec!["setup".to_string(), "base-building".to_string()]];
        let held = holders(BTreeSet::new(), &goldens, Some(&BTreeSet::new()));
        assert_eq!(held.own_count(), 1);
        assert_eq!(held.names(), ["base-building", "setup"]);
    }

    #[test]
    fn a_guest_that_a_golden_creation_names_is_neither_seen_nor_a_second_own_holder() {
        // tart reports the setup VM running, and a meta names it as well. The golden creation
        // holds the one slot of the three names.
        let goldens = [vec!["setup".to_string()]];
        let held = holders(
            names(&["setup", "mac"]),
            &goldens,
            Some(&names(&["setup", "mac", "outside"])),
        );
        assert_eq!(held.own, names(&["mac"]));
        assert_eq!(held.seen, names(&["outside"]));
        assert_eq!(held.own_count(), 2);
        assert_eq!(held.names(), ["mac", "outside", "setup"]);
    }

    #[test]
    fn a_golden_creation_between_its_boots_holds_a_slot_beside_no_running_guest() {
        // The setup VM does not run between two boots, so the answer of tart names nothing.
        let goldens = [vec!["setup".to_string()]];
        let held = holders(BTreeSet::new(), &goldens, Some(&BTreeSet::new()));
        assert_eq!(held.own_count(), 1);
        assert!(held.seen.is_empty());
    }

    #[test]
    fn a_guest_that_no_own_holder_names_is_a_seen_holder() {
        let held = holders(BTreeSet::new(), &[], Some(&names(&["outside"])));
        assert!(held.own.is_empty());
        assert_eq!(held.seen, names(&["outside"]));
    }

    #[test]
    fn before_the_first_answer_no_guest_is_seen() {
        let held = holders(names(&["a"]), &[], None);
        assert_eq!(held.own, names(&["a"]));
        assert!(held.seen.is_empty());
    }

    #[test]
    fn a_meta_whose_tart_run_is_gone_beside_a_running_vm_of_its_name_counts_as_seen() {
        // The meta of `mac` records pid 10, which is gone. A person ran another VM under the
        // name, and tart reports it running. It is no own holder, and it is a seen holder.
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
        let own = macos_guests_with_a_meta(&paths, &Host(Default::default()));
        assert!(own.is_empty());
        let held = holders(own, &[], Some(&names(&["mac"])));
        assert!(held.own.is_empty());
        assert_eq!(held.seen, names(&["mac"]));
    }

    #[test]
    fn the_exit_of_a_tart_run_removes_no_vm_from_the_count() {
        // The hold of the admission ends when the `tart run` exits, and the answer of tart
        // still names the VM, which can be a replacement under the same name. It stays in the
        // count, as a seen holder, until a later answer drops it.
        let before = holders(names(&["a"]), &[], Some(&names(&["a"])));
        assert_eq!(before.own.len() + before.seen.len(), 1);
        let after_the_exit = holders(BTreeSet::new(), &[], Some(&names(&["a"])));
        assert_eq!(after_the_exit.seen, names(&["a"]));
        let later_answer = holders(BTreeSet::new(), &[], Some(&BTreeSet::new()));
        assert!(later_answer.own.is_empty() && later_answer.seen.is_empty());
    }

    #[test]
    fn the_metas_of_stopped_vms_after_a_restart_of_the_mac_hold_no_slot() {
        // The Mac restarted, so no recorded process outlives it: the boot session of each meta
        // is another one. tart reports each VM stopped, so the answer names no guest.
        struct NewBoot;
        impl ProcessProbe for NewBoot {
            fn start_time(&self, pid: i32) -> Option<u64> {
                // The pids may be alive again as other processes, with other start times.
                (pid > 0).then_some(777)
            }
            fn boot_session(&self) -> Option<String> {
                Some("boot-b".into())
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        write_vm(
            &paths,
            "one",
            VmTool::Tart,
            10,
            Some(identity(1)),
            Some("macos"),
        );
        write_vm(
            &paths,
            "two",
            VmTool::Tart,
            11,
            Some(identity(1)),
            Some("macos"),
        );
        let own = macos_guests_with_a_meta(&paths, &NewBoot);
        let held = holders(own, &[], Some(&BTreeSet::new()));
        assert!(held.own.is_empty() && held.seen.is_empty(), "{held:?}");
    }

    #[test]
    fn a_torn_meta_does_not_count() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        std::fs::create_dir_all(paths.vms_dir()).unwrap();
        std::fs::write(paths.meta_path("torn"), b"{ not json").unwrap();
        assert!(macos_guests_with_a_meta(&paths, &Host(Default::default())).is_empty());
    }

    // The read of a stop (spec "admission-queue": "The count").

    fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    fn answered(list: &[&str]) -> Result<BTreeSet<String>, ReadFailed> {
        Ok(names(list))
    }

    #[test]
    fn a_stop_read_that_started_after_the_answer_and_does_not_name_the_vm_removes_it() {
        assert!(stop_read_removes(
            secs(10),
            secs(11),
            &answered(&["other"]),
            "a"
        ));
        assert!(stop_read_removes(secs(10), secs(11), &answered(&[]), "a"));
    }

    #[test]
    fn a_stop_read_that_started_before_the_end_of_the_answer_removes_nothing() {
        // The answer ended at 10 s. A read that began at 9 s can predate a guest that the answer
        // shows, so it is not later. A read that began at the same moment is not later either.
        assert!(!stop_read_removes(secs(10), secs(9), &answered(&[]), "a"));
        assert!(!stop_read_removes(secs(10), secs(10), &answered(&[]), "a"));
    }

    #[test]
    fn a_stop_read_that_fails_proves_nothing() {
        let failed = Err(ReadFailed("tart list failed".into()));
        assert!(!stop_read_removes(secs(10), secs(11), &failed, "a"));
    }

    #[test]
    fn a_stop_read_that_still_names_the_vm_removes_nothing() {
        assert!(!stop_read_removes(
            secs(10),
            secs(11),
            &answered(&["a"]),
            "a"
        ));
    }
}
