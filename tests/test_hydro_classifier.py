from util.hydro_mesh.classifier import RiverReach, classify_reach


def test_estuary_is_explicit_2d_even_when_small():
    reach = RiverReach(
        reach_id="estuary-small",
        upstream_area_km2=800.0,
        width_m=50.0,
        floodplain_width_m=100.0,
        target_dx_km=10.0,
        is_estuary=True,
    )

    result = classify_reach(reach)

    assert result.river_class == "R3"
    assert "estuary" in result.reasons


def test_wide_reach_becomes_explicit_2d_relative_to_mesh_resolution():
    reach = RiverReach(
        reach_id="wide-mainstem",
        upstream_area_km2=2000.0,
        width_m=3000.0,
        floodplain_width_m=1500.0,
        target_dx_km=10.0,
    )

    result = classify_reach(reach)

    assert result.river_class == "R3"
    assert "effective_width_fraction" in result.reasons


def test_medium_reach_gets_1d_with_refinement_buffer():
    reach = RiverReach(
        reach_id="medium-tributary",
        upstream_area_km2=12000.0,
        width_m=200.0,
        floodplain_width_m=300.0,
        target_dx_km=10.0,
    )

    result = classify_reach(reach)

    assert result.river_class == "R2"
    assert "upstream_area_r2" in result.reasons


def test_small_reach_keeps_1d_topology_only():
    reach = RiverReach(
        reach_id="small-channel",
        upstream_area_km2=2000.0,
        width_m=50.0,
        floodplain_width_m=80.0,
        target_dx_km=10.0,
    )

    result = classify_reach(reach)

    assert result.river_class == "R1"
    assert "upstream_area_r1" in result.reasons


def test_tiny_reach_is_aggregated():
    reach = RiverReach(
        reach_id="tiny-channel",
        upstream_area_km2=200.0,
        width_m=20.0,
        floodplain_width_m=20.0,
        target_dx_km=10.0,
    )

    result = classify_reach(reach)

    assert result.river_class == "R0"
    assert result.reasons == ["below_explicit_thresholds"]
