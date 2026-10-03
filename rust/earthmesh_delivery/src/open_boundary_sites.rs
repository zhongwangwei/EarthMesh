//! The open edge of a regional parent (guide 11.116).
//!
//! A regional run's parent is the built region's final mesh, open at its far
//! edge. Its W rows are the cells; the sites on that edge have none, yet they
//! are corners of the boundary triangles, and a model writer needs where they
//! are to compute the parent's edges and vertices up to the boundary. The
//! gridfile carries them beside the mesh: their positions, and which corner
//! of which M row each one is. A closed parent carries none.

use std::io;
use std::path::Path;

use crate::{netcdf_to_io_error, LonLatPoint, UnstructuredMesh};

const SITES: &str = "earthmesh_open_sites";
const LON: &str = "earthmesh_open_site_lon";
const LAT: &str = "earthmesh_open_site_lat";
const CORNERS: &str = "earthmesh_m_open_site";

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// The sites on a regional parent's open edge.
#[derive(Clone, Debug, PartialEq)]
pub struct OpenBoundarySites {
    pub points: Vec<LonLatPoint>,
    /// For each M row, the open site at each corner: 1-based into `points`,
    /// 0 where the corner is a cell (and on placeholder rows).
    pub m_corners: Vec<[i32; 3]>,
}

impl OpenBoundarySites {
    /// Each corner names an open site or none, every site is a corner, and
    /// there is one row per M row.
    pub fn validate(&self, m_rows: usize) -> io::Result<()> {
        if self.m_corners.len() != m_rows {
            return Err(invalid(format!(
                "open boundary corners cover {} M rows, the mesh has {m_rows}",
                self.m_corners.len()
            )));
        }
        if self.points.is_empty() {
            return Err(invalid("an open boundary has at least one site"));
        }
        let mut named = vec![false; self.points.len()];
        for corner in self.m_corners.iter().flatten().copied() {
            if corner == 0 {
                continue;
            }
            let site = usize::try_from(corner)
                .ok()
                .and_then(|site| site.checked_sub(1))
                .filter(|&site| site < self.points.len())
                .ok_or_else(|| invalid(format!("open boundary corner {corner} names no site")))?;
            named[site] = true;
        }
        if named.contains(&false) {
            return Err(invalid("an open boundary site is no M row's corner"));
        }
        if self
            .points
            .iter()
            .any(|point| !point.lon.is_finite() || !(-90.0..=90.0).contains(&point.lat))
        {
            return Err(invalid("open boundary sites need finite positions"));
        }
        Ok(())
    }

    pub fn write(&self, file: &mut netcdf::FileMut) -> io::Result<()> {
        file.add_dimension(SITES, self.points.len())
            .map_err(netcdf_to_io_error)?;
        for (name, values) in [
            (LON, self.points.iter().map(|p| p.lon).collect::<Vec<_>>()),
            (LAT, self.points.iter().map(|p| p.lat).collect::<Vec<_>>()),
        ] {
            let mut var = file
                .add_variable::<f64>(name, &[SITES])
                .map_err(netcdf_to_io_error)?;
            var.put_values(&values, ..).map_err(netcdf_to_io_error)?;
        }
        let flat = self.m_corners.iter().flatten().copied().collect::<Vec<_>>();
        let mut var = file
            .add_variable::<i32>(CORNERS, &["sjx_points", "dimb"])
            .map_err(netcdf_to_io_error)?;
        var.put_values(&flat, ..).map_err(netcdf_to_io_error)?;
        Ok(())
    }
}

/// The open boundary a gridfile carries; `None` for a closed or ordinary
/// gridfile.
pub fn read_open_boundary_sites(path: impl AsRef<Path>) -> io::Result<Option<OpenBoundarySites>> {
    let file = crate::open_netcdf(path.as_ref()).map_err(netcdf_to_io_error)?;
    let Some(corners) = file.variable(CORNERS) else {
        if [LON, LAT].iter().any(|name| file.variable(name).is_some()) {
            return Err(invalid(
                "open boundary positions exist without their corners",
            ));
        }
        return Ok(None);
    };
    let read_f64 = |name: &str| -> io::Result<Vec<f64>> {
        file.variable(name)
            .ok_or_else(|| invalid(format!("open boundary misses {name}")))?
            .get_values::<f64, _>(..)
            .map_err(netcdf_to_io_error)
    };
    let (lon, lat) = (read_f64(LON)?, read_f64(LAT)?);
    if lon.len() != lat.len() {
        return Err(invalid(
            "open boundary longitudes and latitudes differ in length",
        ));
    }
    let flat = corners
        .get_values::<i32, _>(..)
        .map_err(netcdf_to_io_error)?;
    let (triples, rest) = flat.as_chunks::<3>();
    if !rest.is_empty() {
        return Err(invalid("open boundary corners are not triples"));
    }
    let sites = OpenBoundarySites {
        points: lon
            .into_iter()
            .zip(lat)
            .map(|(lon, lat)| LonLatPoint { lon, lat })
            .collect(),
        m_corners: triples.to_vec(),
    };
    let m_rows = file
        .dimension("sjx_points")
        .ok_or_else(|| invalid("open boundary gridfile has no sjx_points"))?
        .len();
    sites.validate(m_rows)?;
    Ok(Some(sites))
}

