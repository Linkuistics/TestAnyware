"""Controls for the acceptance preparation (leaf acceptance-preparation-k224).

No test boots a guest. The preparation tests build one clean preparation
directory for each OS with the k223 test builder, then break one property at a
time. The manifest tests build a fake isolated workspace and check that the
manifest reads with the k223 reader and pins what the trial reads.
"""

from __future__ import annotations

import hashlib
import json
import shutil
import subprocess
import sys
from dataclasses import asdict
from pathlib import Path
from typing import Any

import pytest

import acceptance_k223 as trial
import acceptance_k224 as prep
import controller
import image_audit_k224 as audit
import test_acceptance_k223 as k223

Json = dict[str, Any]
OSES = ("macos", "linux")


class PrepBuilder(k223.Builder):
    """Write one clean preparation directory: the PREPARATION cases on an own VM."""

    def __init__(self, tmp_path: Path, os_name: str) -> None:
        super().__init__(tmp_path, os_name)
        shutil.rmtree(self.directory)
        self.directory = tmp_path / f"{os_name}-prep"
        self.downloads = self.directory / "downloads"
        self.downloads.mkdir(parents=True)
        self.plan = prep.preparation_plan(self.manifest, os_name, sys.executable, self.downloads)
        self.run = self.directory / self.plan.run_id
        self.run.mkdir()
        resource = self.plan.resources[0]
        self.calls = {
            call.name: call
            for call in (
                *self.plan.calls,
                resource.acquire,
                resource.cleanup,
                resource.dispose,
                resource.inspect,
            )
        }
        self.receipts = {}
        self.stdout = {}
        self.inventory["leaf"] = prep.LEAF
        self.inventory["index"] = 0
        self.inventory["plan"] = json.loads(controller._json_bytes(asdict(self.plan)))  # pyright: ignore[reportPrivateUsage]
        for name in prep.SHAPE:
            self.default(name)

    def write(self, order: list[str] | None = None) -> Path:
        return super().write(order or prep.SHAPE)


def _verdict(builder: PrepBuilder, order: list[str] | None = None) -> Json:
    return prep.check_preparation(builder.write(order))


# --- the preparation plan ---------------------------------------------------


@pytest.mark.parametrize("os_name", OSES)
def test_preparation_plan_holds_the_k223_calls_of_its_cases(tmp_path: Path, os_name: str) -> None:
    manifest = trial.read_manifest(k223._manifest(tmp_path))  # pyright: ignore[reportPrivateUsage]
    plan = prep.preparation_plan(manifest, os_name, sys.executable, tmp_path / "downloads")
    controller._validate(plan, tmp_path / "evidence")  # pyright: ignore[reportPrivateUsage]
    assert [call.name for call in plan.calls] == list(prep.PREPARATION)
    clone = prep.preparation_clone(
        manifest, os_name, plan.run_id, sys.executable, tmp_path / "downloads"
    )
    canonical = {call.name: call for call in trial.clone_calls(clone)}
    assert all(call == canonical[call.name] for call in plan.calls)
    vm = plan.resources[0].identity
    trial_vm = trial.plan(manifest, os_name, 1, sys.executable, tmp_path).resources[0].identity
    assert vm == f"k224-{os_name}-00-{plan.run_id}" and len(vm) == len(trial_vm)
    assert plan.resources[0].acquire.budget == manifest.boot
    assert Path(prep.__file__).resolve() in plan.subjects


@pytest.mark.parametrize("os_name", OSES)
def test_clean_preparation_passes_and_each_corruption_control_is_rejected(
    tmp_path: Path, os_name: str
) -> None:
    verdict = _verdict(PrepBuilder(tmp_path, os_name))
    assert verdict["failures"] == [] and verdict["ok"] is True
    assert all(case["ok"] for case in verdict["cases"].values())
    controls = verdict["corruption_controls"]
    assert controls["ok"] is True and controls["originals_unchanged"] is True
    assert set(controls["controls"]) == set(prep.checker.CONTROLS)
    assert all(control["rejected"] is True for control in controls["controls"].values())


