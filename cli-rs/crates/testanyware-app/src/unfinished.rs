//! The decision for a start record that the app finds when it starts (spec "admission-queue":
//! "A quit, a crash and a restart").
//!
//! A start record names a start that a crash of the app, or of the Mac, interrupted. No client
//! waits for that start. So before the app serves its socket, it decides for each record whether
//! its VM is to be removed, and it applies the decision.
//!
//! [`decide`] is the one pure function. Its inputs are facts: the identity check of each recorded
//! process, the state that tart reports for the VM, and the check of the meta's own `tart run`
//! when a meta names the VM. [`run`] gathers those facts and applies the verdict. The app never
//! signals a process that the record does not prove, and it never stops a running VM that no meta
//! proves.
//!
//! This module applies the first three rows of the table. The fourth row (tart does not answer)
//! keeps the VM and the record, and it removes the sidecars unless the meta proves the VM. The
//! decision again, once an observation of tart answers, is the observer's.

use std::path::Path;

use testanyware_app_protocol::UnfinishedStart;
use testanyware_vm::adoption::{check_process, ProcessCheck};
use testanyware_vm::identity::{ProcessProbe, SystemProbe};
use testanyware_vm::meta::{VmMeta, VmTool};
use testanyware_vm::start_record::{self, RecordedProcess, StartRecord};
use testanyware_vm::tools::ToolSearch;
use testanyware_vm::VmPaths;

use crate::admission::observation::{SystemTart, TartReader};

/// What tart reports for the VM of a record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TartState {
    /// `tart list` reports the VM running.
    Running,
    /// `tart list` answered and does not report the VM running: it is stopped, or tart names no
    /// such VM.
    NotRunning,
    /// tart did not answer within the bound.
    Unanswered,
}

/// The check of one recorded process against the live process at its pid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    /// The pid, the start time and the boot session all match.
    Matches,
    /// Nothing proves the process: the record names none, it names no identity, the process is
    /// gone, or the pid names another process.
    NotProven,
}

/// The check of the meta's own `tart run`, when a meta names the VM. It is the check that
/// adoption makes, and it needs no answer from tart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaRun {
    /// No meta names the VM.
    NoMeta,
    /// The meta's `tart run` matches in full.
    Matches,
    /// The meta's `tart run` is gone.
    Gone,
    /// The pid of the meta names another process.
    OtherProcess,
    /// The meta records no identity, as an older meta does, or it cannot be read. No check can
    /// prove that its `tart run` still runs.
    NoIdentity,
}

/// What the app finds for one record of a start on tart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    /// The recorded `tart run`. Only this process decides, because it is the VM.
    pub main: Recorded,
    /// The recorded keeper. It ends only when its next connect fails (ADR-0018), so a keeper that
    /// matches proves nothing about the VM.
    pub keeper: Recorded,
    pub tart: TartState,
    pub meta: MetaRun,
}

/// The row of the table that decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// The recorded `tart run` matches in full.
    First,
    /// No `tart run` matches, and tart reports the VM not running or no such VM.
    Second,
    /// No `tart run` matches, and tart reports the VM running.
    Third,
    /// tart does not answer.
    Fourth,
}

/// What the app does to the VM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmAction {
    /// Stop the recorded `tart run` and delete the VM. The keeper gets a signal only when it
    /// matches too.
    StopAndDelete { signal_keeper: bool },
    /// Delete the VM by name, with no signal.
    DeleteByName,
    /// Keep the VM, and never stop it.
    Keep,
}

/// The verdict for one record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verdict {
    pub row: Row,
    pub vm: VmAction,
    /// The spec and the meta stay: the meta's own `tart run` matches in full, and adoption
    /// serves its VM.
    pub keep_sidecars: bool,
    /// The record goes. In the fourth row it stays, for the next decision.
    pub remove_record: bool,
    /// What `GET /v1/status` reports as `removed`, when the removals succeed.
    pub removed: bool,
}

