from pathlib import Path
p = Path("testanyware_agent/server.py"); s = p.read_text()
old = "    running = exec_registry.REGISTRY.close()\n"
assert s.count(old) == 1
p.write_text(s.replace(old, "    running = []\n"))
