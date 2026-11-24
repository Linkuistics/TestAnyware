"""Pin retained evidence at an observation interval and verify it later.

A pin is a snapshot: it states which bytes one read observed between two
monotonic instants. It is not immutable publication. A later check compares two
snapshots. It cannot see a change that was reverted between them, and it does
not prove that nothing wrote during either read.
"""

import hashlib
import json
import os
import sys
import time
from pathlib import Path

IDENTITY = "result-identity.json"
STORAGE_CLOSE = "storage-close.json"
RESULT = "result.json"
_FORMAT = 1
# The writer's record names and file names. writer.py keeps an equal copy,
# because isolated mode stops it from importing this module.
RESERVED_FILES = {
    "plan": "plan.json",
    "result": RESULT,
    "subjects-before": "subjects-before.json",
    "storage-close": STORAGE_CLOSE,
    "result-identity": IDENTITY,
}


def record_file(name: str) -> str:
    """Return the file name of a writer record. Only reserved records differ."""
    return RESERVED_FILES.get(name, name)


def observe_file(path: Path) -> tuple[bytes | None, dict[str, object]]:
    """Read once through a new descriptor and pin exactly the bytes read."""
    started = time.monotonic()
    try:
        data = path.read_bytes()
    except OSError as error:
        return None, {
            "error": str(error),
            "started_monotonic": started,
            "finished_monotonic": time.monotonic(),
        }
    return data, {
        "length": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
        "started_monotonic": started,
        "finished_monotonic": time.monotonic(),
    }


def _listing(root: Path) -> list[str]:
    return sorted(
        str(path.relative_to(root))
        for path in root.rglob("*")
        if path.is_file() and path.relative_to(root) != Path(IDENTITY)
    )


def write_identity(root: Path) -> dict[str, object]:
    """Pin every other run file; this file itself stays unpinned, so no cycle exists."""
    started = time.monotonic()
    files: list[dict[str, object]] = []
    for name in _listing(root):
        _, pin = observe_file(root / name)
        files.append({"path": name, **pin})
    identity: dict[str, object] = {
        "format": _FORMAT,
        "run_dir": root.name,
        "started_monotonic": started,
        "finished_monotonic": time.monotonic(),
        "wall_ns": time.time_ns(),
        "files": files,
        "limits": (
            "Snapshot at the stated interval, not immutable publication. "
            "A change reverted before a later check is invisible. "
            "Anchor this file's digest outside the run directory."
        ),
    }
    with (root / IDENTITY).open("x") as stream:
        json.dump(identity, stream, indent=2)
    return identity


def _compare(pin: object, path: Path) -> dict[str, object]:
    data, observed = observe_file(path)
    if not isinstance(pin, dict) or "sha256" not in pin or "length" not in pin:
        state = "unpinned"
    elif data is None:
        state = "missing" if not path.exists() else "unreadable"
    elif observed["length"] == pin["length"] and observed["sha256"] == pin["sha256"]:
        state = "unchanged"
    else:
        state = "changed"
    return {"pinned": pin, "observed": observed, "state": state}


def _load(path: Path) -> tuple[object, str]:
    try:
        return json.loads(path.read_text()), "ok"
    except FileNotFoundError:
        return None, "missing"
    except (OSError, ValueError) as error:
        return None, f"invalid: {error}"


def verify(root: Path) -> dict[str, object]:
    """Compare current files with the identity and current streams with receipt pins.

    Every run directory must also pass the completion check. A directory from
    before the writer has no storage-close record, so it fails that check:
    nothing in it shows that publication completed.
    """
    root = root.resolve()
    started = time.monotonic()
    identity, identity_state = _load(root / IDENTITY)
    entries: list[object] = []
    if identity_state == "ok":
        listed = identity.get("files") if isinstance(identity, dict) else None
        if not isinstance(listed, list) or not all(
            isinstance(item, dict) and isinstance(item.get("path"), str) for item in listed
        ):
            identity_state = "invalid: files must be a list of path entries"
        elif not any(item["path"] == "result.json" for item in listed):
            identity_state = "invalid: result.json is not pinned"
        else:
            entries = listed
    files = [
        {"path": item["path"], **_compare(item, root / item["path"])}
        for item in entries
        if isinstance(item, dict)
    ]
    pinned_paths = {item["path"] for item in files}
    unlisted = (
        [name for name in _listing(root) if name not in pinned_paths]
        if identity_state == "ok"
        else []
    )
    result, result_state = _load(root / "result.json")
    streams: list[dict[str, object]] = []
    calls = result.get("calls") if isinstance(result, dict) else None
    for receipt in calls if isinstance(calls, list) else []:
        if not isinstance(receipt, dict) or not isinstance(receipt.get("streams"), dict):
            continue
        name = str(receipt.get("name"))
        for stream, pin in receipt["streams"].items():
            streams.append(
                {"call": name, "stream": stream, **_compare(pin, root / f"{name}.{stream}")}
            )
    identity_ok = identity_state == "ok"
    files_ok = all(item["state"] == "unchanged" for item in files) and not unlisted
    streams_ok = result_state == "ok" and all(item["state"] == "unchanged" for item in streams)
    authority = completion(root)
    complete_ok = authority["complete"] is True
    return {
        "run_dir": str(root),
        "started_monotonic": started,
        "finished_monotonic": time.monotonic(),
        "identity": identity_state,
        "result": result_state,
        "files": files,
        "unlisted": unlisted,
        "streams": streams,
        "completion": authority,
        "ok": identity_ok and files_ok and streams_ok and complete_ok,
    }


