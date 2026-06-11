from __future__ import annotations

import argparse
import json
import re
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence

import numpy as np

_TILE_RE = re.compile(r"^([ns])(\d{2})([ew])(\d{3})\.nc$")


@dataclass(frozen=True)
class MeritWindow:
    tile_path: Path
    lon: np.ndarray
    lat: np.ndarray
    dir: np.ndarray
    upa: np.ndarray
    elv: np.ndarray
    wth: np.ndarray
    landtype_igbp: np.ndarray


def tile_bounds_from_name(name: str) -> tuple[float, float, float, float]:
    match = _TILE_RE.match(Path(name).name)
    if not match:
        raise ValueError(f"not a MERIT-Hydro tile name: {name}")
    lat_sign, lat_text, lon_sign, lon_text = match.groups()
    lat0 = float(lat_text) * (1 if lat_sign == "n" else -1)
    lon0 = float(lon_text) * (1 if lon_sign == "e" else -1)
    return lon0, lat0, lon0 + 5.0, lat0 + 5.0


def select_merit_tiles(root: str | Path, bbox: tuple[float, float, float, float]) -> list[Path]:
    root_path = Path(root)
    selected = []
    for path in sorted(root_path.glob("*.nc")):
        try:
            bounds = tile_bounds_from_name(path.name)
        except ValueError:
            continue
        if _intersects(bounds, bbox):
            selected.append(path)
    return selected


def read_merit_window(tile_path: str | Path, bbox: tuple[float, float, float, float], *, stride: int = 1) -> MeritWindow:
    if stride <= 0:
        raise ValueError("stride must be positive")
    try:
        import netCDF4
    except ImportError as exc:  # pragma: no cover - dependency is present in the test/runtime env
        raise RuntimeError("MERIT-Hydro NetCDF reading requires netCDF4") from exc

    path = Path(tile_path)
    with netCDF4.Dataset(path) as ds:
        lon_all = np.asarray(ds.variables["longitude"][:], dtype=float)
        lat_all = np.asarray(ds.variables["latitude"][:], dtype=float)
        lon_idx = _indices_between(lon_all, bbox[0], bbox[2], stride=stride)
        lat_idx = _indices_between(lat_all, bbox[1], bbox[3], stride=stride)
        if lon_idx.size == 0 or lat_idx.size == 0:
            raise ValueError(f"bbox does not overlap tile coordinates: {path}")
        lon_slice = _slice_from_indices(lon_idx)
        lat_slice = _slice_from_indices(lat_idx)
        return MeritWindow(
            tile_path=path,
            lon=lon_all[lon_idx],
            lat=lat_all[lat_idx],
            dir=np.asarray(ds.variables["dir"][lon_slice, lat_slice]),
            upa=_clean_fill(np.asarray(ds.variables["upa"][lon_slice, lat_slice], dtype=float)),
            elv=_clean_fill(np.asarray(ds.variables["elv"][lon_slice, lat_slice], dtype=float)),
            wth=_clean_fill(np.asarray(ds.variables["wth"][lon_slice, lat_slice], dtype=float)),
            landtype_igbp=np.asarray(ds.variables["landtype_igbp"][lon_slice, lat_slice]),
        )


def _intersects(a: tuple[float, float, float, float], b: tuple[float, float, float, float]) -> bool:
    return a[0] < b[2] and a[2] > b[0] and a[1] < b[3] and a[3] > b[1]


def _indices_between(values: np.ndarray, low: float, high: float, *, stride: int) -> np.ndarray:
    mask = (values >= low) & (values <= high)
    indices = np.where(mask)[0]
    return indices[::stride]


def _slice_from_indices(indices: Sequence[int]) -> slice:
    return slice(int(indices[0]), int(indices[-1]) + 1, int(indices[1] - indices[0]) if len(indices) > 1 else 1)


def _clean_fill(values: np.ndarray) -> np.ndarray:
    cleaned = np.asarray(values, dtype=float)
    cleaned[cleaned <= -9990.0] = np.nan
    return cleaned


