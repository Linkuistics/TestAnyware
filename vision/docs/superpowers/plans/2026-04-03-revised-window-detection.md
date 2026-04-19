# GUIVisionPipeline: Revised Window Detection Plan (VM-Only Data Generation)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Context:** Phase 0 (scaffolding) and much of Phase 1 are already implemented. This revised plan replaces tasks that incorrectly used a synthetic screenshot renderer. ALL training data must come from real VM screenshots captured via GUIVisionVMDriver.

**What's already built and correct:**
- `common/` — BoundingBox, Detection, DetectionSet, GroundTruth, GroundTruthSource, metrics, NMS, image I/O, A/B test harness
- `stages/window-detection/generator/` — WindowSpec, WindowScenario, scenario_library (9 scenarios), VMCaptureSession skeleton
- `stages/window-detection/training/` — YOLO format converter, training script + config
- `stages/window-detection/analysis/` — heuristic detector, YOLO detector wrapper

**What this plan replaces (removed):**
- ~~synthetic.py~~ — drew fake windows with NumPy. WRONG.
- ~~dataset.py~~ — built datasets from synthetic renderer. WRONG.
- ~~cli.py~~ — drove synthetic generation. WRONG.
- ~~test_baseline_accuracy.py~~ — evaluated on synthetic images. WRONG.

**Core principle:** Ground truth comes from **programmatic knowledge**. We tell the VM "open TextEdit at (100, 200) sized 800x600" — we KNOW the ground truth because we defined the scenario. The screenshot is REAL. The ground truth is DERIVED from what we commanded.

---

## Prerequisites

Before executing this plan, a macOS VM must be accessible via VNC and SSH. GUIVisionVMDriver (`guivision` CLI) must be installed and on `$PATH`. The VM does not need to be managed by this project — it just needs to be running and reachable.

Create a connection spec file for the VM:

```json
// vm-connection.json (gitignored — per-machine config)
{
    "vnc": { "host": "localhost", "port": 5901 },
    "ssh": { "user": "admin", "host": "localhost", "port": 2222 },
    "platform": "macos"
}
```

---

## Task R1: Rework VMCaptureSession to Use GUIVisionVMDriver CLI Properly

The current `vm_capture.py` shells out to raw `ssh` and `osascript`. It should use `guivision` CLI's actual subcommands (`ssh exec`, `screenshot`, `input`) with the `--connect` flag.

**Files:**
- Modify: `stages/window-detection/generator/src/window_gen/vm_capture.py`
- Modify: `stages/window-detection/generator/tests/test_vm_capture.py`

- [ ] **Step 1: Update VMCaptureConfig to use connection spec**

Replace the scattered VNC/SSH fields with a single connection spec path:

```python
@dataclass
class VMCaptureConfig:
    connect_spec: Path  # path to vm-connection.json
    settle_delay: float = 1.0  # seconds to wait after window positioning
    guivision_cli: str = "guivision"
```

- [ ] **Step 2: Rewrite VMCaptureSession to use `guivision` CLI**

Key changes:
- `_run_ssh(cmd)` → `guivision ssh exec --connect {spec} -- {cmd}`
- `_capture_screenshot(path)` → `guivision screenshot --connect {spec} --output {path}`
- `_get_screen_size()` → `guivision screen-size --connect {spec}` (parse JSON output)
- Window positioning via `guivision ssh exec` running AppleScript (macOS) or PowerShell (Windows)
- Add `_close_all_windows()` that uses `guivision ssh exec` to close windows before each scenario
- Add `_open_app(app_name)` that launches an app and waits for it to have a window

