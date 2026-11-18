from pathlib import Path
p = Path("testanyware_agent/exec_owner.py"); s = p.read_text()
old = "                left = min(max(0.0, end - time.monotonic()), MAX_KERNEL_WAIT_S)"
assert s.count(old) == 1
p.write_text(s.replace(old, "                left = max(0.0, end - time.monotonic())"))