def test_an_agent_that_is_not_the_frozen_agent_fails(tmp_path: Path) -> None:
    builder = PrepBuilder(tmp_path, "macos")
    builder.exec("identity", {"kind": "exited", "exit_code": 0}, builder.identity_output("0" * 64))
    verdict = _verdict(builder)
    assert verdict["ok"] is False
    assert any("agent identity" in f for f in verdict["cases"]["identity"]["failures"])


def test_an_app_that_is_not_the_frozen_app_fails(tmp_path: Path) -> None:
    builder = PrepBuilder(tmp_path, "linux")
    builder.put("app-identity", builder.app_record(executable="/Applications/TestAnyware.app/x"))
    verdict = _verdict(builder)
    assert verdict["ok"] is False and verdict["cases"]["app-identity"]["ok"] is False


def test_a_timeout_record_without_outcome_voids_the_controls(tmp_path: Path) -> None:
    builder = PrepBuilder(tmp_path, "macos")
    envelope = json.loads(builder.stdout["timeout"])
    del envelope["outcome"]
    builder.put("timeout", envelope, elapsed=4.1)
    verdict = _verdict(builder)
    assert verdict["ok"] is False and verdict["corruption_controls"]["ok"] is False


def test_a_preparation_on_the_trial_vm_identity_fails(tmp_path: Path) -> None:
    builder = PrepBuilder(tmp_path, "linux")
    plan = builder.inventory["plan"]
    vm = plan["resources"][0]["acquire"]["argv"][
        plan["resources"][0]["acquire"]["argv"].index("--id") + 1
    ]
    text = json.dumps(builder.inventory).replace(vm, vm.replace("-00-", "-01-"))
    builder.inventory = json.loads(text)
    verdict = _verdict(builder)
    assert verdict["ok"] is False
    assert any("preparation clone identity" in f for f in verdict["failures"])


def test_a_missing_case_fails_the_preparation(tmp_path: Path) -> None:
    builder = PrepBuilder(tmp_path, "macos")
    del builder.receipts["capture"]
    verdict = _verdict(builder)
    assert verdict["ok"] is False and verdict["cases"]["capture"]["failures"] == ["unreached"]


def test_a_subject_that_differs_from_the_freeze_fails_the_preparation(tmp_path: Path) -> None:
    builder = PrepBuilder(tmp_path, "macos")
    builder.subjects[k223.CLI] = "0" * 64
    verdict = _verdict(builder)
    assert verdict["ok"] is False
    assert any(f.startswith(f"subject {k223.CLI}") for f in verdict["failures"])


def test_a_trial_shape_is_not_a_preparation(tmp_path: Path) -> None:
    directory = k223.Builder(tmp_path, "macos").write()
    verdict = prep.check_preparation(directory)
    assert verdict == {"ok": False, "failures": ["the frozen plan is not the preparation shape"]}


# --- the Linux agent identity ----------------------------------------------


def test_linux_listing_is_the_listing_that_the_identity_case_prints(tmp_path: Path) -> None:
    package = tmp_path / "testanyware_agent"
    (package / "sub").mkdir(parents=True)
    (package / "__pycache__").mkdir()
    (package / "server.py").write_text("a\n")
    (package / "Z.py").write_text("b\n")
    (package / "sub" / "exec.py").write_text("c\n")
    (package / "__pycache__" / "server.cpython-312.pyc").write_bytes(b"\0")
    (package / "notes.txt").write_text("d\n")
    # The guest runs `sha256sum`. `shasum -a 256` prints the same lines on this host.
    command = "find . -type f -name '*.py' | LC_ALL=C sort | xargs shasum -a 256"
    done = subprocess.run(
        ("/bin/sh", "-c", command), cwd=tmp_path, capture_output=True, check=True, timeout=10
    )
    assert prep.linux_listing(tmp_path) == done.stdout.decode()
    stdout = done.stdout + b'NAME="Ubuntu"\n'
    assert prep.checker.agent_identity("linux", stdout) == prep.linux_identity(tmp_path)


