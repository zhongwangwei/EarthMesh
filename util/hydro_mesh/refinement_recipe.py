from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Mapping

from util.hydro_mesh.refine_mask_export import DEFAULT_CLASS_REFINE, parse_class_refine, parse_degree_buffers


def _class_refine_args(class_refine: Mapping[str, int]) -> list[str]:
    return [f"{river_class}={degree}" for river_class, degree in sorted(class_refine.items())]


def _degree_buffer_args(buffer_deg_by_refine_degree: Mapping[int, float]) -> list[str]:
    return [f"{degree}={buffer}" for degree, buffer in sorted(buffer_deg_by_refine_degree.items())]


def build_close_refinement_recipe(
    *,
    input_geojson: str,
    output_prefix: str,
    class_refine: Mapping[str, int] | None = None,
    buffer_deg_by_refine_degree: Mapping[int, float] | None = None,
    simplify_tolerance_deg: float = 0.0,
    example_namelist: str | None = None,
) -> dict[str, object]:
    """Build a reproducible EarthMesh close-refinement recipe for CaMa corridors."""

    class_refine = dict(class_refine or DEFAULT_CLASS_REFINE)
    buffer_deg_by_refine_degree = dict(buffer_deg_by_refine_degree or {})
    max_iter_spc = max(class_refine.values()) if class_refine else 0
    close_mask_command = [
        "python3",
        "-m",
        "util.hydro_mesh.refine_mask_export",
        input_geojson,
        output_prefix,
        "--class-refine",
        *_class_refine_args(class_refine),
    ]
    if buffer_deg_by_refine_degree:
        close_mask_command.extend(["--buffer-deg-by-refine-degree", *_degree_buffer_args(buffer_deg_by_refine_degree)])
    if simplify_tolerance_deg > 0.0:
        close_mask_command.extend(["--simplify-tolerance-deg", f"{simplify_tolerance_deg:g}"])

    recipe: dict[str, object] = {
        "kind": "earthmesh_hydro_close_refinement_recipe",
        "input_geojson": input_geojson,
        "output_prefix": output_prefix,
        "class_refine": class_refine,
        "buffer_deg_by_refine_degree": {str(k): v for k, v in sorted(buffer_deg_by_refine_degree.items())},
        "simplify_tolerance_deg": simplify_tolerance_deg,
        "close_mask_command": close_mask_command,
        "earthmesh_namelist_overrides": {
            "RL%refine_spc": ".TRUE.",
            "RL%max_iter_spc": str(max_iter_spc),
            "RL%mask_refine_spc_type": "'close'",
            "RL%mask_refine_spc_fprefix": f"'{output_prefix}'",
        },
        "notes": [
            "Buffers are mesh-generation envelopes, not CoLM river-area estimates.",
            "Use cumulative close masks for nested refinement unless deliberately testing non-cumulative behavior.",
        ],
    }
    if example_namelist:
        recipe["smoke_run_command"] = ["./mkgrd.x", example_namelist]
    return recipe


def write_close_refinement_recipe_json(
    output_json: str | Path,
    *,
    input_geojson: str,
    output_prefix: str,
    class_refine: Mapping[str, int] | None = None,
    buffer_deg_by_refine_degree: Mapping[int, float] | None = None,
    simplify_tolerance_deg: float = 0.0,
    example_namelist: str | None = None,
) -> dict[str, object]:
    recipe = build_close_refinement_recipe(
        input_geojson=input_geojson,
        output_prefix=output_prefix,
        class_refine=class_refine,
        buffer_deg_by_refine_degree=buffer_deg_by_refine_degree,
        simplify_tolerance_deg=simplify_tolerance_deg,
        example_namelist=example_namelist,
    )
    output_path = Path(output_json)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(recipe, indent=2, sort_keys=True) + "\n")
    return recipe


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Write a reproducible EarthMesh hydro close-refinement recipe JSON.")
    parser.add_argument("input_geojson", help="Corridor Polygon/MultiPolygon GeoJSON")
    parser.add_argument("output_prefix", help="Close-mask output prefix used by RL%mask_refine_spc_fprefix")
    parser.add_argument("output_json", help="Recipe JSON path")
    parser.add_argument("--class-refine", nargs="+", default=None, metavar="CLASS=DEGREE")
    parser.add_argument("--buffer-deg-by-refine-degree", nargs="+", default=None, metavar="DEGREE=BUFFER_DEG")
    parser.add_argument("--simplify-tolerance-deg", type=float, default=0.0)
    parser.add_argument("--example-namelist", default=None)
    args = parser.parse_args(argv)

    recipe = write_close_refinement_recipe_json(
        args.output_json,
        input_geojson=args.input_geojson,
        output_prefix=args.output_prefix,
        class_refine=parse_class_refine(args.class_refine),
        buffer_deg_by_refine_degree=parse_degree_buffers(args.buffer_deg_by_refine_degree),
        simplify_tolerance_deg=args.simplify_tolerance_deg,
        example_namelist=args.example_namelist,
    )
    print(json.dumps(recipe, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
