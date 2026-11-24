"""Stand-in for PyGObject in the k5 container.

The Linux agent imports gi only for accessibility. The exec seams never call it.
"""


def require_version(namespace: str, version: str) -> None:
    del namespace, version
