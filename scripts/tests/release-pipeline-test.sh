#!/usr/bin/env bash
#
# Tests of the release pipeline that need neither Apple's notary service nor GitHub nor the
# cross builds (leaf app-release-bundle-k22). Runs on a macOS host with the Developer ID of team
# TA43A4RUP3 in the keychain, and with a network for the secure timestamp of the release signature.
#
#   1. scripts/tests/test_release_candidate.py: the tag gate, the manifest, the rendering.
#   2. release-build.sh refuses a bad tag before it touches anything.
#   3. The cask template renders to valid Ruby with the stanzas that ADR-0020 requires.
#   4. verify-app.sh on real signed bundles: a development bundle, a bundle of another team and a
#      release bundle that is not notarised are refused. A release bundle passes the identity check.
#   5. release-publish.sh against local bare repositories and a fake gh: the order of the steps, and
#      each refusal before anything is pushed, a Windows host archive in the candidate included.
#   6. release-build.sh stops when the app is not notarised, and its targets name no Windows host.

# shellcheck disable=SC2015,SC2016  # SC2016: the stub bodies expand when the stub runs. SC2015: pass never fails, so `A && pass || fail` is an if-else here
set -euo pipefail
IFS=$'\n\t'

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
failures=0

pass() { echo "  ok   $*"; }
fail() {
  echo "  FAIL $*" >&2
  failures=$((failures + 1))
}

# expect_refusal <description> <message-regex> <command...>: the command fails and says why.
expect_refusal() {
  local description=$1 pattern=$2
  shift 2
  local out status=0
  out=$("$@" 2>&1) || status=$?
  if ((status == 0)); then
    fail "$description: the command succeeded"
  elif grep -qE "$pattern" <<<"$out"; then
    pass "$description"
  else
    fail "$description: the message lacks /$pattern/: $out"
  fi
}

echo "1. tag gate, manifest and rendering"
if python3 -I "$root/scripts/tests/test_release_candidate.py" >"$work/py.log" 2>&1; then
  pass "test_release_candidate.py"
else
  fail "test_release_candidate.py"
  cat "$work/py.log" >&2
fi

echo "2. release-build.sh refuses before it touches anything"
before=$(git -C "$root" status --porcelain)
git -C "$root" rev-parse -q --verify refs/tags/v9.9.9 >/dev/null && {
  echo "the tag v9.9.9 exists; the test needs it absent" >&2
  exit 1
}
expect_refusal "no tag" "usage" "$root/scripts/release-build.sh"
expect_refusal "a tag that does not exist" "does not exist" "$root/scripts/release-build.sh" v9.9.9
expect_refusal "a tag that is not vMAJOR.MINOR.PATCH" "not vMAJOR" "$root/scripts/release-build.sh" 9.9.9
[[ "$(git -C "$root" status --porcelain)" == "$before" ]] && pass "the tree is as it was" ||
  fail "a refused build changed the tree"
[[ ! -e "$root/target/candidates/v9.9.9" ]] && pass "no candidate directory" || fail "a refused build made a directory"

echo "3. the cask template"
"$root/scripts/release_candidate.py" render --template "$root/scripts/templates/testanyware-app.rb.tmpl" \
  --out "$work/testanyware-app.rb" VERSION=1.2.3 SHA_APP=abc123
cask=$work/testanyware-app.rb
ruby -c "$cask" >/dev/null && pass "the cask is valid Ruby" || fail "the cask is not valid Ruby"
code=$(grep -v '^ *#' "$cask")
has() { grep -qF "$1" <<<"$code" && pass "cask: $1" || fail "cask lacks: $1"; }
lacks() { grep -qiE "$1" <<<"$code" && fail "cask has /$1/" || pass "cask has no /$1/"; }
has 'cask "testanyware-app" do'
has 'version "1.2.3"'
has 'sha256 "abc123"'
has 'testanyware-app-v#{version}-aarch64-apple-darwin.zip'
has 'app "TestAnyware.app"'
has 'depends_on formula: "linkuistics/taps/testanyware"'
has 'depends_on arch: :arm64'
has 'uninstall quit: "dev.antony.TestAnyware"'
has '"~/Library/Application Support/TestAnyware"'
lacks 'postflight|xattr|quarantine'
zap=$(awk '/^ *zap /,/\]/' "$cask")
grep -qE '\.local|vms|golden|state' <<<"$zap" && fail "the zap names VM state: $zap" || pass "the zap names no VM state"
grep -q '@' "$cask" && fail "the cask keeps a placeholder" || pass "no placeholder is left"

