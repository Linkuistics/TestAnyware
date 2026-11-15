"""Inspect expired-request fixture absence outside the completed checker process."""

import json
import subprocess
from pathlib import Path


def expired_audits(output: Path) -> list[dict[str, object]]:
    audits: list[dict[str, object]] = []
    for path in sorted((output / "runs").glob("persistent-expired-independent-*.json")):
        value = json.loads(path.read_text())
        pid = value["before_audit"]["owner"]["pid"]
        current = subprocess.run(
            ["/bin/ps", "-p", str(pid), "-o", "pid=,pgid=,stat=,lstart=,command="],
            capture_output=True,
            text=True,
            timeout=1,
        )
        audits.append(
            {
                "journal": str(path),
                "pid": pid,
                "schedule": value["schedule"],
                "exact_child_absent": value["exact_child_absent"],
                "absent": current.returncode == 1 and not current.stdout and not current.stderr,
                "stdout": current.stdout,
                "stderr": current.stderr,
                "returncode": current.returncode,
                "signals": [],
            }
        )
    return audits