```python
class VMCaptureSession:
    def __init__(self, config: VMCaptureConfig):
        self.config = config
        self._connect_args = ["--connect", str(config.connect_spec)]

    def _guivision(self, *args: str) -> subprocess.CompletedProcess:
        cmd = [self.config.guivision_cli] + list(args) + self._connect_args
        return subprocess.run(cmd, capture_output=True, text=True, timeout=30, check=True)

    def _ssh_exec(self, command: str) -> str:
        result = self._guivision("ssh", "exec", "--", command)
        return result.stdout

    def capture_screenshot(self, output_path: Path) -> None:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        self._guivision("screenshot", "--output", str(output_path))

    def get_screen_size(self) -> tuple[int, int]:
        result = self._guivision("screen-size")
        # parse "WIDTHxHEIGHT" or JSON output
        ...

    def close_all_windows(self) -> None:
        """Close all application windows in the VM."""
        self._ssh_exec(
            'osascript -e \'tell application "System Events" to keystroke "q" '
            'using {command down, option down}\''
        )
        import time; time.sleep(0.5)

    def position_window(self, spec: WindowSpec, platform: str = "macos") -> None:
        if platform == "macos":
            script = self._build_macos_position_script(spec)
            self._ssh_exec(f"osascript -e '{script}'")

    def capture_scenario(self, scenario: WindowScenario, output_dir: Path, sample_name: str) -> tuple[Path, GroundTruth]:
        # 1. Close existing windows
        self.close_all_windows()

        # 2. Open apps and position windows (z-order: back to front)
        sorted_windows = sorted(scenario.windows, key=lambda w: w.z_order)
        for spec in sorted_windows:
            self._open_app(spec.app_name)
            self.position_window(spec)

        # 3. Wait for windows to settle
        import time; time.sleep(self.config.settle_delay)

        # 4. Screenshot
        img_path = output_dir / "images" / f"{sample_name}.png"
        self.capture_screenshot(img_path)

        # 5. Ground truth from programmatic knowledge
        gt = scenario.to_ground_truth(image_path=f"images/{sample_name}.png")

        # 6. Save label
        label_path = output_dir / "labels" / f"{sample_name}.json"
        label_path.parent.mkdir(parents=True, exist_ok=True)
        with open(label_path, "w") as f:
            json.dump(gt.to_dict(), f, indent=2)

        return img_path, gt
```

- [ ] **Step 3: Update unit tests**

Unit tests should test the command construction logic WITHOUT requiring a live VM:
- Test that `_build_macos_position_script` generates correct AppleScript
- Test that `capture_scenario` calls the right sequence of guivision subcommands (mock subprocess)
- Test that ground truth JSON is correctly generated from scenario

- [ ] **Step 4: Run tests**

Run: `uv run pytest stages/window-detection/generator/tests/test_vm_capture.py -v -m "not integration"`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add stages/window-detection/generator/
git commit -m "refactor(window-gen): rework VMCaptureSession to use guivision CLI properly"
```

---

## Task R2: Dataset Generator — Orchestrate VM Captures Across All Scenarios

This replaces the deleted `dataset.py`. Instead of rendering synthetic images, it drives `VMCaptureSession` to capture real screenshots for every scenario in the library.

**Files:**
- Create: `stages/window-detection/generator/src/window_gen/dataset.py`
- Create: `stages/window-detection/generator/tests/test_dataset.py`

- [ ] **Step 1: Write failing tests (mocked VM)**

```python
# stages/window-detection/generator/tests/test_dataset.py
import json
from pathlib import Path
from unittest.mock import MagicMock, patch, call

import pytest

from window_gen.dataset import generate_dataset, DatasetConfig
from window_gen.scenarios import WindowScenario, WindowSpec
from window_gen.vm_capture import VMCaptureSession


class TestDatasetConfig:
    def test_create(self, tmp_path):
        config = DatasetConfig(
            connect_spec=tmp_path / "vm.json",
            output_dir=tmp_path / "output",
        )
        assert config.output_dir == tmp_path / "output"
        assert config.repeats_per_scenario == 1


