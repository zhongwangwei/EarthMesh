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
        "component_roles",
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


def write_adapter_model_artifacts(
    adapter_name: str,
    cells: Iterable[CanonicalCell],
    output_dir: str | Path,
) -> dict[str, Path]:
    materialized = list(cells)
    if adapter_name == "mpas":
        return {"mesh": _write_mpas_mesh_netcdf(materialized, Path(output_dir) / "adapter_mpas_mesh.nc")}
    if adapter_name == "fvcom":
        return {"mesh": _write_fvcom_mesh_dat(materialized, Path(output_dir) / "adapter_fvcom_mesh.dat")}
    if adapter_name == "colm20xx":
        return {"exchange": _write_colm20xx_exchange_netcdf(materialized, Path(output_dir) / "adapter_colm20xx_exchange.nc")}
    return {}


def write_adapter_mesh_artifact(adapter_name: str, cells: Iterable[CanonicalCell], output_dir: str | Path) -> Path | None:
    return write_adapter_model_artifacts(adapter_name, cells, output_dir).get("mesh")


def write_adapter_bundle_manifest(
    adapter_name: str,
    plan: AdapterExportPlan,
    files: dict[str, str],
    output_dir: str | Path,
) -> Path:
    """Write a machine-readable bundle contract for an adapter handoff.

    The bundle is the stable boundary between EarthMesh v3 and downstream model
    adapters: it groups the canonical cell table, run manifest, overlay summary,
    and any model-named artifacts with explicit roles and limitations.
    """

    path = Path(output_dir) / f"adapter_{adapter_name}_bundle.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = {
        "kind": "earthmesh_v3_adapter_bundle",
        "adapter_name": adapter_name,
        "adapter_version": plan.adapter_version,
        "output_format": plan.output_format,
        "readiness_level": _adapter_readiness_level(adapter_name),
        "supported_cell_types": plan.supported_cell_types,
        "required_fields": plan.required_fields,
        "cell_type_counts": plan.cell_type_counts,
        "warnings": plan.warnings,
        "files": dict(sorted(files.items())),
        "artifact_roles": _adapter_artifact_roles(adapter_name, files),
        "limitations": _adapter_limitations(adapter_name),
    }
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    return path


def _adapter_readiness_level(adapter_name: str) -> str:
    if adapter_name == "colm20xx":
        return "exchange_schema_contract"
    if adapter_name in {"mpas", "fvcom"}:
        return "model_handoff_contract"
    if adapter_name.startswith("colm"):
        return "coupling_table_contract"
    return "schema_contract"


def _adapter_artifact_roles(adapter_name: str, files: dict[str, str]) -> dict[str, str]:
    roles = {
        "cells": "canonical_adapter_cell_table_csv",
        "manifest": "earthmesh_v3_run_manifest_json",
        "overlay_summary": "mask_overlay_summary_json",
    }
    if adapter_name == "mpas" and "mesh" in files:
        roles["mesh"] = "mpas_unstructured_mesh_netcdf"
    elif adapter_name == "fvcom" and "mesh" in files:
        roles["mesh"] = "fvcom_unstructured_mesh_dat"
    elif adapter_name == "colm20xx" and "exchange" in files:
        roles["exchange"] = "colm20xx_land_ocean_river_exchange_netcdf"
    return {key: roles[key] for key in sorted(roles) if key in files}


def _adapter_limitations(adapter_name: str) -> list[str]:
    common = ["runtime ingestion is not validated here"]
    if adapter_name == "mpas":
        return [*common, "MPAS artifact is an EarthMesh handoff NetCDF, not a complete MPAS init stream"]
    if adapter_name == "fvcom":
        return [*common, "FVCOM artifact preserves EarthMesh cells and vertices, not a finalized FVCOM control deck"]
    if adapter_name == "colm20xx":
        return [*common, "CoLM20XX exchange fields are reserved schema fields pending final model-side naming"]
    if adapter_name == "colm2024":
        return [*common, "CoLM2024 handoff is a coupling table contract, not forcing or restart data"]
    return common


