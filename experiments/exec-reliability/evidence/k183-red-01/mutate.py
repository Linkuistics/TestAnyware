"""Fault injection for the k183 repairs (review k182, F1 and F2).

Each fault rewrites one subject, runs the named controls and restores the subject. A fault
counts as detected only when pytest exits nonzero. Run from the repository root:
  uv run --project experiments/exec-reliability --locked python -I EVIDENCE/mutate.py EVIDENCE
"""

import hashlib
import json
import subprocess
import sys
from pathlib import Path

REPO = Path.cwd()
OUT = Path(sys.argv[1]).resolve()
CHECKER = REPO / "scripts/vm_inventory.py"
TASKFILE = REPO / "Taskfile.yml"
DISPOSAL = "disposal"
SAME_NAME = "same_literal"

FAULTS = {
    "f2-any-ok-envelope": (
        CHECKER,
        '        and envelope.get("stopped") is True\n'
        '        and envelope.get("id") == identity\n'
        '        and "dry_run" not in envelope\n',
        "",
        DISPOSAL,
    ),
    "f2-ignore-id": (CHECKER, '        and envelope.get("id") == identity\n', "", DISPOSAL),
    "f2-ignore-dry-run": (CHECKER, '        and "dry_run" not in envelope\n', "", DISPOSAL),
    "f2-truthy-stopped": (
        CHECKER,
        'envelope.get("stopped") is True',
        'envelope.get("stopped")',
        DISPOSAL,
    ),
    "f1-linux-create-unquoted": (
        TASKFILE,
        '--version {{.VERSION | default "24.04"}} --name {{.NAME | shellQuote}}',
        '--version {{.VERSION | default "24.04"}} --name {{.NAME}}',
        SAME_NAME,
    ),
    "f1-macos-create-unquoted": (
        TASKFILE,
        '--version {{.VERSION | default "27"}} --name {{.NAME | shellQuote}}',
        '--version {{.VERSION | default "27"}} --name {{.NAME}}',
        SAME_NAME,
    ),
    "f1-guard-unquoted": (
        TASKFILE,
        "absent {{.NAME | shellQuote}}",
        "absent {{.NAME}}",
        SAME_NAME,
    ),
}


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    before = {str(p.relative_to(REPO)): digest(p) for p in (CHECKER, TASKFILE)}
    summary: dict[str, object] = {}
    for name, (subject, old, new, selector) in FAULTS.items():
        original = subject.read_text()
        assert original.count(old) == 1, name
        subject.write_text(original.replace(old, new))
        try:
            done = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "pytest",
                    "-p",
                    "no:cacheprovider",
                    "-q",
                    "test_vm_inventory.py",
                    "-k",
                    selector,
                ],
                cwd=REPO / "experiments/exec-reliability",
                capture_output=True,
                text=True,
                timeout=300,
            )
        finally:
            subject.write_text(original)
        (OUT / f"{name}.txt").write_text(done.stdout + done.stderr)
        tail = (done.stdout.strip().splitlines() or [""])[-1]
        summary[name] = {"exit": done.returncode, "tail": tail, "detected": done.returncode != 0}
    after = {str(p.relative_to(REPO)): digest(p) for p in (CHECKER, TASKFILE)}
    summary["subjects-restored"] = before == after
    summary["subjects-sha256"] = after
    (OUT / "summary.json").write_text(json.dumps(summary, indent=1) + "\n")
    ok = before == after and all(
        v["detected"] for k, v in summary.items() if isinstance(v, dict) and "detected" in v
    )
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
