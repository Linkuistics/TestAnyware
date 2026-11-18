#!/bin/bash
# Review integration k234: the final measurement of the repaired macOS owner.
# One writer, in sequence. Subjects are hashed before and after the run.
set -u
ROOT=$(cd "$(dirname "$0")/../../../.." && pwd)
E="$ROOT/experiments/exec-reliability/evidence/macos-agent-lifecycle-k234"
cd "$ROOT"
subjects() {
  shasum -a 256 \
    agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift \
    agents/macos/Sources/TestAnywareAgent/ExecRecordMapping.swift \
    agents/macos/Sources/TestAnywareAgentProtocol/ExecRecord.swift \
    agents/macos/Sources/testanyware-agent/AgentServer.swift \
    agents/macos/Tests/TestAnywareAgentTests/*.swift \
    agents/macos/Package.resolved Taskfile.yml \
    experiments/exec-reliability/outcomes_k12*.py \
    experiments/exec-reliability/evidence/macos-completion-k3/old_path.py
}
subjects > "$E/subjects-before.txt"
echo "source $(jj log -r @- --no-graph -T 'commit_id') plus working-copy diff" > "$E/source.txt"
jj diff --git > "$E/working-copy.diff"

mise exec -- task diagnostic:macos-completion-old-path \
  OUT="$E/completion-new-clean" EXPECT=clean > "$E/completion-new-clean.log" 2>&1
echo "completion EXPECT=clean exit $?" > "$E/results.txt"

mise exec -- task diagnostic:macos-exec-outcomes OUT="$E/outcomes" > "$E/outcomes.log" 2>&1
echo "outcomes run exit $?" >> "$E/results.txt"
mise exec -- task diagnostic:macos-exec-outcomes-check OUT="$E/outcomes" > "$E/outcomes-check.log" 2>&1
echo "outcomes check exit $?" >> "$E/results.txt"
mise exec -- task diagnostic:macos-exec-outcomes-record OUT="$E/outcomes" > "$E/outcomes-record.log" 2>&1
echo "outcomes record exit $?" >> "$E/results.txt"

mkdir -p "$E/agent-test-20"
for n in $(seq -w 1 20); do
  start=$(date +%s)
  mise exec -- task agent:macos:test > "$E/agent-test-20/run-$n.log" 2>&1
  rc=$?
  line=$(grep -E 'Executed [0-9]+ tests' "$E/agent-test-20/run-$n.log" | tail -1 | sed 's/^[[:space:]]*//')
  echo "run $n exit $rc seconds $(( $(date +%s) - start )) $line" >> "$E/agent-test-20/results.txt"
done

subjects > "$E/subjects-after.txt"
if cmp -s "$E/subjects-before.txt" "$E/subjects-after.txt"; then
  echo "subjects unchanged" >> "$E/results.txt"
else
  echo "subjects CHANGED" >> "$E/results.txt"
fi
echo done >> "$E/results.txt"
