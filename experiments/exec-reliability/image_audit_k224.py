"""Record and compare the local Tart images (leaf acceptance-preparation-k224).

    image_audit_k224.py snapshot OUT.json [--hash NAME ...]
    image_audit_k224.py compare BEFORE.json AFTER.json

`snapshot` reads `tart list --format json` and the files of each image under
`~/.tart/vms`. For each file it records the size, the inode and the times of
modification and status change. It records the SHA-256 of `config.json` and
`nvram.bin` for every image, and of `disk.img` for each image that `--hash`
names. It only reads. A read of a disk changes its access time only.

`compare` judges each image that both snapshots hashed:

- Evidence: each file is present in both snapshots, with a size, a SHA-256
  and a modification time. Missing evidence fails the image, because equal
  absent records establish nothing about the content.
- Unchanged: each file has the same size and the same SHA-256.
- Not booted: Tart lists the image as stopped in both snapshots, and the
  modification times of `disk.img` and `nvram.bin` are equal. A boot writes
  both files.

`compare` prints one JSON object and exits 0 only when each hashed image is
unchanged and not booted.
"""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, cast

TART = "/opt/homebrew/bin/tart"
VMS = Path.home() / ".tart" / "vms"
FILES = ("config.json", "nvram.bin", "disk.img")
SMALL = ("config.json", "nvram.bin")
LIST_BOUND = 60.0
CHUNK = 8 * 1024 * 1024


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(CHUNK):
            digest.update(chunk)
    return digest.hexdigest()


def file_record(path: Path, hashed: bool) -> dict[str, Any]:
    if not path.is_file():
        return {"present": False}
    status = path.stat()
    record: dict[str, Any] = {
        "present": True,
        "size": status.st_size,
        "inode": status.st_ino,
        "mtime_ns": status.st_mtime_ns,
        "ctime_ns": status.st_ctime_ns,
    }
    if hashed:
        started = time.monotonic()
        record["sha256"] = sha256(path)
        record["hash_seconds"] = round(time.monotonic() - started, 3)
    return record


def tart_list() -> dict[str, Any]:
    started = time.monotonic()
    done = subprocess.run(
        (TART, "list", "--format", "json"),
        capture_output=True,
        timeout=LIST_BOUND,
        check=False,
    )
    return {
        "argv": [TART, "list", "--format", "json"],
        "returncode": done.returncode,
        "elapsed": round(time.monotonic() - started, 3),
        "stdout": done.stdout.decode("utf-8", "replace"),
        "stderr": done.stderr.decode("utf-8", "replace"),
    }


def snapshot(hash_names: list[str]) -> dict[str, Any]:
    images: dict[str, Any] = {}
    for directory in sorted(VMS.iterdir()):
        if not directory.is_dir():
            continue
        full = directory.name in hash_names
        images[directory.name] = {
            "hashed": full,
            "files": {name: file_record(directory / name, full or name in SMALL) for name in FILES},
        }
    missing = [name for name in hash_names if name not in images]
    if missing:
        raise SystemExit(f"no image directory for {missing}")
    return {
        "schema": "image-audit-k224/1",
        "wall_time": time.time(),
        "vms": str(VMS),
        "tart": tart_list(),
        "images": images,
    }


def _states(record: dict[str, Any]) -> dict[str, str]:
    listing = cast(dict[str, Any], record["tart"])
    if listing["returncode"] != 0:
        return {}
    entries = cast(list[dict[str, Any]], json.loads(listing["stdout"]))
    return {
        str(entry.get("Name")): str(entry.get("State"))
        for entry in entries
        if entry.get("Source") == "local"
    }


def compare(before: dict[str, Any], after: dict[str, Any]) -> dict[str, Any]:
    states_before, states_after = _states(before), _states(after)
    verdicts: dict[str, Any] = {}
    for name, image in cast(dict[str, Any], before["images"]).items():
        if not image["hashed"]:
            continue
        later = cast(dict[str, Any], after["images"]).get(name)
        failures: list[str] = []
        if later is None or not later["hashed"]:
            verdicts[name] = {"ok": False, "failures": ["the later snapshot did not hash it"]}
            continue
        for file in FILES:
            first, second = image["files"][file], later["files"][file]
            for label, record in (("before", first), ("after", second)):
                if (
                    record.get("present") is not True
                    or not isinstance(record.get("size"), int)
                    or not isinstance(record.get("sha256"), str)
                    or not isinstance(record.get("mtime_ns"), int)
                ):
                    failures.append(f"{file} {label} lacks its presence, size, digest or time")
            for key in ("present", "size", "sha256"):
                if first.get(key) != second.get(key):
                    failures.append(f"{file} {key} {first.get(key)!r} -> {second.get(key)!r}")
        for file in ("disk.img", "nvram.bin"):
            first, second = image["files"][file], later["files"][file]
            if first.get("mtime_ns") != second.get("mtime_ns"):
                failures.append(f"{file} was modified: the image may have booted")
        for label, states in (("before", states_before), ("after", states_after)):
            if states.get(name) != "stopped":
                failures.append(f"tart state {label} {states.get(name)!r}")
        verdicts[name] = {"ok": not failures, "failures": failures}
    return {
        "ok": bool(verdicts) and all(v["ok"] for v in verdicts.values()),
        "images": verdicts,
    }


def main() -> None:
    verb = sys.argv[1] if len(sys.argv) > 1 else ""
    if verb == "snapshot" and len(sys.argv) >= 3:
        output = Path(sys.argv[2])
        rest = sys.argv[3:]
        names = [rest[i + 1] for i in range(0, len(rest), 2) if rest[i] == "--hash"]
        if len(names) * 2 != len(rest):
            raise SystemExit("usage: snapshot OUT.json [--hash NAME ...]")
        record = snapshot(names)
        with output.open("x") as stream:
            json.dump(record, stream, indent=2)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        return
    if verb == "compare" and len(sys.argv) == 4:
        first = json.loads(Path(sys.argv[2]).read_bytes())
        second = json.loads(Path(sys.argv[3]).read_bytes())
        verdict = compare(first, second)
        print(json.dumps(verdict, indent=2))
        raise SystemExit(0 if verdict["ok"] else 1)
    raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