# --- the export -------------------------------------------------------------


def test_export_plan_fits_the_export_bound(tmp_path: Path) -> None:
    plan = prep.export_plan(tmp_path / "archives", ["g-macos-1", "g-linux-1"], "/bin/echo")
    controller._validate(plan, tmp_path / "evidence")  # pyright: ignore[reportPrivateUsage]
    assert [call.argv[1:3] for call in plan.calls] == [
        ("export", "g-macos-1"),
        ("export", "g-linux-1"),
    ]
    assert all(c.budget + plan.term_grace + plan.kill_wait == prep.EXPORT for c in plan.calls)


def test_golden_names_need_one_golden_for_each_os() -> None:
    names = ["testanyware-golden-macos-27-k224", "testanyware-golden-linux-24.04-k224"]
    assert prep.golden_names(names) == {"macos": names[0], "linux": names[1]}
    with pytest.raises(SystemExit):
        prep.golden_names(names[:1])
    with pytest.raises(SystemExit):
        prep.golden_names([*names, "testanyware-golden-macos-27-other"])


# --- the manifest -----------------------------------------------------------


def _workspace(tmp_path: Path) -> tuple[Path, Json, Json]:
    root = tmp_path / "build"
    for relative in (prep.CLI, prep.GOLDEN_CLI, prep.APP_EXECUTABLE, prep.MACOS_AGENT):
        (root / relative).parent.mkdir(parents=True, exist_ok=True)
        (root / relative).write_bytes(relative.name.encode())
    package = root / prep.LINUX_AGENT / "testanyware_agent"
    package.mkdir(parents=True)
    (package / "server.py").write_text("x\n")
    vms = tmp_path / "vms"
    goldens = {"macos": "g-macos-27-k224", "linux": "g-linux-24.04-k224"}
    images: Json = {"vms": str(vms), "images": {}}
    exports: Json = {"ok": True, "exports": {}}
    for golden in goldens.values():
        (vms / golden).mkdir(parents=True)
        files: Json = {}
        for name in audit.FILES:
            (vms / golden / name).write_text(name)
            files[name] = {"size": len(name), "sha256": hashlib.sha256(name.encode()).hexdigest()}
        images["images"][golden] = {"hashed": True, "files": files}
        exports["exports"][golden] = {"sha256": "1" * 64, "bytes": 1, "returncode": 0}
    return root, images, exports


@pytest.fixture
def isolated(monkeypatch: pytest.MonkeyPatch, tmp_path: Path) -> tuple[Path, Json, Json]:
    root, images, exports = _workspace(tmp_path)
    tart = tmp_path / "tart"
    tart.write_text("tart")
    monkeypatch.setattr(prep, "TART", str(tart))
    monkeypatch.setattr(
        prep,
        "answering_app",
        lambda cli: {"build": "0.0.1", "executable": str(cli.parents[3] / prep.APP_EXECUTABLE)},  # pyright: ignore[reportUnknownLambdaType]
    )
    monkeypatch.setattr(
        prep,
        "source_identity",
        lambda r: {"commit": "f" * 40, "working_copy_changes": ""},  # pyright: ignore[reportUnknownLambdaType]
    )
    monkeypatch.setattr(prep, "codesign", lambda b: {"TeamIdentifier": "TA43A4RUP3"})  # pyright: ignore[reportUnknownLambdaType]
    return root, images, exports


GOLDENS = {"macos": "g-macos-27-k224", "linux": "g-linux-24.04-k224"}


