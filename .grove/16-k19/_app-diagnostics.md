# app-diagnostics-k19 — brief

## Goal

Make `doctor`, the `llm-instructions` and the documents describe the app. A person or an agent can then find the state of the setup and the remedy of each failure.

## Done when

- `testanyware doctor` has an `app` check on a macOS host. Its `config` check on every host comes from `config-evaluator-cli-k18`. Each failing item carries a remedy.
- The `llm-instructions` describe the one-time setup, the rule that a client never starts the app, each new error with its remedy, and golden creation from Terminal.
- The documents that the spec lists are corrected. The CLI contract has its amendments.
- The ADR set, the spec and the glossary agree with the shipped behaviour.

## Decomposition

1. `doctor-app-check`: the `app` check.
2. `llm-instructions-and-docs`: the text and the documents.

## Pointers

- Spec: `docs/specs/testanyware-app.md`, sections "What the commands report about the app" and "Documents that the implementation corrects".
- `docs/architecture/cli-design-contract.md` and `docs/reference/`.
- ADR-0019 and ADR-0021.

## Notes

- The `using-testanyware` skill in the Grove repo needs no change. It already sends agents to `doctor` first.
