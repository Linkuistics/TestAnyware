"""Judge the k5 Linux exec outcome and timeout lifecycle evidence.

    linux_k5_check.py OUT [--expect-defects]

The checker reads OUT/summary.json and the stdout of each call in the run
directory. It compares each observation with the faithful outcome of its case and
names each difference with one defect class. It writes OUT/check.json.

Without a flag, the exit status is 0 when the evidence is complete and sound:

- The controller verdict is true, and no container of the run remains.
- The reference seam shows the faithful outcome of every case, with no defect.
- The reference deadline phases stay within the grace and the kill bounds.
- No HTTP request or CLI call exceeded its bound or the agreed ceiling.
- Each agent container and the app started and became ready. The app stopped by
  TERM, and the disposal of the containers finished within 45 s.
- Every observation is known, and the bound observation came no earlier than
  the faithful bound.
- The cleanup removed every owned survivor within 30 s.
- Every temporary directory is gone.

The defects of the old path are findings, so they do not fail this mode.

With `--expect-defects`, the checker is the controlled old-path detector. The
exit status is 0 only when the sound evidence also shows every defect in
`REQUIRED`. A fixed agent makes this mode fail, which is the purpose.

Defect classes:

- `trailing-lf-trimmed`: a stream lost its trailing line feed.
- `non-utf8-dropped`: a stream with bytes that are not UTF-8 came back empty.
- `partial-output-dropped`: a stream with text came back empty.
- `timeout-text-in-stderr`: the agent wrote its timeout note into stderr.
- `stream-mismatch`: any other difference in a stream.
- `signal-reported-as-exit`: a signal death came back as an exit status. The
  status is the signal number, its negative, or the negative modulo 256.
- `synthetic-exit-code`: a deadline or a decode failure came back as exit status
  -1, or as 255 on the CLI. Neither is a process status.
- `completed-reported-timeout`: a command that ended by itself came back as a
  timeout.
- `deadline-not-flagged`: a deadline came back without the timeout flag.
- `status-mismatch`: any other difference in the status.
- `spawn-failure-unmapped`: a spawn failure came back without a spawn error that
  carries errno 11.
- `ambiguous-status`: in one seam, two cases with different faithful ends came
  back with the same end, code and flag.
- `term-skipped`: the traced handler sent KILL first to a case whose faithful
  end is TERM.
- `grace-exceeded`: the TERM grace was longer than 2 s.
- `kill-reap-exceeded`: the reap after KILL was longer than 2 s.
- `phase-marks-invalid`: a reference deadline record lacks a phase mark that its
  case needs, or its marks are out of order.
- `survivor-at-return`: a process that the case created was alive when the seam
  returned.
- `survivor-at-bound`: a process that the case created was alive at the faithful
  bound.
"""

from __future__ import annotations

import json
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any

from linux_k5_seam import (
    CLEANUP_BOUND,
    DISPOSAL_BOUND,
    HOST_SEAMS,
    INSIDE_SEAMS,
    SEAMS,
    cases,
)
from outcomes_k12_check import items, mapping, mark_defects, reference_phases
from outcomes_k12_seam import GRACE, HOST_MARGIN, KILL_WAIT, Case

SLACK = 0.25  # the scheduling allowance on a 2 s phase bound
SPAWN_ERRNO = 11  # EAGAIN on Linux, the errno of a spawn under RLIMIT_NPROC of 1
TIMED_OUT_TEXT = b"Process timed out"
HANDLER_FILE = "/agent/testanyware_agent/system_endpoints.py"
EXTRA = "ambiguous-status"

