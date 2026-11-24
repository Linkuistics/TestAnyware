# Execution reliability investigation archive

These reports record the execution reliability investigation and its acceptance
trial. The maintained harness now lives in
[`tests/exec-reliability`](../../tests/exec-reliability/README.md).
Short reports originally stored alongside raw evidence are preserved under
[`retained-reports/`](retained-reports/).

## Complete evidence snapshot

The complete pre-cleanup tree is preserved in Git commit
`1156f0f6eadd59936f63c73137cb95bac94f4493`, an ancestor of `main`.
It includes every verdict, failed sample, successful run, frozen source input
and investigation runner under
[`experiments/exec-reliability/evidence`](https://github.com/Linkuistics/TestAnyware/tree/1156f0f6eadd59936f63c73137cb95bac94f4493/experiments/exec-reliability/evidence).
The cleanup removes the bulk archive from current checkouts; it does not rewrite
Git history or reduce the size of a full historical clone.

Paths, source line numbers and commands in the historical reports refer to that
snapshot. Use its matching tools and Taskfile when replaying an old condition;
do not substitute today's source into a frozen evidence check. The reports'
hashes and absolute recorded paths have not been rewritten.

To read one archived file without restoring the directory:

```sh
git show 1156f0f6eadd59936f63c73137cb95bac94f4493:experiments/exec-reliability/evidence/lifecycle-acceptance-k225/trial-01/trial.json
```

To restore a separate matching checkout, including the Git metadata used by
some runners:

```sh
archive_dir=$(mktemp -d /tmp/testanyware-evidence.XXXXXX)
git worktree add --detach "$archive_dir" 1156f0f6eadd59936f63c73137cb95bac94f4493
```

A shallow clone may need to fetch the historical commit first:

```sh
git fetch origin 1156f0f6eadd59936f63c73137cb95bac94f4493
```

Current regression tests retain only their required inputs under
`tests/exec-reliability/fixtures/`, with a SHA-256 manifest referencing the
same snapshot. New bulk evidence belongs in ignored `target/diagnostics/`
or an external archive, with only minimal regression fixtures and concise
reports committed.
