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

# Linux cross-build config — kept in step with release-build.sh.
readonly FFMPEG_SR_ROOT="${TESTANYWARE_FFMPEG_SR:-/tmp/taw-ffmpeg-sr}"
readonly LINUX_RUST_TARGETS=("aarch64-unknown-linux-gnu" "x86_64-unknown-linux-gnu")
# triple -> BtbN sysroot subdir (matches release-build.sh::ffmpeg_sysroot_for).
ffmpeg_sysroot_dir() {
  case "$1" in
    aarch64-unknown-linux-gnu) echo "$FFMPEG_SR_ROOT/aarch64-linux" ;;
    x86_64-unknown-linux-gnu)  echo "$FFMPEG_SR_ROOT/x86_64-linux" ;;
  esac
}

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

check_cargo() {
  if ! command -v cargo >/dev/null 2>&1; then
    mark_fail "cargo: not on PATH"
    remediation "install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    return
  fi
  local version
  version="$(cargo --version 2>/dev/null || echo unknown)"
  mark_pass "cargo: $version"
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

# --- Linux cross-build (cargo-zigbuild) prerequisites ----------------------
# These gate the Linux tarballs only; the macOS build needs none of them.

check_zig() {
  if ! command -v zig >/dev/null 2>&1; then
    mark_fail "zig: not on PATH (needed for Linux cross-build)"
    remediation "brew install zig   # floor 0.16"
    return
  fi
  local version
  version="$(zig version 2>/dev/null || echo unknown)"
  mark_pass "zig: $version"
}

check_cargo_zigbuild() {
  if ! command -v cargo-zigbuild >/dev/null 2>&1; then
    mark_fail "cargo-zigbuild: not installed (needed for Linux cross-build)"
    remediation "cargo install cargo-zigbuild"
    return
  fi
  mark_pass "cargo-zigbuild: installed"
}

check_rustup_linux_targets() {
  if ! command -v rustup >/dev/null 2>&1; then
    mark_fail "rustup: not on PATH (needed to add Linux std targets)"
    remediation "install via https://rustup.rs, then 'rustup target add ${LINUX_RUST_TARGETS[*]}'"
    return
  fi
  local installed triple
  installed="$(rustup target list --installed 2>/dev/null || echo)"
  for triple in "${LINUX_RUST_TARGETS[@]}"; do
    if grep -qx "$triple" <<<"$installed"; then
      mark_pass "rustup target: $triple"
    else
      mark_fail "rustup target: $triple not installed"
      remediation "rustup target add $triple"
    fi
  done
}

check_ffmpeg_sysroots() {
  local triple sr
  for triple in "${LINUX_RUST_TARGETS[@]}"; do
    sr="$(ffmpeg_sysroot_dir "$triple")"
    if [[ -d "$sr/lib/pkgconfig" ]]; then
      mark_pass "ffmpeg sysroot ($triple): $sr"
    else
      mark_fail "ffmpeg sysroot ($triple): missing $sr/lib/pkgconfig"
      remediation "stage the BtbN gpl-shared bundle (see docs/research/170-ffmpeg-cross-link.md §Reproduce); override root with TESTANYWARE_FFMPEG_SR"
    fi
  done
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
  check_cargo
  check_swift
  check_dotnet
  check_python3
  check_tar_xz
  check_gh_auth

  echo
  echo "release-doctor: Linux cross-build prerequisites"
  check_zig
  check_cargo_zigbuild
  check_rustup_linux_targets
  check_ffmpeg_sysroots

  echo
  if (( failed == 0 )); then
    echo "release-doctor: all prerequisites met"
    exit 0
  fi
  echo "release-doctor: missing prerequisites — fix the items marked above" >&2
  exit 1
}

main "$@"