/// Decide for the record of a start on tart. For a start on tart, the first row that holds
/// decides.
pub fn decide(facts: &Facts) -> Verdict {
    let meta_proves = facts.meta == MetaRun::Matches;
    if facts.main == Recorded::Matches {
        return Verdict {
            row: Row::First,
            vm: VmAction::StopAndDelete {
                signal_keeper: facts.keeper == Recorded::Matches,
            },
            keep_sidecars: false,
            remove_record: true,
            removed: true,
        };
    }
    match facts.tart {
        // A meta whose `tart run` matches in full proves a running process of this name. The
        // list that says otherwise is older than that check, or tart lies. The app deletes no VM
        // that a process it can prove still runs.
        TartState::NotRunning if !meta_proves => Verdict {
            row: Row::Second,
            vm: VmAction::DeleteByName,
            keep_sidecars: false,
            remove_record: true,
            removed: true,
        },
        TartState::NotRunning | TartState::Running => Verdict {
            row: Row::Third,
            vm: VmAction::Keep,
            keep_sidecars: meta_proves,
            remove_record: true,
            // With a meta that proves the VM, the start of the record left nothing else.
            removed: meta_proves,
        },
        TartState::Unanswered => Verdict {
            row: Row::Fourth,
            vm: VmAction::Keep,
            keep_sidecars: meta_proves,
            remove_record: false,
            removed: false,
        },
    }
}

/// The effects of a verdict on tart and on processes. A test records them.
pub trait Effects {
    /// Stop the `tart run` at `main`, delete the VM, and end the keeper at `keeper`. Returns
    /// whether the VM is gone.
    fn stop_and_delete(&self, id: &str, main: i32, keeper: Option<i32>) -> bool;
    /// Delete the VM by name, with no signal. Returns whether the VM is gone.
    fn delete_by_name(&self, id: &str) -> bool;
}

/// The effects on the real `tart`.
pub struct SystemEffects {
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    tools: ToolSearch,
}

impl SystemEffects {
    pub fn new(tools: ToolSearch) -> Self {
        Self { tools }
    }
}

#[cfg(target_os = "macos")]
impl Effects for SystemEffects {
    fn stop_and_delete(&self, id: &str, main: i32, keeper: Option<i32>) -> bool {
        testanyware_vm::tart::TartRunner::stop_in(&self.tools, id, main);
        testanyware_vm::keeper::stop_keeper(id, keeper);
        !testanyware_vm::tart::vm_exists_in(&self.tools, id)
    }

    fn delete_by_name(&self, id: &str) -> bool {
        if testanyware_vm::tart::vm_exists_in(&self.tools, id) {
            testanyware_vm::tart::remove_existing_in(&self.tools, id);
        }
        !testanyware_vm::tart::vm_exists_in(&self.tools, id)
    }
}

/// Only a macOS host runs tart, so no other host holds a record of a start on tart. The app
/// removes nothing there.
#[cfg(not(target_os = "macos"))]
impl Effects for SystemEffects {
    fn stop_and_delete(&self, _id: &str, _main: i32, _keeper: Option<i32>) -> bool {
        false
    }

    fn delete_by_name(&self, _id: &str) -> bool {
        false
    }
}

/// Decide for each start record in the state directory, and apply the verdict. The result of
/// each record goes to `GET /v1/status`. This reads files and asks `tart list` once, and it
/// changes the state directory and tart.
pub fn run(
    paths: &VmPaths,
    tart: &dyn TartReader,
    probe: &impl ProcessProbe,
    effects: &impl Effects,
) -> Vec<UnfinishedStart> {
    let ids = start_record::ids(paths);
    // One bounded `tart list` serves every record of a start on tart. A host that has none asks
    // tart nothing.
    let mut loaded: Vec<(String, Result<StartRecord, String>)> = ids
        .into_iter()
        .map(|id| {
            let record =
                StartRecord::load(&paths.start_record_path(&id)).map_err(|e| e.to_string());
            (id, record)
        })
        .collect();
    let has_tart = loaded
        .iter()
        .any(|(_, r)| matches!(r, Ok(r) if r.backend == VmTool::Tart));
    let listed = has_tart.then(|| tart.running());
    loaded
        .drain(..)
        .map(|(id, record)| match record {
            Err(detail) => UnfinishedStart {
                id,
                removed: false,
                detail: format!(
                    "The start record cannot be read: {detail}. The app keeps it and the VM. \
                     Remove the file when no start of this VM is interrupted."
                ),
            },
            Ok(record) => match record.backend {
                VmTool::Qemu => settle_qemu(paths, &record, probe),
                VmTool::Tart => {
                    let state = match &listed {
                        Some(Ok(running)) if running.contains(&record.id) => TartState::Running,
                        Some(Ok(_)) => TartState::NotRunning,
                        _ => TartState::Unanswered,
                    };
                    settle_tart(paths, &record, state, probe, effects)
                }
            },
        })
        .collect()
}

