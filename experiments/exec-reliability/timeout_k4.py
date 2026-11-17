"""Post-timeout inspection on one fresh unchanged macOS clone (leaf macos-timeout-inspection-k4).

    timeout_k4.py OUT

The script checks the retained r3 fixture against its frozen hashes. Then it freezes
the plan, its bounds and its subject hashes in OUT/inventory.json before the first
live call. It runs one plan once. The plan owns one clone of the retained golden
`testanyware-golden-macos-27`. The `finally` path of the controller cleans up the
known children and disposes the clone. The calls run in this order:

1. `health-before`: agent health.
2. Condition 1, the exact issue #14 payloads:
   `issue-sleep` (`sleep 120`, guest timeout 2 s), `issue-child` (the fork/wait child
   case, guest timeout 2 s) and `issue-observe` (the known-child `ps` observation,
   guest timeout 30 s, host bound 50 s). No call separates them.
3. `after-01` to `after-10`: ten declared observation and health operations.
4. Condition 2, `r3-*`: the clone-1 sequence of the original macOS VM-fit fixture r3,
   with its guest script and lesson unchanged. Only the VM identity and the local
   download paths differ. The original start and stop calls are the resource
   acquisition and disposal here.

A guest timeout gives host exit 0, so the controller continues after it. The controller
stops the plan only at a host failure. The later calls then stay unreached and recorded.
timeout_k4_check.py judges the guest receipts afterwards.
"""

import hashlib
import sys
import uuid
from dataclasses import asdict, replace
from pathlib import Path

from controller import Call, Plan, Resource, execute_outcome, write_json
from routes_k19 import (  # the k19 resource shape, reused unchanged
    AGENTS,
    APP,
    BOOT,
    CHECKER,
    HEALTH,
    HERE,
    REPO,
    _owned,  # pyright: ignore[reportPrivateUsage]
)

LEAF = "macos-timeout-inspection-k4"
GOLDEN = "testanyware-golden-macos-27"
GUEST_TIMEOUT = 2  # intentional guest timeout
OBSERVE = 30  # ordinary observation guest budget
INVENTORY = 60  # inventory guest budget
CANCEL_GUEST = 20  # guest deadline of the r3 cancel case
CANCEL_AFTER = 2.0  # the r3 host interrupts its CLI after 2 s
CLEANUP_GUEST = 6  # three known children, at most 2 s of TERM grace each
# The guest cleanup ceiling is 30 s. The controller reserves 2 s TERM and 2 s KILL.
CLEANUP_BOUND = 26.0
TRANSFER = 150.0  # transfer ceiling
HOST_MARGIN = 20.0  # raw host exec: guest budget plus 20 s

FIXTURE = HERE / "evidence" / LEAF / "fixture-r3"
# The hashes from the retained frozen.json of the original VM-fit run, revision r3.
FIXTURE_SHA256 = {
    "cases.json": "772dacf8888fbf2d8e0b24f984069f1bd487ad463f836dd32530a1dd79a1f6c8",
    "guest.sh": "e379541d8940d9953abc49a2dec8e17e3e4cc406954aed94e0e3d0b92158545b",
    "lesson": "0a649e33d19014a527aaf098dd8ad4c671d18bb02b842eaa4f1fa515f31ea23c",
}
ROOT = "/tmp/vmfit-k15"  # the fixture's own guest root, fixed inside guest.sh