_JSON_SEAMS = ("process", "traced", "http", "cli-json")
REQUIRED: tuple[tuple[str, str, str], ...] = (
    *(
        (seam, case, defect)
        for seam in _JSON_SEAMS
        for case, defect in (
            ("binary", "non-utf8-dropped"),
            ("binary", "synthetic-exit-code"),
            ("exit-23-partial", "trailing-lf-trimmed"),
            ("signal-kill", "signal-reported-as-exit"),
            ("signal-hup", "ambiguous-status"),
            ("deadline", "synthetic-exit-code"),
            ("deadline", "deadline-not-flagged"),
            ("deadline", "timeout-text-in-stderr"),
            ("deadline", "partial-output-dropped"),
            ("deadline-term-ignored", "partial-output-dropped"),
            ("issue-sleep", "deadline-not-flagged"),
            ("issue-fork-wait", "survivor-at-bound"),
            ("spawn-failure", "spawn-failure-unmapped"),
        )
    ),
    ("cli-text", "binary", "non-utf8-dropped"),
    ("cli-text", "exit-23-partial", "trailing-lf-trimmed"),
    ("cli-text", "signal-kill", "signal-reported-as-exit"),
    ("cli-text", "exit-255", "ambiguous-status"),
    ("cli-text", "deadline", "synthetic-exit-code"),
    ("cli-text", "deadline", "deadline-not-flagged"),
    ("cli-text", "deadline", "timeout-text-in-stderr"),
    ("cli-text", "issue-fork-wait", "survivor-at-bound"),
    ("cli-text", "spawn-failure", "spawn-failure-unmapped"),
    ("traced", "deadline", "term-skipped"),
    ("traced", "issue-sleep", "term-skipped"),
)


def without_note(observed: bytes) -> bytes:
    """The stream without the timeout note and the line feed that ends the note."""
    index = observed.index(TIMED_OUT_TEXT)
    tail = observed[index + len(TIMED_OUT_TEXT) :]
    return observed[:index] + tail.removeprefix(b"\n")


def stream_defects(expected: bytes, observed: bytes, stream: str) -> list[str]:
    """Name each difference between an expected and an observed stream."""
    found: list[str] = []
    if stream == "stderr" and TIMED_OUT_TEXT in observed and TIMED_OUT_TEXT not in expected:
        found.append("timeout-text-in-stderr")
        observed = without_note(observed)
    if observed == expected:
        return found
    try:
        expected.decode("utf-8")
        is_text = True
    except UnicodeDecodeError:
        is_text = False
    if not is_text and observed == b"":
        found.append("non-utf8-dropped")
    elif expected.endswith(b"\n") and observed == expected.rstrip(b"\n"):
        found.append("trailing-lf-trimmed")
    elif expected and observed == b"":
        found.append("partial-output-dropped")
    else:
        found.append("stream-mismatch")
    return found


def signal_codes(signal: int) -> tuple[int, ...]:
    return (signal, -signal, -signal % 256)


def status_defects(case: Case, obs: dict[str, Any]) -> list[str]:
    """Name each difference in status. `obs` holds end, code and timed_out."""
    end, code, timed_out = obs.get("end"), obs.get("code"), obs.get("timed_out")
    if case.end == "spawn-failure":
        if end == "spawn-error" and code == SPAWN_ERRNO:
            return []
        return ["spawn-failure-unmapped"]
    found: list[str] = []
    if case.end == "deadline":
        if timed_out is not True:
            found.append("deadline-not-flagged")
        if end == "deadline-signal":
            return found if code == case.code else [*found, "status-mismatch"]
    elif timed_out is True:
        found.append("completed-reported-timeout")
    elif end == case.end and code == case.code:
        return found
    if case.end == "signal" and end == "exit" and code in signal_codes(int(case.code or 0)):
        found.append("signal-reported-as-exit")
    elif end == "exit" and code in (-1, 255):
        found.append("synthetic-exit-code")
    else:
        found.append("status-mismatch")
    return found


def utf8(text: object) -> bytes:
    return str(text).encode("utf-8") if text is not None else b""


def reply_obs(reply: dict[str, Any]) -> dict[str, Any]:
    return {
        "end": "exit",
        "code": reply.get("exitCode"),
        "timed_out": reply.get("timedOut"),
        "stdout": utf8(reply.get("stdout")),
        "stderr": utf8(reply.get("stderr")),
    }


def failed(end: str, code: object = None) -> dict[str, Any]:
    return {"end": end, "code": code, "stdout": b"", "stderr": b""}


