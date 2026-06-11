from __future__ import annotations

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Mapping


@dataclass(frozen=True)
class MeshRecipe:
    grid: str
    base_resolution: str
    kernel: str


@dataclass(frozen=True)
class RegionRecipe:
    bbox: tuple[float, float, float, float]


@dataclass(frozen=True)
class ComponentRecipe:
    type: str
    options: dict[str, Any] = field(default_factory=dict)


@dataclass(frozen=True)
class QARecipe:
    html: bool = True
    png: bool = True
    summary_json: bool = True


@dataclass(frozen=True)
class V3Recipe:
    case_name: str
    output_dir: Path
    mesh: MeshRecipe
    region: RegionRecipe
    components: list[ComponentRecipe]
    adapters: list[str]
    qa: QARecipe

    @classmethod
    def from_mapping(cls, mapping: Mapping[str, Any]) -> "V3Recipe":
        required = ["case", "mesh", "region", "components", "adapters", "qa"]
        for section in required:
            if section not in mapping:
                raise ValueError(f"missing required recipe section: {section}")

        case = _mapping(mapping["case"], "case")
        mesh = _mapping(mapping["mesh"], "mesh")
        region = _mapping(mapping["region"], "region")
        qa = _mapping(mapping["qa"], "qa")
        components_raw = mapping["components"]
        adapters_raw = mapping["adapters"]

        bbox_raw = region.get("bbox")
        if not isinstance(bbox_raw, list) or len(bbox_raw) != 4:
            raise ValueError("region.bbox must contain four numeric values")
        bbox = tuple(float(value) for value in bbox_raw)

        if not isinstance(components_raw, list):
            raise ValueError("components must be a list")
        components = [_component_from_mapping(item) for item in components_raw]

        if not isinstance(adapters_raw, list) or not all(isinstance(item, str) for item in adapters_raw):
            raise ValueError("adapters must be a list of strings")

        return cls(
            case_name=str(case["name"]),
            output_dir=Path(str(case["output_dir"])),
            mesh=MeshRecipe(
                grid=str(mesh["grid"]),
                base_resolution=str(mesh["base_resolution"]),
                kernel=str(mesh["kernel"]),
            ),
            region=RegionRecipe(bbox=bbox),
            components=components,
            adapters=list(adapters_raw),
            qa=QARecipe(
                html=bool(qa.get("html", True)),
                png=bool(qa.get("png", True)),
                summary_json=bool(qa.get("summary_json", True)),
            ),
        )


def _mapping(value: Any, name: str) -> Mapping[str, Any]:
    if not isinstance(value, Mapping):
        raise ValueError(f"{name} must be an object")
    return value


def _component_from_mapping(value: Any) -> ComponentRecipe:
    component = _mapping(value, "component")
    component_type = str(component["type"])
    options = {str(key): item for key, item in component.items() if key != "type"}
    return ComponentRecipe(type=component_type, options=options)


def load_recipe(path: str | Path) -> V3Recipe:
    recipe_path = Path(path)
    if recipe_path.suffix.lower() not in {".json"}:
        raise ValueError("Phase 1 recipe parser accepts JSON files; YAML can be added behind this same V3Recipe contract")
    return V3Recipe.from_mapping(json.loads(recipe_path.read_text()))