def build_merit_masks(
    windows: list[MeritWindow],
    *,
    r2_width_m: float = 50.0,
    r3_width_m: float = 300.0,
    r2_upa_km2: float = 5000.0,
    r3_upa_km2: float = 50000.0,
) -> tuple[dict[str, object], dict[str, object]]:
    features: list[dict[str, object]] = []
    counts: Counter[str] = Counter()
    for window in windows:
        land_ocean_adjacency = _land_ocean_adjacency(window.landtype_igbp)
        for i, lon in enumerate(window.lon):
            for j, lat in enumerate(window.lat):
                mask_class = _classify_cell(
                    float(window.wth[i, j]),
                    float(window.upa[i, j]),
                    int(window.landtype_igbp[i, j]),
                    adjacent_to_other_surface=bool(land_ocean_adjacency[i, j]),
                    r2_width_m=r2_width_m,
                    r3_width_m=r3_width_m,
                    r2_upa_km2=r2_upa_km2,
                    r3_upa_km2=r3_upa_km2,
                )
                if mask_class == "UNKNOWN":
                    continue
                counts[mask_class] += 1
                features.append(_mask_feature(window, i, j, mask_class))
    summary = {
        "tile_count": len(windows),
        "feature_count": len(features),
        "mask_counts": dict(sorted(counts.items())),
        "thresholds": {
            "r2_width_m": r2_width_m,
            "r3_width_m": r3_width_m,
            "r2_upa_km2": r2_upa_km2,
            "r3_upa_km2": r3_upa_km2,
        },
    }
    return {"type": "FeatureCollection", "features": features}, summary


def write_merit_mask_outputs(
    merit_root: str | Path,
    *,
    bbox: tuple[float, float, float, float],
    output_dir: str | Path,
    stride: int = 1,
    r2_width_m: float = 50.0,
    r3_width_m: float = 300.0,
    r2_upa_km2: float = 5000.0,
    r3_upa_km2: float = 50000.0,
) -> dict[str, Path]:
    tiles = select_merit_tiles(merit_root, bbox)
    if not tiles:
        raise ValueError(f"no MERIT-Hydro tiles intersect bbox={bbox}")
    windows = [read_merit_window(tile, bbox, stride=stride) for tile in tiles]
    masks, summary = build_merit_masks(
        windows,
        r2_width_m=r2_width_m,
        r3_width_m=r3_width_m,
        r2_upa_km2=r2_upa_km2,
        r3_upa_km2=r3_upa_km2,
    )
    output = Path(output_dir)
    output.mkdir(parents=True, exist_ok=True)
    mask_path = output / "merit_masks.geojson"
    summary_path = output / "merit_mask_summary.json"
    mask_path.write_text(json.dumps(masks, indent=2, sort_keys=True) + "\n")
    summary_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    return {"masks": mask_path, "summary": summary_path}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Build v3 mask GeoJSON from MERIT-Hydro 90m NetCDF tiles.")
    parser.add_argument("--merit-root", required=True)
    parser.add_argument("--bbox", nargs=4, type=float, metavar=("MIN_LON", "MIN_LAT", "MAX_LON", "MAX_LAT"), required=True)
    parser.add_argument("--output-dir", required=True)
    parser.add_argument("--stride", type=int, default=1, help="Read every Nth MERIT cell; use larger values for smoke tests.")
    parser.add_argument("--r2-width-m", type=float, default=50.0)
    parser.add_argument("--r3-width-m", type=float, default=300.0)
    parser.add_argument("--r2-upa-km2", type=float, default=5000.0)
    parser.add_argument("--r3-upa-km2", type=float, default=50000.0)
    args = parser.parse_args(argv)
    write_merit_mask_outputs(
        args.merit_root,
        bbox=tuple(args.bbox),
        output_dir=args.output_dir,
        stride=args.stride,
        r2_width_m=args.r2_width_m,
        r3_width_m=args.r3_width_m,
        r2_upa_km2=args.r2_upa_km2,
        r3_upa_km2=args.r3_upa_km2,
    )
    return 0


