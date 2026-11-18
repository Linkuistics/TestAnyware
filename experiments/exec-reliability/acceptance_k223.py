"""Frozen trial plan for the guest command lifecycle acceptance (leaf acceptance-plan-k223).

    acceptance_k223.py OUT MANIFEST

The trial runs the root acceptance inventory on five fresh macOS clones, then on
five fresh Linux clones, one clone at a time. One clone is one controller plan
with one owned VM. The trial stops after the first clone that fails its run or
its check. The later clones stay unreached, and the trial fails.

MANIFEST is the freeze manifest of leaf acceptance-preparation-k224. It is one
JSON object with these fields:

- `cli` and `tart`: absolute paths of the private Host CLI and of Tart.
- `goldens`: the new golden name for `macos` and for `linux`.
- `bounds`: `boot` and `slot` in seconds, each positive and finite.
- `sha256`: an object that maps absolute paths to their frozen digests. The
  plan refuses to start when one digest differs. Each path is also a subject.

Each clone runs these cases in the root order. Each `file exec` call names its
own exec id, and the checker compares the record id with it.

1. Preparation: `health-before`, `identity`, `bootstrap-dir` and the uploads of
   the retained fixture's `guest.sh` and `lesson`.
2. `baseline`: the fixture `inventory` action. It checks that the installation
   and the marker are absent.
3. `install`: the offline installation of the lesson.
4. `intact`: the lesson itself, so the record evidence holds the stdout and
   stderr bytes of the byte oracle.
5. `ordinary-01` to `ordinary-10`: health, `printf ordinary-ok` and a process
   table, in turn.
6. `exit-23`: the issue payload `exit 23`.
7. `timeout`: the child payload with guest timeout 2 s.
8. `after-timeout-01` to `after-timeout-10`: a child check, health and
   `printf ordinary-ok`, in turn. The first and the last are child checks.
9. `cancel`: the explicit cancellation helper. It starts the child payload
   with guest timeout 20 s, calls health after 1 s, and sends
   `file cancel --wait 5` after 2 s.
10. `after-cancel-01` to `after-cancel-10`: as step 8, for the cancelled child.
11. `interrupt`: the child payload with guest timeout 20 s. The controller sends
    SIGTERM to the Host CLI after 2 s, then SIGKILL after 2 s more if needed.
    `interrupt-child` and `interrupt-health` follow. `interrupt-fallback` is
    `file cancel --wait 5` for the same exec id.
12. Transfer: `capture` runs the fixture `intact` action. Downloads of
    `stdout.bin`, `stderr.bin`, `artifact.bin`, `status.txt` and the three
    partial files follow. `download-missing` must fail with a typed error.
    The checker runs the four corruption controls on this evidence.
13. The `finally` path of the controller: the guest cleanup of the three known
    children, the disposal of the VM and the absence inspection.

The next fresh clone demonstrates the reset of the guest state, because its
`baseline` finds no installation and no marker. No later clone follows the
fifth clone of an OS, so its reset stays undemonstrated, and the trial record
says so.

The plan writes OUT/<os>-<n>/inventory.json before the first live call of each
clone. acceptance_k223_check.py judges each clone, and the trial writes
OUT/trial.json once at the end.
"""

from __future__ import annotations

import hashlib
import json
import math
import sys
import uuid
from dataclasses import asdict, dataclass, replace
from pathlib import Path
from typing import Any, cast

from controller import Call, Plan, Resource, execute_outcome, write_json
from linux_k204 import FIXTURE as R5_FIXTURE
from linux_k204 import FIXTURE_SHA256 as R5_SHA256
from linux_k204 import FROZEN_SHA256 as R5_FROZEN
from linux_k204 import owned_cleanup
from routes_k19 import (
    CHECKER,
    DISPOSE,
    HERE,
    REPO,
    _owned,  # pyright: ignore[reportPrivateUsage]
)
from timeout_k4 import FIXTURE as R3_FIXTURE
from timeout_k4 import FIXTURE_SHA256 as R3_SHA256

LEAF = "acceptance-plan-k223"
OSES = ("macos", "linux")
CLONES = 5

# Root brief, "Acceptance inventory and bounds": the comparison ceilings.
OBSERVATION = 30.0
INVENTORY = 60.0
GUEST_TIMEOUT = 2
HOST_MARGIN = 20.0  # raw host exec: guest budget plus 20 s
TRANSFER = 150.0
DISPOSAL = 45.0
GUEST_CLEANUP = 30.0
TERM_GRACE = 2.0
KILL_WAIT = 2.0

