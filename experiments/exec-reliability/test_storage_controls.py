"""Required storage controls from docs/specs/diagnostic-evidence-storage.md.

Each control asserts the final behaviour of the spec. On the build before the
evidence writer, every control fails, and a strict expected-failure marker
names the slice that removes it. The marker accepts only an assertion failure
or a watchdog expiry. Any other exception fails the test.

On this build a fault fixture stalls or breaks the storage functions that the
controller calls today: write_json, the stream opens at the start of _call,
_hashes and write_identity. A stall is a FIFO open with no peer, which a
signal can interrupt. The slice that removes a marker switches the fixture to
the writer wrapper and keeps the assertions unchanged. The adapters
storage_outcome and writer_descriptors are fixtures in the same sense.
"""

from __future__ import annotations

import errno
import json
import os
import signal
import stat
import sys
import time
import uuid
from collections.abc import Callable, Iterator, Mapping
from contextlib import contextmanager
from dataclasses import dataclass, fields, replace
from pathlib import Path
from typing import Literal

import pytest

import controller
from controller import Call, Plan, Resource, execute

HERE = Path(__file__).resolve().parent
PEER = HERE / "peer.py"

# Short phase bounds that each control declares through the plan. A field that
# this build lacks is left out, so the declaration is not a setup failure. The
# slices that add the bounds use these names, or they rename them here.
READINESS_BOUND = 1.0
INTENT_GATE = 0.5
PUBLICATION_WINDOW = 1.0
OUTBOX_CAP = 64 * 1024
REAP_GRACE = 2.0

# Watchdog bounds. The final build finishes each control within its bound:
# the declared phases above, the writer reap grace and about four seconds of
# fixture teardown. On this build a stalled storage call holds the controller
# until the watchdog expires, and that expiry is the recorded failure.
READINESS_WATCHDOG = READINESS_BOUND + REAP_GRACE + 2.0
RUN_WATCHDOG = 12.0

FLOOD_BYTES = 1024 * 1024
FLOOD = "import sys; sys.stdout.buffer.write(b'x' * int(sys.argv[1]))"
TOUCH = "import sys; open(sys.argv[1], 'x').close()"
DESCRIPTORS = """
import json, os, stat
null = os.stat(os.devnull).st_rdev
report = {}
for fd in range(1024):
    try:
        st = os.fstat(fd)
    except OSError:
        continue
    if stat.S_ISFIFO(st.st_mode):
        report[fd] = "fifo"
    elif stat.S_ISCHR(st.st_mode) and st.st_rdev == null:
        report[fd] = "null"
    elif stat.S_ISREG(st.st_mode):
        report[fd] = "regular"
    elif stat.S_ISDIR(st.st_mode):
        report[fd] = "directory"
    else:
        report[fd] = "other"
print(json.dumps(report))
"""


class WatchdogExpired(Exception):
    """A control exceeded the bound that the spec gives the run."""


def removed_by(slice_name: str, gap: str) -> pytest.MarkDecorator:
    return pytest.mark.xfail(
        strict=True,
        raises=(AssertionError, WatchdogExpired),
        reason=f"{slice_name} removes this marker: {gap}",
    )


@contextmanager
def watchdog(seconds: float, on_expiry: Callable[[], None]) -> Iterator[None]:
    # The controller thread is the only thread, as fork admission requires, so
    # the watchdog is a signal. PEP 475 retries an interrupted open only when
    # the handler returns, so the raise ends the stalled open.
    # https://docs.python.org/3.12/library/signal.html#signal.setitimer
    def expire(_signum: int, _frame: object) -> None:
        signal.setitimer(signal.ITIMER_REAL, 0)
        on_expiry()
        raise WatchdogExpired(f"the run exceeded its {seconds:g} s bound")

    previous = signal.signal(signal.SIGALRM, expire)
    signal.setitimer(signal.ITIMER_REAL, seconds)
    try:
        yield
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, previous)


