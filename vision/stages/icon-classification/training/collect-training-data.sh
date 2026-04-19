#!/bin/bash
#
# Collect icon training data from macOS screenshots.
#
# Uses the running TestAnyware pipeline to detect UI elements,
# then crops AXImage regions and saves them for manual labeling.
#
# Usage:
#   1. Start a TestAnyware VM session
#   2. Open various macOS apps (System Settings, Safari, Finder, etc.)
#   3. Run this script for each app window
#
# Example:
#   ./collect-training-data.sh "System Settings" /tmp/icon-training
#   ./collect-training-data.sh "Safari" /tmp/icon-training
#   ./collect-training-data.sh "Finder" /tmp/icon-training
#
# Output structure:
#   /tmp/icon-training/
#   ├── unlabeled/          ← review these and move to class folders
#   │   ├── SystemSettings_001.png
#   │   ├── SystemSettings_002.png
#   │   └── ...
#   ├── gear/               ← create folders for each icon class
#   ├── checkmark/
#   ├── chevron-right/
#   └── ...
#
# After collecting and labeling:
#   Open Create ML, create an Image Classifier project,
#   drag in the labeled folders as training data, and train.

set -euo pipefail

WINDOW="${1:?Usage: $0 WINDOW_TITLE OUTPUT_DIR}"
OUTPUT_DIR="${2:?Usage: $0 WINDOW_TITLE OUTPUT_DIR}"
TESTANYWARE="${TESTANYWARE:-testanyware}"
SERVER_URL="${TESTANYWARE_SERVER_URL:-http://localhost:9100}"

# Sanitize window name for filenames
SAFE_NAME=$(echo "$WINDOW" | tr -cs '[:alnum:]' '_' | sed 's/_$//')

# Create output directory
mkdir -p "$OUTPUT_DIR/unlabeled"

echo "Detecting elements in window: $WINDOW"

# Get elements via describe
RESPONSE=$(curl -s -X POST "$SERVER_URL/describe" \
  -H "Content-Type: application/json" \
  -d "{\"window\": \"$WINDOW\"}")

# Check for error
if echo "$RESPONSE" | python3 -c "import sys,json; d=json.load(sys.stdin); sys.exit(0 if 'elements' in d else 1)" 2>/dev/null; then
    :
else
    echo "Error: describe failed. Is the server running?"
    echo "$RESPONSE"
    exit 1
fi

# Extract AXImage element IDs and bboxes
IMAGES=$(echo "$RESPONSE" | python3 -c "
import sys, json
data = json.load(sys.stdin)
for e in data['elements']:
    if e['type'] == 'AXImage':
        bbox = e['bbox']
        # Skip very small icons (< 8px) and very large images (> 200px)
        if bbox[2] >= 8 and bbox[3] >= 8 and bbox[2] <= 200 and bbox[3] <= 200:
            print(f\"{e['id']} {bbox[0]} {bbox[1]} {bbox[2]} {bbox[3]}\")
" 2>/dev/null)

if [ -z "$IMAGES" ]; then
    echo "No AXImage elements found in window '$WINDOW'"
    exit 0
fi

COUNT=0
while IFS=' ' read -r ID X Y W H; do
    COUNT=$((COUNT + 1))
    FILENAME="${SAFE_NAME}_$(printf '%03d' $COUNT)_id${ID}.png"
    OUTPUT_PATH="$OUTPUT_DIR/unlabeled/$FILENAME"

    # Crop the element
    curl -s -X POST "$SERVER_URL/crop" \
      -H "Content-Type: application/json" \
      -d "{\"window\": \"$WINDOW\", \"elementId\": $ID}" \
      -o "$OUTPUT_PATH"

    echo "  Saved: $FILENAME (${W}x${H})"
done <<< "$IMAGES"

echo ""
echo "Collected $COUNT icon images → $OUTPUT_DIR/unlabeled/"
echo ""
echo "Next steps:"
echo "  1. Review images in $OUTPUT_DIR/unlabeled/"
echo "  2. Create class folders: mkdir -p $OUTPUT_DIR/{gear,checkmark,close-x,...}"
echo "  3. Move each image to the correct class folder"
echo "  4. Open Create ML → Image Classifier → drag labeled folders"
echo "  5. Train and export as IconClassifier.mlpackage"
echo "  6. Compile: xcrun coremlcompiler compile IconClassifier.mlpackage ."
echo "  7. Copy IconClassifier.mlmodelc to stages/icon-classification/data/icon_classifier.mlmodelc"
