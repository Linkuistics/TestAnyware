# Execution reliability tests

This directory contains the diagnostic controller, acceptance runners, receipt
checkers and regression tests formerly in `experiments/exec-reliability`.
The `k…` names identify the investigation that introduced each check; they are
kept so the reports can still identify the tools.

From the repository root:

```sh
task diagnostic:test
task diagnostic:self-check OUT="$PWD/target/diagnostics/self-check-01"
```

The default test task runs the controller and acceptance controls configured in
`pyproject.toml`. To include all probe/checker regression tests:

```sh
cd tests/exec-reliability
uv run --locked python -m pytest -q test_*.py
```

Live VM tasks remain in `Taskfile.yml`; inspect their descriptions and required
arguments with `task --list`. They require the appropriate host tools, agents
and golden images. The macOS download investigation also expects the original
Ravel-Strategies evidence checkout; its historical path is documented in
`download_k7.py`. These are not prerequisites for the local regression suite.

## Fixtures and output

`fixtures/` holds the recorded runs, guest scripts and raw HTTP replies consumed
by these tests and by the Rust download regression tests. Its `manifest.json`
records each original SHA-256 and the source commit. Preserve those bytes;
tests copy records to temporary directories before modifying them.

Write new run output beneath the ignored `target/diagnostics/` directory, or
outside the checkout. Do not commit whole run directories, copied source trees,
subject-hash inventories or generated binaries. Promote only a minimal fixture
needed by a regression test, with its provenance, and a concise result report.

The full historical evidence, failed samples, verdicts and frozen replay tools
are recoverable using the [archive instructions](../../docs/experiments/README.md).
The investigation-specific tasks that ran those archived tools are available
in that checkout's `Taskfile.yml`.
