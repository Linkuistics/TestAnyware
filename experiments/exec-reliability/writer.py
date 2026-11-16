"""Evidence writer: apply storage frames to one run directory.

The contract is docs/specs/diagnostic-evidence-storage.md, sections
"Ownership", "Channel contract", "Close transaction", "Completion authority"
and "Controller-crash limits".

The controller starts this module as `python -I writer.py`. Frames arrive on
standard input, and one JSON acknowledgement line per frame leaves on standard
output. After setup, the writer holds only its three standard descriptors and
its run-directory descriptor. Every file operation is relative to that
descriptor, creates exclusively and does not follow links.

A frame is two big-endian 32-bit lengths, a JSON header of the first length
and a payload of the second length. The header holds `seq`, `op` and the
fields of its operation. The writer imports only the standard library,
because isolated mode leaves the script directory off the module path.
"""

from __future__ import annotations

import errno
import hashlib
import json
import os
import stat
import struct
import sys
import time
from typing import cast

MAX_PAYLOAD = 1 << 20
MAX_HEADER = 1 << 20
LENGTHS = struct.Struct(">II")
READ_SIZE = 1 << 16
IDENTITY_FORMAT = 1

# Each reserved record keeps the file name that existing callers read.
RESERVED_FILES = {
    "plan": "plan.json",
    "result": "result.json",
    "subjects-before": "subjects-before.json",
    "storage-close": "storage-close.json",
    "result-identity": "result-identity.json",
}
IDENTITY = RESERVED_FILES["result-identity"]
STORAGE_CLOSE = RESERVED_FILES["storage-close"]

_CREATE = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC
_APPEND = os.O_WRONLY | os.O_APPEND | os.O_NOFOLLOW | os.O_CLOEXEC
_DIRECTORY = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
# O_NONBLOCK keeps a planted FIFO from blocking the identity read.
_PIN = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC


def record_file(name: str) -> str:
    """Return the file name of a record. Only reserved records differ."""
    return RESERVED_FILES.get(name, name)


def encode_frame(seq: int, op: str, payload: bytes = b"", **fields: object) -> bytes:
    """Serialise one frame. Controllers and test clients share this encoding."""
    header = json.dumps({"seq": seq, "op": op, **fields}, separators=(",", ":")).encode()
    return LENGTHS.pack(len(header), len(payload)) + header + payload


class Rejected(Exception):
    """A frame that the writer refused or could not apply."""

    def __init__(self, status: str, code: int, call: str) -> None:
        super().__init__(status, code, call)
        self.status = status
        self.code = code
        self.call = call


def _refused(code: int, call: str = "validate") -> Rejected:
    return Rejected("refused", code, call)


def _failed(call: str, error: OSError) -> Rejected:
    return Rejected("failed", error.errno if error.errno is not None else errno.EIO, call)


def _valid_name(name: object) -> bool:
    return (
        isinstance(name, str)
        and name not in ("", ".", "..")
        and "/" not in name
        and "\0" not in name
    )


# A default macOS volume ignores case, so "RESULT.json" opens result.json.
# The check compares case-folded names, which also covers such volumes.
_RESERVED_FOLDED = {name.casefold() for pair in RESERVED_FILES.items() for name in pair}


def _reserved(name: str) -> bool:
    return name.casefold() in _RESERVED_FOLDED


def _json_bytes(value: object) -> bytes:
    # The same layout as controller.write_json, so readers see no change.
    return json.dumps(value, indent=2).encode() + b"\n"


def _pin_bytes(data: bytes) -> dict[str, object]:
    return {"length": len(data), "sha256": hashlib.sha256(data).hexdigest()}


class FrameReader:
    """Split standard input bytes into frames, and skip oversized parts."""

    def __init__(self) -> None:
        self.buffer = bytearray()
        self.skip = 0

    def frames(self, data: bytes):  # noqa: ANN201 - a generator of frame events
        self.buffer += data
        while True:
            if self.skip:
                count = min(self.skip, len(self.buffer))
                del self.buffer[:count]
                self.skip -= count
                if self.skip:
                    return
            if len(self.buffer) < LENGTHS.size:
                return
            header_length, payload_length = LENGTHS.unpack_from(self.buffer)
            start = LENGTHS.size
            if header_length > MAX_HEADER:
                del self.buffer[:start]
                self.skip = header_length + payload_length
                yield None, None
                continue
            if len(self.buffer) < start + header_length:
                return
            header = bytes(self.buffer[start : start + header_length])
            if payload_length > MAX_PAYLOAD:
                del self.buffer[: start + header_length]
                self.skip = payload_length
                yield header, None
                continue
            end = start + header_length + payload_length
            if len(self.buffer) < end:
                return
            payload = bytes(self.buffer[start + header_length : end])
            del self.buffer[:end]
            yield header, payload


