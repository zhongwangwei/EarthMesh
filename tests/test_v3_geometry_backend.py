from util.v3_core.geometry import MaskFeature
from util.v3_core.geometry_backend import PythonGeometryBackend, get_geometry_backend
from util.v3_core.schema import CanonicalCell


def test_get_geometry_backend_returns_python_backend_by_default():
    backend = get_geometry_backend()

    assert isinstance(backend, PythonGeometryBackend)


def test_python_backend_overlays_cells_with_masks():
    backend = PythonGeometryBackend()
    cell = CanonicalCell.minimal("cell", cell_type="POLYGON")
    mask = MaskFeature("land", "LAND", 1, [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)])

    results = backend.overlay_cells([cell], [mask])

    assert len(results) == 1
    assert results[0].winning_class == "LAND"
    assert results[0].class_fractions == {"LAND": 1.0}
