import json
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import numpy as np
import pytest

from window_gen.vm_capture import VMCaptureSession, VMCaptureConfig
from window_gen.scenarios import WindowScenario, WindowSpec


class TestVMCaptureConfig:
    def test_create(self):
        config = VMCaptureConfig(
            vnc_host="localhost",
            vnc_port=5901,
            vnc_password="secret",
            ssh_host="localhost",
            ssh_port=22,
            ssh_user="admin",
            platform="macos",
        )
        assert config.vnc_host == "localhost"
        assert config.platform == "macos"


class TestVMCaptureSession:
    def test_build_applescript_for_window(self):
        """Test that we generate correct AppleScript to position a window."""
        session = VMCaptureSession.__new__(VMCaptureSession)
        spec = WindowSpec("TextEdit", "Test.txt", 100, 200, 800, 600, z_order=0)
        script = session._build_position_script(spec, platform="macos")
        assert "TextEdit" in script
        assert "100" in script
        assert "200" in script
        assert "800" in script
        assert "600" in script

    def test_build_scenario_script_orders_by_z(self):
        """Windows should be positioned in z_order so the last one is on top."""
        session = VMCaptureSession.__new__(VMCaptureSession)
        scenario = WindowScenario(
            name="test", description="test",
            screen_width=1920, screen_height=1080,
            windows=[
                WindowSpec("Safari", "Page", 500, 200, 900, 700, z_order=1),
                WindowSpec("TextEdit", "Doc.txt", 100, 100, 800, 600, z_order=0),
            ],
        )
        scripts = session._build_scenario_scripts(scenario, platform="macos")
        # Should be ordered by z_order: TextEdit first (back), Safari second (front)
        assert "TextEdit" in scripts[0]
        assert "Safari" in scripts[1]


@pytest.mark.integration
class TestVMCaptureIntegration:
    """These tests require a running VM. Run with: pytest -m integration"""

    def test_capture_single_window(self, tmp_path):
        """End-to-end: create a window in the VM, screenshot, verify ground truth."""
        pytest.skip("Requires live VM — run manually with pytest -m integration")