# docs/specs/guest-command-lifecycle.md, "Bounds": the second group.
SPAWN_MS = 1000
LATENESS_MS = 500
FINALISE_MS = 2000
FINALISE_TERMINATED_MS = 4000
CANCEL_REPLY = 5.0  # cancellation reply with waitMs 5000
SIGTERM_DEATH = 2.0  # CLI SIGTERM to signal death
HEALTH_DURING = 1.0  # health during a running command
ROOT_BOUND_MS = 2000  # termination grace, and SIGKILL to drain and reap

OBSERVE_GUEST = 10  # 10 s guest budget plus 20 s is the 30 s observation ceiling
INVENTORY_GUEST = 40  # 40 s guest budget plus 20 s is the 60 s inventory ceiling
CHILD_GUEST = 20  # the issue's guest deadline for the cancellation and interruption
CANCEL_AFTER = 2.0
HEALTH_AFTER = 1.0
CANCEL_WAIT = 5
INTERRUPT_EXIT = -15  # the Host CLI raises SIGTERM again
CLEANUP_GUEST = 10
# The controller reserves TERM grace and KILL wait inside the 30 s cleanup ceiling.
CLEANUP_BOUND = GUEST_CLEANUP - TERM_GRACE - KILL_WAIT
# The helper bounds its `file exec` at guest budget plus 20 s, then needs 2 s TERM
# and 2 s KILL for that child. One more second covers its own start and exit.
HELPER_BOUND = CHILD_GUEST + HOST_MARGIN + TERM_GRACE + KILL_WAIT + 1.0

FIXTURES = {"macos": R3_FIXTURE, "linux": R5_FIXTURE}
FIXTURE_SHA256 = {"macos": dict(R3_SHA256), "linux": {**R5_SHA256, "frozen.json": R5_FROZEN}}
FIXTURE_ROOT = "/tmp/vmfit-k15"  # fixed inside the retained guest.sh
LESSON = "/usr/local/bin/vmfit-lesson"
CHILD_ROOT = "/tmp/taw-k223"
CHILD_CASES = ("timeout", "cancel", "interrupt")
ORDINARY = "printf ordinary-ok"
EXIT_23 = "exit 23"
TABLES = {
    "macos": "ps -axo pid,ppid,pgid,stat,lstart,etime,args",
    "linux": "ps -eo pid,ppid,pgid,sid,stat,lstart,etime,args",
}
IDENTITY = {
    "macos": "shasum -a 256 /usr/local/bin/testanyware-agent; sw_vers; uname -a",
    "linux": (
        "cd /opt/testanyware && find . -type f -name '*.py' | LC_ALL=C sort | xargs sha256sum; "
        "cat /etc/os-release; uname -a"
    ),
}
# The fixture files that the transfer case downloads, with the byte oracle.
ORACLE = {
    "stdout.bin": bytes.fromhex("4578706c61696e2074686973206368616e67652e"),
    "stderr.bin": bytes.fromhex("7365706172617465207374646572720a"),
    "artifact.bin": bytes.fromhex("00ff80410a"),
    "status.txt": b"0\n",
}
PARTIAL = b"partial\n"
PARTIAL_STDOUT = b"partial-out\n"
PARTIAL_STDERR = b"partial-err\n"
MISSING = f"{FIXTURE_ROOT}/does-not-exist"


def child_payload(case: str) -> str:
    """The issue's child payload, with one directory per case and partial output.

    The two `printf` lines before `wait` give the record partial stdout and stderr.
    """
    return "\n".join(
        (
            f"d={CHILD_ROOT}/{case}",
            'mkdir -p "$d"',
            "sleep 120 & child=$!",
            'printf \'%s\\n\' "$child" > "$d/child.pid"',
            "printf 'partial\\n' > \"$d/partial\"",
            "printf 'partial-out\\n'",
            "printf 'partial-err\\n' >&2",
            "wait",
        )
    )


