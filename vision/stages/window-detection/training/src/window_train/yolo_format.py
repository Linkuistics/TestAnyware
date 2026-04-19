from __future__ import annotations

import json
import shutil
from dataclasses import dataclass
from pathlib import Path

import yaml


CLASS_NAMES = ["window"]


@dataclass
class YOLODatasetConfig:
    manifest_path: Path
    output_dir: Path
    train_ratio: float = 0.8
    val_ratio: float = 0.2


def convert_to_yolo(config: YOLODatasetConfig) -> Path:
    source_dir = config.manifest_path.parent

    with open(config.manifest_path) as f:
        manifest = json.load(f)

    samples = manifest["samples"]
    split_idx = int(len(samples) * config.train_ratio)
    train_samples = samples[:split_idx] if split_idx < len(samples) else samples
    val_samples = samples[split_idx:] if split_idx < len(samples) else []

    for split_name, split_samples in [("train", train_samples), ("val", val_samples)]:
        if not split_samples:
            continue

        img_dir = config.output_dir / "images" / split_name
        lbl_dir = config.output_dir / "labels" / split_name
        img_dir.mkdir(parents=True, exist_ok=True)
        lbl_dir.mkdir(parents=True, exist_ok=True)

        for sample in split_samples:
            src_img = source_dir / sample["image"]
            src_lbl = source_dir / sample["label"]
            name = Path(sample["image"]).stem

            # Copy image
            shutil.copy2(src_img, img_dir / f"{name}.png")

            # Convert label to YOLO format
            with open(src_lbl) as f:
                gt = json.load(f)

            img_w = gt["image_width"]
            img_h = gt["image_height"]

            lines = []
            for det in gt["detections"]:
                bbox = det["bbox"]  # [x1, y1, x2, y2]
                cx = (bbox[0] + bbox[2]) / 2.0 / img_w
                cy = (bbox[1] + bbox[3]) / 2.0 / img_h
                w = (bbox[2] - bbox[0]) / img_w
                h = (bbox[3] - bbox[1]) / img_h
                class_id = CLASS_NAMES.index(det["label"])
                lines.append(f"{class_id} {cx:.6f} {cy:.6f} {w:.6f} {h:.6f}")

            (lbl_dir / f"{name}.txt").write_text("\n".join(lines))

    # Write data.yaml
    data_yaml = {
        "path": str(config.output_dir.resolve()),
        "train": "images/train",
        "val": "images/val" if val_samples else "images/train",
        "nc": len(CLASS_NAMES),
        "names": CLASS_NAMES,
    }
    yaml_path = config.output_dir / "data.yaml"
    with open(yaml_path, "w") as f:
        yaml.dump(data_yaml, f, default_flow_style=False)

    return yaml_path
