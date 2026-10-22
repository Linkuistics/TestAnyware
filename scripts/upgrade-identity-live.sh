#!/usr/bin/env bash
#
# Does a cask upgrade of the TestAnyware app keep its identity? Release N is installed and
# allowed, then the cask installs release N+1. On a macOS host, with one free macOS guest slot.
#
#   upgrade-identity-live.sh run   <old-tag> <new-tag>
#   upgrade-identity-live.sh clean <old-tag> <new-tag>
#
# <old-tag> is release N: a throwaway local tag with a lower version, built with
# `task release:candidate`. <new-tag> is release N+1: the release candidate. Nothing is published.
# A local tap, `local/upgrade-check`, holds the cask of each release with its url set to the zip of
# the candidate. The formula is not touched: the cask depends on it, and the installed one stands.
#
# `run` can stop at a step that only a person can do, and it says which. Run it again after the
# step. It reads the state of the Mac each time and goes on from there:
#   1. The bundle is not installed: install N from the tap.
#   2. N is installed: open it, start a guest through it, and record the baseline. A person
#      answers the "downloaded from the Internet" dialog once (the open waits up to
#      MAX_FIRST_OPEN_SECS for it), and allows the Local Network alert once. This script never
#      waits for the alert. A denial stops the run and names the step.
#   3. The baseline is recorded: `brew upgrade --cask` installs N+1. Homebrew quits N, replaces
#      the bundle and opens it again.
#   4. N+1 is installed: check what the upgrade kept (the signing identity, the approval of the
#      first start, the guest and its processes, the adoption), and send the first command to
#      the guest. It succeeds, or it fails with the denial that names TestAnyware. In that
#      case, a person allows the app again and runs this again.
#
# The check records the facts that a person sees (the dialog, the rows of System Settings, the
# alert) as a list at the end. They are not machine-checkable, and the log of the leaf names who
# observed them.
#
# `clean` stops the guest, quits the app, uninstalls the cask, removes the tap, the scratch
# directory and the candidate directory of <old-tag>, and deletes the local tag <old-tag>. It
# refuses a tag that exists on the remote. Both tags must never reach the publication.
#
# Env: MAX_START_SECS (default 240; the bound on `vm start`),
#      MAX_FIRST_OPEN_SECS (default 180; how long a person has to answer the first-start dialog),
#      MAX_REOPEN_SECS (default 60; how long N+1 may take to answer after the upgrade),
#      MAX_ADOPT_SECS (default 40; how long the adoption probes may take).

set -euo pipefail
IFS=$'\n\t'
trap 'echo "upgrade-identity: error on line $LINENO" >&2' ERR

readonly BUNDLE_ID=dev.antony.TestAnyware
readonly TAP=local/upgrade-check
readonly CASK="$TAP/testanyware-app"
readonly APP=/Applications/TestAnyware.app
readonly GUEST=upgrade-check
# Exit status of a stop that waits for a person.
readonly PERSON_STEP=3

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
socket="$HOME/Library/Application Support/TestAnyware/testanyware.sock"
vms_dir="${XDG_STATE_HOME:-$HOME/.local/state}/testanyware/vms"
max_start=${MAX_START_SECS:-240}
max_first_open=${MAX_FIRST_OPEN_SECS:-180}
max_reopen=${MAX_REOPEN_SECS:-60}
max_adopt=${MAX_ADOPT_SECS:-40}

fail() {
  echo "upgrade-identity: FAIL: $*" >&2
  exit 1
}
ok() { echo "upgrade-identity: ok   $*"; }
info() { echo "upgrade-identity: info $*"; }
# A step for a person. The run stops here and goes on when it runs again.
person() {
  echo "upgrade-identity: PERSON STEP: $*" >&2
  exit "$PERSON_STEP"
}

mode=${1:-}
old_tag=${2:-}
new_tag=${3:-}
[[ "$mode" == run || "$mode" == clean ]] && [[ -n "$old_tag" && -n "$new_tag" ]] ||
  fail "usage: upgrade-identity-live.sh run|clean <old-tag> <new-tag>"
