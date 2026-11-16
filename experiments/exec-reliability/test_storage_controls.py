"""Required storage controls from docs/specs/diagnostic-evidence-storage.md.

Each control asserts the final behaviour of the spec. On the build before the
evidence writer, every control fails, and a strict expected-failure marker
names the slice that removes it. The marker accepts only an assertion failure
or a watchdog expiry. Any other exception fails the test.

A fault fixture runs the writer under writer_faults.py, which stops the
writer, ends it or fails one of its file calls at a named record. The
assertions are the ones that the controls had before the writer. An autouse
fixture records every storage client and fails a test that leaves a writer
unreaped. A reaped writer cannot remain stopped. The adapters storage_outcome
and writer_descriptors are fixtures in the same sense.
"""

from __future__ import annotations

import errno
import json
import math
import os
import shutil
import signal
import sys
import time
import uuid
from collections.abc import Callable, Iterator, Mapping
from contextlib import contextmanager
from dataclasses import dataclass, fields, replace
from pathlib import Path
from typing import cast

import pytest

import controller
from controller import Call, Plan, Resource, execute
from retention import completion
from storage_client import StorageClient
from writer_client import WRITER, healthy_run

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

# Scheduling tolerance for phase assertions. The bounds are conditional
# scheduling guarantees, so each check allows this much lateness. A stalled
# storage path holds a phase for the whole watchdog bound, far beyond it.
TOLERANCE = 0.5
# A teardown step can follow a missed intent gate, so its slack includes it.
STEP_SLACK = INTENT_GATE + TOLERANCE

# Watchdog bounds. The final build finishes each control within its bound:
# the declared phases above, the writer reap grace and about four seconds of
# fixture teardown. On this build a stalled storage call holds the controller
# until the watchdog expires, and that expiry is the recorded failure.
READINESS_WATCHDOG = READINESS_BOUND + REAP_GRACE + 2.0
RUN_WATCHDOG = 12.0

FLOOD_BYTES = 1024 * 1024
FLOOD = "import sys; sys.stdout.buffer.write(b'x' * int(sys.argv[1]))"
TOUCH = "import sys; open(sys.argv[1], 'x').close()"
# A resource or workload command that appends one history entry after its
# action succeeds. A skipped or failed action leaves no entry, and disposal
# cannot erase an acquisition.
RECORDED = """
import json, os, sys, time
action, identity, root, history = sys.argv[1:]
marker = os.path.join(root, identity)
if action == "acquire":
    open(marker, "x").close()
elif action == "dispose":
    os.unlink(marker)
elif action == "inspect":
    if os.path.exists(marker):
        sys.exit(1)
    print("absent")
line = {"action": action, "identity": identity, "monotonic": time.monotonic()}
fd = os.open(history, os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o644)
try:
    os.write(fd, json.dumps(line).encode() + b"\\n")
finally:
    os.close(fd)
"""
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


@dataclass
class ClientHooks:
    """Per-test callbacks on the storage client that the controller creates."""

    on_ready: Callable[[StorageClient], None] | None = None
    on_failure: Callable[[StorageClient, str], None] | None = None


HOOKS = ClientHooks()
CLIENTS: list[StorageClient] = []


class RecordedClient(StorageClient):
    """A storage client that the controls record and observe."""

    def __init__(self, command: tuple[str, ...] = WRITER, **bounds: float | int) -> None:
        super().__init__(command, **cast(dict[str, float], bounds))
        CLIENTS.append(self)

    def start(self, root: str, run: str, plan: bytes, subjects: list[str]) -> bool:
        ready = super().start(root, run, plan, subjects)
        if ready and HOOKS.on_ready is not None:
            HOOKS.on_ready(self)
        return ready

    def fail(self, cause: str) -> None:
        super().fail(cause)
        if HOOKS.on_failure is not None:
            HOOKS.on_failure(self, cause)