class TestGenerateDataset:
    def _simple_scenarios(self):
        return [
            WindowScenario("single", "test", 1920, 1080,
                windows=[WindowSpec("TextEdit", "Test.txt", 100, 100, 800, 600, z_order=0)]),
            WindowScenario("empty", "test", 1920, 1080, windows=[]),
        ]

    def test_calls_capture_for_each_scenario(self, tmp_path):
        """Should call VMCaptureSession.capture_scenario once per scenario per repeat."""
        config = DatasetConfig(
            connect_spec=tmp_path / "vm.json",
            output_dir=tmp_path / "output",
            repeats_per_scenario=2,
        )

        with patch("window_gen.dataset.VMCaptureSession") as MockSession:
            mock_session = MockSession.return_value
            # Make capture_scenario return dummy paths
            mock_session.capture_scenario.return_value = (
                tmp_path / "img.png",
                self._simple_scenarios()[0].to_ground_truth("img.png"),
            )
            generate_dataset(self._simple_scenarios(), config)

            # 2 scenarios * 2 repeats = 4 calls
            assert mock_session.capture_scenario.call_count == 4

    def test_generates_manifest(self, tmp_path):
        """Manifest should list all captured samples."""
        config = DatasetConfig(
            connect_spec=tmp_path / "vm.json",
            output_dir=tmp_path / "output",
            repeats_per_scenario=1,
        )

        with patch("window_gen.dataset.VMCaptureSession") as MockSession:
            mock_session = MockSession.return_value
            mock_session.capture_scenario.return_value = (
                tmp_path / "img.png",
                self._simple_scenarios()[0].to_ground_truth("img.png"),
            )
            generate_dataset(self._simple_scenarios(), config)

        manifest_path = tmp_path / "output" / "manifest.json"
        assert manifest_path.exists()
        with open(manifest_path) as f:
            manifest = json.load(f)
        assert manifest["stage"] == "window-detection"
        assert manifest["num_samples"] == 2


@pytest.mark.integration
class TestGenerateDatasetIntegration:
    """Run with: pytest -m integration (requires live VM)"""

    def test_capture_real_screenshots(self, tmp_path):
        pytest.skip("Requires live VM")
```

- [ ] **Step 2: Implement dataset generator**

```python
# stages/window-detection/generator/src/window_gen/dataset.py
from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path

from window_gen.scenarios import WindowScenario
from window_gen.vm_capture import VMCaptureConfig, VMCaptureSession


@dataclass
class DatasetConfig:
    connect_spec: Path
    output_dir: Path
    repeats_per_scenario: int = 1


def generate_dataset(
    scenarios: list[WindowScenario],
    config: DatasetConfig,
) -> Path:
    vm_config = VMCaptureConfig(connect_spec=config.connect_spec)
    session = VMCaptureSession(vm_config)

    config.output_dir.mkdir(parents=True, exist_ok=True)
    samples = []
    sample_idx = 0

    for scenario in scenarios:
        for repeat in range(config.repeats_per_scenario):
            name = f"{scenario.name}_{repeat:04d}"
            img_path, gt = session.capture_scenario(
                scenario=scenario,
                output_dir=config.output_dir,
                sample_name=name,
            )
            samples.append({
                "image": f"images/{name}.png",
                "label": f"labels/{name}.json",
                "scenario": scenario.name,
                "repeat": repeat,
            })
            sample_idx += 1

    manifest = {
        "stage": "window-detection",
        "num_samples": len(samples),
        "samples": samples,
    }
    manifest_path = config.output_dir / "manifest.json"
    with open(manifest_path, "w") as f:
        json.dump(manifest, f, indent=2)

    return manifest_path
```

- [ ] **Step 3: Run tests**

Run: `uv run pytest stages/window-detection/generator/tests/test_dataset.py -v -m "not integration"`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add stages/window-detection/generator/
git commit -m "feat(window-gen): add VM-based dataset generator orchestrating captures across scenarios"
```

---

## Task R3: CLI — Drive VM-Based Data Generation

Replaces the deleted CLI. This CLI takes a connection spec and output directory, then drives the full pipeline: scenarios → VM capture → labels → manifest.

