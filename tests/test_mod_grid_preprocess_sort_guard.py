from pathlib import Path


def test_get_sort_new_starts_open_adjacency_walk_from_endpoint_and_does_not_stop():
    source = Path("src/MOD_grid_preprocess.F90").read_text()
    start = source.index("SUBROUTINE GetSortNew")
    end = source.index("END SUBROUTINE GetSortNew", start)
    body = source[start:end]

    assert "integer, allocatable :: neighbor_degree(:)" in body
    assert "if (neighbor_degree(j) == 1) then" in body
    assert "start_pos = j" in body
    assert "WARNING! incomplete adjacency walk in SUBROUTINE GetSortNew" in body
    assert 'STOP "ERROR! this do-loop must exit when find we want in SUBROUTINE GetSortNew"' not in body
