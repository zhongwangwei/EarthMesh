from __future__ import annotations

import csv
import json
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any
from typing import Iterable, Protocol

from util.v3_core.schema import CanonicalCell


@dataclass(frozen=True)
class AdapterExportPlan:
    adapter_name: str
    adapter_version: str
    output_format: str
    supported_cell_types: list[str]
    required_fields: list[str]
    cell_type_counts: dict[str, int]
    warnings: list[str]
    files: dict[str, str] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        payload = asdict(self)
        return json.loads(json.dumps(payload, sort_keys=True))

    def write_json(self, output_path: str | Path) -> Path:
        path = Path(output_path)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(self.to_dict(), indent=2, sort_keys=True) + "\n")
        return path


class Adapter(Protocol):
    name: str
    version: str
    supported_cell_types: set[str]

    def validate_cells(self, cells: Iterable[CanonicalCell]) -> list[str]:
        ...

    def plan_export(self, cells: Iterable[CanonicalCell]) -> AdapterExportPlan:
        ...


@dataclass(frozen=True)
class SchemaOnlyAdapter:
    name: str
    version: str
    supported_cell_types: set[str]
    output_format: str = "schema_manifest"
    required_fields: tuple[str, ...] = (
        "cell_id",
        "cell_index",
        "cell_type",
        "center_lon",
        "center_lat",
        "area_m2",
        "vertices",
        "surface_class",
        "hydro_class",
        "coast_class",
        "source_fractions",
        "quality_flags",
    )

    def validate_cells(self, cells: Iterable[CanonicalCell]) -> list[str]:
        warnings: list[str] = []
        for cell in cells:
            if cell.cell_type not in self.supported_cell_types:
                warnings.append(f"{self.name} does not support cell_type={cell.cell_type} for cell_id={cell.cell_id}")
        return warnings

    def plan_export(self, cells: Iterable[CanonicalCell]) -> AdapterExportPlan:
        materialized = list(cells)
        cell_type_counts: dict[str, int] = {}
        for cell in materialized:
            cell_type_counts[cell.cell_type] = cell_type_counts.get(cell.cell_type, 0) + 1

        return AdapterExportPlan(
            adapter_name=self.name,
            adapter_version=self.version,
            output_format=self.output_format,
            supported_cell_types=sorted(self.supported_cell_types),
            required_fields=list(self.required_fields),
            cell_type_counts=cell_type_counts,
            warnings=self.validate_cells(materialized),
        )


class AdapterRegistry:
    def __init__(self) -> None:
        self._adapters: dict[str, Adapter] = {}

    def register(self, adapter: Adapter) -> None:
        if adapter.name in self._adapters:
            raise ValueError(f"duplicate adapter: {adapter.name}")
        self._adapters[adapter.name] = adapter

    def get(self, name: str) -> Adapter:
        return self._adapters[name]

    def names(self) -> list[str]:
        return sorted(self._adapters)


def default_adapter_registry() -> AdapterRegistry:
    any_polygon = {"TRI", "HEX", "POLYGON", "MIXED"}
    registry = AdapterRegistry()
    registry.register(
        SchemaOnlyAdapter("mpas", "0.1", {"HEX", "POLYGON", "MIXED"}, "mpas_unstructured_mesh_contract")
    )
    registry.register(SchemaOnlyAdapter("fvcom", "0.1", {"TRI", "POLYGON", "MIXED"}, "fvcom_tri_mesh_contract"))
    registry.register(SchemaOnlyAdapter("colm2024", "0.1", any_polygon, "colm2024_land_ocean_coupling_contract"))
    registry.register(SchemaOnlyAdapter("colm20xx", "0.1", any_polygon, "colm20xx_mesh_coupling_contract"))
    registry.register(SchemaOnlyAdapter("generic_esmf", "0.1", any_polygon, "generic_esmf_mesh_contract"))
    return registry


_ADAPTER_CELL_COLUMNS = [
    "adapter_name",
    "cell_id",
    "cell_index",
    "cell_type",
    "center_lon",
    "center_lat",
    "area_m2",
    "surface_class",
    "hydro_class",
    "coast_class",
    "mesh_priority",
    "source_mesh_type",
    "geometry_ref",
    "vertex_count",
    "vertices_json",
    "source_fractions_json",
    "quality_flags_json",
]


def write_adapter_cell_table(adapter_name: str, cells: Iterable[CanonicalCell], output_dir: str | Path) -> Path:
    path = Path(output_dir) / f"adapter_{adapter_name}_cells.csv"
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=_ADAPTER_CELL_COLUMNS)
        writer.writeheader()
        for cell in cells:
            writer.writerow(_adapter_cell_row(adapter_name, cell))
    return path


def _adapter_cell_row(adapter_name: str, cell: CanonicalCell) -> dict[str, object]:
    return {
        "adapter_name": adapter_name,
        "cell_id": cell.cell_id,
        "cell_index": cell.cell_index,
        "cell_type": cell.cell_type,
        "center_lon": cell.center_lon,
        "center_lat": cell.center_lat,
        "area_m2": cell.area_m2,
        "surface_class": cell.surface_class,
        "hydro_class": cell.hydro_class,
        "coast_class": cell.coast_class,
        "mesh_priority": cell.mesh_priority,
        "source_mesh_type": cell.source_mesh_type,
        "geometry_ref": cell.geometry_ref,
        "vertex_count": len(cell.vertices),
        "vertices_json": json.dumps(cell.vertices, sort_keys=True),
        "source_fractions_json": json.dumps(cell.source_fractions, sort_keys=True),
        "quality_flags_json": json.dumps(cell.quality_flags, sort_keys=True),
    }