def child_check(case: str) -> str:
    """Observe the known child of one case by its PID file.

    The output is `child pid=N`, the `ps` row when `ps` lists the PID, and
    `child ps-exit=E`. An unreadable PID file gives `child pid-file=unreadable`.
    The command exits 0 in each case, so the checker judges the lines.
    """
    return "\n".join(
        (
            f"f={CHILD_ROOT}/{case}/child.pid",
            "if ! pid=$(cat \"$f\"); then printf 'child pid-file=unreadable\\n'; exit 0; fi",
            "printf 'child pid=%s\\n' \"$pid\"",
            'ps -p "$pid" -o pid=,ppid=,pgid=,stat=,lstart=,args=',
            "printf 'child ps-exit=%s\\n' \"$?\"",
        )
    )


CHILD_FILES = tuple(f"{CHILD_ROOT}/{case}/child.pid" for case in CHILD_CASES)
CLEANUP = owned_cleanup(CHILD_FILES)


def exec_id(run_id: str, name: str) -> str:
    """The exec id of one call: 1 to 64 characters from the spec's alphabet."""
    return f"k223-{name}-{run_id}"


@dataclass(frozen=True)
class Clone:
    """The live identities of one clone plan."""

    os_name: str
    index: int
    run_id: str
    vm: str
    cli: str
    python: str
    downloads: Path


def _exec(clone: Clone, name: str, guest_timeout: int, command: str) -> tuple[str, ...]:
    return (
        clone.cli,
        "file",
        "exec",
        "--vm",
        clone.vm,
        "--timeout",
        str(guest_timeout),
        "--exec-id",
        exec_id(clone.run_id, name),
        "--json",
        command,
    )


def _guest(clone: Clone, name: str, guest_timeout: int, command: str, bound: float) -> Call:
    return Call(name, _exec(clone, name, guest_timeout, command), bound)


def _observe(clone: Clone, name: str, command: str) -> Call:
    return _guest(clone, name, OBSERVE_GUEST, command, OBSERVATION)


def _health(clone: Clone, name: str) -> Call:
    return Call(name, (clone.cli, "agent", "health", "--vm", clone.vm, "--json"), OBSERVATION)


def _cancel(clone: Clone, name: str, target: str) -> Call:
    argv = (
        clone.cli,
        "file",
        "cancel",
        "--vm",
        clone.vm,
        "--exec-id",
        exec_id(clone.run_id, target),
        "--wait",
        str(CANCEL_WAIT),
        "--json",
    )
    return Call(name, argv, OBSERVATION)


def _upload(clone: Clone, fixture: Path, name: str) -> Call:
    argv = (
        clone.cli,
        "file",
        "upload",
        "--vm",
        clone.vm,
        str(fixture / name),
        f"{FIXTURE_ROOT}/{name}",
        "--json",
    )
    return Call(f"upload-{name.replace('.', '-')}", argv, TRANSFER)


def _download(clone: Clone, name: str, remote: str, local: str, exit_: int = 0) -> Call:
    argv = (
        clone.cli,
        "file",
        "download",
        "--vm",
        clone.vm,
        remote,
        str(clone.downloads / local),
        "--json",
    )
    return Call(name, argv, TRANSFER, expected_exit=exit_)


def ordinary_calls(clone: Clone) -> list[Call]:
    """Ten ordinary operations: health, `printf ordinary-ok` and a table, in turn."""
    calls: list[Call] = []
    for number in range(1, 11):
        name = f"ordinary-{number:02d}"
        kind = (number - 1) % 3
        if kind == 0:
            calls.append(_health(clone, name))
        elif kind == 1:
            calls.append(_observe(clone, name, ORDINARY))
        else:
            calls.append(_observe(clone, name, TABLES[clone.os_name]))
    return calls


def after_calls(clone: Clone, case: str) -> list[Call]:
    """Ten operations after a case: a child check, health and `printf ordinary-ok`."""
    calls: list[Call] = []
    for number in range(1, 11):
        name = f"after-{case}-{number:02d}"
        kind = (number - 1) % 3
        if kind == 0:
            calls.append(_observe(clone, name, child_check(case)))
        elif kind == 1:
            calls.append(_health(clone, name))
        else:
            calls.append(_observe(clone, name, ORDINARY))
    return calls


def cancel_call(clone: Clone) -> Call:
    """The explicit cancellation helper, bounded as one controller call."""
    argv = (
        clone.python,
        "-I",
        str(HERE / "acceptance_k223_cancel.py"),
        "--cli",
        clone.cli,
        "--vm",
        clone.vm,
        "--exec-id",
        exec_id(clone.run_id, "cancel"),
        "--guest-timeout",
        str(CHILD_GUEST),
        "--host-margin",
        f"{HOST_MARGIN:g}",
        "--health-after",
        f"{HEALTH_AFTER:g}",
        "--cancel-after",
        f"{CANCEL_AFTER:g}",
        "--cancel-wait",
        str(CANCEL_WAIT),
        "--observation",
        f"{OBSERVATION:g}",
        "--command",
        child_payload("cancel"),
    )
    return Call("cancel", argv, HELPER_BOUND)


