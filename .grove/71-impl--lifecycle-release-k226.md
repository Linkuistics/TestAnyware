# lifecycle-release-k226

## Goal

Publish the next TestAnyware release. It carries the Host CLI and agent
changes of leaves 61 to 66.

## Context

- Taskfile tasks `release:*` and the scripts `scripts/release-*.sh`.
- Earlier release leaves: release-k5 (commit 5fc9602280) and
  queue-release-k26 (commit b0d72942f3).
- The trial result of leaf k225.

## Done when

- Start only after leaf k225 records a passing trial.
- Run `release:doctor`, `release:candidate` and `release:candidate-verify`.
- Run `release:linux-harness` on the candidate.
- Run `release:upgrade-check`, `release:app-verify` and
  `release:app-notarize`.
- Run `release:publish`.
- Record how the new agents reach the guests of a consumer.
- Record the release in the spec.

## Notes

Ask the human before you upgrade the installed app on this Mac. Ask the human
for any credential that a release step needs.
