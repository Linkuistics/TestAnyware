#!/usr/bin/env bash
#
# Builds and signs the TestAnyware app bundle for one channel (spec: "Packaging and identity",
# ADR-0020).
#
#   build-app.sh dev|release
#
# The channel fixes the bundle id and nothing else does:
#   dev      dev.antony.TestAnyware.dev   TestAnyware Dev.app
#   release  dev.antony.TestAnyware       TestAnyware.app
# The two ids differ, so a development build never shares a Local Network row with a release.
#
# The script signs with the Developer ID Application identity of team TA43A4RUP3 and the
# hardened runtime. It refuses any other team before it builds, and it reads the bundle id and
# the team back from the signed bundle. There is no ad-hoc fallback: a bundle with another
# signature loses the Local Network grant and the login item.
#
# Notarisation belongs to the release leaf. A development bundle needs the signature only.
#
# Env: TESTANYWARE_SIGNING_IDENTITY (default the team's Developer ID Application identity; the
# team in its name must still be TA43A4RUP3); TESTANYWARE_APP_OUT (default cli-rs/target/app).
# Prints the bundle path on its last line.

set -euo pipefail
IFS=$'\n\t'
trap 'echo "build-app: error on line $LINENO" >&2' ERR

readonly TEAM_ID=TA43A4RUP3
readonly EXECUTABLE=testanyware-app

die() {
  echo "build-app: $*" >&2
  exit 1
}

plist_string() { /usr/libexec/PlistBuddy -c "Print :$1" "$2" 2>/dev/null; }

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
channel=${1:-}
case "$channel" in
  dev)
    bundle_id=dev.antony.TestAnyware.dev
    app_name="TestAnyware Dev"
    timestamp=--timestamp=none
    ;;
  release)
    bundle_id=dev.antony.TestAnyware
    app_name=TestAnyware
    # Notarisation needs the secure timestamp.
    timestamp=--timestamp
    ;;
  *) die "usage: build-app.sh dev|release" ;;
esac

identity=${TESTANYWARE_SIGNING_IDENTITY:-Developer ID Application: Antony Blakey ($TEAM_ID)}
# The team is the parenthesised token at the end of the identity's name.
identity_team=${identity##*(}
identity_team=${identity_team%)}
[[ "$identity_team" == "$TEAM_ID" ]] ||
  die "the signing identity \"$identity\" is not of team $TEAM_ID. The app keeps one team in every channel."
security find-identity -v -p codesigning | grep -qF "\"$identity\"" ||
  die "no valid code-signing identity \"$identity\" in the keychain."

version=$(awk '/^\[workspace.package\]/ { in_package = 1; next } /^\[/ { in_package = 0 } in_package && $1 == "version" { gsub(/"/, "", $3); print $3; exit }' "$root/cli-rs/Cargo.toml")
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] ||
  die "the workspace version \"$version\" is not MAJOR.MINOR.PATCH."

echo "build-app: building $EXECUTABLE ($channel, $version)" >&2
(cd "$root/cli-rs" && cargo build --release -p testanyware-app)
binary=$root/cli-rs/target/release/$EXECUTABLE
[[ -x "$binary" ]] || die "the build did not produce $binary."

out=${TESTANYWARE_APP_OUT:-$root/cli-rs/target/app}
bundle=$out/$channel/$app_name.app
# Wipe first: the bundle is a pure function of the source tree.
rm -rf "$bundle"
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
cp "$binary" "$bundle/Contents/MacOS/$EXECUTABLE"

# LSUIElement keeps the app out of the Dock and still lets it receive the Local Network alert.
# A background-only app never receives it (spec: "The TestAnyware app").
cat >"$bundle/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key>
  <string>$bundle_id</string>
  <key>CFBundleName</key>
  <string>$app_name</string>
  <key>CFBundleDisplayName</key>
  <string>$app_name</string>
  <key>CFBundleExecutable</key>
  <string>$EXECUTABLE</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$version</string>
  <key>CFBundleVersion</key>
  <string>$version</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>LSUIElement</key>
  <true/>
  <key>NSLocalNetworkUsageDescription</key>
  <string>TestAnyware connects to the virtual machines that run on this Mac, so that commands from a terminal and from agents can drive them.</string>
</dict>
</plist>
PLIST
plutil -lint "$bundle/Contents/Info.plist" >/dev/null

[[ "$(plist_string CFBundleIdentifier "$bundle/Contents/Info.plist")" == "$bundle_id" ]] ||
  die "the bundle's CFBundleIdentifier is not $bundle_id."

xattr -cr "$bundle"
echo "build-app: signing as \"$identity\"" >&2
codesign --force --options runtime "$timestamp" --sign "$identity" "$bundle"

# Read the signature back. The checks above trust the script, and these trust the bundle.
codesign --verify --strict --deep "$bundle" ||
  die "the signed bundle does not verify."
details=$(codesign -dvv "$bundle" 2>&1)
signed_id=$(sed -n 's/^Identifier=//p' <<<"$details")
signed_team=$(sed -n 's/^TeamIdentifier=//p' <<<"$details")
[[ "$signed_id" == "$bundle_id" ]] ||
  die "the signed bundle has the id \"$signed_id\", not $bundle_id."
[[ "$signed_team" == "$TEAM_ID" ]] ||
  die "the signed bundle has the team \"$signed_team\", not $TEAM_ID."
grep -q '^CodeDirectory.*flags=.*(runtime)' <<<"$details" ||
  die "the signed bundle lacks the hardened runtime."

echo "$bundle"
