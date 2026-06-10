from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import PurePosixPath


_REQUIRED_CANONICAL_FIELDS = [
    "lon",
    "lat",
    "downstream_topology",
    "upstream_area",
    "river_width",
    "river_length",
]

_ALIAS_TABLE = {
    "lon": ("lon", "xlon", "longitude"),
    "lat": ("lat", "ylat", "latitude"),
    "downstream_topology": ("nextxy", "nextx", "nexty", "downstream", "downstream_index"),
    "upstream_area": ("uparea", "upstream_area", "upa"),
    "river_width": ("rivwth", "river_width", "width"),
    "river_length": ("rivlen", "river_length", "length"),
    "floodplain_width": ("fldwth", "floodplain_width", "flood_width"),
}

_BINARY_FIELD_CANDIDATES = {
    "downstream_topology": ("nextxy.bin", "downxy.bin"),
    "upstream_area": ("uparea.bin", "uparea_grid.bin"),
    "river_width": ("rivwth.bin", "width.bin"),
    "river_length": ("rivlen.bin", "rivlen_grid.bin"),
    "floodplain_height": ("fldhgt.bin",),
    "catchment_area": ("ctmare.bin",),
}

_REQUIRED_BINARY_FIELDS = [
    "downstream_topology",
    "upstream_area",
    "river_width",
    "river_length",
]


@dataclass(frozen=True)
class CamaVariableReport:
    canonical_to_source: dict[str, str] = field(default_factory=dict)
    missing_required: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)

    @property
    def is_usable(self) -> bool:
        return not self.missing_required


def inspect_cama_variables(variable_names: list[str] | tuple[str, ...] | set[str]) -> CamaVariableReport:
    original_by_normalized = {name.lower(): name for name in variable_names}
    canonical_to_source: dict[str, str] = {}

    for canonical, aliases in _ALIAS_TABLE.items():
        for alias in aliases:
            if alias in original_by_normalized:
                canonical_to_source[canonical] = original_by_normalized[alias]
                break

    missing_required = [
        canonical for canonical in _REQUIRED_CANONICAL_FIELDS if canonical not in canonical_to_source
    ]
    return CamaVariableReport(canonical_to_source, missing_required)


def inspect_cama_file_inventory(file_names: list[str] | tuple[str, ...] | set[str]) -> CamaVariableReport:
    original_by_basename = {PurePosixPath(name).name.lower(): name for name in file_names}
    canonical_to_source: dict[str, str] = {}
    warnings: list[str] = []

    for canonical, candidates in _BINARY_FIELD_CANDIDATES.items():
        for candidate in candidates:
            if candidate in original_by_basename:
                canonical_to_source[canonical] = original_by_basename[candidate]
                break

    if canonical_to_source.get("river_width", "").endswith("width.bin") and "rivwth.bin" not in original_by_basename:
        warnings.append("using width.bin because rivwth.bin is absent")

    missing_required = [
        canonical for canonical in _REQUIRED_BINARY_FIELDS if canonical not in canonical_to_source
    ]
    return CamaVariableReport(canonical_to_source, missing_required, warnings)


def parse_cama_params_text(text: str) -> dict[str, float | int]:
    values: list[float] = []
    for line in text.splitlines():
        payload = line.split("!!", 1)[0].strip()
        if not payload:
            continue
        values.append(float(payload.split()[0]))

    if len(values) < 8:
        raise ValueError("CaMa params text must contain at least 8 numeric records")

    return {
        "nx": int(values[0]),
        "ny": int(values[1]),
        "floodplain_layers": int(values[2]),
        "grid_size_deg": values[3],
        "west": values[4],
        "east": values[5],
        "south": values[6],
        "north": values[7],
    }
