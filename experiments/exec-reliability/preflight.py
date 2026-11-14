"""Finite read-only host preflight. No guest requests, launches or installation."""

import shutil
import sys
import uuid
from dataclasses import asdict
from pathlib import Path

from controller import Call, Plan, execute, write_json

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]


def main() -> None:
    output = Path(sys.argv[1]).resolve()
    output.mkdir(parents=True, exist_ok=False)
    cli = shutil.which("testanyware")
    tart = shutil.which("tart")
    jj = shutil.which("jj")
    if cli is None or tart is None or jj is None:
        raise SystemExit("preflight unavailable: testanyware, tart and jj must be on PATH")
    app = Path("/Applications/TestAnyware.app/Contents/MacOS/testanyware-app")
    socket = Path.home() / "Library/Application Support/TestAnyware/testanyware.sock"
    commands: list[tuple[str, tuple[str, ...], float]] = [
        ("cli-version", (cli, "--version"), 10),
        ("cli-help", (cli, "--help"), 10),
        ("agent-help", (cli, "agent", "--help"), 10),
        ("file-help", (cli, "file", "--help"), 10),
        ("exec-help", (cli, "file", "exec", "--help"), 10),
        ("upload-help", (cli, "file", "upload", "--help"), 10),
        ("download-help", (cli, "file", "download", "--help"), 10),
        ("golden-help", (cli, "vm", "create-golden", "--help"), 10),
        ("start-help", (cli, "vm", "start", "--help"), 10),
        ("stop-help", (cli, "vm", "stop", "--help"), 10),
        ("delete-help", (cli, "vm", "delete", "--help"), 10),
        ("vm-list", (cli, "vm", "list", "--all", "--json"), 60),
        ("doctor", (cli, "doctor", "--json"), 60),
        (
            "app-status",
            (
                "/usr/bin/curl",
                "--silent",
                "--show-error",
                "--include",
                "--max-time",
                "10",
                "--unix-socket",
                str(socket),
                "http://localhost/v1/status",
            ),
            15,
        ),
        ("socket-holder", ("/usr/sbin/lsof", "-n", "-a", "-U", str(socket)), 10),
        ("tart-version", (tart, "--version"), 10),
        ("tart-list", (tart, "list", "--format", "json"), 60),
        ("app-signature", ("/usr/bin/codesign", "-d", "--verbose=4", str(app)), 10),
        ("app-uuid", ("/usr/bin/dwarfdump", "--uuid", str(app)), 10),
        ("host-version", ("/usr/bin/sw_vers",), 10),
        (
            "parent-source",
            (jj, "log", "--no-pager", "-r", "@-", "--no-graph", "-T", "commit_id"),
            10,
        ),
        ("source-diff", (jj, "diff", "--no-pager", "--git"), 10),
    ]
    subjects = (
        Path(__file__),
        HERE / "controller.py",
        HERE / "inspection.py",
        HERE / "peer.py",
        HERE / "test_controller.py",
        HERE / "test_inspection.py",
        HERE / "pyproject.toml",
        HERE / "uv.lock",
        REPO / "Taskfile.yml",
        REPO / "cli-rs/Cargo.toml",
        REPO / "cli-rs/Cargo.lock",
        Path(cli).resolve(),
        Path(tart).resolve(),
        app,
        app.parent.parent / "Info.plist",
    )
    # Every independent condition is frozen before the first one runs. A failed
    # condition is retained; continuing the inventory is not retrying that call.
    plans = tuple(
        Plan(
            uuid.uuid4().hex,
            (Call(name, argv, budget),),
            subjects=subjects,
        )
        for name, argv, budget in commands
    )
    write_json(output / "inventory.json", [asdict(plan) for plan in plans])
    write_json(
        output / "paths.json",
        {
            str(path): path.exists()
            for path in (
                REPO / "cli-rs/target/debug/testanyware",
                REPO / "cli-rs/target/debug/testanyware-app",
                REPO / "cli-rs/target/app/dev/TestAnyware Dev.app",
                REPO / "agents/macos/.build/release/testanyware-agent",
                app,
                socket,
            )
        },
    )
    for plan in plans:
        path = execute(plan, output)
        print(f"{plan.calls[0].name}: {path}", flush=True)


if __name__ == "__main__":
    main()
