from __future__ import annotations

import argparse
import json
from dataclasses import asdict
from pathlib import Path
from typing import Sequence

from util.v3_components.hydro_merit import write_merit_mask_outputs
from util.v3_core.adaptive_grid import write_refined_cells_geojson
from util.v3_core.geojson_io import load_cells_geojson, load_masks_geojson, write_cells_geojson
from util.v3_core.grid import write_bbox_grid_geojson
from util.v3_core.map import canonical_cells_geojson_to_leaflet_html
from util.v3_core.pipeline import build_v3_pipeline_result


def run_merit_v3_pipeline(
    *,
    merit_root: str | Path,
    bbox: tuple[float, float, float, float],
    nx: int,
    ny: int,
    output_dir: str | Path,
    case_name: str,
    recipe_hash: str,
    adapters: Sequence[str],
    stride: int = 1,
    html_map: str | Path | None = None,
    cell_id_prefix: str = "cell",
    r2_width_m: float = 50.0,
    r3_width_m: float = 300.0,
    r2_upa_km2: float = 5000.0,
    r3_upa_km2: float = 50000.0,
    refine_classes: Sequence[str] | None = None,
    refine_factor: int = 2,
) -> dict[str, Path]:
    """Run the bootstrap MERIT-Hydro -> v3 regional pipeline.

    This is a reproducible smoke/development pipeline. It deliberately uses a
    regular bbox grid as the cell input; adaptive coast/river refinement remains
    a separate mesh-generation concern.
    """
    if not adapters:
        raise ValueError("at least one adapter is required")

    root = Path(output_dir)
    root.mkdir(parents=True, exist_ok=True)
    cells_geojson = root / "cells.geojson"
    merit_dir = root / "merit"
    v3_dir = root / "v3"

    write_bbox_grid_geojson(bbox, nx=nx, ny=ny, output_path=cells_geojson, cell_id_prefix=cell_id_prefix)
    merit_outputs = write_merit_mask_outputs(
        merit_root,
        bbox=bbox,
        output_dir=merit_dir,
        stride=stride,
        r2_width_m=r2_width_m,
        r3_width_m=r3_width_m,
        r2_upa_km2=r2_upa_km2,
        r3_upa_km2=r3_upa_km2,
    )
    masks = load_masks_geojson(merit_outputs["masks"])
    refined_classes = _normalize_refine_classes(refine_classes)
    if refined_classes:
        write_refined_cells_geojson(
            load_cells_geojson(cells_geojson),
            masks,
            refine_classes=set(refined_classes),
            factor=refine_factor,
            output_path=cells_geojson,
        )

    result = build_v3_pipeline_result(
        case_name=case_name,
        recipe_hash=recipe_hash,
        cells=load_cells_geojson(cells_geojson),
        masks=masks,
        adapter_names=list(adapters),
    )
    sidecars = result.write_sidecars(v3_dir)
    canonical_cells_json = v3_dir / "canonical_cells.json"
    canonical_cells_json.write_text(json.dumps([asdict(cell) for cell in result.cells], indent=2, sort_keys=True) + "\n")
    canonical_cells_geojson = write_cells_geojson(result.cells, v3_dir / "canonical_cells.geojson")

    outputs: dict[str, Path] = {
        "cells_geojson": cells_geojson,
        "masks_geojson": merit_outputs["masks"],
        "river_masks": merit_outputs["river_masks"],
        "coast_masks": merit_outputs["coast_masks"],
        "surface_masks": merit_outputs["surface_masks"],
        "merit_summary": merit_outputs["summary"],
        "manifest": sidecars["manifest"],
        "overlay_summary": sidecars["overlay_summary"],
        "canonical_cells_json": canonical_cells_json,
        "canonical_cells_geojson": canonical_cells_geojson,
    }
    for adapter_name in adapters:
        key = f"adapter_{adapter_name}"
        if key in sidecars:
            outputs[key] = sidecars[key]

    if html_map is not None:
        map_path = Path(html_map)
        canonical_cells_geojson_to_leaflet_html(canonical_cells_geojson, map_path, title=case_name)
        outputs["html_map"] = map_path

    summary_path = root / "pipeline_summary.json"
    _write_pipeline_summary(
        summary_path,
        case_name=case_name,
        recipe_hash=recipe_hash,
        merit_root=Path(merit_root),
        bbox=bbox,
        nx=nx,
        ny=ny,
        cell_id_prefix=cell_id_prefix,
        adapters=list(adapters),
        stride=stride,
        thresholds={
            "r2_width_m": r2_width_m,
            "r3_width_m": r3_width_m,
            "r2_upa_km2": r2_upa_km2,
            "r3_upa_km2": r3_upa_km2,
        },
        refinement={"enabled": bool(refined_classes), "classes": refined_classes, "factor": refine_factor},
        files=outputs,
    )
    outputs["pipeline_summary"] = summary_path
    return outputs


