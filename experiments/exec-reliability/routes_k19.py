"""Baseline public guest route for one OS (leaf diagnostic-routes-k19).

    routes_k19.py OUT macos|linux

The script freezes every plan, bound and subject in OUT/inventory.json before the first live
call. Then it runs five plans in this order and never repeats one:

1. `host`: `vm start --dry-run --json`, `doctor --json` and the app signature.
2. `never-created`: dispose a VM id that no one created. The result must be
   `disposed ID already-absent`.
3. `failed-acquisition`: start from a golden name that does not exist. The start must exit 3,
   and its disposal must prove VM_NOT_FOUND and absence.
4. `route`: one owned clone of the retained golden. Agent health, then the guest agent hash.
   The `finally` path of the controller disposes the clone.
5. `after-route`: dispose the clone of plan 4 again. The result must be `already-absent`.

Plan 4 runs only when plan 1's dry run exited 0. That rule is frozen with the plans.
"""

import shutil
import sys
import uuid
from dataclasses import asdict
from pathlib import Path
from typing import cast

from controller import Call, Plan, Resource, execute_outcome, write_json

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
CHECKER = REPO / "scripts" / "vm_inventory.py"
APP = Path("/Applications/TestAnyware.app/Contents/MacOS/testanyware-app")
AGENTS = Path("/opt/homebrew/opt/testanyware/share/testanyware/agents")

BOOT = 420.0  # one tart start: 2.8.0 stage polls sum to 360 s, plus 60 s for the clone
HEALTH = 30.0  # ordinary observation bound
GUEST_TIMEOUT = 2  # guest timeout of the root brief
IDENTITY = GUEST_TIMEOUT + 20.0  # raw host exec: guest budget plus 20 s
LISTING = 5.0  # each inventory read inside a disposal: 30 + 2 x 5 + start-up < 41
STOP = 30.0
DISPOSE = 41.0  # 41 + TERM 2 + KILL 2 = the 45 s disposal ceiling
INSPECT = 55.0  # two inventory reads of 25 s each, inside the 60 s inventory bound
OBSERVE = 30.0
CLEANUP = 2.0  # /bin/echo only; the controller allows 30 s less TERM and KILL
DOCTOR = 60.0

GUEST_IDENTITY = {
    "macos": "shasum -a 256 /usr/local/bin/testanyware-agent; sw_vers -productVersion; uname -m",
    "linux": (
        "cd /opt/testanyware && find . -type f -name '*.py' | LC_ALL=C sort | xargs sha256sum; "
        ". /etc/os-release; echo $VERSION_ID; uname -m"
    ),
}


def _checker(python: str, cli: str, tart: str, verb: str, identity: str) -> tuple[str, ...]:
    if verb == "dispose":
        timing = ("--timeout", f"{LISTING:g}", "--stop-timeout", f"{STOP:g}")
    else:
        timing = ("--timeout", "25")
    return (python, "-I", str(CHECKER), verb, identity, "--cli", cli, "--tart", tart, *timing)


def _owned(
    run_id: str,
    os_name: str,
    acquire_argv: tuple[str, ...],
    expected_exit: int,
    tools: dict[str, str],
) -> Resource:
    identity = acquire_argv[acquire_argv.index("--id") + 1]
    assert run_id in identity
    python, cli, tart = tools["python"], tools["cli"], tools["tart"]
    return Resource(
        "vm",
        identity,
        Call("acquire", acquire_argv, BOOT, expected_exit=expected_exit),
        # No guest workload runs, so no guest cleanup exists. The call names the identity
        # because the controller requires that of every resource action.
        Call("cleanup", ("/bin/echo", "no guest workload ran in", identity), CLEANUP),
        Call("dispose", _checker(python, cli, tart, "dispose", identity), DISPOSE),
        Call(
            "inspect",
            _checker(python, cli, tart, "absent", identity),
            INSPECT,
            expected_stdout=f"absent {identity}\n".encode(),
        ),
    )


