from __future__ import annotations

import argparse
import json
from pathlib import Path

from util.hydro_mesh.cama_binary import CamaGridSpec
from util.hydro_mesh.cama_contract import parse_cama_params_text
from util.hydro_mesh.cama_inventory import CamaReachRecord, read_reach_inventory_window
from util.hydro_mesh.classifier import classify_reach


def grid_from_params_file(params_file: str | Path, *, y_reversed_storage: bool = True) -> CamaGridSpec:
    params = parse_cama_params_text(Path(params_file).read_text())
    return CamaGridSpec(
        nx=int(params["nx"]),
        ny=int(params["ny"]),
        west=float(params["west"]),
        south=float(params["south"]),
        grid_size_deg=float(params["grid_size_deg"]),
        y_reversed_storage=y_reversed_storage,
    )


def _record_to_dict(record: CamaReachRecord) -> dict[str, object]:
    classification = classify_reach(record.reach)
    return {
        "reach_id": record.reach.reach_id,
        "river_class": classification.river_class,
        "reasons": classification.reasons,
        "upstream_area_km2": record.reach.upstream_area_km2,
        "width_m": record.reach.width_m,
        "river_length_m": record.river_length_m,
        "lon": record.lon,
        "lat": record.lat,
        "x_index": record.x_index,
        "y_index": record.y_index,
        "downstream_x": record.downstream_x,
        "downstream_y": record.downstream_y,
        "is_estuary": record.reach.is_estuary,
    }


def sample_cama_window_to_jsonl(
    map_dir: str | Path,
    output_jsonl: str | Path,
    *,
    bbox: tuple[float, float, float, float],
    target_dx_km: float,
    uparea_to_km2: float = 1e-6,
    y_reversed_storage: bool = True,
) -> list[dict[str, object]]:
    root = Path(map_dir)
    grid = grid_from_params_file(root / "params.txt", y_reversed_storage=y_reversed_storage)
    west, south, east, north = bbox
    x_start, y_start, width, height = grid.window_for_bbox(
        west=west,
        east=east,
        south=south,
        north=north,
    )
    records = read_reach_inventory_window(
        root,
        grid,
        x_start=x_start,
        y_start=y_start,
        width=width,
        height=height,
        target_dx_km=target_dx_km,
        uparea_to_km2=uparea_to_km2,
    )
    rows = [_record_to_dict(record) for record in records]

    output_path = Path(output_jsonl)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w") as handle:
        for row in rows:
            handle.write(json.dumps(row, sort_keys=True) + "\n")
    return rows


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Sample a CaMa binary map bbox into classified hydro-mesh JSONL records.")
    parser.add_argument("map_dir", help="Directory containing params.txt, nextxy.bin, uparea.bin, width.bin, and rivlen.bin")
    parser.add_argument("output_jsonl", help="Output JSON Lines file")
    parser.add_argument("--bbox", nargs=4, type=float, metavar=("WEST", "SOUTH", "EAST", "NORTH"), required=True)
    parser.add_argument("--target-dx-km", type=float, required=True)
    parser.add_argument("--uparea-to-km2", type=float, default=1e-6)
    parser.add_argument("--no-yrev", action="store_true", help="Disable y-reversed binary row order")
    args = parser.parse_args(argv)

    sample_cama_window_to_jsonl(
        args.map_dir,
        args.output_jsonl,
        bbox=tuple(args.bbox),
        target_dx_km=args.target_dx_km,
        uparea_to_km2=args.uparea_to_km2,
        y_reversed_storage=not args.no_yrev,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
