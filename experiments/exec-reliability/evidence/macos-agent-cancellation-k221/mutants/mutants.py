"""Discrimination mutants of leaf macos-agent-cancellation-k221.

    python3 -I mutants.py WORK OUT

WORK is a scratch copy of agents/macos. For each mutant the script replaces one
exact source fragment, runs the named test classes with `swift test`, records the
output and the exit, and restores the file byte for byte. A mutant whose fragment
is missing is recorded as `not-applied` and its tests do not run. The script judges
nothing beyond `killed`: the test run exited non-zero.
"""

import json
import subprocess
import sys
from pathlib import Path

OWNER = "Sources/TestAnywareAgent/ProcessRunner.swift"
REGISTRY = "Sources/TestAnywareAgent/ExecRegistry.swift"
READER = "Sources/TestAnywareAgent/SingleVersionFile.swift"
SERVER = "Sources/testanyware-agent/AgentServer.swift"

MUTANTS = [
    {
        "name": "owner-ignores-cancellation",
        "file": OWNER,
        "old": "cancelAt = cancel?.requestedAt\n        } while",
        "new": "cancelAt = nil\n        } while",
        "tests": "ExecCancellationTests/testCancellationEndsTheGroupAndRepliesWithTheRecord",
    },
    {
        "name": "cancellation-skips-the-kernel-wait",
        "file": OWNER,
        "old": "            if waiter.hasExited() {\n                (outcome, remaining) = settleExit(waiter, pid)\n                finaliseBy = outcomeAt + ProcessRunner.finaliseNs\n            } else {\n                outcome = cancelAt",
        "new": "            if cancelAt == nil && waiter.hasExited() {\n                (outcome, remaining) = settleExit(waiter, pid)\n                finaliseBy = outcomeAt + ProcessRunner.finaliseNs\n            } else {\n                outcome = cancelAt",
        "tests": "ExecCancellationTests/testTheExitAndCancelRaceResolvesByKernelState",
    },
    {
        "name": "shutdown-cancels-nothing",
        "file": REGISTRY,
        "old": "                signal.request(at: now)\n                running.append(id)",
        "new": "                running.append(id)",
        "tests": "AgentProcessTests/testSigtermDuringACommandEndsTheGroupAndExitsInBounds",
    },
    {
        "name": "download-reopens-the-path",
        "file": READER,
        "old": "            let count = data.withUnsafeMutableBytes { pread(fd, $0.baseAddress, want, off_t(offset)) }",
        "new": "            let again = Darwin.open(path, O_RDONLY)\n            defer { Darwin.close(again) }\n            let count = data.withUnsafeMutableBytes { pread(again, $0.baseAddress, want, off_t(offset)) }",
        "extra": [
            ("    private var offset = 0\n", "    private var offset = 0\n    private var path = \"\"\n"),
            ("        self.fd = fd\n", "        self.fd = fd\n        self.path = path\n"),
        ],
        "tests": "SingleVersionFileTests/testARenameReplacementDuringTheReadGivesTheVersionOpenAtTheStart",
    },
    {
        "name": "eviction-ignores-the-limit",
        "file": REGISTRY,
        "old": "for (id, _) in retained.prefix(max(0, retained.count - limit)) {",
        "new": "for (id, _) in retained.prefix(0) {",
        "tests": "ExecRegistryTests/testTheOldestRetainedEntryGoesFirstBeyond64",
    },
]


def main() -> None:
    work, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=False)
    results = []
    for mutant in MUTANTS:
        path = work / mutant["file"]
        original = path.read_bytes()
        text = original.decode()
        edits = [(mutant["old"], mutant["new"]), *mutant.get("extra", [])]
        if any(text.count(old) != 1 for old, _ in edits):
            results.append({"name": mutant["name"], "result": "not-applied"})
            continue
        for old, new in edits:
            text = text.replace(old, new)
        path.write_text(text)
        try:
            done = subprocess.run(
                ["swift", "test", "--package-path", str(work), "--filter", mutant["tests"]],
                capture_output=True,
                timeout=1200,
                check=False,
            )
            (out / f"{mutant['name']}.log").write_bytes(done.stdout + done.stderr)
            results.append(
                {
                    "name": mutant["name"],
                    "tests": mutant["tests"],
                    "exit": done.returncode,
                    "result": "killed" if done.returncode != 0 else "survived",
                }
            )
        finally:
            path.write_bytes(original)
    (out / "results.json").write_text(json.dumps(results, indent=2) + "\n")
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
