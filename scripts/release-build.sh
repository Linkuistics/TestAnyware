#!/usr/bin/env bash
#
# Build a single bundled tarball for the current git tag and render the
# Homebrew formula from scripts/templates/testanyware.rb.tmpl.
#
# The bundle includes:
#   - testanyware (host CLI, Swift, arm64-apple-darwin)
#   - testanyware-agent (macOS in-VM agent, Swift, arm64-apple-darwin)
#   - testanyware-agent.exe (Windows in-VM agent, .NET 9 self-contained, win-arm64)
#   - testanyware_agent (Linux in-VM agent, Python source)
#   - vm-{create-golden-{macos,linux,windows},start,stop,list,delete}.sh
#   - helpers/* (autounattend.xml, plist, set-wallpaper.swift, etc.)
#
# Output: target/dist/
#   testanyware-v<ver>-aarch64-apple-darwin.tar.xz
#   testanyware.rb
#
# After this completes, inspect target/dist/ and run release-publish.sh.

set -euo pipefail
IFS=$'\n\t'

readonly REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly DIST_DIR="$REPO_ROOT/target/dist"
readonly TEMPLATE="$REPO_ROOT/scripts/templates/testanyware.rb.tmpl"
readonly TARGET="aarch64-apple-darwin"

die() {
  echo "release-build: $*" >&2
  exit 1
}

require_clean_tagged_tree() {
  [[ -z "$(git -C "$REPO_ROOT" status --porcelain)" ]] \
    || die "working tree is dirty; commit or stash before releasing"
  git -C "$REPO_ROOT" describe --tags --exact-match HEAD >/dev/null 2>&1 \
    || die "HEAD is not a tagged commit; create one with 'git tag -a v<x.y.z> -m ...'"
}

read_version() {
  git -C "$REPO_ROOT" describe --tags --abbrev=0 | sed 's/^v//'
}

# Helper functions whose stdout is captured via $(...) MUST send all
# progress output to stderr — otherwise the caller's command substitution
# splices informational text into the returned path. assemble_bundle and
# package_bundle below are the captured callers.
build_cli() {
  local stage_bin="$1"
  echo "release-build: building CLI (testanyware)" >&2
  swift build --package-path "$REPO_ROOT/cli" -c release >&2
  local bin_path
  bin_path="$(swift build --package-path "$REPO_ROOT/cli" -c release --show-bin-path)"
  [[ -f "$bin_path/testanyware" ]] || die "CLI build did not produce $bin_path/testanyware"
  cp "$bin_path/testanyware" "$stage_bin/testanyware"
  chmod +x "$stage_bin/testanyware"
}

build_macos_agent() {
  local stage_agents="$1"
  echo "release-build: building macOS agent (testanyware-agent)" >&2
  swift build --package-path "$REPO_ROOT/agents/macos" -c release >&2
  local bin_path
  bin_path="$(swift build --package-path "$REPO_ROOT/agents/macos" -c release --show-bin-path)"
  [[ -f "$bin_path/testanyware-agent" ]] || die "macOS agent build did not produce $bin_path/testanyware-agent"
  mkdir -p "$stage_agents/macos"
  cp "$bin_path/testanyware-agent" "$stage_agents/macos/testanyware-agent"
  chmod +x "$stage_agents/macos/testanyware-agent"
}

build_windows_agent() {
  local stage_agents="$1"
  echo "release-build: building Windows agent (testanyware-agent.exe, win-arm64)" >&2
  local proj="$REPO_ROOT/agents/windows"
  dotnet publish "$proj" -r win-arm64 --self-contained \
    -p:PublishSingleFile=true -c Release --nologo -v quiet >&2
  local exe="$proj/bin/Release/net9.0-windows/win-arm64/publish/testanyware-agent.exe"
  [[ -f "$exe" ]] || die "Windows agent build did not produce $exe"
  mkdir -p "$stage_agents/windows"
  cp "$exe" "$stage_agents/windows/testanyware-agent.exe"
}

