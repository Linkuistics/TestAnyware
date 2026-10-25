//! One observation of tart: the macOS guests that run now (spec "admission-queue": "The count").
//!
//! `tart list` reports each VM of the user's tart storage and whether it runs, and it does not name
//! the OS. So an observation asks `tart get` for the OS of each VM that runs, and only a guest
//! whose OS is `darwin` counts. The app keeps no answer of `tart get` from one observation to the
//! next, because tart can replace a VM under the same name between two observations.
//!
//! The function [`observe`] reads tart through [`TartReader`], so a test sets every answer and
//! every failure. [`SystemTart`] is the reader that runs the real `tart`.

use std::collections::BTreeSet;

use testanyware_vm::tools::ToolSearch;

/// A call to tart failed, or printed what the app cannot read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadFailed(pub String);

impl std::fmt::Display for ReadFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What an observation reads from tart: two calls.
pub trait TartReader: Send + Sync {
    /// The names of the VMs that `tart list` reports running. A tart that does not answer is an
    /// error, so "no VM runs" is always an answer.
    fn running(&self) -> Result<BTreeSet<String>, ReadFailed>;

    /// The OS of one VM from `tart get`, such as `darwin` or `linux`.
    fn os_of(&self, name: &str) -> Result<String, ReadFailed>;
}

/// The macOS guests that tart reports running, or why the observation failed.
///
/// A failed `tart get` alone cannot tell a VM that has gone from a tart that fails. So the app
/// reads `tart list` again for that VM. The VM does not count when that list answers and does not
/// report it running. Otherwise the observation fails.
pub fn observe(tart: &dyn TartReader) -> Result<BTreeSet<String>, ReadFailed> {
    let mut macos = BTreeSet::new();
    for name in tart.running()? {
        match tart.os_of(&name) {
            Ok(os) => {
                if os == "darwin" {
                    macos.insert(name);
                }
            }
            Err(failed) => {
                if tart.running()?.contains(&name) {
                    return Err(failed);
                }
            }
        }
    }
    Ok(macos)
}

/// The reader that runs the `tart` that `tools` finds.
#[derive(Debug, Clone)]
pub struct SystemTart {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    tools: ToolSearch,
}

impl SystemTart {
    pub fn new(tools: ToolSearch) -> Self {
        Self { tools }
    }
}

#[cfg(target_os = "macos")]
impl TartReader for SystemTart {
    fn running(&self) -> Result<BTreeSet<String>, ReadFailed> {
        testanyware_vm::tart::running_names_strict_in(&self.tools)
            .map(|names| names.into_iter().collect())
            .map_err(ReadFailed)
    }

    fn os_of(&self, name: &str) -> Result<String, ReadFailed> {
        testanyware_vm::tart::os_of_in(&self.tools, name).map_err(ReadFailed)
    }
}

/// Only a macOS host runs tart, so no other host asks for a macOS slot. The reader fails there.
#[cfg(not(target_os = "macos"))]
impl TartReader for SystemTart {
    fn running(&self) -> Result<BTreeSet<String>, ReadFailed> {
        Err(ReadFailed("tart runs on a macOS host only".into()))
    }

    fn os_of(&self, _name: &str) -> Result<String, ReadFailed> {
        Err(ReadFailed("tart runs on a macOS host only".into()))
    }
}

#[cfg(test)]
pub(crate) mod fake {
    //! A tart whose answers a test sets.

    use super::*;
    use std::sync::{Condvar, Mutex};

    /// The VMs that run, with the OS of each, and the calls that fail.
    #[derive(Default)]
    pub struct FakeTart {
        inner: Mutex<Inner>,
        released: Condvar,
    }

    #[derive(Default)]
    struct Inner {
        vms: Vec<(String, String)>,
        list_fails: bool,
        get_fails: BTreeSet<String>,
        /// A VM that `tart list` reports at the first call only, as a VM that stops between two.
        gone_after_first_list: BTreeSet<String>,
        lists: usize,
        /// While set, a call to `tart list` does not answer.
        hold: bool,
        /// The calls to `tart list` so far, answered or not.
        calls: usize,
    }

    impl FakeTart {
        pub fn with(vms: &[(&str, &str)]) -> Self {
            let tart = Self::default();
            tart.set(vms);
            tart
        }

        pub fn set(&self, vms: &[(&str, &str)]) {
            self.inner.lock().unwrap().vms = vms
                .iter()
                .map(|(name, os)| (name.to_string(), os.to_string()))
                .collect();
        }

        pub fn fail_list(&self, fails: bool) {
            self.inner.lock().unwrap().list_fails = fails;
        }

