"""Hash the retained issue #14 macOS evidence, or compare it with the manifest.

The evidence stays in the Ravel-Strategies checkout that recorded it. This
script copies only the receipts that k3 cites. It hashes every file in the
retained run and the setup identity files.

Usage: retained.py snapshot|compare
"""

from __future__ import annotations

import hashlib
import json
import shutil
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
SOURCE = Path(
    "/Users/antony/Development/AgenticOrchestration/Ravel-Strategies.gh-issue-1"
    "/experiments/tutorial-vm-reliability/evidence"
)
MANIFEST = HERE / "retained-manifest.json"
COPIES = HERE / "retained"
TREES = ("macos-candidate-01",)
FILES = (
    "setup/host.txt",
    "setup/tools-before.txt",
    "setup/tools-after.txt",
    "upstream/testanyware-issue-14.md",
)
CITED = (
    "macos-candidate-01/run.json",
    "macos-candidate-01/inventory.json",
    *(
        f"macos-candidate-01/ordinary-{i:02d}.{ext}"
        for i in range(1, 11)
        for ext in ("json", "stdout", "stderr")
    ),
)


def digest() -> dict[str, str]:
    paths = [p for tree in TREES for p in sorted((SOURCE / tree).rglob("*")) if p.is_file()]
    paths += [SOURCE / name for name in FILES]
    return {str(p.relative_to(SOURCE)): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}


def main() -> int:
    mode = sys.argv[1] if len(sys.argv) == 2 else ""
    current = digest()
    if mode == "snapshot":
        with MANIFEST.open("x") as stream:
            json.dump({"source": str(SOURCE), "files": current}, stream, indent=2, sort_keys=True)
        COPIES.mkdir()
        for name in CITED:
            target = COPIES / name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(SOURCE / name, target)
        print(f"snapshot: {len(current)} retained files, {len(CITED)} copies")
        return 0
    if mode == "compare":
        recorded = json.loads(MANIFEST.read_text())["files"]
        copies_ok = all(
            hashlib.sha256((COPIES / name).read_bytes()).hexdigest() == recorded[name]
            for name in CITED
        )
        ok = recorded == current and copies_ok
        print(f"compare: {len(current)} retained files, unchanged={ok}")
        return 0 if ok else 1
    print("usage: retained.py snapshot|compare", file=sys.stderr)
    return 64


if __name__ == "__main__":
    raise SystemExit(main())