stage_linux_agent() {
  local stage_agents="$1"
  echo "release-build: staging Linux agent (Python source)" >&2
  local src="$REPO_ROOT/agents/linux/testanyware_agent"
  [[ -d "$src" ]] || die "Linux agent source not found at $src"
  mkdir -p "$stage_agents/linux"
  # Copy the package, excluding caches.
  (cd "$REPO_ROOT/agents/linux" && \
    tar --exclude='__pycache__' --exclude='*.pyc' -cf - testanyware_agent) \
    | (cd "$stage_agents/linux" && tar -xf -)
}

stage_scripts() {
  local stage_scripts="$1"
  echo "release-build: staging provisioner scripts" >&2
  mkdir -p "$stage_scripts"
  cp "$REPO_ROOT/provisioner/scripts/"_testanyware-paths.sh "$stage_scripts/"
  cp "$REPO_ROOT/provisioner/scripts/"vm-*.sh "$stage_scripts/"
  chmod +x "$stage_scripts/"*.sh
}

stage_helpers() {
  local stage_helpers="$1"
  echo "release-build: staging helpers" >&2
  mkdir -p "$stage_helpers"
  cp -R "$REPO_ROOT/provisioner/helpers/." "$stage_helpers/"
}

assemble_bundle() {
  local version="$1"
  local bundle_root="$DIST_DIR/staging/testanyware-v${version}-${TARGET}"
  rm -rf "$bundle_root"
  mkdir -p "$bundle_root/bin" \
           "$bundle_root/share/testanyware/agents" \
           "$bundle_root/share/testanyware/scripts" \
           "$bundle_root/share/testanyware/helpers"

  build_cli "$bundle_root/bin"
  build_macos_agent "$bundle_root/share/testanyware/agents"
  build_windows_agent "$bundle_root/share/testanyware/agents"
  stage_linux_agent "$bundle_root/share/testanyware/agents"
  stage_scripts "$bundle_root/share/testanyware/scripts"
  stage_helpers "$bundle_root/share/testanyware/helpers"

  cp "$REPO_ROOT/README.md" "$bundle_root/README.md"
  if [[ -f "$REPO_ROOT/LICENSE" ]]; then
    cp "$REPO_ROOT/LICENSE" "$bundle_root/LICENSE"
  fi

  echo "$bundle_root"
}

package_bundle() {
  local bundle_root="$1" version="$2"
  local archive="$DIST_DIR/testanyware-v${version}-${TARGET}.tar.xz"
  echo "release-build: packaging $archive" >&2
  tar -C "$DIST_DIR/staging" -cJf "$archive" "$(basename "$bundle_root")"
  echo "$archive"
}

sha256_of() {
  shasum -a 256 "$1" | awk '{print $1}'
}

render_formula() {
  local version="$1" sha="$2"
  sed \
    -e "s|@VERSION@|${version}|g" \
    -e "s|@SHA_AARCH64_APPLE_DARWIN@|${sha}|g" \
    "$TEMPLATE" >"$DIST_DIR/testanyware.rb"
  echo "release-build: rendered $DIST_DIR/testanyware.rb"
}

main() {
  cd "$REPO_ROOT"
  "$REPO_ROOT/scripts/release-doctor.sh"
  require_clean_tagged_tree
  local version
  version="$(read_version)"
  echo "release-build: building testanyware v${version} for ${TARGET}"

  rm -rf "$DIST_DIR"
  mkdir -p "$DIST_DIR/staging"

  local bundle_root archive sha
  bundle_root="$(assemble_bundle "$version")"
  archive="$(package_bundle "$bundle_root" "$version")"
  sha="$(sha256_of "$archive")"
  render_formula "$version" "$sha"

  rm -rf "$DIST_DIR/staging"

  echo
  echo "release-build: artifacts in $DIST_DIR"
  ls -la "$DIST_DIR"
  echo
  echo "Inspect, then run scripts/release-publish.sh"
}

main "$@"
