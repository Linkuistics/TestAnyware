"""k21 resource audit: verify every retained run in k21-green-01 and check for surviving fixtures."""
import collections, hashlib, json, subprocess, sys
from pathlib import Path
HERE = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(HERE))
import retention
out = Path(sys.argv[1])
runs = HERE / "evidence/k21-green-01/runs"
items, causes = [], collections.Counter()
for d in sorted(p for p in runs.iterdir() if p.is_dir()):
    r = retention.verify(d)
    changed = [(f["path"], f["state"]) for f in r["files"] if f["state"] != "unchanged"]
    streams = [(s["call"], s["stream"], s["state"]) for s in r["streams"] if s["state"] != "unchanged"]
    if r["ok"]: cause = "verified"
    elif r["identity"] != "ok": cause = "identity " + r["identity"].split(":")[0]
    elif changed or streams: cause = "changed or missing"
    else: cause = "unlisted only"
    causes[cause] += 1
    items.append({"run": d.name, "ok": r["ok"], "cause": cause, "identity": r["identity"],
                  "changed": changed, "streams": streams, "unlisted": r["unlisted"]})
ps = subprocess.run(["/bin/ps", "-axo", "pid=,pgid=,command="], capture_output=True, text=True, timeout=10)
fixtures = [l for l in ps.stdout.splitlines() if len(l.split()) > 3 and l.split()[2].endswith("/python") and l.split()[3] == str(HERE / "peer.py")]
report = {"sources": {p: hashlib.sha256((HERE / p).read_bytes()).hexdigest()
                      for p in ("retention.py", "controller.py", "evidence/k21-audit-02/audit.py")},
          "run_dirs": len(items), "causes": causes, "ps_returncode": ps.returncode,
          "ps_stderr": ps.stderr, "surviving_fixture_processes": fixtures, "runs": items}
with out.open("x") as s: json.dump(report, s, indent=2)
print(json.dumps({k: report[k] for k in ("run_dirs", "causes", "ps_returncode", "surviving_fixture_processes")}))
# Correction of k21-audit-01: its filter matched the audit's own shell command,
# whose text contained "peer.py". This filter accepts only a python argv[0]
# followed by this tree's peer.py.
