from __future__ import annotations

import json
from dataclasses import dataclass, replace
from pathlib import Path

from util.v3_core.adapters import AdapterExportPlan, AdapterRegistry, default_adapter_registry, write_adapter_cell_table
from util.v3_core.geometry import MaskFeature, OverlayResult, apply_overlay_to_cell, summarize_overlay_results
from util.v3_core.geometry_backend import GeometryBackend, get_geometry_backend
from util.v3_core.manifest import V3RunManifest
from util.v3_core.schema import CanonicalCell, validate_cell_collection


@dataclass(frozen=True)
class V3PipelineResult:
    cells: list[CanonicalCell]
    overlay_results: list[OverlayResult]
    overlay_summary: dict[str, object]
    adapter_plans: dict[str, AdapterExportPlan]
    manifest: V3RunManifest

    def write_sidecars(self, output_dir: str | Path) -> dict[str, Path]:
        directory = Path(output_dir)
        directory.mkdir(parents=True, exist_ok=True)
        paths = {"manifest": self.manifest.write_json(directory / "manifest.json")}
        paths["overlay_summary"] = _write_json_sidecar(self.overlay_summary, directory / "overlay_summary.json")
        for adapter_name, plan in self.adapter_plans.items():
            cell_table = write_adapter_cell_table(adapter_name, self.cells, directory)
            paths[f"adapter_{adapter_name}_cells"] = cell_table
            sidecar_plan = replace(
                plan,
                files={
                    **plan.files,
                    "cells": cell_table.name,
                    "manifest": "manifest.json",
                    "overlay_summary": "overlay_summary.json",
                },
            )
            paths[f"adapter_{adapter_name}"] = sidecar_plan.write_json(directory / f"adapter_{adapter_name}.json")
        return paths


def build_v3_pipeline_result(
    *,
    case_name: str,
    recipe_hash: str,
    cells: list[CanonicalCell],
    masks: list[MaskFeature],
    adapter_names: list[str],
    registry: AdapterRegistry | None = None,
    geometry_backend: GeometryBackend | None = None,
) -> V3PipelineResult:
    validated_cells = validate_cell_collection(cells)
    backend = geometry_backend or get_geometry_backend()
    adapter_registry = registry or default_adapter_registry()

    overlay_results = backend.overlay_cells(validated_cells, masks)
    overlay_by_cell_id = {result.cell_id: result for result in overlay_results}
    updated_cells = [apply_overlay_to_cell(cell, overlay_by_cell_id[cell.cell_id]) for cell in validated_cells]
    overlay_summary = summarize_overlay_results(overlay_results)

    adapter_plans = {name: adapter_registry.get(name).plan_export(updated_cells) for name in adapter_names}
    adapter_versions = {name: plan.adapter_version for name, plan in adapter_plans.items()}
    warnings = [warning for plan in adapter_plans.values() for warning in plan.warnings]

    manifest = V3RunManifest(
        case_name=case_name,
        recipe_hash=recipe_hash,
        adapter_versions=adapter_versions,
        mask_counts=dict(overlay_summary["winning_class_counts"]),
        cell_counts_by_class=_count_cell_types(updated_cells),
        missing_mask_count=int(overlay_summary["missing_mask_count"]),
        warnings=warnings,
    )

    return V3PipelineResult(
        cells=updated_cells,
        overlay_results=overlay_results,
        overlay_summary=overlay_summary,
        adapter_plans=adapter_plans,
        manifest=manifest,
    )


def _count_cell_types(cells: list[CanonicalCell]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for cell in cells:
        counts[cell.cell_type] = counts.get(cell.cell_type, 0) + 1
    return counts


def _write_json_sidecar(payload: dict[str, object], output_path: Path) -> Path:
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    return output_path
