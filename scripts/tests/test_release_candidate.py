"""Tests of scripts/release_candidate.py against a scratch git repository.

Run with: python3 -I scripts/tests/test_release_candidate.py
"""

import io
import subprocess
import sys
import tarfile
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

import release_candidate as rc  # noqa: E402


def git(repo: Path, *args: str) -> str:
    done = subprocess.run(
        ["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t@example.com", *args],
        capture_output=True,
        text=True,
        check=True,
    )
    return done.stdout.strip()


SKILL_FILES = {
    "skills/using-testanyware/SKILL.md": b"---\nname: using-testanyware\n---\n",
    "bin/run": b"#!/bin/sh\n",
}
HOST_ARCHIVE = "testanyware-v1.2.3-aarch64-apple-darwin.tar.xz"
SKILL_PREFIX = "testanyware-v1.2.3-aarch64-apple-darwin/share/ravel/skills/testanyware/"


def skill_entries() -> dict:
    """The skill of the commit, as a release archive carries it: path -> (mode, data or link)."""
    return {
        "skills/using-testanyware/SKILL.md": (0o644, SKILL_FILES["skills/using-testanyware/SKILL.md"]),
        "bin/run": (0o755, SKILL_FILES["bin/run"]),
        "skills/link.md": ("link", "using-testanyware/SKILL.md"),
    }


def write_host_archive(path: Path, entries: dict) -> None:
    top = path.name.removesuffix(".tar.xz")
    with tarfile.open(path, "w:xz") as archive:
        cli = tarfile.TarInfo(f"{top}/bin/testanyware")
        cli.mode = 0o755
        archive.addfile(cli, io.BytesIO(b""))
        for relative, (mode, data) in entries.items():
            member = tarfile.TarInfo(f"{top}/share/ravel/skills/testanyware/{relative}")
            if mode == "link":
                member.type = tarfile.SYMTYPE
                member.linkname = data
                archive.addfile(member)
            else:
                member.mode = mode
                member.size = len(data)
                archive.addfile(member, io.BytesIO(data))


