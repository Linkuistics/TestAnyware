"""Local fake public command peer; never touches a VM or a real app."""

import os
import signal
import subprocess
import sys
import time
import uuid
from pathlib import Path


def stamp(event: str, **fields: object) -> None:
    """Record one monotonic event when a k152 soak asks for peer timing.

    Each process appends only to its own file, so every file has one writer.
    Without PEER_STAMPS the peer behaves exactly as before.
    """
    directory = os.environ.get("PEER_STAMPS")
    if not directory:
        return
    import json  # Imported only here, so unstamped peers keep their start cost.

    line = {"event": event, "pid": os.getpid(), "monotonic": time.monotonic(), **fields}
    fd = os.open(
        Path(directory) / f"{os.getpid()}.jsonl", os.O_WRONLY | os.O_CREAT | os.O_APPEND, 0o644
    )
    try:
        os.write(fd, json.dumps(line).encode() + b"\n")
    finally:
        os.close(fd)


def main() -> None:
    mode, *args = sys.argv[1:]
    stamp("start", mode=mode, ppid=os.getppid())
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
            stamp("ready_written")
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
        stamp("handler_installed")
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
        stamp("child_spawned", child=child.pid)
        end = time.monotonic() + 4
        while not stopping and not Path(ready).exists() and time.monotonic() < end:
            time.sleep(0.005)
        stamp("readiness_loop_left", ready=Path(ready).exists(), stopping=stopping)
        print(child.pid, flush=True)
        stamp("pid_printed")
        while not stopping and time.monotonic() < end:
            time.sleep(0.005)
        stamp("stop_loop_left", stopping=stopping)
        # PID-only TERM deliberately leaves the child to the independent test
        # owner's release marker. Group TERM lets this parent reap the child.
        try:
            child.wait(timeout=0.03)
        except subprocess.TimeoutExpired:
            pass
        stamp("child_wait_left", returncode=child.returncode)
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
    elif mode == "late-writer":
        # The leader exits at once. Its declared child keeps the inherited
        # stdout descriptor and appends only after the release marker.
        release, declaration = map(Path, args)
        child = subprocess.Popen([sys.executable, __file__, "late-child", str(release)])
        pending = declaration.with_suffix(".tmp")
        pending.write_text(str(child.pid))
        pending.replace(declaration)
        sys.stdout.buffer.write(b"early\n")
        sys.stdout.flush()
    elif mode == "late-child":
        release = Path(args[0])
        end = time.monotonic() + 5
        while not release.exists() and time.monotonic() < end:
            time.sleep(0.005)
        if release.exists():
            sys.stdout.buffer.write(b"late\n")
            sys.stdout.flush()
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