/// The facts for the record of a start on tart.
fn facts(
    paths: &VmPaths,
    record: &StartRecord,
    tart: TartState,
    probe: &impl ProcessProbe,
) -> Facts {
    let check = |process: &Option<RecordedProcess>| match process {
        Some(RecordedProcess {
            pid,
            identity: Some(identity),
        }) if check_process(probe, *pid, identity) == ProcessCheck::Match => Recorded::Matches,
        _ => Recorded::NotProven,
    };
    Facts {
        main: check(&record.main),
        keeper: check(&record.keeper),
        tart,
        meta: meta_run(paths, &record.id, probe),
    }
}

/// The check of the meta's own `tart run`.
fn meta_run(paths: &VmPaths, id: &str, probe: &impl ProcessProbe) -> MetaRun {
    let path = paths.meta_path(id);
    if !path.is_file() {
        return MetaRun::NoMeta;
    }
    let Ok(meta) = VmMeta::load(&path) else {
        return MetaRun::NoIdentity;
    };
    if meta.tool != VmTool::Tart {
        return MetaRun::OtherProcess;
    }
    let Some(recorded) = meta.identity.as_ref() else {
        return MetaRun::NoIdentity;
    };
    match check_process(probe, meta.pid, recorded) {
        ProcessCheck::Match => MetaRun::Matches,
        ProcessCheck::Gone => MetaRun::Gone,
        ProcessCheck::Other(_) => MetaRun::OtherProcess,
    }
}

fn settle_tart(
    paths: &VmPaths,
    record: &StartRecord,
    tart: TartState,
    probe: &impl ProcessProbe,
    effects: &impl Effects,
) -> UnfinishedStart {
    let id = record.id.clone();
    let facts = facts(paths, record, tart, probe);
    let verdict = decide(&facts);

    let vm_gone = match verdict.vm {
        VmAction::StopAndDelete { signal_keeper } => {
            let main = record.main.as_ref().map_or(0, |p| p.pid);
            let keeper = signal_keeper
                .then(|| record.keeper.as_ref().map(|p| p.pid))
                .flatten();
            Some(effects.stop_and_delete(&id, main, keeper))
        }
        VmAction::DeleteByName => Some(effects.delete_by_name(&id)),
        VmAction::Keep => None,
    };
    // A VM that stays after its removal keeps the record and the sidecars: the next start of the
    // app decides again, and `vm stop` can still find the VM.
    if vm_gone == Some(false) {
        return UnfinishedStart {
            id: id.clone(),
            removed: false,
            detail: format!(
                "The start of {id} was interrupted, and the app could not remove its VM. The app \
                 keeps the record and decides again at its next start. Remedy: `tart stop {id}` \
                 and `tart delete {id}`."
            ),
        };
    }

    let mut kept_by_failure = false;
    if !verdict.keep_sidecars {
        remove_sidecars(paths, &id);
    }
    if verdict.remove_record && !StartRecord::remove(paths, &id) {
        kept_by_failure = true;
    }

    let removed = verdict.removed && !kept_by_failure;
    UnfinishedStart {
        id: id.clone(),
        removed,
        detail: detail(&id, &facts, &verdict, kept_by_failure),
    }
}

