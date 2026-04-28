#!/usr/bin/env bash
#
# Verify the prerequisites for running the release pipeline. Checks
# only — never installs anything. Emits a punch list of every missing
# item with the README's remediation command, then exits non-zero if
# anything is missing.
#
# Designed to run twice: standalone before committing to a release
# attempt, and (cheaply) as the first step of release-build.sh so the
# build fails fast on a misconfigured machine instead of mid-toolchain.

set -euo pipefail
IFS=$'\n\t'
trap 'echo "release-doctor: error on line $LINENO" >&2' ERR

failed=0

mark_pass() {
  echo "  ✓ $*"
}

mark_fail() {
  echo "  ✗ $*"
  failed=1
}

remediation() {
  echo "      remediation: $*"
}

check_swift() {
  if ! command -v swift >/dev/null 2>&1; then
    mark_fail "swift: not on PATH"
    remediation "install Xcode Command Line Tools (xcode-select --install) or Xcode 16+"
    return
  fi
  local version
  version="$(swift --version 2>/dev/null | head -n1 || echo unknown)"
  mark_pass "swift: $version"
}

check_dotnet() {
  if ! command -v dotnet >/dev/null 2>&1; then
    mark_fail "dotnet: not on PATH"
    remediation "brew install dotnet"
    return
  fi
  local version
  version="$(dotnet --version 2>/dev/null || echo unknown)"
  mark_pass "dotnet: $version"
}

check_python3() {
  if ! command -v python3 >/dev/null 2>&1; then
    mark_fail "python3: not on PATH"
    remediation "install Xcode Command Line Tools (xcode-select --install)"
    return
  fi
  local version
  version="$(python3 --version 2>/dev/null || echo unknown)"
  mark_pass "python3: $version"
}

check_tar_xz() {
  if ! command -v tar >/dev/null 2>&1; then
    mark_fail "tar: not on PATH"
    remediation "install Xcode Command Line Tools or BSD tar"
    return
  fi
  if ! command -v xz >/dev/null 2>&1; then
    mark_fail "xz: not on PATH"
    remediation "brew install xz"
    return
  fi
  mark_pass "tar + xz: available"
}

check_gh_auth() {
  if ! command -v gh >/dev/null 2>&1; then
    mark_fail "gh: not installed"
    remediation "brew install gh && gh auth login"
    return
  fi
  if gh auth status >/dev/null 2>&1; then
    mark_pass "gh: authenticated"
  else
    mark_fail "gh: not authenticated"
    remediation "gh auth login"
  fi
}

check_arch() {
  local arch
  arch="$(uname -m)"
  if [[ "$arch" == "arm64" ]]; then
    mark_pass "host arch: arm64 (Apple Silicon)"
  else
    mark_fail "host arch: $arch (release pipeline targets aarch64-apple-darwin only)"
    remediation "build on an Apple Silicon Mac"
  fi
}

main() {
  echo "release-doctor: checking release prerequisites"
  echo

  check_arch
  check_swift
  check_dotnet
  check_python3
  check_tar_xz
  check_gh_auth

  echo
  if (( failed == 0 )); then
    echo "release-doctor: all prerequisites met"
    exit 0
  fi
  echo "release-doctor: missing prerequisites — fix the items marked above" >&2
  exit 1
}

main "$@"
