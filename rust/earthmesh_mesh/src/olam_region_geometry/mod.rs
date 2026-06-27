use crate::{CartesianPoint, OlamRefinementRegion};

pub(crate) fn olam_region_contains_method_c(
    region: &OlamRefinementRegion,
    point: CartesianPoint,
    radius: f64,
    use_cartesian_xy: bool,
) -> bool {
    if use_cartesian_xy {
        region.contains_cartesian_xy(point)
    } else {
        region.contains_cartesian(point, radius)
    }
}

pub(crate) fn olam_regions_contain_method_c(
    regions: &[OlamRefinementRegion],
    point: CartesianPoint,
    radius: f64,
    use_cartesian_xy: bool,
) -> bool {
    regions
        .iter()
        .any(|region| olam_region_contains_method_c(region, point, radius, use_cartesian_xy))
}

pub(crate) fn olam_region_close_to_method_c(
    region: &OlamRefinementRegion,
    point: CartesianPoint,
    radius: f64,
    use_cartesian_xy: bool,
) -> bool {
    if use_cartesian_xy {
        region.close_to_cartesian_xy(point)
    } else {
        region.close_to_cartesian(point, radius)
    }
}

pub(crate) fn olam_regions_close_to_method_c(
    regions: &[OlamRefinementRegion],
    point: CartesianPoint,
    radius: f64,
    use_cartesian_xy: bool,
) -> bool {
    regions
        .iter()
        .any(|region| olam_region_close_to_method_c(region, point, radius, use_cartesian_xy))
}
