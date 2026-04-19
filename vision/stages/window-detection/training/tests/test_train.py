import pytest
from pathlib import Path

from window_train.train import TrainConfig, validate_config


class TestTrainConfig:
    def test_create_default(self):
        config = TrainConfig(data_yaml=Path("/tmp/data.yaml"))
        assert config.model == "yolov8s.pt"
        assert config.epochs == 100
        assert config.imgsz == 1280
        assert config.device == "mps"

    def test_validate_missing_data_yaml(self, tmp_path):
        config = TrainConfig(data_yaml=tmp_path / "nonexistent.yaml")
        errors = validate_config(config)
        assert any("data_yaml" in e for e in errors)

    def test_validate_valid_config(self, tmp_path):
        yaml_path = tmp_path / "data.yaml"
        yaml_path.write_text("nc: 1\nnames: [window]\n")
        config = TrainConfig(data_yaml=yaml_path)
        errors = validate_config(config)
        assert len(errors) == 0

    def test_validate_bad_imgsz(self, tmp_path):
        yaml_path = tmp_path / "data.yaml"
        yaml_path.write_text("nc: 1\nnames: [window]\n")
        config = TrainConfig(data_yaml=yaml_path, imgsz=123)
        errors = validate_config(config)
        assert any("imgsz" in e for e in errors)
