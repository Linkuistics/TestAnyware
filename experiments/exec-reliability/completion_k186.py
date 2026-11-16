"""Ordinary macOS completion on one fresh unchanged clone (leaf macos-completion-live-k186).

    completion_k186.py OUT

The script freezes the plan, its bounds and its subject hashes in OUT/inventory.json
before the first live call. Then it runs one plan once. The plan owns one clone of the
retained golden `testanyware-golden-macos-27`. The `finally` path of the controller
disposes the clone. The calls run in this order:

1. `health-before`: agent health.
2. `serial-01` to `serial-10`: the exact issue #14 request
   `file exec --vm VM --timeout 30 --json "printf ordinary-ok"`, host bound 50 s.
3. `identity`: guest agent hash, guest OS version and architecture.
4. `concurrent-1` to `concurrent-3`: four exact requests at once, from
   concurrent_k186.py. Each request has its own host bound of 50 s.
5. `pool`: the guest CPU count, the constrained-thread limit and the agent threads.
6. `health-after`: agent health.

A guest timeout gives host exit 0, so the controller continues after it. The controller
stops the plan only at a host failure. The later calls then stay unreached and recorded.
The guest receipts are judged afterwards by completion_k186_check.py.
"""

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

GOLDEN = "testanyware-golden-macos-27"
SERIAL_COUNT = 10
GUEST_TIMEOUT = 30
HOST_BOUND = GUEST_TIMEOUT + 20.0  # raw host exec: guest budget plus 20 s
CONCURRENT_ROUNDS = 3
CONCURRENT_COUNT = 4
# Each copy: 50 s, then TERM 2 s and KILL 2 s. After the last reap the wrapper reads the
# pipes for at most 1 s until EOF. Start-up takes 2 s more. The controller adds its own
# TERM 2 s and KILL 2 s after this budget.
CONCURRENT_BUDGET = HOST_BOUND + 2.0 + 2.0 + 1.0 + 2.0
PROBE_GUEST_TIMEOUT = 5
PROBE_BOUND = PROBE_GUEST_TIMEOUT + 20.0
ORDINARY = "printf ordinary-ok"
IDENTITY = "shasum -a 256 /usr/local/bin/testanyware-agent; sw_vers -productVersion; uname -m"
POOL = (
    "sysctl hw.ncpu hw.activecpu kern.wq_max_threads kern.wq_max_constrained_threads; "
    "pgrep -x testanyware-agent; ps -M -p $(pgrep -x testanyware-agent)"
)
WRAPPER = HERE / "concurrent_k186.py"


def _exec(cli: str, vm: str, guest_timeout: int, command: str) -> tuple[str, ...]:
    return (cli, "file", "exec", "--vm", vm, "--timeout", str(guest_timeout), "--json", command)


def _resource(run_id: str, start: tuple[str, ...], vm: str, tools: dict[str, str]) -> Resource:
    # The workload leaves no guest process behind: each request exits or the agent kills it.
    # Disposal removes the guest itself, so the cleanup action records only that fact.
    owned = _owned(run_id, "macos", start, 0, tools)
    note = ("/bin/echo", "no guest cleanup beyond disposal for", vm)
    return replace(owned, cleanup=replace(owned.cleanup, argv=note))


def plan(tools: dict[str, str]) -> Plan:
    python, cli = tools["python"], tools["cli"]
    run_id = uuid.uuid4().hex
    vm = f"k186-macos-{run_id}"
    calls = [Call("health-before", (cli, "agent", "health", "--vm", vm, "--json"), HEALTH)]
    calls += [
        Call(f"serial-{i:02d}", _exec(cli, vm, GUEST_TIMEOUT, ORDINARY), HOST_BOUND)
        for i in range(1, SERIAL_COUNT + 1)
    ]
    calls.append(Call("identity", _exec(cli, vm, PROBE_GUEST_TIMEOUT, IDENTITY), PROBE_BOUND))
    calls += [
        Call(
            f"concurrent-{i}",
            (python, "-I", str(WRAPPER), str(CONCURRENT_COUNT), f"{HOST_BOUND:g}", cli, vm),
            CONCURRENT_BUDGET,
        )
        for i in range(1, CONCURRENT_ROUNDS + 1)
    ]
    calls.append(Call("pool", _exec(cli, vm, PROBE_GUEST_TIMEOUT, POOL), PROBE_BOUND))
    calls.append(Call("health-after", (cli, "agent", "health", "--vm", vm, "--json"), HEALTH))
    subjects = (
        Path(__file__),
        HERE / "routes_k19.py",
        HERE / "controller.py",
        WRAPPER,
        HERE / "completion_k186_check.py",
        HERE / "completion_k186_run.sh",
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
    import shutil

    output = Path(sys.argv[1]).resolve()
    cli, tart = shutil.which("testanyware"), shutil.which("tart")
    if cli is None or tart is None:
        raise SystemExit("unavailable prerequisite: testanyware and tart must be on PATH")
    tools = {"python": sys.executable, "cli": cli, "tart": tart}
    output.mkdir(parents=True, exist_ok=False)
    frozen = plan(tools)
    write_json(
        output / "inventory.json",
        {
            "leaf": "macos-completion-live-k186",
            "golden": GOLDEN,
            "rule": "one plan, run once; no retry; failed samples stay",
            "bounds": {
                "boot": BOOT,
                "health": HEALTH,
                "guest_timeout": GUEST_TIMEOUT,
                "host_bound": HOST_BOUND,
                "concurrent_rounds": CONCURRENT_ROUNDS,
                "concurrent_count": CONCURRENT_COUNT,
                "concurrent_budget": CONCURRENT_BUDGET,
                "probe_bound": PROBE_BOUND,
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
