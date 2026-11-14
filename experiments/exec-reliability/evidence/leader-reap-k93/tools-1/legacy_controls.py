"""Run the original auditor with explicitly injected decoded evidence corruption."""

import argparse
import copy
import importlib.util
import json
import sys
from pathlib import Path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    args.output.mkdir(exist_ok=False)
    old = Path(__file__).resolve().parent.with_name("leader-reap-k88")
    spec = importlib.util.spec_from_file_location("legacy", old / "audit.py")
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    original_load, original_digest = module.load, module.digest
    results = []
    for defect in ("omitted-inventory-entry", "unrelated-subject-archive"):

        def load(path: Path, defect: str = defect) -> dict:
            value = copy.deepcopy(original_load(path))
            if defect == "omitted-inventory-entry" and path == old / "prior-evidence.json":
                del value[next(iter(value))]
            if defect == "unrelated-subject-archive" and path == old / "green-2/verdict.json":
                for key in ("subjects_before", "subjects_after"):
                    subject = next(name for name in value[key] if name.endswith("controller.py"))
                    value[key][subject] = original_digest(old / "Taskfile.green-2.yml")
            return value

        def digest(path: Path) -> str:
            # Legitimate historical source identities, explicitly bound by name.
            if path.parent == old.parents[1] and path.name in {
                "controller.py",
                "test_controller.py",
            }:
                return original_digest(old / "green-inputs" / path.name)
            return original_digest(path)

        module.load, module.digest = load, digest
        output = args.output / f"{defect}.json"
        sys.argv = [str(old / "audit.py"), str(output)]
        try:
            module.main()
        except AssertionError as error:
            results.append({"control": defect, "rejected": True, "error": str(error)})
        else:
            results.append({"control": defect, "rejected": False})
    (args.output / "controls.json").write_text(json.dumps(results, indent=2) + "\n")
    print(json.dumps(results, indent=2))
    raise SystemExit(0 if all(item["rejected"] for item in results) else 1)


if __name__ == "__main__":
    main()
