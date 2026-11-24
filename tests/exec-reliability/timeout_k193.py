"""Run the complete original r3 fixture condition alone on one fresh owned macOS clone.

Leaf `macos-timeout-inspection-k193`. The plan is the r3 sequence of the frozen k4 plan
(`timeout_k4.r3_calls`) and nothing else: no issue payloads, no `after-*` calls and no
extra health call run before it. The guest script, the lesson and every argv and guest
timeout come from the original clone-1 records of `macos-03`, through the k4 fixture copy.
Only the VM identity and the local download paths differ.

The plan runs once. A failed clone stays as a failed sample and is never replaced. The
owned cleanup and the disposal are the k4 ones, so the clone and its `sleep 120` children
are disposed even when a call fails.
"""

import shutil
import sys
import uuid
from dataclasses import asdict
from pathlib import Path

from controller import Plan, execute_outcome, write_json
from routes_k19 import AGENTS, APP, BOOT, CHECKER, HEALTH, HERE, REPO
from timeout_k4 import (
    CANCEL_AFTER,
    CANCEL_GUEST,
    CLEANUP_BOUND,
    CLEANUP_GUEST,
    FIXTURE,
    FIXTURE_SHA256,
    GOLDEN,
    GUEST_TIMEOUT,
    HOST_MARGIN,
    OBSERVE,
    ONLINE_GUEST,
    TRANSFER,
    _resource,  # pyright: ignore[reportPrivateUsage]
    fixture_mismatches,
    r3_calls,
)

LEAF = "macos-timeout-inspection-k193"


def plan(tools: dict[str, str], downloads: Path) -> Plan:
    """Freeze the r3-only plan on one owned clone of the golden image."""
    cli = tools["cli"]
    run_id = uuid.uuid4().hex
    vm = f"k193-r3-macos-{run_id}"
    calls = r3_calls(cli, vm, FIXTURE, downloads)
    subjects = (
        Path(__file__),
        HERE / "timeout_k4.py",
        HERE / "timeout_k4_check.py",
        HERE / "timeout_k193_check.py",
        HERE / "timeout_k193_run.sh",
        HERE / "routes_k19.py",
        HERE / "controller.py",
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


def main() -> None:
    output = Path(sys.argv[1]).resolve()
    cli, tart = shutil.which("testanyware"), shutil.which("tart")
    if cli is None or tart is None:
        raise SystemExit("unavailable prerequisite: testanyware and tart must be on PATH")
    mismatches = fixture_mismatches(FIXTURE)
    if mismatches:
        raise SystemExit(f"fixture copy differs from the original: {mismatches}")
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
            "condition": "complete original r3 fixture sequence alone, on one fresh clone",
            "rule": "one plan, run once; no retry; failed samples stay",
            "fixture_sha256": FIXTURE_SHA256,
            "bounds": {
                "boot": BOOT,
                "health": HEALTH,
                "guest_timeout": GUEST_TIMEOUT,
                "observe": OBSERVE,
                "online_guest": ONLINE_GUEST,
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