"$root/scripts/release_candidate.py" render --template "$root/scripts/templates/testanyware.rb.tmpl" \
  --out "$work/testanyware.rb" VERSION=1.2.3 SHA_AARCH64_APPLE_DARWIN=aa SHA_AARCH64_LINUX=bb SHA_X86_64_LINUX=cc
ruby -c "$work/testanyware.rb" >/dev/null && pass "the formula is valid Ruby" || fail "the formula is not valid Ruby"
grep -qE '@[A-Z0-9_]+@' "$work/testanyware.rb" && fail "the formula keeps a placeholder" || pass "the formula keeps no placeholder"
[[ "$(grep -m1 '^ *version ' "$work/testanyware.rb")" == "$(grep -m1 '^ *version ' "$cask")" ]] &&
  grep -q 'version "1.2.3"' "$cask" &&
  pass "the formula and the cask take one version" || fail "the formula and the cask differ in version"

echo "4. verify-app.sh on signed bundles"
dev=$(TESTANYWARE_APP_OUT="$work/app" "$root/scripts/build-app.sh" dev | tail -n1)
release=$(TESTANYWARE_APP_OUT="$work/app" "$root/scripts/build-app.sh" release | tail -n1)
expect_refusal "a development bundle" "bundle id is \"dev.antony.TestAnyware.dev\"" \
  "$root/scripts/verify-app.sh" --identity-only "$dev"
cp -R "$release" "$work/other-team.app"
codesign --force --options runtime --sign - "$work/other-team.app" 2>/dev/null
expect_refusal "a bundle of no team" "team \"not set\", not TA43A4RUP3" \
  "$root/scripts/verify-app.sh" --identity-only "$work/other-team.app"
"$root/scripts/verify-app.sh" --identity-only --version 0.0.1 "$release" >/dev/null &&
  pass "the release bundle passes the identity check" || fail "the release bundle fails the identity check"
expect_refusal "a bundle of another version" "not 9.9.9" \
  "$root/scripts/verify-app.sh" --identity-only --version 9.9.9 "$release"
expect_refusal "a release bundle that is not notarised" "spctl" "$root/scripts/verify-app.sh" "$release"
expect_refusal "notarisation of a development bundle" "bundle id is" \
  "$root/scripts/notarize-app.sh" "$dev" "$work/dev.zip"
[[ ! -e "$work/dev.zip" ]] && pass "a refused notarisation leaves no zip" || fail "a zip exists"

echo "5. release-publish.sh against local repositories and a fake gh"
make_scene() {
  scene=$work/scene-$1
  mkdir -p "$scene/bin" "$scene/repo/scripts" "$scene/repo/target/candidates"
  cp "$root/scripts/release-publish.sh" "$root/scripts/release_candidate.py" "$scene/repo/scripts/"
  git init -q --bare "$scene/remote.git"
  git init -q --bare "$scene/tap.git"
  git init -q "$scene/tap" && git -C "$scene/tap" remote add origin "$scene/tap.git"
  git -C "$scene/tap" -c user.name=t -c user.email=t@example.com commit -q --allow-empty -m init
  git -C "$scene/tap" push -q origin HEAD:refs/heads/main
  git -C "$scene/tap" branch -q --set-upstream-to=origin/main
  git -C "$scene/repo" init -q
  echo one >"$scene/repo/a.txt"
  git -C "$scene/repo" add a.txt scripts
  git -C "$scene/repo" -c user.name=t -c user.email=t@example.com commit -q -m one
  git -C "$scene/repo" -c user.name=t -c user.email=t@example.com tag -a v1.2.3 -m "Release v1.2.3"
  commit=$(git -C "$scene/repo" rev-parse HEAD)
  local dir=$scene/repo/target/candidates/v1.2.3
  mkdir -p "$dir"
  echo mac >"$dir/testanyware-v1.2.3-aarch64-apple-darwin.tar.xz"
  echo linux >"$dir/testanyware-v1.2.3-aarch64-unknown-linux-gnu.tar.xz"
  echo app >"$dir/testanyware-app-v1.2.3-aarch64-apple-darwin.zip"
  echo formula >"$dir/testanyware.rb"
  echo cask >"$dir/testanyware-app.rb"
  python3 -I "$scene/repo/scripts/release_candidate.py" write --dir "$dir" --tag v1.2.3 --commit "$commit" >/dev/null
  # The fake gh records its calls, and whether the tag was on the remote when the release was made.
  cat >"$scene/bin/gh" <<GH
#!/usr/bin/env bash
if [[ "\$1 \$2" == "auth status" ]]; then exit 0; fi
{
  echo "gh \$*"
  echo "tag-on-remote: \$(git -C "$scene/remote.git" rev-parse -q --verify refs/tags/v1.2.3 || echo none)"
} >>"$scene/gh.log"
GH
  chmod +x "$scene/bin/gh"
}