        pub fn fail_get_of(&self, name: &str) {
            self.inner.lock().unwrap().get_fails.insert(name.into());
        }

        /// Make `tart list` hold its answer until `hold_lists(false)`, as a slow tart does.
        pub fn hold_lists(&self, hold: bool) {
            self.inner.lock().unwrap().hold = hold;
            if !hold {
                self.released.notify_all();
            }
        }

        /// The calls to `tart list` so far, those that hold their answer included.
        pub fn list_calls(&self) -> usize {
            self.inner.lock().unwrap().calls
        }

        pub fn drop_from_the_second_list(&self, name: &str) {
            self.inner
                .lock()
                .unwrap()
                .gone_after_first_list
                .insert(name.into());
        }
    }

    impl TartReader for FakeTart {
        fn running(&self) -> Result<BTreeSet<String>, ReadFailed> {
            let mut inner = self.inner.lock().unwrap();
            inner.calls += 1;
            while inner.hold {
                inner = self.released.wait(inner).unwrap();
            }
            if inner.list_fails {
                return Err(ReadFailed("tart list failed".into()));
            }
            inner.lists += 1;
            let second_or_later = inner.lists > 1;
            Ok(inner
                .vms
                .iter()
                .map(|(name, _)| name.clone())
                .filter(|name| !(second_or_later && inner.gone_after_first_list.contains(name)))
                .collect())
        }

        fn os_of(&self, name: &str) -> Result<String, ReadFailed> {
            let inner = self.inner.lock().unwrap();
            if inner.get_fails.contains(name) {
                return Err(ReadFailed(format!("tart get {name} failed")));
            }
            inner
                .vms
                .iter()
                .find(|(vm, _)| vm == name)
                .map(|(_, os)| os.clone())
                .ok_or_else(|| ReadFailed(format!("tart get {name}: no such VM")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::FakeTart;
    use super::*;

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn only_a_guest_whose_os_is_darwin_counts() {
        let tart = FakeTart::with(&[("mac", "darwin"), ("lin", "linux"), ("mac-2", "darwin")]);
        assert_eq!(observe(&tart), Ok(set(&["mac", "mac-2"])));
    }

    #[test]
    fn a_tart_with_no_running_vm_answers_with_an_empty_set() {
        assert_eq!(observe(&FakeTart::default()), Ok(BTreeSet::new()));
    }

    #[test]
    fn a_list_that_fails_fails_the_observation() {
        let tart = FakeTart::with(&[("mac", "darwin")]);
        tart.fail_list(true);
        assert!(observe(&tart).is_err());
    }

    #[test]
    fn the_app_keeps_no_os_between_observations_so_a_replaced_vm_counts_by_its_new_os() {
        // Each list names `vm`. tart replaced the macOS guest with a Linux guest under the same
        // name, so the second observation asks `tart get` again and finds Linux.
        let tart = FakeTart::with(&[("vm", "darwin")]);
        assert_eq!(observe(&tart), Ok(set(&["vm"])));
        tart.set(&[("vm", "linux")]);
        assert_eq!(observe(&tart), Ok(BTreeSet::new()));
        tart.set(&[("vm", "darwin")]);
        assert_eq!(observe(&tart), Ok(set(&["vm"])));
    }

    #[test]
    fn a_failed_get_for_a_vm_that_the_new_list_no_longer_reports_leaves_it_out() {
        let tart = FakeTart::with(&[("gone", "darwin"), ("mac", "darwin")]);
        tart.fail_get_of("gone");
        tart.drop_from_the_second_list("gone");
        assert_eq!(observe(&tart), Ok(set(&["mac"])));
    }

    #[test]
    fn a_failed_get_for_a_vm_that_the_new_list_still_reports_fails_the_observation() {
        let tart = FakeTart::with(&[("mac", "darwin")]);
        tart.fail_get_of("mac");
        assert!(observe(&tart).is_err());
    }

    #[test]
    fn a_failed_get_whose_second_list_fails_fails_the_observation() {
        struct ListsOnce(std::sync::atomic::AtomicUsize);
        impl TartReader for ListsOnce {
            fn running(&self) -> Result<BTreeSet<String>, ReadFailed> {
                if self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                    Ok(set(&["mac"]))
                } else {
                    Err(ReadFailed("tart list failed".into()))
                }
            }
            fn os_of(&self, _: &str) -> Result<String, ReadFailed> {
                Err(ReadFailed("tart get failed".into()))
            }
        }
        assert!(observe(&ListsOnce(Default::default())).is_err());
    }
}
