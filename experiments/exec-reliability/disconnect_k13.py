"""Disconnect availability on one unchanged macOS clone (leaf macos-disconnect-availability-k13).

    disconnect_k13.py OUT

The script freezes one plan, then runs it once with no retry. The plan owns one
fresh clone of the unchanged golden image. Between health checks, it runs the
burst wrapper three times on the clone: `control`, `concurrent` and `term`.
`disconnect_k13_burst.py` holds what each mode does. The controller gives each
burst one budget, and it supervises the owned clone, its cleanup and its
disposal. A failed request inside a burst stays in the evidence as a failure.

The script writes OUT/inventory.json before any live call, then OUT/summary.json.
It prints `verdict VERDICT run RUN`. It exits with status 1 when the controller
verdict is not true.
"""

from __future__ import annotations

import shutil
import sys
import uuid
from dataclasses import asdict, replace
from pathlib import Path

import disconnect_k13_burst as burst
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
from timeout_k4 import CLEANUP_BOUND, CLEANUP_GUEST, GOLDEN, OWNED_CLEANUP

LEAF = "macos-disconnect-availability-k13"
BURST = HERE / "disconnect_k13_burst.py"
# The longest mode is `control`. It waits up to the original bound. Then it runs
# six bounded requests (four follow-ups and two later inspections) and one
# transfer. Each of the seven requests and the original can need a TERM wait and
# a KILL wait. The last 10 s cover interpreter start and the JSON output.
BURST_BUDGET = (
    burst.ORIGINAL_BOUND
    + burst.TRANSFER_BOUND
    + 6 * burst.FOLLOW_BOUND
    + 8 * 2 * burst.STOP_WAIT
    + 10.0
)
K4_FILES = "for f in /tmp/taw-exec-reliability/child.pid "
K13_FILES = "for f in " + " ".join(f"{burst.guest_dir(m)}/child.pid" for m in burst.MODES) + "; do"


def cleanup_script() -> str:
    """The k4 owned cleanup, with the k13 child PID files as its only targets."""
    lines = [str(line) for line in OWNED_CLEANUP.split("\n")]
    index = [i for i, line in enumerate(lines) if line.startswith(K4_FILES)]
    if len(index) != 1:
        raise SystemExit("the k4 cleanup no longer has one file loop")
    lines[index[0]] = K13_FILES
    return "\n".join(lines)


def _resource(run_id: str, start: tuple[str, ...], vm: str, tools: dict[str, str]) -> Resource:
    owned = _owned(run_id, "macos", start, 0, tools)
    argv = tuple(burst.exec_argv(tools["cli"], vm, CLEANUP_GUEST, cleanup_script()))
    return replace(owned, cleanup=Call("cleanup", argv, CLEANUP_BOUND))


def _health(cli: str, vm: str, name: str) -> Call:
    return Call(name, (cli, "agent", "health", "--vm", vm, "--json"), HEALTH)


def plan(tools: dict[str, str], downloads: Path) -> Plan:
    cli, python = tools["cli"], tools["python"]
    run_id = uuid.uuid4().hex
    vm = f"k13-macos-{run_id}"
    bursts = [
        Call(
            f"burst-{mode}",
            (python, "-E", "-s", str(BURST), cli, vm, mode, str(downloads)),
            BURST_BUDGET,
        )
        for mode in burst.MODES
    ]
    calls = [_health(cli, vm, "health-before"), *bursts, _health(cli, vm, "health-after")]
    subjects = (
        Path(__file__),
        BURST,
        HERE / "disconnect_k13_check.py",
        HERE / "disconnect_k13_run.sh",
        HERE / "routes_k19.py",
        HERE / "timeout_k4.py",
        HERE / "controller.py",
        CHECKER,
        REPO / "Taskfile.yml",
        Path(cli).resolve(),
        Path(tools["tart"]).resolve(),
        Path(python),
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


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: disconnect_k13.py OUT")
    output = Path(sys.argv[1]).resolve()
    cli, tart = shutil.which("testanyware"), shutil.which("tart")
    if cli is None or tart is None:
        raise SystemExit("unavailable prerequisite: testanyware and tart must be on PATH")
    tools = {"python": sys.executable, "cli": cli, "tart": tart}
    output.mkdir(parents=True, exist_ok=False)
    downloads = output / "downloads"
    downloads.mkdir()
    frozen = plan(tools, downloads)
    write_json(
        output / "inventory.json",
        {
            "leaf": LEAF,
            "golden": GOLDEN,
            "rule": "one plan, run once; no retry; failed samples stay",
            "bounds": {
                "boot": BOOT,
                "health": HEALTH,
                "burst_budget": BURST_BUDGET,
                "cleanup_guest": CLEANUP_GUEST,
                "cleanup_bound": CLEANUP_BOUND,
                **burst.bounds(),
            },
            "payloads": {mode: burst.payload(mode) for mode in burst.MODES},
            "inspection": {mode: burst.inspection(mode) for mode in burst.MODES},
            "cleanup": cleanup_script(),
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
