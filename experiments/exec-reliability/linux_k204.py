"""Live timeout lifecycle on one fresh unchanged Linux clone (leaf linux-timeout-live-k204).

    linux_k204.py OUT

The script checks the retained r5 fixture against its frozen hashes. Then it freezes
the plan, its bounds and its subject hashes in OUT/inventory.json before the first
live call. It runs one plan once. The plan owns one clone of the retained golden
`testanyware-golden-linux-24.04`. The clone runs the agent of the golden unchanged.
The `finally` path of the controller cleans up the known children and disposes the
clone. The calls run in this order:

1. `health-before`, `identity` and `table-before`: agent health, the agent sources
   and process, and one full process table.
2. The exact `sleep 120` control with guest timeout 2 s, once through the Host CLI
   (`cli-sleep`) and once straight to the agent (`raw-sleep`). A full process table
   follows each control. Bash runs a lone simple command by exec, so the guest
   `sleep` is the shell itself and cannot record its own identity.
3. The fork/wait control with guest timeout 2 s, once through the Host CLI
   (`cli-fork`) and once straight to the agent (`raw-fork`). The shell records its
   own PID and the PID of its `sleep 120` child. It writes partial bytes to stdout,
   stderr and a file before `wait`. An independent observation of the shell and the
   child and a download of the partial file follow each control.
4. `health-mid`, a late observation of both fork controls after a further 5 s, the
   owned cleanup of the two fork children, an observation after it and
   `table-before-r5`.
5. `r5-*`: the clone-1 sequence of the original Linux VM-fit fixture r5, with its
   guest script and lesson unchanged. Each call has the argv and the guest timeout
   of the original call, from the retained clone-1 records of `linux-03`. Only the
   VM identity and the local download paths differ. The original start and stop
   calls are the resource acquisition and disposal here.

A guest timeout gives host exit 0, and a raw call exits 0 whenever it prints its
record. So the controller continues after each control and stops the plan only at a
host failure. The later calls then stay unreached and recorded. linux_k204_check.py
judges the receipts afterwards.
"""

import hashlib
import json
import sys
import uuid
from collections.abc import Sequence
from dataclasses import asdict, replace
from pathlib import Path

from controller import Call, Plan, Resource, execute_outcome, write_json
from routes_k19 import (  # the k19 resource shape, reused unchanged
    AGENTS,
    BOOT,
    CHECKER,
    DISPOSE,
    GUEST_IDENTITY,
    HEALTH,
    HERE,
    REPO,
    _owned,  # pyright: ignore[reportPrivateUsage]
)

LEAF = "linux-timeout-live-k204"
GOLDEN = "testanyware-golden-linux-24.04"
GUEST_TIMEOUT = 2  # intentional guest timeout of each control
HOST_MARGIN = 20.0  # host bound: guest budget plus 20 s
OBSERVE_GUEST = 10  # own observations: guest 10 s, host 30 s, the observation bound
OBSERVE_BOUND = 30.0
# Four known children at most, each with at most 2 s of TERM grace and 0.5 s of KILL wait.
CLEANUP_GUEST = 10
# The guest cleanup ceiling is 30 s. The controller reserves 2 s TERM and 2 s KILL.
CLEANUP_BOUND = 26.0
TRANSFER = 150.0  # transfer ceiling, the command host budget of fixture r5
R5_OBSERVE = 30  # the guest timeout of the ordinary r5 calls in linux-03
R5_INVENTORY = 60
R5_ONLINE = 120
R5_CANCEL_GUEST = 20  # the r5 cancel case: guest deadline 20 s, interrupt after 2 s
R5_CANCEL_AFTER = 2.0

