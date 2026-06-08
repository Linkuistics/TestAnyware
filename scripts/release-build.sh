#!/usr/bin/env bash
#
# Build the bundled tarball(s) for the current git tag and render the
# Homebrew formula from scripts/templates/testanyware.rb.tmpl.
#
# Targets:
#   - aarch64-apple-darwin            (native; the macOS host CLI)
#   - aarch64-unknown-linux-gnu       (cross via cargo-zigbuild; FIRST-CLASS,
#                                       harness-runtime-verified — grove 190)
#   - x86_64-unknown-linux-gnu        (cross via cargo-zigbuild; BUILD-VERIFIED
#                                       ONLY — no native x86_64 guest on this
#                                       Mac to run it, ADR-0009 no-silent-caps)
#   - aarch64-pc-windows-gnullvm      (cross via cargo-zigbuild; FIRST-CLASS,
#                                       harness-runtime-verified — grove 220/040)
#   - x86_64-pc-windows-gnu           (cross via cargo-zigbuild; BUILD-VERIFIED
#                                       ONLY — no native x86_64 Windows guest on
#                                       this Mac, ADR-0009 no-silent-caps)
#
# Delivery format differs by OS: macOS + Linux ship Homebrew-installable
# `.tar.xz` bundles (the rendered testanyware.rb formula points at them); Windows
# ships a plain `.zip` per triple (no Homebrew on Windows — users unzip + run).
#
# Every bundle includes:
#   - testanyware (host CLI, Rust)
#   - testanyware-agent       (macOS in-VM agent, Swift)
#   - testanyware-agent.exe   (Windows in-VM agent, .NET 9 self-contained)
#   - testanyware_agent       (Linux in-VM agent, Python source)
#   - _testanyware-paths.sh + helpers/* (all three goldens are now in-process
#     `vm create-golden --platform <os>` subcommands — macOS 110, Windows
#     220/020, Linux 230 — so no golden shell script ships)
#
# Linux bundles additionally carry:
#   - lib/libav*.so* + libsw*.so*  — the BtbN ffmpeg-8 gpl-shared runtime libs
#     the cross binary hard-NEEDs (sonames don't match any distro's ffmpeg).
#     The binary is linked with RUNPATH=$ORIGIN/../lib and all five sonames as
#     *direct* NEEDED (see build_cli_cross_linux), so once installed into the
#     keg's lib/ it self-locates them with no LD_LIBRARY_PATH (170 + grove 210).
#   - share/testanyware/ocr/  — the `ocr_analyzer` EasyOCR daemon source the
#     formula pip-installs into <prefix>/libexec/venv (the Linux OCR path; macOS
#     uses native Vision and ships none). ADR-0002.
#
# Windows bundles instead carry:
#   - bin/avcodec-62.dll + avformat-62.dll + avutil-60.dll + swscale-9.dll +
#     swresample-6.dll — the BtbN ffmpeg-8 winarm64/win64 gpl-shared runtime DLLs
#     the cross binary load-time-imports. Co-located *beside* testanyware.exe so
#     the PE loader's image-directory search resolves them with no PATH (the
#     Windows analogue of the Linux RUNPATH trick; grove 220/040 proved it).
#   - NO OCR venv: unlike Linux (EasyOCR daemon), Windows `screen find-text`
#     uses the in-process native `Windows.Media.Ocr` engine compiled into the
#     .exe (grove 220/070, ADR-0011 — EasyOCR is uninstallable on win-arm64), so
#     nothing extra ships in the zip. ADR-0002 seam unchanged.
#
# Output: target/dist/
#   testanyware-v<ver>-<triple>.tar.xz   (one per macOS/Linux target)
#   testanyware-v<ver>-<triple>.zip      (one per Windows target)
#   testanyware.rb
#
# After this completes, inspect target/dist/ and run release-publish.sh.
#
# Tool floors picked up by `testanyware doctor` (when the doctor can locate
# this script — i.e. running from a dev build inside the source tree):
# testanyware-min-tool: cargo 1.81
# testanyware-min-tool: swift 6.0
# testanyware-min-tool: dotnet 9.0
# testanyware-min-tool: zig 0.16

set -euo pipefail
IFS=$'\n\t'

