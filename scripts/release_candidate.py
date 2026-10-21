#!/usr/bin/env python3
"""The identity of a release candidate (leaf app-release-bundle-k22).

A candidate is a build of one local annotated tag. It lives in a directory named by the tag and
carries a manifest of the tag, the commit and the sha256 of each artifact. This module is the one
place that states those rules. release-build.sh writes a candidate with it, and release-publish.sh
and the later checks verify one with it.

  release_candidate.py gate-build --tag vX.Y.Z [--repo DIR]
      Refuse unless the tag is local and annotated, HEAD is the tag's commit, and the tree is clean.
      Print the commit.
  release_candidate.py write --dir DIR --tag vX.Y.Z --commit SHA
      Write DIR/manifest.json for every file in DIR. Refuse a Windows host archive.
  release_candidate.py verify --dir DIR --tag vX.Y.Z [--repo DIR]
      Refuse unless the tag still names the manifest's commit and each artifact has its recorded
      sha256. Refuse a Windows host archive. Print the candidate.
  release_candidate.py render --template FILE --out FILE KEY=VALUE...
      Replace each @KEY@ in the template. Refuse an @KEY@ that no argument names.

Nothing here infers a tag. Two candidate tags can exist at the same time.

No release ships a Windows host binary (ADR-0009). The Windows in-VM agent travels inside the
macOS and Linux archives, so no artifact of a candidate is named for a Windows target.
"""

import argparse
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path

MANIFEST = "manifest.json"
SCHEMA = 1
TAG_PATTERN = re.compile(r"^v(\d+\.\d+\.\d+)$")
PLACEHOLDER = re.compile(r"@([A-Z0-9_]+)@")
# A Windows host archive: testanyware-v<ver>-<arch>-pc-windows-<abi>.<ext>.
WINDOWS_HOST_ARCHIVE = re.compile(r"^testanyware-v.*-pc-windows-")
DEFAULT_REPO = Path(__file__).resolve().parent.parent


class Refusal(Exception):
    """A rule of the candidate does not hold. The message names the rule and the remedy."""


def git(repo: Path, *args: str) -> str:
    done = subprocess.run(
        ["git", "-C", str(repo), *args], capture_output=True, text=True, check=False
    )
    if done.returncode != 0:
        raise Refusal(f"git {' '.join(args)} failed: {done.stderr.strip()}")
    return done.stdout.strip()


def version_of(tag: str) -> str:
    match = TAG_PATTERN.match(tag)
    if match is None:
        raise Refusal(f"the tag \"{tag}\" is not vMAJOR.MINOR.PATCH.")
    return match.group(1)


def tag_commit(repo: Path, tag: str) -> str:
    """The commit that a local annotated tag names."""
    version_of(tag)
    ref = f"refs/tags/{tag}"
    probe = subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "-q", "--verify", ref],
        capture_output=True,
        text=True,
        check=False,
    )
    if probe.returncode != 0:
        raise Refusal(
            f"the tag {tag} does not exist in this repository. "
            f"Create it with: git tag -a {tag} -m \"Release {tag}\" <commit>"
        )
    if git(repo, "cat-file", "-t", ref) != "tag":
        raise Refusal(
            f"the tag {tag} is lightweight. A candidate needs an annotated tag. "
            f"Replace it with: git tag -fa {tag} -m \"Release {tag}\" <commit>"
        )
    return git(repo, "rev-parse", f"{ref}^{{commit}}")


def gate_build(repo: Path, tag: str) -> str:
    """The rule of a build: a clean tree at the commit of the tag. Returns the commit."""
    commit = tag_commit(repo, tag)
    head = git(repo, "rev-parse", "HEAD")
    if head != commit:
        raise Refusal(
            f"HEAD is {head[:12]}, but the tag {tag} names {commit[:12]}. "
            f"Check out the tagged commit. In a jj checkout, run: jj new {commit[:12]}"
        )
    if git(repo, "status", "--porcelain"):
        raise Refusal("the working tree is dirty. Commit or discard the changes before a release.")
    return commit


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def artifact_names(directory: Path) -> list[str]:
    return sorted(
        entry.name for entry in directory.iterdir() if entry.is_file() and entry.name != MANIFEST
    )


def refuse_windows_host_archive(directory: Path, names: list[str]) -> None:
    for name in names:
        if WINDOWS_HOST_ARCHIVE.match(name):
            raise Refusal(
                f"{directory} holds {name}, a Windows host archive. No release ships a Windows "
                "host binary (ADR-0009). Remove it and build the candidate again."
            )