class StorageFault:
    """One storage fault over the controller's current storage functions.

    The fault starts at the record named by `at`. With `persist`, every later
    storage call has the same fault, as with a stopped or dead writer. A stall
    blocks on a FIFO open with no peer. A break raises a broken pipe.
    """

    def __init__(
        self,
        monkeypatch: pytest.MonkeyPatch,
        fifo: Path,
        *,
        at: str,
        action: Literal["stall", "break", "observe"],
        persist: bool,
        after: bool = True,
        on_trigger: Callable[[], None] | None = None,
    ) -> None:
        self.fifo = fifo
        self.at = at
        self.action = action
        self.persist = persist
        self.after = after
        self.on_trigger = on_trigger
        self.active = False
        self.released = False
        self.seen: list[str] = []
        os.mkfifo(fifo)
        write_json = controller.write_json
        call = controller._call
        hashes = controller._hashes
        write_identity = controller.write_identity

        def faulty_write_json(path: Path, value: object) -> None:
            self.storage(path.name)
            write_json(path, value)
            self.stored(path.name)

        def faulty_call(
            selected: Call, root: Path, plan: Plan, inspection: controller._InspectionState
        ) -> dict[str, object]:
            self.storage(f"{selected.name}.stdout")
            return call(selected, root, plan, inspection)

        def faulty_hashes(subjects: tuple[Path, ...]) -> dict[str, str]:
            self.storage("subjects")
            return hashes(subjects)

        def faulty_identity(root: Path) -> None:
            self.storage("identity")
            write_identity(root)

        monkeypatch.setattr(controller, "write_json", faulty_write_json)
        monkeypatch.setattr(controller, "_call", faulty_call)
        monkeypatch.setattr(controller, "_hashes", faulty_hashes)
        monkeypatch.setattr(controller, "write_identity", faulty_identity)

    def storage(self, name: str) -> None:
        self.seen.append(name)
        if not self.after and name == self.at:
            self.trigger()
        if not self.active or self.released:
            return
        if not self.persist:
            self.active = False
        if self.action == "stall":
            # Opening a FIFO for reading blocks until a writer opens it.
            # https://man7.org/linux/man-pages/man7/fifo.7.html
            descriptor = os.open(self.fifo, os.O_RDONLY)
            os.close(descriptor)
        elif self.action == "break":
            raise BrokenPipeError(errno.EPIPE, os.strerror(errno.EPIPE))

    def stored(self, name: str) -> None:
        if self.after and name == self.at:
            self.trigger()

    def trigger(self) -> None:
        self.active = True
        if self.on_trigger is not None:
            self.on_trigger()

    def release(self) -> None:
        self.released = True


@dataclass(frozen=True)
class Run:
    returned: object
    raised: OSError | None
    elapsed: float
    directory: Path


@dataclass(frozen=True)
class StorageOutcome:
    publication: str
    writer_reaped: bool
    readiness: str
    first_failure: str | None
    drops: Mapping[str, int]


def storage_outcome(run: Run) -> StorageOutcome | None:
    """Return the terminal storage outcome that the controller reports.

    On this build execute returns only the run directory, so no outcome exists.
    The slice that adds the outcome binds this adapter to it.
    """
    storage = getattr(run.returned, "storage", None)
    return storage if isinstance(storage, StorageOutcome) else None


def writer_descriptors(run: Run) -> Mapping[int, str] | None:
    """Return the descriptor kinds that the writer reported, or None.

    No writer exists on this build. The slice that adds the writer wrapper
    report binds this adapter to it.
    """
    report = getattr(run.returned, "writer_descriptors", None)
    return report if isinstance(report, Mapping) else None


def declare(plan: Plan) -> Plan:
    bounds = {
        "readiness_bound": READINESS_BOUND,
        "intent_gate": INTENT_GATE,
        "publication_window": PUBLICATION_WINDOW,
        "outbox_cap": OUTBOX_CAP,
    }
    present = {field.name for field in fields(Plan)}
    return replace(plan, **{name: value for name, value in bounds.items() if name in present})


def peer(name: str, mode: str, *args: object, budget: float = 3) -> Call:
    return Call(name, (sys.executable, str(PEER), mode, *map(str, args)), budget)


