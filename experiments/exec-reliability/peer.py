"""Local fake public command peer; never touches a VM or a real app."""

import signal
import subprocess
import sys
import time
from pathlib import Path


def main() -> None:
    mode, *args = sys.argv[1:]
    if mode == "bytes":
        sys.stdout.buffer.write(bytes.fromhex("00ff80410a"))
        sys.stderr.buffer.write(b"separate stderr\n")
    elif mode == "hang":
        print("partial", flush=True)
        time.sleep(60)
    elif mode == "ignore-term":
        signal.signal(signal.SIGTERM, signal.SIG_IGN)
        print("ready", flush=True)
        time.sleep(60)
    elif mode == "fork":
        child = subprocess.Popen(["/bin/sleep", "60"])

        def terminate(_signal: int, _frame: object) -> None:
            child.wait(timeout=1)
            sys.exit(0)

        signal.signal(signal.SIGTERM, terminate)
        print(child.pid, flush=True)
        child.wait()
    elif mode == "fail":
        print("forced public interface failure", file=sys.stderr)
        sys.exit(23)
    elif mode == "drift":
        Path(args[0]).write_bytes(b"changed")
    elif mode in {"acquire", "dispose", "inspect"}:
        identity, root = args
        marker = Path(root) / identity
        if mode == "acquire":
            marker.write_bytes(b"owned")
        elif mode == "dispose":
            marker.unlink()
        elif not marker.exists():
            print("absent")
        else:
            sys.exit(1)
    else:
        raise ValueError(mode)


if __name__ == "__main__":
    main()
