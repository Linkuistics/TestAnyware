"""Linux disconnect availability (leaf linux-disconnect-availability-k6).

    linux_k6.py container OUT
    linux_k6.py live OUT

Each mode freezes one plan, then runs it once with no retry.

`container` owns one fresh container of the pinned k5 image. The container runs
`linux_k6_seam.py RUN cases` against the unchanged agent from this checkout. The
seam docstring lists the seven cases. The controller disposes the container in
its `finally` path.

`live` owns one fresh clone of the unchanged golden `testanyware-golden-linux-24.04`.
Between health checks, it runs `disconnect_k13_burst.py` unchanged three times:
`control`, `concurrent` and `term`. The burst docstring holds what each mode does.
The owned cleanup is the k204 cleanup over the three child PID files. The
controller disposes the clone in its `finally` path.

The script writes OUT/inventory.json before the first call, then OUT/summary.json.
It prints `verdict VERDICT run RUN`. It exits with status 1 when the controller
verdict is not true. `linux_k6_check.py` judges the receipts.
"""

from __future__ import annotations

import shutil
import sys
import uuid
from collections.abc import Sequence
from dataclasses import asdict, replace
from pathlib import Path

import disconnect_k13_burst as burst
import linux_k6_seam as seam
from controller import Call, Plan, Resource, execute_outcome, write_json
from linux_k5 import IMAGE, STUB, docker_path, image_record, local_modules, run_tag, sha256
from linux_k204 import CLEANUP_BOUND, CLEANUP_GUEST, GOLDEN
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

LEAF = "linux-disconnect-availability-k6"
SEAM = HERE / "linux_k6_seam.py"
BURST = HERE / "disconnect_k13_burst.py"
AGENT_PACKAGE = REPO / "agents/linux/testanyware_agent"
LABEL = "testanyware.k6.run"
DOCKER_BOUND = 60.0  # one docker create or inspect, inside the 60 s inventory ceiling
DISPOSE_BOUND = 40.0  # `docker rm -f` plus the 4 s reserve, inside the 45 s disposal ceiling
STOP_BOUND = 20.0  # `docker stop -t 2` plus margin, inside the 30 s cleanup ceiling
# Each case can take the ready bound, the observation bound, the host deadline,
# the owned kill wait and two agent stop waits. The last 30 s cover the start of
# the container and the interpreter.
CASES_BUDGET = (
    len(seam.CASES)
    * (
        seam.READY_BOUND
        + seam.OBSERVE_BOUND
        + seam.HOST_DEADLINE
        + seam.KILL_WAIT
        + 2 * seam.STOP_WAIT
    )
    + 30.0
)
# As in k13: the longest burst mode is `control`. It waits up to the original
# bound, then runs six bounded requests and one transfer. Each of the seven
# requests and the original can need a TERM wait and a KILL wait. The last 10 s
# cover interpreter start and the JSON output.
BURST_BUDGET = (
    burst.ORIGINAL_BOUND
    + burst.TRANSFER_BOUND
    + 6 * burst.FOLLOW_BOUND
    + 8 * 2 * burst.STOP_WAIT
    + 10.0
)
CHILD_FILES = tuple(f"{burst.guest_dir(mode)}/child.pid" for mode in burst.MODES)
CLEANUP_EARLY = 5.0  # the child can start this long before its PID file is written
CLEANUP_LATE = 2.0  # /proc/stat gives the boot time in whole seconds
CLEANUP_WAIT = 2.0  # one shared wait after TERM, and one after KILL

