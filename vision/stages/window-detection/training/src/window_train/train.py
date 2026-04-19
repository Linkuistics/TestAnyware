from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class TrainConfig:
    data_yaml: Path
    model: str = "yolov8s.pt"
    epochs: int = 100
    imgsz: int = 1280
    batch: int = -1  # auto-detect based on memory
    device: str = "mps"
    project: str = "runs/window-detection"
    name: str = "train"
    patience: int = 20
    save_period: int = 10
    workers: int = 8
    amp: bool = True
    export_coreml: bool = True


def validate_config(config: TrainConfig) -> list[str]:
    errors = []
    if not config.data_yaml.exists():
        errors.append(f"data_yaml does not exist: {config.data_yaml}")
    if config.imgsz % 32 != 0:
        errors.append(f"imgsz must be divisible by 32, got {config.imgsz}")
    if config.epochs < 1:
        errors.append(f"epochs must be >= 1, got {config.epochs}")
    return errors


def train(config: TrainConfig) -> Path:
    """Run YOLO training. Returns path to best weights."""
    errors = validate_config(config)
    if errors:
        raise ValueError(f"Invalid config: {'; '.join(errors)}")

    from ultralytics import YOLO

    model = YOLO(config.model)
    results = model.train(
        data=str(config.data_yaml),
        epochs=config.epochs,
        imgsz=config.imgsz,
        batch=config.batch,
        device=config.device,
        project=config.project,
        name=config.name,
        patience=config.patience,
        save_period=config.save_period,
        workers=config.workers,
        amp=config.amp,
    )

    best_weights = Path(config.project) / config.name / "weights" / "best.pt"

    if config.export_coreml and best_weights.exists():
        trained = YOLO(str(best_weights))
        trained.export(format="coreml", imgsz=config.imgsz, half=True, nms=True)

    return best_weights