def script(name: str, source: str, *args: object, budget: float = 3) -> Call:
    return Call(name, (sys.executable, "-I", "-c", source, *map(str, args)), budget)


def vm(tmp_path: Path, run_id: str, prefix: str = "") -> Resource:
    identity = f"diag-{run_id}-{prefix or 'first'}"

    def action(name: str, mode: str) -> Call:
        return peer(f"{prefix}{name}", mode, identity, tmp_path, budget=0.2)

    return Resource(
        "vm",
        identity,
        peer(f"{prefix}acquire", "acquire", identity, tmp_path),
        action("cleanup", "bytes"),
        action("dispose", "dispose"),
        replace(action("inspect", "inspect"), expected_stdout=b"absent\n"),
    )


def plan(run_id: str, calls: tuple[Call, ...] = (), resources: tuple[Resource, ...] = ()) -> Plan:
    return declare(
        Plan(
            run_id,
            calls,
            resources,
            (PEER, Path(__file__), HERE / "pyproject.toml", HERE / "uv.lock"),
            term_grace=0.15,
            kill_wait=0.3,
        )
    )


def run_control(tmp_path: Path, selected: Plan, fault: StorageFault | None, bound: float) -> Run:
    root = tmp_path / "evidence"
    returned: object = None
    raised: OSError | None = None
    start = time.monotonic()
    with watchdog(bound, fault.release if fault is not None else lambda: None):
        try:
            returned = execute(selected, root)
        except OSError as error:
            raised = error
    return Run(returned, raised, time.monotonic() - start, root / selected.run_id)


def owned(tmp_path: Path, resource: Resource) -> bool:
    return (tmp_path / resource.identity).exists()


def assert_killed_publication(run: Run, bound: float) -> StorageOutcome:
    assert run.elapsed <= bound
    outcome = storage_outcome(run)
    assert outcome is not None, "the run reported no terminal storage outcome"
    assert outcome.publication == "killed"
    assert outcome.writer_reaped is True
    return outcome


@removed_by("storage-publication-k168", "a stopped storage path holds the controller")
def test_control_stalled_writer(tmp_path, monkeypatch):
    run_id = uuid.uuid4().hex
    first = vm(tmp_path, run_id)
    closed: list[float] = []
    close = controller._InspectionState.close

    def timed_close(self: controller._InspectionState) -> None:
        close(self)
        closed.append(time.monotonic())

    monkeypatch.setattr(controller._InspectionState, "close", timed_close)
    fault = StorageFault(
        monkeypatch, tmp_path / "stall", at="acquire.json", action="stall", persist=True
    )
    selected = plan(run_id, (peer("workload", "bytes"),), (first,))
    run = run_control(tmp_path, selected, fault, RUN_WATCHDOG)
    assert not owned(tmp_path, first), "disposal did not finish"
    assert closed, "the inspection worker did not close"
    assert_killed_publication(run, RUN_WATCHDOG)


@removed_by("storage-publication-k168", "the first P2 open blocks the controller")
def test_control_blocking_storage_call(tmp_path, monkeypatch):
    run_id = uuid.uuid4().hex
    first = vm(tmp_path, run_id)
    fault = StorageFault(
        monkeypatch,
        tmp_path / "stall",
        at="intent-1.json",
        action="stall",
        persist=False,
        after=False,
    )
    selected = plan(run_id, (peer("workload", "bytes"),), (first,))
    run = run_control(tmp_path, selected, fault, RUN_WATCHDOG)
    assert not owned(tmp_path, first)
    assert_killed_publication(run, RUN_WATCHDOG)