# The guest program of `pidfd_cleanup`. It runs as `python3 -I -` under
# `/bin/bash -c`. Its first line is replaced by the PID file list.
PIDFD_PROGRAM = """FILES = []
import os, select, signal, sys, time
EARLY, LATE, WAIT = {early}, {late}, {wait}
HZ = os.sysconf("SC_CLK_TCK")


def boot_time():
    for line in open("/proc/stat"):
        if line.startswith("btime "):
            return int(line.split()[1])
    raise OSError("no btime in /proc/stat")


def identity(pid):
    stat = open(f"/proc/{{pid}}/stat", "rb").read()
    ticks = int(stat[stat.rindex(b")") + 2 :].split()[19])
    command = open(f"/proc/{{pid}}/cmdline", "rb").read().replace(b"\\0", b" ").strip()
    return command.decode(errors="replace"), boot_time() + ticks / HZ


def exited(fd, seconds):
    poll = select.poll()
    poll.register(fd, select.POLLIN)
    return bool(poll.poll(max(0, int(seconds * 1000))))


def signal_fd(fd, sig):
    try:
        signal.pidfd_send_signal(fd, sig)
        return True
    except ProcessLookupError:
        return False


uncertain = False
held = []
for path in FILES:
    try:
        text = open(path).read().strip()
        written = os.stat(path).st_mtime
    except FileNotFoundError:
        continue
    except OSError as error:
        print(f"{{path}} unreadable: {{error}}")
        uncertain = True
        continue
    try:
        pid = int(text)
        fd = os.pidfd_open(pid)
    except ProcessLookupError:
        print(f"{{path}} pid={{text}} result=gone")
        continue
    except (ValueError, OSError) as error:
        print(f"{{path}} pid={{text!r}} result=unknown error={{error}}")
        uncertain = True
        continue
    try:
        command, started = identity(pid)
    except (FileNotFoundError, ProcessLookupError):
        command, started = None, None
    except OSError as error:
        print(f"{{path}} pid={{pid}} result=unknown error={{error}}")
        uncertain = True
        os.close(fd)
        continue
    if command is None or exited(fd, 0):
        print(f"{{path}} pid={{pid}} result=gone")
        os.close(fd)
        continue
    print(f"{{path}} pid={{pid}} command={{command}}", end=" ")
    print(f"started={{started:.2f}} written={{written:.2f}}")
    if command != "sleep 120" or not written - EARLY <= started <= written + LATE:
        print(f"other pid={{pid}}")
        os.close(fd)
        continue
    if signal_fd(fd, signal.SIGTERM):
        held.append((pid, fd))
    else:
        print(f"gone pid={{pid}}")
        os.close(fd)

for sig, name in ((signal.SIGKILL, "terminated"), (None, "killed")):
    deadline = time.monotonic() + WAIT
    left = []
    for pid, fd in held:
        if exited(fd, deadline - time.monotonic()):
            print(f"{{name}} pid={{pid}}")
            os.close(fd)
        elif sig is not None and not signal_fd(fd, sig):
            print(f"{{name}} pid={{pid}}")
            os.close(fd)
        else:
            left.append((pid, fd))
    held = left
for pid, fd in held:
    print(f"survived pid={{pid}}")
    os.close(fd)
    uncertain = True
print("cleanup uncertain" if uncertain else "cleanup confirmed")
sys.exit(1 if uncertain else 0)
"""


def pidfd_cleanup(pid_files: Sequence[str]) -> str:
    """Clean up only the known children, through a pidfd for each identified child.

    Integration k208 replaced the k204 numeric-PID cleanup here. That cleanup
    checks the command once and then signals the number twice, so a reused PID
    can receive the signal. This cleanup opens a pidfd first. It accepts the
    process only when it runs exactly `sleep 120` and started within
    `CLEANUP_EARLY` before to `CLEANUP_LATE` after the mtime of its PID file. The
    payload writes that file just after it starts the child. A read is trusted
    only when the pidfd process is still alive after it, because a live process
    keeps its PID. TERM and then KILL go through the pidfd, so no later holder of
    the PID receives either signal.

    The output keeps the k204 vocabulary: `gone`, `other`, `terminated`,
    `killed`, `unknown` and `survived`. The last line is `cleanup confirmed`
    unless a PID ended as `unknown` or `survived`. Then it is `cleanup
    uncertain`, and the command exits 1. Disposal removes everything else.

    The residual limit is the identity. A process that runs `sleep 120` and
    starts inside the window under the recorded PID passes the check. A guest
    clock step between the write and the cleanup can turn the true child into
    `other`, which leaves it to disposal.
    """
    program = PIDFD_PROGRAM.format(early=CLEANUP_EARLY, late=CLEANUP_LATE, wait=CLEANUP_WAIT)
    program = program.replace("FILES = []", f"FILES = {list(pid_files)!r}", 1)
    return f"python3 -I - <<'K6_CLEANUP'\n{program}K6_CLEANUP"


LIVE_CLEANUP = pidfd_cleanup(CHILD_FILES)


def source_paths() -> list[Path]:
    """The experiment closure, the agent and the stubs. Hashed before and after the run."""
    roots = (Path(__file__).resolve(), SEAM, HERE / "linux_k6_check.py")
    return [
        *local_modules(roots),
        HERE / "linux_k6_run.sh",
        CHECKER,
        REPO / "Taskfile.yml",
        *sorted(AGENT_PACKAGE.glob("*.py")),
        *sorted(STUB.rglob("*.py")),
    ]


def container_resource(run_id: str, docker: str) -> Resource:
    name = f"k6-{run_id}"
    create = (
        docker,
        "create",
        "--init",
        "--network",
        "none",
        "-u",
        "1000:1000",
        "--name",
        name,
        "--label",
        f"{LABEL}={run_id}",
        "-v",
        f"{REPO / 'agents/linux'}:/agent:ro",
        "-v",
        f"{STUB}:/stub:ro",
        "-v",
        f"{HERE}:/seam:ro",
        IMAGE,
        "python3",
        "-E",
        "-s",
        "-B",
        "/seam/linux_k6_seam.py",
        run_tag(run_id),
        "cases",
    )
    return Resource(
        "container",
        name,
        Call("acquire", create, DOCKER_BOUND),
        Call("cleanup", (docker, "stop", "-t", "2", name), STOP_BOUND),
        Call("dispose", (docker, "rm", "-f", name), DISPOSE_BOUND),
        Call(
            "inspect",
            (docker, "container", "inspect", name),
            DOCKER_BOUND,
            expected_exit=1,
            expected_stdout=b"[]\n",
        ),
    )


