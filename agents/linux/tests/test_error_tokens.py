"""Verify the Linux agent emits §4.5 canonical error tokens.

Automated guard for the realignment work tracked under
`realign-linux-agent-error-strings-to-cli-design-contract-4-5`. A future
change that introduces a non-canonical token (e.g. a typo like
`notFound` or an ad-hoc invented token) MUST fail one of these tests
rather than silently regressing through manual `grep` review.

Source of truth: docs/architecture/cli-design-contract.md §4.5
"""

from __future__ import annotations

import re
from pathlib import Path

import pytest

# §4.5 canonical wire tokens. The full catalogue — agents may not all
# emit every token today, but no agent may emit anything outside this
# set (apart from `KNOWN_NON_CANONICAL_EXCEPTIONS` below).
CANONICAL_TOKENS: frozenset[str] = frozenset({
    "not_found",
    "ambiguous",
    "window_not_found",
    "action_unsupported",
    "accessibility_unavailable",
    "exec_failed",
    "upload_failed",
    "download_failed",
    "invalid_json",
})

# Tokens emitted by Linux agent source today that are NOT in §4.5.
# Each entry must be justified — typically a transitional human-readable
# string predating the §4.5 effort. Adding to this set is a deliberate
# design decision; do not extend casually.
KNOWN_NON_CANONICAL_EXCEPTIONS: frozenset[str] = frozenset()

# Subset of §4.5 that the realignment work has actually wired up in
# the Linux agent. These tokens MUST be present in source — losing
# any one is a contract regression for a path that already shipped.
REALIGNED_TOKENS: frozenset[str] = frozenset({
    "not_found",
    "ambiguous",
    "window_not_found",
    "upload_failed",
    "download_failed",
    "invalid_json",
})

AGENT_ROOT = Path(__file__).resolve().parent.parent / "testanyware_agent"
SOURCE_FILES = sorted(AGENT_ROOT.rglob("*.py"))
ERROR_PATTERN = re.compile(r'"error"\s*:\s*"([^"]+)"')


def _all_emitted_tokens() -> set[str]:
    tokens: set[str] = set()
    for source in SOURCE_FILES:
        text = source.read_text(encoding="utf-8")
        tokens.update(ERROR_PATTERN.findall(text))
    return tokens


@pytest.mark.parametrize("token", sorted(REALIGNED_TOKENS))
def test_realigned_token_present_in_source(token: str) -> None:
    """Each realigned §4.5 token must be emitted at least once."""
    emitted = _all_emitted_tokens()
    assert token in emitted, (
        f'Canonical token "{token}" is no longer emitted by any '
        f"source file under agents/linux/testanyware_agent/. "
        f"This is a §4.5 contract regression — see "
        f"docs/architecture/cli-design-contract.md."
    )


def test_no_unknown_non_canonical_tokens() -> None:
    """Every emitted error token is canonical or a documented exception."""
    emitted = _all_emitted_tokens()
    allowed = CANONICAL_TOKENS | KNOWN_NON_CANONICAL_EXCEPTIONS
    unknown = emitted - allowed
    assert not unknown, (
        f"Source emits non-canonical error tokens: {sorted(unknown)}. "
        f"Either align with §4.5 ({sorted(CANONICAL_TOKENS)}) or — if "
        f"the new token is a deliberate transitional surface — add it "
        f"to KNOWN_NON_CANONICAL_EXCEPTIONS with a comment explaining "
        f"why and the contract gap it represents."
    )


def test_handle_download_emits_download_failed_for_missing_file(tmp_path) -> None:
    """Real behavioural test on system_endpoints (no AT-SPI2 needed)."""
    from testanyware_agent.system_endpoints import handle_download

    missing = tmp_path / "does-not-exist"
    status, error, fileobj = handle_download(str(missing))

    assert status == 400
    assert error["error"] == "download_failed"
    assert "details" in error, "download_failed must carry details for diagnostics"
    assert fileobj is None
