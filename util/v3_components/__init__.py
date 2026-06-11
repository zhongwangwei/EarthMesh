"""EarthMesh v3 component bridges."""

from __future__ import annotations

_MERIT_EXPORTS = {
    "MeritWindow",
    "build_merit_masks",
    "read_merit_window",
    "select_merit_tiles",
    "split_merit_mask_layers",
    "tile_bounds_from_name",
    "write_merit_mask_outputs",
}

_MERIT_PIPELINE_EXPORTS = {
    "run_merit_v3_pipeline",
}

__all__ = sorted(_MERIT_EXPORTS | _MERIT_PIPELINE_EXPORTS)


def __getattr__(name: str):
    if name in _MERIT_EXPORTS:
        from util.v3_components import hydro_merit

        return getattr(hydro_merit, name)
    if name in _MERIT_PIPELINE_EXPORTS:
        from util.v3_components import hydro_merit_pipeline

        return getattr(hydro_merit_pipeline, name)
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
