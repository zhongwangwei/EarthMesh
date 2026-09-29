use super::*;

/// A lon/lat raster with `level` within `radius_km` of each centre.
fn raster(spots: &[((f64, f64), f64, usize)]) -> RasterLevelField {
    let (nlon, nlat) = (360usize, 180usize);
    let mut levels = vec![0usize; nlon * nlat];
    for j in 0..nlat {
        let lat = (-90.0 + (j as f64 + 0.5)).to_radians();
        for i in 0..nlon {
            let lon = (-180.0 + (i as f64 + 0.5)).to_radians();
            let p = [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()];
            for &((clon, clat), radius_km, level) in spots {
                let (clon, clat) = (clon.to_radians(), clat.to_radians());
                let c = [clat.cos() * clon.cos(), clat.cos() * clon.sin(), clat.sin()];
                let d = (p[0] * c[0] + p[1] * c[1] + p[2] * c[2])
                    .clamp(-1.0, 1.0)
                    .acos()
                    * 6371.0;
                if d <= radius_km {
                    levels[j * nlon + i] = levels[j * nlon + i].max(level);
                }
            }
        }
    }
    RasterLevelField::new(nlon, nlat, levels).unwrap()
}

fn degrees(mesh: &MeshState) -> Vec<usize> {
    let mut degree = vec![0usize; mesh.vertices().len()];
    for t in mesh.active_triangle_slots() {
        for v in mesh.triangles()[t] {
            degree[v] += 1;
        }
    }
    mesh.active_vertex_slots().map(|v| degree[v]).collect()
}

#[test]
fn a_level_three_demand_is_served_by_a_stretched_level_two_mother() {
    let demand = raster(&[((115.0, 23.0), 500.0, 3)]);
    let stretched = stretched_certified_mother(
        20,
        3,
        &demand,
        AngleContractId::LegacyStrict40To80,
        10_000_000,
    )
    .unwrap_or_else(|why| panic!("{why:?}"));
    // One level coarser than the safe mother: a quarter of its cells.
    assert_eq!((stretched.mother_level, stretched.subdivision), (2, 80));
    assert!(
        stretched.factor > 2.0 && stretched.factor <= 4.0,
        "{}",
        stretched.factor
    );
    // Every cell at least the base level, the demand at its level, and the
    // grid still icosahedral: degree 5 or 6 everywhere.
    assert!(stretched.delivered_levels.iter().max() >= Some(&3));
    let mesh = stretched.geometry.primal();
    let degree = degrees(mesh);
    assert!(degree.iter().all(|&d| d == 5 || d == 6));
    assert_eq!(degree.iter().filter(|&&d| d == 5).count(), 12);
    assert_eq!(stretched.final_requirements.physical_residuals(), 0);
    assert_eq!(stretched.final_requirements.balance_residuals(), 0);
}

#[test]
fn a_demand_one_focus_cannot_reach_is_refused_with_reasons() {
    // Two regions on opposite sides of the globe.
    let demand = raster(&[((115.0, 23.0), 500.0, 3), ((-60.0, -20.0), 500.0, 3)]);
    let why = stretched_certified_mother(
        20,
        3,
        &demand,
        AngleContractId::LegacyStrict40To80,
        10_000_000,
    )
    .err()
    .expect("scattered demand has no stretched mother");
    assert!(
        why.iter()
            .any(|reason| reason.contains("one focus cannot serve")),
        "{why:?}"
    );
}

#[test]
fn a_shallow_demand_has_no_stretched_mother_below_it() {
    // Level 2: a level-1 mother may stretch by 2 at most, and a demand of any
    // extent needs more; level 0 cannot stretch at all.
    let demand = raster(&[((115.0, 23.0), 500.0, 2)]);
    assert!(stretched_certified_mother(
        20,
        2,
        &demand,
        AngleContractId::LegacyStrict40To80,
        10_000_000
    )
    .is_err());
}
