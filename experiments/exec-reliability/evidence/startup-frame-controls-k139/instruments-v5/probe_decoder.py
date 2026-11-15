"""Retain decoder thresholds independently of pytest and worker framing."""

import json
import sys

print(json.dumps({"python": sys.version, "recursion_limit": sys.getrecursionlimit()}))
for limit in (100, 1000, 3000):
    sys.setrecursionlimit(limit)
    for depth in (1100, 2040, 3000, 5000, 10000):
        frame = b"[" * depth + b"0" + b"]" * depth
        try:
            value = json.loads(frame)
            result = "decoded"
        except RecursionError as error:
            result = str(error)
        print(
            json.dumps({"limit": limit, "depth": depth, "bytes": len(frame), "result": result}),
            flush=True,
        )
