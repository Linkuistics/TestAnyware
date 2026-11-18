"""Old-code controls of leaf macos-agent-cancellation-k238.

    python3 -I reverts.py WORK OUT

WORK is a scratch copy of agents/macos with the k238 repairs. For each case the
script puts back one exact fragment of the producer code of k221 (commit
45fb4da1), runs the named tests with `swift test`, records the output and the
exit, and restores the file byte for byte. A case whose fragment is missing is
recorded as `not-applied`, and its tests do not run.

Each case states the exit that shows the old defect. `fail` means the new
control fails on the old fragment. `pass` means the old gate passes although
the agent cannot serve, which is the defect of review finding R4. The script
records `as-expected` or `unexpected` and judges nothing more.
"""

import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

OWNER = "Sources/TestAnywareAgent/ProcessRunner.swift"
REGISTRY = "Sources/TestAnywareAgent/ExecRegistry.swift"
READER = "Sources/TestAnywareAgent/SingleVersionFile.swift"
FIXTURE = "Tests/TestAnywareAgentTests/AgentProcessTests.swift"

SKIP_OLD = (
    "        process.terminate()\n"
    "        let output = (try? String(contentsOf: process.log, encoding: .utf8)) ?? \"\"\n"
    "        process.stop()\n"
    "        throw NotHealthy(binary: binary, output: output)\n"
)

CASES = [
    {
        "name": "r1-long-wait-without-a-wake-source",
        "file": OWNER,
        "old": "let longest = wake == nil || watching ? Self.longestWaitNs : UInt64(Self.fallbackPollUs) * 1_000",
        "new": "let longest = Self.longestWaitNs",
        "tests": "ExecCancellationTests/testACancellation(WithoutAPipe|WhoseReadSourceFails)",
        "expect": "fail",
    },
    {
        "name": "r2-blocking-open",
        "file": READER,
        "old": "Darwin.open(path, O_RDONLY | O_CLOEXEC | O_NONBLOCK | O_NOCTTY)",
        "new": "Darwin.open(path, O_RDONLY | O_CLOEXEC)",
        "tests": "SingleVersionFileTests/testAFIFOWithoutAWriterIsRejectedAtOnce|AgentProcessTests/testADownloadOfAFIFOAnswers404AtOnce",
        "expect": "fail",
    },
    {
        "name": "r3-waiter-reads-the-id",
        "file": REGISTRY,
        "old": "if let reply = run.reply { return (.terminal, reply) }",
        "new": "if case .terminal(let reply)? = entries[id]?.state { return (.terminal, reply) }",
        "tests": "ExecRegistryTests/testAWaitingCancellationReturnsItsOwnRecordAfterEvictionAndReuse",
        "expect": "fail",
    },
    {
        "name": "r4-startup-skips",
        "file": FIXTURE,
        "old": SKIP_OLD,
        "new": (
            "        process.stop()\n"
            "        let output = (try? String(contentsOf: process.log, encoding: .utf8)) ?? \"\"\n"
            "        throw XCTSkip(\"the agent \\(binary) did not answer /health: \\(output)\")\n"
        ),
        "tests": "AgentProcessStartTests",
        "expect": "fail",
    },
    {
        "name": "r4-log-removed-first",
        "file": FIXTURE,
        "old": SKIP_OLD,
        "new": (
            "        process.stop()\n"
            "        let output = (try? String(contentsOf: process.log, encoding: .utf8)) ?? \"\"\n"
            "        throw NotHealthy(binary: binary, output: output)\n"
        ),
        "tests": "AgentProcessStartTests",
        "expect": "fail",
    },
    {
        "name": "r4-old-gate-with-a-broken-agent",
        "file": FIXTURE,
        "old": SKIP_OLD,
        "new": (
            "        process.stop()\n"
            "        let output = (try? String(contentsOf: process.log, encoding: .utf8)) ?? \"\"\n"
            "        throw XCTSkip(\"the agent \\(binary) did not answer /health: \\(output)\")\n"
        ),
        "tests": "AgentProcessTests",
        "env": {"TESTANYWARE_AGENT_UNDER_TEST": "/usr/bin/false"},
        "expect": "pass",
    },
    {
        "name": "r4-new-gate-with-a-broken-agent",
        "file": None,
        "tests": "AgentProcessTests",
        "env": {"TESTANYWARE_AGENT_UNDER_TEST": "/usr/bin/false"},
        "expect": "fail",
    },
]


def digests(root: Path) -> dict[str, str]:
    result = {}
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if path.is_file() and relative.parts[0] not in (".build", ".swiftpm"):
            result[str(relative)] = hashlib.sha256(path.read_bytes()).hexdigest()
    return result


def main() -> None:
    work, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=False)
    before = digests(work)
    (out / "work-before.json").write_text(json.dumps(before, indent=2) + "\n")
    results = []
    for case in CASES:
        path = work / case["file"] if case["file"] else None
        original = path.read_bytes() if path else None
        if path:
            text = original.decode()
            if text.count(case["old"]) != 1:
                results.append({"name": case["name"], "result": "not-applied"})
                continue
            path.write_text(text.replace(case["old"], case["new"]))
        try:
            done = subprocess.run(
                ["swift", "test", "--package-path", str(work), "--filter", case["tests"]],
                capture_output=True,
                timeout=1200,
                check=False,
                env={**os.environ, **case.get("env", {})},
            )
            (out / f"{case['name']}.log").write_bytes(done.stdout + done.stderr)
            observed = "pass" if done.returncode == 0 else "fail"
            results.append(
                {
                    "name": case["name"],
                    "tests": case["tests"],
                    "env": case.get("env", {}),
                    "exit": done.returncode,
                    "expect": case["expect"],
                    "result": "as-expected" if observed == case["expect"] else "unexpected",
                }
            )
        finally:
            if path:
                path.write_bytes(original)
    after = digests(work)
    (out / "work-after.json").write_text(json.dumps(after, indent=2) + "\n")
    summary = {"work-unchanged": before == after, "cases": results}
    (out / "results.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
