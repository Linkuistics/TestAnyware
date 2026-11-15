"""Run each frozen mutant once, then the unchanged frozen repaired subjects."""

import json
import subprocess
from pathlib import Path

from prepare import MUTATIONS


def main() -> None:
    here = Path(__file__).resolve().parent
    repo = here.parents[3]
    records: list[dict[str, object]] = []

    def run(task: str, **variables: str) -> int:
        argv = ["task", task, *(f"{key}={value}" for key, value in variables.items())]
        result = subprocess.run(argv, cwd=repo, check=False)
        records.append({"argv": argv, "exit": result.returncode})
        (here / "batch.json").write_text(json.dumps(records, indent=2) + "\n")
        return result.returncode

    clean = str(here / "inputs")
    assert run("diagnostic:bootstrap-review-prepare", INPUTS=str(here.parents[1]), OUT=clean) == 0
    for mutant in MUTATIONS:
        subject = str(here / f"inputs-{mutant}")
        assert (
            run("diagnostic:bootstrap-review-prepare", INPUTS=clean, OUT=subject, MUTANT=mutant)
            == 0
        )
        assert (
            run(
                "diagnostic:bootstrap-review-check", INPUTS=subject, OUT=str(here / f"red-{mutant}")
            )
            == 1
        )
    assert run("diagnostic:bootstrap-review-check", INPUTS=clean, OUT=str(here / "green")) == 0
    assert (
        run("diagnostic:bootstrap-review-check", INPUTS=clean, OUT=str(here / "full"), MODE="full")
        == 0
    )


if __name__ == "__main__":
    main()
