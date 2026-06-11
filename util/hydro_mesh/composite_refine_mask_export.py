from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Mapping, Sequence

from util.hydro_mesh.refine_mask_export import (
    CloseMaskSpec,
    DEFAULT_CLASS_REFINE,
    geojson_to_close_mask_specs,
    write_close_mask_specs,
)


@dataclass(frozen=True)
class CompositeCloseMaskSummary:
    output_prefix: str
    paths: list[Path]
    counts_by_component: dict[str, int]
    counts_by_class_degree: dict[str, int]
    components: list[dict[str, object]]

    def to_json_dict(self) -> dict[str, object]:
        return {
            "kind": "earthmesh_composite_close_mask_summary",
            "output_prefix": self.output_prefix,
            "files_written": len(self.paths),
            "files": [str(path) for path in self.paths],
            "counts_by_component": self.counts_by_component,
            "counts_by_class_degree": self.counts_by_class_degree,
            "components": self.components,
        }


def _as_int_mapping(value: object, *, default: Mapping[str, int] | None = None) -> dict[str, int]:
    if value is None:
        return dict(default or {})
    if not isinstance(value, Mapping):
        raise ValueError("expected an object mapping class names to integer values")
    return {str(key): int(count) for key, count in value.items()}


def _as_degree_float_mapping(value: object) -> dict[int, float]:
    if value is None:
        return {}
    if not isinstance(value, Mapping):
        raise ValueError("expected an object mapping refinement degrees to buffer values")
    return {int(degree): float(buffer) for degree, buffer in value.items()}


def _component_specs(component: Mapping[str, object]) -> list[CloseMaskSpec]:
    input_geojson = component.get("input_geojson")
    if not input_geojson:
        raise ValueError("each component requires input_geojson")
    collection = json.loads(Path(str(input_geojson)).read_text())
    return geojson_to_close_mask_specs(
        collection,
        class_refine=_as_int_mapping(component.get("class_refine"), default=DEFAULT_CLASS_REFINE),
        simplify_tolerance_deg=float(component.get("simplify_tolerance_deg", 0.0)),
        max_rings_per_class=(
            int(component["max_rings_per_class"]) if component.get("max_rings_per_class") is not None else None
        ),
        max_rings_by_class=_as_int_mapping(component.get("max_rings_by_class")),
        max_masks_per_refine_degree=None,
        cumulative_refine=bool(component.get("cumulative_refine", True)),
        buffer_deg=float(component.get("buffer_deg", 0.0)),
        buffer_deg_by_refine_degree=_as_degree_float_mapping(component.get("buffer_deg_by_refine_degree")),
    )


def _sort_key(item: tuple[str, CloseMaskSpec]) -> tuple[object, ...]:
    component_name, spec = item
    return (
        spec.river_class,
        spec.refine_degree,
        component_name,
        spec.source_feature_index,
        spec.ring_index,
    )


def _apply_refine_degree_cap(
    items: Sequence[tuple[str, CloseMaskSpec]],
    *,
    max_masks_per_refine_degree: int | None,
) -> list[tuple[str, CloseMaskSpec]]:
    if max_masks_per_refine_degree is None:
        return list(items)
    counts: dict[int, int] = {}
    kept: list[tuple[str, CloseMaskSpec]] = []
    for item in sorted(
        items,
        key=lambda entry: (
            entry[1].refine_degree,
            -entry[1].target_refine_degree,
            entry[1].river_class,
            entry[0],
            entry[1].source_feature_index,
            entry[1].ring_index,
        ),
    ):
        spec = item[1]
        if counts.get(spec.refine_degree, 0) >= max_masks_per_refine_degree:
            continue
        kept.append(item)
        counts[spec.refine_degree] = counts.get(spec.refine_degree, 0) + 1
    return kept


def write_composite_close_mask_nmls(
    recipe: Mapping[str, object],
    output_prefix: str | Path,
) -> CompositeCloseMaskSummary:
    components = recipe.get("components")
    if not isinstance(components, list) or not components:
        raise ValueError("composite close-mask recipe requires a non-empty components list")

    tagged_specs: list[tuple[str, CloseMaskSpec]] = []
    component_summaries: list[dict[str, object]] = []
    for index, component in enumerate(components):
        if not isinstance(component, Mapping):
            raise ValueError("each component must be an object")
        component_name = str(component.get("name") or f"component_{index + 1}")
        specs = _component_specs(component)
        tagged_specs.extend((component_name, spec) for spec in specs)
        component_summaries.append(
            {
                "name": component_name,
                "input_geojson": str(component.get("input_geojson")),
                "files_selected": len(specs),
                "class_refine": _as_int_mapping(component.get("class_refine"), default=DEFAULT_CLASS_REFINE),
                "max_rings_by_class": _as_int_mapping(component.get("max_rings_by_class")),
                "max_rings_per_class": component.get("max_rings_per_class"),
            }
        )

    capped_specs = _apply_refine_degree_cap(
        tagged_specs,
        max_masks_per_refine_degree=(
            int(recipe["max_masks_per_refine_degree"])
            if recipe.get("max_masks_per_refine_degree") is not None
            else 999
        ),
    )
    capped_specs = sorted(capped_specs, key=_sort_key)
    paths = write_close_mask_specs([spec for _, spec in capped_specs], output_prefix)

    counts_by_component: dict[str, int] = {}
    counts_by_class_degree: dict[str, int] = {}
    for component_name, spec in capped_specs:
        counts_by_component[component_name] = counts_by_component.get(component_name, 0) + 1
        key = f"{spec.river_class}_d{spec.refine_degree}"
        counts_by_class_degree[key] = counts_by_class_degree.get(key, 0) + 1

    return CompositeCloseMaskSummary(
        output_prefix=str(output_prefix),
        paths=paths,
        counts_by_component=dict(sorted(counts_by_component.items())),
        counts_by_class_degree=dict(sorted(counts_by_class_degree.items())),
        components=component_summaries,
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Compose river and coast GeoJSON sources into one EarthMesh close-refinement mask set."
    )
    parser.add_argument("recipe_json", help="Composite close-mask recipe JSON")
    parser.add_argument("output_prefix", help="Output file prefix used by RL%mask_refine_spc_fprefix")
    parser.add_argument("--summary-json", help="Optional path for machine-readable summary JSON")
    args = parser.parse_args(argv)

    recipe = json.loads(Path(args.recipe_json).read_text())
    summary = write_composite_close_mask_nmls(recipe, args.output_prefix)
    summary_json = json.dumps(summary.to_json_dict(), indent=2, sort_keys=True) + "\n"
    if args.summary_json:
        summary_path = Path(args.summary_json)
        summary_path.parent.mkdir(parents=True, exist_ok=True)
        summary_path.write_text(summary_json)
    print(summary_json, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