def test_manifest_reads_and_pins_what_the_trial_reads(
    isolated: tuple[Path, Json, Json], tmp_path: Path
) -> None:
    root, images, exports = isolated
    data = prep.build_manifest(root, GOLDENS, images, exports)
    path = tmp_path / "manifest.json"
    path.write_text(json.dumps(data))
    manifest = trial.read_manifest(path)
    assert manifest.cli == str(root / prep.CLI) and manifest.app_build == "0.0.1"
    assert manifest.boot == prep.BOOT and manifest.slot == prep.SLOT
    assert data["bounds"]["export"] == prep.EXPORT and data["ceilings"]["export"] == prep.EXPORT
    assert manifest.agents["macos"] == hashlib.sha256(prep.MACOS_AGENT.name.encode()).hexdigest()
    assert manifest.agents["linux"] == prep.linux_identity(root / prep.LINUX_AGENT)
    pinned = set(manifest.sha256)
    for module in (*trial.checker_modules(), prep.HERE / "acceptance_k223_cancel.py"):
        assert str(module.resolve()) in pinned
    assert str(prep.REPO / "Taskfile.yml") in pinned and str(trial.CHECKER) in pinned
    assert str(root / prep.MACOS_AGENT) in pinned
    for golden in GOLDENS.values():
        assert str(tmp_path / "vms" / golden / "nvram.bin") in pinned
    assert str(Path(prep.__file__).resolve()) not in pinned
    assert trial.freeze_refusals(manifest, hashlib.sha256(path.read_bytes()).hexdigest()) == []


def test_manifest_refuses_another_answering_app(
    isolated: tuple[Path, Json, Json], monkeypatch: pytest.MonkeyPatch
) -> None:
    root, images, exports = isolated
    monkeypatch.setattr(
        prep,
        "answering_app",
        lambda cli: {"build": "2.8.0", "executable": "/Applications/x"},  # pyright: ignore[reportUnknownLambdaType]
    )
    with pytest.raises(SystemExit, match="answering app"):
        prep.build_manifest(root, GOLDENS, images, exports)


def test_manifest_refuses_a_changed_workspace(
    isolated: tuple[Path, Json, Json], monkeypatch: pytest.MonkeyPatch
) -> None:
    root, images, exports = isolated
    monkeypatch.setattr(
        prep,
        "source_identity",
        lambda r: {"commit": "f" * 40, "working_copy_changes": "M x"},  # pyright: ignore[reportUnknownLambdaType]
    )
    with pytest.raises(SystemExit, match="changes"):
        prep.build_manifest(root, GOLDENS, images, exports)


def test_manifest_refuses_a_failed_export(isolated: tuple[Path, Json, Json]) -> None:
    root, images, exports = isolated
    with pytest.raises(SystemExit, match="exports"):
        prep.build_manifest(root, GOLDENS, images, exports | {"ok": False})


# --- the image audit --------------------------------------------------------


def _snapshot(state: str = "stopped", **changes: Any) -> Json:
    files = {
        "config.json": {"present": True, "size": 1, "sha256": "a", "mtime_ns": 1},
        "nvram.bin": {"present": True, "size": 2, "sha256": "b", "mtime_ns": 2},
        "disk.img": {"present": True, "size": 3, "sha256": "c", "mtime_ns": 3},
    }
    for key, value in changes.items():
        name, field = key.split("__")
        files[{"disk": "disk.img", "nvram": "nvram.bin", "config": "config.json"}[name]][field] = (
            value
        )
    listing = json.dumps([{"Name": "g", "State": state, "Source": "local"}])
    return {
        "tart": {"returncode": 0, "stdout": listing},
        "images": {"g": {"hashed": True, "files": files}},
    }


@pytest.mark.parametrize(
    ("after", "ok"),
    [
        (_snapshot(), True),
        (_snapshot(disk__sha256="d"), False),
        (_snapshot(nvram__mtime_ns=9), False),
        (_snapshot(disk__mtime_ns=9), False),
        (_snapshot(state="running"), False),
        (_snapshot(config__size=7), False),
    ],
)
def test_image_audit_requires_unchanged_and_not_booted(after: Json, ok: bool) -> None:
    assert audit.compare(_snapshot(), after)["ok"] is ok
