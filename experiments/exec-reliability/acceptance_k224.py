"""Prepare and freeze the guest command lifecycle acceptance (leaf acceptance-preparation-k224).

    acceptance_k224.py export OUT DEST GOLDEN...
    acceptance_k224.py manifest OUT BUILD_ROOT IMAGES EXPORTS
    acceptance_k224.py prepare OUT MANIFEST OS
    acceptance_k224.py check DIRECTORY

`export` exports each new golden once with `tart export` into the new
directory DEST. Each export is one controller call within the export bound.
OUT receives the controller evidence and `exports.json`, which gives the size
and the SHA-256 of each archive: the export identity.

`manifest` writes the freeze manifest OUT for leaf lifecycle-acceptance-k225.
BUILD_ROOT is the isolated jj workspace that holds the private builds. IMAGES
is the `image_audit_k224.py` snapshot that hashed the new goldens. EXPORTS is
the `exports.json` of `export`. The manifest has every field that
`acceptance_k223.read_manifest` reads. Its `sha256` field pins the private
Host CLI, Tart, the app executable, the macOS agent binary, the small files of
each new golden, and each file that the trial reads on the host. The trial
refuses a clone when one of these digests differs. The other fields record the
builds, the images, the exports, the agent identities and every ceiling and
bound. The command reads the answering app from `doctor --json` and refuses to
write the manifest when that app is not the private app of BUILD_ROOT.

`prepare` runs one preparation clone of the new golden of OS through the
private builds. It is not a trial clone. Its plan holds the k223 cases that
PREPARATION names, with the k223 argv, budgets and expectations. The
directory layout is the layout of a trial clone, so the k223 checker judges
each case and runs the corruption controls on the real evidence.

`check` judges one preparation directory again and exits 0 only when it
passes. The verdict applies the k223 judgement to each case, the freeze and
the recorded execution. It also requires that each corruption control rejects
its damaged copy and that the originals still match.
"""

from __future__ import annotations

import hashlib
import json
import os
import subprocess
import sys
from dataclasses import asdict
from pathlib import Path
from typing import Any, cast

import acceptance_k223 as trial
import acceptance_k223_check as checker
from completion_k186_check import _run_failures  # pyright: ignore[reportPrivateUsage]
from controller import Call, Plan, execute_outcome, write_json

LEAF = "acceptance-preparation-k224"
HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]

BOOT = 420.0  # 360 s of `vm start` stage polls, plus 60 s for the clone, as in k19
SLOT = 30.0  # one ordinary observation. No trial clone waits for a slot.
EXPORT = 1800.0  # one `tart export` of one new golden
EXPORT_CALL = EXPORT - trial.TERM_GRACE - trial.KILL_WAIT
DOCTOR = trial.OBSERVATION

TART = "/opt/homebrew/bin/tart"
PREPARATION = (
    "app-identity",
    "health-before",
    "identity",
    "bootstrap-dir",
    "upload-guest-sh",
    "upload-lesson",
    "baseline",
    "install",
    "intact",
    "timeout",
    "capture",
    "download-artifact-bin",
)
SHAPE = ["acquire", *PREPARATION, *checker.RESOURCE[1:]]
ORIGINALS = ("intact", "timeout", "download-artifact-bin")

