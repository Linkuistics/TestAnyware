"""Record the app that answers the Host CLI (leaf acceptance-plan-k242).

    acceptance_k223_app.py --cli CLI --observation S

On a macOS host every guest connection goes through the TestAnyware app, so
the trial must show which app answered. `doctor --json` reads `GET /v1/status`
of that app and reports its build and its executable. `doctor` exits 1 when
any host check fails, for example the install path of a private Host CLI. The
controller stops a plan at its first failed call, so this helper runs `doctor`
as its child and exits 0 when that child ran and was reaped.

The helper prints one JSON object with the child's argv, exit status, raw
stdout and stderr in base64, and its monotonic times. acceptance_k223_check.py
compares the answering build and executable with the freeze manifest.
"""

import argparse
import json
import sys
import time
from pathlib import Path

# The plan runs this helper with `python -I`, which leaves the script directory
# out of sys.path. The cancellation helper holds the bounded child runner.
sys.path.insert(0, str(Path(__file__).resolve().parent))

from acceptance_k223_cancel import _run  # noqa: E402  # pyright: ignore[reportPrivateUsage]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cli", required=True)
    parser.add_argument("--observation", required=True, type=float)
    args = parser.parse_args()
    origin = time.monotonic()
    doctor = _run([args.cli, "doctor", "--json"], args.observation, origin)
    record = {"schema": "acceptance-k223-app/1", "doctor": doctor, "ok": doctor["reaped"]}
    sys.stdout.write(json.dumps(record, indent=2) + "\n")
    sys.stdout.flush()
    raise SystemExit(0 if record["ok"] is True else 1)


if __name__ == "__main__":
    main()