class Writer:
    """Apply frames in sequence order and run the close transaction."""

    def __init__(self, ack_fd: int = 1) -> None:
        self.ack_fd = ack_fd
        self.acks_open = True
        self.run_fd: int | None = None
        self.run_name: str | None = None
        self.closed = False
        self.subjects: list[str] = []
        self.created: set[str] = set()
        self.last_seq: int | None = None
        self.last_applied: int | None = None
        self.refused = 0
        self.failed = 0

    # Acknowledgements -------------------------------------------------------

    def ack(self, message: dict[str, object]) -> None:
        if not self.acks_open:
            return
        line = json.dumps(message, separators=(",", ":")).encode() + b"\n"
        view = memoryview(line)
        try:
            while view:
                view = view[os.write(self.ack_fd, view) :]
        except OSError:
            # The controller is gone. Frames still apply until end of input.
            self.acks_open = False

    def reject(self, seq: object, op: object, rejection: Rejected) -> None:
        if rejection.status == "refused":
            self.refused += 1
        else:
            self.failed += 1
        self.ack(
            {
                "seq": seq,
                "op": op,
                "status": rejection.status,
                "errno": rejection.code,
                "error": os.strerror(rejection.code),
                "call": rejection.call,
            }
        )

    # Frame handling ---------------------------------------------------------

    def handle(self, header_bytes: bytes | None, payload: bytes | None) -> bool:
        """Apply one frame. Return True when the writer must read no further frame."""
        if header_bytes is None:
            self.reject(None, None, _refused(errno.EMSGSIZE))
            return False
        try:
            decoded: object = json.loads(header_bytes)
        except ValueError:
            decoded = None
        if not isinstance(decoded, dict):
            self.reject(None, None, _refused(errno.EPROTO))
            return False
        header = cast(dict[str, object], decoded)
        seq, op = header.get("seq"), header.get("op")
        if not isinstance(seq, int) or isinstance(seq, bool):
            self.reject(None, op, _refused(errno.EPROTO))
            return False
        if self.last_seq is not None and seq <= self.last_seq:
            self.reject(seq, op, _refused(errno.EPROTO))
            return False
        self.last_seq = seq
        if payload is None:
            self.reject(seq, op, _refused(errno.EMSGSIZE))
            return False
        if op == "close":
            return self.close(seq)
        try:
            extra = self.apply(op, header, payload)
        except Rejected as rejection:
            self.reject(seq, op, rejection)
            return False
        self.last_applied = seq
        self.ack({"seq": seq, "op": op, "status": "ok", **extra})
        return False

    def apply(self, op: object, header: dict[str, object], payload: bytes) -> dict[str, object]:
        if op == "setup":
            if self.run_fd is not None:
                raise _refused(errno.EALREADY)
            return self.setup(header, payload)
        if op not in ("create", "append", "result", "subjects"):
            raise _refused(errno.EPROTO)
        if self.run_fd is None:
            raise _refused(errno.EBADF)
        if op == "subjects":
            return {"subjects": self.hash_subjects()}
        name = header.get("name")
        if not _valid_name(name):
            raise _refused(errno.EINVAL)
        assert isinstance(name, str)
        if op == "result":
            if name != "result":
                raise _refused(errno.EINVAL)
            self.create(name, payload)
        elif op == "create":
            if _reserved(name):
                raise _refused(errno.EPERM)
            self.create(name, payload)
        else:
            if _reserved(name) and name != "result":
                raise _refused(errno.EPERM)
            if name not in self.created:
                raise _refused(errno.ENOENT)
            self.append(name, payload)
        return {}

    def setup(self, header: dict[str, object], payload: bytes) -> dict[str, object]:
        run, root, subjects = header.get("run"), header.get("root"), header.get("subjects")
        if not _valid_name(run) or not isinstance(root, str) or not os.path.isabs(root):
            raise _refused(errno.EINVAL)
        if not isinstance(subjects, list):
            raise _refused(errno.EPROTO)
        listed = [subject for subject in cast(list[object], subjects) if isinstance(subject, str)]
        if len(listed) != len(cast(list[object], subjects)):
            raise _refused(errno.EPROTO)
        assert isinstance(run, str)
        try:
            os.makedirs(root, exist_ok=True)
        except OSError as error:
            raise _failed("mkdir", error) from error
        resolved = os.path.realpath(root)
        try:
            root_fd = os.open(resolved, os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
        except OSError as error:
            raise _failed("open", error) from error
        try:
            try:
                os.mkdir(run, 0o755, dir_fd=root_fd)
            except OSError as error:
                raise _failed("mkdir", error) from error
            try:
                self.run_fd = os.open(run, _DIRECTORY, dir_fd=root_fd)
            except OSError as error:
                raise _failed("open", error) from error
        finally:
            os.close(root_fd)
        self.run_name = run
        self.subjects = listed
        self.create("plan", payload)
        digests = self.hash_subjects()
        before = _json_bytes(digests)
        self.create("subjects-before", before)
        return {
            "run_dir": os.path.join(resolved, run),
            "subjects": digests,
            "subjects_before": _pin_bytes(before),
        }

    def hash_subjects(self) -> dict[str, str]:
        # The same values as controller._hashes, so the result keeps its shape.
        digests: dict[str, str] = {}
        for subject in self.subjects:
            try:
                with open(subject, "rb") as stream:
                    digests[subject] = hashlib.file_digest(stream, "sha256").hexdigest()
            except OSError as error:
                digests[subject] = f"unavailable: {error}"
        return digests

    # File operations relative to the run-directory descriptor ---------------

    def _write(self, flags: int, name: str, data: bytes) -> None:
        file_name = record_file(name)
        try:
            descriptor = os.open(file_name, flags, 0o644, dir_fd=self.run_fd)
        except OSError as error:
            raise _failed("open", error) from error
        try:
            view = memoryview(data)
            while view:
                view = view[os.write(descriptor, view) :]
        except OSError as error:
            raise _failed("write", error) from error
        finally:
            os.close(descriptor)

    def create(self, name: str, data: bytes) -> None:
        self._write(_CREATE, name, data)
        self.created.add(name)

    def append(self, name: str, data: bytes) -> None:
        self._write(_APPEND, name, data)

    def _pin(self, directory: int, name: str, path: str) -> dict[str, object]:
        started = time.monotonic()
        try:
            descriptor = os.open(name, _PIN, dir_fd=directory)
        except OSError as error:
            return {"path": path, "error": str(error), "started_monotonic": started,
                    "finished_monotonic": time.monotonic()}  # fmt: skip
        try:
            if not stat.S_ISREG(os.fstat(descriptor).st_mode):
                return {"path": path, "error": "not a regular file",
                        "started_monotonic": started,
                        "finished_monotonic": time.monotonic()}  # fmt: skip
            digest = hashlib.sha256()
            length = 0
            while chunk := os.read(descriptor, MAX_PAYLOAD):
                digest.update(chunk)
                length += len(chunk)
        except OSError as error:
            return {"path": path, "error": str(error), "started_monotonic": started,
                    "finished_monotonic": time.monotonic()}  # fmt: skip
        finally:
            os.close(descriptor)
        return {
            "path": path,
            "length": length,
            "sha256": digest.hexdigest(),
            "started_monotonic": started,
            "finished_monotonic": time.monotonic(),
        }

    def _listing(self, directory: int, prefix: str) -> list[dict[str, object]]:
        try:
            with os.scandir(directory) as entries:
                children = sorted((entry.name, entry.is_dir(follow_symlinks=False))
                                  for entry in entries)  # fmt: skip
        except OSError as error:
            raise _failed("scandir", error) from error
        pins: list[dict[str, object]] = []
        for name, is_directory in children:
            path = prefix + name
            if not prefix and name == IDENTITY:
                continue
            if not is_directory:
                pins.append(self._pin(directory, name, path))
                continue
            try:
                child = os.open(name, _DIRECTORY, dir_fd=directory)
            except OSError as error:
                raise _failed("open", error) from error
            try:
                pins.extend(self._listing(child, path + "/"))
            finally:
                os.close(child)
        return pins

    # Close transaction ------------------------------------------------------

    def write_storage_close(self, state: str, **extra: object) -> dict[str, object]:
        record = {
            "state": state,
            "last_applied": self.last_applied,
            "refused": self.refused,
            "failed": self.failed,
            **extra,
        }
        data = _json_bytes(record)
        self.create("storage-close", data)
        return _pin_bytes(data)

    def write_identity(self) -> dict[str, object]:
        """Pin every file under the run directory except the identity record."""
        assert self.run_fd is not None
        started = time.monotonic()
        files = self._listing(self.run_fd, "")
        identity = {
            "format": IDENTITY_FORMAT,
            "run_dir": self.run_name,
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
        data = _json_bytes(identity)
        self.create("result-identity", data)
        return _pin_bytes(data)

    def close_transaction(self) -> dict[str, object]:
        """Write storage-close, then the identity record that lists it."""
        storage_close = self.write_storage_close("orderly")
        identity = self.write_identity()
        return {"storage_close": storage_close, "identity": identity}

    def close(self, seq: int) -> bool:
        if self.run_fd is None:
            self.reject(seq, "close", _refused(errno.EBADF))
            return True
        try:
            pins = self.close_transaction()
        except Rejected as rejection:
            self.reject(seq, "close", rejection)
            return True
        self.ack({"seq": seq, "op": "close", "status": "ok", **pins})
        self.closed = True
        return True

    def end_of_input(self, discarded: int) -> None:
        """Record that input ended without a close frame. Write no identity record."""
        if self.run_fd is None:
            return
        try:
            self.write_storage_close("eof_without_close", discarded_bytes=discarded)
        except Rejected:
            pass


def run(writer: Writer, input_fd: int = 0) -> int:
    """Read frames until close or end of input. Exit 0 only after an orderly close."""
    reader = FrameReader()
    while True:
        data = os.read(input_fd, READ_SIZE)
        if not data:
            writer.end_of_input(len(reader.buffer))
            return 1
        for header, payload in reader.frames(data):
            if writer.handle(header, payload):
                return 0 if writer.closed else 1


def main(argv: list[str]) -> int:
    if argv:
        print("usage: python -I writer.py < frames > acknowledgements", file=sys.stderr)
        return 2
    return run(Writer())


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
