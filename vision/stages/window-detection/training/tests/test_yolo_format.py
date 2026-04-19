import json
from pathlib import Path

import pytest
import yaml

from window_train.yolo_format import convert_to_yolo, YOLODatasetConfig


class TestConvertToYOLO:
    def _make_manifest(self, tmp_path):
        """Create a minimal manifest with one sample."""
        images_dir = tmp_path / "source" / "images"
        labels_dir = tmp_path / "source" / "labels"
        images_dir.mkdir(parents=True)
        labels_dir.mkdir(parents=True)

        # Create a dummy image file
        (images_dir / "sample_0000.png").write_bytes(b"fake_png_data")

        # Create a ground truth label
        gt = {
            "stage": "window-detection",
            "image_path": "images/sample_0000.png",
            "image_width": 1920,
            "image_height": 1080,
            "detections": [
                {
                    "label": "window",
                    "bbox": [100, 200, 900, 800],
                    "confidence": 1.0,
                    "metadata": {"title": "Test"},
                }
            ],
            "sources": ["programmatic"],
        }
        with open(labels_dir / "sample_0000.json", "w") as f:
            json.dump(gt, f)

        manifest = {
            "stage": "window-detection",
            "num_samples": 1,
            "samples": [
                {"image": "images/sample_0000.png", "label": "labels/sample_0000.json"}
            ],
        }
        manifest_path = tmp_path / "source" / "manifest.json"
        with open(manifest_path, "w") as f:
            json.dump(manifest, f)

        return manifest_path

    def test_creates_yolo_directory_structure(self, tmp_path):
        manifest_path = self._make_manifest(tmp_path)
        output_dir = tmp_path / "yolo"
        config = YOLODatasetConfig(
            manifest_path=manifest_path,
            output_dir=output_dir,
            train_ratio=1.0,
        )
        convert_to_yolo(config)

        assert (output_dir / "images" / "train").exists()
        assert (output_dir / "labels" / "train").exists()
        assert (output_dir / "data.yaml").exists()

    def test_yolo_label_format(self, tmp_path):
        manifest_path = self._make_manifest(tmp_path)
        output_dir = tmp_path / "yolo"
        config = YOLODatasetConfig(
            manifest_path=manifest_path,
            output_dir=output_dir,
            train_ratio=1.0,
        )
        convert_to_yolo(config)

        label_files = list((output_dir / "labels" / "train").glob("*.txt"))
        assert len(label_files) == 1

        content = label_files[0].read_text().strip()
        parts = content.split()
        assert len(parts) == 5  # class_id cx cy w h

        class_id = int(parts[0])
        cx, cy, w, h = float(parts[1]), float(parts[2]), float(parts[3]), float(parts[4])

        assert class_id == 0  # "window" is class 0
        # Normalised coordinates should be in [0, 1]
        assert 0.0 <= cx <= 1.0
        assert 0.0 <= cy <= 1.0
        assert 0.0 < w <= 1.0
        assert 0.0 < h <= 1.0

        # Check actual values: bbox [100,200,900,800] on 1920x1080
        # center_x = (100+900)/2/1920 = 500/1920 ≈ 0.2604
        # center_y = (200+800)/2/1080 = 500/1080 ≈ 0.4630
        # width = (900-100)/1920 ≈ 0.4167
        # height = (800-200)/1080 ≈ 0.5556
        assert abs(cx - 500 / 1920) < 0.001
        assert abs(cy - 500 / 1080) < 0.001

    def test_data_yaml_contents(self, tmp_path):
        manifest_path = self._make_manifest(tmp_path)
        output_dir = tmp_path / "yolo"
        config = YOLODatasetConfig(
            manifest_path=manifest_path,
            output_dir=output_dir,
            train_ratio=1.0,
        )
        convert_to_yolo(config)

        with open(output_dir / "data.yaml") as f:
            data = yaml.safe_load(f)

        assert data["nc"] == 1
        assert data["names"] == ["window"]
        assert "train" in data
