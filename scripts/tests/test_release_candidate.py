"""Tests of scripts/release_candidate.py against a scratch git repository.

Run with: python3 -I scripts/tests/test_release_candidate.py
"""

import subprocess
import sys
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


class Candidate(unittest.TestCase):
    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        git(self.repo, "init", "-q")
        (self.repo / "a.txt").write_text("one\n")
        git(self.repo, "add", "a.txt")
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