readonly REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly DIST_DIR="$REPO_ROOT/target/dist"
readonly TEMPLATE="$REPO_ROOT/scripts/templates/testanyware.rb.tmpl"
readonly CLI_RS="$REPO_ROOT/cli-rs"
readonly CARGO_TOML="$CLI_RS/Cargo.toml"

readonly MACOS_TARGET="aarch64-apple-darwin"
# Linux cross targets. aarch64 is first-class (harness-runtime-verified);
# x86_64 is build/link-verified only (no native x86_64 guest here).
readonly LINUX_TARGETS=("aarch64-unknown-linux-gnu" "x86_64-unknown-linux-gnu")
# Windows cross targets. aarch64 is first-class (grove 220/040 harness-runtime-
# verified); x86_64 is build/link-verified only (no native x86_64 Windows guest
# here). aarch64 uses -gnullvm (there is no aarch64-pc-windows-gnu in rustup);
# x86_64 uses -gnu. Both cross from a Mac via cargo-zigbuild (msvc cannot).
readonly WINDOWS_TARGETS=("aarch64-pc-windows-gnullvm" "x86_64-pc-windows-gnu")

# The five ffmpeg-8 runtime DLLs the cross Windows binary load-time-imports: the
# four it references directly (av{codec,format,util}, swscale) plus swresample (a
# transitive dep of avcodec). x264/x265 are statically linked into BtbN's
# avcodec-62.dll, so they are not separate DLLs. All five ship co-located beside
# the .exe (image-directory search). Mirrors the harness's REQUIRED_DLLS.
readonly WINDOWS_DLLS=(
  avcodec-62.dll
  avformat-62.dll
  avutil-60.dll
  swscale-9.dll
  swresample-6.dll
)

# Root holding the per-triple BtbN ffmpeg-8 gpl-shared sysroots, laid out as
# <root>/<arch>-linux/lib/{*.so*,pkgconfig}. Staged by release-doctor.sh /
# docs/research/170-ffmpeg-cross-link.md. Overridable for a non-default cache.
readonly FFMPEG_SR_ROOT="${TESTANYWARE_FFMPEG_SR:-/tmp/taw-ffmpeg-sr}"

# The ffmpeg-8 sonames the cross binary loads: the four it references plus
# libswresample (a transitive dep of libavcodec we force to a *direct* NEEDED
# so RUNPATH resolves it — see build_cli_cross_linux). All five must ship.
readonly REQUIRED_SONAMES=(
  libavcodec.so.62
  libavformat.so.62
  libavutil.so.60
  libswscale.so.9
  libswresample.so.6
)

die() {
  echo "release-build: $*" >&2
  exit 1
}

# build_cli* temporarily rewrites the [workspace.package] version in
# cli-rs/Cargo.toml so the released binary's CARGO_PKG_VERSION (hence --version
# and `capabilities`.version) reflects the tag. set_cli_version is called once
# in main() before any build; restore_cli_version reverts Cargo.toml +
# Cargo.lock on exit (including failure) so the tree returns to the dev version.
restore_cli_version() {
  git -C "$REPO_ROOT" checkout -- "$CARGO_TOML" "$CLI_RS/Cargo.lock" 2>/dev/null || true
}
trap restore_cli_version EXIT

require_clean_tagged_tree() {
  [[ -z "$(git -C "$REPO_ROOT" status --porcelain)" ]] \
    || die "working tree is dirty; commit or stash before releasing"
  git -C "$REPO_ROOT" describe --tags --exact-match HEAD >/dev/null 2>&1 \
    || die "HEAD is not a tagged commit; create one with 'git tag -a v<x.y.z> -m ...'"
}

read_version() {
  git -C "$REPO_ROOT" describe --tags --abbrev=0 | sed 's/^v//'
}

# Helper functions whose stdout is captured via $(...) MUST send all progress
# output to stderr — otherwise command substitution splices informational text
# into the returned path. assemble_bundle and package_bundle are such callers.

