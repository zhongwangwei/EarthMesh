from __future__ import annotations

import re
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