**Files:**
- Create: `stages/window-detection/generator/src/window_gen/cli.py`
- Create: `stages/window-detection/generator/tests/test_cli.py`

- [ ] **Step 1: Write failing tests**

```python
# stages/window-detection/generator/tests/test_cli.py
from unittest.mock import patch, MagicMock
from pathlib import Path

import pytest

from window_gen.cli import main, parse_args


class TestParseArgs:
    def test_generate_subcommand(self, tmp_path):
        spec = tmp_path / "vm.json"
        spec.write_text("{}")
        args = parse_args(["generate", "--connect", str(spec), "--output", str(tmp_path / "out")])
        assert args.command == "generate"
        assert args.connect == spec
        assert args.output == tmp_path / "out"

    def test_generate_with_repeats(self, tmp_path):
        spec = tmp_path / "vm.json"
        spec.write_text("{}")
        args = parse_args(["generate", "--connect", str(spec), "--output", str(tmp_path / "out"), "--repeats", "5"])
        assert args.repeats == 5

    def test_list_scenarios_subcommand(self):
        args = parse_args(["list-scenarios"])
        assert args.command == "list-scenarios"


class TestCLIGenerate:
    def test_calls_generate_dataset(self, tmp_path):
        spec = tmp_path / "vm.json"
        spec.write_text("{}")

        with patch("window_gen.cli.generate_dataset") as mock_gen, \
             patch("window_gen.cli.build_scenario_library") as mock_lib:
            mock_lib.return_value = []
            main(["generate", "--connect", str(spec), "--output", str(tmp_path / "out")])
            mock_gen.assert_called_once()
```

- [ ] **Step 2: Implement CLI**

```python
# stages/window-detection/generator/src/window_gen/cli.py
from __future__ import annotations

import argparse
from pathlib import Path

from window_gen.dataset import DatasetConfig, generate_dataset
from window_gen.scenario_library import build_scenario_library


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Window detection training data generator (VM-based)")
    subparsers = parser.add_subparsers(dest="command", required=True)

    gen = subparsers.add_parser("generate", help="Generate training data by capturing VM screenshots")
    gen.add_argument("--connect", type=Path, required=True, help="Path to vm-connection.json")
    gen.add_argument("--output", type=Path, required=True, help="Output directory for dataset")
    gen.add_argument("--repeats", type=int, default=1, help="Captures per scenario (default: 1)")

    subparsers.add_parser("list-scenarios", help="List available scenarios")

    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> None:
    args = parse_args(argv)

    if args.command == "generate":
        scenarios = build_scenario_library()
        config = DatasetConfig(
            connect_spec=args.connect,
            output_dir=args.output,
            repeats_per_scenario=args.repeats,
        )
        manifest_path = generate_dataset(scenarios, config)
        print(f"Generated {len(scenarios) * args.repeats} samples → {manifest_path}")

    elif args.command == "list-scenarios":
        scenarios = build_scenario_library()
        for s in scenarios:
            print(f"  {s.name:25s}  windows={len(s.windows):2d}  {s.description}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 3: Re-add CLI entry point to pyproject.toml**

Add to `stages/window-detection/generator/pyproject.toml`:
```toml
[project.scripts]
window-gen = "window_gen.cli:main"
```

- [ ] **Step 4: Run tests**

Run: `uv run pytest stages/window-detection/generator/tests/test_cli.py -v`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add stages/window-detection/generator/
git commit -m "feat(window-gen): add CLI for VM-based training data generation"
```

---

## Task R4: Baseline Accuracy Evaluation on Real VM Screenshots

Replaces the deleted `test_baseline_accuracy.py`. This is an integration test that:
1. Captures real screenshots from the VM using the scenario library
2. Runs the heuristic detector on them
3. Reports precision/recall/F1 against programmatic ground truth

**Files:**
- Create: `stages/window-detection/analysis/tests/test_baseline_accuracy.py`

- [ ] **Step 1: Write accuracy test**

