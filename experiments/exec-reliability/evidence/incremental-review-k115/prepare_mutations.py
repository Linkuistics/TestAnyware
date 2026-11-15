"""Freeze three precise incremental shutdown regressions for control challenge."""

import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
DIAGNOSTIC = HERE.parents[1]
MUTATIONS = {
    "drain-loop": (
        "        assert state is not None\n",
        "        assert state is not None\n"
        "        while time.monotonic() < state.end:\n"
        "            time.sleep(0.001)\n",
    ),
    "wake-kill": (
        "                        state.kill_at if not state.kill_sent else state.end,\n",
        "                        state.end,\n",
    ),
    "wake-end": (
        "                    return min(\n"
        "                        now + 0.001,\n"
        "                        state.end,\n"
        "                        state.kill_at if not state.kill_sent else state.end,\n"
        "                    )\n",
        "                    return now + 0.001\n",
    ),
}


def main() -> None:
    for name, (before, after) in MUTATIONS.items():
        destination = HERE / f"{name}-inputs"
        subprocess.run(
            [
                sys.executable,
                str(HERE.parent / "worker-reap-scope-k112/prepare.py"),
                str(HERE / "green-inputs"),
                str(destination),
            ],
            check=True,
        )
        path = destination / "worker_capture.py"
        source = path.read_text()
        assert source.count(before) == 1, name
        path.write_text(source.replace(before, after))
        with (destination / "mutation.json").open("x") as stream:
            json.dump({"name": name, "before": before, "after": after}, stream, indent=2)
            stream.write("\n")


if __name__ == "__main__":
    main()
