from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any, Sequence

from util.hydro_mesh.composite_refine_mask_export import CompositeCloseMaskSummary, write_composite_close_mask_nmls
from util.v3_components.hydro_merit import write_merit_mask_outputs


def write_merit_mesh_regeneration_inputs(
    *,
    case_name: str,
    merit_root: str | Path,
    bbox: tuple[float, float, float, float],
    output_dir: str | Path,
    template_nml: str | Path | None = None,
    case_base_dir: str | Path | None = None,
    stride: int = 1,
    compress_raw_merit: bool = True,
    r2_width_m: float = 50.0,
    r3_width_m: float = 300.0,
    r2_upa_km2: float = 5000.0,
    r3_upa_km2: float = 50000.0,
    r2_cap: int = 60,
    r3_cap: int = 20,
    coast_cap: int = 20,
    simplify_tolerance_deg: float = 0.005,
    min_river_ring_separation_deg: float = 0.25,
) -> dict[str, Any]:
    """Write MERIT-driven EarthMesh close-mask inputs and an optional patched namelist.

    This is the reproducible boundary for the first true MERIT -> mkgrd.x
    regeneration loop: MERIT masks become EarthMesh close-mask ``.nml`` files,
    and a template namelist can be patched to point at the generated prefix.
    """

    directory = Path(output_dir)
    raw_dir = directory / "raw_merit_source"
    close_mask_prefix = directory / "refine_spc_merit"
    recipe_path = directory / "merit_close_mask_recipe.json"
    summary_path = directory / "merit_mesh_regeneration_summary.json"
    directory.mkdir(parents=True, exist_ok=True)

    raw_merit = write_merit_mask_outputs(
        merit_root,
        bbox=bbox,
        output_dir=raw_dir,
        stride=stride,
        r2_width_m=r2_width_m,
        r3_width_m=r3_width_m,
        r2_upa_km2=r2_upa_km2,
        r3_upa_km2=r3_upa_km2,
        write_combined_mask=False,
        write_surface_mask=False,
        compress_geojson=compress_raw_merit,
    )
    recipe = _merit_close_mask_recipe(
        raw_merit,
        r2_cap=r2_cap,
        r3_cap=r3_cap,
        coast_cap=coast_cap,
        simplify_tolerance_deg=simplify_tolerance_deg,
        min_river_ring_separation_deg=min_river_ring_separation_deg,
    )
    recipe_path.write_text(json.dumps(recipe, indent=2, sort_keys=True) + "\n")
    close_mask_summary = write_composite_close_mask_nmls(recipe, close_mask_prefix)
    max_iter_spc = _max_refinement_degree(close_mask_summary)

    patched_nml = None
    if template_nml is not None:
        if case_base_dir is None:
            case_base_dir = directory / "cases"
        patched_nml = directory / f"{case_name}.mnl"
        _write_patched_namelist(
            template_nml,
            patched_nml,
            case_name=case_name,
            case_base_dir=Path(case_base_dir),
            close_mask_prefix=close_mask_prefix,
            max_iter_spc=max_iter_spc,
        )

    summary = {
        "kind": "earthmesh_merit_mesh_regeneration_inputs",
        "case_name": case_name,
        "bbox": list(bbox),
        "stride": stride,
        "compress_raw_merit": compress_raw_merit,
        "files": {
            "raw_merit_dir": str(raw_dir),
            "recipe_json": str(recipe_path),
            "close_mask_prefix": str(close_mask_prefix),
            "close_mask_files": [str(path) for path in close_mask_summary.paths],
            "summary_json": str(summary_path),
            **({"patched_nml": str(patched_nml)} if patched_nml is not None else {}),
        },
        "counts_by_component": close_mask_summary.counts_by_component,
        "counts_by_class_degree": close_mask_summary.counts_by_class_degree,
    }
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")

    return {
        "raw_merit": raw_merit,
        "recipe_json": recipe_path,
        "close_mask_prefix": close_mask_prefix,
        "close_mask_summary": close_mask_summary,
        "patched_nml": patched_nml,
        "summary_json": summary_path,
    }