class Candidate(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        git(self.repo, "init", "-q")
        (self.repo / "a.txt").write_text("one\n")
        # The skill source: a file, an executable file and a link, as Git records them.
        skill = self.repo / "plugins" / "testanyware"
        for path, data in SKILL_FILES.items():
            (skill / path).parent.mkdir(parents=True, exist_ok=True)
            (skill / path).write_bytes(data)
        (skill / "bin" / "run").chmod(0o755)
        (skill / "skills" / "link.md").symlink_to("using-testanyware/SKILL.md")
        git(self.repo, "add", "a.txt", "plugins")
        git(self.repo, "commit", "-q", "-m", "one")
        self.first = git(self.repo, "rev-parse", "HEAD")

    def tearDown(self):
        self._tmp.cleanup()

    def commit_again(self) -> str:
        (self.repo / "a.txt").write_text("two\n")
        git(self.repo, "commit", "-q", "-am", "two")
        return git(self.repo, "rev-parse", "HEAD")

    def candidate(self, tag="v1.2.3") -> Path:
        directory = self.root / "candidates" / tag
        directory.mkdir(parents=True)
        (directory / "one.tar.xz").write_bytes(b"first")
        (directory / "two.zip").write_bytes(b"second")
        rc.write_manifest(directory, tag, self.first)
        return directory

    # --- the gate of a build

    def test_gate_accepts_a_clean_annotated_tag_at_head(self):
        git(self.repo, "tag", "-a", "v1.2.3", "-m", "r")
        self.assertEqual(rc.gate_build(self.repo, "v1.2.3"), self.first)

    def test_gate_refuses_a_malformed_tag(self):
        for tag in ["1.2.3", "v1.2", "v1.2.3-rc1", "v1.2.3.4", ""]:
            with self.assertRaisesRegex(rc.Refusal, "not vMAJOR.MINOR.PATCH"):
                rc.gate_build(self.repo, tag)

    def test_gate_refuses_a_missing_tag(self):
        with self.assertRaisesRegex(rc.Refusal, "does not exist"):
            rc.gate_build(self.repo, "v1.2.3")

    def test_gate_refuses_a_lightweight_tag(self):
        git(self.repo, "tag", "v1.2.3")
        with self.assertRaisesRegex(rc.Refusal, "lightweight"):
            rc.gate_build(self.repo, "v1.2.3")

    def test_gate_refuses_a_head_that_the_tag_does_not_name(self):
        git(self.repo, "tag", "-a", "v1.2.3", "-m", "r")
        self.commit_again()
        with self.assertRaisesRegex(rc.Refusal, "names " + self.first[:12]):
            rc.gate_build(self.repo, "v1.2.3")

    def test_gate_refuses_a_dirty_tree(self):
        git(self.repo, "tag", "-a", "v1.2.3", "-m", "r")
        (self.repo / "a.txt").write_text("edited\n")
        with self.assertRaisesRegex(rc.Refusal, "dirty"):
            rc.gate_build(self.repo, "v1.2.3")

    def test_gate_refuses_an_untracked_file(self):
        git(self.repo, "tag", "-a", "v1.2.3", "-m", "r")
        (self.repo / "new.txt").write_text("x\n")
        with self.assertRaisesRegex(rc.Refusal, "dirty"):
            rc.gate_build(self.repo, "v1.2.3")

    def test_gate_names_the_tag_it_was_given_when_two_tags_exist(self):
        git(self.repo, "tag", "-a", "v1.2.3", "-m", "r")
        second = self.commit_again()
        git(self.repo, "tag", "-a", "v1.2.4", "-m", "r")
        self.assertEqual(rc.gate_build(self.repo, "v1.2.4"), second)
        with self.assertRaises(rc.Refusal):
            rc.gate_build(self.repo, "v1.2.3")

    # --- the manifest

    def test_manifest_records_the_tag_the_commit_and_each_hash(self):
        git(self.repo, "tag", "-a", "v1.2.3", "-m", "r")
        directory = self.candidate()
        manifest = rc.verify_candidate(self.repo, directory, "v1.2.3")
        self.assertEqual(manifest["tag"], "v1.2.3")
        self.assertEqual(manifest["version"], "1.2.3")
        self.assertEqual(manifest["commit"], self.first)
        self.assertEqual(set(manifest["artifacts"]), {"one.tar.xz", "two.zip"})
        self.assertEqual(
            manifest["artifacts"]["one.tar.xz"],
            "a7937b64b8caa58f03721bb6bacf5c78cb235febe0e70b1b84cd99541461a08e",
        )

    def test_verify_refuses_an_artifact_that_differs(self):
        git(self.repo, "tag", "-a", "v1.2.3", "-m", "r")
        directory = self.candidate()
        (directory / "two.zip").write_bytes(b"tampered")
        with self.assertRaisesRegex(rc.Refusal, "two.zip has sha256"):
            rc.verify_candidate(self.repo, directory, "v1.2.3")

    def test_verify_refuses_a_missing_artifact(self):
        git(self.repo, "tag", "-a", "v1.2.3", "-m", "r")
        directory = self.candidate()
        (directory / "two.zip").unlink()
        with self.assertRaisesRegex(rc.Refusal, "two.zip is missing"):
            rc.verify_candidate(self.repo, directory, "v1.2.3")

    def test_verify_refuses_a_file_the_manifest_does_not_record(self):
        git(self.repo, "tag", "-a", "v1.2.3", "-m", "r")
        directory = self.candidate()
        (directory / "extra.zip").write_bytes(b"x")
        with self.assertRaisesRegex(rc.Refusal, "extra.zip, which the manifest does not record"):
            rc.verify_candidate(self.repo, directory, "v1.2.3")

    # --- the skill in each host archive

    def host_candidate(self, entries: dict) -> Path:
        git(self.repo, "tag", "-a", "v1.2.3", "-m", "r")
        directory = self.root / "candidates" / "v1.2.3"
        directory.mkdir(parents=True)
        write_host_archive(directory / HOST_ARCHIVE, entries)
        (directory / "testanyware-app-v1.2.3-aarch64-apple-darwin.zip").write_bytes(b"app")
        rc.write_manifest(directory, "v1.2.3", self.first)
        return directory

    def test_verify_accepts_a_host_archive_whose_skill_is_the_commits(self):
        directory = self.host_candidate(skill_entries())
        manifest = rc.verify_candidate(self.repo, directory, "v1.2.3")
        self.assertIn(HOST_ARCHIVE, manifest["artifacts"])

    def test_verify_refuses_a_host_archive_without_the_skill(self):
        directory = self.host_candidate({})
        with self.assertRaisesRegex(rc.Refusal, HOST_ARCHIVE + " lacks share/ravel/skills/testanyware"):
            rc.verify_candidate(self.repo, directory, "v1.2.3")

    def test_verify_refuses_a_skill_file_whose_content_differs(self):
        entries = skill_entries()
        entries["skills/using-testanyware/SKILL.md"] = (0o644, b"edited\n")
        directory = self.host_candidate(entries)
        with self.assertRaisesRegex(rc.Refusal, "skills/using-testanyware/SKILL.md differs"):
            rc.verify_candidate(self.repo, directory, "v1.2.3")

    def test_verify_refuses_a_skill_file_without_its_executable_bit(self):
        entries = skill_entries()
        entries["bin/run"] = (0o644, SKILL_FILES["bin/run"])
        directory = self.host_candidate(entries)
        with self.assertRaisesRegex(rc.Refusal, "bin/run differs"):
            rc.verify_candidate(self.repo, directory, "v1.2.3")

    def test_verify_refuses_a_link_that_names_another_target(self):
        entries = skill_entries()
        entries["skills/link.md"] = ("link", "elsewhere.md")
        directory = self.host_candidate(entries)
        with self.assertRaisesRegex(rc.Refusal, "skills/link.md differs"):
            rc.verify_candidate(self.repo, directory, "v1.2.3")

    def test_verify_refuses_a_skill_file_that_the_commit_lacks(self):
        entries = skill_entries()
        entries["._SKILL.md"] = (0o644, b"metadata")
        directory = self.host_candidate(entries)
        with self.assertRaisesRegex(rc.Refusal, "._SKILL.md, which plugins/testanyware"):
            rc.verify_candidate(self.repo, directory, "v1.2.3")

    def test_verify_refuses_a_skill_file_that_is_missing(self):
        entries = skill_entries()
        del entries["bin/run"]
        directory = self.host_candidate(entries)
        with self.assertRaisesRegex(rc.Refusal, "lacks bin/run"):
            rc.verify_candidate(self.repo, directory, "v1.2.3")

    def test_write_refuses_a_windows_host_archive(self):
        directory = self.root / "candidates" / "v1.2.3"
        directory.mkdir(parents=True)
        (directory / "testanyware-v1.2.3-aarch64-apple-darwin.tar.xz").write_bytes(b"mac")
        (directory / "testanyware-v1.2.3-aarch64-pc-windows-gnullvm.zip").write_bytes(b"win")
        with self.assertRaisesRegex(rc.Refusal, "Windows host archive"):
            rc.write_manifest(directory, "v1.2.3", self.first)
        self.assertFalse((directory / rc.MANIFEST).exists())

    def test_verify_refuses_a_windows_host_archive_that_appears_later(self):
        git(self.repo, "tag", "-a", "v1.2.3", "-m", "r")
        directory = self.candidate()
        (directory / "testanyware-v1.2.3-x86_64-pc-windows-gnu.zip").write_bytes(b"win")
        with self.assertRaisesRegex(rc.Refusal, "testanyware-v1.2.3-x86_64-pc-windows-gnu.zip, a Windows host archive"):
            rc.verify_candidate(self.repo, directory, "v1.2.3")

    def test_the_windows_agent_is_not_a_windows_host_archive(self):
        # The Windows in-VM agent ships inside the macOS and Linux archives, never as an artifact.
        directory = self.root / "candidates" / "v1.2.3"
        directory.mkdir(parents=True)
        (directory / "testanyware-v1.2.3-aarch64-unknown-linux-gnu.tar.xz").write_bytes(b"linux")
        (directory / "testanyware-app-v1.2.3-aarch64-apple-darwin.zip").write_bytes(b"app")
        manifest = rc.write_manifest(directory, "v1.2.3", self.first)
        self.assertEqual(len(manifest["artifacts"]), 2)

    def test_verify_refuses_a_tag_that_names_another_commit(self):
        git(self.repo, "tag", "-a", "v1.2.3", "-m", "r")
        directory = self.candidate()
        self.commit_again()
        git(self.repo, "tag", "-fa", "v1.2.3", "-m", "moved")
        with self.assertRaisesRegex(rc.Refusal, "candidate is void"):
            rc.verify_candidate(self.repo, directory, "v1.2.3")

    def test_verify_refuses_a_directory_that_the_tag_does_not_name(self):
        git(self.repo, "tag", "-a", "v1.2.3", "-m", "r")
        directory = self.candidate()
        renamed = directory.rename(directory.parent / "v9.9.9")
        git(self.repo, "tag", "-a", "v9.9.9", "-m", "r")
        with self.assertRaisesRegex(rc.Refusal, "is of the tag v1.2.3, not v9.9.9"):
            rc.verify_candidate(self.repo, renamed, "v9.9.9")

    def test_verify_refuses_a_candidate_without_a_manifest(self):
        git(self.repo, "tag", "-a", "v1.2.3", "-m", "r")
        directory = self.root / "candidates" / "v1.2.3"
        directory.mkdir(parents=True)
        with self.assertRaisesRegex(rc.Refusal, "does not exist"):
            rc.verify_candidate(self.repo, directory, "v1.2.3")

    def test_a_second_candidate_leaves_the_first_alone(self):
        git(self.repo, "tag", "-a", "v1.2.3", "-m", "r")
        first = self.candidate("v1.2.3")
        second = self.candidate("v1.2.4")
        git(self.repo, "tag", "-a", "v1.2.4", "-m", "r")
        rc.verify_candidate(self.repo, first, "v1.2.3")
        rc.verify_candidate(self.repo, second, "v1.2.4")

    def test_write_refuses_an_empty_directory(self):
        directory = self.root / "empty"
        directory.mkdir()
        with self.assertRaisesRegex(rc.Refusal, "holds no artifact"):
            rc.write_manifest(directory, "v1.2.3", self.first)

    # --- rendering

    def test_render_replaces_each_placeholder(self):
        template = self.root / "t.rb"
        template.write_text('version "@VERSION@"\nsha256 "@SHA@"\nurl "x-@VERSION@"\n')
        out = self.root / "o.rb"
        rc.render(template, out, {"VERSION": "1.2.3", "SHA": "ab"})
        self.assertEqual(out.read_text(), 'version "1.2.3"\nsha256 "ab"\nurl "x-1.2.3"\n')

    def test_render_refuses_a_placeholder_that_nothing_names(self):
        template = self.root / "t.rb"
        template.write_text('version "@VERSION@"\nsha256 "@SHA@"\n')
        with self.assertRaisesRegex(rc.Refusal, "SHA"):
            rc.render(template, self.root / "o.rb", {"VERSION": "1.2.3"})


if __name__ == "__main__":
    unittest.main()
