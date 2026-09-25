#!/usr/bin/env python3
"""Structural rules the Makefile's grep checks cannot express.

1. No Rust module directory may only forward one child module.
2. Layering (docs/architecture_layering_audit_2026-09-25.md): the refinement
   backends are reached only through the orchestration and adapter modules.
   The foundation crates and the request layer never depend on a backend
   crate, and inside the CLI only the modules in `CLI_BACKEND_ADAPTERS` may
   name one -- so input and output code cannot start depending on which
   algorithm ran without this gate saying so.
"""

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


BACKEND_CRATES = (
    "earthmesh_refine_method_c",
    "earthmesh_refine_redgreen",
    "earthmesh_refine_certified",
)
BACKEND_REFERENCE = re.compile(r"\b(" + "|".join(BACKEND_CRATES) + r")\b")

# Crates below the backends, and the request layer above the inputs.
NEUTRAL_CRATES = (
    "earthmesh_core",
    "earthmesh_geometry",
    "earthmesh_boundary",
    "earthmesh_mesh",
    "earthmesh_hfield",
    "earthmesh_quality",
    "earthmesh_project",
    "earthmesh_refine",
    "earthmesh_refine_planner",
)

# The only CLI sources that may name a backend crate: orchestration, the
# per-backend adapters, and the run record that archives backend diagnostics.
CLI_BACKEND_ADAPTERS = frozenset(
    {
        "refine_pipeline/global_source.rs",
        "refine_pipeline/cmrc_local_updates.rs",
        "refine_pipeline/lepp_targets.rs",
        "redgreen_bridge.rs",
        "method_c_adaptive_nest.rs",
        "certified_options.rs",
        "mkgrd_run_types/refine.rs",
    }
)


def code_without_comments(path: Path) -> str:
    return re.sub(r"//[^\n]*", "", path.read_text(encoding="utf-8"))


def backend_dependencies(manifest: Path) -> list[str]:
    """Backend crates a manifest depends on, in any dependency table."""
    found = []
    for line in manifest.read_text(encoding="utf-8").splitlines():
        match = re.match(r"\s*([A-Za-z0-9_-]+)\s*=", line)
        if match and match.group(1) in BACKEND_CRATES:
            found.append(match.group(1))
    return found


def layering_violations(root: Path) -> list[str]:
    violations = []
    for crate in NEUTRAL_CRATES:
        manifest = root / "rust" / crate / "Cargo.toml"
        if manifest.is_file():
            for dependency in backend_dependencies(manifest):
                violations.append(
                    f"{manifest}: depends on {dependency}; {crate} sits below the "
                    "refinement backends and must not depend on one"
                )
        for source in sorted((root / "rust" / crate / "src").glob("**/*.rs")):
            if match := BACKEND_REFERENCE.search(code_without_comments(source)):
                violations.append(
                    f"{source}: names {match.group(1)}; {crate} must not depend on a "
                    "refinement backend"
                )
    cli_source = root / "rust" / "earthmesh_cli" / "src"
    for source in sorted(cli_source.glob("**/*.rs")):
        relative = source.relative_to(cli_source).as_posix()
        if relative in CLI_BACKEND_ADAPTERS:
            continue
        if match := BACKEND_REFERENCE.search(code_without_comments(source)):
            violations.append(
                f"{source}: names {match.group(1)} outside the backend adapters; reach "
                "the backend through an adapter module (CLI_BACKEND_ADAPTERS in "
                "scripts/check_architecture.py), so input and output code stay "
                "independent of the algorithm"
            )
    return violations


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
    layering = layering_violations(root)
    for violation in layering:
        print(violation)
    return 1 if violations or layering else 0


if __name__ == "__main__":
    raise SystemExit(main())