def _merit_close_mask_recipe(
    raw_merit: dict[str, Path | None],
    *,
    r2_cap: int,
    r3_cap: int,
    coast_cap: int,
    simplify_tolerance_deg: float,
    min_river_ring_separation_deg: float,
) -> dict[str, object]:
    return {
        "kind": "earthmesh_merit_close_mask_recipe",
        "max_masks_per_refine_degree": 999,
        "components": [
            {
                "name": "merit_coast",
                "input_geojson": str(raw_merit["coast_masks"]),
                "class_refine": {"COAST_LAND": 1, "COAST_OCEAN": 1},
                "max_rings_by_class": {"COAST_LAND": coast_cap, "COAST_OCEAN": coast_cap},
                "simplify_tolerance_deg": simplify_tolerance_deg,
            },
            {
                "name": "merit_river",
                "input_geojson": str(raw_merit["river_masks"]),
                "class_refine": {"R2": 1, "R3": 3},
                "max_rings_by_class": {"R2": r2_cap, "R3": r3_cap},
                "buffer_deg_by_refine_degree": {"1": 1.5, "2": 1.0, "3": 0.5},
                "simplify_tolerance_deg": simplify_tolerance_deg,
                "min_ring_separation_deg": min_river_ring_separation_deg,
            },
        ],
    }


def _max_refinement_degree(summary: CompositeCloseMaskSummary) -> int:
    max_degree = 1
    for key in summary.counts_by_class_degree:
        match = re.search(r"_d(\d+)$", key)
        if match:
            max_degree = max(max_degree, int(match.group(1)))
    return max_degree


def _write_patched_namelist(
    template_nml: str | Path,
    output_nml: str | Path,
    *,
    case_name: str,
    case_base_dir: Path,
    close_mask_prefix: Path,
    max_iter_spc: int,
) -> Path:
    text = Path(template_nml).read_text()
    base_dir = _fortran_dir(case_base_dir)
    replacements = {
        r"NL%EXPNME\s*=\s*'[^']*'": f"NL%EXPNME = '{case_name}'",
        r"NL%base_dir\s*=\s*'[^']*'": f"NL%base_dir = '{base_dir}'",
        r"RL%mask_refine_spc_fprefix\s*=\s*'[^']*'": f"RL%mask_refine_spc_fprefix = '{close_mask_prefix}'",
        r"RL%max_iter_spc\s*=\s*\d+": f"RL%max_iter_spc = {max_iter_spc}",
    }
    for pattern, value in replacements.items():
        text, count = re.subn(pattern, value, text)
        if count != 1:
            raise ValueError(f"expected exactly one namelist field matching {pattern}")
    output_path = Path(output_nml)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(text)
    return output_path


def _fortran_dir(path: Path) -> str:
    text = str(path)
    return text if text.endswith("/") else f"{text}/"


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Prepare MERIT-driven EarthMesh close-mask regeneration inputs.")
    parser.add_argument("--case-name", required=True)
    parser.add_argument("--merit-root", required=True)
    parser.add_argument("--bbox", nargs=4, type=float, metavar=("WEST", "SOUTH", "EAST", "NORTH"), required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--template-nml")
    parser.add_argument("--case-base-dir")
    parser.add_argument("--stride", type=int, default=1)
    parser.add_argument("--no-compress-raw-merit", action="store_true")
    parser.add_argument("--r2-width-m", type=float, default=50.0)
    parser.add_argument("--r3-width-m", type=float, default=300.0)
    parser.add_argument("--r2-upa-km2", type=float, default=5000.0)
    parser.add_argument("--r3-upa-km2", type=float, default=50000.0)
    parser.add_argument("--r2-cap", type=int, default=60)
    parser.add_argument("--r3-cap", type=int, default=20)
    parser.add_argument("--coast-cap", type=int, default=20)
    parser.add_argument("--simplify-tolerance-deg", type=float, default=0.005)
    parser.add_argument("--min-river-ring-separation-deg", type=float, default=0.25)
    args = parser.parse_args(argv)

    result = write_merit_mesh_regeneration_inputs(
        case_name=args.case_name,
        merit_root=args.merit_root,
        bbox=tuple(args.bbox),
        output_dir=args.output_dir,
        template_nml=args.template_nml,
        case_base_dir=args.case_base_dir,
        stride=args.stride,
        compress_raw_merit=not args.no_compress_raw_merit,
        r2_width_m=args.r2_width_m,
        r3_width_m=args.r3_width_m,
        r2_upa_km2=args.r2_upa_km2,
        r3_upa_km2=args.r3_upa_km2,
        r2_cap=args.r2_cap,
        r3_cap=args.r3_cap,
        coast_cap=args.coast_cap,
        simplify_tolerance_deg=args.simplify_tolerance_deg,
        min_river_ring_separation_deg=args.min_river_ring_separation_deg,
    )
    printable = {
        key: str(value)
        for key, value in result.items()
        if key not in {"raw_merit", "close_mask_summary"}
    }
    printable["raw_merit"] = {
        key: str(value) if value is not None else None for key, value in result["raw_merit"].items()
    }
    printable["close_mask_files"] = [str(path) for path in result["close_mask_summary"].paths]
    print(json.dumps(printable, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