FIXTURE = HERE / "evidence" / LEAF / "fixture-r5"
# The hashes from the retained frozen.json of the original VM-fit run, revision r5.
FIXTURE_SHA256 = {
    "cases.json": "ad97fe2e5f9965809684d3efee7de60e998b06121ad23636bcef85c415aae97b",
    "guest.sh": "2f1c022a6dac48fb6d026d63331199c5b6bfdb7cef91e22549859ba40aabfe80",
    "lesson": "0a649e33d19014a527aaf098dd8ad4c671d18bb02b842eaa4f1fa515f31ea23c",
}
FROZEN_SHA256 = "a954f67cd4412af281b641c605d48207d98ba964154aee0434bd226fca8389c8"
R5_ROOT = "/tmp/vmfit-k15"  # the fixture's own guest root, fixed inside guest.sh
ONLINE = json.loads((FIXTURE / "cases.json").read_bytes())["platforms"]["linux"]

K204_ROOT = "/tmp/taw-k204"
FORK_LABELS = ("cli-fork", "raw-fork")
ISSUE_SLEEP = "sleep 120"


def fork_wait(label: str) -> str:
    """The fork/wait control: the failure-B shape plus the shell's own PID and partial bytes."""
    return "\n".join(
        (
            f"d={K204_ROOT}/{label}",
            'mkdir -p "$d"',
            'printf \'%s\\n\' "$$" > "$d/shell.pid"',
            "sleep 120 & p=$!",
            'printf \'%s\\n\' "$p" > "$d/child.pid"',
            "printf 'partial-out\\n'",
            "printf 'partial-err\\n' >&2",
            "printf 'partial-file\\n' > \"$d/partial\"",
            "wait",
        )
    )


def observe(labels: Sequence[str]) -> str:
    """Observe each recorded shell and child by its PID file, one `ps` per PID.

    Each PID gives `<role> pid=N`, the `ps` row when `ps` lists it, and then
    `<role> ps-exit=E`. An unreadable PID file gives `<role> pid-file=unreadable`.
    """
    lines: list[str] = []
    for label in labels:
        lines += [
            "for role in shell child; do",
            f"  f={K204_ROOT}/{label}/$role.pid; r={label}-$role",
            '  if ! pid=$(cat "$f"); then printf \'%s pid-file=unreadable\\n\' "$r"; continue; fi',
            '  printf \'%s pid=%s\\n\' "$r" "$pid"',
            '  ps -p "$pid" -o pid=,ppid=,pgid=,sid=,stat=,lstart=,args=',
            '  printf \'%s ps-exit=%s\\n\' "$r" "$?"',
            "done",
        ]
    return "\n".join(lines)


# A child that `ps` still lists here outlived its deadline by at least 5 s. That is
# beyond the 2 s grace plus the 2 s kill and reap bound, whatever the host delays were.
LATE_WAIT = 5
LATE_OBSERVE = f"sleep {LATE_WAIT}\n{observe(FORK_LABELS)}"
TABLE = "ps -eo pid,ppid,pgid,sid,stat,lstart,etime,args"
IDENTITY = (
    f"{GUEST_IDENTITY['linux']}; python3 --version; id; "
    "ps -eo pid,ppid,pgid,sid,user,lstart,args | grep -F -e testanyware -e PID | grep -v -F grep"
)