def _classify_cell(
    width_m: float,
    upa_km2: float,
    landtype_igbp: int,
    *,
    adjacent_to_other_surface: bool = False,
    r2_width_m: float,
    r3_width_m: float,
    r2_upa_km2: float,
    r3_upa_km2: float,
) -> str:
    if np.isfinite(width_m) and np.isfinite(upa_km2):
        if width_m >= r3_width_m or upa_km2 >= r3_upa_km2:
            return "R3"
        if width_m >= r2_width_m or upa_km2 >= r2_upa_km2:
            return "R2"
    if landtype_igbp == 0 or landtype_igbp == 17:
        if adjacent_to_other_surface:
            return "COAST_OCEAN"
        return "OCEAN"
    if landtype_igbp > 0:
        if adjacent_to_other_surface:
            return "COAST_LAND"
        return "LAND"
    return "UNKNOWN"


def _land_ocean_adjacency(landtype: np.ndarray) -> np.ndarray:
    surface = np.asarray(landtype)
    is_ocean = (surface == 0) | (surface == 17)
    is_land = surface > 0
    is_land = is_land & ~is_ocean
    adjacency = np.zeros(surface.shape, dtype=bool)
    for di in (-1, 0, 1):
        for dj in (-1, 0, 1):
            if di == 0 and dj == 0:
                continue
            shifted_ocean = _shift_bool(is_ocean, di, dj)
            shifted_land = _shift_bool(is_land, di, dj)
            adjacency |= (is_land & shifted_ocean) | (is_ocean & shifted_land)
    return adjacency


def _shift_bool(values: np.ndarray, di: int, dj: int) -> np.ndarray:
    shifted = np.zeros(values.shape, dtype=bool)
    src_i_start = max(0, -di)
    src_i_end = values.shape[0] - max(0, di)
    src_j_start = max(0, -dj)
    src_j_end = values.shape[1] - max(0, dj)
    dst_i_start = max(0, di)
    dst_i_end = values.shape[0] - max(0, -di)
    dst_j_start = max(0, dj)
    dst_j_end = values.shape[1] - max(0, -dj)
    shifted[dst_i_start:dst_i_end, dst_j_start:dst_j_end] = values[src_i_start:src_i_end, src_j_start:src_j_end]
    return shifted


def _mask_feature(window: MeritWindow, i: int, j: int, mask_class: str) -> dict[str, object]:
    lon = float(window.lon[i])
    lat = float(window.lat[j])
    dlon = _cell_delta(window.lon, i)
    dlat = _cell_delta(window.lat, j)
    lon0, lon1 = lon - dlon / 2.0, lon + dlon / 2.0
    lat0, lat1 = lat - dlat / 2.0, lat + dlat / 2.0
    feature_id = f"{window.tile_path.stem}:{i}:{j}:{mask_class}"
    return {
        "type": "Feature",
        "geometry": {
            "type": "Polygon",
            "coordinates": [[[lon0, lat0], [lon1, lat0], [lon1, lat1], [lon0, lat1], [lon0, lat0]]],
        },
        "properties": {
            "feature_id": feature_id,
            "mask_class": mask_class,
            "source": "MERIT-Hydro",
            "tile": window.tile_path.name,
            "width_m": _finite_or_none(float(window.wth[i, j])),
            "upstream_area_km2": _finite_or_none(float(window.upa[i, j])),
            "elevation_m": _finite_or_none(float(window.elv[i, j])),
            "landtype_igbp": int(window.landtype_igbp[i, j]),
        },
    }


def _cell_delta(values: np.ndarray, index: int) -> float:
    if len(values) <= 1:
        return 0.0008333333333333334
    if index == 0:
        return abs(float(values[1] - values[0]))
    return abs(float(values[index] - values[index - 1]))


def _finite_or_none(value: float) -> float | None:
    return value if np.isfinite(value) else None


if __name__ == "__main__":
    raise SystemExit(main())
