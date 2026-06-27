pub(crate) fn olam_calculated_region_level(
    mask_refine_degree: usize,
    max_level: usize,
) -> Option<usize> {
    if mask_refine_degree == 0 {
        Some(max_level)
    } else if mask_refine_degree <= max_level {
        Some(mask_refine_degree)
    } else {
        None
    }
}
