"""Controls for scripts/vm_inventory.py against fake peers (leaf diagnostic-routes-k19).

Each fake is an executable that prints a fixture and exits with a fixture code. A control
passes only when the checker refuses what the fixture breaks.
"""

import importlib.util
import json
import shutil
import stat
import subprocess
import sys
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts" / "vm_inventory.py"
_spec = importlib.util.spec_from_file_location("vm_inventory", SCRIPT)
assert _spec is not None and _spec.loader is not None
vm_inventory = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(vm_inventory)

NAME = "k19-linux-0123456789abcdef0123456789abcdef"


def envelope(*names: str, **overrides: object) -> dict[str, object]:
    items = [{"kind": "running", "name": n, "platform": "linux", "backend": "tart"} for n in names]
    items.insert(
        0,
        {
            "kind": "golden",
            "name": "testanyware-golden-linux-24.04",
            "platform": "linux",
            "backend": "tart",
        },
    )
    value: dict[str, object] = {
        "schema_version": "1.0",
        "ok": True,
        "items": items,
        "returned": len(items),
        "total": len(items),
        "truncated": False,
    }
    value.update(overrides)
    return value


def tart_items(*names: str) -> list[dict[str, object]]:
    return [{"Name": n, "Running": True, "State": "running", "Source": "local"} for n in names]


def fake(tmp_path: Path, label: str, replies: dict[str, tuple[int, object, float]]) -> str:
    """An executable that answers by its first two arguments with (exit, stdout, sleep)."""
    table = {
        key: {
            "exit": code,
            "stdout": out if isinstance(out, str) else json.dumps(out),
            "sleep": pause,
        }
        for key, (code, out, pause) in replies.items()
    }
    data = tmp_path / f"{label}.json"
    data.write_text(json.dumps(table))
    path = tmp_path / label
    path.write_text(
        f"#!{sys.executable}\n"
        "import json, sys, time\n"
        f"table = json.load(open({str(data)!r}))\n"
        "reply = table.get(' '.join(sys.argv[1:3]))\n"
        "if reply is None:\n"
        "    sys.stderr.write('unexpected ' + repr(sys.argv))\n"
        "    sys.exit(99)\n"
        "time.sleep(reply['sleep'])\n"
        "sys.stdout.write(reply['stdout'])\n"
        "sys.exit(reply['exit'])\n"
    )
    path.chmod(path.stat().st_mode | stat.S_IXUSR)
    return str(path)


def check(
    tmp_path: Path,
    verb: str,
    vm: object = None,
    tart: object = None,
    stop: tuple[int, object, float] | None = None,
    vm_exit: int = 0,
    tart_exit: int = 0,
    timeout: float = 5,
) -> subprocess.CompletedProcess[str]:
    cli_replies: dict[str, tuple[int, object, float]] = {
        "vm list": (vm_exit, envelope() if vm is None else vm, 0)
    }
    if stop is not None:
        cli_replies["vm stop"] = stop
    cli = fake(tmp_path, "testanyware", cli_replies)
    hyper = fake(
        tmp_path, "tart", {"list --format": (tart_exit, tart_items() if tart is None else tart, 0)}
    )
    argv = [
        sys.executable,
        str(SCRIPT),
        verb,
        NAME,
        "--cli",
        cli,
        "--tart",
        hyper,
        "--timeout",
        str(timeout),
    ]
    if verb == "dispose":
        argv += ["--stop-timeout", str(timeout)]
    return subprocess.run(argv, capture_output=True, text=True, timeout=30)


# ---- absence ---------------------------------------------------------------------------------


def test_absent_when_both_inventories_parse_and_omit_the_name(tmp_path: Path) -> None:
    done = check(tmp_path, "absent", tart=tart_items("testanyware-golden-linux-24.04"))
    assert (done.returncode, done.stdout) == (0, f"absent {NAME}\n")


def test_a_name_that_differs_by_a_suffix_is_not_the_name(tmp_path: Path) -> None:
    done = check(tmp_path, "absent", vm=envelope(NAME + "-x"), tart=tart_items(NAME[:-1]))
    assert (done.returncode, done.stdout) == (0, f"absent {NAME}\n")


@pytest.mark.parametrize(
    ("vm", "tart"),
    [
        (envelope(NAME), tart_items()),
        (envelope(), tart_items(NAME)),
        (envelope(NAME), tart_items(NAME)),
    ],
    ids=["public-only", "hypervisor-only", "both"],
)
def test_present_in_either_inventory(tmp_path: Path, vm: object, tart: object) -> None:
    done = check(tmp_path, "absent", vm=vm, tart=tart)
    assert done.returncode == 1 and done.stdout == ""
    assert NAME in done.stderr