@pytest.fixture(autouse=True)
def recorded_clients(monkeypatch: pytest.MonkeyPatch) -> Iterator[list[StorageClient]]:
    """Record each storage client, and fail a test that leaves a writer unreaped."""
    CLIENTS.clear()
    monkeypatch.setattr(HOOKS, "on_ready", None)
    monkeypatch.setattr(HOOKS, "on_failure", None)
    monkeypatch.setattr(controller, "StorageClient", RecordedClient)
    yield CLIENTS
    live = [client.pid for client in CLIENTS if client.pid is not None and client.reaped_at is None]
    for pid in live:
        # Clean up before the failure is reported, so no writer outlives the test.
        os.kill(pid, signal.SIGKILL)
        os.waitpid(pid, 0)
    if live:
        # Not an AssertionError: the expected-failure markers accept that type
        # in every phase, so it would hide a live writer behind a marker.
        pytest.fail(f"the run left writers unreaped: {live}")


class WriterFault:
    """Run the writer under writer_faults.py with the given faults.

    Each fault is `CALL:NAME`. `stall` stops the writer at the open of NAME,
    `exit` ends it there, and `deny` fails that open with EACCES. The writer
    stays stopped until it receives KILL or, with `resume`, CONT.
    """

    def __init__(
        self,
        monkeypatch: pytest.MonkeyPatch,
        *faults: str,
        on_ready: Callable[[StorageClient], None] | None = None,
        resume_on: str | None = None,
    ) -> None:
        command = (sys.executable, "-I", str(HERE / "writer_faults.py"), *faults)
        monkeypatch.setattr(controller, "WRITER", command)
        if on_ready is not None:
            monkeypatch.setattr(HOOKS, "on_ready", on_ready)
        if resume_on is not None:

            def resume(client: StorageClient, cause: str) -> None:
                if cause == resume_on and client.pid is not None and client.reaped_at is None:
                    os.kill(client.pid, signal.SIGCONT)

            monkeypatch.setattr(HOOKS, "on_failure", resume)

    def release(self) -> None:
        """Send KILL to every unreaped writer. The controller then reaps it."""
        for client in CLIENTS:
            if client.pid is not None and client.reaped_at is None and not client.residual:
                os.kill(client.pid, signal.SIGKILL)


@dataclass(frozen=True)
class Run:
    returned: object
    raised: OSError | None
    elapsed: float
    directory: Path


@dataclass(frozen=True)
class Phase:
    """One declared phase receipt: absolute start, deadline, finish and outcome."""

    started: float
    deadline: float
    finished: float
    outcome: str


@dataclass(frozen=True)
class StorageOutcome:
    publication: str
    writer_reaped: bool
    readiness: str
    first_failure: str | None
    drops: Mapping[str, int]
    # Keyed "readiness", "intent-<n>" and "publication".
    phases: Mapping[str, Phase]
    killed_at: float | None
    reaped_at: float | None


def storage_outcome(run: Run) -> StorageOutcome | None:
    """Return the terminal storage outcome that the controller reports.

    execute returns the run directory with the controller's storage outcome.
    A phase that has not finished reports an infinite finish.
    """
    storage = getattr(run.returned, "storage", None)
    if not isinstance(storage, controller.StorageOutcome):
        return None
    return StorageOutcome(
        publication=storage.publication,
        writer_reaped=storage.writer_reaped,
        readiness=storage.readiness,
        first_failure=storage.first_failure,
        drops=storage.drops,
        phases={
            name: Phase(
                phase.started,
                phase.deadline,
                math.inf if phase.finished is None else phase.finished,
                phase.outcome,
            )
            for name, phase in storage.phases.items()
        },
        killed_at=storage.killed_at,
        reaped_at=storage.reaped_at,
    )


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


def script(name: str, source: str, *args: object, budget: float = 3) -> Call:
    return Call(name, (sys.executable, "-I", "-c", source, *map(str, args)), budget)


def history_path(tmp_path: Path) -> Path:
    return tmp_path / "history.jsonl"


def recorded(tmp_path: Path, name: str, action: str, identity: str, budget: float) -> Call:
    return script(name, RECORDED, action, identity, tmp_path, history_path(tmp_path), budget=budget)


def workload(tmp_path: Path) -> Call:
    return recorded(tmp_path, "workload", "workload", "workload", 3)


