import json

from util.v3_core.geometry import MaskFeature, OverlayResult
from util.v3_core.pipeline import build_v3_pipeline_result
from util.v3_core.schema import CanonicalCell


def test_pipeline_applies_masks_to_cells_and_builds_manifest():
    cells = [
        CanonicalCell.minimal("land", cell_type="TRI"),
        CanonicalCell(
            cell_id="river",
            cell_index=1,
            cell_type="HEX",
            center_lon=1.0,
            center_lat=1.0,
            area_m2=4.0,
            vertices=[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)],
        ),
        CanonicalCell(
            cell_id="unknown",
            cell_index=2,
            cell_type="POLYGON",
            center_lon=10.0,
            center_lat=10.0,
            area_m2=1.0,
            vertices=[(10.0, 10.0), (11.0, 10.0), (10.0, 11.0)],
        ),
    ]
    masks = [
        MaskFeature("land-mask", "LAND", 1, [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]),
        MaskFeature("river-mask", "R3", 30, [(1.0, 0.0), (2.0, 0.0), (2.0, 2.0), (1.0, 2.0)]),
    ]

    result = build_v3_pipeline_result(
        case_name="unit_case",
        recipe_hash="abc123",
        cells=cells,
        masks=masks,
        adapter_names=["colm2024", "mpas"],
    )

    assert [cell.surface_class for cell in result.cells] == ["LAND", "UNKNOWN", "UNKNOWN"]
    assert [cell.hydro_class for cell in result.cells] == ["NONE", "R3", "NONE"]
    assert result.overlay_summary["missing_mask_count"] == 1
    assert result.manifest.case_name == "unit_case"
    assert result.manifest.mask_counts == {"LAND": 1, "R3": 1, "UNKNOWN": 1}
    assert result.manifest.cell_counts_by_class == {"TRI": 1, "HEX": 1, "POLYGON": 1}
    assert result.manifest.missing_mask_count == 1
    assert sorted(result.adapter_plans) == ["colm2024", "mpas"]
    assert result.adapter_plans["mpas"].warnings == ["mpas does not support cell_type=TRI for cell_id=land"]
    assert result.manifest.adapter_versions == {"colm2024": "0.1", "mpas": "0.1"}
    assert result.manifest.warnings == ["mpas does not support cell_type=TRI for cell_id=land"]


def test_pipeline_result_writes_manifest_and_adapter_sidecars(tmp_path):
    result = build_v3_pipeline_result(
        case_name="sidecar_case",
        recipe_hash="abc123",
        cells=[CanonicalCell.minimal("land", cell_type="TRI")],
        masks=[MaskFeature("land-mask", "LAND", 1, [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)])],
        adapter_names=["colm2024"],
    )

    paths = result.write_sidecars(tmp_path)

    assert sorted(paths) == ["adapter_colm2024", "adapter_colm2024_cells", "manifest", "overlay_summary"]
    manifest_payload = json.loads(paths["manifest"].read_text())
    adapter_payload = json.loads(paths["adapter_colm2024"].read_text())
    overlay_payload = json.loads(paths["overlay_summary"].read_text())
    assert manifest_payload["case_name"] == "sidecar_case"
    assert adapter_payload["adapter_name"] == "colm2024"
    assert adapter_payload["files"] == {
        "cells": "adapter_colm2024_cells.csv",
        "manifest": "manifest.json",
        "overlay_summary": "overlay_summary.json",
    }
    assert paths["adapter_colm2024_cells"].read_text().splitlines()[0].startswith("adapter_name,cell_id")
    assert overlay_payload["winning_class_counts"] == {"LAND": 1}
    assert overlay_payload["missing_mask_count"] == 0


def test_pipeline_records_effective_geometry_backend_name():
    class FixtureBackend:
        name = "fixture_backend"

        def overlay_cells(self, cells, masks):
            return [
                OverlayResult(
                    cell_id=cells[0].cell_id,
                    winning_class="LAND",
                    winning_priority=1,
                    class_fractions={"LAND": 1.0},
                    source_feature_ids=["fixture-mask"],
                    quality_flags=[],
                )
            ]

    result = build_v3_pipeline_result(
        case_name="backend_case",
        recipe_hash="abc123",
        cells=[CanonicalCell.minimal("land", cell_type="TRI")],
        masks=[],
        adapter_names=["colm2024"],
        geometry_backend=FixtureBackend(),
    )

    assert result.overlay_summary["geometry_backend"] == "fixture_backend"


def test_pipeline_sidecars_include_mpas_and_fvcom_mesh_artifacts(tmp_path):
    result = build_v3_pipeline_result(
        case_name="adapter_mesh_case",
        recipe_hash="abc123",
        cells=[
            CanonicalCell(
                cell_id="poly",
                cell_index=1,
                cell_type="POLYGON",
                center_lon=120.0,
                center_lat=30.0,
                area_m2=12.0,
                vertices=[(119.9, 29.9), (120.1, 29.9), (120.0, 30.1)],
            )
        ],
        masks=[MaskFeature("ocean", "OCEAN", 1, [(119.8, 29.8), (120.2, 29.8), (120.0, 30.2)])],
        adapter_names=["mpas", "fvcom"],
    )

    paths = result.write_sidecars(tmp_path)

    assert paths["adapter_mpas_mesh"].name == "adapter_mpas_mesh.nc"
    assert paths["adapter_fvcom_mesh"].name == "adapter_fvcom_mesh.dat"
    mpas_payload = json.loads(paths["adapter_mpas"].read_text())
    fvcom_payload = json.loads(paths["adapter_fvcom"].read_text())
    assert mpas_payload["files"]["mesh"] == "adapter_mpas_mesh.nc"
    assert fvcom_payload["files"]["mesh"] == "adapter_fvcom_mesh.dat"


def test_pipeline_sidecars_include_colm20xx_exchange_artifact(tmp_path):
    result = build_v3_pipeline_result(
        case_name="colm20xx_exchange_case",
        recipe_hash="abc123",
        cells=[
            CanonicalCell(
                cell_id="delta-cell",
                cell_index=9,
                cell_type="POLYGON",
                center_lon=121.0,
                center_lat=31.0,
                area_m2=250.0,
                vertices=[(120.9, 30.9), (121.1, 30.9), (121.1, 31.1), (120.9, 31.1)],
                surface_class="COAST",
                hydro_class="R3",
                coast_class="DELTA",
                component_roles=["colm_land", "colm_ocean", "cama_river", "exchange_cell"],
                source_fractions={"LAND": 0.4, "OCEAN": 0.5, "R3": 0.1},
            )
        ],
        masks=[],
        adapter_names=["colm20xx"],
    )

    paths = result.write_sidecars(tmp_path)

    assert paths["adapter_colm20xx_exchange"].name == "adapter_colm20xx_exchange.nc"
    payload = json.loads(paths["adapter_colm20xx"].read_text())
    assert payload["files"]["exchange"] == "adapter_colm20xx_exchange.nc"
