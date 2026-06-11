import json

from util.v3_core.geometry import MaskFeature
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

    assert sorted(paths) == ["adapter_colm2024", "manifest"]
    manifest_payload = json.loads(paths["manifest"].read_text())
    adapter_payload = json.loads(paths["adapter_colm2024"].read_text())
    assert manifest_payload["case_name"] == "sidecar_case"
    assert adapter_payload["adapter_name"] == "colm2024"