# Rewrite the [workspace.package] version in cli-rs/Cargo.toml to $1, in place.
# Section-scoped so dependency `version = …` keys elsewhere are untouched.
set_cli_version() {
  local version="$1"
  awk -v ver="$version" '
    /^\[/ { in_pkg = ($0 == "[workspace.package]") }
    in_pkg && /^version = / { print "version = \"" ver "\""; next }
    { print }
  ' "$CARGO_TOML" > "$CARGO_TOML.tmp" && mv "$CARGO_TOML.tmp" "$CARGO_TOML"
}

# Map a cross Rust triple to its BtbN ffmpeg-8 sysroot directory.
ffmpeg_sysroot_for() {
  case "$1" in
    aarch64-unknown-linux-gnu)   echo "$FFMPEG_SR_ROOT/aarch64-linux" ;;
    x86_64-unknown-linux-gnu)    echo "$FFMPEG_SR_ROOT/x86_64-linux" ;;
    aarch64-pc-windows-gnullvm)  echo "$FFMPEG_SR_ROOT/aarch64-windows" ;;
    x86_64-pc-windows-gnu)       echo "$FFMPEG_SR_ROOT/x86_64-windows" ;;
    *) die "no ffmpeg sysroot mapping for $1" ;;
  esac
}

# Map a Windows Rust triple to the clang `--target` bindgen uses to parse the
# ffmpeg headers. Always the `-gnu` spelling even for the gnullvm Rust target
# (the Rust/zig *link* target stays gnullvm; only the header-parse triple is
# -gnu), and correct for Windows LLP64 (170 research §bindgen).
windows_bindgen_target() {
  case "$1" in
    aarch64-pc-windows-gnullvm) echo "aarch64-pc-windows-gnu" ;;
    x86_64-pc-windows-gnu)      echo "x86_64-pc-windows-gnu" ;;
    *) die "no bindgen target mapping for $1" ;;
  esac
}

build_cli() {
  local stage_bin="$1"
  echo "release-build: building CLI (testanyware, Rust, $MACOS_TARGET)" >&2
  local git_describe
  git_describe="$(git -C "$REPO_ROOT" describe --tags --always --dirty 2>/dev/null || echo unknown)"
  TESTANYWARE_GIT_REVISION="$git_describe" \
    cargo build --manifest-path "$CARGO_TOML" -p testanyware-cli --release >&2
  local bin_path="$CLI_RS/target/release/testanyware"
  [[ -f "$bin_path" ]] || die "CLI build did not produce $bin_path"
  cp "$bin_path" "$stage_bin/testanyware"
  chmod +x "$stage_bin/testanyware"
}

# Cross-build the host CLI for a Linux triple via cargo-zigbuild against the
# BtbN ffmpeg-8 sysroot (170). Two link details make the binary self-locating
# once its ffmpeg libs sit in a sibling lib/:
#   * RUNPATH=$ORIGIN/../lib — zig's lld always emits DT_RUNPATH (not DT_RPATH),
#     and RUNPATH is searched for the executable's *direct* NEEDED only.
#   * -Wl,--no-as-needed -lswresample — libswresample is otherwise a
#     transitive-only dep of libavcodec; forcing it to a direct NEEDED brings it
#     under RUNPATH. The other four are referenced directly already. With all
#     five direct + co-located in lib/, every cross-reference resolves from the
#     already-loaded global scope (grove 210 ELF analysis).
build_cli_cross_linux() {
  local triple="$1" stage_bin="$2"
  local sr; sr="$(ffmpeg_sysroot_for "$triple")"
  [[ -d "$sr/lib/pkgconfig" ]] \
    || die "ffmpeg sysroot for $triple not found at $sr/lib/pkgconfig (run release-doctor.sh)"
  echo "release-build: cross-building CLI for $triple (cargo-zigbuild)" >&2
  local git_describe
  git_describe="$(git -C "$REPO_ROOT" describe --tags --always --dirty 2>/dev/null || echo unknown)"
  (
    cd "$CLI_RS" \
    && PKG_CONFIG_ALLOW_CROSS=1 \
       PKG_CONFIG_LIBDIR="$sr/lib/pkgconfig" \
       TESTANYWARE_GIT_REVISION="$git_describe" \
       RUSTFLAGS="-C link-arg=-Wl,-rpath,\$ORIGIN/../lib -C link-arg=-L$sr/lib -C link-arg=-Wl,--no-as-needed -C link-arg=-lswresample -C link-arg=-Wl,--as-needed" \
       cargo zigbuild -p testanyware-cli --bin testanyware --target "$triple" --release >&2
  )
  local bin_path="$CLI_RS/target/$triple/release/testanyware"
  [[ -f "$bin_path" ]] || die "cross build for $triple did not produce $bin_path"
  cp "$bin_path" "$stage_bin/testanyware"
  chmod +x "$stage_bin/testanyware"
}

