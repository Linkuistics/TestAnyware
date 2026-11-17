"""One traced run of the k4 run 02 workload on one owned macOS clone (leaf k195).

Leaf `macos-exit-observation-instrumented-k195` asks why the macOS agent reports a
timeout after an intentional timeout. The traced agent is the checkout agent built
with `-Xswiftc -DEXIT_TRACE`. It writes `/tmp/testanyware-exit-trace.jsonl` in the
guest: each wait phase of `runShell`, and the kernel fork, exec and exit of each
shell and its children.

The plan has three parts:

1. Install. Health on the golden agent, then upload of the traced agent, then one
   exec that checks its digest, installs it at `/usr/local/bin/testanyware-agent`
   and asks launchd to restart the job 2 s later. A host pause of `RESTART_PAUSE`
   seconds follows. The verify exec then prints its parent PID, the installed
   digest and the first trace record. The checker accepts the route only when the
   trace was opened by that parent PID, which differs from the PID before.
2. Workload. The k4 run 02 sequence without its r3 part: the three issue #14
   payloads, then the ten after-timeout operations. `after-09-table` timed out in
   that run. The traced agent has a new cdhash, so the TCC grants of the golden
   agent no longer match it. Health therefore answers with exit 4 after the
   install. A wrapper lets each such health call accept exit 0 or 4, and exec
   needs no grant.
3. Evidence. A download of the trace, then the owned cleanup and the disposal of
   the k4 resource shape.

The CLI is the checkout build named by the first argument, never the one on PATH.
The app that answers the CLI is the development app that the runner checks.
"""

from __future__ import annotations

import hashlib
import sys
import uuid
from dataclasses import asdict
from pathlib import Path

from controller import Call, Plan, execute_outcome, write_json
from routes_k19 import CHECKER, HEALTH, HERE, REPO
from timeout_k4 import (
    GOLDEN,
    GUEST_TIMEOUT,
    HOST_MARGIN,
    OBSERVE,
    TRANSFER,
    _guest,  # pyright: ignore[reportPrivateUsage]
    _resource,  # pyright: ignore[reportPrivateUsage]
    after_calls,
    issue_calls,
)

LEAF = "macos-exit-observation-instrumented-k195"
AGENT_PATH = "/usr/local/bin/testanyware-agent"
STAGED = "/tmp/testanyware-agent-k195"
TRACE = "/tmp/testanyware-exit-trace.jsonl"
JOB = "com.linkuistics.testanyware.agent"
RESTART_PAUSE = 15  # launchd restart 2 s after the install exec, ThrottleInterval 5 s


def install_command(digest: str) -> str:
    """The guest command that installs the traced agent and schedules the restart.

    The restart runs in a detached shell, so the install exec returns first. The
    old agent is the parent of this shell, so `$PPID` names it.
    """
    restart = f"sleep 2; launchctl kickstart -k gui/$(id -u)/{JOB} || kill $old"
    return "\n".join(
        (
            "set -eu",
            "echo old=$PPID",
            f'test "$(shasum -a 256 {STAGED} | cut -d" " -f1)" = {digest}',
            f"rm -f {TRACE}",
            f"sudo -n install -m 0755 {STAGED} {AGENT_PATH}",
            f'test "$(shasum -a 256 {AGENT_PATH} | cut -d" " -f1)" = {digest}',
            "old=$PPID",
            "export old",
            f"nohup sh -c '{restart}' </dev/null >/dev/null 2>&1 &",
            "echo restart-scheduled",
        )
    )


VERIFY = "\n".join(
    (
        "echo agent=$PPID",
        f'echo sha=$(shasum -a 256 {AGENT_PATH} | cut -d" " -f1)',
        "echo started=$(ps -o lstart= -p $PPID)",
        f"echo first=$(head -n 1 {TRACE})",
    )
)


