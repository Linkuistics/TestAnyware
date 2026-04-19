import sys
from testanyware_agent.server import run_server

def main():
    port = 8648
    if len(sys.argv) > 1:
        for i, arg in enumerate(sys.argv[1:]):
            if arg == "--port" and i + 1 < len(sys.argv) - 1:
                port = int(sys.argv[i + 2])
                break

    print(f"testanyware-agent listening on http://0.0.0.0:{port}")
    run_server(port)

if __name__ == "__main__":
    main()