BROKEN_VM: dict[str, object] = {
    "not-json": "{",
    "not-object": [],
    "error-envelope": {"schema_version": "1.0", "ok": False, "code": "APP_UNREACHABLE"},
    "other-schema": envelope(schema_version="2.0"),
    "no-items": envelope(items=None),
    "truncated": envelope(truncated=True),
    "truncation-unstated": {k: v for k, v in envelope().items() if k != "truncated"},
    "count-mismatch": envelope(total=7),
    "nameless-item": envelope(
        items=[{"kind": "running", "platform": "linux", "backend": "tart"}], returned=1, total=1
    ),
    "empty-name": envelope(
        items=[{"kind": "running", "name": "", "backend": "tart"}], returned=1, total=1
    ),
    "unknown-kind": envelope(
        items=[{"kind": "ghost", "name": "a", "backend": "tart"}], returned=1, total=1
    ),
    "unknown-backend": envelope(
        items=[{"kind": "running", "name": "a", "backend": "vz"}], returned=1, total=1
    ),
}


@pytest.mark.parametrize("broken", sorted(BROKEN_VM))
def test_a_malformed_public_inventory_is_unreadable(tmp_path: Path, broken: str) -> None:
    done = check(tmp_path, "absent", vm=BROKEN_VM[broken])
    assert done.returncode == 2 and done.stdout == ""
    assert "absence unknown" in done.stderr


BROKEN_TART: dict[str, object] = {
    "not-json": "[",
    "object": {"Name": NAME},
    "nameless-item": [{"Running": False}],
    "non-string-name": [{"Name": 7}],
    "item-not-object": ["testanyware-golden-linux-24.04"],
}


@pytest.mark.parametrize("broken", sorted(BROKEN_TART))
def test_a_malformed_hypervisor_inventory_is_unreadable(tmp_path: Path, broken: str) -> None:
    done = check(tmp_path, "absent", tart=BROKEN_TART[broken])
    assert done.returncode == 2 and done.stdout == ""


def test_a_failing_listing_is_unreadable_even_when_it_prints_an_empty_inventory(
    tmp_path: Path,
) -> None:
    assert check(tmp_path, "absent", vm_exit=1).returncode == 2
    assert check(tmp_path, "absent", tart_exit=1).returncode == 2