# Stage the five ffmpeg-8 runtime sonames into a bundle's lib/. Each is shipped
# as a single dereferenced regular file named by the soname (the loader opens by
# the DT_NEEDED string), so no symlinks need to survive the tar.
stage_ffmpeg_libs() {
  local triple="$1" stage_lib="$2"
  local sr; sr="$(ffmpeg_sysroot_for "$triple")"
  mkdir -p "$stage_lib"
  local soname
  for soname in "${REQUIRED_SONAMES[@]}"; do
    local src="$sr/lib/$soname"
    [[ -e "$src" ]] \
      || die "ffmpeg runtime lib $src missing for $triple (stage the BtbN gpl-shared bundle; see docs/research/170-ffmpeg-cross-link.md)"
    cp -L "$src" "$stage_lib/$soname"
  done
  echo "release-build: staged ${#REQUIRED_SONAMES[@]} ffmpeg-8 .so's for $triple into lib/" >&2
}

# Cross-build the host CLI for a Windows triple via cargo-zigbuild against the
# BtbN ffmpeg-8 sysroot (170 + grove 220/040). Unlike the Linux arm, this needs
# no RUNPATH/-lswresample link surgery: Windows resolves the ffmpeg DLLs by the
# PE loader's image-directory search, so co-locating them beside the .exe
# (stage_ffmpeg_dlls) is all the runtime link needs. PKG_CONFIG_LIBDIR feeds the
# import libs (.dll.a) to the link; BINDGEN_EXTRA_CLANG_ARGS sets the header
# parse target. Recipe is verbatim the one windows-host-harness.rs documents.
build_cli_cross_windows() {
  local triple="$1" stage_bin="$2"
  local sr; sr="$(ffmpeg_sysroot_for "$triple")"
  [[ -d "$sr/lib/pkgconfig" ]] \
    || die "ffmpeg sysroot for $triple not found at $sr/lib/pkgconfig (run release-doctor.sh)"
  local bindgen_target; bindgen_target="$(windows_bindgen_target "$triple")"
  echo "release-build: cross-building CLI for $triple (cargo-zigbuild)" >&2
  local git_describe
  git_describe="$(git -C "$REPO_ROOT" describe --tags --always --dirty 2>/dev/null || echo unknown)"
  (
    cd "$CLI_RS" \
    && PKG_CONFIG_ALLOW_CROSS=1 \
       PKG_CONFIG_LIBDIR="$sr/lib/pkgconfig" \
       BINDGEN_EXTRA_CLANG_ARGS="--target=$bindgen_target" \
       TESTANYWARE_GIT_REVISION="$git_describe" \
       cargo zigbuild -p testanyware-cli --bin testanyware --target "$triple" --release >&2
  )
  local bin_path="$CLI_RS/target/$triple/release/testanyware.exe"
  [[ -f "$bin_path" ]] || die "cross build for $triple did not produce $bin_path"
  cp "$bin_path" "$stage_bin/testanyware.exe"
}

# Stage the five ffmpeg-8 runtime DLLs *beside* the .exe (BtbN ships them in the
# sysroot's bin/, the import libs in lib/; the loader needs the former). No lib/
# subdir and no RUNPATH: the PE image-directory search finds same-dir DLLs first.
stage_ffmpeg_dlls() {
  local triple="$1" stage_bin="$2"
  local sr; sr="$(ffmpeg_sysroot_for "$triple")"
  local dll
  for dll in "${WINDOWS_DLLS[@]}"; do
    local src="$sr/bin/$dll"
    [[ -e "$src" ]] \
      || die "ffmpeg runtime DLL $src missing for $triple (stage the BtbN gpl-shared bundle's bin/; see docs/research/170-ffmpeg-cross-link.md)"
    cp -L "$src" "$stage_bin/$dll"
  done
  echo "release-build: staged ${#WINDOWS_DLLS[@]} ffmpeg-8 DLLs for $triple beside the .exe" >&2
}