def vm(tmp_path: Path, run_id: str, prefix: str = "") -> Resource:
    identity = f"diag-{run_id}-{prefix or 'first'}"

    def action(name: str, budget: float = 0.2) -> Call:
        return recorded(tmp_path, f"{prefix}{name}", name, identity, budget)

    return Resource(
        "vm",
        identity,
        action("acquire", 3),
        action("cleanup"),
        action("dispose"),
        replace(action("inspect"), expected_stdout=b"absent\n"),
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


def run_control(tmp_path: Path, selected: Plan, fault: WriterFault | None, bound: float) -> Run:
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


@dataclass(frozen=True)
class Entry:
    action: str
    identity: str
    monotonic: float


def history(tmp_path: Path) -> list[Entry]:
    path = history_path(tmp_path)
    if not path.exists():
        return []
    return [Entry(**json.loads(line)) for line in path.read_text().splitlines()]


def entries(tmp_path: Path, identity: str) -> list[Entry]:
    return [entry for entry in history(tmp_path) if entry.identity == identity]


def acquired(tmp_path: Path, resource: Resource) -> Entry | None:
    found = [e for e in entries(tmp_path, resource.identity) if e.action == "acquire"]
    assert len(found) <= 1, f"{resource.identity} was acquired more than once"
    return found[0] if found else None


def workload_finished(tmp_path: Path) -> Entry:
    found = entries(tmp_path, "workload")
    assert len(found) == 1, "the workload did not finish exactly once"
    return found[0]


def capture_workers(monkeypatch: pytest.MonkeyPatch) -> list[controller.Worker]:
    workers: list[controller.Worker] = []
    start = controller.Worker.start

    def recorded_start(self: controller.Worker) -> bool:
        workers.append(self)
        return start(self)

    monkeypatch.setattr(controller.Worker, "start", recorded_start)
    return workers


def assert_teardown_unchanged(
    tmp_path: Path, resource: Resource, anchor: float, workers: list[controller.Worker]
) -> float:
    """Assert that cleanup, disposal, inspection and worker close each ran.

    Each step must finish within its own budget and the step slack after the
    previous step. The anchor is the last forward-progress event before the
    teardown. The return value is the worker's shutdown finish.
    """
    steps = (resource.cleanup, resource.dispose, resource.inspect)
    actions = [e.action for e in entries(tmp_path, resource.identity) if e.action != "acquire"]
    assert actions == ["cleanup", "dispose", "inspect"], f"teardown actions were {actions}"
    done = [e for e in entries(tmp_path, resource.identity) if e.action != "acquire"]
    previous = anchor
    for step, entry in zip(steps, done, strict=True):
        assert entry.monotonic >= previous, f"{step.name} ran before the previous step"
        late = entry.monotonic - previous - step.budget - STEP_SLACK
        assert late <= 0, f"{step.name} finished {late:.3f} s past its ceiling"
        previous = entry.monotonic
    assert len(workers) == 1, "the inspection worker did not start exactly once"
    shutdown = workers[0].shutdown
    assert shutdown is not None, "the inspection worker did not close"
    assert shutdown["outcome"] == "reaped", f"the worker close ended {shutdown['outcome']}"
    assert shutdown["overrun"] is False, "the worker close overran its bounds"
    started = cast(float, shutdown["started_monotonic"])
    finished = cast(float, shutdown["finished_monotonic"])
    assert started >= previous, "the worker close started before the last inspection"
    assert started - previous <= TOLERANCE, "the worker close started late"
    return finished


def assert_phase(outcome: StorageOutcome, name: str, bound: float) -> Phase:
    phase = outcome.phases.get(name)
    assert phase is not None, f"the run recorded no {name} phase"
    assert phase.deadline - phase.started <= bound + 1e-6, f"the {name} phase exceeds {bound} s"
    assert phase.finished <= phase.deadline + TOLERANCE, f"the {name} phase finished late"
    return phase


def assert_killed_publication(run: Run, bound: float, worker_closed: float) -> StorageOutcome:
    assert run.elapsed <= bound
    outcome = storage_outcome(run)
    assert outcome is not None, "the run reported no terminal storage outcome"
    assert outcome.publication == "killed"
    assert outcome.writer_reaped is True
    publication = outcome.phases.get("publication")
    assert publication is not None, "the run recorded no publication phase"
    assert publication.started >= worker_closed, "publication started before worker close"
    window = publication.deadline - publication.started
    assert abs(window - PUBLICATION_WINDOW) <= 1e-6, f"publication window was {window} s"
    assert outcome.killed_at is not None, "the writer received no KILL"
    assert outcome.killed_at >= publication.deadline, "the writer received KILL before P"
    assert outcome.killed_at - publication.deadline <= TOLERANCE, "KILL came late after P"
    assert outcome.reaped_at is not None, "the writer reap was not recorded"
    assert outcome.reaped_at <= publication.deadline + REAP_GRACE, "reap came after P + 2 s"
    return outcome


@removed_by("storage-publication-k168", "a stopped storage path holds the controller")
def test_control_stalled_writer(tmp_path, monkeypatch):
    run_id = uuid.uuid4().hex
    first = vm(tmp_path, run_id)
    workers = capture_workers(monkeypatch)
    fault = WriterFault(monkeypatch, "stall:acquire.json")
    selected = plan(run_id, (workload(tmp_path),), (first,))
    run = run_control(tmp_path, selected, fault, RUN_WATCHDOG)
    assert acquired(tmp_path, first) is not None, "the first resource was never acquired"
    anchor = workload_finished(tmp_path).monotonic
    closed = assert_teardown_unchanged(tmp_path, first, anchor, workers)
    assert_killed_publication(run, RUN_WATCHDOG, closed)


@removed_by("storage-publication-k168", "the first P2 open blocks the controller")
def test_control_blocking_storage_call(tmp_path, monkeypatch):
    run_id = uuid.uuid4().hex
    first = vm(tmp_path, run_id)
    fault = WriterFault(monkeypatch, "stall:intent-1.json")
    workers = capture_workers(monkeypatch)
    selected = plan(run_id, (workload(tmp_path),), (first,))
    run = run_control(tmp_path, selected, fault, RUN_WATCHDOG)
    # The blocked intent-1 record misses its gate, so nothing is acquired and
    # no workload runs. The rest matches the stalled-writer control.
    assert acquired(tmp_path, first) is None, "the first resource was acquired past the gate"
    assert not entries(tmp_path, "workload"), "the workload ran after the forward stop"
    assert len(workers) == 1, "the inspection worker did not start exactly once"
    shutdown = workers[0].shutdown
    assert shutdown is not None, "the inspection worker did not close"
    assert shutdown["outcome"] == "reaped", f"the worker close ended {shutdown['outcome']}"
    assert shutdown["overrun"] is False, "the worker close overran its bounds"
    closed = cast(float, shutdown["finished_monotonic"])
    outcome = assert_killed_publication(run, RUN_WATCHDOG, closed)
    assert_phase(outcome, "intent-1", INTENT_GATE)


def test_control_storage_error(tmp_path, monkeypatch):
    run_id = uuid.uuid4().hex
    first = vm(tmp_path, run_id)
    second = vm(tmp_path, run_id, "second-")
    # The writer's create of the next record after the first acquisition fails.
    fault = WriterFault(monkeypatch, "deny:intent-2.json")
    workers = capture_workers(monkeypatch)
    run = run_control(tmp_path, plan(run_id, (), (first, second)), fault, RUN_WATCHDOG)
    start = acquired(tmp_path, first)
    assert start is not None, "the first resource was never acquired"
    assert acquired(tmp_path, second) is None, "forward progress continued after the error"
    try:
        assert_teardown_unchanged(tmp_path, first, start.monotonic, workers)
    except AssertionError as error:
        raise AssertionError(f"teardown changed after the storage error: {error}") from None
    assert run.raised is None, f"the controller raised {run.raised!r}"
    outcome = storage_outcome(run)
    assert outcome is not None, "the run reported no terminal storage outcome"
    assert outcome.first_failure == errno.errorcode[errno.EACCES]


def test_control_writer_death(tmp_path, monkeypatch):
    run_id = uuid.uuid4().hex
    first = vm(tmp_path, run_id)
    second = vm(tmp_path, run_id, "second-")
    # The writer ends when the first acquisition's receipt arrives.
    fault = WriterFault(monkeypatch, "exit:acquire.json")
    workers = capture_workers(monkeypatch)
    run = run_control(tmp_path, plan(run_id, (), (first, second)), fault, RUN_WATCHDOG)
    start = acquired(tmp_path, first)
    assert start is not None, "the first resource was never acquired"
    assert acquired(tmp_path, second) is None, "forward progress continued after the death"
    try:
        assert_teardown_unchanged(tmp_path, first, start.monotonic, workers)
    except AssertionError as error:
        raise AssertionError(f"teardown changed after the writer died: {error}") from None
    assert run.raised is None, f"the controller raised {run.raised!r}"
    outcome = storage_outcome(run)
    assert outcome is not None, "the run reported no terminal storage outcome"
    assert outcome.first_failure in {errno.errorcode[errno.EPIPE], "writer_exit"}
    assert outcome.writer_reaped is True


@removed_by("storage-publication-k168", "the controller blocks on storage instead of dropping")
def test_control_outbox_overflow(tmp_path, monkeypatch):
    run_id = uuid.uuid4().hex
    marker = tmp_path / "later-ran"
    # Until pipe-capture-k167 the flood goes to a file, so the writer stops at
    # the first record after it.
    fault = WriterFault(monkeypatch, "stall:flood.json")
    calls = (script("flood", FLOOD, FLOOD_BYTES), script("later", TOUCH, marker))
    run = run_control(tmp_path, plan(run_id, calls), fault, RUN_WATCHDOG)
    assert not marker.exists(), "forward progress continued after the outbox drop"
    outcome = assert_killed_publication(run, RUN_WATCHDOG)
    assert outcome.first_failure == "outbox_drop"
    assert outcome.drops.get("flood.stdout", 0) > 0


def test_control_readiness_failure(tmp_path, monkeypatch):
    run_id = uuid.uuid4().hex
    first = vm(tmp_path, run_id)
    workers = capture_workers(monkeypatch)
    fault = WriterFault(monkeypatch, "stall:plan.json")
    selected = plan(run_id, (workload(tmp_path),), (first,))
    run = run_control(tmp_path, selected, fault, READINESS_WATCHDOG)
    assert run.elapsed <= READINESS_WATCHDOG
    assert acquired(tmp_path, first) is None, "the controller acquired before readiness"
    assert not entries(tmp_path, "workload"), "a workload ran before readiness"
    assert not workers, "an inspection worker started before readiness"
    assert not list(run.directory.glob("intent-*")), "an intent record exists"
    outcome = storage_outcome(run)
    assert outcome is not None, "the run reported no terminal storage outcome"
    assert outcome.readiness == "failed"
    assert outcome.writer_reaped is True
    readiness = assert_phase(outcome, "readiness", READINESS_BOUND)
    assert outcome.reaped_at is not None, "the writer reap was not recorded"
    assert outcome.reaped_at <= readiness.deadline + REAP_GRACE, "reap came after deadline + 2 s"


@removed_by("storage-publication-k168", "a stalled intent record holds the controller")
def test_control_intent_gate(tmp_path, monkeypatch):
    run_id = uuid.uuid4().hex
    first = vm(tmp_path, run_id)
    second = vm(tmp_path, run_id, "second-")
    fault = WriterFault(monkeypatch, "stall:intent-2.json")
    workers = capture_workers(monkeypatch)
    run = run_control(tmp_path, plan(run_id, (), (first, second)), fault, RUN_WATCHDOG)
    assert acquired(tmp_path, second) is None, "the second resource was acquired past the gate"
    start = acquired(tmp_path, first)
    assert start is not None, "the first resource was never acquired"
    try:
        closed = assert_teardown_unchanged(tmp_path, first, start.monotonic, workers)
    except AssertionError as error:
        raise AssertionError(f"the first resource's teardown changed: {error}") from None
    outcome = assert_killed_publication(run, RUN_WATCHDOG, closed)
    assert outcome.first_failure == "intent_gate"
    gate = assert_phase(outcome, "intent-2", INTENT_GATE)
    assert gate.outcome == "missed", f"the intent-2 gate ended {gate.outcome}"


def test_intent_gate_with_resumed_writer(tmp_path, monkeypatch):
    """The intent-2 gate misses while the writer is stopped, and the writer resumes.

    The writer stops at the open of intent-2 and receives CONT only after the
    gate deadline fails storage. The resumed writer then stores intent-2,
    reads the end of its frame pipe and exits without a close. So the close
    wait ends, which the persistent stall in test_control_intent_gate does
    not establish.
    """
    run_id = uuid.uuid4().hex
    first = vm(tmp_path, run_id)
    second = vm(tmp_path, run_id, "second-")
    fault = WriterFault(monkeypatch, "stall:intent-2.json", resume_on="intent_gate")
    workers = capture_workers(monkeypatch)
    selected = plan(run_id, (workload(tmp_path),), (first, second))
    run = run_control(tmp_path, selected, fault, RUN_WATCHDOG)
    assert run.raised is None, f"the controller raised {run.raised!r}"
    assert isinstance(run.returned, Path) and run.returned == run.directory
    assert acquired(tmp_path, second) is None, "the second resource was acquired past the gate"
    assert not entries(tmp_path, "workload"), "a workload ran after the forward stop"
    start = acquired(tmp_path, first)
    assert start is not None, "the first resource was never acquired"
    assert_teardown_unchanged(tmp_path, first, start.monotonic, workers)
    outcome = storage_outcome(run)
    assert outcome is not None, "the run reported no storage outcome"
    assert outcome.first_failure == "intent_gate"
    gate = assert_phase(outcome, "intent-2", INTENT_GATE)
    assert gate.outcome == "missed", f"the intent-2 gate ended {gate.outcome}"
    assert outcome.writer_reaped is True
    assert outcome.publication == "incomplete"
    assert (run.directory / "intent-2.json").exists(), "the resumed writer did not store intent-2"
    verdict = completion(run.directory)
    assert verdict["complete"] is False


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

    def ready(_client: StorageClient) -> None:
        window.append(record_audit_events())

    fault = WriterFault(monkeypatch, on_ready=ready)
    selected = plan(run_id, (workload(tmp_path),), (first,))
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


# Review control: close record ----------------------------------------------
#
# The spec's review controls must each fail on a writer build that lacks the
# rule that they name. This control drives the writer through the test client
# in writer_client.py, so it needs no controller and passes from k164 on.

IDENTITY_FIRST = """\
import sys
sys.path.insert(0, sys.argv[1])
import writer


class IdentityFirst(writer.Writer):
    def close_transaction(self):
        identity = self.write_identity()
        storage_close = self.write_storage_close("orderly")
        return {"storage_close": storage_close, "identity": identity}


raise SystemExit(writer.run(IdentityFirst()))
"""


def close_record_control(tmp_path: Path, command: tuple[str, ...]) -> None:
    run = healthy_run(tmp_path / "evidence", command=command)
    assert run.status == 0
    healthy = completion(run.run_dir)
    assert healthy["complete"] is True, healthy["reasons"]

    missing = tmp_path / "missing-close"
    shutil.copytree(run.run_dir, missing)
    (missing / "storage-close.json").unlink()
    assert completion(missing)["complete"] is False

    changed = tmp_path / "changed-close"
    shutil.copytree(run.run_dir, changed)
    record = json.loads((changed / "storage-close.json").read_bytes())
    record["refused"] = 0
    record["last_applied"] = record["last_applied"] + 1
    (changed / "storage-close.json").write_text(json.dumps(record, indent=2) + "\n")
    assert completion(changed)["complete"] is False


def test_control_close_record(tmp_path):
    close_record_control(tmp_path, WRITER)


def test_control_close_record_fails_identity_before_storage_close(tmp_path):
    build = (sys.executable, "-I", "-c", IDENTITY_FIRST, str(HERE))
    with pytest.raises(AssertionError) as failure:
        close_record_control(tmp_path, build)
    assert "the identity listing lacks the storage-close record" in str(failure.value)
