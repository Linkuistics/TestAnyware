"""`python -m ocr_analyzer --daemon` entry point.

The Rust bridge always launches with ``--daemon`` (`OcrChildBridgeConfig::new`).
The flag is required (not assumed) so a stray ``python -m ocr_analyzer`` fails
loudly rather than hanging on stdin.
"""

import sys

from .daemon import build_easyocr_reader, serve


def main(argv: list[str]) -> int:
    if "--daemon" not in argv:
        sys.stderr.write("usage: python -m ocr_analyzer --daemon\n")
        return 2
    serve(build_easyocr_reader, stdin=sys.stdin, stdout=sys.stdout)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
