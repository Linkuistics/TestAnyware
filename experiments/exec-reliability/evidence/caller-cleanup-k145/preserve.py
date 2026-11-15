"""Digest every prior evidence path and content; allow only the k140 report correction."""

import hashlib
import json
import os
import sys
from pathlib import Path

REPORT = "caller-scheduling-k140/report.md"


def summarize(snapshot: dict[str, str]) -> dict[str, object]:
    # Length-prefixed JSON records bind every path, value and symlink target.
    digest = hashlib.sha256()
    for key, value in sorted(snapshot.items()):
        if key != REPORT:
            record = json.dumps([key, value], ensure_ascii=True).encode()
            digest.update(len(record).to_bytes(8, "big"))
            digest.update(record)
    return {
        "count": len(snapshot),
        "sha256_except_report": digest.hexdigest(),
        "report_sha256": snapshot[REPORT],
    }


def main() -> None:
    mode = sys.argv[1]
    here = Path(__file__).resolve().parent
    evidence = here.parent
    snapshot: dict[str, str] = {}
    for directory, dirs, files in os.walk(evidence):
        base = Path(directory)
        dirs[:] = sorted(
            name
            for name in dirs
            if base / name != here and name not in {"__pycache__", ".pytest_cache", ".ruff_cache"}
        )
        for name in sorted(files + [name for name in dirs if (base / name).is_symlink()]):
            path = base / name
            snapshot[str(path.relative_to(evidence))] = (
                "symlink:" + os.readlink(path)
                if path.is_symlink()
                else hashlib.sha256(path.read_bytes()).hexdigest()
            )
    after = summarize(snapshot)
    if mode == "before":
        with (here / "prior-before.json").open("x") as stream:
            json.dump(after, stream, indent=2)
    elif mode == "after":
        before = json.loads((here / "prior-before.json").read_text())
        ok = (
            before["count"] == after["count"]
            and before["sha256_except_report"] == after["sha256_except_report"]
            and before["report_sha256"] != after["report_sha256"]
        )
        with (here / "prior-after.json").open("x") as stream:
            json.dump(
                {"before": before, "after": after, "ok": ok, "permitted_change": REPORT},
                stream,
                indent=2,
            )
        if not ok:
            raise SystemExit("prior evidence changed outside the k140 report correction")
    else:
        raise ValueError("expected before or after")
    print(f"{mode}: {len(snapshot)} prior evidence paths")


if __name__ == "__main__":
    main()
