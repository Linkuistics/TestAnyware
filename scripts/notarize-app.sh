#!/usr/bin/env bash
#
# Notarise a signed release bundle of the TestAnyware app, staple the ticket and archive the
# stapled bundle as the zip that the cask downloads.
#
#   notarize-app.sh <bundle> <out-zip>
#
# The zip is made after the staple, because the ticket lives in the bundle. A zip made earlier
# carries no ticket, and a Mac that downloads it has to reach Apple to launch the app.
#
# Notarisation uses a keychain profile of notarytool, which the owner stores once from an App
# Store Connect API key. Env: TESTANYWARE_NOTARY_PROFILE (default testanyware-notary). There is no
# fallback: a missing profile or a rejected submission fails the script and leaves no zip.

set -euo pipefail
IFS=$'\n\t'
trap 'echo "notarize-app: error on line $LINENO" >&2' ERR

readonly NOTARY_PROFILE=${TESTANYWARE_NOTARY_PROFILE:-testanyware-notary}
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

die() {
  echo "notarize-app: $*" >&2
  exit 1
}

(($# == 2)) || die "usage: notarize-app.sh <bundle> <out-zip>"
bundle=$1
out_zip=$2

# Refuse a bundle of another id or team before anything leaves this Mac.
"$root/scripts/verify-app.sh" --identity-only "$bundle"

# Ask notarytool itself, before an upload: a submission is minutes of Apple's time.
probe=$(xcrun notarytool history --keychain-profile "$NOTARY_PROFILE" 2>&1 || true)
if grep -q 'No Keychain password item found' <<<"$probe"; then
  {
    echo "notarize-app: no notary keychain profile \"$NOTARY_PROFILE\"."
    echo
    echo "  Create an App Store Connect API key with the Developer role at"
    echo "  https://appstoreconnect.apple.com/access/integrations/api,"
    echo "  download AuthKey_<KEYID>.p8 (it downloads once), and store it:"
    echo
    echo "    xcrun notarytool store-credentials \"$NOTARY_PROFILE\" \\"
    echo "        --key /path/to/AuthKey_<KEYID>.p8 --key-id <KEYID> --issuer <ISSUER-UUID>"
    echo
    echo "  To use a profile of team TA43A4RUP3 that exists under another name, set"
    echo "  TESTANYWARE_NOTARY_PROFILE to that name. A release is never unnotarized."
  } >&2
  exit 1
fi

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
submission=$work/submission.zip
log=$work/submission.log

echo "notarize-app: submitting $bundle as profile \"$NOTARY_PROFILE\""
ditto -c -k --keepParent "$bundle" "$submission"

# One submission, waited on in the foreground. The verdict is read from its own output.
status=0
xcrun notarytool submit "$submission" --keychain-profile "$NOTARY_PROFILE" --wait 2>&1 |
  tee "$log" || status=$?
submission_id=$(awk '/^ *id: /{print $2; exit}' "$log")
if [[ "$status" != 0 ]] || ! grep -qE '^ *status: Accepted' "$log"; then
  echo "notarize-app: notarisation did not return Accepted." >&2
  if [[ -n "$submission_id" ]]; then
    echo "notarize-app: Apple's reasons for submission $submission_id:" >&2
    xcrun notarytool log "$submission_id" --keychain-profile "$NOTARY_PROFILE" 2>&1 | sed 's/^/    /' >&2
  fi
  exit 1
fi
echo "notarize-app: accepted (submission $submission_id)."

xcrun stapler staple "$bundle"
xcrun stapler validate "$bundle"

mkdir -p "$(dirname "$out_zip")"
rm -f "$out_zip"
ditto -c -k --keepParent "$bundle" "$out_zip"
echo "notarize-app: notarized and stapled $bundle, archived as $out_zip"