```python
# stages/window-detection/analysis/tests/test_baseline_accuracy.py
import json
from pathlib import Path

import pytest

from guivision_common.image_io import load_image
from guivision_common.metrics import compute_metrics
from guivision_common.types import Detection
from window_analysis.heuristic import detect_windows_heuristic
from window_gen.scenario_library import build_scenario_library
from window_gen.vm_capture import VMCaptureConfig, VMCaptureSession


def _get_connect_spec() -> Path:
    """Find the VM connection spec, or skip if not available."""
    candidates = [
        Path("vm-connection.json"),
        Path.home() / ".config" / "guivision" / "vm-connection.json",
    ]
    for p in candidates:
        if p.exists():
            return p
    pytest.skip("No vm-connection.json found — skipping VM-based accuracy test")


@pytest.mark.integration
class TestBaselineAccuracyOnRealScreenshots:
    """Measure heuristic detector accuracy on real VM screenshots.

    Run with: uv run pytest -m integration -s
    Requires: running macOS VM + vm-connection.json
    """

    def test_accuracy_on_scenario_library(self, tmp_path):
        spec_path = _get_connect_spec()
        config = VMCaptureConfig(connect_spec=spec_path)
        session = VMCaptureSession(config)

        scenarios = build_scenario_library()
        all_preds: list[Detection] = []
        all_gts: list[Detection] = []

        for scenario in scenarios:
            name = scenario.name
            img_path, gt = session.capture_scenario(scenario, tmp_path, name)
            img = load_image(img_path)

            preds = detect_windows_heuristic(img)
            gts = [d for d in gt.detections]

            all_preds.extend(preds)
            all_gts.extend(gts)

            scenario_result = compute_metrics(preds, gts, iou_threshold=0.5)
            print(
                f"  {name:25s}  "
                f"gt={len(gts):2d}  pred={len(preds):2d}  "
                f"P={scenario_result.precision:.2f}  R={scenario_result.recall:.2f}  "
                f"F1={scenario_result.f1:.2f}"
            )

        result = compute_metrics(all_preds, all_gts, iou_threshold=0.5)

        print(f"\n{'='*60}")
        print(f"HEURISTIC BASELINE — Window Detection (Real VM Screenshots)")
        print(f"{'='*60}")
        print(f"Scenarios:  {len(scenarios)}")
        print(f"GT windows: {len(all_gts)}")
        print(f"Predicted:  {len(all_preds)}")
        print(f"Precision:  {result.precision:.3f}")
        print(f"Recall:     {result.recall:.3f}")
        print(f"F1:         {result.f1:.3f}")
        print(f"TP={result.true_pos} FP={result.false_pos} FN={result.false_neg}")
        print(f"{'='*60}")

        # Baseline should detect at least something
        assert result.f1 > 0.0, "Heuristic detector should detect at least some windows"
```

- [ ] **Step 2: Commit**

```bash
git add stages/window-detection/analysis/tests/
git commit -m "test(window-analysis): add baseline accuracy evaluation on real VM screenshots"
```

---

## Task R5: Expand Scenario Library for Real-World Diversity

The current scenario library has 9 static layouts. For real VM captures, we should add:
1. **Randomised scenarios** — random app/position/size combinations for data variety
2. **Real app content** — scenarios that open files so windows have realistic content (not just empty docs)
3. **Resolution variants** — scenarios at common screen sizes (1920x1080, 2560x1440, 1440x900)

**Files:**
- Modify: `stages/window-detection/generator/src/window_gen/scenario_library.py`
- Modify: `stages/window-detection/generator/tests/test_scenario_library.py`

- [ ] **Step 1: Add randomised scenario generator**