[[ "$old_tag" != "$new_tag" ]] || fail "the old and the new tag are the same."
old_version=${old_tag#v}
new_version=${new_tag#v}
old_dir=$root/target/candidates/$old_tag
new_dir=$root/target/candidates/$new_tag
# The sibling directory that git ignores and the candidate check never sees.
scratch=$root/target/candidates/$new_tag.checks/upgrade
state=$scratch/state

for tool in jq tart curl brew xattr codesign dwarfdump spctl xcrun python3 tar git; do
  command -v "$tool" >/dev/null || fail "$tool is not on PATH."
done

# The run proves the default only when nothing names a socket or a guest.
unset TESTANYWARE_APP_SOCKET TESTANYWARE_VM_ID TESTANYWARE_AGENT TESTANYWARE_VNC TESTANYWARE_CONNECT
export HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_INSTALL_CLEANUP=1 HOMEBREW_NO_ENV_HINTS=1

status() { curl -sf --max-time 2 --unix-socket "$socket" http://localhost/v1/status; }
app_pattern="^$APP/Contents/MacOS/testanyware-app\$"
app_pid() { pgrep -f "$app_pattern" | head -n 1; }
tart_names() { tart list --format json | jq -r '.[].Name'; }
tart_state() { tart list --format json | jq -r --arg n "$1" '.[] | select(.Name == $n) | .State'; }
alive() { kill -0 "$1" 2>/dev/null; }
started_at() { ps -o lstart= -p "$1" 2>/dev/null; }
meta_of() { printf '%s/%s.meta.json' "$vms_dir" "$1"; }
bundle_version() { /usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP/Contents/Info.plist" 2>/dev/null; }
tap_dir() { brew --repository "$TAP" 2>/dev/null; }
verify_candidate() { "$root/scripts/release_candidate.py" verify --dir "$1" --tag "$2" >/dev/null; }

# ---- clean -----------------------------------------------------------------------------------

quit_app() {
  osascript -e "tell application id \"$BUNDLE_ID\" to quit" >/dev/null 2>&1 || true
  for _ in $(seq 1 50); do
    [[ -n "$(app_pid || true)" ]] || return 0
    sleep 0.2
  done
  pkill -f "$app_pattern" 2>/dev/null || true
}

clean() {
  # The guest first, through the app that is running, then tart when the app cannot.
  local cli=$scratch/cli-new/bin/testanyware
  if [[ -x "$cli" && -n "$(app_pid || true)" ]] && [[ -f "$(meta_of "$GUEST")" ]]; then
    "$cli" vm stop "$GUEST" >/dev/null 2>&1 || true
  fi
  if tart_names 2>/dev/null | grep -qx "$GUEST"; then
    info "$GUEST survived the app's stop. Removing it with tart."
    tart stop "$GUEST" >/dev/null 2>&1 || true
    tart delete "$GUEST" >/dev/null 2>&1 || true
    rm -f "$vms_dir/$GUEST".json "$vms_dir/$GUEST".meta.json
  fi
  quit_app
  if [[ -d "$APP" ]]; then
    if brew list --cask "$CASK" >/dev/null 2>&1; then
      brew uninstall --cask "$CASK"
    else
      fail "$APP exists and the cask $CASK did not install it. Remove it by hand."
    fi
  fi
  if [[ -n "$(tap_dir)" && -d "$(tap_dir)" ]]; then
    brew untap "$TAP"
  fi
  rm -rf "$scratch" "$old_dir" "$old_dir.checks"
  if git -C "$root" rev-parse -q --verify "refs/tags/$old_tag" >/dev/null; then
    if [[ -n "$(git -C "$root" ls-remote --tags origin "$old_tag")" ]]; then
      fail "the remote has the tag $old_tag. It is published, so this script does not delete it."
    fi
    git -C "$root" tag -d "$old_tag"
  fi
  [[ ! -e "$APP" ]] || fail "$APP is still installed."
  ok "clean: no guest, no app, no tap, no directory of $old_tag, and no local tag $old_tag"
  exit 0
}

if [[ "$mode" == clean ]]; then
  clean
fi

# ---- preflight -------------------------------------------------------------------------------

[[ "$(printf '%s\n%s\n' "$old_version" "$new_version" | sort -V | head -n 1)" == "$old_version" ]] ||
  fail "the old version $old_version is not lower than $new_version."
verify_candidate "$old_dir" "$old_tag" || fail "the candidate $old_tag does not verify."
verify_candidate "$new_dir" "$new_tag" || fail "the candidate $new_tag does not verify."
info "old: $old_tag at $(jq -r .commit "$old_dir/manifest.json")"
info "new: $new_tag at $(jq -r .commit "$new_dir/manifest.json")"

mkdir -p "$scratch" "$state"

# unpack_cli <label> <candidate dir> <version>: the Host CLI of one candidate, for its own app.
unpack_cli() {
  local dir=$2 version=$3 out=$scratch/cli-$1
  if [[ ! -x "$out/bin/testanyware" ]]; then
    rm -rf "$out"
    mkdir -p "$out"
    tar -xJf "$dir/testanyware-v$version-aarch64-apple-darwin.tar.xz" -C "$out" --strip-components=1
  fi
  "$out/bin/testanyware" --version | grep -q "$version" ||
    fail "the CLI $out/bin/testanyware does not report $version."
}
unpack_cli old "$old_dir" "$old_version"
unpack_cli new "$new_dir" "$new_version"
old_cli=$scratch/cli-old/bin/testanyware
new_cli=$scratch/cli-new/bin/testanyware

# ---- helpers ---------------------------------------------------------------------------------

# bundle_facts: what an upgrade must keep or change, one JSON object.
bundle_facts() {
  local exe=$APP/Contents/MacOS/testanyware-app sig req uuid
  sig=$(codesign -dv --verbose=4 "$APP" 2>&1)
  # The designated requirement names the identity. The cdhash belongs to one build.
  req=$(codesign -d -r- "$APP" 2>&1 | sed -n 's/^designated => //p')
  uuid=$(dwarfdump --uuid "$exe" | awk '{print $2}')
  jq -n \
    --arg version "$(bundle_version)" \
    --arg bundle_id "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$APP/Contents/Info.plist")" \
    --arg identifier "$(sed -n 's/^Identifier=//p' <<<"$sig")" \
    --arg team "$(sed -n 's/^TeamIdentifier=//p' <<<"$sig")" \
    --arg requirement "$req" \
    --arg uuid "$uuid" \
    --arg quarantine "$(xattr -p com.apple.quarantine "$APP" 2>/dev/null || true)" \
    --arg spctl "$(spctl --assess --type execute -vv "$APP" 2>&1 | tr '\n' ' ')" \
    --arg stapler "$(xcrun stapler validate "$APP" 2>&1 | tail -n 1)" \
    '{version: $version, bundle_id: $bundle_id, identifier: $identifier, team: $team,
      requirement: $requirement, uuid: $uuid, quarantine: $quarantine, spctl: $spctl, stapler: $stapler}'
}

# approved <quarantine>: has the person's Open set the user-approved bit (0x40) of the flags?
# Homebrew copies that bit to the upgraded bundle when the signer is unchanged. Source, Homebrew 7.0.10:
# https://github.com/Homebrew/brew/blob/master/Library/Homebrew/cask/upgrade.rb (quarantine_release_decision)
# and cask/quarantine.rb (USER_APPROVED_FLAG, inherit_user_approval!).
approved() {
  local flags=${1%%;*}
  [[ -n "$flags" ]] && (((16#$flags & 0x40) != 0))
}

# write_cask <candidate dir> <version>: the cask of that release, with its url on the local zip.
write_cask() {
  local dir=$1 version=$2 tap zip
  tap=$(tap_dir)
  zip=$dir/testanyware-app-v$version-aarch64-apple-darwin.zip
  [[ -f "$zip" ]] || fail "$zip does not exist."
  mkdir -p "$tap/Casks"
  sed "s|^  url \".*\"\$|  url \"file://$zip\"|" "$dir/testanyware-app.rb" >"$tap/Casks/testanyware-app.rb"
  grep -q "^  url \"file://$zip\"\$" "$tap/Casks/testanyware-app.rb" ||
    fail "the cask of $version has no url line that this script could set."
}

# wait_app <seconds>: wait for the app of /Applications to answer on its socket.
wait_app() {
  local limit=$1 body=
  for _ in $(seq 1 $((limit * 5))); do
    if body=$(status 2>/dev/null) && [[ -n "$(app_pid || true)" ]]; then
      printf '%s' "$body"
      return 0
    fi
    sleep 0.2
  done
  return 1
}

free_slot() {
  local running
  running=$(tart list --format json | jq -r '[.[] | select(.State == "running")] | length')
  ((running < 2)) || fail "both macOS guest slots are taken. Stop a guest, and run this again."
}

record_processes() {
  local meta pid keeper
  meta=$(meta_of "$GUEST")
  pid=$(jq -r .pid "$meta")
  keeper=$(jq -r '.keeper_pid // empty' "$meta")
  alive "$pid" || fail "$GUEST: the tart process $pid does not run."
  printf '%s %s\n' "$pid" "$(started_at "$pid")" >"$state/guest.main"
  if [[ -n "$keeper" ]]; then
    alive "$keeper" || fail "$GUEST: the keeper $keeper does not run."
    printf '%s %s\n' "$keeper" "$(started_at "$keeper")" >"$state/guest.keeper"
  fi
}

same_processes() {
  local file line pid start
  for file in "$state/guest.main" "$state/guest.keeper"; do
    [[ -f "$file" ]] || continue
    # The line is "<pid> <start time>", and the start time holds spaces.
    line=$(<"$file")
    pid=${line%% *}
    start=${line#* }
    alive "$pid" || fail "$GUEST: process $pid ($(basename "$file")) ended during the upgrade."
    [[ "$(started_at "$pid")" == "$start" ]] || fail "$GUEST: process $pid ($(basename "$file")) restarted."
  done
}

# denial <output>: does the output of a failed command show a Local Network denial?
denial() { grep -qi 'local_network_denied\|local.network' <<<"$1"; }

# ---- 0: nothing else holds the app's place ---------------------------------------------------

if [[ -d "$APP" ]]; then
  brew list --cask "$CASK" >/dev/null 2>&1 ||
    fail "$APP exists, and the cask $CASK did not install it. Remove it, and run this again."
elif body=$(status 2>/dev/null); then
  fail "an app that is not $APP answers on the socket (build $(jq -r .build <<<"$body")). Quit it, and run this again: the answer must be the app of the cask."
fi

if [[ -z "$(tap_dir)" || ! -d "$(tap_dir)" ]]; then
  brew tap-new --no-git "$TAP" >/dev/null
fi

# ---- 1: install N ----------------------------------------------------------------------------

if [[ ! -d "$APP" ]]; then
  free_slot
  write_cask "$old_dir" "$old_version"
  brew install --cask "$CASK"
  [[ "$(bundle_version)" == "$old_version" ]] || fail "the installed bundle is $(bundle_version), not $old_version."
  bundle_facts >"$state/before.json"
  ok "installed $old_version from the tap. $(jq -c '{version, identifier, team, uuid, quarantine}' "$state/before.json")"
fi

# ---- 2: N is installed: first start, guest, baseline -----------------------------------------

installed=$(bundle_version)
if [[ "$installed" == "$old_version" ]]; then
  [[ -f "$state/before.json" ]] || bundle_facts >"$state/before.json"
  if ! status >/dev/null 2>&1; then
    echo "upgrade-identity: PERSON STEP (once): answer the dialog \"TestAnyware is an app downloaded from the Internet\" with Open. Waiting up to $max_first_open s."
    open -n -g "$APP"
    body=$(wait_app "$max_first_open") ||
      person "the app did not answer within $max_first_open s. Answer the first-start dialog with Open, and run this again."
  else
    body=$(status)
  fi
  ok "the app $old_version answers, build $(jq -r .build <<<"$body"), pid $(app_pid)"
  bundle_facts >"$state/before.json"
  approved "$(jq -r .quarantine "$state/before.json")" ||
    info "the quarantine flags of N do not show the approval of the first start: $(jq -r .quarantine "$state/before.json")"

  if [[ ! -f "$(meta_of "$GUEST")" ]]; then
    free_slot
    begin=$SECONDS
    if ! out=$("$old_cli" vm start --platform macos --id "$GUEST" --json 2>"$state/stderr"); then
      out="$out$(<"$state/stderr")"
      echo "$out" >&2
      denial "$out" && person "allow TestAnyware in System Settings, Privacy & Security, Local Network (the alert appears at the first guest connection), and run this again. This check never waits for it."
      fail "vm start failed."
    fi
    [[ "$(jq -r .id <<<"$out")" == "$GUEST" ]] || fail "vm start: the id is not $GUEST: $out"
    ((SECONDS - begin <= max_start)) || fail "vm start took more than $max_start s."
    ok "vm start through $old_version: $GUEST reached a healthy agent in $((SECONDS - begin)) s"
  fi
  out=$("$old_cli" agent snapshot --vm "$GUEST" --json) || fail "agent snapshot through $old_version failed."
  [[ "$(jq -r .ok <<<"$out")" == true ]] || fail "agent snapshot through $old_version: ok is not true: $out"
  ok "agent snapshot through $old_version: $(jq '.windows | length' <<<"$out") windows"
  record_processes
  printf '%s %s\n' "$(app_pid)" "$(started_at "$(app_pid)")" >"$state/app.before"
  ok "baseline recorded: app pid $(app_pid), guest $GUEST running"

  # ---- 3: the upgrade ----------------------------------------------------------------------
  write_cask "$new_dir" "$new_version"
  begin=$SECONDS
  brew upgrade --cask --verbose "$CASK" 2>&1 | tee "$state/brew-upgrade.log"
  ok "brew upgrade --cask finished in $((SECONDS - begin)) s"
  installed=$(bundle_version)
fi

# ---- 4: N+1 is installed: what did the upgrade keep? -----------------------------------------

[[ "$installed" == "$new_version" ]] || fail "the installed bundle is $installed, not $new_version."
[[ -f "$state/before.json" && -f "$state/app.before" ]] || fail "no baseline of $old_version in $state. Run clean, and start again."

if ! body=$(wait_app "$max_reopen"); then
  fail "the app $new_version did not answer within $max_reopen s of the upgrade. A dialog may block it (Gatekeeper, or the Local Network alert), or Homebrew did not open it. Check the screen."
fi
ok "the app $new_version answers, build $(jq -r .build <<<"$body"), pid $(app_pid)"
# A cask upgrade opens again each app that its `uninstall quit:` closed: reopen_apps_after_upgrade in
# the upgrade.rb above runs `open -b <bundle id>`.
if grep -q "Reopening" "$state/brew-upgrade.log" 2>/dev/null; then
  ok "Homebrew reopened the app it quit: $(grep -A1 Reopening "$state/brew-upgrade.log" | tr '\n' ' ')"
else
  info "the log of brew upgrade has no \"Reopening\" line"
fi

bundle_facts >"$state/after.json"
before=$state/before.json
after=$state/after.json
for key in bundle_id identifier team requirement; do
  [[ "$(jq -r ".$key" "$before")" == "$(jq -r ".$key" "$after")" ]] ||
    fail "the upgrade changed $key: $(jq -r ".$key" "$before") -> $(jq -r ".$key" "$after")"
done
ok "the signing identity is unchanged: $(jq -c '{bundle_id, identifier, team}' "$after")"
info "designated requirement: $(jq -r .requirement "$after")"
[[ "$(jq -r .uuid "$before")" != "$(jq -r .uuid "$after")" ]] ||
  fail "the executable UUID is the same in both builds. This is not the case that the check exists for."
ok "the executable UUID changed: $(jq -r .uuid "$before") -> $(jq -r .uuid "$after")"
[[ "$(jq -r .version "$after")" == "$new_version" ]] || fail "the bundle version is not $new_version."
grep -q 'source=Notarized Developer ID' <<<"$(jq -r .spctl "$after")" || fail "spctl: $(jq -r .spctl "$after")"
grep -q 'The validate action worked' <<<"$(jq -r .stapler "$after")" || fail "stapler: $(jq -r .stapler "$after")"
ok "the new bundle is notarised and stapled"
info "quarantine before: $(jq -r .quarantine "$before")"
info "quarantine after:  $(jq -r .quarantine "$after")"
if [[ -n "$(jq -r .quarantine "$after")" ]]; then
  approved "$(jq -r .quarantine "$after")" ||
    fail "the new bundle is quarantined and not approved: macOS asks the first-start question again."
  ok "the new bundle keeps the approval of the first start"
else
  info "the new bundle has no quarantine attribute"
fi

[[ "$(app_pid)" != "$(cut -d' ' -f1 "$state/app.before")" ]] || fail "the app pid is the one of $old_version. The app did not restart."
ok "the app restarted: a new process"

[[ "$(tart_state "$GUEST")" == running ]] || fail "after the upgrade, tart says $GUEST is $(tart_state "$GUEST")."
same_processes
ok "the guest ran through the upgrade: its tart process and keeper keep their pids and start times"

adopted=
for _ in $(seq 1 $((max_adopt * 2))); do
  if body=$(status 2>/dev/null) && [[ "$(jq -r '.adoption.finished' <<<"$body")" == true ]]; then
    adopted=$(jq -c --arg id "$GUEST" '.adoption.adopted[] | select(.id == $id)' <<<"$body")
    break
  fi
  sleep 0.5
done
[[ -n "$adopted" ]] || fail "$GUEST is not in adoption.adopted of the new app: $(jq -c .adoption <<<"${body:-null}")"
[[ "$(jq -r .legacy <<<"$adopted")" == false ]] || fail "the new app adopted $GUEST as legacy: $adopted"
[[ "$(jq -r .agent <<<"$adopted")" == answering ]] || fail "the agent of $GUEST does not answer the probe: $adopted"
ok "the new app adopted $GUEST: $adopted"

# The first command to the guest. The grant is evaluated again for a new executable UUID.
begin=$(date +%s)
if out=$("$new_cli" agent snapshot --vm "$GUEST" --json 2>"$state/stderr"); then
  [[ "$(jq -r .ok <<<"$out")" == true ]] || fail "agent snapshot through $new_version: ok is not true: $out"
  ok "the first command to the guest succeeded in $(($(date +%s) - begin)) s: $(jq '.windows | length' <<<"$out") windows"
  outcome=succeeded
else
  out="$out$(<"$state/stderr")"
  echo "$out" >&2
  denial "$out" || fail "the first command failed, and the failure is not a Local Network denial."
  grep -q 'TestAnyware' <<<"$out" || fail "the denial does not name TestAnyware in its remedy."
  info "the first command failed with the denial, and its remedy names TestAnyware"
  person "allow TestAnyware again in System Settings, Privacy & Security, Local Network, and run this again. This check never waits for it."
fi

echo
echo "upgrade-identity: observed by a person, to record in the log of the leaf:"
echo "  - Did a \"downloaded from the Internet\" dialog appear after the upgrade? (expected: no)"
echo "  - Did a Local Network alert appear after the upgrade, and was it allowed? (this run: first command $outcome)"
echo "  - How many TestAnyware rows does System Settings, Privacy & Security, Local Network show? (expected: one)"
echo "upgrade-identity: PASS"