# Stage the `ocr_analyzer` daemon project (pyproject + src) so the Linux formula
# can `pip install --no-deps` it into <prefix>/libexec/venv. macOS skips this.
stage_ocr_module() {
  local stage_ocr="$1"
  local src="$REPO_ROOT/vision/stages/text-ocr"
  [[ -d "$src/src/ocr_analyzer" ]] || die "ocr_analyzer source not found at $src/src/ocr_analyzer"
  mkdir -p "$stage_ocr"
  cp "$src/pyproject.toml" "$stage_ocr/"
  [[ -f "$src/README.md" ]] && cp "$src/README.md" "$stage_ocr/"
  (cd "$src" && tar --exclude='__pycache__' --exclude='*.pyc' -cf - src) \
    | (cd "$stage_ocr" && tar -xf -)
  echo "release-build: staged ocr_analyzer module" >&2
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
  (cd "$REPO_ROOT/agents/linux" && \
    tar --exclude='__pycache__' --exclude='*.pyc' -cf - testanyware_agent) \
    | (cd "$stage_agents/linux" && tar -xf -)
}

stage_scripts() {
  local stage_scripts="$1"
  echo "release-build: staging provisioner scripts" >&2
  mkdir -p "$stage_scripts"
  # All three goldens (macOS 110, Windows 220/020, Linux 230) are now the
  # in-process `vm create-golden --platform <os>` command; no golden shell
  # script ships. Only the runtime path helper remains.
  cp "$REPO_ROOT/provisioner/scripts/"_testanyware-paths.sh "$stage_scripts/"
  chmod +x "$stage_scripts/"*.sh
}

stage_helpers() {
  local stage_helpers="$1"
  echo "release-build: staging helpers" >&2
  mkdir -p "$stage_helpers"
  cp -R "$REPO_ROOT/provisioner/helpers/." "$stage_helpers/"
}

# Build the host-arch agents + stage scripts/helpers ONCE into a shared dir;
# every per-target bundle copies them in. The Swift/.NET agent builds are
# host-arch and identical across bundles, so this avoids rebuilding them per
# triple. Echoes the shared dir path (stdout); progress to stderr.
build_shared_payload() {
  local shared="$DIST_DIR/staging/shared"
  rm -rf "$shared"
  mkdir -p "$shared/agents" "$shared/scripts" "$shared/helpers"
  build_macos_agent "$shared/agents"
  build_windows_agent "$shared/agents"
  stage_linux_agent "$shared/agents"
  stage_scripts "$shared/scripts"
  stage_helpers "$shared/helpers"
  echo "$shared"
}

# Assemble one bundle. $1=triple $2=version $3=shared-payload-dir.
# The CLI is native for the macOS target and cross-built for Linux/Windows
# triples. Linux bundles additionally carry lib/ (ffmpeg .so) + share/.../ocr/;
# Windows bundles carry the ffmpeg DLLs co-located in bin/ and NO ocr module.
assemble_bundle() {
  local triple="$1" version="$2" shared="$3"
  local bundle_root="$DIST_DIR/staging/testanyware-v${version}-${triple}"
  rm -rf "$bundle_root"
  mkdir -p "$bundle_root/bin" "$bundle_root/share/testanyware"

  if [[ "$triple" == "$MACOS_TARGET" ]]; then
    build_cli "$bundle_root/bin"
  elif [[ "$triple" == *-pc-windows-* ]]; then
    build_cli_cross_windows "$triple" "$bundle_root/bin"
    stage_ffmpeg_dlls "$triple" "$bundle_root/bin"
  else
    build_cli_cross_linux "$triple" "$bundle_root/bin"
    stage_ffmpeg_libs "$triple" "$bundle_root/lib"
    stage_ocr_module "$bundle_root/share/testanyware/ocr"
  fi

  cp -R "$shared/agents" "$bundle_root/share/testanyware/agents"
  cp -R "$shared/scripts" "$bundle_root/share/testanyware/scripts"
  cp -R "$shared/helpers" "$bundle_root/share/testanyware/helpers"

  cp "$REPO_ROOT/README.md" "$bundle_root/README.md"
  [[ -f "$REPO_ROOT/LICENSE" ]] && cp "$REPO_ROOT/LICENSE" "$bundle_root/LICENSE"

  echo "$bundle_root"
}

