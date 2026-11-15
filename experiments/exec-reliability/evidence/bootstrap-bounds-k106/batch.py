"""Run each frozen mutant once, then the unchanged frozen repaired subjects."""

import json
import runpy
import subprocess
from pathlib import Path
from typing import cast


def main() -> None:
    here = Path(__file__).resolve().parent
    repo = here.parents[3]
    records: list[dict[str, object]] = []

    def run(task: str, **variables: str) -> int:
        argv = ["task", task, *(f"{key}={value}" for key, value in variables.items())]
        result = subprocess.run(argv, cwd=repo, check=False)
        records.append({"argv": argv, "exit": result.returncode})
        (here / "batch-attempt-2.json").write_text(json.dumps(records, indent=2) + "\n")
        return result.returncode

    attempt = here / "attempt-2"
    attempt.mkdir()
    clean = str(attempt / "inputs")
    assert run("diagnostic:bootstrap-review-prepare", INPUTS=str(here.parents[1]), OUT=clean) == 0
    mutations = cast(
        dict[str, tuple[str, str, str]],
        runpy.run_path(str(here / "prepare.py"))["MUTATIONS"],
    )
    for mutant in mutations:
        subject = str(attempt / f"inputs-{mutant}")
        assert (
            run("diagnostic:bootstrap-review-prepare", INPUTS=clean, OUT=subject, MUTANT=mutant)
            == 0
        )
        assert (
            run(
                "diagnostic:bootstrap-review-check",
                INPUTS=subject,
                OUT=str(attempt / f"red-{mutant}"),
            )
            == 201
        )
        verdict = json.loads((attempt / f"red-{mutant}" / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"]
        assert all(check["reaped"] and not check["timed_out"] for check in verdict["checks"])
        assert verdict["checks"][0]["returncode"] == 1
    assert run("diagnostic:bootstrap-review-check", INPUTS=clean, OUT=str(attempt / "green")) == 0
    assert (
        run(
            "diagnostic:bootstrap-review-check",
            INPUTS=clean,
            OUT=str(attempt / "full"),
            MODE="full",
        )
        == 0
    )


if __name__ == "__main__":
    main()
