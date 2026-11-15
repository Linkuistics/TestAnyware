"""Hash prior evidence without following directory symlinks; compare exact inventory."""

import hashlib
import json
import os
import sys
from pathlib import Path


def main() -> None:
    mode = sys.argv[1]
    here = Path(__file__).resolve().parent
    evidence = here.parent
    snapshot: dict[str, str] = {}
    for directory, dirs, files in os.walk(evidence):
        base = Path(directory)
        dirs[:] = sorted(name for name in dirs if base / name != here)
        for name in sorted(files + [name for name in dirs if (base / name).is_symlink()]):
            path = base / name
            snapshot[str(path.relative_to(evidence))] = (
                "symlink:" + os.readlink(path)
                if path.is_symlink()
                else hashlib.sha256(path.read_bytes()).hexdigest()
            )
    if mode == "before":
        with (here / "prior-before.json").open("x") as stream:
            json.dump(snapshot, stream, indent=2)
    elif mode == "after":
        before = json.loads((here / "prior-before.json").read_text())
        changed = sorted(
            key for key in before.keys() | snapshot.keys() if before.get(key) != snapshot.get(key)
        )
        with (here / "prior-after.json").open("x") as stream:
            json.dump(
                {
                    "count": len(snapshot),
                    "changed": changed,
                    "snapshot": snapshot,
                    "ok": not changed,
                },
                stream,
                indent=2,
            )
        if changed:
            raise SystemExit("prior evidence changed")
    else:
        raise ValueError("expected before or after")
    print(f"{mode}: {len(snapshot)} prior evidence paths")


if __name__ == "__main__":
    main()
