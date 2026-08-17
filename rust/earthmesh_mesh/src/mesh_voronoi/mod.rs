//! The dual, rebuilt over a neighbourhood rather than the whole sphere.
//!
//! A site's Voronoi cell is the polygon on the circumcentres of the triangles
//! around it, walked in order. That is a purely local statement -- it needs the
//! fan and nothing else -- so a change that touches a handful of triangles
//! costs a handful of cells to follow, not a global pass.
//!
//! # Not the same dual as the output pipeline
//!
//! [`crate::voronoi_grid_from_triangular_mesh`] builds the gridfile's dual from
//! triangle *barycentres*, and the circumcentres arrive later as a separate
//! preprocessing step. That order is Canonical's and the writers depend on it.
//! This module is for deciding, not for writing: the criteria want the real
//! Voronoi cell, so the circumcentre is where it starts.
//!
//! # Radius
//!
//! Corners come back at the mesh's own radius, because that is where the sites
//! are and comparing the two at different scales is how a candidate ends up at
//! the centre of the earth. Areas are the exception: spherical excess is an
//! angle, so [`VoronoiCell::area_on_unit_sphere`] normalises first and leaves
//! the caller to multiply by whatever radius it meant.

use std::collections::BTreeSet;

use crate::mesh_area_primitives::spherical_cell_area_from_vertices_unit;
use crate::mesh_state::MeshState;
#[cfg(test)]
use crate::mesh_state::MESH_STATE_FIRST_ID;
use crate::spherical_circumcenter::spherical_circumcenter_from_barycenter;
use crate::CartesianPoint;

/// One site's cell, and the fan it came from.
#[derive(Clone, Debug, PartialEq)]
pub struct VoronoiCell {
    pub site: usize,
    /// The incident triangles, in rotational order.
    pub triangles: Vec<usize>,
    /// Their circumcentres, in the same order, at the mesh's radius.
    pub corners: Vec<CartesianPoint>,
}

impl VoronoiCell {
    /// How many sides the cell has, which is how many neighbours the site has.
    pub fn degree(&self) -> usize {
        self.triangles.len()
    }

    /// Area on the unit sphere. Multiply by `radius * radius` for a real one.
    ///
    /// Normalised first: the area primitives read arc lengths as angles, so
    /// handing them metres returns a number that is neither an area nor an
    /// error.
    pub fn area_on_unit_sphere(&self) -> Option<f64> {
        let unit: Vec<CartesianPoint> = self
            .corners
            .iter()
            .map(|corner| {
                let radius =
                    (corner.x * corner.x + corner.y * corner.y + corner.z * corner.z).sqrt();
                CartesianPoint::new(corner.x / radius, corner.y / radius, corner.z / radius)
            })
            .collect();
        spherical_cell_area_from_vertices_unit(&unit, unit.len())
    }
}

/// Why a cell could not be built.
#[derive(Clone, Debug, PartialEq)]
pub enum VoronoiError {
    /// The mesh does not carry this site.
    UnknownSite { site: usize },
    /// No triangle has this site as a corner, so it has no cell.
    SiteIsInNoTriangle { site: usize },
    /// The seed offered as a starting point does not touch the site.
    SeedDoesNotTouchTheSite { site: usize, seed: usize },
    /// The fan ran into an edge with nothing across it. On a closed sphere that
    /// is a hole; on a bounded mesh it is the boundary, and either way the
    /// polygon does not close.
    FanIsOpen { site: usize, at: usize },
    /// The fan visited more triangles than the mesh has without returning to
    /// its start, which means the adjacency around this site is not a disk.
    FanDidNotClose { site: usize, visited: usize },
    /// Three sites with no circumcentre: collinear on the sphere, or coincident.
    CircumcentreUndefined { triangle: usize },
}

