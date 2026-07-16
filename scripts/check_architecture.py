#!/usr/bin/env python3
"""Reject Rust module directories that only forward one child module."""

from __future__ import annotations

import re
import sys
from pathlib import Path


MOD_DECLARATION = re.compile(
    r"^(?:(?:pub(?:\([^)]*\))?)\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)$",
    re.DOTALL,
)
USE_DECLARATION = re.compile(
    r"^(?:(?:pub(?:\([^)]*\))?)\s+)?use\s+([A-Za-z_][A-Za-z0-9_]*)::.+$",
    re.DOTALL,
)


def forwarding_child(module_file: Path) -> str | None:
    """Return the forwarded child name when ``mod.rs`` is a pure wrapper."""
    source = re.sub(r"//[^\n]*", "", module_file.read_text(encoding="utf-8"))
    statements = [statement.strip() for statement in source.split(";") if statement.strip()]
    if len(statements) < 2:
        return None

    declaration = MOD_DECLARATION.fullmatch(statements[0])
    if declaration is None:
        return None
    child = declaration.group(1)

    if not all(
        (use_match := USE_DECLARATION.fullmatch(statement)) is not None
        and use_match.group(1) == child
        for statement in statements[1:]
    ):
        return None

    children = [path.stem for path in module_file.parent.glob("*.rs") if path.name != "mod.rs"]
    return child if children == [child] else None


def main() -> int:
    root = Path(sys.argv[1] if len(sys.argv) > 1 else ".")
    violations: list[tuple[Path, str]] = []
    for module_file in sorted(root.glob("rust/*/src/**/mod.rs")):
        if child := forwarding_child(module_file):
            violations.append((module_file, child))

    for module_file, child in violations:
        print(
            f"{module_file}: single-child forwarding directory; "
            f"move {child}.rs to {module_file.parent}.rs"
        )
    return 1 if violations else 0


if __name__ == "__main__":
    raise SystemExit(main())
