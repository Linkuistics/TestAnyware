"""Stand-in for pyatspi in the k5 container.

GET /health calls Registry.getDesktop(0). The stub answers, so readiness can use /health.
"""

DESKTOP_COORDS = 0
STATE_ACTIVE = 0


class Accessible:
    pass


class Registry:
    @staticmethod
    def getDesktop(index: int) -> Accessible:  # noqa: N802
        del index
        return Accessible()


def clearCache() -> None:  # noqa: N802
    pass