@removed_by("storage-cutover-k166", "a storage error stops owned teardown")
def test_control_storage_error(tmp_path, monkeypatch):
    if os.geteuid() == 0:
        pytest.skip("unavailable: root ignores the removed write permission")
    run_id = uuid.uuid4().hex
    first = vm(tmp_path, run_id)
    second = vm(tmp_path, run_id, "second-")
    directory = tmp_path / "evidence" / run_id

    def deny() -> None:
        directory.chmod(stat.S_IRUSR | stat.S_IXUSR)

    fault = StorageFault(
        monkeypatch,
        tmp_path / "unused",
        at="acquire.json",
        action="observe",
        persist=False,
        on_trigger=deny,
    )
    try:
        run = run_control(tmp_path, plan(run_id, (), (first, second)), fault, RUN_WATCHDOG)
    finally:
        if directory.exists():
            directory.chmod(stat.S_IRWXU)
    assert not owned(tmp_path, first), "teardown changed after the storage error"
    assert not owned(tmp_path, second), "forward progress continued after the storage error"
    assert run.raised is None, f"the controller raised {run.raised!r}"
    outcome = storage_outcome(run)
    assert outcome is not None, "the run reported no terminal storage outcome"
    assert outcome.first_failure == errno.errorcode[errno.EACCES]


@removed_by("storage-cutover-k166", "a dead storage path stops owned teardown")
def test_control_writer_death(tmp_path, monkeypatch):
    run_id = uuid.uuid4().hex
    first = vm(tmp_path, run_id)
    second = vm(tmp_path, run_id, "second-")
    fault = StorageFault(
        monkeypatch, tmp_path / "unused", at="acquire.json", action="break", persist=True
    )
    run = run_control(tmp_path, plan(run_id, (), (first, second)), fault, RUN_WATCHDOG)
    assert not owned(tmp_path, first), "teardown changed after the writer died"
    assert not owned(tmp_path, second), "forward progress continued after the writer died"
    assert run.raised is None, f"the controller raised {run.raised!r}"
    outcome = storage_outcome(run)
    assert outcome is not None, "the run reported no terminal storage outcome"
    assert outcome.first_failure in {errno.errorcode[errno.EPIPE], "writer_exit"}
    assert outcome.writer_reaped is True


@removed_by("storage-publication-k168", "the controller blocks on storage instead of dropping")
def test_control_outbox_overflow(tmp_path, monkeypatch):
    run_id = uuid.uuid4().hex
    marker = tmp_path / "later-ran"
    fault = StorageFault(
        monkeypatch, tmp_path / "stall", at="subjects-before.json", action="stall", persist=True
    )
    calls = (script("flood", FLOOD, FLOOD_BYTES), script("later", TOUCH, marker))
    run = run_control(tmp_path, plan(run_id, calls), fault, RUN_WATCHDOG)
    assert not marker.exists(), "forward progress continued after the outbox drop"
    outcome = assert_killed_publication(run, RUN_WATCHDOG)
    assert outcome.first_failure == "outbox_drop"
    assert outcome.drops.get("flood.stdout", 0) > 0


@removed_by("storage-cutover-k166", "a missing readiness holds the controller")
def test_control_readiness_failure(tmp_path, monkeypatch):
    run_id = uuid.uuid4().hex
    first = vm(tmp_path, run_id)
    started: list[bool] = []
    start = controller.Worker.start

    def recorded_start(self: controller.Worker) -> bool:
        started.append(True)
        return start(self)

    monkeypatch.setattr(controller.Worker, "start", recorded_start)
    fault = StorageFault(
        monkeypatch, tmp_path / "stall", at="plan.json", action="stall", persist=True, after=False
    )
    selected = plan(run_id, (peer("workload", "bytes"),), (first,))
    run = run_control(tmp_path, selected, fault, READINESS_WATCHDOG)
    assert run.elapsed <= READINESS_WATCHDOG
    assert not owned(tmp_path, first), "the controller acquired before readiness"
    assert not started, "an inspection worker started before readiness"
    assert not list(run.directory.glob("intent-*")), "an intent record exists"
    outcome = storage_outcome(run)
    assert outcome is not None, "the run reported no terminal storage outcome"
    assert outcome.readiness == "failed"
    assert outcome.writer_reaped is True