/// The sentences of a result: the reason, and the remedy when a person has to act.
fn detail(id: &str, facts: &Facts, verdict: &Verdict, record_stays: bool) -> String {
    if record_stays && verdict.remove_record {
        return format!(
            "The start of {id} was interrupted, and the app could not remove its record. It \
             decides again at its next start."
        );
    }
    match verdict.row {
        Row::First => format!(
            "The start of {id} was interrupted. The `tart run` of the record still runs. The app \
             stopped it and removed the VM, its clone, its sidecars and the record."
        ),
        Row::Second => format!(
            "The start of {id} was interrupted. No `tart run` of the record runs, and tart \
             reports no running VM of that name. The app deleted the VM by name, with no signal, \
             and removed its sidecars and the record."
        ),
        Row::Third if verdict.keep_sidecars => format!(
            "The start of {id} was interrupted. The VM that runs under that name belongs to an \
             earlier start, whose `tart run` the meta proves. The app keeps it and serves it \
             again. The interrupted start left nothing else."
        ),
        Row::Third => format!(
            "The start of {id} was interrupted, and tart reports a running VM of that name. The \
             record does not prove the process that runs it. It can be a VM that a person made, \
             or the `tart run` of the start before the record named it. The app signals nothing, \
             keeps the VM, and removed the record{sidecars}. The VM counts as an external holder, \
             and `testanyware vm stop {id}` fails with VM_NOT_FOUND. Remedy: `tart stop {id}` \
             and `tart delete {id}`, when the VM is not your own.",
            sidecars = if facts.meta == MetaRun::NoMeta {
                ""
            } else {
                " and the sidecars that prove nothing"
            }
        ),
        Row::Fourth => format!(
            "The start of {id} was interrupted, and tart does not answer. The app keeps the VM \
             and the record{sidecars}, and decides again once an observation of tart answers.",
            sidecars = if verdict.keep_sidecars {
                ", and it serves the VM of the meta again"
            } else {
                ". It removed the sidecars that prove nothing"
            }
        ),
    }
}

fn remove_sidecars(paths: &VmPaths, id: &str) {
    let _ = std::fs::remove_file(paths.spec_path(id));
    let _ = std::fs::remove_file(paths.meta_path(id));
    let _ = std::fs::remove_dir_all(paths.vms_dir().join(format!("display-{id}")));
}

/// A start on QEMU has no tart name. The app removes it as the stop of the app does: it signals
/// the QEMU and the swtpm that the record proves, removes the clone, the session directory and
/// the sidecars, and asks tart nothing.
fn settle_qemu(
    paths: &VmPaths,
    record: &StartRecord,
    probe: &impl ProcessProbe,
) -> UnfinishedStart {
    let id = record.id.clone();
    let proven = |process: &Option<RecordedProcess>| {
        let process = process.as_ref()?;
        let identity = process.identity.as_ref()?;
        (check_process(probe, process.pid, identity) == ProcessCheck::Match).then_some(process.pid)
    };
    testanyware_vm::qemu::teardown_verified(
        proven(&record.main),
        proven(&record.swtpm),
        &paths.clone_dir(&id),
        &paths.session_dir(&id),
    );
    remove_sidecars(paths, &id);
    let gone = !Path::new(&paths.clone_dir(&id)).exists() && StartRecord::remove(paths, &id);
    UnfinishedStart {
        id: id.clone(),
        removed: gone,
        detail: if gone {
            format!(
                "The start of {id} on QEMU was interrupted. The app stopped the processes that \
                 the record proves, and removed the clone, the sidecars and the record."
            )
        } else {
            format!(
                "The start of {id} on QEMU was interrupted, and the app could not remove all of \
                 it. It keeps the record and decides again at its next start."
            )
        },
    }
}

