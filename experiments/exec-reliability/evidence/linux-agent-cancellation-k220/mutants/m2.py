from pathlib import Path
p = Path("testanyware_agent/server.py"); s = p.read_text()
old = "            accessible = _probe_of(self.server).current(HEALTH_PROBE_WAIT_S)"
assert s.count(old) == 1
p.write_text(s.replace(old, "            with accessibility.AT_SPI_LOCK:\n                accessible = accessibility.is_accessible()"))
