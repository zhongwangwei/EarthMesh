from __future__ import annotations

from dataclasses import dataclass, field


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


@dataclass(frozen=True)
class CamaVariableReport:
    canonical_to_source: dict[str, str] = field(default_factory=dict)
    missing_required: list[str] = field(default_factory=list)

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