def normalise(seam: str, record: dict[str, Any]) -> dict[str, Any]:
    """One observation per record: end, code, timed_out, stdout and stderr."""
    if seam == "reference":
        if record.get("outcome") == "spawn-error":
            return failed("spawn-error", record.get("errno"))
        status = mapping(record.get("status"))
        end = status.get("end")
        if record.get("timed_out") and end == "signal":
            end = "deadline-signal"
        return {
            "end": end,
            "code": status.get("code"),
            "timed_out": record.get("timed_out"),
            "stdout": bytes.fromhex(record.get("stdout_hex", "")),
            "stderr": bytes.fromhex(record.get("stderr_hex", "")),
        }
    if seam in INSIDE_SEAMS:
        if "helper_error" in record or record.get("http_status") != 200:
            return failed(f"http-{record.get('http_status')}")
        return reply_obs(mapping(record.get("body")))
    if seam == "http":
        if record.get("status") != 200:
            return failed(f"http-{record.get('status')}")
        return reply_obs(mapping(json.loads(bytes.fromhex(record.get("body_hex", "")))))
    stdout = bytes.fromhex(record.get("stdout_hex", ""))
    stderr = bytes.fromhex(record.get("stderr_hex", ""))
    if seam == "cli-text":
        # Text mode has no timeout flag. The checker never infers one from stderr.
        code = record.get("returncode")
        return {"end": "exit", "code": code, "timed_out": None, "stdout": stdout, "stderr": stderr}
    reply = mapping(json.loads(stdout))
    if reply.get("ok") is not True:
        return failed("cli-error", reply.get("code"))
    return {
        "end": "exit",
        "code": reply.get("exit_code"),
        "timed_out": reply.get("timed_out"),
        "stdout": utf8(reply.get("stdout")),
        "stderr": utf8(reply.get("stderr")),
    }


def owned_live(baseline: object, later: object) -> list[dict[str, Any]] | None:
    """The live processes of `later` that `baseline` did not list, or None when unknown."""
    first, second = mapping(baseline), mapping(later)
    if "rows" not in first or "rows" not in second:
        return None
    before = {(row.get("pid"), row.get("start_ticks")) for row in items(first["rows"])}
    return [
        row
        for row in items(second["rows"])
        if (row.get("pid"), row.get("start_ticks")) not in before and row.get("state") != "Z"
    ]


def trace_marks(rows: object) -> dict[str, Any]:
    """The first timeout, first signal and first reap of one handler trace."""
    marks: dict[str, Any] = {}
    for row in items(rows):
        event = row.get("event")
        if event == "communicate-timeout":
            marks.setdefault("timeout_ns", row.get("ns"))
        elif event == "signal":
            marks.setdefault("signal_ns", row.get("ns"))
            marks.setdefault("first_signal", row.get("signal"))
        elif event == "wait-return" and "signal_ns" in marks:
            marks.setdefault("reap_ns", row.get("ns"))
        elif event == "run-timeout":
            marks.setdefault("partial_stdout_hex", row.get("partial_stdout_hex"))
            marks.setdefault("partial_stderr_hex", row.get("partial_stderr_hex"))
    return marks


def trace_phases(marks: dict[str, Any]) -> dict[str, float]:
    phases: dict[str, float] = {}
    if "signal_ns" in marks and "reap_ns" in marks:
        phases["kill_reap"] = (marks["reap_ns"] - marks["signal_ns"]) / 1e9
    return phases


def phase_defects(phases: dict[str, float]) -> list[str]:
    found: list[str] = []
    if phases.get("grace", 0.0) > GRACE + SLACK:
        found.append("grace-exceeded")
    if phases.get("kill_reap", 0.0) > KILL_WAIT + SLACK:
        found.append("kill-reap-exceeded")
    return found


def observed_ns(observation: object) -> int | None:
    """The host clock of a host observation, otherwise the container clock."""
    data = mapping(observation)
    value = data.get("host_ns", data.get("ns"))
    return value if isinstance(value, int) else None