def install_calls(cli: str, vm: str, agent: Path, digest: str) -> list[Call]:
    upload = (cli, "file", "upload", "--vm", vm, str(agent), STAGED, "--json")
    return [
        Call("health-before", (cli, "agent", "health", "--vm", vm, "--json"), HEALTH),
        Call("upload-agent", upload, TRANSFER),
        _guest(cli, vm, "install-agent", OBSERVE, install_command(digest)),
        Call("restart-pause", ("/bin/sleep", str(RESTART_PAUSE)), RESTART_PAUSE + 5),
        _guest(cli, vm, "verify-agent", OBSERVE, VERIFY),
    ]


# A health call after the install runs inside this wrapper. The agent request is the
# same, but the call succeeds on exit 0 and on exit 4, so a kept grant cannot stop the
# workload. The wrapper prints the CLI exit as its last line.
HEALTH_WRAPPER = '"$0" "$@"; c=$?; echo "health-exit=$c"; test "$c" = 0 || test "$c" = 4'


def workload_calls(cli: str, vm: str) -> list[Call]:
    """The k4 run 02 workload. Health after the install accepts exit 0 or 4."""
    calls: list[Call] = [*issue_calls(cli, vm), *after_calls(cli, vm)]
    return [
        Call(c.name, ("/bin/sh", "-c", HEALTH_WRAPPER, *c.argv), c.budget)
        if c.argv[1] == "agent"
        else c
        for c in calls
    ]


def evidence_calls(cli: str, vm: str, downloads: Path) -> list[Call]:
    trace = (cli, "file", "download", "--vm", vm, TRACE, str(downloads / "trace.jsonl"), "--json")
    return [Call("download-trace", trace, TRANSFER)]


def plan(tools: dict[str, str], agent: Path, downloads: Path) -> Plan:
    cli = tools["cli"]
    digest = hashlib.sha256(agent.read_bytes()).hexdigest()
    run_id = uuid.uuid4().hex
    vm = f"k195-exit-macos-{run_id}"
    calls = [
        *install_calls(cli, vm, agent, digest),
        *workload_calls(cli, vm),
        *evidence_calls(cli, vm, downloads),
    ]
    subjects = (
        Path(__file__),
        HERE / "exit_k195_check.py",
        HERE / "exit_k195_run.sh",
        HERE / "timeout_k4.py",
        HERE / "timeout_k4_check.py",
        HERE / "routes_k19.py",
        HERE / "controller.py",
        CHECKER,
        REPO / "Taskfile.yml",
        Path(cli).resolve(),
        Path(tools["tart"]).resolve(),
        Path(tools["python"]),
        agent.resolve(),
    )
    start = (cli, "vm", "start", "--platform", "macos", "--base", GOLDEN, "--id", vm, "--json")
    return Plan(
        run_id,
        tuple(calls),
        resources=(_resource(run_id, start, vm, tools),),
        subjects=subjects,
    )


def main() -> None:
    if len(sys.argv) != 5:
        raise SystemExit("usage: exit_k195.py OUT CLI TART AGENT")
    output, cli, tart, agent = (Path(arg).resolve() for arg in sys.argv[1:])
    for path in (cli, tart, agent):
        if not path.is_file():
            raise SystemExit(f"unavailable prerequisite: {path}")
    tools = {"python": sys.executable, "cli": str(cli), "tart": str(tart)}
    output.mkdir(parents=True, exist_ok=False)
    downloads = output / "downloads"
    downloads.mkdir()
    frozen = plan(tools, agent, downloads)
    write_json(
        output / "inventory.json",
        {
            "leaf": LEAF,
            "golden": GOLDEN,
            "rule": "one plan, run once; no retry; failed samples stay",
            "agent_sha256": hashlib.sha256(agent.read_bytes()).hexdigest(),
            "bounds": {
                "health": HEALTH,
                "guest_timeout": GUEST_TIMEOUT,
                "observe": OBSERVE,
                "transfer": TRANSFER,
                "host_margin": HOST_MARGIN,
                "restart_pause": RESTART_PAUSE,
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