def owned_cleanup(pid_files: Sequence[str]) -> str:
    """Clean up only the known children, by PID files that this plan or fixture wrote.

    The cleanup acts on a PID only while it still runs exactly `sleep 120`. TERM has
    a grace of at most 2 s. KILL follows only for a survivor. Disposal removes
    everything else. `state` reads `kill -0`: alive, gone (no such process) or
    unknown (any other error). Each PID ends as `gone`, `other` (the PID now runs
    another command), `terminated`, `killed`, `unknown` or `survived`. The last line
    is `cleanup confirmed` only when no PID ended as `unknown` or `survived`.
    Otherwise it is `cleanup uncertain`, and the command exits 1. This is the k4
    cleanup with the Linux `args` column and the k204 PID files.
    """
    return "\n".join(
        (
            "state() {",
            '  if err=$(kill -0 "$1" 2>&1); then echo alive',
            '  else case "$err" in',
            "    *[Nn]o\\ such\\ process*) echo gone ;;",
            "    *) echo unknown ;;",
            "  esac; fi",
            "}",
            "wait_gone() {",
            "  i=0",
            '  while [ "$(state "$1")" = alive ] && [ "$i" -lt "$2" ]; do',
            "    sleep 0.1; i=$((i + 1))",
            "  done",
            "}",
            "outcome=confirmed",
            f"for f in {' '.join(pid_files)}; do",
            '  [ -f "$f" ] || continue',
            '  pid=$(cat "$f")',
            "  cmd=$(ps -p \"$pid\" -o args= | sed 's/ *$//')",
            '  printf \'%s pid=%s command=%s\\n\' "$f" "$pid" "$cmd"',
            '  if [ "$cmd" != "sleep 120" ]; then',
            '    s=$(state "$pid")',
            '    if [ -z "$cmd" ] && [ "$s" = gone ]; then printf \'gone pid=%s\\n\' "$pid"',
            '    elif [ -n "$cmd" ] && [ "$s" = alive ]; then printf \'other pid=%s\\n\' "$pid"',
            '    else printf \'unknown pid=%s state=%s\\n\' "$pid" "$s"; outcome=uncertain; fi',
            "    continue",
            "  fi",
            '  kill -TERM "$pid"',
            '  wait_gone "$pid" 20',
            "  action=terminated",
            '  if [ "$(state "$pid")" = alive ]; then',
            '    kill -KILL "$pid"; action=killed',
            '    wait_gone "$pid" 5',
            "  fi",
            '  s=$(state "$pid")',
            '  if [ "$s" = gone ]; then printf \'%s pid=%s\\n\' "$action" "$pid"',
            '  else printf \'survived pid=%s state=%s\\n\' "$pid" "$s"; outcome=uncertain; fi',
            "done",
            "printf 'cleanup %s\\n' \"$outcome\"",
            '[ "$outcome" = confirmed ]',
        )
    )


CHILD_FILES = tuple(f"{K204_ROOT}/{label}/child.pid" for label in FORK_LABELS)
CONTROL_CLEANUP = owned_cleanup(CHILD_FILES)
FINAL_CLEANUP = owned_cleanup((*CHILD_FILES, f"{R5_ROOT}/timeout.pid", f"{R5_ROOT}/cancel.pid"))


def _safe(name: str) -> str:
    """Make a fixture file name a call name: `guest.sh` becomes `guest-sh`."""
    return name.replace(".", "-")


def _exec(cli: str, vm: str, guest_timeout: int, command: str) -> tuple[str, ...]:
    return (cli, "file", "exec", "--vm", vm, "--timeout", str(guest_timeout), "--json", command)


def _guest(cli: str, vm: str, name: str, guest_timeout: int, command: str) -> Call:
    return Call(name, _exec(cli, vm, guest_timeout, command), guest_timeout + HOST_MARGIN)


def _raw(python: str, vm: str, name: str, guest_timeout: int, command: str) -> Call:
    argv = (python, "-I", str(HERE / "linux_k204_raw.py"), vm, str(guest_timeout), command)
    return Call(name, argv, guest_timeout + HOST_MARGIN)


def _health(cli: str, vm: str, name: str) -> Call:
    return Call(name, (cli, "agent", "health", "--vm", vm, "--json"), HEALTH)


def _download(cli: str, vm: str, name: str, remote: str, local: Path, exit_: int = 0) -> Call:
    argv = (cli, "file", "download", "--vm", vm, remote, str(local), "--json")
    return Call(name, argv, TRANSFER, expected_exit=exit_)