def _write_pipeline_summary(
    path: Path,
    *,
    case_name: str,
    recipe_hash: str,
    merit_root: Path,
    bbox: tuple[float, float, float, float],
    nx: int,
    ny: int,
    cell_id_prefix: str,
    adapters: list[str],
    stride: int,
    thresholds: dict[str, float],
    refinement: dict[str, object],
    files: dict[str, Path],
) -> Path:
    payload = {
        "case_name": case_name,
        "recipe_hash": recipe_hash,
        "merit_root": str(merit_root),
        "bbox": list(bbox),
        "grid": {"nx": nx, "ny": ny, "cell_id_prefix": cell_id_prefix},
        "adapters": adapters,
        "stride": stride,
        "thresholds": thresholds,
        "refinement": refinement,
        "files": {name: str(file_path) for name, file_path in sorted(files.items())},
    }
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    return path


def _normalize_refine_classes(refine_classes: Sequence[str] | None) -> list[str]:
    if not refine_classes:
        return []
    normalized: list[str] = []
    for item in refine_classes:
        for value in str(item).split(","):
            value = value.strip()
            if value and value not in normalized:
                normalized.append(value)
    return normalized


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Run a MERIT-Hydro -> EarthMesh v3 regional smoke pipeline.")
    parser.add_argument("--merit-root", required=True)
    parser.add_argument("--bbox", nargs=4, type=float, metavar=("MIN_LON", "MIN_LAT", "MAX_LON", "MAX_LAT"), required=True)
    parser.add_argument("--nx", type=int, required=True)
    parser.add_argument("--ny", type=int, required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--case-name", required=True)
    parser.add_argument("--recipe-hash", required=True)
    parser.add_argument("--adapters", required=True, help="Comma-separated adapter names, e.g. colm2024,mpas,fvcom.")
    parser.add_argument("--stride", type=int, default=1, help="Read every Nth MERIT cell; use larger values for smoke tests.")
    parser.add_argument("--html-map", help="Optional output HTML path for a Leaflet QA map of canonical cells.")
    parser.add_argument("--cell-id-prefix", default="cell")
    parser.add_argument("--r2-width-m", type=float, default=50.0)
    parser.add_argument("--r3-width-m", type=float, default=300.0)
    parser.add_argument("--r2-upa-km2", type=float, default=5000.0)
    parser.add_argument("--r3-upa-km2", type=float, default=50000.0)
    parser.add_argument("--refine-classes", help="Comma-separated mask classes to refine, e.g. R2,R3,COAST_LAND,COAST_OCEAN.")
    parser.add_argument("--refine-factor", type=int, default=2)
    args = parser.parse_args(argv)

    adapters = [name.strip() for name in args.adapters.split(",") if name.strip()]
    run_merit_v3_pipeline(
        merit_root=args.merit_root,
        bbox=tuple(args.bbox),
        nx=args.nx,
        ny=args.ny,
        output_dir=args.output_dir,
        case_name=args.case_name,
        recipe_hash=args.recipe_hash,
        adapters=adapters,
        stride=args.stride,
        html_map=args.html_map,
        cell_id_prefix=args.cell_id_prefix,
        r2_width_m=args.r2_width_m,
        r3_width_m=args.r3_width_m,
        r2_upa_km2=args.r2_upa_km2,
        r3_upa_km2=args.r3_upa_km2,
        refine_classes=_normalize_refine_classes([args.refine_classes] if args.refine_classes else None),
        refine_factor=args.refine_factor,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