def clone_calls(clone: Clone) -> list[Call]:
    """The cases of one clone, in the root order."""
    fixture = FIXTURES[clone.os_name]
    interrupt = Call(
        "interrupt",
        _exec(clone, "interrupt", CHILD_GUEST, child_payload("interrupt")),
        CHILD_GUEST + HOST_MARGIN,
        interrupt_after=CANCEL_AFTER,
        expected_exit=INTERRUPT_EXIT,
        expected_reason="interrupted",
    )
    downloads = [
        _download(clone, f"download-{name.replace('.', '-')}", f"{FIXTURE_ROOT}/{name}", name)
        for name in ORACLE
    ]
    partials = [
        _download(
            clone, f"download-{case}-partial", f"{CHILD_ROOT}/{case}/partial", f"{case}.partial"
        )
        for case in CHILD_CASES
    ]
    return [
        _health(clone, "health-before"),
        _observe(clone, "identity", IDENTITY[clone.os_name]),
        _observe(clone, "bootstrap-dir", f"mkdir -p {FIXTURE_ROOT}"),
        _upload(clone, fixture, "guest.sh"),
        _upload(clone, fixture, "lesson"),
        _guest(
            clone,
            "baseline",
            INVENTORY_GUEST,
            f"bash {FIXTURE_ROOT}/guest.sh inventory",
            INVENTORY,
        ),
        _observe(clone, "install", f"bash {FIXTURE_ROOT}/guest.sh install"),
        _observe(clone, "intact", LESSON),
        *ordinary_calls(clone),
        _observe(clone, "exit-23", EXIT_23),
        _guest(
            clone, "timeout", GUEST_TIMEOUT, child_payload("timeout"), GUEST_TIMEOUT + HOST_MARGIN
        ),
        *after_calls(clone, "timeout"),
        cancel_call(clone),
        *after_calls(clone, "cancel"),
        interrupt,
        _observe(clone, "interrupt-child", child_check("interrupt")),
        _health(clone, "interrupt-health"),
        _cancel(clone, "interrupt-fallback", "interrupt"),
        _observe(clone, "capture", f"bash {FIXTURE_ROOT}/guest.sh intact"),
        *downloads,
        *partials,
        _download(clone, "download-missing", MISSING, "missing.bin", exit_=1),
    ]


def _resource(clone: Clone, golden: str, tart: str, acquire_budget: float) -> Resource:
    start = (
        clone.cli,
        "vm",
        "start",
        "--platform",
        clone.os_name,
        "--base",
        golden,
        "--id",
        clone.vm,
        "--json",
    )
    tools = {"python": clone.python, "cli": clone.cli, "tart": tart}
    owned = _owned(clone.run_id, clone.os_name, start, 0, tools)
    cleanup = _guest(clone, "cleanup", CLEANUP_GUEST, CLEANUP, CLEANUP_BOUND)
    return replace(owned, acquire=replace(owned.acquire, budget=acquire_budget), cleanup=cleanup)


@dataclass(frozen=True)
class Manifest:
    """The fields of the freeze manifest that the plan reads."""

    path: Path
    cli: str
    tart: str
    goldens: dict[str, str]
    boot: float
    slot: float
    sha256: dict[str, str]


def _positive(value: object, name: str) -> float:
    if isinstance(value, bool) or not isinstance(value, int | float):
        raise ValueError(f"manifest bound {name} is not a number")
    number = float(value)
    if not (math.isfinite(number) and number > 0):
        raise ValueError(f"manifest bound {name} is not positive and finite")
    return number


