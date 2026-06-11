from __future__ import annotations

from dataclasses import dataclass
from typing import Iterable, Protocol

from util.v3_core.schema import CanonicalCell


class Adapter(Protocol):
    name: str
    version: str
    supported_cell_types: set[str]

    def validate_cells(self, cells: Iterable[CanonicalCell]) -> list[str]:
        ...


@dataclass(frozen=True)
class SchemaOnlyAdapter:
    name: str
    version: str
    supported_cell_types: set[str]

    def validate_cells(self, cells: Iterable[CanonicalCell]) -> list[str]:
        warnings: list[str] = []
        for cell in cells:
            if cell.cell_type not in self.supported_cell_types:
                warnings.append(f"{self.name} does not support cell_type={cell.cell_type} for cell_id={cell.cell_id}")
        return warnings


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
    registry.register(SchemaOnlyAdapter("mpas", "0.1", {"HEX", "POLYGON", "MIXED"}))
    registry.register(SchemaOnlyAdapter("fvcom", "0.1", {"TRI", "POLYGON", "MIXED"}))
    registry.register(SchemaOnlyAdapter("colm2024", "0.1", any_polygon))
    registry.register(SchemaOnlyAdapter("colm20xx", "0.1", any_polygon))
    registry.register(SchemaOnlyAdapter("generic_esmf", "0.1", any_polygon))
    return registry
