#!/usr/bin/env bash
#
# Verify the release identity of a TestAnyware app bundle (spec: "Packaging and identity",
# ADR-0020).
#
#   verify-app.sh [--identity-only] [--version X.Y.Z] <bundle>
#
# The bundle must carry the bundle id dev.antony.TestAnyware and the team TA43A4RUP3, in its
# Info.plist and in its signature, with the hardened runtime. A development bundle fails here, and
# so does a bundle of another team: either one would lose the Local Network grant of every user.
#
# Without --identity-only the script also asks macOS what the release asks of it: codesign
# --verify, spctl --assess (it must say Notarized Developer ID) and stapler validate. Notarisation
# staples the ticket, so a bundle that notarize-app.sh has not finished fails the last two.
#
# --version X.Y.Z also requires CFBundleShortVersionString and CFBundleVersion to equal X.Y.Z.

set -euo pipefail
IFS=$'\n\t'

readonly BUNDLE_ID=dev.antony.TestAnyware
readonly TEAM_ID=TA43A4RUP3

die() {
  echo "verify-app: $*" >&2
  exit 1
}

plist_string() { /usr/libexec/PlistBuddy -c "Print :$1" "$2" 2>/dev/null; }

identity_only=0
version=
while (($#)); do
  case "$1" in
    --identity-only) identity_only=1 ;;
    --version)
      shift
      version=${1:-}
      [[ -n "$version" ]] || die "--version needs a value."
      ;;
    -*) die "unknown option $1." ;;
    *) break ;;
  esac
  shift
done
(($# == 1)) || die "usage: verify-app.sh [--identity-only] [--version X.Y.Z] <bundle>"
bundle=$1
[[ -d "$bundle/Contents" ]] || die "$bundle is not an app bundle."
info=$bundle/Contents/Info.plist

plist_id=$(plist_string CFBundleIdentifier "$info") || die "$info has no CFBundleIdentifier."
[[ "$plist_id" == "$BUNDLE_ID" ]] ||
  die "the bundle id is \"$plist_id\", not $BUNDLE_ID. A release bundle has one id in every version."

if [[ -n "$version" ]]; then
  short=$(plist_string CFBundleShortVersionString "$info") || short=
  build=$(plist_string CFBundleVersion "$info") || build=
  [[ "$short" == "$version" && "$build" == "$version" ]] ||
    die "the bundle is version \"$short\" (build \"$build\"), not $version. Rebuild it."
fi

codesign --verify --strict --deep "$bundle" 2>&1 || die "the signature does not verify."
details=$(codesign -dvv "$bundle" 2>&1)
signed_id=$(sed -n 's/^Identifier=//p' <<<"$details")
signed_team=$(sed -n 's/^TeamIdentifier=//p' <<<"$details")
[[ "$signed_id" == "$BUNDLE_ID" ]] ||
  die "the signature names the id \"$signed_id\", not $BUNDLE_ID."
[[ "$signed_team" == "$TEAM_ID" ]] ||
  die "the signature names the team \"$signed_team\", not $TEAM_ID. The app keeps one team in every release."
grep -q '^CodeDirectory.*flags=.*(runtime)' <<<"$details" ||
  die "the signature lacks the hardened runtime."

if ((identity_only)); then
  echo "verify-app: $bundle has the id $BUNDLE_ID and the team $TEAM_ID."
  exit 0
fi

assessment=$(spctl --assess --type execute --verbose=2 "$bundle" 2>&1) ||
  die "spctl rejects the bundle: $assessment"
grep -q 'source=Notarized Developer ID' <<<"$assessment" ||
  die "spctl accepts the bundle, but not as Notarized Developer ID: $assessment"
xcrun stapler validate "$bundle" >/dev/null 2>&1 ||
  die "the bundle has no valid stapled ticket. Run scripts/notarize-app.sh."

echo "verify-app: $bundle is $BUNDLE_ID of team $TEAM_ID, signed with the hardened runtime, notarized and stapled."