def read_manifest(path: Path) -> Manifest:
    """Read and check the freeze manifest. Refuse a missing or malformed field."""
    data = json.loads(path.read_bytes())
    if not isinstance(data, dict):
        raise ValueError("the manifest is not one JSON object")
    fields = cast(dict[str, Any], data)
    tools: dict[str, str] = {}
    for key in ("cli", "tart"):
        value = fields.get(key)
        if not isinstance(value, str) or not Path(value).is_absolute():
            raise ValueError(f"manifest {key} is not an absolute path")
        tools[key] = value
    goldens = fields.get("goldens")
    if not isinstance(goldens, dict):
        raise ValueError("manifest goldens is not an object")
    names = cast(dict[str, Any], goldens)
    if set(names) != set(OSES) or not all(isinstance(names[key], str) for key in OSES):
        raise ValueError("manifest goldens must name exactly macos and linux")
    bounds = fields.get("bounds")
    if not isinstance(bounds, dict):
        raise ValueError("manifest bounds is not an object")
    bound_values = cast(dict[str, Any], bounds)
    digests = fields.get("sha256")
    if not isinstance(digests, dict):
        raise ValueError("manifest sha256 is not an object")
    frozen = cast(dict[str, Any], digests)
    for key, value in frozen.items():
        if not Path(key).is_absolute() or not isinstance(value, str):
            raise ValueError(f"manifest sha256 entry {key!r} is malformed")
    return Manifest(
        path=path.resolve(),
        cli=tools["cli"],
        tart=tools["tart"],
        goldens={key: cast(str, names[key]) for key in OSES},
        boot=_positive(bound_values.get("boot"), "boot"),
        slot=_positive(bound_values.get("slot"), "slot"),
        sha256={key: cast(str, value) for key, value in frozen.items()},
    )


def _digest(path: Path) -> str | None:
    return hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None


def digest_mismatches(expected: dict[str, str]) -> list[str]:
    """Name each path whose digest differs from its frozen digest."""
    return [
        f"{path}: {actual} != {digest}"
        for path, digest in expected.items()
        if (actual := _digest(Path(path))) != digest
    ]


def fixture_mismatches(os_name: str) -> list[str]:
    """Name each retained fixture file whose digest differs from its frozen digest."""
    fixture = FIXTURES[os_name]
    return digest_mismatches(
        {str(fixture / name): digest for name, digest in FIXTURE_SHA256[os_name].items()}
    )


def plan(manifest: Manifest, os_name: str, index: int, python: str, downloads: Path) -> Plan:
    """The frozen controller plan of one clone."""
    run_id = uuid.uuid4().hex
    clone = Clone(
        os_name=os_name,
        index=index,
        run_id=run_id,
        vm=f"k223-{os_name}-{index:02d}-{run_id}",
        cli=manifest.cli,
        python=python,
        downloads=downloads,
    )
    fixture = FIXTURES[os_name]
    subjects = (
        Path(__file__),
        HERE / "acceptance_k223_cancel.py",
        HERE / "acceptance_k223_check.py",
        HERE / "controller.py",
        HERE / "linux_k204.py",
        HERE / "routes_k19.py",
        HERE / "timeout_k4.py",
        *(fixture / name for name in sorted(FIXTURE_SHA256[os_name])),
        CHECKER,
        REPO / "Taskfile.yml",
        manifest.path,
        Path(manifest.cli),
        Path(manifest.tart),
        Path(python),
        *(Path(path) for path in sorted(manifest.sha256)),
    )
    acquire = manifest.boot + manifest.slot
    return Plan(
        run_id,
        tuple(clone_calls(clone)),
        resources=(_resource(clone, manifest.goldens[os_name], manifest.tart, acquire),),
        subjects=subjects,
        term_grace=TERM_GRACE,
        kill_wait=KILL_WAIT,
    )


def bounds(manifest: Manifest) -> dict[str, float]:
    """Every ceiling and bound that the plan and the checker use, by name."""
    return {
        "observation": OBSERVATION,
        "inventory": INVENTORY,
        "guest_timeout": GUEST_TIMEOUT,
        "host_margin": HOST_MARGIN,
        "transfer": TRANSFER,
        "disposal": DISPOSAL,
        "dispose_call": DISPOSE,
        "guest_cleanup": GUEST_CLEANUP,
        "cleanup_call": CLEANUP_BOUND,
        "term_grace": TERM_GRACE,
        "kill_wait": KILL_WAIT,
        "root_bound_ms": ROOT_BOUND_MS,
        "spawn_ms": SPAWN_MS,
        "lateness_ms": LATENESS_MS,
        "finalise_ms": FINALISE_MS,
        "finalise_terminated_ms": FINALISE_TERMINATED_MS,
        "cancel_reply": CANCEL_REPLY,
        "sigterm_death": SIGTERM_DEATH,
        "health_during": HEALTH_DURING,
        "observe_guest": OBSERVE_GUEST,
        "inventory_guest": INVENTORY_GUEST,
        "child_guest": CHILD_GUEST,
        "cancel_after": CANCEL_AFTER,
        "health_after": HEALTH_AFTER,
        "cancel_wait": CANCEL_WAIT,
        "cleanup_guest": CLEANUP_GUEST,
        "helper": HELPER_BOUND,
        "boot": manifest.boot,
        "slot": manifest.slot,
        "acquire": manifest.boot + manifest.slot,
    }