def test_a_listing_that_cannot_run_is_unreadable(tmp_path: Path) -> None:
    missing = str(tmp_path / "missing")
    done = subprocess.run(
        [sys.executable, str(SCRIPT), "absent", NAME, "--cli", missing, "--tart", missing],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert done.returncode == 2 and "could not run" in done.stderr


def test_a_listing_that_does_not_finish_is_unreadable(tmp_path: Path) -> None:
    cli = fake(tmp_path, "testanyware", {"vm list": (0, envelope(), 5)})
    hyper = fake(tmp_path, "tart", {"list --format": (0, tart_items(), 0)})
    done = subprocess.run(
        [
            sys.executable,
            str(SCRIPT),
            "absent",
            NAME,
            "--cli",
            cli,
            "--tart",
            hyper,
            "--timeout",
            "0.5",
        ],
        capture_output=True,
        text=True,
        timeout=30,
    )
    assert done.returncode == 2 and "did not finish" in done.stderr


def test_an_unreadable_hypervisor_inventory_hides_no_present_public_entry(tmp_path: Path) -> None:
    # Both inventories are read before either answer counts: a reader that stopped at the first
    # inventory would report "present" here and hide the inspection failure.
    done = check(tmp_path, "absent", vm=envelope(NAME), tart="[")
    assert done.returncode == 2


# ---- disposal --------------------------------------------------------------------------------

STOPPED = (0, {"schema_version": "1.0", "ok": True, "id": NAME}, 0.0)
NOT_FOUND = (3, {"schema_version": "1.0", "ok": False, "code": "VM_NOT_FOUND", "message": "m"}, 0.0)


def test_a_proved_stop_and_absence_dispose(tmp_path: Path) -> None:
    done = check(tmp_path, "dispose", stop=STOPPED)
    assert (done.returncode, done.stdout) == (0, f"disposed {NAME} stopped\n")


def test_a_proved_vm_not_found_and_absence_dispose_an_already_absent_vm(tmp_path: Path) -> None:
    done = check(tmp_path, "dispose", stop=NOT_FOUND)
    assert (done.returncode, done.stdout) == (0, f"disposed {NAME} already-absent\n")


REFUSED_STOPS: dict[str, tuple[int, object, float]] = {
    "golden-not-found-exit-3": (
        3,
        {"schema_version": "1.0", "ok": False, "code": "GOLDEN_NOT_FOUND"},
        0.0,
    ),
    "bare-exit-3": (3, "", 0.0),
    "text-exit-3": (3, "error: no such VM", 0.0),
    "vm-not-found-other-exit": (1, NOT_FOUND[1], 0.0),
    "ok-envelope-exit-3": (3, STOPPED[1], 0.0),
    "stop-failed": (1, {"schema_version": "1.0", "ok": False, "code": "VM_STOP_FAILED"}, 0.0),
    "exit-0-error-envelope": (0, NOT_FOUND[1], 0.0),
    "exit-0-not-json": (0, "stopped", 0.0),
    "other-schema": (3, {"schema_version": "2.0", "ok": False, "code": "VM_NOT_FOUND"}, 0.0),
    "stop-hangs": (0, STOPPED[1], 5.0),
}


@pytest.mark.parametrize("stop", sorted(REFUSED_STOPS))
def test_disposal_refuses_any_other_stop_result(tmp_path: Path, stop: str) -> None:
    done = check(tmp_path, "dispose", stop=REFUSED_STOPS[stop], timeout=1)
    assert done.returncode == 3 and done.stdout == ""
    assert f"tart stop {NAME}" in done.stderr.replace(str(tmp_path / "tart"), "tart")


@pytest.mark.parametrize("stop", [STOPPED, NOT_FOUND], ids=["stopped", "not-found"])
def test_disposal_needs_independent_absence(
    tmp_path: Path, stop: tuple[int, object, float]
) -> None:
    still_listed = check(tmp_path, "dispose", stop=stop, tart=tart_items(NAME))
    unreadable = check(tmp_path, "dispose", stop=stop, vm="{")
    for done in (still_listed, unreadable):
        assert done.returncode == 3 and done.stdout == ""
        assert "remedy:" in done.stderr


def test_the_remedy_never_repeats_only_the_failing_command(tmp_path: Path) -> None:
    done = check(tmp_path, "dispose", stop=REFUSED_STOPS["stop-failed"])
    remedy = done.stderr.splitlines()[-1]
    assert (
        "list --format json" in remedy
        and "delete" in remedy
        and "Never pass a golden name" in remedy
    )


def test_the_module_rejects_a_blank_name() -> None:
    assert vm_inventory.main(["absent", ""]) == 2


# ---- the golden-name guard of the Taskfile ---------------------------------------------------

REPO = SCRIPT.parents[1]
GOLDEN = "testanyware-golden-linux-24.04"


def guard(
    tmp_path: Path, vm: object, tart: object, *, cli: str | None = None, vm_exit: int = 0
) -> int:
    task = shutil.which("task")
    assert task is not None, "run the controls through `mise exec -- task`, which puts task on PATH"
    fake_cli = cli or fake(tmp_path, "testanyware", {"vm list": (vm_exit, vm, 0)})
    fake_tart = fake(tmp_path, "tart", {"list --format": (0, tart, 0)})
    done = subprocess.run(
        [
            task,
            "-d",
            str(REPO),
            "vm:golden-name-free",
            f"NAME={GOLDEN}",
            f"GUARD_CLI={fake_cli}",
            f"GUARD_TART={fake_tart}",
        ],
        capture_output=True,
        text=True,
        timeout=60,
    )
    return done.returncode


def test_the_guard_admits_a_name_that_neither_inventory_lists(tmp_path: Path) -> None:
    vm = envelope(items=[], returned=0, total=0)
    assert guard(tmp_path, vm, tart_items("testanyware-golden-macos-27")) == 0


@pytest.mark.parametrize(
    ("vm", "tart"),
    [
        (envelope(), tart_items(GOLDEN)),
        (envelope(), tart_items()),
        (envelope(items=[], returned=0, total=0), tart_items(GOLDEN)),
        ("{", tart_items()),
        (envelope(truncated=True), tart_items()),
        (envelope(items=[], returned=0, total=0), {"Name": GOLDEN}),
    ],
    ids=[
        "retained-golden",
        "public-only",
        "hypervisor-only",
        "malformed",
        "truncated",
        "malformed-tart",
    ],
)
def test_the_guard_refuses_a_listed_or_unreadable_name(
    tmp_path: Path, vm: object, tart: object
) -> None:
    assert guard(tmp_path, vm, tart) != 0


def test_the_guard_refuses_an_inaccessible_inventory(tmp_path: Path) -> None:
    assert guard(tmp_path, envelope(), tart_items(), vm_exit=1) != 0
    assert guard(tmp_path, envelope(), tart_items(), cli=str(tmp_path / "missing")) != 0


@pytest.mark.parametrize("platform", ["macos", "linux"])
def test_each_golden_recipe_runs_the_guard_before_its_create(platform: str) -> None:
    task = shutil.which("task")
    assert task is not None
    done = subprocess.run(
        [task, "-d", str(REPO), "--dry", f"vm:create-golden-{platform}", "NAME=k19-order"],
        capture_output=True,
        text=True,
        timeout=60,
    )
    lines = done.stderr.splitlines()
    guard_at = [i for i, line in enumerate(lines) if "vm_inventory.py' absent k19-order" in line]
    create_at = [i for i, line in enumerate(lines) if "vm create-golden" in line]
    assert done.returncode == 0 and len(guard_at) == 1 and len(create_at) == 1
    assert guard_at[0] < create_at[0]
    assert "--cli " + str(REPO / "cli-rs/target/debug/testanyware") in lines[guard_at[0]]
