# Icon Classifier — Training Workflow

**Status:** model not yet trained. Shape-analysis heuristic used as fallback.

The icon classifier identifies ~52 common UI symbols (gear, checkmark, close-x, chevrons, etc.) on cropped button regions. Until a trained model is available in `../data/icon_classifier.mlmodelc` (or `../data/icon_classifier.onnx`), classification falls back to the shape-analysis heuristic in `src/icon_classification/shape_analysis.py`, which handles ~8 geometric icons.

This document describes how to collect training data, label it, train the model, and bundle it.

## Prerequisites

- A running TestAnyware VM session (`testanyware vm start`)
- macOS with Create ML (included with Xcode)
- The `collect-training-data.sh` script in this directory

## Step 1: Collect Training Data

Open diverse macOS apps inside the VM to get a variety of icon styles:

```bash
# Open several apps in the VM
testanyware exec "open -a 'System Settings'"
testanyware exec "open -a 'Safari'"
testanyware exec "open -a 'Finder'"
testanyware exec "open -a 'Xcode'"
testanyware exec "open -a 'Mail'"
testanyware exec "open -a 'Calendar'"
testanyware exec "open -a 'Music'"
testanyware exec "open -a 'Terminal'"
testanyware exec "open -a 'App Store'"
testanyware exec "open -a 'Notes'"
```

Run the collection script for each app window:

```bash
./collect-training-data.sh "System Settings" /tmp/icon-training
./collect-training-data.sh "Safari" /tmp/icon-training
./collect-training-data.sh "Finder" /tmp/icon-training
./collect-training-data.sh "Xcode" /tmp/icon-training
./collect-training-data.sh "Mail" /tmp/icon-training
./collect-training-data.sh "Calendar" /tmp/icon-training
./collect-training-data.sh "Music" /tmp/icon-training
./collect-training-data.sh "App Store" /tmp/icon-training
./collect-training-data.sh "Notes" /tmp/icon-training
```

Navigate to different views within each app and re-run the script to capture more icons. The script saves cropped AXImage elements (8-200px) to `/tmp/icon-training/unlabeled/`.

## Step 2: Label the Images

Review images in `unlabeled/` and sort them into class folders:

```bash
# Create folders for each icon class
cd /tmp/icon-training
mkdir -p arrow-{down,left,right,up} battery bell bluetooth calendar camera
mkdir -p checkmark chevron-{down,left,right,up} close-x cloud document
mkdir -p download edit-pencil ellipsis external-link eye eye-slash
mkdir -p folder gear hamburger-menu heart home info-circle link lock
mkdir -p magnifying-glass microphone minus pause person play plus
mkdir -p question-circle refresh share skip-{back,forward} star stop-media
mkdir -p trash unlock upload volume-{off,up} warning-triangle wifi
```

Move each image to the correct class folder. Discard images that don't match any icon class (decorative images, logos, photos, etc.).

**Target:** 30+ samples per class across the ~52 classes.

**Tips:**
- Navigate to different macOS views and app states to get icon variation
- Include both light and dark mode versions if possible
- Include icons at different sizes (toolbar, sidebar, status bar)
- Don't force-fit ambiguous images — skip them

## Step 3: Train with Create ML

1. Open **Create ML** (Xcode menu → Open Developer Tool → Create ML)
2. Create a new **Image Classifier** project
3. Set the training data folder to `/tmp/icon-training` (the parent folder containing class subfolders)
4. Train the model
5. Review metrics — aim for >85% validation accuracy
6. Export as `IconClassifier.mlpackage`

## Step 4: Bundle the Model

```bash
# Compile the model
xcrun coremlcompiler compile IconClassifier.mlpackage .

# Copy into the stage's data directory
cp -r IconClassifier.mlmodelc ../data/icon_classifier.mlmodelc

# (Optional) convert to ONNX for cross-platform use
# python -m coremltools --convert IconClassifier.mlpackage --output ../data/icon_classifier.onnx

# Verify the pipeline loads it
uv run pytest stages/icon-classification/ -v
```

After bundling, `IconClassifier` will load the model at construction time and use it for all icon classification. The shape-analysis heuristic continues to run as a fallback when the model's confidence is below the configured threshold.

## Icon Vocabulary

The classifier uses a fixed vocabulary of 52 icon classes (see `src/icon_classification/vocabulary.py`):

| Category | Icons |
|----------|-------|
| Navigation | chevron-left/right/up/down, arrow-left/right/up/down, home, external-link |
| Actions | close-x, plus, minus, checkmark, edit-pencil, share, download, upload, trash, refresh, link |
| Objects | gear, magnifying-glass, star, heart, person, lock, unlock, bell, calendar, camera, microphone, folder, document |
| Status | eye, eye-slash, info-circle, question-circle, warning-triangle |
| Media | play, pause, stop-media, skip-forward, skip-back, volume-up, volume-off |
| System | wifi, bluetooth, battery, cloud, hamburger-menu, ellipsis |

## Shape Analysis (No Model Required)

Even without the trained model, shape-analysis heuristics detect these geometric icons:

- **plus** (+) — cross pattern with horizontal and vertical bars
- **minus** (-) — horizontal bar only
- **close-x** (x) — diagonal cross pattern
- **checkmark** (check) — bottom-heavy shape
- **chevron-left/right/up/down** — directional foreground concentration

Shape analysis uses Otsu thresholding for binarization and quadrant/strip analysis for shape detection.