```python
def build_random_scenarios(count: int = 20, seed: int = 42) -> list[WindowScenario]:
    """Generate randomised window layouts for training variety."""
    rng = random.Random(seed)
    scenarios = []
    apps = ["TextEdit", "Safari", "Finder", "Terminal", "Preview", "Notes", "Calculator"]

    for i in range(count):
        n_windows = rng.randint(1, 6)
        screen_w, screen_h = rng.choice([(1920, 1080), (2560, 1440), (1440, 900)])
        windows = []
        for z in range(n_windows):
            w = rng.randint(300, min(1200, screen_w))
            h = rng.randint(200, min(900, screen_h))
            x = rng.randint(0, screen_w - w)
            y = rng.randint(25, screen_h - h)  # 25px for menu bar
            app = rng.choice(apps)
            windows.append(WindowSpec(app, f"Window {z}", x, y, w, h, z_order=z))
        scenarios.append(WindowScenario(
            name=f"random-{i:03d}",
            description=f"Random layout with {n_windows} windows",
            screen_width=screen_w,
            screen_height=screen_h,
            windows=windows,
        ))
    return scenarios
```

- [ ] **Step 2: Update build_scenario_library to include random scenarios**

```python
def build_scenario_library(include_random: bool = True, random_count: int = 20) -> list[WindowScenario]:
    library = [_empty_desktop(), _single_centered_window(), ...]  # existing
    if include_random:
        library.extend(build_random_scenarios(count=random_count))
    return library
```

- [ ] **Step 3: Add tests for random scenarios**

- All random windows must be within screen bounds
- Random scenarios must produce diverse layouts (not all identical)
- Random scenarios must be deterministic given the same seed

- [ ] **Step 4: Run tests, commit**

---

## Task R6: End-to-End Integration Test

**Files:**
- Create: `stages/window-detection/generator/tests/test_integration.py`

- [ ] **Step 1: Write end-to-end integration test**

```python
@pytest.mark.integration
class TestEndToEnd:
    """Full pipeline: generate dataset from VM → convert to YOLO → verify structure."""

    def test_generate_and_convert(self, tmp_path):
        spec_path = _get_connect_spec()
        scenarios = build_scenario_library(include_random=False)[:3]  # just 3 for speed

        # Generate from VM
        config = DatasetConfig(connect_spec=spec_path, output_dir=tmp_path / "raw")
        generate_dataset(scenarios, config)

        # Convert to YOLO
        yolo_config = YOLODatasetConfig(
            manifest_path=tmp_path / "raw" / "manifest.json",
            output_dir=tmp_path / "yolo",
            train_ratio=1.0,
        )
        convert_to_yolo(yolo_config)

        # Verify YOLO structure
        assert (tmp_path / "yolo" / "data.yaml").exists()
        assert len(list((tmp_path / "yolo" / "images" / "train").glob("*.png"))) == 3
        assert len(list((tmp_path / "yolo" / "labels" / "train").glob("*.txt"))) == 3
```

- [ ] **Step 2: Commit**

---

## Updated CLAUDE.md

After completing these tasks, update `CLAUDE.md` to document the VM-based workflow:

```markdown
### Generating Training Data (requires running VM)

# Create vm-connection.json with your VM's VNC/SSH details
# Then generate the dataset:
uv run window-gen generate --connect vm-connection.json --output data/window-detection/

# Convert to YOLO format for training:
uv run python -c "
from window_train.yolo_format import convert_to_yolo, YOLODatasetConfig
from pathlib import Path
convert_to_yolo(YOLODatasetConfig(
    manifest_path=Path('data/window-detection/manifest.json'),
    output_dir=Path('data/window-detection/yolo'),
))
"
```

---

## Future: What About Environments Without a VM?

For CI or quick dev iteration where no VM is available, the approach is NOT to draw fake windows. Instead:
- **Golden dataset**: A pre-captured set of real VM screenshots + labels, committed to a separate data repo or stored in cloud storage, downloaded on demand.
- **Snapshot restore**: VM snapshots with known window configurations, so capture is fast (restore → screenshot → next snapshot).
- **Never synthesize**: If you can't screenshot a real OS, you don't have training data. That's a signal to set up the VM, not to fake the data.
