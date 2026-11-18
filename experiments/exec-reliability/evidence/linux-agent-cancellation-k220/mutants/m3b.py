from pathlib import Path
p = Path("testanyware_agent/system_endpoints.py"); s = p.read_text()
old = s[s.index("    remaining = length\n    while remaining > 0:"):s.index("    return True\n\n\ndef handle_shutdown")]
new = "    while chunk := fileobj.read(CHUNK_SIZE):\n        write(chunk)\n"
p.write_text(s.replace(old, new))
