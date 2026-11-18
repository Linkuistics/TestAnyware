"""Drop the evidence directory of leaf k221 from a recorded `jj diff --git`.

    python3 -I filter_dirty_diff.py FILE...

measure.sh and the k7 runner recorded `jj diff --git` of the whole working copy.
That diff held this evidence directory too, so each run copied the earlier runs'
records, and the files doubled in size from run to run. This script keeps every
section whose path lies outside the directory, unchanged and in order. It drops
each section inside it. It writes the original size, the original SHA-256 and the
rule to FILE.note, then replaces FILE.
"""

import hashlib
import sys
from pathlib import Path

PREFIX = b"diff --git a/experiments/exec-reliability/evidence/macos-agent-cancellation-k221/"


def main() -> None:
    for name in sys.argv[1:]:
        path = Path(name)
        digest = hashlib.sha256()
        size = 0
        kept: list[bytes] = []
        dropped = 0
        keep = True
        with path.open("rb") as source:
            for line in source:
                digest.update(line)
                size += len(line)
                if line.startswith(b"diff --git "):
                    keep = not line.startswith(PREFIX)
                    dropped += 0 if keep else 1
                if keep:
                    kept.append(line)
        note = (
            f"original bytes {size}\n"
            f"original sha256 {digest.hexdigest()}\n"
            f"dropped sections {dropped}\n"
            "rule: filter_dirty_diff.py dropped each section under "
            "experiments/exec-reliability/evidence/macos-agent-cancellation-k221/\n"
        )
        Path(f"{name}.note").write_text(note)
        path.write_bytes(b"".join(kept))
        print(name, size, "->", path.stat().st_size, "dropped", dropped)


if __name__ == "__main__":
    main()