# Package a staged bundle into its delivery archive. Windows triples ship a
# `.zip` (no Homebrew on Windows); macOS + Linux ship a Homebrew-installable
# `.tar.xz`. Both archives carry the same top-level `testanyware-v<ver>-<triple>/`
# dir. Echoes the archive path (stdout); progress to stderr.
package_bundle() {
  local bundle_root="$1"
  local name; name="$(basename "$bundle_root")"
  local archive
  if [[ "$name" == *-pc-windows-* ]]; then
    archive="$DIST_DIR/${name}.zip"
    echo "release-build: packaging $archive" >&2
    ( cd "$DIST_DIR/staging" && zip -qr "$archive" "$name" ) >&2
  else
    archive="$DIST_DIR/${name}.tar.xz"
    echo "release-build: packaging $archive" >&2
    tar -C "$DIST_DIR/staging" -cJf "$archive" "$name" >&2
  fi
  echo "$archive"
}

sha256_of() {
  shasum -a 256 "$1" | awk '{print $1}'
}

# Render the formula, substituting the version and every per-target sha.
render_formula() {
  local version="$1" sha_darwin="$2" sha_linux_arm="$3" sha_linux_x86="$4"
  sed \
    -e "s|@VERSION@|${version}|g" \
    -e "s|@SHA_AARCH64_APPLE_DARWIN@|${sha_darwin}|g" \
    -e "s|@SHA_AARCH64_LINUX@|${sha_linux_arm}|g" \
    -e "s|@SHA_X86_64_LINUX@|${sha_linux_x86}|g" \
    "$TEMPLATE" >"$DIST_DIR/testanyware.rb"
  echo "release-build: rendered $DIST_DIR/testanyware.rb"
}

main() {
  cd "$REPO_ROOT"
  "$REPO_ROOT/scripts/release-doctor.sh"
  require_clean_tagged_tree
  local version
  version="$(read_version)"
  echo "release-build: building testanyware v${version}"
  echo "release-build: targets: $MACOS_TARGET ${LINUX_TARGETS[*]} ${WINDOWS_TARGETS[*]}"

  rm -rf "$DIST_DIR"
  mkdir -p "$DIST_DIR/staging"

  # Version bump invalidates the cli crate fingerprint so option_env! re-reads
  # TESTANYWARE_GIT_REVISION on every (re)build below. Reverted by the trap.
  set_cli_version "$version"

  local shared
  shared="$(build_shared_payload)"

  # macOS (native) + each Linux triple (.tar.xz) + each Windows triple (.zip).
  # Collect each archive's sha; only the macOS/Linux ones feed the formula
  # (Windows ships a standalone zip, not a Homebrew bottle).
  local -A sha
  local triple bundle_root archive
  for triple in "$MACOS_TARGET" "${LINUX_TARGETS[@]}" "${WINDOWS_TARGETS[@]}"; do
    bundle_root="$(assemble_bundle "$triple" "$version" "$shared")"
    archive="$(package_bundle "$bundle_root")"
    sha["$triple"]="$(sha256_of "$archive")"
  done

  render_formula "$version" \
    "${sha[$MACOS_TARGET]}" \
    "${sha[aarch64-unknown-linux-gnu]}" \
    "${sha[x86_64-unknown-linux-gnu]}"

  rm -rf "$DIST_DIR/staging"

  echo
  echo "release-build: artifacts in $DIST_DIR"
  ls -la "$DIST_DIR"
  echo
  echo "NOTE: the x86_64 triples (x86_64-unknown-linux-gnu, x86_64-pc-windows-gnu)"
  echo "      are BUILD/LINK-VERIFIED ONLY — no native x86_64 guest on this Mac"
  echo "      runs them (ADR-0009 no-silent-caps). Their record/OCR runtime path"
  echo "      is unverified; ship knowingly. aarch64 (both linux + windows) is the"
  echo "      harness-runtime-verified tier."
  echo
  echo "NOTE: Windows 'screen find-text' uses the in-process native"
  echo "      Windows.Media.Ocr engine compiled into the .exe (grove 220/070,"
  echo "      ADR-0011) — no OCR venv ships in the zip; nothing extra to bundle."
  echo
  echo "Inspect, then run scripts/release-publish.sh"
}

main "$@"