def judge_one(seam: str, case: Case, record: dict[str, Any]) -> dict[str, Any]:
    obs = normalise(seam, record)
    defects = status_defects(case, obs)
    if obs["end"] not in ("spawn-error", "cli-error") and not str(obs["end"]).startswith("http-"):
        defects += stream_defects(case.stdout, obs["stdout"], "stdout")
        defects += stream_defects(case.stderr, obs["stderr"], "stderr")
    marks: dict[str, Any] = {}
    if seam == "reference":
        invalid = mark_defects(case, record)
        defects += invalid
        phases = {} if invalid else reference_phases(record)
    elif seam == "traced":
        marks = trace_marks(record.get("trace"))
        phases = trace_phases(marks)
        if case.end == "deadline" and case.code == 15 and marks.get("first_signal") == 9:
            defects.append("term-skipped")
    else:
        phases = {}
    defects += phase_defects(phases)
    at_return = owned_live(record.get("baseline"), record.get("at_return"))
    at_bound = owned_live(record.get("baseline"), record.get("at_bound"))
    if at_return:
        defects.append("survivor-at-return")
    if at_bound:
        defects.append("survivor-at-bound")
    cleanup = mapping(record.get("cleanup"))
    after = owned_live(record.get("baseline"), cleanup.get("after"))
    bound_ns = observed_ns(record.get("at_bound"))
    return {
        "seam": seam,
        "case": case.name,
        "defects": sorted(set(defects)),
        "end": obs["end"],
        "code": obs["code"],
        "timed_out": obs.get("timed_out"),
        "stdout_hex": obs["stdout"].hex(),
        "stderr_hex": obs["stderr"].hex(),
        "seconds": (record["end_ns"] - record["start_ns"]) / 1e9,
        "ceiling_seconds": case.timeout + HOST_MARGIN,
        "phases": phases,
        "trace": marks,
        "bound_exceeded": bool(record.get("bound_exceeded") or record.get("transport_error")),
        "observations_known": at_return is not None and at_bound is not None,
        "bound_seconds": None if bound_ns is None else (bound_ns - record["start_ns"]) / 1e9,
        "survivors_at_bound": [
            {key: row.get(key) for key in ("pid", "ppid", "pgid", "argv")} for row in at_bound or []
        ],
        "child_pid_file": record.get("child_pid_file"),
        "cleanup_targets": len(items(cleanup.get("targets"))),
        "cleanup_seconds": cleanup.get("seconds"),
        "cleanup_complete": not cleanup.get("targets") or after == [],
    }


def mark_ambiguous(verdicts: list[dict[str, Any]]) -> None:
    """Mark the cases of one seam whose different faithful ends came back the same."""
    faithful = {case.name: (case.end, case.code) for case in cases(SEAMS[0])}
    groups: dict[tuple[str, str, str], list[dict[str, Any]]] = defaultdict(list)
    for verdict in verdicts:
        key = (verdict["seam"], repr((verdict["end"], verdict["code"])), repr(verdict["timed_out"]))
        groups[key].append(verdict)
    for group in groups.values():
        if len({faithful[verdict["case"]] for verdict in group}) > 1:
            for verdict in group:
                verdict["defects"] = sorted({*verdict["defects"], EXTRA})


