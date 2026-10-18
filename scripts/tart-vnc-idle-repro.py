#!/usr/bin/env python3
"""Reproduce the tart VNC idle-reconnect crash on a macOS 27 host.

Clones a macOS guest, runs it with `tart run --no-graphics --vnc-experimental`,
connects a bare RFB client (handshake only), disconnects, waits GAP seconds
with no client, and connects again. On an affected host (macOS 27.0.1, tart
2.40.1) the second connection fails and tart aborts when GAP is 60 or more.
With --hold, a bare TCP connection stays open for the whole run and the
second connection succeeds.

Uses no TestAnyware code. Needs tart, python3 and /usr/bin/openssl (LibreSSL
supplies the single-DES that VNC authentication needs). Always stops and
deletes its clone.

Exit status: 0 = the second connection worked, 1 = it failed and tart stopped.
See docs/research/tart-vnc-reconnect-crash-macos-27.md.
"""

import argparse
import re
import socket
import struct
import subprocess
import sys
import tempfile
import time
from pathlib import Path


def read_exactly(sock: socket.socket, n: int) -> bytes:
    buf = b""
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise EOFError(f"connection closed after {len(buf)} of {n} bytes")
        buf += chunk
    return buf


def vnc_auth_response(password: str, challenge: bytes) -> bytes:
    # RFB VNC authentication: single DES-ECB with each key byte bit-reversed.
    key = bytes(int(f"{b:08b}"[::-1], 2) for b in password.encode()[:8].ljust(8, b"\0"))
    return subprocess.run(
        ["/usr/bin/openssl", "enc", "-des-ecb", "-nopad", "-K", key.hex()],
        input=challenge,
        capture_output=True,
        check=True,
    ).stdout


def rfb_session(host: str, port: int, password: str) -> tuple[int, int]:
    """Handshake, authenticate, ClientInit, ServerInit, disconnect cleanly. Returns (w, h)."""
    with socket.create_connection((host, port), timeout=15) as sock:
        read_exactly(sock, 12)
        sock.sendall(b"RFB 003.008\n")
        count = read_exactly(sock, 1)[0]
        types = list(read_exactly(sock, count))
        if 2 not in types:
            raise RuntimeError(f"server offered no VNC authentication: {types}")
        sock.sendall(bytes([2]))
        sock.sendall(vnc_auth_response(password, read_exactly(sock, 16)))
        if struct.unpack(">I", read_exactly(sock, 4))[0] != 0:
            raise RuntimeError("authentication failed")
        sock.sendall(b"\x01")
        width, height = struct.unpack(">HH", read_exactly(sock, 4))
        read_exactly(sock, 16)
        read_exactly(sock, struct.unpack(">I", read_exactly(sock, 4))[0])
        return width, height


def tart_state(vm: str) -> str:
    listing = subprocess.run(["tart", "list"], capture_output=True, text=True).stdout
    for line in listing.splitlines():
        fields = line.split()
        if len(fields) >= 2 and fields[1] == vm:
            return fields[-1]
    return "absent"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--base", default="testanyware-golden-macos-27", help="guest to clone")
    parser.add_argument("--gap", type=float, default=70, help="seconds with no client between the two connections")
    parser.add_argument("--settle", type=float, default=30, help="seconds to let the guest boot before the first connection")
    parser.add_argument("--hold", action="store_true", help="keep a bare TCP connection open for the whole run")
    args = parser.parse_args()

    vm = f"vnc-idle-repro-{int(time.time())}"
    log = Path(tempfile.mkdtemp()) / "tart.log"
    subprocess.run(["tart", "clone", args.base, vm], check=True)
    tart = subprocess.Popen(
        ["tart", "run", vm, "--no-graphics", "--vnc-experimental"],
        stdout=log.open("w"),
        stderr=subprocess.STDOUT,
    )
    try:
        url = None
        for _ in range(60):
            url = re.search(r"vnc://:?([^@]*)@([\d.]+):(\d+)", log.read_text())
            if url:
                break
            time.sleep(1)
        if not url:
            print("tart printed no VNC URL", file=sys.stderr)
            return 2
        password, host, port = url.group(1), url.group(2), int(url.group(3))

        time.sleep(args.settle)
        holder = socket.create_connection((host, port)) if args.hold else None
        print(f"first connection: {rfb_session(host, port, password)}")
        print(f"waiting {args.gap:g} s with {'a bare TCP holder' if holder else 'no client'}")
        time.sleep(args.gap)
        try:
            print(f"second connection: {rfb_session(host, port, password)}")
            print("RESULT: no crash")
            return 0
        except (EOFError, OSError) as err:
            time.sleep(2)
            print(f"second connection failed: {err}; tart state: {tart_state(vm)}")
            print("RESULT: crash")
            return 1
    finally:
        tart.terminate()
        subprocess.run(["tart", "stop", vm], capture_output=True)
        time.sleep(2)
        subprocess.run(["tart", "delete", vm], capture_output=True)


if __name__ == "__main__":
    sys.exit(main())