# The paths of the private builds inside the isolated workspace.
CLI = Path("cli-rs/target/release/testanyware")
GOLDEN_CLI = Path("cli-rs/target/debug/testanyware")
APP_EXECUTABLE = Path("cli-rs/target/app/dev/TestAnyware Dev.app/Contents/MacOS/testanyware-app")
APP_BUNDLE = Path("cli-rs/target/app/dev/TestAnyware Dev.app")
MACOS_AGENT = Path("agents/macos/.build/release/testanyware-agent")
LINUX_AGENT = Path("agents/linux")
GOLDEN_FILES = ("config.json", "nvram.bin")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(8 * 1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def linux_listing(agent_root: Path) -> str:
    """The `sha256sum` listing that the k223 `identity` case prints in the guest.

    The Linux golden unpacks `<agent_root>/testanyware_agent` into
    `/opt/testanyware`. The case lists each `*.py` file there, sorted in the C
    locale, with the path relative to `/opt/testanyware`.
    """
    paths = sorted(
        (f"./{path.relative_to(agent_root).as_posix()}", path)
        for path in (agent_root / "testanyware_agent").rglob("*.py")
        if path.is_file()
    )
    paths.sort(key=lambda item: item[0].encode())
    return "".join(f"{sha256(path)}  {name}\n" for name, path in paths)


def linux_identity(agent_root: Path) -> str:
    return hashlib.sha256(linux_listing(agent_root).encode()).hexdigest()


def trial_files() -> list[Path]:
    """Each host file that the trial reads, besides the tools and the manifest.

    The list holds every module under this directory that the trial and its
    checker import, the two helpers that the trial runs as scripts, the
    dependency lock, the Taskfile and the inventory checker.
    """
    modules = {
        Path(cast(str, module.__file__)).resolve()
        for module in list(sys.modules.values())
        if getattr(module, "__file__", None)
        and Path(cast(str, module.__file__)).resolve().parent == HERE
    }
    modules -= {Path(__file__).resolve()}
    return sorted(
        {
            *modules,
            HERE / "acceptance_k223_cancel.py",
            HERE / "acceptance_k223_app.py",
            HERE / "pyproject.toml",
            HERE / "uv.lock",
            REPO / "Taskfile.yml",
            trial.CHECKER,
        }
    )


# --- export -----------------------------------------------------------------


def export_plan(destination: Path, goldens: list[str], tart: str = TART) -> Plan:
    """One `tart export` call for each golden. Budget, TERM and KILL fit the export bound."""
    calls = tuple(
        Call(
            f"export-{index}",
            (tart, "export", golden, str(destination / f"{golden}.tvm")),
            EXPORT_CALL,
        )
        for index, golden in enumerate(goldens, start=1)
    )
    return Plan(os.urandom(16).hex(), calls, subjects=(Path(__file__).resolve(), Path(tart)))


def run_export(output: Path, destination: Path, goldens: list[str]) -> dict[str, Any]:
    output.mkdir(parents=True, exist_ok=False)
    destination.mkdir(parents=True, exist_ok=False)
    plan = export_plan(destination, goldens)
    write_json(output / "plan.json", {"leaf": LEAF, "export_bound": EXPORT, "plan": asdict(plan)})
    outcome = execute_outcome(plan, output)
    receipts = {
        str(call["name"]): call for call in cast(list[dict[str, Any]], outcome.result["calls"])
    }
    exports: dict[str, Any] = {}
    for index, golden in enumerate(goldens, start=1):
        archive = destination / f"{golden}.tvm"
        receipt = receipts.get(f"export-{index}", {})
        entry: dict[str, Any] = {
            "archive": str(archive),
            "returncode": receipt.get("returncode"),
            "elapsed": receipt.get("elapsed"),
            "within_bound": isinstance(receipt.get("elapsed"), int | float)
            and receipt["elapsed"] <= EXPORT,
        }
        if archive.is_file():
            entry["bytes"] = archive.stat().st_size
            entry["sha256"] = sha256(archive)
        exports[golden] = entry
    record = {
        "leaf": LEAF,
        "export_bound": EXPORT,
        "run": str(outcome.run_dir),
        "verdict": outcome.verdict,
        "residuals": outcome.residuals,
        "exports": exports,
        "ok": outcome.verdict is True
        and all(
            e.get("returncode") == 0 and e["within_bound"] and "sha256" in e
            for e in exports.values()
        ),
    }
    write_json(output / "exports.json", record)
    return record


# --- manifest ---------------------------------------------------------------


def _run(argv: list[str], cwd: Path | None = None, timeout: float = DOCTOR) -> str:
    done = subprocess.run(argv, cwd=cwd, capture_output=True, timeout=timeout, check=True)
    return done.stdout.decode()


def answering_app(cli: Path) -> dict[str, Any]:
    """The running app as `doctor --json` of the private Host CLI reports it."""
    done = subprocess.run(
        (str(cli), "doctor", "--json"), capture_output=True, timeout=DOCTOR, check=False
    )
    report = cast(dict[str, Any], json.loads(done.stdout))
    return cast(dict[str, Any], report["checks"]["app"]["running"])


def source_identity(build_root: Path) -> dict[str, Any]:
    jj = ["jj", "--no-pager", "--repository", str(build_root)]
    parent = _run([*jj, "log", "--no-graph", "-r", "@-", "-T", "commit_id"]).strip()
    diff = _run([*jj, "diff", "--summary", "-r", "@"]).strip()
    return {"workspace": str(build_root), "commit": parent, "working_copy_changes": diff}


def codesign(bundle: Path) -> dict[str, str]:
    done = subprocess.run(
        ("codesign", "-dvv", str(bundle)), capture_output=True, timeout=DOCTOR, check=True
    )
    fields: dict[str, str] = {}
    for line in done.stderr.decode().splitlines():
        key, sep, value = line.partition("=")
        if sep and key in ("Identifier", "TeamIdentifier", "CDHash", "Authority", "Timestamp"):
            fields.setdefault(key, value)
    return fields


def _image(images: dict[str, Any], golden: str) -> dict[str, Any]:
    entry = cast(dict[str, Any], images["images"][golden])
    if not entry["hashed"]:
        raise SystemExit(f"the image snapshot did not hash {golden}")
    files = cast(dict[str, dict[str, Any]], entry["files"])
    return {
        name: {"bytes": record["size"], "sha256": record["sha256"]}
        for name, record in files.items()
    }


def golden_names(names: list[str]) -> dict[str, str]:
    """Map each OS to the one exported golden whose name holds `-<os>-`."""
    goldens: dict[str, str] = {}
    for os_name in trial.OSES:
        matches = [name for name in names if f"-{os_name}-" in name]
        if len(matches) != 1:
            raise SystemExit(f"the exports name {len(matches)} goldens for {os_name}")
        goldens[os_name] = matches[0]
    if len(names) != len(trial.OSES):
        raise SystemExit(f"the exports name {names}, not one golden for each OS")
    return goldens


def build_manifest(
    build_root: Path, goldens: dict[str, str], images: dict[str, Any], exports: dict[str, Any]
) -> dict[str, Any]:
    cli, app = build_root / CLI, build_root / APP_EXECUTABLE
    running = answering_app(cli)
    if running.get("executable") != str(app):
        raise SystemExit(f"the answering app {running.get('executable')!r} is not {app}")
    build = running.get("build")
    if not isinstance(build, str) or not build:
        raise SystemExit("the answering app reports no build")
    vms = Path(cast(str, images["vms"]))
    pins = [cli, Path(TART), app, build_root / MACOS_AGENT, build_root / GOLDEN_CLI]
    pins += [vms / golden / name for golden in goldens.values() for name in GOLDEN_FILES]
    pins += [Path(sys.executable), *trial_files()]
    agents = {
        "macos": sha256(build_root / MACOS_AGENT),
        "linux": linux_identity(build_root / LINUX_AGENT),
    }
    source = source_identity(build_root)
    if source["working_copy_changes"]:
        raise SystemExit(f"the build workspace has changes: {source['working_copy_changes']}")
    export_entries = cast(dict[str, Any], exports["exports"])
    if exports.get("ok") is not True:
        raise SystemExit("the exports did not all pass")
    data: dict[str, Any] = {
        "leaf": LEAF,
        "cli": str(cli),
        "tart": TART,
        "goldens": dict(goldens),
        "bounds": {"boot": BOOT, "slot": SLOT, "export": EXPORT},
        "agents": agents,
        "app": {"build": build, "executable": str(app)},
        "sha256": {str(path): sha256(path) for path in pins},
        "source": source,
        "builds": {
            "cli": {"path": str(cli), "profile": "release"},
            "golden_cli": {"path": str(build_root / GOLDEN_CLI), "profile": "debug"},
            "app": {
                "bundle": str(build_root / APP_BUNDLE),
                "executable": str(app),
                "build": build,
                "codesign": codesign(build_root / APP_BUNDLE),
            },
            "tart": {"path": TART, "resolved": str(Path(TART).resolve())},
        },
        "agent_versions": {
            os_name: {
                "identity": agents[os_name],
                "source_commit": source["commit"],
                "note": "the agent reports no version; its identity and source commit name it",
            }
            for os_name in trial.OSES
        },
        "agent_sources": {
            "macos": str(build_root / MACOS_AGENT),
            "linux": str(build_root / LINUX_AGENT / "testanyware_agent"),
            "linux_listing": linux_listing(build_root / LINUX_AGENT),
        },
        "images": {golden: _image(images, golden) for golden in goldens.values()},
        "exports": {golden: export_entries[golden] for golden in goldens.values()},
        "ceilings": trial.bounds(trial.Manifest(Path("/"), "", "", {}, BOOT, SLOT, {}, "", "", {}))
        | {"export": EXPORT},
        "slot": trial.SLOT_NOTE,
        "export": "declared; each new golden was exported once in k224, and no trial clone exports",
    }
    return data


# --- preparation ------------------------------------------------------------


def preparation_clone(
    manifest: trial.Manifest, os_name: str, run_id: str, python: str, downloads: Path
) -> trial.Clone:
    return trial.Clone(
        os_name=os_name,
        index=0,
        run_id=run_id,
        vm=f"k224-{os_name}-00-{run_id}",
        cli=manifest.cli,
        python=python,
        downloads=downloads,
    )


def preparation_plan(manifest: trial.Manifest, os_name: str, python: str, downloads: Path) -> Plan:
    """The preparation plan: the PREPARATION cases of a trial clone, on an own VM."""
    frozen = trial.plan(manifest, os_name, 0, python, downloads)
    clone = preparation_clone(manifest, os_name, frozen.run_id, python, downloads)
    calls = tuple(call for call in trial.clone_calls(clone) if call.name in PREPARATION)
    resource = trial._resource(clone, manifest.goldens[os_name], manifest.tart, manifest.boot)  # pyright: ignore[reportPrivateUsage]
    return Plan(
        frozen.run_id,
        calls,
        resources=(resource,),
        subjects=(*frozen.subjects, Path(__file__).resolve()),
        term_grace=trial.TERM_GRACE,
        kill_wait=trial.KILL_WAIT,
    )


def run_preparation(output: Path, manifest_path: Path, os_name: str) -> dict[str, Any]:
    manifest_sha256 = hashlib.sha256(manifest_path.read_bytes()).hexdigest()
    manifest = trial.read_manifest(manifest_path)
    refusals = trial.freeze_refusals(manifest, manifest_sha256)
    if refusals:
        raise SystemExit(f"frozen subjects differ: {refusals}")
    output.mkdir(parents=True, exist_ok=False)
    downloads = output / "downloads"
    downloads.mkdir()
    plan = preparation_plan(manifest, os_name, sys.executable, downloads)
    write_json(
        output / "inventory.json",
        {
            "leaf": LEAF,
            "role": "preparation clone, not a trial clone",
            "os": os_name,
            "index": 0,
            "golden": manifest.goldens[os_name],
            "rule": "one plan, run once; no retry; a failed sample stays",
            "manifest": str(manifest.path),
            "manifest_sha256": manifest_sha256,
            "frozen_sha256": manifest.sha256,
            "identities": trial.identities(manifest, os_name),
            "fixture_sha256": trial.FIXTURE_SHA256[os_name],
            "bounds": trial.bounds(manifest),
            "slot": trial.SLOT_NOTE,
            "plan": asdict(plan),
        },
    )
    outcome = execute_outcome(plan, output)
    write_json(
        output / "summary.json",
        {
            "verdict": outcome.verdict,
            "run": str(outcome.run_dir),
            "ledger": outcome.ledger,
            "residuals": outcome.residuals,
        },
    )
    verdict = check_preparation(output)
    write_json(output / "check.json", verdict)
    return verdict


def shape_failures(inventory: dict[str, Any]) -> list[str]:
    """Rebuild the preparation plan from its identities and compare each field."""
    plan = checker._mapping(inventory.get("plan"))  # pyright: ignore[reportPrivateUsage]
    try:
        calls = {str(c["name"]): c for c in cast(list[dict[str, Any]], plan["calls"])}
        resource = cast(list[dict[str, Any]], plan["resources"])[0]
        acquire = cast(list[str], resource["acquire"]["argv"])
        dispose = cast(list[str], resource["dispose"]["argv"])
        run_id = str(plan["run_id"])
        os_name = str(inventory["os"])
        clone = trial.Clone(
            os_name=os_name,
            index=0,
            run_id=run_id,
            vm=acquire[acquire.index("--id") + 1],
            cli=acquire[0],
            python=str(calls["app-identity"]["argv"][0]),
            downloads=Path(str(calls["download-artifact-bin"]["argv"][6])).parent,
        )
        golden = acquire[acquire.index("--base") + 1]
        tart = dispose[dispose.index("--tart") + 1]
        boot = float(checker._mapping(inventory.get("bounds"))["acquire"])  # pyright: ignore[reportPrivateUsage]
    except (KeyError, IndexError, TypeError, ValueError):
        return ["the frozen plan lacks an identity of the preparation shape"]
    failures: list[str] = []
    if clone.vm != f"k224-{os_name}-00-{run_id}":
        failures.append(f"VM {clone.vm!r} is not the preparation clone identity")
    expected = [asdict(c) for c in trial.clone_calls(clone) if c.name in PREPARATION]
    if checker._normal(plan["calls"]) != checker._normal(expected):  # pyright: ignore[reportPrivateUsage]
        failures.append("the frozen calls are not the k223 calls of the preparation cases")
    resource_expected = trial._resource(clone, golden, tart, boot)  # pyright: ignore[reportPrivateUsage]
    if checker._normal(resource) != checker._normal(asdict(resource_expected)):  # pyright: ignore[reportPrivateUsage]
        failures.append("the frozen resource is not the acceptance resource")
    return failures


def check_preparation(directory: Path) -> dict[str, Any]:
    """Judge one preparation directory with the k223 judgement."""
    clone = checker.Clone(directory)
    if clone.order != SHAPE:
        return {"ok": False, "failures": ["the frozen plan is not the preparation shape"]}
    failures = shape_failures(clone.inventory)
    run_plan = checker._json(checker._read(clone.run / "plan.json"))  # pyright: ignore[reportPrivateUsage]
    if run_plan != clone.inventory.get("plan"):
        failures.append("the controller plan differs from the frozen inventory plan")
    records = cast(list[Any], clone.result.get("calls") or [])
    names = [checker._mapping(r).get("name") for r in records]  # pyright: ignore[reportPrivateUsage]
    failures += _run_failures(clone.run, clone.result, cast(list[object], names))
    failures += checker.execution_failures(clone)
    failures += checker.freeze_failures(clone)
    if clone.summary.get("verdict") is not True or clone.summary.get("residuals") != []:
        failures.append(
            f"controller verdict {clone.summary.get('verdict')!r} "
            f"residuals {clone.summary.get('residuals')!r}"
        )
    ledger = [checker._mapping(e) for e in cast(list[Any], clone.summary.get("ledger") or [])]  # pyright: ignore[reportPrivateUsage]
    if [(e.get("kind"), e.get("state")) for e in ledger] != [("vm", "disposed")]:
        failures.append(f"ledger {ledger!r}")
    cases: dict[str, Any] = {}
    for name in clone.order:
        if name not in clone.receipts:
            cases[name] = {"ok": False, "failures": ["unreached"]}
            continue
        case_failures = checker.judge_call(clone, name)
        cases[name] = {"ok": not case_failures, "failures": case_failures}
    controls = (
        checker.corruption_controls(clone)
        if all(cases.get(name, {}).get("ok") for name in ORIGINALS)
        else {"ok": False, "reason": "an original item failed or was unreached"}
    )
    ok = not failures and all(case["ok"] for case in cases.values()) and controls["ok"] is True
    return {
        "ok": ok,
        "directory": str(directory),
        "run": str(clone.run),
        "failures": failures,
        "cases": cases,
        "corruption_controls": controls,
    }


def main() -> None:
    verb, arguments = (sys.argv[1], sys.argv[2:]) if len(sys.argv) > 1 else ("", [])
    if verb == "export" and len(arguments) >= 3:
        record = run_export(
            Path(arguments[0]).resolve(), Path(arguments[1]).resolve(), arguments[2:]
        )
        print(json.dumps(record, indent=2))
        raise SystemExit(0 if record["ok"] else 1)
    if verb == "manifest" and len(arguments) == 4:
        output, build_root, images, exports = (Path(a) for a in arguments)
        exported = cast(dict[str, Any], json.loads(exports.read_bytes()))
        goldens = golden_names(list(cast(dict[str, Any], exported["exports"])))
        data = build_manifest(
            build_root.resolve(), goldens, json.loads(images.read_bytes()), exported
        )
        with output.open("x") as stream:
            json.dump(data, stream, indent=2, sort_keys=True)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        trial.read_manifest(output)
        print(f"{output}: {hashlib.sha256(output.read_bytes()).hexdigest()}")
        return
    if verb == "prepare" and len(arguments) == 3:
        verdict = run_preparation(Path(arguments[0]).resolve(), Path(arguments[1]), arguments[2])
        print(json.dumps(verdict, indent=2))
        raise SystemExit(0 if verdict["ok"] is True else 1)
    if verb == "check" and len(arguments) == 1:
        verdict = check_preparation(Path(arguments[0]).resolve())
        print(json.dumps(verdict, indent=2))
        raise SystemExit(0 if verdict["ok"] is True else 1)
    raise SystemExit(__doc__)


if __name__ == "__main__":
    main()
