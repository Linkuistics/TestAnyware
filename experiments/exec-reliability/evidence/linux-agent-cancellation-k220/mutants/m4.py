from pathlib import Path
p = Path("testanyware_agent/exec_owner.py"); s = p.read_text()
old = "            status = waiter.peek()\n            outcome_at = time.monotonic()\n            cancel_at = None if cancel is None else cancel.requested_at\n"
new = "            outcome_at = time.monotonic()\n            cancel_at = None if cancel is None else cancel.requested_at\n            status = None if cancel_at is not None else waiter.peek()\n"
assert s.count(old) == 1
p.write_text(s.replace(old, new))
