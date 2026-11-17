"""Judge the k6 Linux disconnect evidence.

    linux_k6_check.py container OUT
    linux_k6_check.py live OUT

`container` reads OUT/summary.json and the stdout of the `cases` call.
`live` reads OUT/summary.json and the receipts of the live calls. Each mode
writes OUT/check.json and prints one line per judged item.

The exit status is 0 when the evidence is complete and sound. A finding, such as
a blocked channel or a surviving child, is a measured outcome. A finding does
not change the exit status. An unknown outcome is incomplete evidence, so it is
a problem.

Responsiveness oracle, declared before any measured run. A follow-up request is
`responsive` when three conditions hold:

1. It ends with its expected outcome inside its bound.
2. It does not span the end of the original handler. A request spans the end
   when it began before the shell was last seen alive and answered after the
   shell was first seen gone.
3. Its latency is at most the latency of the same request in the control, plus
   `LATENCY_ALLOWANCE` seconds. The control is `serial-control` in the
   container and the `control` mode on the clone. Its follow-ups run after the
   handler ends, so they are the `baseline`.

A request with the expected outcome that fails condition 2 or 3 is `blocked`.
A blocked request is a defect even when it ends inside its bound. A request
that ends with another outcome, or exceeds its bound, is `failed`. A request
with no sound reference latency is `unknown`.

Handler outcomes, from the end of the payload shell after the original start:
`deadline` inside the window from `GUEST_TIMEOUT - DEADLINE_EARLY` to
`GUEST_TIMEOUT + DEADLINE_LATE`, `cancelled` before it, `overran` after it, and
`unknown` without evidence. The container watch places the end between two
samples 50 ms apart.

The live mode uses the k13 follow-up rules unchanged. Correction r2, made
after the failed check of `live-01` and before `live-02`, replaces two k13
rules that do not hold on Linux:

- The original reply. The Linux agent's deadline reply has no `timedOut`
  field, so the CLI prints `"timed_out": false`. The Linux shape is exit 0,
  `ok` true, `exit_code -1` and `stderr "Process timed out"`. The checker
  records `timed_out` as a finding.
- The handler bracket. On the live clone, `systemd --user` adopts the
  orphaned child. The serial agent answers the first inspection only after
  the handler ends, so that inspection records the adopter as the shell. A
  shell sighting is alive only when its command names the mode directory. A
  sighting is gone when the shell is absent, when another command holds the
  recorded shell pid, or when no shell pid is recorded and the child is
  present. For `control` and `concurrent`, the exit of the CLI also bounds the
  end.

No inspection can see the shell alive while the serial handler runs. When the
bracket has no lower end, the handler is classified by its upper end alone.
That basis is `queue-release`. The container cases measure it: there the first
queued request ends within `QUEUE_RELEASE` seconds of the shell end. Condition
2 cannot be evaluated without a sighting alive, so condition 3 decides.

Child outcomes: `survived-handler` when the child is alive after the handler
ends, `ended-with-handler` when it ended within `CHILD_SLACK` of the shell, and
`unknown` otherwise.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any, cast

import disconnect_k13_check as k13
import linux_k6_seam as seam
from disconnect_k13_burst import MODES, guest_dir
from disconnect_k13_check import (
    CHILD_SLACK,
    DEADLINE_EARLY,
    DEADLINE_LATE,
    LATENCY_ALLOWANCE,
    ORDINARY_STDOUT,
    items,
    mapping,
    seconds,
)

GUEST_TIMEOUT = seam.GUEST_TIMEOUT
QUEUE_RELEASE = 0.5  # the first queued answer ends this close to the shell end
CONTROL = "serial-control"
HEALTH_BODY = {"accessible": True, "platform": "linux"}
DEADLINE_BODY = {"exitCode": -1, "stdout": "", "stderr": "Process timed out"}
LISTEN = "0A"


# ---- container: parsing ----


def reply(record: dict[str, Any]) -> tuple[int | None, bytes]:
    """The status and the body of one raw HTTP response."""
    raw = bytes.fromhex(str(record.get("response_hex", "")))
    head, sep, body = raw.partition(b"\r\n\r\n")
    if not sep:
        return None, b""
    words = head.split(b"\r\n", 1)[0].split()
    try:
        return int(words[1]), body
    except (IndexError, ValueError):
        return None, body


def body_json(body: bytes) -> object:
    try:
        return json.loads(body)
    except ValueError:
        return None


def expected(record: dict[str, Any]) -> bool:
    """The follow-up ended with its expected outcome inside its bound."""
    if record.get("bound_exceeded") or record.get("error"):
        return False
    status, body = reply(record)
    if status != 200:
        return False
    name = str(record.get("name"))
    if name.startswith("health"):
        return body_json(body) == HEALTH_BODY
    if name == "ordinary":
        value = mapping(body_json(body))
        return value.get("exitCode") == 0 and value.get("stdout") == ORDINARY_STDOUT
    return body == seam.PARTIAL


def bracket(changes: list[dict[str, Any]], role: str) -> tuple[int, int] | None:
    """The interval in which `role` disappeared, after it was first seen."""
    seen = False
    for change in changes:
        present = any(mapping(row).get("role") == role for row in items(change.get("value")))
        if present:
            seen = True
        elif seen and change.get("prev_ns") is not None:
            return int(change["prev_ns"]), int(change["ns"])
    return None


def handler_class(lo: float | None, hi: float | None) -> str:
    if hi is None:
        return "unknown"
    if hi < GUEST_TIMEOUT - DEADLINE_EARLY:
        return "cancelled"
    if lo is not None and lo > GUEST_TIMEOUT + DEADLINE_LATE:
        return "overran"
    if hi > GUEST_TIMEOUT + DEADLINE_LATE + seam.POLL * 2:
        return "overran"
    return "deadline"


def container_handler(case: dict[str, Any]) -> dict[str, Any]:
    start = case.get("start_ns")
    shell = bracket(items(case.get("processes")), "shell")
    if shell is None:
        seen = any(
            mapping(row).get("role") == "shell"
            for change in items(case.get("processes"))
            for row in items(change.get("value"))
        )
        return {"class": "overran" if seen else "unknown", "alive_at": None, "gone_by": None}
    lo, hi = seconds(start, shell[0]), seconds(start, shell[1])
    return {"class": handler_class(lo, hi), "alive_at": lo, "gone_by": hi}


def container_child(case: dict[str, Any]) -> dict[str, Any]:
    after = [mapping(row) for row in items(mapping(case.get("after_handler")).get("rows"))]
    child = [row for row in after if row.get("role") == "child"]
    if child:
        return {"class": "survived-handler", "ppid": child[0].get("ppid")}
    shell = bracket(items(case.get("processes")), "shell")
    gone = bracket(items(case.get("processes")), "child")
    if shell is None or gone is None:
        return {"class": "unknown"}
    late = (gone[1] - shell[1]) / 1e9
    return {"class": "ended-with-handler" if late <= CHILD_SLACK else "unknown"}


def original_record(case: dict[str, Any]) -> dict[str, Any]:
    record = mapping(case.get("original"))
    start = case.get("start_ns")
    status, body = reply(record)
    outcome: dict[str, Any] = {
        "status": status,
        "body": body_json(body),
        "answered_at": seconds(start, record.get("end_ns")) if body else None,
        "error": record.get("error"),
    }
    act = mapping(case.get("act"))
    if act:
        outcome["acted_at"] = seconds(start, act.get("ns"))
    log = bytes.fromhex(str(case.get("agent_log_hex", ""))).decode(errors="replace")
    outcome["agent_broken_pipes"] = log.count("BrokenPipeError")
    return outcome


def socket_rows(change: dict[str, Any]) -> list[list[Any]]:
    """The /proc/net/tcp rows of one socket sample."""
    value = change.get("value")
    if not isinstance(value, list):
        return []
    return [cast(list[Any], row) for row in cast(list[Any], value) if isinstance(row, list)]


def server_rows(case: dict[str, Any], port: object) -> list[tuple[int, int | None, list[Any]]]:
    """Each socket sample as (ns, prev_ns, server rows whose remote end is `port`)."""
    agent = f":{int(case['port']):04X}"
    remote = f":{int(str(port)):04X}"
    samples: list[tuple[int, int | None, list[Any]]] = []
    for change in items(case.get("sockets")):
        rows = [
            row
            for row in socket_rows(change)
            if str(row[0]).endswith(agent) and str(row[1]).endswith(remote)
        ]
        samples.append((int(change["ns"]), change.get("prev_ns"), rows))
    return samples


def queue_record(case: dict[str, Any], record: dict[str, Any]) -> dict[str, Any]:
    """When the agent read the request of one follow-up, from its unread byte count.

    `waited_from` is the first sample with unread request bytes. `read_by` is the
    first later sample in which the row has no unread bytes or no longer exists.
    A request read within one sample interval has no `waited_from`.
    """
    start = case.get("start_ns")
    waited: int | None = None
    read: int | None = None
    for ns, _prev, rows in server_rows(case, record.get("local_port")):
        unread = any(int(row[4]) > 0 for row in rows)
        if waited is None and unread:
            waited = ns
        elif waited is not None and not unread:
            read = ns
            break
    return {"waited_from": seconds(start, waited), "read_by": seconds(start, read)}


def listen_peak(case: dict[str, Any]) -> int:
    agent = f":{int(case['port']):04X}"
    peak = 0
    for change in items(case.get("sockets")):
        for row in socket_rows(change):
            if str(row[0]).endswith(agent) and row[2] == LISTEN:
                peak = max(peak, int(row[4]))
    return peak


def original_states(case: dict[str, Any]) -> list[list[Any]]:
    """The distinct (state, unread bytes) of the agent end of the original connection."""
    seen: list[list[Any]] = []
    port = mapping(case.get("original")).get("local_port")
    if port is None:
        return seen
    for _ns, _prev, rows in server_rows(case, port):
        for row in rows:
            pair = [row[2], int(row[4])]
            if pair not in seen:
                seen.append(pair)
    return seen


# ---- container: verdicts ----


def follow_verdicts(
    case: dict[str, Any], handler: dict[str, Any], reference: dict[str, float] | None
) -> list[dict[str, Any]]:
    start = case.get("start_ns")
    lo, hi = handler["alive_at"], handler["gone_by"]
    verdicts: list[dict[str, Any]] = []
    for record in items(case.get("follow")):
        name = str(record.get("name"))
        status, _body = reply(record)
        began = seconds(start, record.get("start_ns"))
        answered = seconds(start, record.get("end_ns"))
        latency = seconds(record.get("start_ns"), record.get("end_ns"))
        spanned = (
            None not in (began, answered, lo, hi)
            and float(began) <= float(lo)  # type: ignore[arg-type]
            and float(answered) >= float(hi)  # type: ignore[arg-type]
        )
        verdict: dict[str, Any] = {
            "name": name,
            "status": status,
            "began_at": began,
            "answered_at": answered,
            "latency": latency,
            "bound": record.get("bound"),
            "bound_exceeded": bool(record.get("bound_exceeded")),
            "error": record.get("error"),
            "spans_handler_end": spanned,
            "queue": queue_record(case, record),
        }
        key = "health" if name == "health-deadline" else name
        if not expected(record):
            verdict["class"] = "failed"
        elif reference is None:
            verdict["class"] = "baseline"
        elif key not in reference or latency is None:
            verdict["class"] = "unknown"
        else:
            verdict["reference_latency"] = reference[key]
            fast = latency <= reference[key] + LATENCY_ALLOWANCE
            verdict["class"] = "blocked" if spanned or not fast else "responsive"
        verdicts.append(verdict)
    return verdicts


def queue_release(case: dict[str, Any], handler: dict[str, Any]) -> float | None:
    """Seconds from the shell end to the end of the first answered follow-up.

    Only a follow-up that began before the shell was last seen alive counts.
    Only the serial server queues, so the threading control has no release.
    """
    lo, hi = handler["alive_at"], handler["gone_by"]
    if case.get("server") != "serial" or lo is None or hi is None:
        return None
    start = case.get("start_ns")
    ends = [
        seconds(start, record.get("end_ns"))
        for record in items(case.get("follow"))
        if expected(record) and (seconds(start, record.get("start_ns")) or hi) <= lo
    ]
    known = [end for end in ends if end is not None]
    return round(min(known) - hi, 3) if known else None


def container_check(summary: dict[str, Any], run: Path) -> dict[str, Any]:
    problems: list[str] = []
    if summary.get("verdict") is not True:
        problems.append(f"controller verdict is {summary.get('verdict')!r}")
    if not k13.receipt_ok(run, "cases"):
        problems.append("the cases receipt is not ok")
    try:
        result = mapping(json.loads((run / "cases.stdout").read_bytes()))
    except (OSError, ValueError):
        return {"problems": [*problems, "the cases call printed no JSON"], "cases": []}
    cases = {str(case.get("case")): case for case in items(result.get("cases"))}
    if list(cases) != [case.name for case in seam.CASES]:
        problems.append(f"the cases are {list(cases)}")
    reference: dict[str, float] | None = None
    verdicts: list[dict[str, Any]] = []
    for name, case in cases.items():
        if mapping(case.get("ready")).get("ready") is not True:
            problems.append(f"{name}: the agent did not become ready")
            continue
        handler = container_handler(case)
        child = container_child(case)
        follow = follow_verdicts(case, handler, None if name == CONTROL else reference)
        cleanup = mapping(case.get("cleanup"))
        original = original_record(case)
        verdict: dict[str, Any] = {
            "case": name,
            "server": case.get("server"),
            "handler": handler,
            "child": child,
            "original": original,
            "original_states": original_states(case),
            "listen_peak": listen_peak(case),
            "queue_release": queue_release(case, handler),
            "follow": follow,
            "evidence": case.get("evidence"),
            "cleanup": {"kills": cleanup.get("kills"), "left": cleanup.get("left")},
            "agent_stop": case.get("agent_stop"),
        }
        if name == CONTROL:
            reference = {
                str(f["name"]): float(f["latency"])
                for f in follow
                if f["class"] == "baseline" and f["latency"] is not None
            }
            if original["body"] != DEADLINE_BODY:
                problems.append(f"{name}: the control has no deadline reply")
            if any(f["class"] != "baseline" for f in follow):
                problems.append(f"{name}: a control follow-up is not as expected")
        if handler["class"] == "unknown" or child["class"] == "unknown":
            problems.append(f"{name}: handler {handler['class']}, child {child['class']}")
        if name == CONTROL and handler["class"] != "deadline":
            problems.append(f"{name}: the control did not end at the deadline")
        if any(f["class"] == "unknown" for f in follow):
            problems.append(f"{name}: a follow-up outcome is unknown")
        if cleanup.get("left"):
            problems.append(f"{name}: owned processes remain after the cleanup")
        verdicts.append(verdict)
    sweep = mapping(result.get("final_sweep"))
    if items(sweep.get("rows")):
        problems.append("the final sweep found owned processes")
    return {"problems": problems, "bounds": result.get("bounds"), "cases": verdicts}


def container_main(out: Path) -> int:
    summary = json.loads((out / "summary.json").read_text())
    report = container_check(summary, Path(str(summary.get("run", ""))))
    (out / "check.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    for verdict in report["cases"]:
        follow = " ".join(
            f"{f['name']}={f['class']}({'?' if f['latency'] is None else f'{f["latency"]:.3f}'})"
            for f in verdict["follow"]
        )
        handler = verdict["handler"]
        gone = handler["gone_by"]
        print(
            f"{verdict['case']:<18} handler {handler['class']:<9} "
            f"{'?' if gone is None else f'{gone:.2f}':>6} child {verdict['child']['class']:<18} "
            f"{follow}"
        )
    for problem in report["problems"]:
        print(f"PROBLEM {problem}")
    return 0 if not report["problems"] else 1


# ---- live ----


LINUX_DEADLINE_STDERR = "Process timed out"


def payload_shell(result: dict[str, Any], shell: dict[str, str]) -> bool:
    return guest_dir(str(result.get("mode"))) in shell.get("command", "")


def linux_bracket(result: dict[str, Any]) -> tuple[float | None, float | None]:
    """The latest time the payload shell is known alive and the earliest time it is known gone."""
    start = result.get("start_ns")
    lo: float | None = None
    hi: float | None = None
    for record in k13.inspections(result):
        roles = k13.parse_inspection(record)
        if roles is None:
            continue
        shell, child = roles["shell"], roles["child"]
        began = seconds(start, record.get("start_ns"))
        ended = seconds(start, record.get("end_ns"))
        alive = shell["state"] == "present" and payload_shell(result, shell)
        gone = (
            shell["state"] == "absent"
            or (shell["state"] == "present" and not payload_shell(result, shell))
            or (shell["state"] == "none" and child["state"] == "present")
        )
        if alive and began is not None:
            lo = began if lo is None else max(lo, began)
        if gone and ended is not None:
            hi = ended if hi is None else min(hi, ended)
    if result.get("mode") != "term" and linux_original(result)["expected"]:
        reply = seconds(start, mapping(result.get("original")).get("exit_ns"))
        if reply is not None:
            hi = reply if hi is None else min(hi, reply)
    return lo, hi


def live_handler(result: dict[str, Any]) -> tuple[str, float | None, float | None, str]:
    """The handler outcome, by the Linux bracket, with its basis."""
    lo, hi = linux_bracket(result)
    if lo is None:
        return handler_class(None, hi), lo, hi, "queue-release"
    return handler_class(lo, hi), lo, hi, "bracket"


def linux_original(result: dict[str, Any]) -> dict[str, Any]:
    """The original request, judged by the Linux deadline reply outside `term`."""
    if result.get("mode") == "term":
        return k13.original_live(result)
    record = mapping(result.get("original"))
    body = k13.cli_body(record)
    expected = (
        record.get("bound_exceeded") is False
        and record.get("returncode") == 0
        and body.get("ok") is True
        and body.get("exit_code") == -1
        and body.get("stderr") == LINUX_DEADLINE_STDERR
    )
    return {
        "returncode": record.get("returncode"),
        "exit_code": body.get("exit_code"),
        "stderr": body.get("stderr"),
        "timed_out": body.get("timed_out"),
        "seconds": seconds(result.get("start_ns"), record.get("exit_ns")),
        "expected": expected,
    }


def adoption(result: dict[str, Any]) -> dict[str, Any] | None:
    """The parent of the child in the late inspection, and the command at that pid."""
    roles = k13.parse_inspection(mapping(result.get("late")))
    if roles is None:
        return None
    shell, child = roles["shell"], roles["child"]
    record: dict[str, Any] = {"child_ppid": child.get("ppid"), "child_command": child["command"]}
    if shell.get("pid") == child.get("ppid"):
        record["parent_command"] = shell["command"]
    return record


def live_check(summary: dict[str, Any], run: Path) -> dict[str, Any]:
    problems: list[str] = []
    if summary.get("verdict") is not True:
        problems.append(f"controller verdict is {summary.get('verdict')!r}")
    for name in ("health-before", "health-after"):
        if not k13.receipt_ok(run, name):
            problems.append(f"{name} did not succeed")
    results: dict[str, dict[str, Any]] = {}
    for mode in MODES:
        try:
            results[mode] = mapping(json.loads((run / f"burst-{mode}.stdout").read_bytes()))
        except (OSError, ValueError):
            problems.append(f"burst-{mode} printed no JSON")
            continue
        if not k13.receipt_ok(run, f"burst-{mode}"):
            problems.append(f"the burst-{mode} receipt is not ok")
    reference: dict[str, float] | None = None
    verdicts: list[dict[str, Any]] = []
    for mode in MODES:
        result = results.get(mode)
        if result is None:
            continue
        handler, lo, hi, basis = live_handler(result)
        child = k13.live_child(result)
        original = linux_original(result)
        follow = k13.live_follow(result, lo, hi, None if mode == "control" else reference)
        if mode == "control":
            reference = {
                str(f["name"]): float(f["latency"])
                for f in follow
                if f["class"] == "baseline" and f["latency"] is not None
            }
            if any(f["class"] != "baseline" for f in follow):
                problems.append("control: a control follow-up is not as expected")
        verdicts.append(
            {
                "mode": mode,
                "handler": handler,
                "handler_basis": basis,
                "handler_alive_at": lo,
                "handler_gone_by": hi,
                "child": child,
                "adoption": adoption(result),
                "original": original,
                "follow": follow,
                "mid": k13.parse_inspection(mapping(result.get("mid"))),
                "late": k13.parse_inspection(mapping(result.get("late"))),
            }
        )
        if handler == "unknown" or child == "unknown":
            problems.append(f"{mode}: handler {handler}, child {child}")
        if original["expected"] is not True:
            problems.append(f"{mode}: the original request has an unexpected outcome")
        if any(f["class"] == "unknown" for f in follow):
            problems.append(f"{mode}: a follow-up outcome is unknown")
        if mode != "term" and handler != "deadline":
            problems.append(f"{mode}: the control did not end at the deadline")
    return {"problems": problems, "modes": verdicts}


def live_main(out: Path) -> int:
    summary = json.loads((out / "summary.json").read_text())
    report = live_check(summary, Path(str(summary.get("run", ""))))
    (out / "check.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    for verdict in report["modes"]:
        follow = " ".join(f"{f['name']}={f['class']}" for f in verdict["follow"]) or "-"
        hi = verdict["handler_gone_by"]
        print(
            f"{verdict['mode']:<10} handler {verdict['handler']:<9} "
            f"({verdict['handler_basis']}, {'?' if hi is None else f'{hi:.2f}'}) "
            f"child {verdict['child']:<18} {follow}"
        )
    for problem in report["problems"]:
        print(f"PROBLEM {problem}")
    return 0 if not report["problems"] else 1


def main(argv: list[str]) -> int:
    if len(argv) != 3 or argv[1] not in ("container", "live"):
        print(__doc__, file=sys.stderr)
        return 64
    if argv[1] == "container":
        return container_main(Path(argv[2]))
    return live_main(Path(argv[2]))


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