def control_calls(tools: dict[str, str], vm: str, downloads: Path) -> list[Call]:
    """Steps 1 to 4: the exact controls, their observations and the owned cleanup."""
    cli, python = tools["cli"], tools["python"]

    def fork(prefix: str, call: Call) -> list[Call]:
        label = f"{prefix}-fork"
        partial = _download(
            cli,
            vm,
            f"download-{label}-partial",
            f"{K204_ROOT}/{label}/partial",
            downloads / f"{label}.partial",
        )
        observation = Call(
            f"observe-{label}", _exec(cli, vm, OBSERVE_GUEST, observe((label,))), OBSERVE_BOUND
        )
        return [call, observation, partial]

    def table(name: str) -> Call:
        return Call(name, _exec(cli, vm, OBSERVE_GUEST, TABLE), OBSERVE_BOUND)

    return [
        _health(cli, vm, "health-before"),
        Call("identity", _exec(cli, vm, OBSERVE_GUEST, IDENTITY), OBSERVE_BOUND),
        table("table-before"),
        _guest(cli, vm, "cli-sleep", GUEST_TIMEOUT, ISSUE_SLEEP),
        table("table-after-cli-sleep"),
        _raw(python, vm, "raw-sleep", GUEST_TIMEOUT, ISSUE_SLEEP),
        table("table-after-raw-sleep"),
        *fork("cli", _guest(cli, vm, "cli-fork", GUEST_TIMEOUT, fork_wait("cli-fork"))),
        *fork("raw", _raw(python, vm, "raw-fork", GUEST_TIMEOUT, fork_wait("raw-fork"))),
        _health(cli, vm, "health-mid"),
        Call("observe-forks-late", _exec(cli, vm, OBSERVE_GUEST, LATE_OBSERVE), OBSERVE_BOUND),
        Call("control-cleanup", _exec(cli, vm, CLEANUP_GUEST, CONTROL_CLEANUP), CLEANUP_BOUND),
        Call(
            "observe-after-control-cleanup",
            _exec(cli, vm, OBSERVE_GUEST, observe(FORK_LABELS)),
            OBSERVE_BOUND,
        ),
        table("table-before-r5"),
    ]


def r5_calls(cli: str, vm: str, fixture: Path, downloads: Path) -> list[Call]:
    """Step 5: the clone-1 sequence of fixture r5 between its start and its stop."""

    def case(name: str, guest_timeout: int, action: str) -> Call:
        return _guest(cli, vm, f"r5-{name}", guest_timeout, f"bash {R5_ROOT}/guest.sh {action}")

    def upload(name: str) -> Call:
        argv = (
            cli,
            "file",
            "upload",
            "--vm",
            vm,
            str(fixture / name),
            f"{R5_ROOT}/{name}",
            "--json",
        )
        return Call(f"r5-upload-{_safe(name)}", argv, TRANSFER)

    def download(name: str) -> Call:
        return _download(
            cli, vm, f"r5-download-{_safe(name)}", f"{R5_ROOT}/{name}", downloads / name
        )

    cancel = Call(
        "r5-cancel",
        _exec(cli, vm, R5_CANCEL_GUEST, f"bash {R5_ROOT}/guest.sh cancel"),
        R5_CANCEL_GUEST + HOST_MARGIN,
        interrupt_after=R5_CANCEL_AFTER,
        # The controller sends TERM to the CLI, which then exits by that signal.
        expected_exit=-15,
        expected_reason="interrupted",
    )
    missing = _download(
        cli,
        vm,
        "r5-missing-artifact",
        f"{R5_ROOT}/does-not-exist",
        downloads / "missing-artifact.bin",
        exit_=1,
    )
    online = f"online {ONLINE['url']} {ONLINE['sha256']}"
    return [
        _health(cli, vm, "r5-health"),
        _guest(cli, vm, "r5-bootstrap-dir", R5_OBSERVE, f"mkdir -p {R5_ROOT}"),
        upload("guest.sh"),
        upload("lesson"),
        case("inventory", R5_INVENTORY, "inventory"),
        case("install", R5_OBSERVE, "install"),
        case("intact", R5_OBSERVE, "intact"),
        *(download(name) for name in ("stdout.bin", "stderr.bin", "artifact.bin", "status.txt")),
        case("online", R5_ONLINE, online),
        case("nonzero", R5_OBSERVE, "nonzero"),
        case("bad-install", R5_OBSERVE, "bad-install"),
        case("timeout", GUEST_TIMEOUT, "timeout"),
        case("observe-timeout", R5_OBSERVE, "observe"),
        cancel,
        case("observe-cancel", R5_OBSERVE, "observe"),
        download("timeout.partial"),
        download("cancel.partial"),
        missing,
        case("guest-cleanup", R5_OBSERVE, "cleanup"),
        case("observe-after-cleanup", R5_OBSERVE, "observe"),
    ]


