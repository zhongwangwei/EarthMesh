from __future__ import annotations

import argparse
import json
from dataclasses import asdict
from pathlib import Path
from typing import Any, Sequence

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
    parser.add_argument("--cells", required=True, help="Path to canonical cells JSON list.")
    parser.add_argument("--masks", required=True, help="Path to mask features JSON list.")
    parser.add_argument("--adapters", required=True, help="Comma-separated adapter names, e.g. colm2024,mpas,fvcom.")
    parser.add_argument("--output-dir", required=True)
    args = parser.parse_args(argv)

    adapter_names = [name.strip() for name in args.adapters.split(",") if name.strip()]
    result = build_v3_pipeline_result(
        case_name=args.case_name,
        recipe_hash=args.recipe_hash,
        cells=load_cells(args.cells),
        masks=load_masks(args.masks),
        adapter_names=adapter_names,
    )

    output_dir = Path(args.output_dir)
    result.write_sidecars(output_dir)
    (output_dir / "canonical_cells.json").write_text(
        json.dumps([asdict(cell) for cell in result.cells], indent=2, sort_keys=True) + "\n"
    )
    return 0


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
