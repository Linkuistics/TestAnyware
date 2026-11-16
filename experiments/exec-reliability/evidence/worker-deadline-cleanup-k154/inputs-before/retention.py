"""Pin retained evidence at an observation interval and verify it later.

A pin is a snapshot: it states which bytes one read observed between two
monotonic instants. It is not immutable publication. A later check compares two
snapshots. It cannot see a change that was reverted between them, and it does
not prove that nothing wrote during either read.
"""

import hashlib
import json
import sys
import time
from pathlib import Path

IDENTITY = "result-identity.json"
_FORMAT = 1


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
    """Compare current files with the identity and current streams with receipt pins."""
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
    return {
        "run_dir": str(root),
        "started_monotonic": started,
        "finished_monotonic": time.monotonic(),
        "identity": identity_state,
        "result": result_state,
        "files": files,
        "unlisted": unlisted,
        "streams": streams,
        "ok": identity_ok and files_ok and streams_ok,
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
