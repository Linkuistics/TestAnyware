"""Judge the k13 disconnect evidence.

    disconnect_k13_check.py host OUT
    disconnect_k13_check.py live OUT

`host` reads OUT/summary.json and the stdout of the `host` call in the run
directory. `live` reads OUT/summary.json and the receipts of the live calls.
Each mode writes OUT/check.json and prints one line per judged item.

The exit status is 0 when the evidence is complete and sound. A finding, such as
a blocked channel or a surviving child, is a measured outcome. A finding does
not change the exit status. An unknown outcome is incomplete evidence, so it is
a problem.

Responsiveness oracle, declared before any run. A follow-up request is
`responsive` when three conditions hold:

1. It ends with its expected outcome inside its bound.
2. Its answer comes before the original handler ends. This condition applies
   only to a request that began before the handler ended.

In the live mode, condition 2 fails only when the request provably spans the
handler end. It began no later than one sighting of the shell alive, and it
answered no earlier than one sighting of the shell gone.
3. Its latency is at most the latency of the same request in the concurrent
   control, plus `LATENCY_ALLOWANCE` seconds.

A request with the expected outcome that fails condition 2 or 3 is `blocked`.
A request that ends with another outcome, or exceeds its bound, is `failed`.
In the live mode, an exec follow-up with complete output but a deadline reply
is `completed-reported-timeout`. The k12 report names this receipt.
A request whose control or handler end is unknown is `unknown`.

In the live mode, the reference for condition 3 is a mode of the same clone.
`concurrent` compares with `control`, and `term` compares with `concurrent`.
When that mode has no sound latency for a request, `term` uses `control`. The
`control` follow-ups run after the handler ends, so they are the `baseline`.

Handler outcomes, from the end of the parent shell after the start:

- `deadline`: the shell ended inside the deadline window.
- `cancelled`: the shell ended before the window, so a disconnect ended it.
- `overran`: the shell ended after the window, or did not end in the bound.
- `unknown`: the evidence has no shell identity or end.

The host mode reads each end from a process watch. The live mode cannot watch
the guest, so it places the end between two inspections. One inspection saw the
shell alive, and a later one saw it gone. The live mode accepts `deadline` only
when the shell was seen alive at most 1 s before `MID_AT`.

Child outcomes, from the end of the child relative to the end of the shell:

- `ended-with-handler`: the child ended within `CHILD_SLACK` of the shell.
  In the live mode, the late inspection found the child gone.
- `survived-handler`: the child was still present after that.
  In the live mode, the late inspection found `sleep 120` at the child PID.
- `unknown`: the evidence has no child identity, or the inspection failed.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

from disconnect_k13_burst import MID_AT, MODES, PARTIAL, guest_dir
from disconnect_k13_seam import CASES, GUEST_TIMEOUT, KILL_WAIT, ORIGINAL_BOUND, TERM_GRACE

LATENCY_ALLOWANCE = 5.0  # the oracle allowance over the control latency
SLACK = 0.25  # the scheduling allowance on a 2 s signal bound
DEADLINE_EARLY = 0.5  # the deadline window starts this long before the guest timeout
DEADLINE_LATE = 3.0 + 1.0  # the terminate wait of the runner, plus scheduling
CHILD_SLACK = 1.0  # a child that ends within this of the shell ended with the handler
ORDINARY_STDOUT = "ordinary-ok"


def mapping(value: object) -> dict[str, Any]:
    return value if isinstance(value, dict) else {}  # type: ignore[return-value]


def items(value: object) -> list[dict[str, Any]]:
    if not isinstance(value, list):
        return []
    return [mapping(item) for item in value]  # type: ignore[misc]


def seconds(start_ns: object, end_ns: object) -> float | None:
    if not isinstance(start_ns, int) or not isinstance(end_ns, int):
        return None
    return (end_ns - start_ns) / 1e9


def json_body(data: bytes) -> dict[str, Any] | None:
    try:
        value = json.loads(data)
    except ValueError:
        return None
    return mapping(value) if isinstance(value, dict) else None  # pyright: ignore[reportUnknownArgumentType]


# ---- host mode ----


def http_reply(raw: bytes) -> tuple[int | None, dict[str, Any] | None]:
    head, sep, body = raw.partition(b"\r\n\r\n")
    if not sep:
        return None, None
    parts = head.split(b"\r\n", 1)[0].split()
    status = int(parts[1]) if len(parts) > 1 and parts[1].isdigit() else None
    return status, json_body(body)


def original_outcome(case: dict[str, Any]) -> dict[str, Any]:
    """What the original request returned, and whether that is its expected outcome."""
    record = mapping(case.get("original"))
    start = case.get("start_ns")
    if case.get("path") == "http":
        if case.get("action") in ("close", "reset"):
            return {"outcome": "not-read", "expected": True}
        raw = b"".join(bytes.fromhex(str(c.get("hex", ""))) for c in items(record.get("chunks")))
        status, body = http_reply(raw)
        first = items(record.get("chunks"))[0].get("ns") if record.get("chunks") else None
        body = body or {}
        expected = status == 200 and body.get("exitCode") == -1 and body.get("timedOut") is True
        return {
            "outcome": "response" if status is not None else "no-response",
            "status": status,
            "exit_code": body.get("exitCode"),
            "timed_out": body.get("timedOut"),
            "error": record.get("error"),
            "seconds": seconds(start, first),
            "expected": expected,
        }
    returncode = record.get("returncode")
    stdout = bytes.fromhex(str(record.get("stdout_hex", "")))
    end = seconds(start, record.get("exit_ns"))
    if case.get("action") == "term":
        signals = [str(s.get("signal")) for s in items(record.get("signals"))]
        after = seconds(case.get("act_ns"), record.get("exit_ns"))
        bound = TERM_GRACE + SLACK if signals == ["SIGTERM"] else TERM_GRACE + KILL_WAIT + SLACK
        expected = returncode in (-15, -9) and after is not None and after <= bound
        return {
            "outcome": "exit",
            "returncode": returncode,
            "signals": signals,
            "seconds": end,
            "exit_after_signal": after,
            "expected": expected,
        }
    body = json_body(stdout) or {}
    expected = (
        returncode == 0
        and body.get("ok") is True
        and body.get("exit_code") == -1
        and body.get("timed_out") is True
    )
    return {
        "outcome": "exit",
        "returncode": returncode,
        "exit_code": body.get("exit_code"),
        "timed_out": body.get("timed_out"),
        "seconds": end,
        "expected": expected,
    }


def follow_expected(record: dict[str, Any]) -> bool:
    """Whether one follow-up request ended with its expected outcome inside its bound."""
    if record.get("path") == "http":
        if record.get("transport_error") or record.get("status") != 200:
            return False
        body = json_body(bytes.fromhex(str(record.get("body_hex", "")))) or {}
        if record.get("kind") == "health":
            return body.get("platform") == "macos"
        return body.get("stdout") == ORDINARY_STDOUT and body.get("exitCode") == 0
    if record.get("bound_exceeded") is not False:
        return False
    body = json_body(bytes.fromhex(str(record.get("stdout_hex", "")))) or {}
    if record.get("kind") == "health":
        # The host denies accessibility, so a healthy agent gives exit status 4.
        return (
            record.get("returncode") == 4
            and body.get("ok") is True
            and body.get("reachable") is True
        )
    return (
        record.get("returncode") == 0
        and body.get("stdout") == ORDINARY_STDOUT
        and body.get("exit_code") == 0
    )


def host_completed_reported_timeout(record: dict[str, Any]) -> bool:
    """An ordinary follow-up with complete output, but a deadline reply (the k12 receipt)."""
    if record.get("kind") != "ordinary":
        return False
    if record.get("path") == "http":
        if record.get("transport_error") or record.get("status") != 200:
            return False
        body = json_body(bytes.fromhex(str(record.get("body_hex", "")))) or {}
        return (
            body.get("stdout") == ORDINARY_STDOUT
            and body.get("exitCode") == -1
            and body.get("timedOut") is True
        )
    if record.get("bound_exceeded") is not False or record.get("returncode") != 0:
        return False
    body = json_body(bytes.fromhex(str(record.get("stdout_hex", "")))) or {}
    return reported_timeout(body) and body.get("stdout") == ORDINARY_STDOUT


def identity(case: dict[str, Any], role: str) -> dict[str, Any] | None:
    for record in items(case.get("identities")):
        if record.get("role") == role:
            return record
    return None


def handler_outcome(case: dict[str, Any]) -> tuple[str, float | None]:
    shell = identity(case, "shell")
    if shell is None:
        return "unknown", None
    if shell.get("gone_reason") not in ("absent", None):
        return "unknown", None
    end = seconds(case.get("start_ns"), shell.get("gone_ns"))
    if end is None:
        return "overran", None
    if end < GUEST_TIMEOUT - DEADLINE_EARLY:
        return "cancelled", end
    if end <= GUEST_TIMEOUT + DEADLINE_LATE:
        return "deadline", end
    return "overran", end


def child_outcome(case: dict[str, Any]) -> tuple[str, float | None]:
    shell, child = identity(case, "shell"), identity(case, "child")
    if child is None or shell is None or child.get("gone_reason") not in ("absent", None):
        return "unknown", None
    child_end = seconds(case.get("start_ns"), child.get("gone_ns"))
    shell_end = seconds(case.get("start_ns"), shell.get("gone_ns"))
    if child_end is None:
        return "survived-handler", None
    if shell_end is not None and child_end <= shell_end + CHILD_SLACK:
        return "ended-with-handler", child_end
    return "survived-handler", child_end


def follow_verdicts(
    case: dict[str, Any], handler_end: float | None, control: dict[str, float]
) -> list[dict[str, Any]]:
    verdicts: list[dict[str, Any]] = []
    start = case.get("start_ns")
    for record in items(case.get("follow")):
        kind = str(record.get("kind"))
        latency = seconds(record.get("start_ns"), record.get("end_ns"))
        answered = seconds(start, record.get("end_ns"))
        verdict: dict[str, Any] = {
            "kind": kind,
            "latency": latency,
            "answered_at": answered,
            "control_latency": control.get(kind),
        }
        if host_completed_reported_timeout(record):
            verdict["class"] = "completed-reported-timeout"
        elif not follow_expected(record):
            verdict["class"] = "failed"
        elif kind not in control:
            # The control request of this kind did not give a sound latency.
            verdict["class"] = "unknown"
            verdict["unknown_reason"] = "no-sound-control"
        elif latency is None or answered is None or handler_end is None:
            verdict["class"] = "unknown"
        elif answered < handler_end and latency <= control[kind] + LATENCY_ALLOWANCE:
            verdict["class"] = "responsive"
        else:
            verdict["class"] = "blocked"
        verdicts.append(verdict)
    return verdicts


def host_check(summary: dict[str, Any], seam: dict[str, Any] | None) -> dict[str, Any]:
    problems: list[str] = []
    if summary.get("verdict") is not True:
        problems.append(f"controller verdict is {summary.get('verdict')!r}")
    if summary.get("leftovers"):
        problems.append(f"the runner found leftovers: {summary.get('leftovers')}")
    if summary.get("build_dir_removed") is not True:
        problems.append("the build directory remains")
    if seam is None:
        problems.append("the host call printed no JSON")
        return {"problems": problems, "cases": []}
    if seam.get("error"):
        problems.append(f"seam error: {seam.get('error')}")
    for server in items(seam.get("servers")):
        if server.get("ready") is not True:
            problems.append(f"{server.get('role')} did not become ready")
    for stop in items(seam.get("stops")):
        if stop.get("state") != "terminated":
            problems.append(f"{stop.get('role')} stop state is {stop.get('state')}")
    if seam.get("final_sweep"):
        problems.append(
            f"a token process remained after the servers stopped: {seam['final_sweep']}"
        )
    if seam.get("root_removed") is not True:
        problems.append("the temporary root remains")
    cases = {str(case.get("case")): case for case in items(seam.get("cases"))}
    missing = [case.name for case in CASES if case.name not in cases]
    if missing:
        problems.append(f"missing cases: {missing}")
    controls: dict[str, dict[str, float]] = {}
    for name in ("http-concurrent", "cli-concurrent"):
        for record in items(cases.get(name, {}).get("follow")):
            latency = seconds(record.get("start_ns"), record.get("end_ns"))
            if latency is not None and follow_expected(record):
                controls.setdefault(name.split("-")[0], {})[str(record.get("kind"))] = latency
    verdicts: list[dict[str, Any]] = []
    for name, case in cases.items():
        handler, handler_end = handler_outcome(case)
        child, child_end = child_outcome(case)
        original = original_outcome(case)
        control = controls.get(str(case.get("path")), {})
        follow = follow_verdicts(case, handler_end, control)
        verdict = {
            "case": name,
            "identity_error": case.get("identity_error"),
            "handler": handler,
            "handler_end": handler_end,
            "child": child,
            "child_end": child_end,
            "original": original,
            "follow": follow,
            "swept": case.get("sweep", []),
        }
        verdicts.append(verdict)
        if case.get("identity_error"):
            problems.append(f"{name}: no identity ({case['identity_error']})")
        if handler == "unknown" or child == "unknown":
            problems.append(f"{name}: handler {handler}, child {child}")
        if any(item["class"] == "unknown" for item in follow):
            problems.append(f"{name}: a follow-up outcome is unknown")
        # The controls must show the expected behaviour, or no comparison holds.
        if name in ("http-control", "http-concurrent", "cli-concurrent"):
            if handler != "deadline" or original["expected"] is not True:
                problems.append(f"{name}: the control did not end at the deadline as expected")
            # A completed reported timeout in a control is a finding. Each comparison
            # that needs its latency becomes unknown, and that is the problem.
            sound = ("responsive", "completed-reported-timeout")
            if any(item["class"] not in sound for item in follow):
                problems.append(f"{name}: a control follow-up is not responsive")
        elif original["expected"] is not True:
            problems.append(f"{name}: the original request has an unexpected outcome")
        if original.get("seconds") is not None and original["seconds"] > ORIGINAL_BOUND:
            problems.append(f"{name}: the original request exceeded {ORIGINAL_BOUND} s")
    return {"problems": problems, "controls": controls, "cases": verdicts}


def host_main(out: Path) -> int:
    summary = json.loads((out / "summary.json").read_text())
    run = Path(str(summary.get("run", "")))
    try:
        seam = json.loads((run / "host.stdout").read_bytes())
    except (OSError, ValueError):
        seam = None
    report = host_check(summary, seam)
    (out / "check.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    for verdict in report["cases"]:
        follow = " ".join(f"{f['kind']}={f['class']}" for f in verdict["follow"]) or "-"
        end = verdict["handler_end"]
        shown = f"{end:7.3f}s" if end is not None else "      ?"
        print(
            f"{verdict['case']:<16} handler {verdict['handler']:<9} {shown} "
            f"child {verdict['child']:<18} {follow}"
        )
    for problem in report["problems"]:
        print(f"PROBLEM {problem}")
    return 0 if not report["problems"] else 1


# ---- live mode ----

PARTIAL_HEX = PARTIAL.hex()
# Each mode compares its follow-up latencies with one reference mode.
REFERENCE = {"concurrent": "control", "term": "concurrent"}
FOLLOW_NAMES = ("inspect", "health", "ordinary", "download", "inspect-after")


def cli_body(record: dict[str, Any]) -> dict[str, Any]:
    return json_body(bytes.fromhex(str(record.get("stdout_hex", "")))) or {}


def reported_timeout(body: dict[str, Any]) -> bool:
    """The agent reported its deadline, with exit -1, for a command."""
    return body.get("timed_out") is True and body.get("exit_code") == -1


def parse_inspection(record: dict[str, Any]) -> dict[str, dict[str, str]] | None:
    """The role table of one inspection, or None when the inspection failed.

    A reported timeout with a complete role table still holds a sighting. The
    sighting happened somewhere inside the request window, and the bracket uses
    only the ends of that window.
    """
    body = cli_body(record)
    if record.get("bound_exceeded") is not False or record.get("returncode") != 0:
        return None
    if body.get("exit_code") != 0 and not reported_timeout(body):
        return None
    roles: dict[str, dict[str, str]] = {}
    for line in str(body.get("stdout", "")).splitlines():
        role, _, rest = line.partition(" ")
        head, _, command = rest.partition(" command=")
        fields = {"state": "none", "command": command}
        for word in head.split():
            key, sep, value = word.partition("=")
            if sep:
                fields[key] = value
            else:
                fields["state"] = word
        roles[role] = fields
    if set(roles) != {"shell", "child"}:
        return None
    shell, child = roles["shell"], roles["child"]
    parent = child.get("ppid", "")
    if child["state"] == "present" and parent not in ("", "1") and shell.get("pid") != parent:
        return None  # the recorded shell is not the parent of this child
    return roles


def completed_reported_timeout(record: dict[str, Any]) -> bool:
    """An exec follow-up whose output is complete, but whose reply is a deadline."""
    name = str(record.get("name"))
    body = cli_body(record)
    if record.get("bound_exceeded") is not False or not reported_timeout(body):
        return False
    if name.startswith("inspect"):
        return parse_inspection(record) is not None
    return name == "ordinary" and body.get("stdout") == ORDINARY_STDOUT


def live_follow_expected(record: dict[str, Any]) -> bool:
    name = str(record.get("name"))
    if record.get("bound_exceeded") is not False:
        return False
    if name.startswith("inspect"):
        return parse_inspection(record) is not None and not reported_timeout(cli_body(record))
    body = cli_body(record)
    if name == "health":
        return (
            record.get("returncode") == 0
            and body.get("ok") is True
            and body.get("reachable") is True
        )
    if name == "ordinary":
        return (
            record.get("returncode") == 0
            and body.get("stdout") == ORDINARY_STDOUT
            and body.get("exit_code") == 0
        )
    return record.get("returncode") == 0 and record.get("local_hex") == PARTIAL_HEX


def inspections(result: dict[str, Any]) -> list[dict[str, Any]]:
    named = [r for r in items(result.get("follow")) if str(r.get("name")).startswith("inspect")]
    return [*named, mapping(result.get("mid")), mapping(result.get("late"))]


def handler_bracket(result: dict[str, Any]) -> tuple[float | None, float | None]:
    """The latest time the shell is known alive and the earliest time it is known gone."""
    start = result.get("start_ns")
    lo: float | None = None
    hi: float | None = None
    for record in inspections(result):
        roles = parse_inspection(record)
        if roles is None:
            continue
        began = seconds(start, record.get("start_ns"))
        ended = seconds(start, record.get("end_ns"))
        shell, child = roles["shell"], roles["child"]
        # The agent runs `/bin/bash -c PAYLOAD`, and only the payload names the mode directory.
        alive = (
            shell["state"] == "present" and guest_dir(str(result.get("mode"))) in shell["command"]
        )
        orphan = child["state"] == "present" and child.get("ppid") == "1"
        gone = shell["state"] == "absent" or (shell["state"] == "none" and orphan)
        if alive and began is not None:
            lo = began if lo is None else max(lo, began)
        if gone and ended is not None:
            hi = ended if hi is None else min(hi, ended)
    original = mapping(result.get("original"))
    if result.get("mode") != "term" and original_live(result)["expected"]:
        # The CLI prints the reply after the handler ends, so its exit bounds the end.
        reply = seconds(start, original.get("exit_ns"))
        if reply is not None:
            hi = reply if hi is None else min(hi, reply)
    return lo, hi


def live_handler(result: dict[str, Any]) -> tuple[str, float | None, float | None]:
    lo, hi = handler_bracket(result)
    late = parse_inspection(mapping(result.get("late")))
    if late is not None and late["shell"]["state"] == "present":
        return "overran", lo, hi
    if hi is None:
        return "unknown", lo, hi
    if hi < GUEST_TIMEOUT - DEADLINE_EARLY:
        return "cancelled", lo, hi
    if result.get("mode") == "control":
        # No inspection runs before the reply in the control, so the reply alone places the end.
        return ("deadline" if hi <= GUEST_TIMEOUT + DEADLINE_LATE else "overran"), lo, hi
    if lo is None or lo < MID_AT - 1.0:
        return "unknown", lo, hi
    if lo > GUEST_TIMEOUT + DEADLINE_LATE:
        return "overran", lo, hi
    return "deadline", lo, hi


def live_child(result: dict[str, Any]) -> str:
    late = parse_inspection(mapping(result.get("late")))
    if late is None or late["child"]["state"] in ("none", "unknown"):
        return "unknown"
    if late["child"]["state"] == "present" and late["child"]["command"] == "sleep 120":
        return "survived-handler"
    if late["child"]["state"] == "absent":
        return "ended-with-handler"
    return "unknown"


def original_live(result: dict[str, Any]) -> dict[str, Any]:
    record = mapping(result.get("original"))
    start = result.get("start_ns")
    returncode = record.get("returncode")
    end = seconds(start, record.get("exit_ns"))
    if result.get("mode") == "term":
        signals = items(record.get("signals"))
        names = [str(s.get("signal")) for s in signals]
        first = signals[0].get("ns") if signals else None
        after = seconds(first, record.get("exit_ns"))
        bound = TERM_GRACE + SLACK if names == ["SIGTERM"] else TERM_GRACE + KILL_WAIT + SLACK
        expected = returncode in (-15, -9) and after is not None and after <= bound
        return {
            "returncode": returncode,
            "signals": names,
            "seconds": end,
            "exit_after_signal": after,
            "expected": expected,
        }
    body = cli_body(record)
    expected = (
        record.get("bound_exceeded") is False
        and returncode == 0
        and body.get("ok") is True
        and body.get("exit_code") == -1
        and body.get("timed_out") is True
    )
    return {
        "returncode": returncode,
        "exit_code": body.get("exit_code"),
        "timed_out": body.get("timed_out"),
        "seconds": end,
        "expected": expected,
    }


def live_follow(
    result: dict[str, Any], lo: float | None, hi: float | None, reference: dict[str, float] | None
) -> list[dict[str, Any]]:
    verdicts: list[dict[str, Any]] = []
    start = result.get("start_ns")
    for record in items(result.get("follow")):
        name = str(record.get("name"))
        latency = seconds(record.get("start_ns"), record.get("end_ns"))
        answered = seconds(start, record.get("end_ns"))
        verdict: dict[str, Any] = {
            "name": name,
            "returncode": record.get("returncode"),
            "bound_exceeded": record.get("bound_exceeded"),
            "latency": latency,
            "answered_at": answered,
        }
        if name == "download":
            verdict["content_matches"] = record.get("local_hex") == PARTIAL_HEX
        roles = parse_inspection(record) if name.startswith("inspect") else None
        if roles is not None:
            verdict["shell"] = roles["shell"]["state"]
            verdict["child"] = roles["child"]["state"]
        began = seconds(start, record.get("start_ns"))
        # Condition 2 fails only when the request provably spans the handler end:
        # it began no later than a sighting of the shell alive, and it answered no
        # earlier than a sighting of the shell gone. Condition 3 bounds any other hold.
        spanned = (
            began is not None
            and answered is not None
            and lo is not None
            and hi is not None
            and began <= lo
            and answered >= hi
        )
        verdict["spans_handler_end"] = spanned
        if completed_reported_timeout(record):
            verdict["class"] = "completed-reported-timeout"
        elif not live_follow_expected(record):
            verdict["class"] = "failed"
        elif reference is None:
            verdict["class"] = "baseline"
        elif name not in reference:
            verdict["class"] = "unknown"
            verdict["unknown_reason"] = "no-sound-control"
        elif latency is None:
            verdict["class"] = "unknown"
        else:
            verdict["reference_latency"] = reference[name]
            fast = latency <= reference[name] + LATENCY_ALLOWANCE
            verdict["class"] = "blocked" if spanned or not fast else "responsive"
        verdicts.append(verdict)
    return verdicts


def receipt_ok(run: Path, name: str) -> bool:
    try:
        return mapping(json.loads((run / f"{name}.json").read_bytes())).get("ok") is True
    except (OSError, ValueError):
        return False


def chained_reference(mode: str, references: dict[str, dict[str, float]]) -> dict[str, float]:
    """The sound latency of each request, from the nearest reference mode that has one.

    The chain is `term`, then `concurrent`, then `control`. This fallback was
    declared before the live run.
    """
    merged: dict[str, float] = {}
    current = mode
    while current in REFERENCE:
        current = REFERENCE[current]
        for name, latency in references.get(current, {}).items():
            merged.setdefault(name, latency)
    return merged


def live_check(summary: dict[str, Any], run: Path) -> dict[str, Any]:
    problems: list[str] = []
    if summary.get("verdict") is not True:
        problems.append(f"controller verdict is {summary.get('verdict')!r}")
    for name in ("health-before", "health-after"):
        if not receipt_ok(run, name):
            problems.append(f"{name} did not succeed")
    results: dict[str, dict[str, Any]] = {}
    for mode in MODES:
        try:
            results[mode] = mapping(json.loads((run / f"burst-{mode}.stdout").read_bytes()))
        except (OSError, ValueError):
            problems.append(f"burst-{mode} printed no JSON")
            continue
        if not receipt_ok(run, f"burst-{mode}"):
            problems.append(f"the burst-{mode} receipt is not ok")
    references: dict[str, dict[str, float]] = {}
    verdicts: list[dict[str, Any]] = []
    for mode in MODES:
        result = results.get(mode)
        if result is None:
            continue
        handler, lo, hi = live_handler(result)
        child = live_child(result)
        original = original_live(result)
        reference = chained_reference(mode, references) if mode in REFERENCE else None
        if mode in REFERENCE and not reference:
            problems.append(f"{mode}: the reference mode {REFERENCE[mode]} has no latencies")
        follow = live_follow(result, lo, hi, reference)
        references[mode] = {
            str(f["name"]): float(f["latency"])
            for f in follow
            if f["class"] in ("baseline", "responsive") and f["latency"] is not None
        }
        verdicts.append(
            {
                "mode": mode,
                "handler": handler,
                "handler_alive_at": lo,
                "handler_gone_by": hi,
                "child": child,
                "original": original,
                "follow": follow,
                "mid": parse_inspection(mapping(result.get("mid"))),
                "late": parse_inspection(mapping(result.get("late"))),
            }
        )
        if handler == "unknown" or child == "unknown":
            problems.append(f"{mode}: handler {handler}, child {child}")
        if original["expected"] is not True:
            problems.append(f"{mode}: the original request has an unexpected outcome")
        if any(f["class"] == "unknown" for f in follow):
            problems.append(f"{mode}: a follow-up outcome is unknown")
        if mode != "term":
            # The controls must show the expected behaviour, or no comparison holds.
            if handler != "deadline":
                problems.append(f"{mode}: the control did not end at the deadline")
            # A completed reported timeout is a finding, as in the host mode.
            sound = ("baseline", "responsive", "completed-reported-timeout")
            if any(f["class"] not in sound for f in follow):
                problems.append(f"{mode}: a control follow-up is not as expected")
    return {"problems": problems, "modes": verdicts}


def live_main(out: Path) -> int:
    summary = json.loads((out / "summary.json").read_text())
    report = live_check(summary, Path(str(summary.get("run", ""))))
    (out / "check.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    for verdict in report["modes"]:
        follow = " ".join(f"{f['name']}={f['class']}" for f in verdict["follow"]) or "-"
        lo, hi = verdict["handler_alive_at"], verdict["handler_gone_by"]
        bracket = f"({'?' if lo is None else f'{lo:.2f}'}, {'?' if hi is None else f'{hi:.2f}'}]"
        print(
            f"{verdict['mode']:<10} handler {verdict['handler']:<9} {bracket:<15} "
            f"child {verdict['child']:<18} {follow}"
        )
    for problem in report["problems"]:
        print(f"PROBLEM {problem}")
    return 0 if not report["problems"] else 1


def main(argv: list[str]) -> int:
    if len(argv) != 3 or argv[1] not in ("host", "live"):
        print(__doc__, file=sys.stderr)
        return 64
    if argv[1] == "host":
        return host_main(Path(argv[2]))
    return live_main(Path(argv[2]))


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
