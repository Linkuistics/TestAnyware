#!/usr/bin/env bash
#
# Publish a recorded release candidate made by release-build.sh. OUTWARD-FACING: it pushes a tag,
# creates a GitHub Release and pushes the Homebrew tap. It has not run for a real release yet.
#
#   release-publish.sh vX.Y.Z
#
# The tag is an explicit input and the candidate is target/candidates/<tag>. The script refuses to
# run unless the local tag still names the commit of the manifest and every artifact has its
# recorded sha256. Then, in this order:
#   1. push the existing local tag
#   2. create the GitHub Release for that tag with the candidate's archives
#   3. push the formula and the cask to the tap, in one commit.
# It never creates a tag and it never rebuilds: `gh release create --verify-tag` aborts when the
# tag is absent on the remote, where gh would otherwise make one from the default branch.
#
# Env: TESTANYWARE_TAP_DIR (default ~/Development/homebrew-taps), TESTANYWARE_RELEASE_REMOTE
# (default origin; tests name a local repository).

set -euo pipefail
IFS=$'\n\t'

readonly REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
readonly CANDIDATES_DIR="$REPO_ROOT/target/candidates"
readonly CANDIDATE_PY="$REPO_ROOT/scripts/release_candidate.py"
readonly TAP_DIR="${TESTANYWARE_TAP_DIR:-$HOME/Development/homebrew-taps}"
readonly REMOTE="${TESTANYWARE_RELEASE_REMOTE:-origin}"
readonly GITHUB_REPO=Linkuistics/TestAnyware

die() {
  echo "release-publish: $*" >&2
  exit 1
}

preflight() {
  command -v gh >/dev/null || die "gh CLI not on PATH"
  gh auth status >/dev/null 2>&1 || die "gh not authenticated; run 'gh auth login'"
  [[ -d "$TAP_DIR/.git" ]] || die "tap clone not found at $TAP_DIR (set TESTANYWARE_TAP_DIR)"
  [[ -z "$(git -C "$TAP_DIR" status --porcelain)" ]] \
    || die "the tap clone at $TAP_DIR has uncommitted changes"
  git -C "$TAP_DIR" fetch --quiet || die "cannot fetch the tap at $TAP_DIR"
  # A tap that is behind rejects the push after the release exists, and leaves a half publication.
  if git -C "$TAP_DIR" rev-parse --verify --quiet '@{u}' >/dev/null; then
    [[ "$(git -C "$TAP_DIR" rev-list --count 'HEAD..@{u}')" == 0 ]] \
      || die "the tap clone at $TAP_DIR is behind its upstream; pull it first"
  fi
}

# Push the existing local tag. A remote that already holds the tag must hold it at the same commit.
push_tag() {
  local tag="$1" commit="$2" remote_commit
  # An annotated tag lists twice: the tag object, then its commit on the line that ends in ^{}.
  # A lightweight tag lists once, as the commit itself.
  remote_commit="$(git -C "$REPO_ROOT" ls-remote "$REMOTE" "refs/tags/$tag" "refs/tags/${tag}^{}" \
    | awk '$2 ~ /\^\{\}$/ { peeled = $1 } { plain = $1 } END { print (peeled != "") ? peeled : plain }')"
  if [[ -n "$remote_commit" ]]; then
    [[ "$remote_commit" == "$commit" ]] \
      || die "$REMOTE holds $tag at ${remote_commit:0:12}, but the candidate is ${commit:0:12}"
    echo "release-publish: $REMOTE already holds $tag at ${commit:0:12}"
    return
  fi
  echo "release-publish: pushing $tag to $REMOTE"
  git -C "$REPO_ROOT" push "$REMOTE" "refs/tags/$tag"
}

create_github_release() {
  local tag="$1" dir="$2"
  echo "release-publish: creating GitHub Release $tag"
  # The archives are release assets: .tar.xz feed the formula and the app zip feeds the cask.
  # The rendered .rb files and the manifest go to the tap and stay local.
  local assets=() file
  for file in "$dir"/*.tar.xz "$dir"/*.zip; do
    [[ -f "$file" ]] && assets+=("$file")
  done
  ((${#assets[@]} > 0)) || die "no archives in $dir"
  gh release create "$tag" \
    --repo "$GITHUB_REPO" \
    --verify-tag \
    --title "Release $tag" \
    --notes "Release $tag" \
    "${assets[@]}"
}

push_tap() {
  local tag="$1" dir="$2"
  echo "release-publish: pushing the formula and the cask to $TAP_DIR"
  mkdir -p "$TAP_DIR/Formula" "$TAP_DIR/Casks"
  cp "$dir/testanyware.rb" "$TAP_DIR/Formula/testanyware.rb"
  cp "$dir/testanyware-app.rb" "$TAP_DIR/Casks/testanyware-app.rb"
  git -C "$TAP_DIR" add Formula/testanyware.rb Casks/testanyware-app.rb
  git -C "$TAP_DIR" commit -m "testanyware $tag"
  git -C "$TAP_DIR" push
}

main() {
  [[ $# -eq 1 ]] || die "usage: release-publish.sh vX.Y.Z  (the tag of a recorded candidate)"
  local tag="$1" dir="$CANDIDATES_DIR/$1"
  [[ -d "$dir" ]] || die "no candidate at $dir; build it with scripts/release-build.sh $tag"
  [[ -f "$dir/testanyware.rb" && -f "$dir/testanyware-app.rb" ]] \
    || die "the candidate in $dir lacks the formula or the cask"

  # The candidate must still be the one that was built: same commit under the tag, same artifacts.
  "$CANDIDATE_PY" verify --dir "$dir" --tag "$tag" --repo "$REPO_ROOT" || die "the candidate does not verify"
  local commit
  commit="$(git -C "$REPO_ROOT" rev-parse "refs/tags/${tag}^{commit}")"

  preflight
  push_tag "$tag" "$commit"
  create_github_release "$tag" "$dir"
  push_tap "$tag" "$dir"

  echo
  echo "release-publish: done. Verify with:"
  echo "  brew update && brew install linkuistics/taps/testanyware"
  echo "  brew install --cask linkuistics/taps/testanyware-app"
}

main "$@"