def container_plan(python: str) -> Plan:
    run_id = uuid.uuid4().hex
    docker = docker_path()
    resource = container_resource(run_id, docker)
    calls = (Call("cases", (docker, "start", "-a", resource.identity), CASES_BUDGET),)
    subjects = (*source_paths(), Path(python))
    return Plan(run_id, calls, resources=(resource,), subjects=subjects)


def _health(cli: str, vm: str, name: str) -> Call:
    return Call(name, (cli, "agent", "health", "--vm", vm, "--json"), HEALTH)


def live_plan(tools: dict[str, str], downloads: Path) -> Plan:
    cli, python = tools["cli"], tools["python"]
    run_id = uuid.uuid4().hex
    vm = f"k6-linux-{run_id}"
    bursts = [
        Call(
            f"burst-{mode}",
            (python, "-E", "-s", str(BURST), cli, vm, mode, str(downloads)),
            BURST_BUDGET,
        )
        for mode in burst.MODES
    ]
    calls = [_health(cli, vm, "health-before"), *bursts, _health(cli, vm, "health-after")]
    start = (cli, "vm", "start", "--platform", "linux", "--base", GOLDEN, "--id", vm, "--json")
    owned = _owned(run_id, "linux", start, 0, tools)
    cleanup_argv = tuple(burst.exec_argv(cli, vm, CLEANUP_GUEST, LIVE_CLEANUP))
    resource = replace(owned, cleanup=Call("cleanup", cleanup_argv, CLEANUP_BOUND))
    subjects = (
        *source_paths(),
        BURST,
        Path(cli).resolve(),
        Path(tools["tart"]).resolve(),
        Path(python),
        APP,
        APP.parent.parent / "Info.plist",
        *installed_agent(),
    )
    return Plan(run_id, tuple(calls), resources=(resource,), subjects=subjects)


def installed_agent() -> list[Path]:
    """Every installed Linux agent module. k6 hashed only `server.py`."""
    return sorted((AGENTS / "linux/testanyware_agent").resolve().glob("*.py"))


def bounds() -> dict[str, object]:
    return {
        "boot": BOOT,
        "health": HEALTH,
        "docker": DOCKER_BOUND,
        "dispose": DISPOSE_BOUND,
        "stop": STOP_BOUND,
        "cases_budget": CASES_BUDGET,
        "burst_budget": BURST_BUDGET,
        "cleanup_guest": CLEANUP_GUEST,
        "cleanup_bound": CLEANUP_BOUND,
        "seam": seam.bounds(),
        "burst": burst.bounds(),
    }


def finish(output: Path, frozen: Plan) -> None:
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


def main() -> None:
    if len(sys.argv) != 3 or sys.argv[1] not in ("container", "live"):
        raise SystemExit("usage: linux_k6.py container|live OUT")
    mode, output = sys.argv[1], Path(sys.argv[2]).resolve()
    rule = "one plan, run once; no retry; failed samples stay"
    sources = {str(path.relative_to(REPO)): sha256(path) for path in source_paths()}
    if mode == "container":
        frozen = container_plan(sys.executable)
        output.mkdir(parents=True, exist_ok=False)
        inventory = {
            "leaf": LEAF,
            "mode": mode,
            "rule": rule,
            "image": image_record(),
            "cases": [asdict(case) for case in seam.CASES],
            "bounds": bounds(),
            "sources": sources,
            "plan": asdict(frozen),
        }
        write_json(output / "inventory.json", inventory)
        finish(output, frozen)
        return
    cli, tart = shutil.which("testanyware"), shutil.which("tart")
    if cli is None or tart is None:
        raise SystemExit("unavailable prerequisite: testanyware and tart must be on PATH")
    tools = {"python": sys.executable, "cli": cli, "tart": tart}
    output.mkdir(parents=True, exist_ok=False)
    downloads = output / "downloads"
    downloads.mkdir()
    frozen = live_plan(tools, downloads)
    inventory = {
        "leaf": LEAF,
        "mode": mode,
        "rule": rule,
        "golden": GOLDEN,
        "bounds": bounds(),
        "payloads": {m: burst.payload(m) for m in burst.MODES},
        "inspection": {m: burst.inspection(m) for m in burst.MODES},
        "cleanup": LIVE_CLEANUP,
        "sources": sources,
        "plan": asdict(frozen),
    }
    write_json(output / "inventory.json", inventory)
    finish(output, frozen)


if __name__ == "__main__":
    main()
