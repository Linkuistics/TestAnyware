"""Import the agent's HTTP handler on a host without AT-SPI.

`server.py` imports the accessibility module, which needs PyGObject and
pyatspi. The exec path never calls them. When they are missing, this module
installs empty stand-ins for the import only, as the k5 container did
(experiments/exec-reliability/linux_k5_stub). An installed real module wins.
"""

from __future__ import annotations

import importlib
import importlib.util
import sys
import types


def _stand_in(name: str, **attributes: object) -> None:
    module = types.ModuleType(name)
    module.__dict__.update(attributes)
    sys.modules[name] = module


def _missing(name: str) -> bool:
    # A stand-in from an earlier call has no spec, and `find_spec` raises on it.
    return name not in sys.modules and importlib.util.find_spec(name) is None


def agent_request_handler() -> type:
    if _missing("gi"):
        _stand_in("gi", require_version=lambda namespace, version: None)
        _stand_in("gi.repository", Atspi=type("Atspi", (), {}))
    if _missing("pyatspi"):
        _stand_in("pyatspi", Accessible=type("Accessible", (), {}))
    return importlib.import_module("testanyware_agent.server").AgentRequestHandler
