use crate::{CartesianPoint, MethodCRefinementRegion};

pub(crate) fn method_c_region_contains_method_c(
    region: &MethodCRefinementRegion,
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

pub(crate) fn refine_regions_contain_method_c(
    regions: &[MethodCRefinementRegion],
    point: CartesianPoint,
    radius: f64,
    use_cartesian_xy: bool,
) -> bool {
    regions
        .iter()
        .any(|region| method_c_region_contains_method_c(region, point, radius, use_cartesian_xy))
}

pub(crate) fn method_c_region_close_to_method_c(
    region: &MethodCRefinementRegion,
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

pub(crate) fn refine_regions_close_to_method_c(
    regions: &[MethodCRefinementRegion],
    point: CartesianPoint,
    radius: f64,
    use_cartesian_xy: bool,
) -> bool {
    regions
        .iter()
        .any(|region| method_c_region_close_to_method_c(region, point, radius, use_cartesian_xy))
}