ISSUE_SLEEP = "sleep 120"
ISSUE_CHILD = "\n".join(
    (
        "mkdir -p /tmp/taw-exec-reliability",
        "sleep 120 & child=$!",
        "printf '%s\\n' \"$child\" > /tmp/taw-exec-reliability/child.pid",
        "printf 'partial\\n' > /tmp/taw-exec-reliability/partial",
        "wait",
    )
)
ISSUE_OBSERVE = "\n".join(
    (
        "pid=$(cat /tmp/taw-exec-reliability/child.pid)",
        "printf 'child pid=%s\\n' \"$pid\"",
        'ps -p "$pid" -o pid=,ppid=,stat=,comm=',
    )
)
AGENT_PIDS = "ps -axo pid=,comm= | awk '$2 ~ /testanyware-agent$/ {print $1}' | paste -sd, -"
TABLE = "ps -axo pid,ppid,pgid,stat,lstart,etime,command"
# k186 found no process named exactly `testanyware-agent` with pgrep -x, so this
# operation first lists every process whose command name contains "anyware".
THREADS = (
    "ps -axo pid=,ppid=,user=,comm= | grep -i anyware; "
    f'agent=$({AGENT_PIDS}); printf "agent pids=%s\\n" "$agent"; ps -M -p "$agent"'
)
IDENTITY = (
    "shasum -a 256 /usr/local/bin/testanyware-agent; sw_vers -productVersion; uname -m; "
    "sysctl hw.ncpu hw.activecpu kern.wq_max_threads kern.wq_max_constrained_threads"
)
ORDINARY = "printf ordinary-ok"
# The cleanup acts only on the known children: a PID read from a file that this plan
# wrote, and only while that PID still runs exactly `sleep 120`. TERM has a grace of
# at most 2 s. KILL follows only for a survivor. Disposal removes everything else.
OWNED_CLEANUP = "\n".join(
    (
        f"for f in /tmp/taw-exec-reliability/child.pid {ROOT}/timeout.pid {ROOT}/cancel.pid; do",
        '  [ -f "$f" ] || continue',
        '  pid=$(cat "$f")',
        "  cmd=$(ps -p \"$pid\" -o command= | sed 's/ *$//')",
        '  printf \'%s pid=%s command=%s\\n\' "$f" "$pid" "$cmd"',
        '  [ "$cmd" = "sleep 120" ] || continue',
        '  kill -TERM "$pid"',
        "  i=0",
        '  while kill -0 "$pid" 2>/dev/null && [ "$i" -lt 20 ]; do sleep 0.1; i=$((i + 1)); done',
        '  if kill -0 "$pid" 2>/dev/null; then',
        '    kill -KILL "$pid"; printf "killed pid=%s\\n" "$pid"',
        '  else printf "terminated pid=%s\\n" "$pid"; fi',
        "done",
        "printf 'cleanup complete\\n'",
    )
)


def _exec(cli: str, vm: str, guest_timeout: int, command: str) -> tuple[str, ...]:
    return (cli, "file", "exec", "--vm", vm, "--timeout", str(guest_timeout), "--json", command)


def _guest(cli: str, vm: str, name: str, guest_timeout: int, command: str) -> Call:
    return Call(name, _exec(cli, vm, guest_timeout, command), guest_timeout + HOST_MARGIN)


def _health(cli: str, vm: str, name: str) -> Call:
    return Call(name, (cli, "agent", "health", "--vm", vm, "--json"), HEALTH)


def after_calls(cli: str, vm: str) -> list[Call]:
    """The ten declared after-timeout operations, in their frozen order."""
    return [
        _health(cli, vm, "after-01-health"),
        _guest(cli, vm, "after-02-observe", OBSERVE, ISSUE_OBSERVE),
        _guest(cli, vm, "after-03-table", OBSERVE, TABLE),
        _guest(cli, vm, "after-04-threads", OBSERVE, THREADS),
        _guest(cli, vm, "after-05-ordinary", OBSERVE, ORDINARY),
        _guest(cli, vm, "after-06-ordinary", OBSERVE, ORDINARY),
        _guest(cli, vm, "after-07-identity", OBSERVE, IDENTITY),
        _guest(cli, vm, "after-08-observe", OBSERVE, ISSUE_OBSERVE),
        _guest(cli, vm, "after-09-table", OBSERVE, TABLE),
        _health(cli, vm, "after-10-health"),
    ]


def r3_calls(cli: str, vm: str, fixture: Path, downloads: Path) -> list[Call]:
    """The clone-1 sequence of fixture r3 between its start and its stop."""

    def case(name: str, guest_timeout: int, action: str) -> Call:
        return _guest(cli, vm, f"r3-{name}", guest_timeout, f"bash {ROOT}/guest.sh {action}")

    def upload(name: str) -> Call:
        argv = (cli, "file", "upload", "--vm", vm, str(fixture / name), f"{ROOT}/{name}", "--json")
        return Call(f"r3-upload-{name}", argv, TRANSFER)

    def download(name: str, expected_exit: int = 0, label: str | None = None) -> Call:
        local = str(downloads / name if label is None else downloads / f"{label}.bin")
        argv = (cli, "file", "download", "--vm", vm, f"{ROOT}/{name}", local, "--json")
        return Call(f"r3-{label or 'download-' + name}", argv, TRANSFER, expected_exit)

    cancel = Call(
        "r3-cancel",
        _exec(cli, vm, CANCEL_GUEST, f"bash {ROOT}/guest.sh cancel"),
        CANCEL_GUEST + HOST_MARGIN,
        interrupt_after=CANCEL_AFTER,
        expected_reason="interrupted",
    )
    return [
        _guest(cli, vm, "r3-bootstrap-dir", OBSERVE, f"mkdir -p {ROOT}"),
        upload("guest.sh"),
        upload("lesson"),
        case("inventory", INVENTORY, "inventory"),
        case("install", OBSERVE, "install"),
        case("intact", OBSERVE, "intact"),
        *(download(name) for name in ("stdout.bin", "stderr.bin", "artifact.bin", "status.txt")),
        case("online", OBSERVE, "online"),
        case("nonzero", OBSERVE, "nonzero"),
        case("bad-install", OBSERVE, "bad-install"),
        case("timeout", GUEST_TIMEOUT, "timeout"),
        case("observe-timeout", OBSERVE, "observe"),
        cancel,
        case("observe-cancel", OBSERVE, "observe"),
        download("timeout.partial"),
        download("cancel.partial"),
        download("does-not-exist", expected_exit=1, label="missing-artifact"),
        case("guest-cleanup", OBSERVE, "cleanup"),
        case("observe-after-cleanup", OBSERVE, "observe"),
    ]


