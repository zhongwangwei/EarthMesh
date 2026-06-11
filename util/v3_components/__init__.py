"""EarthMesh v3 component bridges."""

from __future__ import annotations

_MERIT_EXPORTS = {
    "MeritWindow",
    "build_merit_masks",
    "read_merit_window",
    "select_merit_tiles",
    "tile_bounds_from_name",
    "write_merit_mask_outputs",
}

__all__ = sorted(_MERIT_EXPORTS)


def __getattr__(name: str):
    if name in _MERIT_EXPORTS:
        from util.v3_components import hydro_merit

        return getattr(hydro_merit, name)
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