run_publish() {
  PATH="$scene/bin:$PATH" TESTANYWARE_TAP_DIR="$scene/tap" TESTANYWARE_RELEASE_REMOTE="$scene/remote.git" \
    "$scene/repo/scripts/release-publish.sh" "$@"
}

untouched() {
  [[ ! -e "$scene/gh.log" ]] && pass "$1: gh did not run" || fail "$1: gh ran"
  [[ -z "$(git -C "$scene/remote.git" tag)" ]] && pass "$1: no tag reached the remote" || fail "$1: a tag reached the remote"
  [[ "$(git -C "$scene/tap.git" rev-list --count main)" == 1 ]] && pass "$1: the tap is as it was" ||
    fail "$1: the tap changed"
}

make_scene happy
run_publish v1.2.3 >"$scene/out.log" 2>&1 && pass "a recorded candidate publishes" || {
  fail "a recorded candidate publishes"
  cat "$scene/out.log" >&2
}
[[ "$(git -C "$scene/remote.git" rev-parse 'refs/tags/v1.2.3^{commit}')" == "$commit" ]] &&
  pass "the remote holds the existing tag at the candidate's commit" || fail "the remote tag differs"
[[ "$(git -C "$scene/remote.git" cat-file -t refs/tags/v1.2.3)" == tag ]] &&
  pass "the pushed tag is the annotated one" || fail "the pushed tag is not annotated"
grep -qE '^gh release create v1.2.3 --repo Linkuistics/TestAnyware --verify-tag ' "$scene/gh.log" &&
  pass "gh creates the release for the tag and never makes one (--verify-tag)" || fail "gh arguments: $(cat "$scene/gh.log")"
grep -qE "^tag-on-remote: [0-9a-f]{40}$" "$scene/gh.log" &&
  pass "the tag reached the remote before the release" || fail "the release came before the tag"
for asset in testanyware-v1.2.3-aarch64-apple-darwin.tar.xz testanyware-v1.2.3-aarch64-unknown-linux-gnu.tar.xz \
  testanyware-app-v1.2.3-aarch64-apple-darwin.zip; do
  grep -q "/$asset" "$scene/gh.log" && pass "release asset $asset" || fail "release lacks $asset"
done
grep -qE 'manifest.json|\.rb( |$)' "$scene/gh.log" && fail "the manifest or a formula became an asset" ||
  pass "only the archives are assets"
grep -qE 'pc-windows' "$scene/gh.log" && fail "a Windows host archive became an asset" ||
  pass "no Windows host archive is an asset"
[[ "$(git -C "$scene/tap.git" rev-list --count main)" == 2 ]] && pass "the tap has one new commit" || fail "tap commits"
[[ "$(git -C "$scene/tap.git" show main:Formula/testanyware.rb)" == formula ]] &&
  [[ "$(git -C "$scene/tap.git" show main:Casks/testanyware-app.rb)" == cask ]] &&
  pass "the formula and the cask are in that commit" || fail "the tap lacks the formula or the cask"

make_scene windows-host
echo win >"$scene/repo/target/candidates/v1.2.3/testanyware-v1.2.3-aarch64-pc-windows-gnullvm.zip"
expect_refusal "a Windows host archive in a candidate" "Windows host archive" run_publish v1.2.3
untouched "windows host"

make_scene tampered
echo changed >"$scene/repo/target/candidates/v1.2.3/testanyware-app-v1.2.3-aarch64-apple-darwin.zip"
expect_refusal "an artifact that differs from the manifest" "has sha256" run_publish v1.2.3
untouched "tampered"

make_scene moved
echo two >"$scene/repo/a.txt"
git -C "$scene/repo" -c user.name=t -c user.email=t@example.com commit -q -am two
git -C "$scene/repo" -c user.name=t -c user.email=t@example.com tag -fa v1.2.3 -m moved >/dev/null
expect_refusal "a tag that names another commit" "candidate is void" run_publish v1.2.3
untouched "moved"

