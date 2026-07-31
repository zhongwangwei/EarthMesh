mod geometry;
mod json;
mod writer;

pub(crate) use geometry::geometry_outer_rings;
pub use geometry::read_polygon_outer_rings;
pub(crate) use geometry::{LocalEqualArea, SphericalCap};
pub(crate) use json::json_node_to_string;
pub use writer::{
    write_disjoint_earthmesh_intersection_geojson,
    write_disjoint_earthmesh_intersection_geojson_with_domain,
    write_earthmesh_intersection_geojson, write_earthmesh_intersection_geojson_with_domain,
    HydroDomainComponent,
};