def plans(os_name: str, tools: dict[str, str]) -> dict[str, Plan]:
    python, cli = tools["python"], tools["cli"]
    host_id, never_id, failed_id, route_id = (uuid.uuid4().hex for _ in range(4))
    route_vm = f"k19-{os_name}-{route_id}"
    dry_vm = f"k19-{os_name}-dry-{host_id}"
    never_vm = f"k19-{os_name}-never-{never_id}"
    failed_vm = f"k19-{os_name}-failed-{failed_id}"
    missing_golden = f"k19-missing-golden-{failed_id}"
    start = (cli, "vm", "start", "--platform", os_name)
    subjects = (
        Path(__file__),
        HERE / "controller.py",
        CHECKER,
        REPO / "Taskfile.yml",
        Path(cli).resolve(),
        Path(tools["tart"]).resolve(),
        Path(python),
        APP,
        APP.parent.parent / "Info.plist",
        (AGENTS / "macos/testanyware-agent").resolve(),
        *sorted((AGENTS / "linux/testanyware_agent").resolve().glob("*.py")),
    )
    after_id = uuid.uuid4().hex
    return {
        "host": Plan(
            host_id,
            (
                Call("dry-run", (*start, "--id", dry_vm, "--dry-run", "--json"), OBSERVE),
                Call(
                    "app-signature", ("/usr/bin/codesign", "-dv", "--verbose=4", str(APP)), OBSERVE
                ),
                Call(
                    "missing-golden-absent",
                    _checker(python, cli, tools["tart"], "absent", missing_golden),
                    INSPECT,
                    expected_stdout=f"absent {missing_golden}\n".encode(),
                ),
                # Last: the controller stops a plan at its first failed call, and on
                # the Linux run (k19-route-linux-01) a failed doctor check hid the rest.
                Call("doctor", (cli, "doctor", "--json"), DOCTOR),
            ),
            subjects=subjects,
        ),
        "never-created": Plan(
            never_id,
            (
                Call(
                    "dispose-never-created",
                    _checker(python, cli, tools["tart"], "dispose", never_vm),
                    DISPOSE,
                    expected_stdout=f"disposed {never_vm} already-absent\n".encode(),
                ),
            ),
            subjects=subjects,
        ),
        "failed-acquisition": Plan(
            failed_id,
            (),
            resources=(
                _owned(
                    failed_id,
                    os_name,
                    (*start, "--base", missing_golden, "--id", failed_vm, "--json"),
                    3,
                    tools,
                ),
            ),
            subjects=subjects,
        ),
        "route": Plan(
            route_id,
            (
                Call("agent-health", (cli, "agent", "health", "--vm", route_vm, "--json"), HEALTH),
                Call(
                    "agent-identity",
                    (
                        cli,
                        "file",
                        "exec",
                        "--vm",
                        route_vm,
                        "--timeout",
                        str(GUEST_TIMEOUT),
                        "--json",
                        GUEST_IDENTITY[os_name],
                    ),
                    IDENTITY,
                ),
            ),
            resources=(_owned(route_id, os_name, (*start, "--id", route_vm, "--json"), 0, tools),),
            subjects=subjects,
        ),
        "after-route": Plan(
            after_id,
            (
                Call(
                    "dispose-after-route",
                    _checker(python, cli, tools["tart"], "dispose", route_vm),
                    DISPOSE,
                    expected_stdout=f"disposed {route_vm} already-absent\n".encode(),
                ),
            ),
            subjects=subjects,
        ),
    }


def main() -> None:
    output = Path(sys.argv[1]).resolve()
    os_name = sys.argv[2]
    if os_name not in GUEST_IDENTITY:
        raise SystemExit("the OS must be macos or linux")
    cli, tart = shutil.which("testanyware"), shutil.which("tart")
    if cli is None or tart is None:
        raise SystemExit("unavailable prerequisite: testanyware and tart must be on PATH")
    tools = {"python": sys.executable, "cli": cli, "tart": tart}
    output.mkdir(parents=True, exist_ok=False)
    frozen = plans(os_name, tools)
    write_json(
        output / "inventory.json",
        {
            "os": os_name,
            "order": list(frozen),
            "rule": "route runs only when host/dry-run exited 0; no plan repeats",
            "bounds": {
                "boot": BOOT,
                "health": HEALTH,
                "identity": IDENTITY,
                "listing": LISTING,
                "stop": STOP,
                "dispose": DISPOSE,
                "inspect": INSPECT,
                "observe": OBSERVE,
                "doctor": DOCTOR,
            },
            "plans": {name: asdict(plan) for name, plan in frozen.items()},
        },
    )
    exits: dict[str, dict[str, object]] = {}
    summary: dict[str, object] = {}
    for name, plan in frozen.items():
        if name == "route" and exits["host"].get("dry-run") != 0:
            summary[name] = "not run: the dry run did not exit 0"
            print(f"{name}: not run", flush=True)
            continue
        outcome = execute_outcome(plan, output)
        calls = cast(list[dict[str, object]], outcome.result.get("calls", []))
        exits[name] = {str(call.get("name")): call.get("returncode") for call in calls}
        summary[name] = {
            "verdict": outcome.verdict,
            "run": str(outcome.run_dir),
            "exits": exits[name],
            "ledger": outcome.ledger,
            "residuals": outcome.residuals,
        }
        print(f"{name}: verdict {outcome.verdict} run {outcome.run_dir}", flush=True)
    write_json(output / "summary.json", summary)


if __name__ == "__main__":
    main()