/// The parent with its open sites appended as W rows that are not cells: a
/// position each and no ring. Every corner that named no cell names its site,
/// so each edge and vertex of a cell is whole; only the sites' own cells are
/// missing, which nothing published reaches. Returns the mesh and the first
/// appended row.
pub fn mesh_with_open_sites(
    mesh: &UnstructuredMesh,
    sites: &OpenBoundarySites,
) -> io::Result<(UnstructuredMesh, usize)> {
    sites.validate(mesh.m_points.len())?;
    let layout = crate::unstructured_mesh_support::unstructured_w_row_layout(mesh);
    let first = mesh.w_points.len();
    let mut open = mesh.clone();
    let width = open.w_to_m.first().map_or(1, Vec::len).max(1);
    let mut ids = Vec::with_capacity(sites.points.len());
    for (offset, point) in sites.points.iter().enumerate() {
        let row = first + offset;
        let id = layout
            .canonical_id_for_physical_row(row)
            .ok_or_else(|| invalid(format!("open site row {row} has no id")))?;
        ids.push(id);
        open.w_points.push(*point);
        open.w_to_m.push(vec![1; width]);
        open.n_w_to_m.push(0);
    }
    for (row, corners) in sites.m_corners.iter().enumerate() {
        for (slot, &corner) in corners.iter().enumerate() {
            if corner == 0 {
                continue;
            }
            if open.m_to_w[row][slot] > 1 {
                return Err(invalid(format!(
                    "M row {row} corner {slot} is a cell and an open site"
                )));
            }
            open.m_to_w[row][slot] = ids[corner as usize - 1];
        }
    }
    Ok((open, first))
}

/// A triangle parent open at its far edge, as its own rows tell it: a W row
/// whose fan does not close -- some spoke from the site lies on one face of
/// the fan, not two -- keeps its position and loses its ring, so the cells
/// inside keep every edge and vertex whole and the edge rows stand only as
/// sites (guide 11.116). Fans are read as sets: a closed parent's rows need
/// not list them in turn. Returns the mesh and how many rows lost their ring.
pub fn mesh_with_open_fans_as_sites(
    mesh: &UnstructuredMesh,
) -> io::Result<(UnstructuredMesh, usize)> {
    use crate::unstructured_mesh_support::{
        mesh_m_has_two_placeholder_rows, mesh_row_for_canonical_id, unstructured_w_row_layout,
    };
    use std::collections::BTreeMap;
    let m_two = mesh_m_has_two_placeholder_rows(mesh);
    let layout = unstructured_w_row_layout(mesh);
    let mut open = mesh.clone();
    let mut opened = 0;
    for row in layout.first_physical_row..mesh.w_points.len() {
        let id = layout
            .canonical_id_for_physical_row(row)
            .ok_or_else(|| invalid(format!("W row {row} has no id")))?;
        let count = usize::try_from(mesh.n_w_to_m[row]).unwrap_or(0);
        let mut spokes = BTreeMap::<i32, usize>::new();
        for &face in mesh.w_to_m[row].iter().take(count) {
            let corners = mesh_row_for_canonical_id(face, mesh.m_points.len(), m_two)
                .map(|m| mesh.m_to_w[m])
                .ok_or_else(|| invalid(format!("W row {row} names no M row {face}")))?;
            for corner in corners.into_iter().filter(|&corner| corner != id) {
                *spokes.entry(corner).or_default() += 1;
            }
        }
        if count < 3 || spokes.values().any(|&faces| faces != 2) {
            open.n_w_to_m[row] = 0;
            opened += 1;
        }
    }
    Ok((open, opened))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_sites_round_trip_and_close_every_corner() {
        let point = |lon: f64, lat: f64| LonLatPoint { lon, lat };
        // One placeholder row, ids from 2: cells 2 and 3, and two triangles
        // whose third corners are open sites.
        let mesh = UnstructuredMesh {
            m_points: vec![point(0.0, 0.0), point(1.0, 1.0), point(1.0, -1.0)],
            w_points: vec![point(0.0, 0.0), point(0.5, 0.0), point(1.5, 0.0)],
            m_to_w: vec![[1, 1, 1], [2, 3, 1], [3, 2, 1]],
            w_to_m: vec![vec![1, 1, 1], vec![2, 3, 1], vec![3, 2, 1]],
            n_w_to_m: vec![1, 2, 2],
        };
        let sites = OpenBoundarySites {
            points: vec![point(1.0, 2.0), point(1.0, -2.0)],
            m_corners: vec![[0, 0, 0], [0, 0, 1], [0, 0, 2]],
        };
        sites.validate(mesh.m_points.len()).unwrap();
        let (open, first) = mesh_with_open_sites(&mesh, &sites).unwrap();
        assert_eq!(first, 3);
        assert_eq!(open.w_points[3..], sites.points[..]);
        assert_eq!(open.n_w_to_m[3..], [0, 0]);
        assert_eq!(open.m_to_w[1], [2, 3, 4]);
        assert_eq!(open.m_to_w[2], [3, 2, 5]);

        let path = std::env::temp_dir().join(format!(
            "earthmesh_open_boundary_sites_{}.nc4",
            std::process::id()
        ));
        crate::write_unstructured_mesh_netcdf_with_metadata(
            &path,
            &mesh,
            crate::GridfileMetadataSlices {
                open_boundary: Some(&sites),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            read_open_boundary_sites(&path).unwrap(),
            Some(sites.clone())
        );
        crate::write_unstructured_mesh_netcdf(&path, &mesh).unwrap();
        assert_eq!(read_open_boundary_sites(&path).unwrap(), None);
        let _ = std::fs::remove_file(&path);

        let mut unnamed = sites.clone();
        unnamed.m_corners[2] = [0, 0, 0];
        assert!(unnamed.validate(3).is_err());
        let mut both = sites;
        both.m_corners[1] = [1, 0, 1];
        assert!(mesh_with_open_sites(&mesh, &both).is_err());
    }
}