@removed_by("storage-publication-k168", "a stalled intent record holds the controller")
def test_control_intent_gate(tmp_path, monkeypatch):
    run_id = uuid.uuid4().hex
    first = vm(tmp_path, run_id)
    second = vm(tmp_path, run_id, "second-")
    fault = StorageFault(
        monkeypatch,
        tmp_path / "stall",
        at="intent-2.json",
        action="stall",
        persist=True,
        after=False,
    )
    run = run_control(tmp_path, plan(run_id, (), (first, second)), fault, RUN_WATCHDOG)
    assert not owned(tmp_path, second), "the second resource was acquired past the gate"
    assert not owned(tmp_path, first), "the first resource's teardown changed"
    outcome = assert_killed_publication(run, RUN_WATCHDOG)
    assert outcome.first_failure == "intent_gate"


FILESYSTEM_EVENTS = frozenset(
    {
        "open",
        "import",
        "glob.glob",
        "glob.glob/2",
        "os.chdir",
        "os.chflags",
        "os.chmod",
        "os.chown",
        "os.fwalk",
        "os.getxattr",
        "os.link",
        "os.listdir",
        "os.listxattr",
        "os.lockf",
        "os.mkdir",
        "os.mkfifo",
        "os.mknod",
        "os.remove",
        "os.removexattr",
        "os.rename",
        "os.rmdir",
        "os.scandir",
        "os.setxattr",
        "os.symlink",
        "os.truncate",
        "os.utime",
        "os.walk",
        "pathlib.Path.glob",
        "pathlib.Path.rglob",
        "shutil.chown",
        "shutil.copyfile",
        "shutil.copymode",
        "shutil.copystat",
        "shutil.copytree",
        "shutil.move",
        "shutil.rmtree",
        "tempfile.mkdtemp",
        "tempfile.mkstemp",
    }
)
_audit_log: list[tuple[str, str]] | None = None
_audit_installed = False


def _audit(event: str, args: tuple[object, ...]) -> None:
    log = _audit_log
    if log is not None and event in FILESYSTEM_EVENTS:
        log.append((event, repr(args)[:200]))


def record_audit_events() -> list[tuple[str, str]]:
    # An audit hook cannot be removed, so one hook records only while a log
    # is set. https://docs.python.org/3.12/library/sys.html#sys.addaudithook
    global _audit_installed, _audit_log
    if not _audit_installed:
        sys.addaudithook(_audit)
        _audit_installed = True
    _audit_log = []
    return _audit_log


def stop_audit_events() -> None:
    global _audit_log
    _audit_log = None


@removed_by("storage-publication-k168", "the controller thread makes the storage calls")
def test_control_no_controller_storage_calls(tmp_path, monkeypatch):
    run_id = uuid.uuid4().hex
    first = vm(tmp_path, run_id)
    window: list[list[tuple[str, str]]] = []

    def ready() -> None:
        window.append(record_audit_events())

    fault = StorageFault(
        monkeypatch,
        tmp_path / "unused",
        at="subjects-before.json",
        action="observe",
        persist=False,
        on_trigger=ready,
    )
    selected = plan(run_id, (peer("workload", "bytes"),), (first,))
    try:
        run = run_control(tmp_path, selected, fault, RUN_WATCHDOG)
    finally:
        stop_audit_events()
    assert window, "the run never reached readiness"
    events = window[0]
    assert events == [], f"controller storage events after readiness: {events[:5]}"
    assert run.raised is None
    assert not owned(tmp_path, first)


@removed_by("pipe-capture-k167", "calls write their streams to evidence files")
def test_control_inherited_state(tmp_path):
    run_id = uuid.uuid4().hex
    run = run_control(
        tmp_path, plan(run_id, (script("descriptors", DESCRIPTORS),)), None, RUN_WATCHDOG
    )
    report = json.loads((run.directory / "descriptors.stdout").read_bytes())
    assert report == {"0": "null", "1": "fifo", "2": "fifo"}
    writer = writer_descriptors(run)
    assert writer is not None, "the writer reported no descriptors"
    assert set(writer) >= {0, 1, 2}
    assert sorted(writer.values()) == ["directory", "fifo", "fifo", "null"]