def _write_mpas_mesh_netcdf(cells: list[CanonicalCell], output_path: Path) -> Path:
    try:
        import netCDF4
    except ImportError as exc:  # pragma: no cover - dependency is available in test/runtime env
        raise RuntimeError("MPAS adapter mesh artifact requires netCDF4") from exc
    output_path.parent.mkdir(parents=True, exist_ok=True)
    max_vertices = max((len(cell.vertices) for cell in cells), default=0)
    with netCDF4.Dataset(output_path, "w", format="NETCDF4") as ds:
        ds.createDimension("cell", len(cells))
        ds.createDimension("vertex", max_vertices)
        ds.setncattr("kind", "earthmesh_v3_adapter_mesh_metadata")
        ds.setncattr("adapter_name", "mpas")
        ds.setncattr("cell_type_code_meanings", "0=UNKNOWN 1=TRI 2=HEX 3=POLYGON 4=MIXED")
        ds.setncattr("surface_class_code_meanings", "0=UNKNOWN 1=LAND 2=OCEAN 3=COAST")
        ds.setncattr("hydro_class_code_meanings", "0=NONE 2=R2 3=R3")
        cell_index = ds.createVariable("cell_index", "i4", ("cell",))
        center_lon = ds.createVariable("center_lon", "f8", ("cell",))
        center_lat = ds.createVariable("center_lat", "f8", ("cell",))
        area_m2 = ds.createVariable("area_m2", "f8", ("cell",))
        vertex_count = ds.createVariable("vertex_count", "i4", ("cell",))
        cell_type_code = ds.createVariable("cell_type_code", "i1", ("cell",))
        surface_class_code = ds.createVariable("surface_class_code", "i1", ("cell",))
        hydro_class_code = ds.createVariable("hydro_class_code", "i1", ("cell",))
        coast_class_code = ds.createVariable("coast_class_code", "i1", ("cell",))
        vertex_lon = ds.createVariable("vertex_lon", "f8", ("cell", "vertex"), fill_value=float("nan"))
        vertex_lat = ds.createVariable("vertex_lat", "f8", ("cell", "vertex"), fill_value=float("nan"))
        center_lon.units = "degrees_east"
        center_lat.units = "degrees_north"
        vertex_lon.units = "degrees_east"
        vertex_lat.units = "degrees_north"
        area_m2.units = "m2"
        for i, cell in enumerate(cells):
            cell_index[i] = cell.cell_index
            center_lon[i] = cell.center_lon
            center_lat[i] = cell.center_lat
            area_m2[i] = cell.area_m2
            vertex_count[i] = len(cell.vertices)
            cell_type_code[i] = _CELL_TYPE_CODES.get(cell.cell_type, 0)
            surface_class_code[i] = _SURFACE_CLASS_CODES.get(cell.surface_class, 0)
            hydro_class_code[i] = _HYDRO_CLASS_CODES.get(cell.hydro_class, 0)
            coast_class_code[i] = _COAST_CLASS_CODES.get(cell.coast_class, 0)
            for j, (lon, lat) in enumerate(cell.vertices):
                vertex_lon[i, j] = lon
                vertex_lat[i, j] = lat
    return output_path


def _write_fvcom_mesh_dat(cells: list[CanonicalCell], output_path: Path) -> Path:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with output_path.open("w") as stream:
        stream.write("# EarthMesh v3 FVCOM mesh metadata\n")
        stream.write("# cell_id cell_index cell_type center_lon center_lat area_m2 vertex_count surface_class hydro_class coast_class vertices\n")
        for cell in cells:
            vertices = ";".join(f"{lon},{lat}" for lon, lat in cell.vertices)
            stream.write(
                f"{cell.cell_id} {cell.cell_index} {cell.cell_type} {cell.center_lon} {cell.center_lat} "
                f"{cell.area_m2} {len(cell.vertices)} {cell.surface_class} {cell.hydro_class} {cell.coast_class} {vertices}\n"
            )
    return output_path