impl std::fmt::Display for VoronoiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownSite { site } => {
                write!(formatter, "the mesh does not carry site {site}")
            }
            Self::SiteIsInNoTriangle { site } => write!(
                formatter,
                "site {site} is a corner of no triangle, so it has no cell"
            ),
            Self::SeedDoesNotTouchTheSite { site, seed } => write!(
                formatter,
                "triangle {seed} was offered as a starting point for site {site} and does not \
                 have it as a corner"
            ),
            Self::FanIsOpen { site, at } => write!(
                formatter,
                "the fan around site {site} reached an edge of triangle {at} with nothing across \
                 it, so the cell does not close"
            ),
            Self::FanDidNotClose { site, visited } => write!(
                formatter,
                "the fan around site {site} visited {visited} triangles without returning to its \
                 start"
            ),
            Self::CircumcentreUndefined { triangle } => write!(
                formatter,
                "triangle {triangle} has no circumcentre; its three sites are collinear or \
                 coincident"
            ),
        }
    }
}

impl std::error::Error for VoronoiError {}

impl MeshState {
    /// The triangles with `site` as a corner, in rotational order, starting at
    /// `seed`.
    ///
    /// Rotation rather than a scan: each step crosses one of the two edges at
    /// the site, always the same one relative to it, so the walk comes back
    /// round having visited each incident triangle exactly once.
    pub fn triangle_fan_from(&self, site: usize, seed: usize) -> Result<Vec<usize>, VoronoiError> {
        let corner_of = |triangle: usize| {
            self.is_triangle_live(triangle)
                .then(|| {
                    self.triangles()[triangle]
                        .iter()
                        .position(|&corner| corner == site)
                })
                .flatten()
        };
        if !self.is_vertex_live(site) {
            return Err(VoronoiError::UnknownSite { site });
        }
        if !self.is_triangle_live(seed) {
            return Err(VoronoiError::SeedDoesNotTouchTheSite { site, seed });
        }
        let Some(mut corner) = corner_of(seed) else {
            return Err(VoronoiError::SeedDoesNotTouchTheSite { site, seed });
        };

        let mut fan = vec![seed];
        let mut current = seed;
        // Slots, not the active count. This is a runaway backstop, so any bound
        // at least as large as the number of live triangles is correct, and the
        // slot count is one in O(1) where `triangle_count` scans every slot to
        // get an exact figure nothing here needs. A fan is about six triangles;
        // paying a full sweep of the mesh to decide how far it may walk made
        // this the dominant cost of the quality optimiser -- 90.9 percent of
        // samples on the NXP=21 CLI fixture were inside `triangle_count`.
        let limit = self.triangles().len() + 1;
        for _ in 0..limit {
            let next = self.neighbours()[current][(corner + 1) % 3];
            if next == 0 || !self.is_triangle_live(next) {
                return Err(VoronoiError::FanIsOpen { site, at: current });
            }
            if next == seed {
                return Ok(fan);
            }
            let Some(next_corner) = corner_of(next) else {
                return Err(VoronoiError::SeedDoesNotTouchTheSite { site, seed: next });
            };
            fan.push(next);
            current = next;
            corner = next_corner;
        }
        Err(VoronoiError::FanDidNotClose {
            site,
            visited: fan.len(),
        })
    }

    /// The fan around a site, finding a starting triangle by scanning.
    ///
    /// Linear in the mesh. Where the caller already knows a triangle at the
    /// site -- and after a local change it does -- [`Self::triangle_fan_from`]
    /// is the one to use.
    pub fn triangle_fan(&self, site: usize) -> Result<Vec<usize>, VoronoiError> {
        if !self.is_vertex_live(site) {
            return Err(VoronoiError::UnknownSite { site });
        }
        let seed = self
            .active_triangle_slots()
            .find(|&triangle| self.triangles()[triangle].contains(&site))
            .ok_or(VoronoiError::SiteIsInNoTriangle { site })?;
        self.triangle_fan_from(site, seed)
    }

    /// How many triangles meet at a site, starting from one that touches it.
    ///
    /// The seed is what makes this local. [`Self::vertex_degree`] has to scan
    /// for one, which is linear in the mesh -- affordable once, and quadratic
    /// for a caller measuring a neighbourhood per change.
    pub fn vertex_degree_from(&self, site: usize, seed: usize) -> Result<usize, VoronoiError> {
        self.triangle_fan_from(site, seed).map(|fan| fan.len())
    }