def _resource(run_id: str, start: tuple[str, ...], vm: str, tools: dict[str, str]) -> Resource:
    owned = _owned(run_id, "macos", start, 0, tools)
    cleanup = Call("cleanup", _exec(tools["cli"], vm, CLEANUP_GUEST, OWNED_CLEANUP), CLEANUP_BOUND)
    return replace(owned, cleanup=cleanup)


def plan(tools: dict[str, str], downloads: Path) -> Plan:
    cli = tools["cli"]
    run_id = uuid.uuid4().hex
    vm = f"k4-macos-{run_id}"
    calls = [
        _health(cli, vm, "health-before"),
        _guest(cli, vm, "issue-sleep", GUEST_TIMEOUT, ISSUE_SLEEP),
        _guest(cli, vm, "issue-child", GUEST_TIMEOUT, ISSUE_CHILD),
        _guest(cli, vm, "issue-observe", OBSERVE, ISSUE_OBSERVE),
        *after_calls(cli, vm),
        *r3_calls(cli, vm, FIXTURE, downloads),
    ]
    subjects = (
        Path(__file__),
        HERE / "routes_k19.py",
        HERE / "controller.py",
        HERE / "timeout_k4_check.py",
        HERE / "timeout_k4_run.sh",
        *(FIXTURE / name for name in sorted(FIXTURE_SHA256)),
        CHECKER,
        REPO / "Taskfile.yml",
        Path(cli).resolve(),
        Path(tools["tart"]).resolve(),
        Path(tools["python"]),
        APP,
        APP.parent.parent / "Info.plist",
        (AGENTS / "macos/testanyware-agent").resolve(),
    )
    start = (cli, "vm", "start", "--platform", "macos", "--base", GOLDEN, "--id", vm, "--json")
    return Plan(
        run_id,
        tuple(calls),
        resources=(_resource(run_id, start, vm, tools),),
        subjects=subjects,
    )


def fixture_mismatches(fixture: Path) -> list[str]:
    """Name each retained fixture file whose hash differs from the frozen r3 hash."""
    mismatches: list[str] = []
    for name, expected in FIXTURE_SHA256.items():
        path = fixture / name
        actual = hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
        if actual != expected:
            mismatches.append(f"{name}: {actual} != {expected}")
    return mismatches


def main() -> None:
    import shutil

    output = Path(sys.argv[1]).resolve()
    cli, tart = shutil.which("testanyware"), shutil.which("tart")
    if cli is None or tart is None:
        raise SystemExit("unavailable prerequisite: testanyware and tart must be on PATH")
    mismatches = fixture_mismatches(FIXTURE)
    if mismatches:
        raise SystemExit(f"retained fixture differs from frozen r3: {mismatches}")
    tools = {"python": sys.executable, "cli": cli, "tart": tart}
    output.mkdir(parents=True, exist_ok=False)
    downloads = output / "r3-downloads"
    downloads.mkdir()
    frozen = plan(tools, downloads)
    write_json(
        output / "inventory.json",
        {
            "leaf": LEAF,
            "golden": GOLDEN,
            "rule": "one plan, run once; no retry; failed samples stay",
            "fixture_sha256": FIXTURE_SHA256,
            "bounds": {
                "boot": BOOT,
                "health": HEALTH,
                "guest_timeout": GUEST_TIMEOUT,
                "observe": OBSERVE,
                "inventory": INVENTORY,
                "host_margin": HOST_MARGIN,
                "cancel_guest": CANCEL_GUEST,
                "cancel_after": CANCEL_AFTER,
                "transfer": TRANSFER,
                "cleanup_guest": CLEANUP_GUEST,
                "cleanup_bound": CLEANUP_BOUND,
            },
            "plan": asdict(frozen),
        },
    )
    outcome = execute_outcome(frozen, output)
    write_json(
        output / "summary.json",
        {
            "verdict": outcome.verdict,
            "run": str(outcome.run_dir),
            "ledger": outcome.ledger,
            "residuals": outcome.residuals,
        },
    )
    print(f"verdict {outcome.verdict} run {outcome.run_dir}", flush=True)
    if outcome.verdict is not True:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
