"""Judge the guest receipts of one completion_k186.py run.

    completion_k186_check.py RUN_DIR

A sample passes when all of these hold:

- The host CLI exited 0 inside its host bound of 50 s.
- The receipt is one JSON object with `ok` true, `exit_code` 0, `timed_out` false,
  `stdout` exactly "ordinary-ok" and `stderr` empty.

Every other sample fails, and the checker names the failed condition. The checker prints
one JSON object and exits 0 only when every reached sample passes and none is unreached.
"""

import json
import sys
from pathlib import Path
from typing import Any, cast

HOST_BOUND = 50.0
EXPECTED = {"ok": True, "exit_code": 0, "timed_out": False, "stdout": "ordinary-ok", "stderr": ""}


def judge(host_exit: object, elapsed: float, raw: bytes) -> dict[str, Any]:
    failures: list[str] = []
    if host_exit != 0:
        failures.append(f"host exit {host_exit}")
    if elapsed > HOST_BOUND:
        failures.append(f"elapsed {elapsed:.3f} s above {HOST_BOUND:g} s")
    receipt: object = None
    try:
        receipt = json.loads(raw)
    except ValueError:
        failures.append("receipt is not JSON")
    if isinstance(receipt, dict):
        fields = cast(dict[str, object], receipt)
        for key, value in EXPECTED.items():
            if fields.get(key) != value:
                failures.append(f"{key} {fields.get(key)!r}")
    elif receipt is not None:
        failures.append("receipt is not an object")
    return {
        "host_exit": host_exit,
        "elapsed": elapsed,
        "receipt": receipt,
        "verdict": "pass" if not failures else "fail",
        "failures": failures,
    }


def main() -> int:
    run = Path(sys.argv[1])
    result = json.loads((run / "result.json").read_text())
    samples: dict[str, Any] = {}
    for call in result["calls"]:
        name = call["name"]
        if name.startswith("serial-"):
            raw = (run / f"{name}.stdout").read_bytes()
            samples[name] = judge(call.get("returncode"), call["elapsed"], raw)
        elif name.startswith("concurrent-"):
            round_ = json.loads((run / f"{name}.stdout").read_bytes())
            for child in round_["children"]:
                key = f"{name}.{child['index']}"
                samples[key] = judge(
                    child.get("returncode"), child["elapsed"], bytes.fromhex(child["stdout_hex"])
                )
                samples[key]["started_monotonic"] = child["started_monotonic"]
    unreached = [
        name for name in result.get("unreached", []) if str(name).startswith(("serial-", "conc"))
    ]
    passed = sum(1 for sample in samples.values() if sample["verdict"] == "pass")
    print(
        json.dumps(
            {"run": str(run), "passed": passed, "samples": samples, "unreached": unreached},
            indent=2,
        )
    )
    return 0 if passed == len(samples) and not unreached else 1


if __name__ == "__main__":
    sys.exit(main())
