from __future__ import annotations

import json
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

from testanyware_common.image_io import load_image
from testanyware_common.types import GroundTruth, GroundTruthSource
from window_gen.scenarios import WindowScenario, WindowSpec


@dataclass
class VMCaptureConfig:
    vnc_host: str
    vnc_port: int
    vnc_password: str | None = None
    ssh_host: str | None = None
    ssh_port: int = 22
    ssh_user: str = "admin"
    platform: str = "macos"
    testanyware_cli: str = "testanyware"  # path to TestAnywareVMDriver CLI


class VMCaptureSession:
    """Drives a VM to create window scenarios and capture screenshots."""

    def __init__(self, config: VMCaptureConfig):
        self.config = config

    def capture_scenario(
        self,
        scenario: WindowScenario,
        output_dir: Path,
        sample_name: str,
    ) -> tuple[Path, GroundTruth]:
        """Create the scenario in the VM, capture screenshot, return (image_path, ground_truth)."""
        # 1. Close all existing windows
        self._run_ssh("osascript -e 'tell application \"System Events\" to keystroke \"w\" using {command down, option down}'")

        # 2. Position windows according to scenario (ordered by z_order)
        scripts = self._build_scenario_scripts(scenario, self.config.platform)
        for script in scripts:
            self._run_ssh(f"osascript -e '{script}'")

        # 3. Brief pause for windows to settle
        self._run_ssh("sleep 0.5")

        # 4. Capture screenshot via TestAnywareVMDriver
        img_path = output_dir / "images" / f"{sample_name}.png"
        img_path.parent.mkdir(parents=True, exist_ok=True)
        self._capture_screenshot(img_path)

        # 5. Build ground truth from programmatic knowledge
        gt = scenario.to_ground_truth(image_path=f"images/{sample_name}.png")

        # 6. Optionally enrich with agent data if agent is available
        agent_gt = self._query_agent_windows()
        if agent_gt:
            gt.sources.append(GroundTruthSource.AGENT)

        # 7. Save ground truth
        label_path = output_dir / "labels" / f"{sample_name}.json"
        label_path.parent.mkdir(parents=True, exist_ok=True)
        with open(label_path, "w") as f:
            json.dump(gt.to_dict(), f, indent=2)

        return img_path, gt

    def _build_scenario_scripts(
        self, scenario: WindowScenario, platform: str
    ) -> list[str]:
        sorted_windows = sorted(scenario.windows, key=lambda w: w.z_order)
        return [self._build_position_script(w, platform) for w in sorted_windows]

    def _build_position_script(self, spec: WindowSpec, platform: str) -> str:
        if platform == "macos":
            return (
                f'tell application "{spec.app_name}" to activate\n'
                f'tell application "System Events" to tell process "{spec.app_name}"\n'
                f"  set position of front window to {{{spec.x}, {spec.y}}}\n"
                f"  set size of front window to {{{spec.width}, {spec.height}}}\n"
                f"end tell"
            )
        elif platform == "windows":
            # PowerShell approach — to be implemented with Windows agent
            return f"# Windows positioning for {spec.app_name} — requires agent"
        else:
            return f"# Linux positioning for {spec.app_name} — requires wmctrl or agent"

    def _capture_screenshot(self, output_path: Path) -> None:
        cmd = [
            self.config.testanyware_cli,
            "screenshot",
            "--host", self.config.vnc_host,
            "--port", str(self.config.vnc_port),
            "--output", str(output_path),
        ]
        if self.config.vnc_password:
            cmd.extend(["--password", self.config.vnc_password])
        subprocess.run(cmd, check=True, capture_output=True)

    def _run_ssh(self, command: str) -> str:
        if not self.config.ssh_host:
            return ""
        cmd = [
            "ssh",
            "-o", "StrictHostKeyChecking=no",
            "-p", str(self.config.ssh_port),
            f"{self.config.ssh_user}@{self.config.ssh_host}",
            command,
        ]
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        return result.stdout

    def _query_agent_windows(self) -> list[dict] | None:
        """Query guest agent for window list — returns None if agent unavailable."""
        # Agent integration will be implemented as part of the agents/ subproject
        return None
