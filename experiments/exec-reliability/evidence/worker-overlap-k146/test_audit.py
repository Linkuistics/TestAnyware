"""Corrupt retained records in memory to check the independent evidence reader."""

import audit
import pytest


@pytest.mark.parametrize("fault", ["subjects", "artifact", "checker"])
def test_condition_reader_rejects_corrupt_records(monkeypatch, fault):
    original = audit.load

    def load(path):
        record = original(path)
        if path.name == "verdict.json":
            if fault == "subjects":
                record["subjects_after"] = {}
            elif fault == "artifact":
                record["artifact_sha256"][next(iter(record["artifact_sha256"]))] = "invalid"
            else:
                record["checks"][0]["reaped"] = False
        return record

    monkeypatch.setattr(audit, "load", load)
    with pytest.raises(
        ValueError,
        match={"subjects": "subject drift", "artifact": "artifact drift", "checker": "checker:"}[
            fault
        ],
    ):
        audit.condition("candidate-full-1", True)


@pytest.mark.parametrize("fault", ["absence", "identity", "cutoff", "signals"])
def test_overlap_reader_rejects_corrupt_lifecycle(monkeypatch, fault):
    original = audit.load

    def load(path):
        record = original(path)
        if path.name == "production-overlap-audit.json":
            if fault == "absence":
                record["helpers"][0]["state"] = "live"
            elif fault == "identity":
                record["worker"]["pid"] += 1
        lifecycle = (
            record.get("lifecycle")
            if path.name == "production-overlap-audit.json"
            else record.get("inspection_worker")
        )
        if lifecycle is not None:
            if fault == "cutoff":
                lifecycle["owner"]["shutdown_deadline_monotonic"] += 100
            elif fault == "signals":
                lifecycle["owner"]["signals"][1]["signal"] = "SIGTERM"
        return record

    monkeypatch.setattr(audit, "load", load)
    with pytest.raises(
        ValueError,
        match={
            "absence": "independent absence unresolved",
            "identity": "worker identity",
            "cutoff": "request cutoff renewed",
            "signals": "signals:",
        }[fault],
    ):
        audit.overlap(audit.HERE / "candidate-full-1")


def test_overlap_reader_rejects_missing_receipts(tmp_path):
    (tmp_path / "runs").mkdir()
    with pytest.raises(ValueError, match="missing overlap receipts"):
        audit.overlap(tmp_path)


def test_overlap_reader_accepts_complete_real_receipts():
    assert len(audit.overlap(audit.HERE / "candidate-full-1")) == 8
