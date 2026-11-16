"""Inventory all earlier evidence without permitting a changed path or byte."""

import hashlib
import json
import os
import sys
from pathlib import Path


def main() -> None:
    mode = sys.argv[1]
    here = Path(__file__).resolve().parent
    evidence = here.parent
    digest = hashlib.sha256()
    count = 0
    for directory, dirs, files in os.walk(evidence):
        base = Path(directory)
        dirs[:] = sorted(
            name
            for name in dirs
            if base / name != here and name not in {"__pycache__", ".pytest_cache", ".ruff_cache"}
        )
        for name in sorted(files + [name for name in dirs if (base / name).is_symlink()]):
            path = base / name
            value = (
                "symlink:" + os.readlink(path)
                if path.is_symlink()
                else hashlib.sha256(path.read_bytes()).hexdigest()
            )
            record = json.dumps([str(path.relative_to(evidence)), value]).encode()
            digest.update(len(record).to_bytes(8, "big"))
            digest.update(record)
            count += 1
    snapshot = {"count": count, "sha256": digest.hexdigest()}
    if mode == "before":
        with (here / "prior-before.json").open("x") as stream:
            json.dump(snapshot, stream, indent=2)
    elif mode == "after":
        before = json.loads((here / "prior-before.json").read_text())
        ok = before == snapshot
        with (here / "prior-after.json").open("x") as stream:
            json.dump({"before": before, "after": snapshot, "ok": ok}, stream, indent=2)
        if not ok:
            raise SystemExit("prior evidence changed")
    else:
        raise ValueError("expected before or after")
    print(f"{mode}: {count} prior evidence paths")


if __name__ == "__main__":
    main()
