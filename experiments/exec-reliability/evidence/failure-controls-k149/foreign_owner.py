"""Separately authorised owner for the real UID-denial control (k149 control 2).

A human runs this script with sudo. The diagnostic sender never runs it and
never acquires privilege. The owner forks one fixture. The fixture starts a new
session, drops all groups, and sets its real, effective and saved IDs to nobody.
It then runs /bin/sleep. The owner publishes the fixture identity and waits for
the sender's done marker. It then sends SIGKILL to its own child and reaps that
exact PID. The owner writes a cleanup receipt and gives every file it wrote to
the invoking user.

The script targets the macOS system Python 3.9, so it avoids newer syntax.
"""

from __future__ import annotations

import argparse
import json
import os
import pwd
import signal
import subprocess
import sys
import time

FIXTURE_USER = "nobody"
WAIT_SECONDS = 900.0


def identity(pid: int) -> dict[str, object]:
    result = subprocess.run(
        ["/bin/ps", "-o", "pid=,ppid=,pgid=,uid=,ruid=,svuid=,stat=,lstart=,comm=", "-p", str(pid)],
        capture_output=True,
        check=False,
    )
    return {"returncode": result.returncode, "row": result.stdout.decode().strip()}


def publish(path: str, value: object, owner_uid: int, owner_gid: int) -> None:
    temporary = path + ".tmp"
    with open(temporary, "w") as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")
    os.chown(temporary, owner_uid, owner_gid)
    os.replace(temporary, path)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("out")
    args = parser.parse_args()
    if os.geteuid() != 0:
        sys.exit("run this owner script with sudo")
    sender_uid = int(os.environ["SUDO_UID"])
    sender_gid = int(os.environ["SUDO_GID"])
    fixture = pwd.getpwnam(FIXTURE_USER)
    out = os.path.abspath(args.out)
    os.makedirs(out)
    os.chown(out, sender_uid, sender_gid)
    receipt_path = os.path.join(out, "owner-receipt.json")
    done_path = os.path.join(out, "sender-done")
    cleanup_path = os.path.join(out, "owner-cleanup.json")

    created = time.monotonic()
    pid = os.fork()
    if pid == 0:
        try:
            os.setsid()
            os.setgroups([])
            os.setgid(fixture.pw_gid)
            os.setuid(fixture.pw_uid)
            os.execv("/bin/sleep", ["/bin/sleep", str(int(WAIT_SECONDS) + 60)])
        finally:
            os._exit(127)

    time.sleep(0.2)
    publish(
        receipt_path,
        {
            "owner_pid": os.getpid(),
            "owner_euid": os.geteuid(),
            "sender_uid": sender_uid,
            "fixture_user": FIXTURE_USER,
            "fixture_uid": fixture.pw_uid,
            "fixture_gid": fixture.pw_gid,
            "fixture_pid": pid,
            "fixture_created_monotonic": created,
            "fixture_identity": identity(pid),
            "python": sys.version,
        },
        sender_uid,
        sender_gid,
    )
    print(f"fixture pid {pid} is ready; run the sender control now", flush=True)

    deadline = time.monotonic() + WAIT_SECONDS
    while not os.path.exists(done_path) and time.monotonic() < deadline:
        time.sleep(0.1)
    marker_seen = os.path.exists(done_path)
    before_kill = identity(pid)
    kill_error = None
    try:
        os.kill(pid, signal.SIGKILL)
    except OSError as error:
        kill_error = f"{error.errno}: {error.strerror}"
    waited_pid, status = os.waitpid(pid, 0)
    try:
        os.waitpid(-1, os.WNOHANG)
        remaining_children = True
    except ChildProcessError:
        remaining_children = False
    publish(
        cleanup_path,
        {
            "marker_seen": marker_seen,
            "identity_before_kill": before_kill,
            "kill_error": kill_error,
            "waited_pid": waited_pid,
            "wait_status": status,
            "signalled": os.WIFSIGNALED(status),
            "terminating_signal": os.WTERMSIG(status) if os.WIFSIGNALED(status) else None,
            "identity_after_reap": identity(pid),
            "owner_has_children_after": remaining_children,
            "finished_monotonic": time.monotonic(),
        },
        sender_uid,
        sender_gid,
    )
    print(f"fixture pid {pid} killed and reaped; cleanup receipt written", flush=True)


if __name__ == "__main__":
    main()