/// Decide for every start record of the app, as the app starts. The result is what `GET
/// /v1/status` reports.
pub fn run_for_app(ctx: &crate::AppContext) -> Vec<UnfinishedStart> {
    run(
        &crate::vms::vm_paths(ctx),
        &SystemTart::new(ctx.tools.clone()),
        &SystemProbe,
        &SystemEffects::new(ctx.tools.clone()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::admission::observation::fake::FakeTart;
    use std::cell::RefCell;
    use testanyware_vm::identity::ProcessIdentity;

    fn facts(main: Recorded, keeper: Recorded, tart: TartState, meta: MetaRun) -> Facts {
        Facts {
            main,
            keeper,
            tart,
            meta,
        }
    }

    const METAS: [MetaRun; 5] = [
        MetaRun::NoMeta,
        MetaRun::Matches,
        MetaRun::Gone,
        MetaRun::OtherProcess,
        MetaRun::NoIdentity,
    ];
    const STATES: [TartState; 3] = [
        TartState::Running,
        TartState::NotRunning,
        TartState::Unanswered,
    ];

    #[test]
    fn the_first_row_holds_for_a_matching_tart_run_whatever_tart_and_the_meta_say() {
        for tart in STATES {
            for meta in METAS {
                let verdict = decide(&facts(Recorded::Matches, Recorded::NotProven, tart, meta));
                assert_eq!(verdict.row, Row::First, "{tart:?} {meta:?}");
                assert_eq!(
                    verdict.vm,
                    VmAction::StopAndDelete {
                        signal_keeper: false
                    }
                );
                assert!(!verdict.keep_sidecars && verdict.remove_record && verdict.removed);
            }
        }
    }

    #[test]
    fn the_first_row_signals_a_keeper_only_when_the_keeper_matches_too() {
        let verdict = decide(&facts(
            Recorded::Matches,
            Recorded::Matches,
            TartState::Running,
            MetaRun::NoMeta,
        ));
        assert_eq!(
            verdict.vm,
            VmAction::StopAndDelete {
                signal_keeper: true
            }
        );
    }

    #[test]
    fn a_keeper_that_matches_while_its_tart_run_does_not_proves_nothing_about_the_vm() {
        // The keeper ends only when its next connect fails, so it can outlive a `tart run`. In
        // each state the VM gets no signal and no stop, and the keeper none either.
        for tart in STATES {
            let verdict = decide(&facts(
                Recorded::NotProven,
                Recorded::Matches,
                tart,
                MetaRun::NoMeta,
            ));
            assert_ne!(verdict.row, Row::First, "{tart:?}");
            assert!(
                !matches!(verdict.vm, VmAction::StopAndDelete { .. }),
                "{tart:?}: {verdict:?}"
            );
        }
        let verdict = decide(&facts(
            Recorded::NotProven,
            Recorded::Matches,
            TartState::NotRunning,
            MetaRun::NoMeta,
        ));
        assert_eq!(
            verdict.vm,
            VmAction::DeleteByName,
            "no signal to the keeper"
        );
    }

    #[test]
    fn the_second_row_deletes_by_name_when_tart_reports_no_running_vm() {
        let verdict = decide(&facts(
            Recorded::NotProven,
            Recorded::NotProven,
            TartState::NotRunning,
            MetaRun::NoMeta,
        ));
        assert_eq!(verdict.row, Row::Second);
        assert_eq!(verdict.vm, VmAction::DeleteByName);
        assert!(!verdict.keep_sidecars && verdict.remove_record && verdict.removed);
    }

    #[test]
    fn the_third_row_keeps_a_running_vm_that_no_meta_proves_and_reports_removed_false() {
        for meta in [
            MetaRun::NoMeta,
            MetaRun::Gone,
            MetaRun::OtherProcess,
            MetaRun::NoIdentity,
        ] {
            let verdict = decide(&facts(
                Recorded::NotProven,
                Recorded::NotProven,
                TartState::Running,
                meta,
            ));
            assert_eq!(verdict.row, Row::Third, "{meta:?}");
            assert_eq!(verdict.vm, VmAction::Keep);
            assert!(verdict.remove_record, "{meta:?}");
            assert!(
                !verdict.keep_sidecars,
                "{meta:?}: a meta that proves nothing goes"
            );
            assert!(!verdict.removed, "{meta:?}");
        }
    }

    #[test]
    fn the_third_row_keeps_the_meta_of_an_earlier_start_whose_tart_run_matches() {
        let verdict = decide(&facts(
            Recorded::NotProven,
            Recorded::NotProven,
            TartState::Running,
            MetaRun::Matches,
        ));
        assert_eq!(verdict.row, Row::Third);
        assert!(verdict.keep_sidecars && verdict.remove_record);
        assert!(verdict.removed, "the start of the record left nothing else");
    }

    #[test]
    fn a_meta_that_proves_its_tart_run_stops_the_second_row_from_deleting_that_vm() {
        let verdict = decide(&facts(
            Recorded::NotProven,
            Recorded::NotProven,
            TartState::NotRunning,
            MetaRun::Matches,
        ));
        assert_eq!(verdict.vm, VmAction::Keep);
        assert!(verdict.keep_sidecars);
    }

    #[test]
    fn the_fourth_row_keeps_the_vm_and_the_record_in_each_outcome_of_the_check_of_the_meta() {
        for meta in METAS {
            let verdict = decide(&facts(
                Recorded::NotProven,
                Recorded::NotProven,
                TartState::Unanswered,
                meta,
            ));
            assert_eq!(verdict.row, Row::Fourth, "{meta:?}");
            assert_eq!(verdict.vm, VmAction::Keep);
            assert!(
                !verdict.remove_record && !verdict.removed,
                "removed is false for each outcome"
            );
            assert_eq!(verdict.keep_sidecars, meta == MetaRun::Matches, "{meta:?}");
        }
    }

    #[test]
    fn an_older_meta_beside_a_running_vm_goes_in_the_third_and_the_fourth_row() {
        for tart in [TartState::Running, TartState::Unanswered] {
            let verdict = decide(&facts(
                Recorded::NotProven,
                Recorded::NotProven,
                tart,
                MetaRun::NoIdentity,
            ));
            assert!(!verdict.keep_sidecars, "{tart:?}");
            assert!(!verdict.removed, "{tart:?}");
        }
    }

    // ----- the procedure over a state directory -----

    /// The processes that a test makes live, by pid and start time. The boot session is fixed.
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

    #[derive(Default)]
    struct Recording {
        calls: RefCell<Vec<String>>,
        stays: bool,
    }

    impl Effects for Recording {
        fn stop_and_delete(&self, id: &str, main: i32, keeper: Option<i32>) -> bool {
            self.calls
                .borrow_mut()
                .push(format!("stop {id} {main} {keeper:?}"));
            !self.stays
        }
        fn delete_by_name(&self, id: &str) -> bool {
            self.calls.borrow_mut().push(format!("delete {id}"));
            !self.stays
        }
    }

    fn paths(dir: &Path) -> VmPaths {
        let paths = VmPaths::new(dir.join("state"), dir.join("data"), dir.join("tmp"));
        std::fs::create_dir_all(paths.vms_dir()).unwrap();
        paths
    }

    fn record(id: &str, main: Option<(i32, u64)>, keeper: Option<(i32, u64)>) -> StartRecord {
        let process = |(pid, start)| RecordedProcess {
            pid,
            identity: Some(identity(start)),
        };
        StartRecord {
            main: main.map(process),
            keeper: keeper.map(process),
            ..StartRecord::new(id, VmTool::Tart)
        }
    }

    fn sidecars(paths: &VmPaths, id: &str, meta: VmMeta) {
        std::fs::write(paths.spec_path(id), b"{}").unwrap();
        meta.write_atomic(&paths.meta_path(id)).unwrap();
    }

    fn meta_of(id: &str, pid: i32, start: Option<u64>) -> VmMeta {
        VmMeta {
            identity: start.map(identity),
            ..VmMeta::new(id, VmTool::Tart, pid)
        }
    }

    fn files(paths: &VmPaths) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(paths.vms_dir())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn only(results: Vec<UnfinishedStart>) -> UnfinishedStart {
        assert_eq!(results.len(), 1, "{results:?}");
        results.into_iter().next().unwrap()
    }

    #[test]
    fn a_record_whose_tart_run_matches_is_stopped_and_removed_with_its_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        record("v", Some((10, 1)), Some((11, 2)))
            .write(&paths)
            .unwrap();
        sidecars(&paths, "v", meta_of("v", 10, Some(1)));
        let effects = Recording::default();
        let tart = FakeTart::with(&[("v", "darwin")]);

        let result = only(run(
            &paths,
            &tart,
            &Host([(10, 1), (11, 2)].into()),
            &effects,
        ));

        assert!(result.removed, "{result:?}");
        assert_eq!(*effects.calls.borrow(), ["stop v 10 Some(11)"]);
        assert!(files(&paths).is_empty(), "{:?}", files(&paths));
    }

    #[test]
    fn a_keeper_that_matches_beside_a_tart_run_that_does_not_gets_no_signal() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        // The `tart run` is gone, the keeper lives, and tart reports no running VM.
        record("v", Some((10, 1)), Some((11, 2)))
            .write(&paths)
            .unwrap();
        let effects = Recording::default();
        let tart = FakeTart::default();

        let result = only(run(&paths, &tart, &Host([(11, 2)].into()), &effects));

        assert!(result.removed);
        assert_eq!(
            *effects.calls.borrow(),
            ["delete v"],
            "no `stop`, so no signal"
        );
    }

    #[test]
    fn a_record_beside_a_running_vm_that_no_meta_proves_keeps_the_vm_and_reports_removed_false() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        record("v", None, None).write(&paths).unwrap();
        sidecars(&paths, "v", meta_of("v", 99, None));
        let effects = Recording::default();
        let tart = FakeTart::with(&[("v", "darwin")]);

        let result = only(run(&paths, &tart, &Host(Default::default()), &effects));

        assert!(!result.removed, "{result:?}");
        assert!(result.detail.contains("tart stop v") && result.detail.contains("tart delete v"));
        assert!(effects.calls.borrow().is_empty(), "no stop and no delete");
        assert!(
            files(&paths).is_empty(),
            "the record and the sidecars that prove nothing go"
        );
    }

    #[test]
    fn a_record_beside_the_meta_of_an_earlier_start_keeps_that_meta_and_reports_removed() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        // The second start of the id wrote its record, and the crash came before its clone step
        // cleared the sidecars of the first start. The first start's `tart run` lives.
        record("v", None, None).write(&paths).unwrap();
        sidecars(&paths, "v", meta_of("v", 20, Some(5)));
        let effects = Recording::default();
        let tart = FakeTart::with(&[("v", "darwin")]);

        let result = only(run(&paths, &tart, &Host([(20, 5)].into()), &effects));

        assert!(result.removed, "{result:?}");
        assert!(effects.calls.borrow().is_empty());
        assert_eq!(files(&paths), ["v.json", "v.meta.json"]);
    }

    #[test]
    fn an_older_meta_beside_a_running_vm_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        record("v", None, None).write(&paths).unwrap();
        sidecars(&paths, "v", meta_of("v", 20, None));
        let effects = Recording::default();
        let tart = FakeTart::with(&[("v", "darwin")]);

        let result = only(run(&paths, &tart, &Host([(20, 5)].into()), &effects));

        assert!(!result.removed, "{result:?}");
        assert!(files(&paths).is_empty());
        assert!(effects.calls.borrow().is_empty());
    }

    #[test]
    fn a_tart_that_does_not_answer_keeps_the_vm_and_the_record() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        record("v", Some((10, 1)), None).write(&paths).unwrap();
        sidecars(&paths, "v", meta_of("v", 99, Some(7)));
        let effects = Recording::default();
        let tart = FakeTart::default();
        tart.fail_list(true);

        let result = only(run(&paths, &tart, &Host(Default::default()), &effects));

        assert!(!result.removed, "{result:?}");
        assert!(result.detail.contains("does not answer"));
        assert!(effects.calls.borrow().is_empty());
        assert_eq!(
            files(&paths),
            ["v.start-record"],
            "the sidecars that prove nothing go"
        );
    }

    #[test]
    fn a_vm_that_stays_after_its_removal_keeps_the_record_and_the_sidecars() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        record("v", Some((10, 1)), None).write(&paths).unwrap();
        sidecars(&paths, "v", meta_of("v", 10, Some(1)));
        let effects = Recording {
            stays: true,
            ..Recording::default()
        };
        let tart = FakeTart::with(&[("v", "darwin")]);

        let result = only(run(&paths, &tart, &Host([(10, 1)].into()), &effects));

        assert!(!result.removed, "{result:?}");
        assert_eq!(files(&paths), ["v.json", "v.meta.json", "v.start-record"]);
    }

    #[test]
    fn a_host_with_no_record_of_a_start_on_tart_asks_tart_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        let tart = FakeTart::default();
        assert!(run(
            &paths,
            &tart,
            &Host(Default::default()),
            &Recording::default()
        )
        .is_empty());
        assert_eq!(tart.list_calls(), 0);
    }

    #[test]
    fn a_record_that_cannot_be_read_stays_and_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        std::fs::write(paths.start_record_path("v"), b"{ not json").unwrap();
        let result = only(run(
            &paths,
            &FakeTart::default(),
            &Host(Default::default()),
            &Recording::default(),
        ));
        assert!(!result.removed);
        assert_eq!(files(&paths), ["v.start-record"]);
    }

    #[test]
    fn a_start_on_qemu_is_removed_as_the_stop_of_the_app_removes_it_and_asks_tart_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        StartRecord::new("q", VmTool::Qemu).write(&paths).unwrap();
        std::fs::create_dir_all(paths.clone_dir("q")).unwrap();
        std::fs::write(paths.clone_dir("q").join("q.qcow2"), b"x").unwrap();
        sidecars(&paths, "q", meta_of("q", 1, None));
        let tart = FakeTart::default();

        let result = only(run(
            &paths,
            &tart,
            &Host(Default::default()),
            &Recording::default(),
        ));

        assert!(result.removed, "{result:?}");
        assert_eq!(tart.list_calls(), 0);
        assert!(!paths.clone_dir("q").exists());
        assert!(files(&paths).is_empty());
    }
}