def _present(root: Path, prefix: str = "") -> list[str]:
    """List every non-directory entry, links included, without following links."""
    names: list[str] = []
    with os.scandir(root) as entries:
        for entry in sorted(entries, key=lambda item: item.name):
            path = prefix + entry.name
            if entry.is_dir(follow_symlinks=False):
                names.extend(_present(root / entry.name, path + "/"))
            elif path != IDENTITY:
                names.append(path)
    return names


def _safe_record(name: object) -> bool:
    return isinstance(name, str) and name not in ("", ".", "..") and "/" not in name


def completion(root: Path) -> dict[str, object]:
    """Apply the completion authority rules of the evidence storage spec.

    docs/specs/diagnostic-evidence-storage.md, "Completion authority". A run
    directory is complete only when every rule holds. Each failed rule adds a
    reason. verify() applies this check to every run directory.
    """
    root = root.resolve()
    started = time.monotonic()
    reasons: list[str] = []

    identity, identity_state = _load(root / IDENTITY)
    files: list[dict[str, object]] = []
    unlisted: list[str] = []
    listed: set[str] = set()
    entries = identity.get("files") if isinstance(identity, dict) else None
    if identity_state != "ok":
        reasons.append(f"identity record {identity_state}")
    elif not isinstance(entries, list) or not all(
        isinstance(item, dict) and isinstance(item.get("path"), str) for item in entries
    ):
        reasons.append("identity record files must be a list of path entries")
    else:
        files = [{"path": item["path"], **_compare(item, root / item["path"])} for item in entries]
        listed = {item["path"] for item in files}
        for item in files:
            if item["state"] != "unchanged":
                reasons.append(f"{item['path']} is {item['state']}")
        unlisted = [name for name in _present(root) if name not in listed]
        for name in unlisted:
            reasons.append(f"{name} is not in the identity listing")

    close, close_state = _load(root / STORAGE_CLOSE)
    last_applied: object = None
    if STORAGE_CLOSE not in listed:
        reasons.append("the identity listing lacks the storage-close record")
    if close_state != "ok" or not isinstance(close, dict):
        reasons.append(f"storage-close record {close_state}")
    else:
        if close.get("state") != "orderly":
            reasons.append(f"storage-close says {close.get('state')!r}, not 'orderly'")
        for count in ("refused", "failed"):
            if close.get(count) != 0:
                reasons.append(f"storage-close reports {close.get(count)!r} {count} frames")
        last_applied = close.get("last_applied")

    result, result_state = _load(root / RESULT)
    storage = result.get("storage") if isinstance(result, dict) else None
    sequence = storage.get("sequence") if isinstance(storage, dict) else None
    digests = storage.get("digests") if isinstance(storage, dict) else None
    checked: list[dict[str, object]] = []
    if result_state != "ok":
        reasons.append(f"result record {result_state}")
    elif not isinstance(sequence, int) or not isinstance(digests, dict):
        reasons.append("result record lacks a storage sequence and digest entries")
    else:
        if not isinstance(last_applied, int) or last_applied < sequence:
            reasons.append(
                f"storage-close last applied {last_applied!r} precedes result {sequence}"
            )
        for name, pin in digests.items():
            if not _safe_record(name) or name == "result":
                reasons.append(f"digest entry {name!r} cannot name a run record")
                continue
            path = record_file(name)
            checked.append({"record": name, "path": path, **_compare(pin, root / path)})
            if checked[-1]["state"] != "unchanged":
                reasons.append(f"digest entry {name} is {checked[-1]['state']}")

    return {
        "run_dir": str(root),
        "started_monotonic": started,
        "finished_monotonic": time.monotonic(),
        "identity": identity_state,
        "files": files,
        "unlisted": unlisted,
        "storage_close": close if close_state == "ok" else close_state,
        "result": result_state,
        "digests": checked,
        "reasons": reasons,
        "complete": not reasons,
    }


def main(argv: list[str]) -> int:
    """`verify RUN_DIR [--out NEW_FILE]`: exit 0 verified, 1 rejected, 2 usage or output error."""
    if len(argv) not in (2, 4) or argv[0] != "verify" or (len(argv) == 4 and argv[2] != "--out"):
        print("usage: retention.py verify RUN_DIR [--out NEW_FILE]", file=sys.stderr)
        return 2
    report = verify(Path(argv[1]))
    text = json.dumps(report, indent=2) + "\n"
    if len(argv) == 4:
        try:
            with Path(argv[3]).open("x") as stream:
                stream.write(text)
        except OSError as error:
            print(f"cannot retain the verdict: {error}", file=sys.stderr)
            return 2
    sys.stdout.write(text)
    return 0 if report["ok"] is True else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
