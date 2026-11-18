from pathlib import Path
p = Path("testanyware_agent/system_endpoints.py"); s = p.read_text()
old = "    registry = exec_registry.REGISTRY\n    admission = registry.admit(exec_id)\n"
new = old + "    import threading as _t\n    if admission.signal is not None:\n        _t.Timer(0.2, admission.signal.request, args=(0.0,)).start()\n"
assert s.count(old) == 1
p.write_text(s.replace(old, new))