def _write_colm20xx_exchange_netcdf(cells: list[CanonicalCell], output_path: Path) -> Path:
    try:
        import netCDF4
    except ImportError as exc:  # pragma: no cover - dependency is available in test/runtime env
        raise RuntimeError("CoLM20XX exchange artifact requires netCDF4") from exc
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with netCDF4.Dataset(output_path, "w", format="NETCDF4") as ds:
        ds.createDimension("cell", len(cells))
        ds.setncattr("kind", "earthmesh_colm20xx_exchange_netcdf")
        ds.setncattr("adapter_name", "colm20xx")
        ds.setncattr("schema_version", "0.1")
        ds.setncattr("surface_class_code_meanings", "0=UNKNOWN 1=LAND 2=OCEAN 3=COAST 4=LAKE 5=ICE 6=WETLAND")
        ds.setncattr("hydro_class_code_meanings", "0=none/R0 1=R1 2=R2 3=R3 4=ESTUARY 5=DELTA")
        ds.setncattr("coast_class_code_meanings", "0=none 1=COAST 2=COAST_LAND 3=COAST_OCEAN 4=ESTUARY 5=DELTA 6=TIDAL_FLAT 7=SHELF")

        cell_id = ds.createVariable("cell_id", str, ("cell",))
        cell_index = ds.createVariable("cell_index", "i4", ("cell",))
        center_lon = ds.createVariable("center_lon", "f8", ("cell",))
        center_lat = ds.createVariable("center_lat", "f8", ("cell",))
        area_m2 = ds.createVariable("area_m2", "f8", ("cell",))
        surface_class_code = ds.createVariable("surface_class_code", "i1", ("cell",))
        hydro_class_code = ds.createVariable("hydro_class_code", "i1", ("cell",))
        coast_class_code = ds.createVariable("coast_class_code", "i1", ("cell",))
        land_fraction = ds.createVariable("land_fraction", "f8", ("cell",))
        ocean_fraction = ds.createVariable("ocean_fraction", "f8", ("cell",))
        river_fraction = ds.createVariable("river_fraction", "f8", ("cell",))
        coastal_fraction = ds.createVariable("coastal_fraction", "f8", ("cell",))
        supports_land_ocean_exchange = ds.createVariable("supports_land_ocean_exchange", "i1", ("cell",))
        supports_river_land_exchange = ds.createVariable("supports_river_land_exchange", "i1", ("cell",))
        supports_river_ocean_exchange = ds.createVariable("supports_river_ocean_exchange", "i1", ("cell",))
        supports_land_atmos_exchange = ds.createVariable("supports_land_atmos_exchange", "i1", ("cell",))
        supports_ocean_atmos_exchange = ds.createVariable("supports_ocean_atmos_exchange", "i1", ("cell",))

        center_lon.units = "degrees_east"
        center_lat.units = "degrees_north"
        area_m2.units = "m2"
        land_fraction.units = "1"
        ocean_fraction.units = "1"
        river_fraction.units = "1"
        coastal_fraction.units = "1"

        for index, cell in enumerate(cells):
            fractions = _colm20xx_fractions(cell)
            cell_id[index] = cell.cell_id
            cell_index[index] = cell.cell_index
            center_lon[index] = cell.center_lon
            center_lat[index] = cell.center_lat
            area_m2[index] = cell.area_m2
            surface_class_code[index] = _SURFACE_CLASS_CODES.get(cell.surface_class, 0)
            hydro_class_code[index] = _HYDRO_CLASS_CODES.get(cell.hydro_class, 0)
            coast_class_code[index] = _COAST_CLASS_CODES.get(cell.coast_class, 0)
            land_fraction[index] = fractions["land"]
            ocean_fraction[index] = fractions["ocean"]
            river_fraction[index] = fractions["river"]
            coastal_fraction[index] = fractions["coast"]
            supports_land_ocean_exchange[index] = 1 if fractions["land"] > 0.0 and fractions["ocean"] > 0.0 else 0
            supports_river_land_exchange[index] = 1 if fractions["river"] > 0.0 and fractions["land"] > 0.0 else 0
            supports_river_ocean_exchange[index] = 1 if fractions["river"] > 0.0 and fractions["ocean"] > 0.0 else 0
            supports_land_atmos_exchange[index] = 1 if fractions["land"] > 0.0 else 0
            supports_ocean_atmos_exchange[index] = 1 if fractions["ocean"] > 0.0 else 0
    return output_path


def _colm20xx_fractions(cell: CanonicalCell) -> dict[str, float]:
    land = float(cell.source_fractions.get("LAND", 1.0 if cell.surface_class == "LAND" else 0.0))
    ocean = float(cell.source_fractions.get("OCEAN", 1.0 if cell.surface_class == "OCEAN" else 0.0))
    river = sum(float(cell.source_fractions.get(name, 0.0)) for name in ("R1", "R2", "R3", "ESTUARY", "DELTA"))
    if river == 0.0 and cell.hydro_class not in {"NONE", "R0"}:
        river = 1.0
    coast = float(cell.source_fractions.get("COAST", 0.0))
    if coast == 0.0 and (cell.surface_class == "COAST" or cell.coast_class != "NONE"):
        coast = min(1.0, land + ocean) if (land + ocean) > 0.0 else 1.0
    return {"land": land, "ocean": ocean, "river": river, "coast": coast}


_CELL_TYPE_CODES = {"UNKNOWN": 0, "TRI": 1, "HEX": 2, "POLYGON": 3, "MIXED": 4}
_SURFACE_CLASS_CODES = {"UNKNOWN": 0, "LAND": 1, "OCEAN": 2, "COAST": 3, "LAKE": 4, "ICE": 5, "WETLAND": 6}
_HYDRO_CLASS_CODES = {"NONE": 0, "R0": 0, "R1": 1, "R2": 2, "R3": 3, "ESTUARY": 4, "DELTA": 5}
_COAST_CLASS_CODES = {"NONE": 0, "COAST": 1, "COAST_LAND": 2, "COAST_OCEAN": 3, "ESTUARY": 4, "DELTA": 5, "TIDAL_FLAT": 6, "SHELF": 7}


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