def judge(seam: str, records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    by_name = {case.name: case for case in cases(seam)}
    return [judge_one(seam, by_name[record["case"]], record) for record in records]


def minimum_bound(verdict: dict[str, Any]) -> float:
    case = {case.name: case for case in cases(SEAMS[0])}[verdict["case"]]
    if case.end == "deadline":
        return case.timeout + GRACE + KILL_WAIT
    return 0.0


def host_problems(host: dict[str, Any]) -> list[str]:
    problems: list[str] = []
    if host.get("error"):
        problems.append(f"host: {host['error']}")
    for agent in items(host.get("agents")):
        if agent.get("ready") is not True:
            problems.append(f"agent {agent.get('name')} was not ready")
        if "limit" in agent and '"applied": true' not in str(mapping(agent["limit"]).get("stdout")):
            problems.append(f"agent {agent.get('name')} has no process limit")
    if mapping(host.get("app")).get("ready") is not True:
        problems.append("the app was not ready")
    for stop in items(host.get("stops")):
        if stop.get("state") != "terminated":
            problems.append(f"{stop.get('role')} stop state {stop.get('state')}")
    disposal = mapping(host.get("disposal"))
    if disposal.get("returncode") != 0 or float(disposal.get("seconds", 1e9)) > DISPOSAL_BOUND:
        problems.append(f"the disposal of the containers failed: {disposal.get('returncode')}")
    if str(mapping(host.get("containers_after")).get("stdout", "x")).strip():
        problems.append("a host container remains")
    if host.get("root_removed") is not True:
        problems.append("the host root remains")
    return problems


def soundness(
    summary: dict[str, Any], seams: dict[str, Any], verdicts: list[dict[str, Any]]
) -> list[str]:
    problems: list[str] = []
    if summary.get("verdict") is not True:
        problems.append("the controller verdict is not true")
    leftovers = mapping(summary.get("leftovers"))
    if leftovers.get("found") or leftovers.get("after"):
        problems.append("a container of the run remained after the plan")
    for name, data in seams.items():
        if data is None:
            problems.append(f"call {name} gave no evidence")
    for name in INSIDE_SEAMS:
        data = mapping(seams.get(name))
        if data and data.get("work_removed") is not True:
            problems.append(f"the {name} work directory remains")
        if data and data.get("uid") != 1000:
            problems.append(f"the {name} seam ran as uid {data.get('uid')}")
        if data and name != "reference" and data.get("handler_file") != HANDLER_FILE:
            problems.append(f"the {name} seam loaded {data.get('handler_file')}")
    host = mapping(seams.get("host"))
    if host:
        problems += host_problems(host)
    expected = {(seam, case.name) for seam in SEAMS for case in cases(seam)}
    seen = {(v["seam"], v["case"]) for v in verdicts}
    if seen != expected:
        problems.append(f"missing cases: {sorted(expected - seen)}")
    for verdict in verdicts:
        where = f"{verdict['seam']}/{verdict['case']}"
        if verdict["bound_exceeded"]:
            problems.append(f"{where} exceeded its bound")
        if verdict["seconds"] > verdict["ceiling_seconds"]:
            problems.append(f"{where} exceeded the agreed {verdict['ceiling_seconds']} s ceiling")
        if not verdict["observations_known"]:
            problems.append(f"{where} has an unknown observation")
        bound = verdict["bound_seconds"]
        if bound is None or bound < minimum_bound(verdict):
            problems.append(f"{where} was observed before its faithful bound")
        if not verdict["cleanup_complete"]:
            problems.append(f"{where} cleanup left an owned process")
        if float(verdict["cleanup_seconds"] or 0.0) > CLEANUP_BOUND:
            problems.append(f"{where} cleanup took longer than {CLEANUP_BOUND} s")
        if verdict["seam"] == "reference" and verdict["defects"]:
            problems.append(f"{where} is not faithful: {verdict['defects']}")
    return problems


def missing_required(verdicts: list[dict[str, Any]]) -> list[str]:
    index = {(v["seam"], v["case"]): set(v["defects"]) for v in verdicts}
    return [
        f"{seam}/{case}: {defect}"
        for seam, case, defect in REQUIRED
        if defect not in index.get((seam, case), set())
    ]


def load(out: Path) -> tuple[dict[str, Any], dict[str, Any]]:
    summary = json.loads((out / "summary.json").read_text())
    run = Path(str(summary.get("run", "")))
    seams: dict[str, Any] = {}
    for name in (*INSIDE_SEAMS, "host"):
        try:
            seams[name] = json.loads((run / f"{name}.stdout").read_bytes())
        except (OSError, ValueError):
            seams[name] = None
    return summary, seams


def verdicts_of(seams: dict[str, Any]) -> list[dict[str, Any]]:
    verdicts: list[dict[str, Any]] = []
    for name in INSIDE_SEAMS:
        data = mapping(seams.get(name))
        verdicts += judge(name, items(data.get(name)))
    host = mapping(seams.get("host"))
    for name in HOST_SEAMS:
        verdicts += judge(name, items(host.get(name)))
    mark_ambiguous(verdicts)
    return verdicts


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3) or (len(argv) == 3 and argv[2] != "--expect-defects"):
        print(__doc__, file=sys.stderr)
        return 64
    out = Path(argv[1])
    summary, seams = load(out)
    verdicts = verdicts_of(seams)
    problems = soundness(summary, seams, verdicts)
    missing = missing_required(verdicts)
    report = {"problems": problems, "missing_required": missing, "verdicts": verdicts}
    (out / "check.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    for verdict in verdicts:
        found = ",".join(verdict["defects"]) or "faithful"
        print(f"{verdict['seam']:>9} {verdict['case']:<22} {verdict['seconds']:7.3f}s {found}")
    for problem in problems:
        print(f"PROBLEM {problem}")
    if len(argv) == 3:
        for item in missing:
            print(f"MISSING {item}")
        return 0 if not problems and not missing else 1
    return 0 if not problems else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
