from __future__ import annotations

import json
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class V3RunManifest:
    case_name: str
    recipe_hash: str
    component_versions: dict[str, str] = field(default_factory=dict)
    adapter_versions: dict[str, str] = field(default_factory=dict)
    input_sources: dict[str, str] = field(default_factory=dict)
    geometry_backend: str = "python_reference"
    mask_counts: dict[str, int] = field(default_factory=dict)
    cell_counts_by_class: dict[str, int] = field(default_factory=dict)
    missing_mask_count: int = 0
    cell_size_distribution: dict[str, float] = field(default_factory=dict)
    coupling_row_counts: dict[str, int] = field(default_factory=dict)
    qa_artifacts: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        payload = asdict(self)
        return json.loads(json.dumps(payload, sort_keys=True))

    def write_json(self, output_path: str | Path) -> Path:
        path = Path(output_path)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(self.to_dict(), indent=2, sort_keys=True) + "\n")
        return path