make_scene lightweight
git -C "$scene/repo" tag -d v1.2.3 >/dev/null
git -C "$scene/repo" tag v1.2.3
expect_refusal "a lightweight tag" "lightweight" run_publish v1.2.3
untouched "lightweight"

make_scene missing-tag
git -C "$scene/repo" tag -d v1.2.3 >/dev/null
expect_refusal "a tag that does not exist locally" "does not exist" run_publish v1.2.3
untouched "missing tag"

make_scene no-candidate
expect_refusal "a candidate that was not built" "no candidate" run_publish v1.2.4
untouched "no candidate"

make_scene remote-differs
# The remote holds v1.2.3 at a second commit. The candidate was built from the first.
echo two >"$scene/repo/a.txt"
git -C "$scene/repo" -c user.name=t -c user.email=t@example.com commit -q -am two
git -C "$scene/repo" push -q "$scene/remote.git" HEAD:refs/heads/main
git -C "$scene/remote.git" update-ref refs/tags/v1.2.3 "$(git -C "$scene/repo" rev-parse HEAD)"
expect_refusal "a remote that holds the tag at another commit" "holds v1.2.3 at" run_publish v1.2.3
[[ ! -e "$scene/gh.log" ]] && pass "remote-differs: gh did not run" || fail "remote-differs: gh ran"

make_scene no-arg
expect_refusal "no tag" "usage" run_publish

echo "6. release-build.sh stops when the app is not notarised"
# A scratch repository with the real build script and gate, and stubs for the steps that need Apple
# and the toolchains. bash does not carry `set -e` into $(...), so the test proves that a failed
# notarisation ends the build before any cross build or manifest.
scene=$work/scene-build
mkdir -p "$scene/scripts/templates" "$scene/cli-rs"
cp "$root/scripts/release-build.sh" "$root/scripts/release_candidate.py" "$scene/scripts/"
cp "$root/scripts/templates/"*.tmpl "$scene/scripts/templates/"
printf '[workspace.package]\nversion = "0.0.1"\n' >"$scene/cli-rs/Cargo.toml"
echo lock >"$scene/cli-rs/Cargo.lock"
stub() { printf '#!/usr/bin/env bash\n%s\n' "$2" >"$scene/scripts/$1"; }
stub release-doctor.sh 'exit 0'
stub build-app.sh 'mkdir -p "$TESTANYWARE_APP_OUT/release/TestAnyware.app/Contents"; echo "$TESTANYWARE_APP_OUT/release/TestAnyware.app"'
stub verify-app.sh 'exit 0'
stub notarize-app.sh 'echo "notarize-app: stub rejection" >&2; exit 1'
chmod +x "$scene/scripts/"*.sh "$scene/scripts/release_candidate.py"
git -C "$scene" init -q
git -C "$scene" add .
git -C "$scene" -c user.name=t -c user.email=t@example.com commit -q -m one
git -C "$scene" -c user.name=t -c user.email=t@example.com tag -a v1.2.3 -m r
status=0
"$scene/scripts/release-build.sh" v1.2.3 >"$work/build.log" 2>&1 || status=$?
((status != 0)) && pass "a rejected notarisation fails the build" || fail "the build succeeded"
targets=$(grep '^release-build: targets:' "$work/build.log" || true)
[[ "$targets" == "release-build: targets: aarch64-apple-darwin aarch64-unknown-linux-gnu x86_64-unknown-linux-gnu" ]] &&
  pass "the build names the macOS and Linux targets" || fail "the targets line: ${targets:-missing}"
grep -q 'pc-windows' "$work/build.log" && fail "the build names a Windows host target" ||
  pass "the build names no Windows host target"
grep -q "stub rejection" "$work/build.log" && pass "the rejection is shown" || fail "the rejection is not shown: $(cat "$work/build.log")"
grep -qE 'building macOS agent|packaging|rendered' "$work/build.log" &&
  fail "the build ran on after the rejection" || pass "nothing was built after the rejection"
[[ ! -e "$scene/target/candidates/v1.2.3/manifest.json" ]] && pass "no manifest" || fail "a manifest exists"
[[ "$(awk -F'"' '/^version/ {print $2}' "$scene/cli-rs/Cargo.toml")" == 0.0.1 ]] &&
  pass "Cargo.toml has its development version again" || fail "Cargo.toml keeps the release version"

echo
if ((failures == 0)); then
  echo "release-pipeline-test: all passed"
else
  echo "release-pipeline-test: $failures failed" >&2
  exit 1
fi
