from pathlib import Path


def test_bdy_refine_segment_guard_is_per_closed_curve():
    source = Path("src/MOD_refine.F90").read_text()
    start = source.index("SUBROUTINE bdy_refine_segment_make")
    end = source.index("END SUBROUTINE bdy_refine_segment_make", start)
    body = source[start:end]

    assert "if (num_sum /= n_close_curve(i)-1) then" in body
    assert "num_sum must same as n_close_curve(i)-1" in body
    assert "num_sum /= sum(n_close_curve)-1" not in body
