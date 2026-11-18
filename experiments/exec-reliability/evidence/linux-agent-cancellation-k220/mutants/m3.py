from pathlib import Path
p = Path("testanyware_agent/system_endpoints.py"); s = p.read_text()
old = "        chunk = fileobj.read(min(CHUNK_SIZE, remaining))"
assert s.count(old) == 1
p.write_text(s.replace(old, "        chunk = fileobj.read(CHUNK_SIZE)"))
