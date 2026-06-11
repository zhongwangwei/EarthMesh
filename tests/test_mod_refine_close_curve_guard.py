from pathlib import Path


def test_bdy_refine_segment_guard_is_per_closed_curve():
    source = Path("src/MOD_refine.F90").read_text()
    start = source.index("SUBROUTINE bdy_refine_segment_make")
    end = source.index("END SUBROUTINE bdy_refine_segment_make", start)
    body = source[start:end]

    assert "if (num_sum /= n_close_curve(i)-1) then" in body
    assert "num_sum must same as n_close_curve(i)-1" in body
    assert "num_sum /= sum(n_close_curve)-1" not in body


def test_weak_concav_segment_does_not_stop_before_supported_unequal_segment_cases():
    source = Path("src/MOD_refine.F90").read_text()
    start = source.index("SUBROUTINE weak_concav_segment_make")
    end = source.index("END SUBROUTINE weak_concav_segment_make", start)
    body = source[start:end]

    assert 'STOP "ERROR! only 1+1 and n+n HERE!"' not in body


def test_weak_concav_segment_allows_sparse_accounting_without_overwriting_empty_slots():
    source = Path("src/MOD_refine.F90").read_text()
    start = source.index("SUBROUTINE weak_concav_segment_make")
    end = source.index("END SUBROUTINE weak_concav_segment_make", start)
    body = source[start:end]

    assert "intent(inout) :: num_ref_weak_concav" in body
    assert "weak_concav_capacity = max(num_ref_weak_concav, 2*num_bdy_refine_segment)" in body
    assert "weak concav accounting expands allocation" in body
    assert "weak concav accounting sparse" in body
    assert "pair_end = num_weak_concav_segment + num_weak_concav_pair" in body
    assert "num_weak_concav_segment+1:num_ref_weak_concav" not in body


def test_m1w1_child_lookup_reports_missing_adjacency_without_stopping():
    source = Path("src/MOD_refine.F90").read_text()
    start = source.index("SUBROUTINE m1w1_to_m11w11")
    end = source.index("END SUBROUTINE m1w1_to_m11w11", start)
    body = source[start:end]

    assert "logical, optional, intent(out) :: found" in body
    assert "if (present(found)) found = isexist" in body
    assert "WARNING! missing child adjacency in SUBROUTINE m1w1_to_m11w11" in body
    assert "if (m11 == 0 .or. w11 == 0) cycle" in body
    assert "stop \"ERROR! isexist .eqv. .false. in SUBROUTINE m1w1_to_m11w11\"" not in body

    weak_start = source.index("SUBROUTINE weak_concav_lop_judge")
    weak_end = source.index("END SUBROUTINE weak_concav_lop_judge", weak_start)
    weak_body = source[weak_start:weak_end]
    assert "CALL m1w1_to_m11w11(m1, w1, sjx_child, ngrmw_new, m11, w11, found_child)" in weak_body
    assert "if (.not. found_child) cycle" in weak_body

def test_final_polygon_triangle_adjacency_capacity_is_dynamic():
    source = Path("src/MOD_refine.F90").read_text()
    start = source.index("SUBROUTINE NGR_RENEW")
    end = source.index("END SUBROUTINE NGR_RENEW", start)
    body = source[start:end]

    assert "integer :: ngrwm_f_capacity" in body
    assert "ngrwm_f_capacity = max(7, maxval(n_ngrwm_f))" in body
    assert "WARNING! expanding final ngrwm_f adjacency capacity" in body
    assert "allocate(ngrwm_f(ngrwm_f_capacity, num_dbx))" in body
    assert "allocate(ngrwm_f(7, num_dbx))" not in body
