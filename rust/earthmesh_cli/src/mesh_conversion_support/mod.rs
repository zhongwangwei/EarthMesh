mod mesh_connectivity;
mod netcdf_rows;
mod primitives;

pub(crate) use mesh_connectivity::{
    cells_on_triangle_one_based_from_mesh, n_edges_on_cell_usize_from_mesh,
    parse_value_after_equals, triangles_on_cell_one_based_from_mesh,
};
pub(crate) use netcdf_rows::{
    f64_matrix_width, flatten_i32_rows, i32_matrix_from_flat, matrix_width, one_to_n_i32,
    rows_from_flat_i32, usize_values_to_i32, write_f64_1d, write_f64_matrix_rows, write_i32_1d,
    write_i32_matrix_rows, write_i32_pair_rows,
};
pub(crate) use primitives::{
    i32_counts_as_usize, i32_rows_as_usize, lat_values, lon_values, lonlat_degrees_from_points,
    lonlat_pairs_from_points, lonlat_points_from_pairs, lookup_f64, m_to_w_as_usize_rows,
    normalize_degrees, patchtype_indices, rad_to_deg, require_len, rows_to_triangle_connectivity,
    scale_cartesian_points_by_earth_radius, split_cartesian_components,
    usize_from_i32_connectivity, usize_from_i32_nonnegative, usize_from_i32_positive,
    usize_rows_to_i32, usize_to_i32, validate_mask_postproc_layout,
};