def write_manifest(directory: Path, tag: str, commit: str) -> dict:
    version = version_of(tag)
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise Refusal(f"\"{commit}\" is not a full commit id.")
    names = artifact_names(directory)
    if not names:
        raise Refusal(f"{directory} holds no artifact.")
    refuse_windows_host_archive(directory, names)
    manifest = {
        "schema": SCHEMA,
        "tag": tag,
        "version": version,
        "commit": commit,
        "artifacts": {name: sha256_file(directory / name) for name in names},
    }
    (directory / MANIFEST).write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def verify_candidate(repo: Path, directory: Path, tag: str) -> dict:
    version_of(tag)
    if directory.name != tag:
        raise Refusal(f"the candidate directory {directory} is not named by the tag {tag}.")
    path = directory / MANIFEST
    if not path.is_file():
        raise Refusal(f"{path} does not exist. Build the candidate with: task release:candidate TAG={tag}")
    manifest = json.loads(path.read_text())
    if manifest.get("schema") != SCHEMA:
        raise Refusal(f"{path} has schema {manifest.get('schema')}, not {SCHEMA}.")
    if manifest.get("tag") != tag:
        raise Refusal(f"the manifest is of the tag {manifest.get('tag')}, not {tag}.")
    commit = tag_commit(repo, tag)
    if manifest.get("commit") != commit:
        raise Refusal(
            f"the tag {tag} names {commit[:12]}, but the candidate was built from "
            f"{str(manifest.get('commit'))[:12]}. The candidate is void."
        )
    recorded = manifest["artifacts"]
    present = artifact_names(directory)
    refuse_windows_host_archive(directory, present)
    for name in sorted(set(recorded) - set(present)):
        raise Refusal(f"the artifact {name} is missing from {directory}.")
    for name in sorted(set(present) - set(recorded)):
        raise Refusal(f"{directory} holds {name}, which the manifest does not record.")
    for name, expected in sorted(recorded.items()):
        actual = sha256_file(directory / name)
        if actual != expected:
            raise Refusal(f"the artifact {name} has sha256 {actual}, but the manifest records {expected}.")
    return manifest


def render(template: Path, out: Path, values: dict[str, str]) -> None:
    text = template.read_text()
    unknown = sorted(set(PLACEHOLDER.findall(text)) - set(values))
    if unknown:
        raise Refusal(f"{template} has placeholders that no argument names: {', '.join(unknown)}.")
    out.write_text(PLACEHOLDER.sub(lambda match: values.get(match.group(1), match.group(0)), text))


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawTextHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)

    gate = commands.add_parser("gate-build")
    gate.add_argument("--tag", required=True)
    gate.add_argument("--repo", type=Path, default=DEFAULT_REPO)

    write = commands.add_parser("write")
    write.add_argument("--dir", type=Path, required=True)
    write.add_argument("--tag", required=True)
    write.add_argument("--commit", required=True)

    verify = commands.add_parser("verify")
    verify.add_argument("--dir", type=Path, required=True)
    verify.add_argument("--tag", required=True)
    verify.add_argument("--repo", type=Path, default=DEFAULT_REPO)

    rendering = commands.add_parser("render")
    rendering.add_argument("--template", type=Path, required=True)
    rendering.add_argument("--out", type=Path, required=True)
    rendering.add_argument("values", nargs="*", metavar="KEY=VALUE")

    args = parser.parse_args(argv)
    try:
        if args.command == "gate-build":
            print(gate_build(args.repo, args.tag))
        elif args.command == "write":
            manifest = write_manifest(args.dir, args.tag, args.commit)
            print(f"release-candidate: wrote {args.dir / MANIFEST} ({len(manifest['artifacts'])} artifacts)")
        elif args.command == "verify":
            manifest = verify_candidate(args.repo, args.dir, args.tag)
            print(f"release-candidate: {manifest['tag']} at {manifest['commit']}")
            for name, digest in manifest["artifacts"].items():
                print(f"  {digest}  {name}")
        else:
            pairs = [value.split("=", 1) for value in args.values]
            if any(len(pair) != 2 for pair in pairs):
                raise Refusal("each value is KEY=VALUE.")
            render(args.template, args.out, dict(pairs))
    except Refusal as refusal:
        print(f"release-candidate: {refusal}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
