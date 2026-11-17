"""Judge one run of the r3-only condition of leaf `macos-timeout-inspection-k193`.

This checker adds no rule. It applies `timeout_k4_check.check` revision 3 to the r3 calls
alone, with the r3 observations as the observations that must be present. So the r3 roles,
the original exit codes, the typed receipts, the stall admission after a valid intentional
timeout (`r3-timeout`), the owned cleanup state and the disposal checks are the k4 ones.

Exit 0 only if the stall is determined (`reproduced` or `not-reproduced`) and the disposal
has no failure.
"""

import json
import sys
from pathlib import Path
from typing import Any

from timeout_k4_check import R3, R3_OBSERVATIONS, check

SHAPE = R3
OBSERVATIONS = R3_OBSERVATIONS


def check_r3(run: Path) -> dict[str, Any]:
    """Judge one r3-only run directory and return the checker's JSON object."""
    return {"condition": "r3-only", **check(run, SHAPE, OBSERVATIONS)}


def main() -> int:
    report = check_r3(Path(sys.argv[1]))
    print(json.dumps(report, indent=2))
    determined = report["stall"] in {"reproduced", "not-reproduced"}
    return 0 if determined and not report["disposal_failures"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
