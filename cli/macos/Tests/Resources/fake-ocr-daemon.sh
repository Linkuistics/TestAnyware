#!/bin/bash
# Configurable fake Python daemon for OCRChildBridge tests.
# Reads $FAKE_OCR_BEHAVIOR to select behavior mode.

case "${FAKE_OCR_BEHAVIOR}" in
    ready_then_echo)
        echo '{"ready": true}'
        while IFS= read -r line; do
            echo '{"detections":[{"text":"fake","x":0,"y":0,"width":10,"height":10,"confidence":0.99}],"engine":"fake"}'
        done
        ;;
    ready_then_die)
        echo '{"ready": true}'
        exit 1
        ;;
    ready_then_die_on_request)
        echo '{"ready": true}'
        IFS= read -r line
        exit 1
        ;;
    import_error)
        echo "ModuleNotFoundError: No module named 'easyocr'" >&2
        exit 1
        ;;
    hang_forever)
        sleep 86400
        ;;
    ready_then_malformed)
        echo '{"ready": true}'
        while IFS= read -r line; do
            echo 'not-valid-json'
        done
        ;;
    ready_then_hang)
        echo '{"ready": true}'
        IFS= read -r line
        sleep 86400
        ;;
    permission_denied)
        echo "Permission denied" >&2
        exit 126
        ;;
    *)
        echo "Unknown FAKE_OCR_BEHAVIOR: ${FAKE_OCR_BEHAVIOR}" >&2
        exit 2
        ;;
esac
