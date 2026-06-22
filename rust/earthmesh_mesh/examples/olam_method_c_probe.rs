use std::{env, io};

use earthmesh_mesh::{LonLatDegrees, OlamDelaunayMesh, OlamRefinementRegion};

fn main() -> io::Result<()> {
    let mut args = env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| "tables".to_string());
    let case_name = args.next().unwrap_or_else(|| "nxp6_circle".to_string());

    match mode.as_str() {
        "tables" => {
            let mesh = build_case(&case_name, false)?;
            print!("{}", mesh.olam_delaunay_topology_dump());
        }
        "spring" => {
            let nxp = case_nxp(&case_name)?;
            let mesh = OlamDelaunayMesh::from_icosahedron(nxp, 100, 1.25, 0.035, 100)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Rust spring failed"))?;
            println!("spring counts nmd={} nud={} nwd={}", mesh.nmd, mesh.nud, mesh.nwd);
            for &im in spring_sample_points(mesh.nmd).iter() {
                let point = mesh.m_points[im];
                println!(
                    "spring M {im} x={:.3} y={:.3} z={:.3}",
                    point.x, point.y, point.z
                );
            }
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown probe mode: {mode}"),
            ));
        }
    }

    Ok(())
}

fn build_case(case_name: &str, with_spring: bool) -> io::Result<OlamDelaunayMesh> {
    let nxp = case_nxp(case_name)?;
    let regions = case_regions(case_name)?;
    let max_level = regions
        .iter()
        .map(OlamRefinementRegion::level)
        .max()
        .unwrap_or(1);
    let mesh = OlamDelaunayMesh::from_icosahedron(nxp, 0, 1.0, 0.25, 100)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "base OLAM mesh failed"))?;
    if with_spring {
        mesh.spawn_nest_with_spring_as_atmosmesh(&regions, max_level, nxp, 100)
            .map(|(mesh, _)| mesh)
    } else {
        mesh.spawn_nest_as_atmosmesh(&regions, max_level)
    }
}

fn case_nxp(case_name: &str) -> io::Result<usize> {
    match case_name {
        "nxp6_circle"
        | "nxp6_corridor"
        | "nxp6_variable_corridor"
        | "nxp6_three_point_corridor"
        | "nxp6_two_circle"
        | "nxp6_two_corridor"
        | "nxp6_bad_two_circle"
        | "nxp6_bad_two_corridor" => Ok(6),
        "nxp7_circle" | "nxp7_corridor" | "nxp7_two_circle" | "nxp7_two_corridor" => Ok(7),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown case: {case_name}"),
        )),
    }
}

fn case_regions(case_name: &str) -> io::Result<Vec<OlamRefinementRegion>> {
    let ll = |lon, lat| LonLatDegrees::new(lon, lat);
    let regions = match case_name {
        "nxp6_circle" | "nxp7_circle" => vec![OlamRefinementRegion::Circle {
            center: ll(115.0, 25.0),
            radius_meters: 2_500_000.0,
            level: 1,
        }],
        "nxp6_corridor" | "nxp7_corridor" => vec![OlamRefinementRegion::Corridor {
            points: vec![ll(115.0, 25.0), ll(130.0, 25.0)],
            radius_meters: vec![2_500_000.0, 2_500_000.0],
            level: 1,
        }],
        "nxp6_variable_corridor" => vec![OlamRefinementRegion::Corridor {
            points: vec![ll(115.0, 25.0), ll(130.0, 25.0)],
            radius_meters: vec![2_500_000.0, 1_250_000.0],
            level: 1,
        }],
        "nxp6_three_point_corridor" => vec![OlamRefinementRegion::Corridor {
            points: vec![ll(115.0, 25.0), ll(130.0, 25.0), ll(150.0, 0.0)],
            radius_meters: vec![2_500_000.0, 2_500_000.0, 2_500_000.0],
            level: 1,
        }],
        "nxp6_two_circle" => vec![
            OlamRefinementRegion::Circle {
                center: ll(115.0, 25.0),
                radius_meters: 4_000_000.0,
                level: 1,
            },
            OlamRefinementRegion::Circle {
                center: ll(115.0, 25.0),
                radius_meters: 1_000_000.0,
                level: 2,
            },
        ],
        "nxp7_two_circle" => vec![
            OlamRefinementRegion::Circle {
                center: ll(115.0, 25.0),
                radius_meters: 3_000_000.0,
                level: 1,
            },
            OlamRefinementRegion::Circle {
                center: ll(115.0, 25.0),
                radius_meters: 1_000_000.0,
                level: 2,
            },
        ],
        "nxp6_two_corridor" => vec![
            OlamRefinementRegion::Corridor {
                points: vec![ll(115.0, 25.0), ll(130.0, 25.0)],
                radius_meters: vec![6_000_000.0, 6_000_000.0],
                level: 1,
            },
            OlamRefinementRegion::Corridor {
                points: vec![ll(120.0, 25.0), ll(125.0, 25.0)],
                radius_meters: vec![1_000_000.0, 1_000_000.0],
                level: 2,
            },
        ],
        "nxp7_two_corridor" => vec![
            OlamRefinementRegion::Corridor {
                points: vec![ll(115.0, 25.0), ll(130.0, 25.0)],
                radius_meters: vec![2_500_000.0, 2_500_000.0],
                level: 1,
            },
            OlamRefinementRegion::Corridor {
                points: vec![ll(120.0, 25.0), ll(125.0, 25.0)],
                radius_meters: vec![500_000.0, 500_000.0],
                level: 2,
            },
        ],
        "nxp6_bad_two_circle" => vec![
            OlamRefinementRegion::Circle {
                center: ll(115.0, 25.0),
                radius_meters: 2_500_000.0,
                level: 1,
            },
            OlamRefinementRegion::Circle {
                center: ll(115.0, 25.0),
                radius_meters: 1_000_000.0,
                level: 2,
            },
        ],
        "nxp6_bad_two_corridor" => vec![
            OlamRefinementRegion::Corridor {
                points: vec![ll(115.0, 25.0), ll(130.0, 25.0)],
                radius_meters: vec![6_000_000.0, 6_000_000.0],
                level: 1,
            },
            OlamRefinementRegion::Corridor {
                points: vec![ll(115.0, 25.0), ll(130.0, 25.0)],
                radius_meters: vec![1_000_000.0, 1_000_000.0],
                level: 2,
            },
        ],
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown case: {case_name}"),
            ));
        }
    };
    Ok(regions)
}

fn spring_sample_points(nmd: usize) -> [usize; 8] {
    [
        2,
        3,
        4,
        5,
        nmd / 4,
        nmd / 2,
        nmd * 3 / 4,
        nmd,
    ]
}
