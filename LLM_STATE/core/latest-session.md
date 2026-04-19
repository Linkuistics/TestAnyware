### Session 7 (2026-04-19T07:55:35Z) — P5: guivision vm delete + vm-delete.sh wrapper

- Implemented Task 3 P5 (vm delete subcommand) as the final phase of the guivision vm subcommands refactor
- Added `QEMURunner.deleteGolden(name:paths:)`, `runningClonesBacked(byGoldenName:paths:)`, and private `backingFile(ofQcow2:)` (uses `qemu-img info --output=json` + `JSONSerialization`, replacing bash's `python3 -c`)
- Added `TartRunner.deleteGolden(name:)` and `TartRunner.runningClones()` for tart backend
- Added `VMLifecycle.delete(name:force:)`: auto-detects backend (tart if name in `runList()`, else qemu if `.qcow2` exists), refuses with `runningClonesPresent` when live clones detected unless `--force`, three new error cases (`goldenNotFound`, `runningClonesPresent`, `tartDeleteFailed`)
- Wired `VMCommand.Delete` with `<name>` positional argument and `--force` flag; removed stub that threw ExitCode(1)
- Added 3 new `QEMURunnerTests`: deleteGolden artefact removal + untouched sibling, idempotency on missing artefacts, runningClonesBacked returns empty when clonesDir absent
- Flipped `scripts/macos/vm-delete.sh` from 118-line bash implementation to 12-line `exec guivision vm delete "$@"` wrapper
- README gained VM lifecycle CLI examples block, vm-list.sh/vm-delete.sh table rows, and First-run permission subsection for AppleScript Automation TCC grant
- `instructions-for-llms-using-this-as-a-tool.md` gained `### VM lifecycle via guivision vm` reference and dropped stale `--id` flag documentation
- Debug + release builds clean; 272 tests / 270 pass (2 pre-existing QEMU integration failures from sun_path limit, tracked as Task 12)
- Notable deviation: `TartRunner.which("qemu-img")` used in `backingFile`, falling back to homebrew path — consistent with `QEMURunner.start` pattern rather than hard-coding the path
- Task 3 status already `done` in backlog; no safety-net corrections needed
- Pre-existing swtpm sun_path bug (Task 12) confirmed as independent of P5; does not block completion
