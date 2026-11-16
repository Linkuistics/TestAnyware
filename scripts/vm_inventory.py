#!/usr/bin/env python3
"""Exact-name absence in the public VM and tart inventories (leaf diagnostic-routes-k19).

  vm_inventory.py absent NAME [--cli PATH] [--tart PATH] [--timeout S]
      Print `absent NAME` and exit 0 only when both inventories parse and neither lists NAME.
      Exit 1 when an inventory lists NAME. Exit 2 when an inventory cannot be read.
  vm_inventory.py dispose ID [--cli PATH] [--tart PATH] [--stop-timeout S] [--timeout S]
      Run `vm stop ID --json`. Accept exit 0 with a success envelope that has `id` ID,
      `stopped: true` and no `dry_run`, or exit 3 with an error envelope whose code is
      VM_NOT_FOUND. Then require absence from both inventories. Print
      `disposed ID stopped` or `disposed ID already-absent` and exit 0. Any other outcome exits
      nonzero and prints a remedy on stderr.

The public inventory is `testanyware vm list --json --all` (schema vm-list: each item has a
string `name`, the golden name or the VM id). The hypervisor inventory is `tart list --format
json` (each item has a string `Name`). Both affected guest OSes start on tart. QEMU has no public
inventory, so a QEMU identity is out of scope here.

A malformed listing, a nonzero exit, a timeout or an executable that cannot run is an inspection
failure. It never counts as absence. Each listing runs with no shell and a finite timeout.
"""

import argparse
import json
import subprocess
import sys
from dataclasses import dataclass
from typing import cast

ABSENT = 0
PRESENT = 1
UNREADABLE = 2
DISPOSAL_FAILED = 3


class InspectionError(Exception):
    """An inventory that could not be read. Absence is unknown."""


@dataclass(frozen=True)
class Ran:
    exit: int
    stdout: bytes
    stderr: bytes


def run(argv: list[str], timeout: float) -> Ran:
    try:
        done = subprocess.run(argv, capture_output=True, timeout=timeout, stdin=subprocess.DEVNULL)
    except subprocess.TimeoutExpired as error:
        raise InspectionError(f"{argv[0]} did not finish within {timeout:g} s") from error
    except OSError as error:
        raise InspectionError(f"{argv[0]} could not run: {error}") from error
    return Ran(done.returncode, done.stdout, done.stderr)


def _object(value: object, what: str) -> dict[str, object]:
    if not isinstance(value, dict):
        raise InspectionError(f"{what} is not a JSON object")
    return {str(key): item for key, item in cast(dict[object, object], value).items()}


def _list(value: object, what: str) -> list[object]:
    if not isinstance(value, list):
        raise InspectionError(what)
    return cast(list[object], value)


def _decode(data: bytes, what: str) -> object:
    try:
        return json.loads(data)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise InspectionError(f"{what} is not JSON: {error}") from error


def vm_names(cli: str, timeout: float) -> set[str]:
    """The names in `vm list --json --all`, checked against schema vm-list."""
    ran = run([cli, "vm", "list", "--json", "--all"], timeout)
    if ran.exit != 0:
        raise InspectionError(f"vm list exited {ran.exit}: {ran.stderr[-400:]!r}")
    envelope = _object(_decode(ran.stdout, "vm list output"), "vm list output")
    if envelope.get("ok") is not True or envelope.get("schema_version") != "1.0":
        raise InspectionError("vm list output is not a successful 1.0 envelope")
    items = _list(envelope.get("items"), "vm list output has no items array")
    count = len(items)
    if envelope.get("truncated") is not False:
        raise InspectionError("vm list output is truncated or does not say")
    if envelope.get("returned") != count or envelope.get("total") != count:
        raise InspectionError("vm list counts do not match its items")
    names: set[str] = set()
    for raw in items:
        item = _object(raw, "a vm list item")
        name = item.get("name")
        if not isinstance(name, str) or not name:
            raise InspectionError("a vm list item has no string name")
        if item.get("kind") not in {"golden", "running"}:
            raise InspectionError(f"vm list item {name!r} has an unknown kind")
        if item.get("backend") not in {"qemu", "tart"}:
            raise InspectionError(f"vm list item {name!r} has an unknown backend")
        names.add(name)
    return names


def tart_names(tart: str, timeout: float) -> set[str]:
    """The names in `tart list --format json`."""
    ran = run([tart, "list", "--format", "json"], timeout)
    if ran.exit != 0:
        raise InspectionError(f"tart list exited {ran.exit}: {ran.stderr[-400:]!r}")
    items = _list(_decode(ran.stdout, "tart list output"), "tart list output is not a JSON array")
    names: set[str] = set()
    for raw in items:
        name = _object(raw, "a tart list item").get("Name")
        if not isinstance(name, str) or not name:
            raise InspectionError("a tart list item has no string Name")
        names.add(name)
    return names


