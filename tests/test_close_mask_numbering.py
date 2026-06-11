from pathlib import Path


def test_fortran_close_mask_temp_numbering_uses_three_digits_consistently():
    area = Path("src/MOD_Area_judge.F90").read_text()
    mkgrd = Path("src/mkgrd.F90").read_text()
    refine = Path("src/MOD_refine.F90").read_text()

    area_block = area[area.index("SUBROUTINE IsInArea_close_Calculation"): area.index("END SUBROUTINE IsInArea_close_Calculation")]
    assert "write(numc, '(I3.3)') n" in area_block
    assert "write(numc, '(I2.2)') n" not in area_block

    close_block = mkgrd[mkgrd.index("subroutine close_mask_make"): mkgrd.index("end subroutine close_mask_make")]
    assert "write(numc, '(I3.3)') mask_domain_ndm" in close_block
    assert "write(numc, '(I3.3)') mask_refine_ndm(refine_degree)" in close_block
    assert "write(numc, '(I3.3)') mask_patch_ndm(refine_degree)" in close_block
    assert "write(numc, '(I2.2)')" not in close_block

    patch_line = "lndname = trim(file_dir)// 'tmpfile/mask_patch_close_'//trim(refinec)//'_'//trim(numc)//'.nc4'"
    patch_index = refine.index(patch_line)
    patch_context = refine[patch_index - 120: patch_index + len(patch_line)]
    assert "write(numc, '(I3.3)') i" in patch_context
