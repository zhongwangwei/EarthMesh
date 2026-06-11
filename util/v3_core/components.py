from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path


@dataclass(frozen=True)
class ComponentRunContext:
    case_name: str
    output_dir: Path
    work_dir: Path
    dry_run: bool = True


@dataclass(frozen=True)
class ComponentProduct:
    layer_name: str
    semantic_type: str
    path: Path
    description: str


@dataclass(frozen=True)
class ComponentResult:
    component_name: str
    products: list[ComponentProduct] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)

    @property
    def product_names(self) -> list[str]:
        return [product.layer_name for product in self.products]

    @property
    def has_warnings(self) -> bool:
        return bool(self.warnings)
