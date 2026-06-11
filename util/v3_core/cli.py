from __future__ import annotations

import argparse
import json
from dataclasses import asdict
from pathlib import Path
from typing import Any, Sequence

from util.v3_core.geojson_io import load_cells_geojson, load_masks_geojson, write_cells_geojson
from util.v3_core.geometry import MaskFeature
from util.v3_core.pipeline import build_v3_pipeline_result
from util.v3_core.schema import CanonicalCell


def load_cells(path: str | Path) -> list[CanonicalCell]:
    payload = json.loads(Path(path).read_text())
    if not isinstance(payload, list):
        raise ValueError("cells JSON must contain a list")
    return [CanonicalCell(**_cell_mapping(item)) for item in payload]


def load_masks(path: str | Path) -> list[MaskFeature]:
    payload = json.loads(Path(path).read_text())
    if not isinstance(payload, list):
        raise ValueError("masks JSON must contain a list")
    return [MaskFeature(**_mask_mapping(item)) for item in payload]


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Run the EarthMesh v3 canonical pipeline from JSON cells and masks.")
    parser.add_argument("--case-name", required=True)
    parser.add_argument("--recipe-hash", required=True)
    cell_inputs = parser.add_mutually_exclusive_group(required=True)
    cell_inputs.add_argument("--cells", help="Path to canonical cells JSON list.")
    cell_inputs.add_argument("--cells-geojson", help="Path to cell Polygon GeoJSON FeatureCollection.")
    mask_inputs = parser.add_mutually_exclusive_group(required=True)
    mask_inputs.add_argument("--masks", help="Path to mask features JSON list.")
    mask_inputs.add_argument("--masks-geojson", help="Path to mask Polygon GeoJSON FeatureCollection.")
    parser.add_argument("--adapters", required=True, help="Comma-separated adapter names, e.g. colm2024,mpas,fvcom.")
    parser.add_argument("--output-dir", required=True)
    args = parser.parse_args(argv)

    adapter_names = [name.strip() for name in args.adapters.split(",") if name.strip()]
    result = build_v3_pipeline_result(
        case_name=args.case_name,
        recipe_hash=args.recipe_hash,
        cells=_load_cells_from_args(args),
        masks=_load_masks_from_args(args),
        adapter_names=adapter_names,
    )

    output_dir = Path(args.output_dir)
    result.write_sidecars(output_dir)
    (output_dir / "canonical_cells.json").write_text(
        json.dumps([asdict(cell) for cell in result.cells], indent=2, sort_keys=True) + "\n"
    )
    write_cells_geojson(result.cells, output_dir / "canonical_cells.geojson")
    return 0


def _load_cells_from_args(args: argparse.Namespace) -> list[CanonicalCell]:
    if args.cells_geojson:
        return load_cells_geojson(args.cells_geojson)
    return load_cells(args.cells)


def _load_masks_from_args(args: argparse.Namespace) -> list[MaskFeature]:
    if args.masks_geojson:
        return load_masks_geojson(args.masks_geojson)
    return load_masks(args.masks)


def _cell_mapping(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError("each cell must be an object")
    return dict(value)


def _mask_mapping(value: Any) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValueError("each mask must be an object")
    return dict(value)


if __name__ == "__main__":
    raise SystemExit(main())