    /// How many triangles meet at a site, which is how many neighbours it has.
    ///
    /// The gridfile carries seven and no more -- `ItabW`'s rows are `[i32; 7]`
    /// -- so a backend that can raise this has to check it before committing.
    pub fn vertex_degree(&self, site: usize) -> Result<usize, VoronoiError> {
        self.triangle_fan(site).map(|fan| fan.len())
    }

    /// The sites cornering any of these triangles, each with a triangle that
    /// names it.
    ///
    /// What a local change hands to whatever has to re-check the
    /// neighbourhood: the cells to rebuild, the degrees to re-measure.
    pub fn sites_touching(
        &self,
        triangles: &BTreeSet<usize>,
    ) -> std::collections::BTreeMap<usize, usize> {
        let mut seeds = std::collections::BTreeMap::new();
        for &triangle in triangles {
            if !self.is_triangle_live(triangle) {
                continue;
            }
            for corner in self.triangles()[triangle] {
                seeds.entry(corner).or_insert(triangle);
            }
        }
        seeds
    }

    /// The Voronoi cell of one site, given a triangle it belongs to.
    ///
    /// The ring is pinned to its lowest triangle before the corners are taken,
    /// so the cell does not depend on which incident triangle the caller
    /// happened to know about. That matters for more than tidiness: the corners
    /// come back in ring order and their areas are summed in that order, so a
    /// rotation is a different float. [`Self::voronoi_cell`] scans for the
    /// lowest incident triangle, which makes this the rotation it already
    /// returns -- seeding the walk is then free of any effect on the result.
    pub fn voronoi_cell_from(&self, site: usize, seed: usize) -> Result<VoronoiCell, VoronoiError> {
        let mut triangles = self.triangle_fan_from(site, seed)?;
        if let Some((start, _)) = triangles.iter().enumerate().min_by_key(|(_, &t)| t) {
            triangles.rotate_left(start);
        }
        let corners = triangles
            .iter()
            .map(|&triangle| self.circumcentre(triangle))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(VoronoiCell {
            site,
            triangles,
            corners,
        })
    }

    /// The Voronoi cell of one site.
    pub fn voronoi_cell(&self, site: usize) -> Result<VoronoiCell, VoronoiError> {
        let triangles = self.triangle_fan(site)?;
        let corners = triangles
            .iter()
            .map(|&triangle| self.circumcentre(triangle))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(VoronoiCell {
            site,
            triangles,
            corners,
        })
    }

    /// Rebuild the cells of every site cornering one of these triangles.
    ///
    /// This is the local rebuild: hand it the triangles a change created and
    /// get back exactly the cells that moved. Each site is seeded from the
    /// triangle that named it, so no scanning happens.
    pub fn voronoi_cells_touching(
        &self,
        triangles: &BTreeSet<usize>,
    ) -> Result<Vec<VoronoiCell>, VoronoiError> {
        self.sites_touching(triangles)
            .into_iter()
            .map(|(site, seed)| self.voronoi_cell_from(site, seed))
            .collect()
    }

    /// The point equidistant from a triangle's three sites, on its own side of
    /// the sphere.
    pub fn circumcentre(&self, triangle: usize) -> Result<CartesianPoint, VoronoiError> {
        if !self.is_triangle_live(triangle) {
            return Err(VoronoiError::CircumcentreUndefined { triangle });
        }
        let corners = self.triangles()[triangle];
        let points = corners.map(|corner| self.vertices()[corner]);
        let barycentre = CartesianPoint::new(
            (points[0].x + points[1].x + points[2].x) / 3.0,
            (points[0].y + points[1].y + points[2].y) / 3.0,
            (points[0].z + points[1].z + points[2].z) / 3.0,
        );
        spherical_circumcenter_from_barycenter(barycentre, points)
            .ok_or(VoronoiError::CircumcentreUndefined { triangle })
    }
}

#[cfg(test)]
mod tests;
