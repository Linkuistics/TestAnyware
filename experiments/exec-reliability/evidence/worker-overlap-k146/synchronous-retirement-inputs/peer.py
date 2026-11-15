"""Local fake public command peer; never touches a VM or a real app."""

import signal
import subprocess
import sys
import time
import uuid
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
    elif mode in {"scope-child", "stubborn-child", "held-child"}:
        if mode == "stubborn-child":
            signal.signal(signal.SIGTERM, signal.SIG_IGN)
        release, ready = map(Path, args)
        if mode != "held-child":
            ready.write_bytes(b"ready")
        end = time.monotonic() + 5
        while not release.exists() and time.monotonic() < end:
            time.sleep(0.005)
    elif mode == "scope-parent":
        release, ready, *child_mode = args
        stopping = False

        def stop(_signal: int, _frame: object) -> None:
            nonlocal stopping
            stopping = True

        signal.signal(signal.SIGTERM, stop)
        child = subprocess.Popen(
            [
                sys.executable,
                __file__,
                child_mode[0] if child_mode else "scope-child",
                release,
                ready,
            ]
        )
        # Retain the exact child identity even if TERM arrives before readiness.
        declaration = Path(ready).with_suffix(".spawned.tmp")
        declaration.write_text(str(child.pid))
        declaration.replace(Path(ready).with_suffix(".spawned"))
        end = time.monotonic() + 4
        while not stopping and not Path(ready).exists() and time.monotonic() < end:
            time.sleep(0.005)
        print(child.pid, flush=True)
        while not stopping and time.monotonic() < end:
            time.sleep(0.005)
        # PID-only TERM deliberately leaves the child to the independent test
        # owner's release marker. Group TERM lets this parent reap the child.
        try:
            child.wait(timeout=0.03)
        except subprocess.TimeoutExpired:
            pass
    elif mode == "fork":
        release, ready = map(Path, args)
        child_ready = ready.with_suffix(".child")
        stopping = False

        def terminate(_signal: int, _frame: object) -> None:
            nonlocal stopping
            stopping = True

        signal.signal(signal.SIGTERM, terminate)
        child = subprocess.Popen(
            [sys.executable, __file__, "scope-child", str(release), str(child_ready)]
        )
        # Publish identity before stdout, which may be closed or unreadable.
        ready.write_text(str(child.pid))
        end = time.monotonic() + 4
        while not child_ready.exists() and time.monotonic() < end:
            time.sleep(0.005)
        print(child.pid, flush=True)
        while not stopping and not release.exists() and time.monotonic() < end:
            time.sleep(0.005)
        # Never re-enter Popen.wait from a handler interrupting another wait.
        child.wait(timeout=1)
    elif mode == "fail":
        print("forced public interface failure", file=sys.stderr)
        sys.exit(23)
    elif mode == "controller":
        from controller import Call, Plan, execute

        plan = Plan(
            uuid.uuid4().hex,
            (
                Call(
                    "interrupt",
                    (sys.executable, __file__, "ignore-term"),
                    1,
                    -9,
                    interrupt_after=0.2,
                    expected_reason="interrupted",
                ),
                Call("after", (sys.executable, __file__, "bytes"), 1),
            ),
            subjects=(Path(__file__).resolve(),),
            term_grace=0.15,
            kill_wait=0.3,
        )
        print(execute(plan, Path(args[0])), flush=True)
        print("controller continued", flush=True)
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
