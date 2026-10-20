#!/usr/bin/env bash
#
# Does the signed development bundle of the TestAnyware app start through LaunchServices and
# answer on its socket? Seconds, on a macOS host, no guest.
#
#   app-bundle-live.sh
#
# Steps:
#   1. Refuse to start when something already answers on the socket.
#   2. Build the development bundle (scripts/build-app.sh dev) and check its Info.plist.
#   3. `open` the bundle, as macOS starts a login item, and wait for `GET /v1/status`.
#   4. Check that `build` in the status is the bundle's version, and that the socket file
#      is mode 0600 in a directory of mode 0700.
#   5. Read the status item's menu through System Events. Without the Accessibility grant for
#      the terminal this step reports SKIP, and the run still passes.
#   6. Quit through the Apple event that the Dock's Quit sends, and check that the process
#      is gone and the socket file with it.
#   7. Open the bundle again with TESTANYWARE_CONFIG naming a config.rhai that fails (ADR-0021),
#      check that the status reports the CONFIG_INVALID error and that the menu shows the file,
#      the error and an item that opens the file, and quit.
#
# The app uses the real socket, ~/Library/Application Support/TestAnyware/testanyware.sock,
# because the bundle takes no arguments. Always quits the app that it started.

set -euo pipefail
IFS=$'\n\t'
trap 'echo "app-bundle-live: error on line $LINENO" >&2' ERR

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
socket="$HOME/Library/Application Support/TestAnyware/testanyware.sock"
readonly bundle_id=dev.antony.TestAnyware.dev

fail() {
  echo "app-bundle-live: FAIL: $*" >&2
  exit 1
}

status() { curl -sf --max-time 2 --unix-socket "$socket" http://localhost/v1/status; }

started=0
cleanup() {
  if ((started)); then
    osascript -e "tell application id \"$bundle_id\" to quit" >/dev/null 2>&1 || true
    sleep 1
    pkill -f "TestAnyware Dev.app/Contents/MacOS" 2>/dev/null || true
  fi
}
trap cleanup EXIT

# 1
if status >/dev/null 2>&1; then
  fail "an app already answers on $socket. Quit it first, because the bundle uses this socket."
fi

# 2
bundle=$("$root/scripts/build-app.sh" dev | tail -n 1)
plist=$bundle/Contents/Info.plist
version=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$plist")
[[ "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$plist")" == "$bundle_id" ]] ||
  fail "the bundle id is not $bundle_id."
[[ "$(/usr/libexec/PlistBuddy -c 'Print :LSUIElement' "$plist")" == true ]] ||
  fail "LSUIElement is not set."
echo "app-bundle-live: ok   built $bundle ($version)"

# 3
open -n -g "$bundle"
started=1
body=
for _ in $(seq 1 50); do
  if body=$(status 2>/dev/null); then break; fi
  sleep 0.2
done
[[ -n "$body" ]] || fail "no answer on the socket within 10 s."
echo "app-bundle-live: ok   GET /v1/status answers"

# 4
build=$(jq -r .build <<<"$body")
[[ "$build" == "$version" ]] ||
  fail "the status says build \"$build\", and the bundle's version is \"$version\"."
echo "app-bundle-live: ok   build $build matches the bundle"
configuration=$(jq -r .configuration.state <<<"$body")
case "$configuration" in
  absent | loaded) ;;
  *) fail "the status reports configuration \"$configuration\", and the user's own file is absent or it loads." ;;
esac
echo "app-bundle-live: ok   the status reports the configuration ($configuration)"
pgrep -f "$bundle/Contents/MacOS/testanyware-app" >/dev/null ||
  fail "no process runs the bundle's executable."
[[ "$(stat -f %Lp "$socket")" == 600 ]] || fail "the socket file is not mode 0600."
[[ "$(stat -f %Lp "$(dirname "$socket")")" == 700 ]] || fail "the socket directory is not mode 0700."

# 5
# AXCancel closes the menu. The Escape key does not, on macOS 27, and an open menu keeps the
# app in its menu tracking loop, which handles no Apple event, so the quit in step 6 times out.
read_menu() {
  osascript 2>/dev/null <<'EOF' || true
tell application "System Events"
  tell process "testanyware-app"
    click menu bar item 1 of menu bar 1
    delay 0.4
    set t to title of every menu item of menu 1 of menu bar item 1 of menu bar 1
    perform action "AXCancel" of menu 1 of menu bar item 1 of menu bar 1
    return t
  end tell
end tell
EOF
}
menu=$(read_menu)
if [[ -z "$menu" ]]; then
  echo "app-bundle-live: SKIP the menu: System Events cannot read the status item (Accessibility grant?)"
else
  for want in "TestAnyware $version" "Serving its socket" "Local Network:" "Start at Login" "Configuration:" "each VM keeps running"; do
    grep -qF "$want" <<<"$menu" || fail "the menu lacks \"$want\": $menu"
  done
  echo "app-bundle-live: ok   the menu shows the version, the socket, the Local Network state, Start at Login, the configuration and Quit"
fi

# 6
quit_app() {
  osascript -e "tell application id \"$bundle_id\" to quit" >/dev/null
  for _ in $(seq 1 25); do
    pgrep -f "$bundle/Contents/MacOS/testanyware-app" >/dev/null || break
    sleep 0.2
  done
  if pgrep -f "$bundle/Contents/MacOS/testanyware-app" >/dev/null; then
    fail "the app did not quit."
  fi
  started=0
  [[ ! -e "$socket" ]] || fail "the socket file stays after a quit."
}
quit_app
echo "app-bundle-live: ok   quit leaves no process and no socket file"

# 7
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"; cleanup' EXIT
printf 'let ok = 1;\nlet bad = ;\n' >"$scratch/config.rhai"
open -n -g --env "TESTANYWARE_CONFIG=$scratch/config.rhai" "$bundle"
started=1
body=
for _ in $(seq 1 50); do
  if body=$(status 2>/dev/null); then break; fi
  sleep 0.2
done
[[ -n "$body" ]] || fail "no answer on the socket within 10 s with a config.rhai that fails."
[[ "$(jq -r .configuration.state <<<"$body")" == failed ]] ||
  fail "the status does not report the file as failed: $(jq -c .configuration <<<"$body")"
[[ "$(jq -r .configuration.error.code <<<"$body")" == CONFIG_INVALID ]] ||
  fail "the status carries no CONFIG_INVALID error: $(jq -c .configuration <<<"$body")"
[[ "$(jq -r .configuration.error.details.line <<<"$body")" == 2 ]] ||
  fail "the error does not name line 2: $(jq -c .configuration.error <<<"$body")"
echo "app-bundle-live: ok   a config.rhai that fails runs the app on its defaults and the status reports the error"
menu=$(read_menu)
if [[ -z "$menu" ]]; then
  echo "app-bundle-live: SKIP the menu of a failing config.rhai: System Events cannot read the status item"
else
  for want in "$scratch/config.rhai (failed)" "Error at line 2:" "Last evaluated:" "The app runs with its built-in defaults" "Open config.rhai"; do
    grep -qF "$want" <<<"$menu" || fail "the menu lacks \"$want\": $menu"
  done
  echo "app-bundle-live: ok   the menu shows the file, the time of the evaluation, the error and an item that opens the file"
fi
quit_app
echo "app-bundle-live: ok   the second quit leaves no process and no socket file"
echo "app-bundle-live: PASS"
