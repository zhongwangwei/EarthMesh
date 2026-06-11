from pathlib import Path

from util.v3_core.components import ComponentProduct, ComponentResult, ComponentRunContext


def test_component_run_context_carries_case_paths_and_dry_run_flag(tmp_path):
    context = ComponentRunContext(
        case_name="gba_hydro",
        output_dir=tmp_path / "out",
        work_dir=tmp_path / "work",
        dry_run=True,
    )

    assert context.case_name == "gba_hydro"
    assert context.output_dir == Path(tmp_path / "out")
    assert context.work_dir == Path(tmp_path / "work")
    assert context.dry_run is True


def test_component_result_lists_products_and_warnings():
    product = ComponentProduct(
        layer_name="hydro_reaches",
        semantic_type="hydro",
        path=Path("hydro/reaches.jsonl"),
        description="Classified CaMa reaches",
    )
    result = ComponentResult(
        component_name="hydro_cama",
        products=[product],
        warnings=["width_source=fallback_width_bin"],
    )

    assert result.product_names == ["hydro_reaches"]
    assert result.has_warnings is True
