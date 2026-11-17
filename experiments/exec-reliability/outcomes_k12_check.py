"""Judge the k12 exec outcome evidence.

    outcomes_k12_check.py OUT [--expect-defects]

The checker reads OUT/summary.json and the stdout of each seam call in the run
directory. It compares each observation with the faithful outcome of its case and
names each difference with one defect class. It writes OUT/check.json.

Without a flag, the exit status is 0 when the evidence is complete and sound:

- The controller verdict is true and the runner found no leftover.
- The reference seam shows the faithful outcome of every case, with no defect.
- The reference deadline phases stay within the grace and the kill bounds.
- No probe, HTTP request or CLI call exceeded its bound.
- Each agent and the app started, became ready and stopped by TERM.
- Every temporary directory is gone, and no case left a residual process.

The defects of the old path are findings, so they do not fail this mode.

With `--expect-defects`, the checker is the controlled old-path detector. The
exit status is 0 only when the sound evidence also shows every defect in
`REQUIRED`. A fixed runner makes this mode fail, which is the purpose.

Defect classes:

- `trailing-lf-trimmed`: a stream lost its trailing line feed.
- `non-utf8-dropped`: a stream with bytes that are not UTF-8 came back empty.
- `timeout-text-in-stderr`: the runner wrote its timeout note into stderr.
- `stream-mismatch`: any other difference in a stream.
- `signal-reported-as-exit`: a signal death came back as an exit status.
- `synthetic-exit-code`: a deadline came back as exit status -1, or as 255 on
  the CLI, which is not a process status.
- `completed-reported-timeout`: a command that ended by itself came back as a
  timeout.
- `deadline-not-flagged`: a deadline came back without the timeout flag.
- `status-mismatch`: any other difference in the status.
- `spawn-failure-unmapped`: a spawn failure came back as a generic HTTP 500 or
  CLI error, without the errno.
- `grace-exceeded`: the TERM grace was longer than 2 s.
- `kill-reap-exceeded`: the reap after KILL was longer than 2 s.
- `phase-marks-invalid`: a reference deadline record lacks a phase mark that
  its case needs, or its marks are out of order. TERM needs the deadline, TERM
  and reap marks. A case that ignores TERM also needs the grace-expired and KILL
  marks. Without this class, absent marks would read as zero-length phases.
- `residual-process`: a case left a process that carries its sleep token.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any, cast

from outcomes_k12_seam import GRACE, HOST_MARGIN, KILL_WAIT, TIMED_OUT_TEXT, Case, cases

SLACK = 0.25  # the scheduling allowance on a 2 s phase bound
SPAWN_ERRNO = 35  # EAGAIN, the errno of a spawn under RLIMIT_NPROC of 1

# (seam, case, alternatives): at least one alternative must be present.
# A command that ends by itself sometimes comes back as a timeout (the k3 race).
# That defect then replaces the deterministic one, so it is an alternative.
RACE = "completed-reported-timeout"
REQUIRED: tuple[tuple[str, str, frozenset[str]], ...] = (
    *(
        (seam, case, frozenset(classes))
        for seam in ("process", "traced", "http", "cli-json")
        for case, classes in (
            ("binary", {"non-utf8-dropped"}),
            ("exit-23-partial", {"trailing-lf-trimmed", RACE}),
            ("signal-kill", {"signal-reported-as-exit", RACE}),
            ("deadline", {"synthetic-exit-code"}),
            ("deadline", {"timeout-text-in-stderr"}),
            ("deadline", {"trailing-lf-trimmed"}),
        )
    ),
    ("cli-text", "binary", frozenset({"non-utf8-dropped"})),
    # In text mode the race shows only as exit status 255 and the stderr note.
    ("cli-text", "signal-kill", frozenset({"signal-reported-as-exit", "synthetic-exit-code"})),
    ("cli-text", "deadline", frozenset({"synthetic-exit-code"})),
    ("cli-text", "deadline", frozenset({"deadline-not-flagged"})),
    ("cli-text", "deadline", frozenset({"timeout-text-in-stderr"})),
    ("traced", "deadline-term-ignored", frozenset({"grace-exceeded"})),
    ("http", "spawn-failure", frozenset({"spawn-failure-unmapped"})),
    ("cli-text", "spawn-failure", frozenset({"spawn-failure-unmapped"})),
    ("cli-json", "spawn-failure", frozenset({"spawn-failure-unmapped"})),
)


def mapping(value: object) -> dict[str, Any]:
    """The value as a JSON object, or an empty one when it is not an object."""
    return cast(dict[str, Any], value) if isinstance(value, dict) else {}


def items(value: object) -> list[dict[str, Any]]:
    """The JSON objects in a list value."""
    if not isinstance(value, list):
        return []
    return [mapping(entry) for entry in cast(list[object], value)]


def stream_defects(expected: bytes, observed: bytes, stream: str) -> list[str]:
    """Name each difference between an expected and an observed stream."""
    if observed == expected:
        return []
    found: list[str] = []
    if stream == "stderr" and TIMED_OUT_TEXT in observed:
        found.append("timeout-text-in-stderr")
        head = observed[: observed.index(TIMED_OUT_TEXT)]
        observed = head.removesuffix(b"[").removesuffix(b"\n") if head else b""
        if observed == expected:
            return found
        if expected.endswith(b"\n") and observed + b"\n" == expected:
            return found  # the note replaced the line feed; that is not a second defect
    try:
        expected.decode("utf-8")
        is_text = True
    except UnicodeDecodeError:
        is_text = False
    if not is_text and observed == b"":
        found.append("non-utf8-dropped")
    elif expected.endswith(b"\n") and observed == expected.rstrip(b"\n"):
        found.append("trailing-lf-trimmed")
    else:
        found.append("stream-mismatch")
    return found


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
            # Only the reference reports this end. Its signal must be the one the
            # case needs: KILL when the shell ignores TERM, TERM otherwise.
            if code != case.code:
                found.append("status-mismatch")
            return found
    elif timed_out is True:
        found.append(RACE)
    elif end == case.end and code == case.code:
        return found
    if end == "exit" and code in (-1, 255):
        found.append("synthetic-exit-code")
    elif case.end == "signal" and end == "exit" and code == case.code:
        found.append("signal-reported-as-exit")
    else:
        found.append("status-mismatch")
    return found


def utf8(text: object) -> bytes:
    return str(text).encode("utf-8") if text is not None else b""


def normalise(seam: str, record: dict[str, Any]) -> dict[str, Any]:
    """One observation per record: end, code, timed_out, stdout and stderr."""
    if seam == "reference":
        if record.get("outcome") == "spawn-error":
            return {"end": "spawn-error", "code": record.get("errno"), "stdout": b"", "stderr": b""}
        status = mapping(record.get("status"))
        end, code = status.get("end"), status.get("code")
        if record.get("timed_out") and end == "signal":
            end = "deadline-signal"
        return {
            "end": end,
            "code": code,
            "timed_out": record.get("timed_out"),
            "stdout": bytes.fromhex(record.get("stdout_hex", "")),
            "stderr": bytes.fromhex(record.get("stderr_hex", "")),
        }
    if seam in ("process", "traced"):
        result = mapping(record.get("result"))
        if result.get("kind") == "thrown":
            spawn = result.get("domain") == "NSPOSIXErrorDomain"
            return {
                "end": "spawn-error" if spawn else "thrown",
                "code": result.get("code"),
                "stdout": b"",
                "stderr": b"",
            }
        return {
            "end": "exit",
            "code": result.get("exitCode"),
            "timed_out": result.get("timedOut"),
            "stdout": bytes.fromhex(result.get("stdout_hex", "")),
            "stderr": bytes.fromhex(result.get("stderr_hex", "")),
        }
    if seam == "http":
        body = bytes.fromhex(record.get("body_hex", ""))
        if record.get("status") != 200:
            return {
                "end": f"http-{record.get('status')}",
                "code": None,
                "stdout": b"",
                "stderr": b"",
            }
        reply = json.loads(body)
        return {
            "end": "exit",
            "code": reply.get("exitCode"),
            "timed_out": reply.get("timedOut"),
            "stdout": utf8(reply.get("stdout")),
            "stderr": utf8(reply.get("stderr")),
        }
    stdout = bytes.fromhex(record.get("stdout_hex", ""))
    stderr = bytes.fromhex(record.get("stderr_hex", ""))
    if seam == "cli-text":
        # Text mode has no timeout flag. The checker never infers one from stderr.
        code = record.get("returncode")
        if b"error: HTTP" in stderr:
            return {"end": "cli-error", "code": code, "stdout": stdout, "stderr": b""}
        return {"end": "exit", "code": code, "timed_out": None, "stdout": stdout, "stderr": stderr}
    reply = json.loads(stdout)
    if reply.get("ok") is not True:
        return {"end": "cli-error", "code": reply.get("code"), "stdout": b"", "stderr": b""}
    return {
        "end": "exit",
        "code": reply.get("exit_code"),
        "timed_out": reply.get("timed_out"),
        "stdout": utf8(reply.get("stdout")),
        "stderr": utf8(reply.get("stderr")),
    }


def trace_phases(rows: list[Any]) -> dict[str, float]:
    """The grace and the kill-to-reap seconds from one exit trace."""
    marks: dict[str, int] = {}
    for row in items(rows):
        event, phase = row.get("event"), row.get("phase")
        key = f"{event}:{phase}" if phase else str(event)
        marks.setdefault(key, int(row.get("t_ns", 0)))
    phases: dict[str, float] = {}
    if "terminate-sent" in marks and "wait-result:terminate" in marks:
        phases["grace"] = (marks["wait-result:terminate"] - marks["terminate-sent"]) / 1e9
    if "kill-sent" in marks and "reap-return" in marks:
        phases["kill_reap"] = (marks["reap-return"] - marks["kill-sent"]) / 1e9
    return phases


def reference_phases(record: dict[str, Any]) -> dict[str, float]:
    phases: dict[str, float] = {}
    if "term_ns" in record:
        stop = record.get("grace_expired_ns", record.get("reap_ns", 0))
        phases["grace"] = (stop - record["term_ns"]) / 1e9
    if "kill_ns" in record:
        phases["kill_reap"] = (record["reap_ns"] - record["kill_ns"]) / 1e9
    return phases


def mark_defects(case: Case, record: dict[str, Any]) -> list[str]:
    """Check that a reference deadline record has the ordered marks its case needs."""
    if case.end != "deadline":
        return []
    names = ["deadline_ns", "term_ns"]
    if case.code == 9:
        names += ["grace_expired_ns", "kill_ns"]
    names.append("reap_ns")
    marks: list[int] = []
    for name in names:
        mark = record.get(name)
        if not isinstance(mark, int):
            return ["phase-marks-invalid"]
        marks.append(mark)
    if any(later < earlier for earlier, later in zip(marks, marks[1:], strict=False)):
        return ["phase-marks-invalid"]
    return []


def phase_defects(phases: dict[str, float]) -> list[str]:
    found: list[str] = []
    if phases.get("grace", 0.0) > GRACE + SLACK:
        found.append("grace-exceeded")
    if phases.get("kill_reap", 0.0) > KILL_WAIT + SLACK:
        found.append("kill-reap-exceeded")
    return found


def judge(seam: str, records: list[dict[str, Any]]) -> list[dict[str, Any]]:
    by_name = {case.name: case for case in cases(seam)}
    verdicts: list[dict[str, Any]] = []
    for record in records:
        case = by_name[record["case"]]
        obs = normalise(seam, record)
        defects = status_defects(case, obs)
        if obs["end"] not in ("spawn-error", "cli-error") and not obs["end"].startswith("http-"):
            defects += stream_defects(case.stdout, obs["stdout"], "stdout")
            defects += stream_defects(case.stderr, obs["stderr"], "stderr")
        if seam == "reference":
            invalid = mark_defects(case, record)
            defects += invalid
            phases = {} if invalid else reference_phases(record)
        elif seam == "traced":
            phases = trace_phases(record.get("trace", []))
        else:
            phases = {}
        defects += phase_defects(phases)
        if record.get("residuals"):
            defects.append("residual-process")
        verdicts.append(
            {
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
                "bound_exceeded": bool(record.get("bound_exceeded")) or "transport_error" in record,
            }
        )
    return verdicts


def soundness(
    summary: dict[str, Any], seams: dict[str, Any], verdicts: list[dict[str, Any]]
) -> list[str]:
    problems: list[str] = []
    if summary.get("verdict") is not True:
        problems.append("the controller verdict is not true")
    if summary.get("leftovers"):
        problems.append("the runner found leftover processes")
    if summary.get("build_dir_removed") is not True:
        problems.append("the build directory remains")
    for name, data in seams.items():
        if data is None:
            problems.append(f"seam {name} gave no evidence")
    http_cli = mapping(seams.get("http-cli"))
    for server in items(http_cli.get("servers")):
        if server.get("ready") is not True:
            problems.append(f"{server.get('role')} was not ready")
    for stop in items(http_cli.get("stops")):
        if stop.get("state") != "terminated":
            problems.append(f"{stop.get('role')} stop state {stop.get('state')}")
    if http_cli and http_cli.get("root_removed") is not True:
        problems.append("the http-cli root remains")
    for name in ("reference", "process", "traced"):
        data = mapping(seams.get(name))
        if data and data.get("work_removed") is not True:
            problems.append(f"the {name} work directory remains")
    expected = {
        (seam, case.name)
        for seam in ("reference", "process", "traced", "http", "cli-text", "cli-json")
        for case in cases(seam)
    }
    seen = {(v["seam"], v["case"]) for v in verdicts}
    if seen != expected:
        problems.append(f"missing cases: {sorted(expected - seen)}")
    for verdict in verdicts:
        where = f"{verdict['seam']}/{verdict['case']}"
        if verdict["bound_exceeded"]:
            problems.append(f"{where} exceeded its bound")
        # The agreed ceiling is the guest budget plus 20 s; for an ordinary case
        # that is 30 s. The checker measures it here and does not trust the flag.
        if verdict["seconds"] > verdict["ceiling_seconds"]:
            problems.append(f"{where} exceeded the agreed {verdict['ceiling_seconds']} s ceiling")
        if "residual-process" in verdict["defects"]:
            problems.append(f"{where} left a residual process")
        if verdict["seam"] == "reference" and verdict["defects"]:
            problems.append(f"{where} is not faithful: {verdict['defects']}")
    return problems


def missing_required(verdicts: list[dict[str, Any]]) -> list[str]:
    index = {(v["seam"], v["case"]): set(v["defects"]) for v in verdicts}
    missing: list[str] = []
    for seam, case, alternatives in REQUIRED:
        if not index.get((seam, case), set()) & alternatives:
            missing.append(f"{seam}/{case}: {sorted(alternatives)}")
    return missing


def load(out: Path) -> tuple[dict[str, Any], dict[str, Any]]:
    summary = json.loads((out / "summary.json").read_text())
    run = Path(str(summary.get("run", "")))
    seams: dict[str, Any] = {}
    for name in ("reference", "process", "traced", "http-cli"):
        path = run / f"{name}.stdout"
        try:
            seams[name] = json.loads(path.read_bytes())
        except (OSError, ValueError):
            seams[name] = None
    return summary, seams


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3) or (len(argv) == 3 and argv[2] != "--expect-defects"):
        print(__doc__, file=sys.stderr)
        return 64
    out = Path(argv[1])
    summary, seams = load(out)
    verdicts: list[dict[str, Any]] = []
    for name in ("reference", "process", "traced"):
        data = seams.get(name)
        if data:
            verdicts += judge(name, data[name])
    http_cli = seams.get("http-cli")
    if http_cli:
        for name in ("http", "cli-text", "cli-json"):
            verdicts += judge(name, http_cli.get(name, []))
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