def _resource(run_id: str, start: tuple[str, ...], vm: str, tools: dict[str, str]) -> Resource:
    owned = _owned(run_id, "linux", start, 0, tools)
    cleanup = Call("cleanup", _exec(tools["cli"], vm, CLEANUP_GUEST, FINAL_CLEANUP), CLEANUP_BOUND)
    return replace(owned, cleanup=cleanup)


def plan(tools: dict[str, str], output: Path) -> Plan:
    cli = tools["cli"]
    run_id = uuid.uuid4().hex
    vm = f"k204-linux-{run_id}"
    calls = [
        *control_calls(tools, vm, output / "k204-downloads"),
        *r5_calls(cli, vm, FIXTURE, output / "r5-downloads"),
    ]
    subjects = (
        Path(__file__),
        HERE / "linux_k204_raw.py",
        HERE / "linux_k204_check.py",
        HERE / "linux_k204_run.sh",
        HERE / "routes_k19.py",
        HERE / "controller.py",
        *(FIXTURE / name for name in sorted(FIXTURE_SHA256)),
        FIXTURE / "frozen.json",
        CHECKER,
        REPO / "Taskfile.yml",
        Path(cli).resolve(),
        Path(tools["tart"]).resolve(),
        Path(tools["python"]),
        *sorted((AGENTS / "linux/testanyware_agent").resolve().glob("*.py")),
    )
    start = (cli, "vm", "start", "--platform", "linux", "--base", GOLDEN, "--id", vm, "--json")
    return Plan(
        run_id,
        tuple(calls),
        resources=(_resource(run_id, start, vm, tools),),
        subjects=subjects,
    )


def fixture_mismatches(fixture: Path) -> list[str]:
    """Name each retained fixture file whose hash differs from the frozen r5 hash."""
    expected = {**FIXTURE_SHA256, "frozen.json": FROZEN_SHA256}
    mismatches: list[str] = []
    for name, digest in expected.items():
        path = fixture / name
        actual = hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
        if actual != digest:
            mismatches.append(f"{name}: {actual} != {digest}")
    return mismatches


def main() -> None:
    import shutil

    output = Path(sys.argv[1]).resolve()
    cli, tart = shutil.which("testanyware"), shutil.which("tart")
    if cli is None or tart is None:
        raise SystemExit("unavailable prerequisite: testanyware and tart must be on PATH")
    mismatches = fixture_mismatches(FIXTURE)
    if mismatches:
        raise SystemExit(f"retained fixture differs from frozen r5: {mismatches}")
    tools = {"python": sys.executable, "cli": cli, "tart": tart}
    output.mkdir(parents=True, exist_ok=False)
    (output / "k204-downloads").mkdir()
    (output / "r5-downloads").mkdir()
    frozen = plan(tools, output)
    write_json(
        output / "inventory.json",
        {
            "leaf": LEAF,
            "golden": GOLDEN,
            "rule": "one plan, run once; no retry; failed samples stay",
            "fixture_sha256": {**FIXTURE_SHA256, "frozen.json": FROZEN_SHA256},
            "bounds": {
                "boot": BOOT,
                "health": HEALTH,
                "guest_timeout": GUEST_TIMEOUT,
                "host_margin": HOST_MARGIN,
                "observe_guest": OBSERVE_GUEST,
                "observe_bound": OBSERVE_BOUND,
                "cleanup_guest": CLEANUP_GUEST,
                "cleanup_bound": CLEANUP_BOUND,
                "transfer": TRANSFER,
                "dispose": DISPOSE,
                "r5_observe": R5_OBSERVE,
                "r5_inventory": R5_INVENTORY,
                "r5_online": R5_ONLINE,
                "r5_cancel_guest": R5_CANCEL_GUEST,
                "r5_cancel_after": R5_CANCEL_AFTER,
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