def run_clone(manifest: Manifest, os_name: str, index: int, output: Path) -> dict[str, Any]:
    """Freeze, run and judge one clone. Return its trial entry."""
    from acceptance_k223_check import check_clone

    directory = output / f"{os_name}-{index:02d}"
    directory.mkdir()
    downloads = directory / "downloads"
    downloads.mkdir()
    frozen = plan(manifest, os_name, index, sys.executable, downloads)
    write_json(
        directory / "inventory.json",
        {
            "leaf": LEAF,
            "os": os_name,
            "index": index,
            "golden": manifest.goldens[os_name],
            "rule": "one plan, run once; no retry; a failed sample stays",
            "manifest": str(manifest.path),
            "fixture_sha256": FIXTURE_SHA256[os_name],
            "bounds": bounds(manifest),
            "plan": asdict(frozen),
        },
    )
    outcome = execute_outcome(frozen, directory)
    write_json(
        directory / "summary.json",
        {
            "verdict": outcome.verdict,
            "run": str(outcome.run_dir),
            "ledger": outcome.ledger,
            "residuals": outcome.residuals,
        },
    )
    verdict = check_clone(directory)
    write_json(directory / "check.json", verdict)
    return {
        "clone": directory.name,
        "run_id": frozen.run_id,
        "controller_verdict": outcome.verdict,
        "check_ok": verdict["ok"],
        "baseline_ok": verdict.get("baseline_ok") is True,
        "passed": outcome.verdict is True and verdict["ok"] is True,
    }


def main() -> None:
    output = Path(sys.argv[1]).resolve()
    manifest = read_manifest(Path(sys.argv[2]))
    refusals = digest_mismatches(manifest.sha256)
    for os_name in OSES:
        refusals += fixture_mismatches(os_name)
    if refusals:
        raise SystemExit(f"frozen subjects differ: {refusals}")
    output.mkdir(parents=True, exist_ok=False)
    order = [(os_name, index) for os_name in OSES for index in range(1, CLONES + 1)]
    clones: list[dict[str, Any]] = []
    for os_name, index in order:
        entry = run_clone(manifest, os_name, index, output)
        clones.append(entry)
        print(f"{entry['clone']}: passed {entry['passed']}", flush=True)
        if entry["passed"] is not True:
            break
    passed = sum(1 for entry in clones if entry["passed"] is True)
    reached = {entry["clone"] for entry in clones}
    unreached = [f"{os}-{n:02d}" for os, n in order if f"{os}-{n:02d}" not in reached]
    write_json(
        output / "trial.json",
        {
            "leaf": LEAF,
            "manifest": str(manifest.path),
            "result": f"{passed}/{len(order)}",
            "passed": passed == len(order),
            "clones": clones,
            "unreached": unreached,
            "reset": reset_record(clones),
        },
    )
    print(f"trial {passed}/{len(order)}", flush=True)
    if passed != len(order):
        raise SystemExit(1)


def reset_record(clones: list[dict[str, Any]]) -> dict[str, str]:
    """Name the clone that demonstrates the reset of each reached clone.

    A clone's reset is demonstrated when the `baseline` of the next clone of the
    same OS passed, because that case found no installation and no marker.
    """
    entries = {str(entry["clone"]): entry for entry in clones}
    record: dict[str, str] = {}
    for os_name in OSES:
        for index in range(1, CLONES + 1):
            name = f"{os_name}-{index:02d}"
            if name not in entries:
                continue
            following = entries.get(f"{os_name}-{index + 1:02d}")
            if index == CLONES:
                record[name] = "undemonstrated: no later clone of this OS"
            elif following is None:
                record[name] = "undemonstrated: the next clone was not reached"
            elif following["baseline_ok"] is True:
                record[name] = f"demonstrated by {following['clone']}"
            else:
                record[name] = f"not demonstrated: the baseline of {following['clone']} failed"
    return record


if __name__ == "__main__":
    main()
