from pathlib import Path
p = Path("testanyware_agent/server.py"); s = p.read_text()
old = "                with accessibility.AT_SPI_LOCK:\n                    status, response_body = serialised[route]()"
assert s.count(old) == 1
p.write_text(s.replace(old, "                status, response_body = serialised[route]()"))
