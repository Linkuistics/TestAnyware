#!/usr/bin/env python3
"""Send one input command straight to the VNC server of a tart guest, with no TestAnyware code.

The control of the input live checks (scripts/input-live.sh). It connects a bare RFB client to the
loopback endpoint that tart serves, authenticates, sends the events of one command, and closes
the connection in the way that --close names. The app is not in the path, so a command that has
no effect here and an effect through the app (or the reverse) shows where the fault lies.

  tart-vnc-input-control.py <host> <port> <password> [options] click X Y
  tart-vnc-input-control.py <host> <port> <password> [options] type TEXT
  tart-vnc-input-control.py <host> <port> <password> [options] scroll X Y DY
  tart-vnc-input-control.py <host> <port> <password> [options] drag X1 Y1 X2 Y2

Options:
  --close ordinary   close the socket the usual way (FIN). The default.
  --close reset      close it with a reset (RST), as the app does at the end of a relay.
  --linger SECS      stay connected this long after the last event, before the close.
  --handshake-only   do the full handshake, send no event, and close. Shows the handshake works.
  --like-cli         send what the TestAnyware client sends after ServerInit, before its events:
                     SetPixelFormat (RGBA32) and SetEncodings (ZRLE, Tight, CopyRect, Raw,
                     DesktopSize, LastRect). A SetEncodings without DesktopSize makes tart stop
                     the guest, so this option never sends a shorter list.
  --gap SECS         wait this long between two events (default 0.02).
  --burst            collect the events of the command and send them in one write, then close at
                     once. The app relays a command in this way: it reads the client's bytes
                     and writes them to the guest together, and it resets when the client is gone.

The host, port and password are the `vnc` object of the connection spec of the guest.
Needs /usr/bin/openssl (LibreSSL supplies the single-DES that VNC authentication needs).
"""

import argparse
import socket
import struct
import subprocess
import sys
import time

# X11 keysyms for the keys that `type` needs besides the printable ASCII range.
SHIFT_L = 0xFFE1


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


def handshake(sock: socket.socket, password: str) -> tuple[int, int]:
    read_exactly(sock, 12)
    sock.sendall(b"RFB 003.008\n")
    count = read_exactly(sock, 1)[0]
    types = list(read_exactly(sock, count))
    if 2 not in types:
        raise RuntimeError(f"the server offered no VNC authentication: {types}")
    sock.sendall(bytes([2]))
    sock.sendall(vnc_auth_response(password, read_exactly(sock, 16)))
    if struct.unpack(">I", read_exactly(sock, 4))[0] != 0:
        raise RuntimeError("authentication failed")
    sock.sendall(b"\x01")
    width, height = struct.unpack(">HH", read_exactly(sock, 4))
    read_exactly(sock, 16)
    read_exactly(sock, struct.unpack(">I", read_exactly(sock, 4))[0])
    return width, height


def like_cli(sock: socket.socket) -> None:
    # SetPixelFormat: the layout of PixelFormat::rgba32_le in testanyware-rfb.
    pixel_format = struct.pack(">BBBBHHHBBBxxx", 32, 24, 0, 1, 255, 255, 255, 16, 8, 0)
    sock.sendall(struct.pack(">Bxxx", 0) + pixel_format)
    # SetEncodings: encoding_preferences() in testanyware-rfb, in its order.
    encodings = [16, 7, 1, 0, -223, -224]
    sock.sendall(struct.pack(">BxH", 2, len(encodings)) + struct.pack(f">{len(encodings)}i", *encodings))


def pointer(sock: socket.socket, buttons: int, x: int, y: int) -> None:
    sock.sendall(struct.pack(">BBHH", 5, buttons, x, y))


def key(sock: socket.socket, down: bool, keysym: int) -> None:
    sock.sendall(struct.pack(">BBxxI", 4, int(down), keysym))


def keysym_of(char: str) -> tuple[int, bool]:
    """The keysym of one printable ASCII character, and whether it needs Shift."""
    code = ord(char)
    if not 0x20 <= code < 0x7F:
        raise ValueError(f"cannot type {char!r}: only printable ASCII")
    return code, char.isupper() or char in '~!@#$%^&*()_+{}|:"<>?'


class Burst:
    """Collects what the events write, so that one write sends the whole command."""

    def __init__(self) -> None:
        self.data = b""

    def sendall(self, data: bytes) -> None:
        self.data += data


def run(sock, args: argparse.Namespace) -> None:
    gap = 0 if args.burst else args.gap
    cmd = args.command
    if cmd == "click":
        x, y = int(args.rest[0]), int(args.rest[1])
        pointer(sock, 0, x, y)
        time.sleep(gap)
        pointer(sock, 1, x, y)
        time.sleep(gap)
        pointer(sock, 0, x, y)
    elif cmd == "type":
        for char in args.rest[0]:
            sym, shifted = keysym_of(char)
            if shifted:
                key(sock, True, SHIFT_L)
            key(sock, True, sym)
            time.sleep(gap)
            key(sock, False, sym)
            if shifted:
                key(sock, False, SHIFT_L)
            time.sleep(gap)
    elif cmd == "scroll":
        x, y, dy = int(args.rest[0]), int(args.rest[1]), int(args.rest[2])
        # Wheel up is button 4 (mask 8) and wheel down is button 5 (mask 16), one click per step.
        mask = 8 if dy < 0 else 16
        pointer(sock, 0, x, y)
        for _ in range(abs(dy)):
            pointer(sock, mask, x, y)
            time.sleep(gap)
            pointer(sock, 0, x, y)
            time.sleep(gap)
    elif cmd == "drag":
        x1, y1, x2, y2 = (int(v) for v in args.rest[:4])
        pointer(sock, 0, x1, y1)
        time.sleep(gap)
        pointer(sock, 1, x1, y1)
        time.sleep(gap)
        steps = 10
        for i in range(1, steps + 1):
            pointer(sock, 1, x1 + (x2 - x1) * i // steps, y1 + (y2 - y1) * i // steps)
            time.sleep(gap)
        pointer(sock, 0, x2, y2)
    else:
        raise ValueError(f"unknown command {cmd!r}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("host")
    parser.add_argument("port", type=int)
    parser.add_argument("password")
    parser.add_argument("--close", choices=["ordinary", "reset"], default="ordinary")
    parser.add_argument("--linger", type=float, default=0)
    parser.add_argument("--handshake-only", action="store_true")
    parser.add_argument("--like-cli", action="store_true")
    parser.add_argument("--gap", type=float, default=0.02)
    parser.add_argument("--burst", action="store_true")
    parser.add_argument("command", nargs="?", choices=["click", "type", "scroll", "drag"])
    parser.add_argument("rest", nargs="*")
    args = parser.parse_args()
    if not args.handshake_only and args.command is None:
        parser.error("name a command, or pass --handshake-only")

    sock = socket.create_connection((args.host, args.port), timeout=15)
    try:
        width, height = handshake(sock, args.password)
        print(f"handshake ok: {width}x{height}")
        if args.like_cli:
            like_cli(sock)
        if not args.handshake_only:
            if args.burst:
                burst = Burst()
                run(burst, args)
                sock.sendall(burst.data)
            else:
                run(sock, args)
            print(f"sent {args.command} {' '.join(args.rest)}")
        if args.linger:
            time.sleep(args.linger)
    finally:
        if args.close == "reset":
            # SO_LINGER with a zero timeout makes close() send a reset and drop unsent bytes.
            sock.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
        sock.close()
        print(f"closed ({args.close})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