def listed_in(name: str, cli: str, tart: str, timeout: float) -> list[str]:
    """The inventories that list `name`. Both are read before either answer counts."""
    public = vm_names(cli, timeout)
    hypervisor = tart_names(tart, timeout)
    return [
        label for label, names in (("vm list", public), ("tart list", hypervisor)) if name in names
    ]


def absent(args: argparse.Namespace) -> int:
    try:
        found = listed_in(args.name, args.cli, args.tart, args.timeout)
    except InspectionError as error:
        print(f"inventory unreadable, absence unknown: {error}", file=sys.stderr)
        return UNREADABLE
    if found:
        print(f"present {args.name} in {', '.join(found)}", file=sys.stderr)
        return PRESENT
    print(f"absent {args.name}")
    return ABSENT


def _stop_outcome(ran: Ran, identity: str) -> str | None:
    """`stopped`, `already-absent`, or None for any other result of `vm stop IDENTITY --json`.

    A stop counts only when the reply names IDENTITY and says `stopped: true`. A dry run, a
    reply for another VM or a success envelope without that field is no proof of a stop.
    """
    try:
        envelope = _object(_decode(ran.stdout, "vm stop output"), "vm stop output")
    except InspectionError:
        return None
    if envelope.get("schema_version") != "1.0":
        return None
    if (
        ran.exit == 0
        and envelope.get("ok") is True
        and envelope.get("stopped") is True
        and envelope.get("id") == identity
        and "dry_run" not in envelope
    ):
        return "stopped"
    if ran.exit == 3 and envelope.get("ok") is False and envelope.get("code") == "VM_NOT_FOUND":
        return "already-absent"
    return None


def _remedy(identity: str, cli: str, tart: str) -> str:
    return (
        f"remedy: read `{cli} vm list --json --all` and `{tart} list --format json`. "
        f"If either lists {identity}, run `{cli} vm stop {identity} --json`. "
        f"If tart still lists {identity} after that, a person runs `{tart} stop {identity}` "
        f"and then `{tart} delete {identity}`. Never pass a golden name."
    )


def dispose(args: argparse.Namespace) -> int:
    identity: str = args.name
    try:
        ran = run([args.cli, "vm", "stop", identity, "--json"], args.stop_timeout)
    except InspectionError as error:
        print(f"vm stop gave no result: {error}", file=sys.stderr)
        print(_remedy(identity, args.cli, args.tart), file=sys.stderr)
        return DISPOSAL_FAILED
    outcome = _stop_outcome(ran, identity)
    if outcome is None:
        print(
            f"vm stop exited {ran.exit} without a proved stop or VM_NOT_FOUND: "
            f"{ran.stdout[-400:]!r} {ran.stderr[-400:]!r}",
            file=sys.stderr,
        )
        print(_remedy(identity, args.cli, args.tart), file=sys.stderr)
        return DISPOSAL_FAILED
    try:
        found = listed_in(identity, args.cli, args.tart, args.timeout)
    except InspectionError as error:
        print(f"vm stop said {outcome}, but absence is unknown: {error}", file=sys.stderr)
        print(_remedy(identity, args.cli, args.tart), file=sys.stderr)
        return DISPOSAL_FAILED
    if found:
        print(
            f"vm stop said {outcome}, but {', '.join(found)} still lists {identity}",
            file=sys.stderr,
        )
        print(_remedy(identity, args.cli, args.tart), file=sys.stderr)
        return DISPOSAL_FAILED
    print(f"disposed {identity} {outcome}")
    return 0


def parser() -> argparse.ArgumentParser:
    top = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    verbs = top.add_subparsers(dest="verb", required=True)
    for verb, action in (("absent", absent), ("dispose", dispose)):
        sub = verbs.add_parser(verb)
        sub.add_argument("name")
        sub.add_argument("--cli", default="testanyware")
        sub.add_argument("--tart", default="tart")
        sub.add_argument("--timeout", type=float, default=25.0, help="seconds for each listing")
        if verb == "dispose":
            sub.add_argument("--stop-timeout", type=float, default=30.0)
        sub.set_defaults(action=action)
    return top


def main(argv: list[str] | None = None) -> int:
    args = parser().parse_args(argv)
    if not args.name or args.timeout <= 0 or getattr(args, "stop_timeout", 1) <= 0:
        print("a name and positive timeouts are required", file=sys.stderr)
        return UNREADABLE
    return args.action(args)


if __name__ == "__main__":
    sys.exit(main())
