use std::f64::consts::PI;
use std::fs;
use std::io;
use std::path::Path;

const WGS84_A: f64 = 6_378_137.0;
const WGS84_E2: f64 = 0.006_694_379_990_141_316_5;

#[derive(Clone, Debug, PartialEq)]
pub struct ShapefilePolygonComponent {
    pub shell: Vec<(f64, f64)>,
    pub holes: Vec<Vec<(f64, f64)>>,
}

impl ShapefilePolygonComponent {
    /// Lower one validated spherical component to the legacy close-mask
    /// even/odd ring representation used by the current engine.
    pub fn into_close_ring(self) -> io::Result<Vec<(f64, f64)>> {
        assemble_polygon_component(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ShapefileCrs {
    Wgs84,
    WebMercator,
    Utm { zone: u8, north: bool },
    Proj4(String),
}

pub fn read_close_mask_nml_points(path: &Path) -> io::Result<Vec<(f64, f64)>> {
    let text = fs::read_to_string(path)?;
    let mut lines = text.lines();
    let count = lines
        .next()
        .and_then(|line| line.split_once('='))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing close_num"))?;
    let _ = lines.next();
    let points = read_lonlat_rows(lines.take(count))?;
    if points.len() != count {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("close_num says {count}, found {}", points.len()),
        ));
    }
    Ok(points)
}

pub fn read_lonlat_text_points(path: &Path) -> io::Result<Vec<(f64, f64)>> {
    let text = fs::read_to_string(path)?;
    read_lonlat_rows(text.lines())
}

pub fn write_close_mask_nml(
    path: &Path,
    ring: &[(f64, f64)],
    refine_degree: usize,
) -> io::Result<()> {
    let mut body = format!(
        "close_num = {}\nclose_refine = {refine_degree}\n",
        ring.len()
    );
    for &(lon, lat) in ring {
        body.push_str(&format!("{lon:.10} {lat:.10}\n"));
    }
    fs::write(path, body)
}

pub fn read_shapefile_polygon_rings(path: &Path) -> io::Result<Vec<Vec<(f64, f64)>>> {
    read_shapefile_polygon_components(path)?
        .into_iter()
        .map(ShapefilePolygonComponent::into_close_ring)
        .collect()
}

/// Read validated polygon components while preserving SHP feature-record union
/// semantics and even/odd nesting within each record.
pub fn read_shapefile_polygon_components(
    path: &Path,
) -> io::Result<Vec<ShapefilePolygonComponent>> {
    polygon_components_from_records(read_shapefile_polygon_records(path)?, "shapefile")
}

/// Read native polygon parts grouped by their SHP record.
///
/// A record's rings use even/odd nesting; records are independent polygon
/// features whose areas are unioned. Keeping this boundary prevents a polygon
/// nested inside a different record from being misclassified as a hole.
#[allow(clippy::type_complexity)]
pub fn read_shapefile_polygon_parts(path: &Path) -> io::Result<Vec<Vec<Vec<(f64, f64)>>>> {
    read_shapefile_polygon_records(path)
}

#[allow(clippy::type_complexity)]
fn read_shapefile_polygon_records(path: &Path) -> io::Result<Vec<Vec<Vec<(f64, f64)>>>> {
    let bytes = fs::read(path)?;
    if bytes.len() < 100 || be_i32(&bytes, 0)? != 9994 || le_i32(&bytes, 28)? != 1000 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not an ESRI shapefile",
        ));
    }
    let mut offset = 100;
    let mut records = Vec::new();
    while offset < bytes.len() {
        if offset + 8 > bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated shapefile record header",
            ));
        }
        let content_len = be_usize(&bytes, offset + 4)?
            .checked_mul(2)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "record length overflow"))?;
        let start = offset + 8;
        let end = start
            .checked_add(content_len)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "record end overflow"))?;
        if end > bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated shapefile record content",
            ));
        }
        if let Some(rings) = read_polygon_record(&bytes[start..end])? {
            records.push(rings);
        }
        offset = end;
    }
    if records.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "shapefile contains no polygon rings",
        ));
    }
    let all_rings = records.iter().flatten().cloned().collect::<Vec<_>>();
    let crs = read_shapefile_crs(path, &all_rings)?;
    for rings in &mut records {
        reproject_rings(rings, &crs)?;
    }
    Ok(records)
}

fn read_lonlat_rows<'a>(lines: impl Iterator<Item = &'a str>) -> io::Result<Vec<(f64, f64)>> {
    let mut points = Vec::new();
    for (index, line) in lines.enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts = line
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if parts.len() < 2 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("lon/lat row {} needs two numbers", index + 1),
            ));
        }
        let lon = parts[0].parse::<f64>().map_err(|err| {
            io::Error::new(io::ErrorKind::InvalidData, format!("invalid lon: {err}"))
        })?;
        let lat = parts[1].parse::<f64>().map_err(|err| {
            io::Error::new(io::ErrorKind::InvalidData, format!("invalid lat: {err}"))
        })?;
        validate_lonlat(lon, lat)?;
        points.push((lon, lat));
    }
    if points.len() < 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "close lon/lat text needs at least three points",
        ));
    }
    Ok(points)
}

fn read_shapefile_crs(path: &Path, rings: &[Vec<(f64, f64)>]) -> io::Result<ShapefileCrs> {
    let prj = [path.with_extension("prj"), path.with_extension("PRJ")]
        .into_iter()
        .find(|candidate| candidate.is_file());
    let Some(prj) = prj else {
        if rings
            .iter()
            .flatten()
            .all(|&(lon, lat)| validate_lonlat(lon, lat).is_ok())
        {
            return Ok(ShapefileCrs::Wgs84);
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "projected-looking SHP coordinates require a .prj CRS file",
        ));
    };
    detect_prj_crs(&fs::read_to_string(&prj)?).map_err(|message| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported SHP CRS in {}: {message}", prj.display()),
        )
    })
}

fn detect_prj_crs(prj: &str) -> Result<ShapefileCrs, String> {
    let upper = prj.to_ascii_uppercase();
    if [
        "PSEUDO_MERCATOR",
        "WEB_MERCATOR",
        "MERCATOR_AUXILIARY_SPHERE",
        "EPSG\",3857",
        "EPSG:3857",
        "102100",
        "900913",
    ]
    .iter()
    .any(|needle| upper.contains(needle))
    {
        return Ok(ShapefileCrs::WebMercator);
    }
    if let Some((zone, north)) = parse_utm_zone(&upper) {
        return Ok(ShapefileCrs::Utm { zone, north });
    }
    let projected = upper.contains("PROJCS[") || upper.contains("PROJCRS[");
    let wgs84 = upper.contains("WGS_1984")
        || upper.contains("WGS 84")
        || upper.contains("EPSG\",4326")
        || upper.contains("EPSG:4326");
    if wgs84 && !projected {
        Ok(ShapefileCrs::Wgs84)
    } else if projected || upper.contains("GEOGCS[") || upper.contains("GEOGCRS[") {
        proj4wkt::wkt_to_projstring(prj)
            .map(ShapefileCrs::Proj4)
            .map_err(|err| format!("WKT projection is not supported: {err}"))
    } else {
        Err("CRS is not recognized as WGS84".to_string())
    }
}

fn parse_utm_zone(upper: &str) -> Option<(u8, bool)> {
    let start = upper
        .find("UTM_ZONE_")
        .map(|index| index + "UTM_ZONE_".len())
        .or_else(|| {
            upper
                .find("UTM ZONE ")
                .map(|index| index + "UTM ZONE ".len())
        })?;
    let tail = &upper[start..];
    let digits = tail
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    let zone = digits.parse::<u8>().ok()?;
    if !(1..=60).contains(&zone) {
        return None;
    }
    let hemisphere = tail[digits.len()..]
        .chars()
        .find(|ch| matches!(ch, 'N' | 'S'))
        .unwrap_or('N');
    Some((zone, hemisphere == 'N'))
}

fn reproject_rings(rings: &mut [Vec<(f64, f64)>], crs: &ShapefileCrs) -> io::Result<()> {
    if let ShapefileCrs::Proj4(definition) = crs {
        let from = proj4rs::proj::Proj::from_proj_string(definition).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("invalid SHP CRS: {err}"),
            )
        })?;
        let to = proj4rs::proj::Proj::from_proj_string(
            "+proj=longlat +datum=WGS84 +ellps=WGS84 +no_defs +type=crs",
        )
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
        for point in rings.iter_mut().flatten() {
            let mut transformed = (point.0, point.1, 0.0);
            proj4rs::transform::transform(&from, &to, &mut transformed).map_err(|err| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("reproject SHP coordinate: {err}"),
                )
            })?;
            *point = (transformed.0.to_degrees(), transformed.1.to_degrees());
            validate_lonlat(point.0, point.1)?;
        }
        return Ok(());
    }
    for point in rings.iter_mut().flatten() {
        *point = to_wgs84(*point, crs)?;
    }
    Ok(())
}

fn to_wgs84(point: (f64, f64), crs: &ShapefileCrs) -> io::Result<(f64, f64)> {
    let (lon, lat) = match crs {
        ShapefileCrs::Wgs84 => point,
        ShapefileCrs::WebMercator => web_mercator_to_wgs84(point)?,
        ShapefileCrs::Utm { zone, north } => utm_to_wgs84(point, *zone, *north)?,
        ShapefileCrs::Proj4(_) => unreachable!("generic CRS is transformed in one batch"),
    };
    validate_lonlat(lon, lat)?;
    Ok((lon, lat))
}

fn web_mercator_to_wgs84((x, y): (f64, f64)) -> io::Result<(f64, f64)> {
    if !x.is_finite() || !y.is_finite() || x.abs() > 20_037_508.35 || y.abs() > 20_048_966.11 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Web Mercator coordinate is outside the valid world extent",
        ));
    }
    Ok((
        (x / WGS84_A).to_degrees(),
        (2.0 * (y / WGS84_A).exp().atan() - PI / 2.0).to_degrees(),
    ))
}

fn utm_to_wgs84((easting, northing): (f64, f64), zone: u8, north: bool) -> io::Result<(f64, f64)> {
    if !easting.is_finite()
        || !northing.is_finite()
        || !(100_000.0..=1_000_000.0).contains(&easting)
        || !(0.0..=10_000_000.0).contains(&northing)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "UTM coordinate is outside the supported range",
        ));
    }
    let k0 = 0.9996;
    let x = easting - 500_000.0;
    let y = if north {
        northing
    } else {
        northing - 10_000_000.0
    };
    let e1 = (1.0 - (1.0 - WGS84_E2).sqrt()) / (1.0 + (1.0 - WGS84_E2).sqrt());
    let m = y / k0;
    let mu = m
        / (WGS84_A
            * (1.0
                - WGS84_E2 / 4.0
                - 3.0 * WGS84_E2.powi(2) / 64.0
                - 5.0 * WGS84_E2.powi(3) / 256.0));
    let phi1 = mu
        + (3.0 * e1 / 2.0 - 27.0 * e1.powi(3) / 32.0) * (2.0 * mu).sin()
        + (21.0 * e1.powi(2) / 16.0 - 55.0 * e1.powi(4) / 32.0) * (4.0 * mu).sin()
        + 151.0 * e1.powi(3) / 96.0 * (6.0 * mu).sin()
        + 1097.0 * e1.powi(4) / 512.0 * (8.0 * mu).sin();
    let ep2 = WGS84_E2 / (1.0 - WGS84_E2);
    let n1 = WGS84_A / (1.0 - WGS84_E2 * phi1.sin().powi(2)).sqrt();
    let t1 = phi1.tan().powi(2);
    let c1 = ep2 * phi1.cos().powi(2);
    let r1 = WGS84_A * (1.0 - WGS84_E2) / (1.0 - WGS84_E2 * phi1.sin().powi(2)).powf(1.5);
    let d = x / (n1 * k0);
    let lat = phi1
        - (n1 * phi1.tan() / r1)
            * (d.powi(2) / 2.0
                - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1.powi(2) - 9.0 * ep2) * d.powi(4) / 24.0
                + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1.powi(2)
                    - 252.0 * ep2
                    - 3.0 * c1.powi(2))
                    * d.powi(6)
                    / 720.0);
    let lon = (f64::from(zone) - 1.0) * 6.0 - 180.0
        + 3.0
        + ((d - (1.0 + 2.0 * t1 + c1) * d.powi(3) / 6.0
            + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1.powi(2) + 8.0 * ep2 + 24.0 * t1.powi(2))
                * d.powi(5)
                / 120.0)
            / phi1.cos())
        .to_degrees();
    Ok((lon, lat.to_degrees()))
}

fn validate_lonlat(lon: f64, lat: f64) -> io::Result<()> {
    if !lon.is_finite()
        || !lat.is_finite()
        || !(-180.0..=180.0).contains(&lon)
        || !(-90.0..=90.0).contains(&lat)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("coordinate ({lon}, {lat}) is not valid WGS84 longitude/latitude"),
        ));
    }
    Ok(())
}

#[allow(clippy::type_complexity)]
fn read_polygon_record(content: &[u8]) -> io::Result<Option<Vec<Vec<(f64, f64)>>>> {
    if content.len() < 4 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "truncated shapefile record",
        ));
    }
    let shape_type = le_i32(content, 0)?;
    if shape_type == 0 {
        return Ok(None);
    }
    if !matches!(shape_type, 5 | 15 | 25) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported shapefile shape type {shape_type}; expected Polygon"),
        ));
    }
    if content.len() < 44 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "truncated polygon record",
        ));
    }
    let num_parts = le_usize(content, 36)?;
    let num_points = le_usize(content, 40)?;
    if num_parts == 0 || num_points == 0 || num_parts > num_points {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid polygon parts/points count",
        ));
    }
    let points_start =
        44_usize
            .checked_add(num_parts.checked_mul(4).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "parts length overflow")
            })?)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "parts offset overflow"))?;
    let points_end =
        points_start
            .checked_add(num_points.checked_mul(16).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "points length overflow")
            })?)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "points offset overflow"))?;
    if points_end > content.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid polygon parts/points length",
        ));
    }
    let mut starts = (0..num_parts)
        .map(|index| le_usize(content, 44 + index * 4))
        .collect::<io::Result<Vec<_>>>()?;
    if starts.first().copied() != Some(0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "first polygon part must start at point zero",
        ));
    }
    starts.push(num_points);
    let mut rings = Vec::new();
    for pair in starts.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        if from >= to || to > num_points {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid polygon part index",
            ));
        }
        let mut ring = (from..to)
            .map(|point| {
                let offset = points_start + point * 16;
                Ok((le_f64(content, offset)?, le_f64(content, offset + 8)?))
            })
            .collect::<io::Result<Vec<_>>>()?;
        if same_point(ring.first(), ring.last()) {
            ring.pop();
        }
        if ring.len() < 3 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "polygon part has fewer than three distinct vertices",
            ));
        }
        if !ring.iter().all(|(x, y)| x.is_finite() && y.is_finite()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "polygon part contains a non-finite coordinate",
            ));
        }
        rings.push(ring);
    }
    if rings.is_empty() {
        Ok(None)
    } else {
        Ok(Some(rings))
    }
}

/// Convert an ESRI multipart polygon into simple rings that retain holes.
/// A hole is joined to its containing shell with a doubled bridge; the existing
/// even/odd point-in-polygon engine then excludes the hole without a parallel
/// topology representation or a new mask file format.
fn polygon_components_from_records(
    records: Vec<Vec<Vec<(f64, f64)>>>,
    label: &str,
) -> io::Result<Vec<ShapefilePolygonComponent>> {
    let mut components = Vec::new();
    for (record, rings) in records.into_iter().enumerate() {
        components.extend(polygon_components_from_record(
            rings,
            &format!("{label} record {}", record + 1),
        )?);
    }
    if components.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} contains no polygon rings"),
        ));
    }
    Ok(components)
}

fn polygon_components_from_record(
    mut rings: Vec<Vec<(f64, f64)>>,
    label: &str,
) -> io::Result<Vec<ShapefilePolygonComponent>> {
    if rings.is_empty() || rings.iter().any(|ring| ring.len() < 3) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} contains a polygon ring with fewer than three vertices"),
        ));
    }
    for ring in &mut rings {
        if ring.first() != ring.last() {
            ring.push(ring[0]);
        }
    }
    let areas = rings
        .iter()
        .enumerate()
        .map(|(ring_index, ring)| {
            let points = ring
                .iter()
                .map(|&(lon, lat)| earthmesh_geometry::Point::new(lon, lat))
                .collect::<Vec<_>>();
            earthmesh_geometry::try_spherical_polygon_excess(
                &points,
                earthmesh_geometry::SphericalAreaBranch::Minor,
            )
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{label} ring {} is invalid: {error}", ring_index + 1),
                )
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let mut contains = vec![vec![false; rings.len()]; rings.len()];
    for left in 0..rings.len() {
        for right in (left + 1)..rings.len() {
            if spherical_rings_intersect_or_touch(&rings[left], &rings[right]) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{label} rings {} and {} intersect or touch; rings within one polygon record must be strictly nested or disjoint",
                        left + 1,
                        right + 1
                    ),
                ));
            }
            contains[right][left] = spherical_point_relation(rings[left][0], &rings[right])
                == SphericalPointRelation::Inside;
            contains[left][right] = spherical_point_relation(rings[right][0], &rings[left])
                == SphericalPointRelation::Inside;
            if contains[right][left] && contains[left][right] {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "{label} rings {} and {} have ambiguous spherical nesting",
                        left + 1,
                        right + 1
                    ),
                ));
            }
        }
    }
    let mut parents = vec![None; rings.len()];
    for child in 0..rings.len() {
        parents[child] = (0..rings.len())
            .filter(|&candidate| {
                candidate != child
                    && areas[candidate] > areas[child] + 1.0e-12
                    && contains[candidate][child]
            })
            .min_by(|&left, &right| areas[left].total_cmp(&areas[right]));
    }
    let mut depths = vec![0usize; rings.len()];
    for (index, ring_depth) in depths.iter_mut().enumerate() {
        let mut cursor = index;
        for depth in 0..rings.len() {
            let Some(parent) = parents[cursor] else {
                *ring_depth = depth;
                break;
            };
            cursor = parent;
            if depth + 1 == rings.len() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{label} contains cyclic polygon nesting"),
                ));
            }
        }
    }
    Ok((0..rings.len())
        .filter(|&index| depths[index].is_multiple_of(2))
        .map(|shell| ShapefilePolygonComponent {
            shell: rings[shell].clone(),
            holes: (0..rings.len())
                .filter(|&index| {
                    parents[index] == Some(shell) && depths[index] == depths[shell] + 1
                })
                .map(|index| rings[index].clone())
                .collect(),
        })
        .collect())
}

fn assemble_polygon_component(
    mut component: ShapefilePolygonComponent,
) -> io::Result<Vec<(f64, f64)>> {
    if [90.0, -90.0].into_iter().any(|latitude| {
        spherical_point_relation((0.0, latitude), &component.shell)
            != SphericalPointRelation::Outside
    }) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "polar SHP polygons cannot be lowered to a planar close ring; use an enclosing_cap or bbox/circle domain",
        ));
    }
    if same_point(component.shell.first(), component.shell.last()) {
        component.shell.pop();
    }
    for hole in &mut component.holes {
        if same_point(hole.first(), hole.last()) {
            hole.pop();
        }
    }
    let anchor = component.shell[0].0;
    let mut shell = unwrap_ring_near(&component.shell, anchor);
    let mut holes = component
        .holes
        .into_iter()
        .map(|hole| unwrap_ring_near(&hole, anchor))
        .collect::<Vec<_>>();
    holes.sort_by(|left, right| ring_rightmost_x(right).total_cmp(&ring_rightmost_x(left)));
    for hole in holes {
        shell = bridge_hole(shell, hole);
    }
    Ok(shell
        .into_iter()
        .map(|(lon, lat)| (wrap_lon(lon), lat))
        .collect())
}

fn unwrap_ring_near(ring: &[(f64, f64)], anchor: f64) -> Vec<(f64, f64)> {
    let mut previous = unwrap_lon_near(ring[0].0, anchor);
    let mut unwrapped = vec![(previous, ring[0].1)];
    for &(lon, lat) in &ring[1..] {
        previous = unwrap_lon_near(lon, previous);
        unwrapped.push((previous, lat));
    }
    let mean = unwrapped.iter().map(|point| point.0).sum::<f64>() / unwrapped.len() as f64;
    let shift = ((anchor - mean) / 360.0).round() * 360.0;
    for point in &mut unwrapped {
        point.0 += shift;
    }
    unwrapped
}

fn unwrap_lon_near(lon: f64, anchor: f64) -> f64 {
    anchor + (lon - anchor + 180.0).rem_euclid(360.0) - 180.0
}

fn wrap_lon(lon: f64) -> f64 {
    let wrapped = (lon + 180.0).rem_euclid(360.0) - 180.0;
    if wrapped == -180.0 && lon > 0.0 {
        180.0
    } else {
        wrapped
    }
}

#[cfg(test)]
fn point_in_ring(point: (f64, f64), ring: &[(f64, f64)]) -> bool {
    let point = (unwrap_lon_near(point.0, ring[0].0), point.1);
    let mut inside = false;
    for (&a, &b) in ring
        .iter()
        .zip(ring.iter().cycle().skip(1))
        .take(ring.len())
    {
        let a = (unwrap_lon_near(a.0, point.0), a.1);
        let b = (unwrap_lon_near(b.0, point.0), b.1);
        if (a.1 > point.1) != (b.1 > point.1)
            && point.0 < a.0 + (point.1 - a.1) * (b.0 - a.0) / (b.1 - a.1)
        {
            inside = !inside;
        }
    }
    inside
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SphericalPointRelation {
    Outside,
    Boundary,
    Inside,
}

type SphericalVector = [f64; 3];

fn spherical_dot(left: SphericalVector, right: SphericalVector) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

fn spherical_cross(left: SphericalVector, right: SphericalVector) -> SphericalVector {
    [
        left[1] * right[2] - left[2] * right[1],
        left[2] * right[0] - left[0] * right[2],
        left[0] * right[1] - left[1] * right[0],
    ]
}

fn spherical_normalize(vector: SphericalVector) -> Option<SphericalVector> {
    let norm = spherical_dot(vector, vector).sqrt();
    (norm > 64.0 * f64::EPSILON).then(|| [vector[0] / norm, vector[1] / norm, vector[2] / norm])
}

fn lonlat_unit((lon, lat): (f64, f64)) -> SphericalVector {
    let lon = lon.to_radians();
    let lat = lat.to_radians();
    [lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin()]
}

fn spherical_angle(left: SphericalVector, right: SphericalVector) -> f64 {
    spherical_dot(left, right).clamp(-1.0, 1.0).acos()
}

fn point_on_minor_arc(
    point: SphericalVector,
    start: SphericalVector,
    end: SphericalVector,
) -> bool {
    (spherical_angle(start, point) + spherical_angle(point, end) - spherical_angle(start, end))
        .abs()
        <= 1.0e-9
}

fn open_ring_units(ring: &[(f64, f64)]) -> Vec<SphericalVector> {
    let mut units = ring.iter().copied().map(lonlat_unit).collect::<Vec<_>>();
    if units.len() > 1
        && spherical_dot(units[0], *units.last().expect("non-empty ring")) >= 1.0 - 1.0e-14
    {
        units.pop();
    }
    units
}

fn tangent_winding(point: SphericalVector, vertices: &[SphericalVector]) -> Option<f64> {
    let mut winding = 0.0;
    for index in 0..vertices.len() {
        let start = vertices[index];
        let end = vertices[(index + 1) % vertices.len()];
        let start_tangent = spherical_normalize([
            start[0] - point[0] * spherical_dot(point, start),
            start[1] - point[1] * spherical_dot(point, start),
            start[2] - point[2] * spherical_dot(point, start),
        ])?;
        let end_tangent = spherical_normalize([
            end[0] - point[0] * spherical_dot(point, end),
            end[1] - point[1] * spherical_dot(point, end),
            end[2] - point[2] * spherical_dot(point, end),
        ])?;
        winding += spherical_dot(point, spherical_cross(start_tangent, end_tangent))
            .atan2(spherical_dot(start_tangent, end_tangent));
    }
    Some(winding)
}

fn spherical_point_relation(point: (f64, f64), ring: &[(f64, f64)]) -> SphericalPointRelation {
    let vertices = open_ring_units(ring);
    if vertices.len() < 3 {
        return SphericalPointRelation::Outside;
    }
    let point = lonlat_unit(point);
    if (0..vertices.len()).any(|index| {
        point_on_minor_arc(
            point,
            vertices[index],
            vertices[(index + 1) % vertices.len()],
        )
    }) {
        return SphericalPointRelation::Boundary;
    }
    let classify = |probe| tangent_winding(probe, &vertices).map(|winding| winding.abs() > PI);
    let inside = classify(point).unwrap_or_else(|| {
        let seed = if point[2].abs() < 0.9 {
            [0.0, 0.0, 1.0]
        } else {
            [1.0, 0.0, 0.0]
        };
        let tangent = spherical_normalize(spherical_cross(seed, point))
            .expect("axis seed is not parallel to point");
        let perturbed = spherical_normalize([
            point[0] + 1.0e-10 * tangent[0],
            point[1] + 1.0e-10 * tangent[1],
            point[2] + 1.0e-10 * tangent[2],
        ])
        .expect("perturbed unit point");
        classify(perturbed).unwrap_or(false)
    });
    if inside {
        SphericalPointRelation::Inside
    } else {
        SphericalPointRelation::Outside
    }
}

fn minor_arcs_intersect_or_touch(
    left_start: SphericalVector,
    left_end: SphericalVector,
    right_start: SphericalVector,
    right_end: SphericalVector,
) -> bool {
    let intersections = spherical_cross(
        spherical_cross(left_start, left_end),
        spherical_cross(right_start, right_end),
    );
    if let Some(intersection) = spherical_normalize(intersections) {
        [
            intersection,
            [-intersection[0], -intersection[1], -intersection[2]],
        ]
        .into_iter()
        .any(|point| {
            point_on_minor_arc(point, left_start, left_end)
                && point_on_minor_arc(point, right_start, right_end)
        })
    } else {
        [left_start, left_end]
            .into_iter()
            .any(|point| point_on_minor_arc(point, right_start, right_end))
            || [right_start, right_end]
                .into_iter()
                .any(|point| point_on_minor_arc(point, left_start, left_end))
    }
}

fn spherical_rings_intersect_or_touch(left: &[(f64, f64)], right: &[(f64, f64)]) -> bool {
    let left = open_ring_units(left);
    let right = open_ring_units(right);
    (0..left.len()).any(|left_index| {
        (0..right.len()).any(|right_index| {
            minor_arcs_intersect_or_touch(
                left[left_index],
                left[(left_index + 1) % left.len()],
                right[right_index],
                right[(right_index + 1) % right.len()],
            )
        })
    })
}

fn bridge_hole(mut shell: Vec<(f64, f64)>, mut hole: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    if signed_area(&shell).signum() == signed_area(&hole).signum() {
        hole.reverse();
    }
    let hole_index = hole
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)))
        .map(|(index, _)| index)
        .unwrap_or(0);
    let hp = hole[hole_index];
    let ray_hit = shell
        .iter()
        .copied()
        .zip(shell.iter().copied().cycle().skip(1))
        .take(shell.len())
        .enumerate()
        .filter_map(|(index, (a, b))| {
            if (a.1 > hp.1) == (b.1 > hp.1) {
                return None;
            }
            let x = a.0 + (hp.1 - a.1) * (b.0 - a.0) / (b.1 - a.1);
            (x >= hp.0).then_some((index, x))
        })
        .min_by(|(_, a), (_, b)| a.total_cmp(b));
    let shell_index = if let Some((edge, x)) = ray_hit {
        let next = (edge + 1) % shell.len();
        let bridge = (x, hp.1);
        if squared_distance(shell[edge], bridge) < 1.0e-24 {
            edge
        } else if squared_distance(shell[next], bridge) < 1.0e-24 {
            next
        } else {
            shell.insert(edge + 1, bridge);
            edge + 1
        }
    } else {
        shell
            .iter()
            .enumerate()
            .min_by(|(_, a), (_, b)| {
                squared_distance(**a, hp).total_cmp(&squared_distance(**b, hp))
            })
            .map(|(index, _)| index)
            .unwrap_or(0)
    };
    let mut joined = Vec::with_capacity(shell.len() + hole.len() + 2);
    joined.extend_from_slice(&shell[..=shell_index]);
    joined.extend(hole[hole_index..].iter().copied());
    joined.extend(hole[..=hole_index].iter().copied());
    joined.push(shell[shell_index]);
    joined.extend(shell.drain(shell_index + 1..));
    joined
}

fn squared_distance(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)
}

fn ring_rightmost_x(ring: &[(f64, f64)]) -> f64 {
    ring.iter()
        .map(|point| point.0)
        .fold(f64::NEG_INFINITY, f64::max)
}

fn be_i32(bytes: &[u8], offset: usize) -> io::Result<i32> {
    let chunk = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "truncated i32"))?;
    Ok(i32::from_be_bytes(chunk.try_into().expect("four bytes")))
}

fn be_usize(bytes: &[u8], offset: usize) -> io::Result<usize> {
    usize::try_from(be_i32(bytes, offset)?)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative shapefile integer"))
}

fn le_i32(bytes: &[u8], offset: usize) -> io::Result<i32> {
    let chunk = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "truncated i32"))?;
    Ok(i32::from_le_bytes(chunk.try_into().expect("four bytes")))
}

fn le_usize(bytes: &[u8], offset: usize) -> io::Result<usize> {
    usize::try_from(le_i32(bytes, offset)?)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative shapefile integer"))
}

fn le_f64(bytes: &[u8], offset: usize) -> io::Result<f64> {
    let chunk = bytes
        .get(offset..offset + 8)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "truncated f64"))?;
    Ok(f64::from_le_bytes(chunk.try_into().expect("eight bytes")))
}

fn same_point(a: Option<&(f64, f64)>, b: Option<&(f64, f64)>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if (a.0 - b.0).abs() < 1e-12 && (a.1 - b.1).abs() < 1e-12)
}

fn signed_area(ring: &[(f64, f64)]) -> f64 {
    ring.iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
        .sum::<f64>()
        * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_supported_prj_families() {
        assert_eq!(
            detect_prj_crs(r#"GEOGCS["WGS 84",AUTHORITY["EPSG","4326"]]"#).unwrap(),
            ShapefileCrs::Wgs84
        );
        assert_eq!(
            detect_prj_crs(r#"PROJCS["WGS_1984_Web_Mercator_Auxiliary_Sphere"]"#).unwrap(),
            ShapefileCrs::WebMercator
        );
        assert_eq!(
            detect_prj_crs(r#"PROJCS["WGS_1984_UTM_Zone_50N"]"#).unwrap(),
            ShapefileCrs::Utm {
                zone: 50,
                north: true
            }
        );
    }

    #[test]
    fn converts_web_mercator_and_utm_to_wgs84() {
        let (lon, lat) = web_mercator_to_wgs84((WGS84_A * PI, 0.0)).unwrap();
        assert!((lon - 180.0).abs() < 1e-9);
        assert!(lat.abs() < 1e-9);

        let (lon, lat) = utm_to_wgs84((500_000.0, 0.0), 31, true).unwrap();
        assert!((lon - 3.0).abs() < 1e-8);
        assert!(lat.abs() < 1e-8);
    }

    #[test]
    fn converts_generic_wkt_projection_to_wgs84() {
        let wkt = concat!(
            r#"PROJCS["NAD83 / Massachusetts Mainland",GEOGCS["NAD83","#,
            r#"DATUM["North_American_Datum_1983",SPHEROID["GRS 1980",6378137,298.257222101]],"#,
            r#"PRIMEM["Greenwich",0],UNIT["degree",0.01745329251994328]],UNIT["metre",1],"#,
            r#"PROJECTION["Lambert_Conformal_Conic_2SP"],PARAMETER["standard_parallel_1",42.68333333333333],"#,
            r#"PARAMETER["standard_parallel_2",41.71666666666667],PARAMETER["latitude_of_origin",41],"#,
            r#"PARAMETER["central_meridian",-71.5],PARAMETER["false_easting",200000],"#,
            r#"PARAMETER["false_northing",750000]]"#,
        );
        let crs = detect_prj_crs(wkt).unwrap();
        assert!(matches!(crs, ShapefileCrs::Proj4(_)));
        let mut rings = vec![vec![
            (200_000.0, 750_000.0),
            (201_000.0, 750_000.0),
            (200_000.0, 751_000.0),
        ]];
        reproject_rings(&mut rings, &crs).unwrap();
        assert!((rings[0][0].0 + 71.5).abs() < 0.05);
        assert!((rings[0][0].1 - 41.0).abs() < 0.05);
    }

    #[test]
    fn projected_shapefile_coordinates_are_reprojected_exactly_once() {
        let root = std::env::temp_dir().join(format!(
            "earthmesh_project_projected_shp_once_{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let path = root.join("projected.shp");
        let mercator = |lon: f64, lat: f64| {
            (
                WGS84_A * lon.to_radians(),
                WGS84_A * (PI / 4.0 + lat.to_radians() / 2.0).tan().ln(),
            )
        };
        write_test_polygon_shp(
            &path,
            &[
                mercator(9.0, -1.0),
                mercator(11.0, -1.0),
                mercator(11.0, 1.0),
                mercator(9.0, 1.0),
            ],
        );
        fs::write(
            path.with_extension("prj"),
            r#"PROJCS["WGS_1984_Web_Mercator_Auxiliary_Sphere"]"#,
        )
        .unwrap();

        let components = read_shapefile_polygon_components(&path).unwrap();
        assert_eq!(components.len(), 1);
        let first = components[0].shell[0];
        assert!((first.0 - 9.0).abs() < 1.0e-9, "{first:?}");
        assert!((first.1 + 1.0).abs() < 1.0e-9, "{first:?}");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn preserves_holes_and_nested_islands_as_even_odd_polygons() {
        let shell = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let hole = vec![(2.0, 2.0), (2.0, 8.0), (8.0, 8.0), (8.0, 2.0)];
        let island = vec![(4.0, 4.0), (6.0, 4.0), (6.0, 6.0), (4.0, 6.0)];
        let polygons =
            polygon_components_from_records(vec![vec![hole, island, shell]], "test shapefile")
                .unwrap()
                .into_iter()
                .map(assemble_polygon_component)
                .collect::<io::Result<Vec<_>>>()
                .unwrap();
        assert_eq!(polygons.len(), 2);
        let bridged_shell = polygons.iter().max_by_key(|ring| ring.len()).unwrap();
        assert!(point_in_ring((1.0, 1.0), bridged_shell));
        assert!(!point_in_ring((3.0, 3.0), bridged_shell));
        assert!(polygons.iter().any(|ring| point_in_ring((5.0, 5.0), ring)));
    }

    #[test]
    fn antimeridian_holes_use_spherical_parity_and_a_local_planar_bridge() {
        let shell = vec![
            (170.0, -10.0),
            (-170.0, -10.0),
            (-170.0, 10.0),
            (170.0, 10.0),
        ];
        let hole = vec![(175.0, -5.0), (175.0, 5.0), (-175.0, 5.0), (-175.0, -5.0)];
        let components = polygon_components_from_records(vec![vec![hole, shell]], "seam").unwrap();
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].holes.len(), 1);
        let bridged = assemble_polygon_component(components[0].clone()).unwrap();
        assert!(point_in_ring((172.0, 0.0), &bridged));
        assert!(!point_in_ring((179.0, 0.0), &bridged));
        assert!(point_in_ring((-172.0, 0.0), &bridged));
    }

    #[test]
    fn polar_polygon_is_rejected_before_planar_close_lowering() {
        let component = ShapefilePolygonComponent {
            shell: vec![
                (-135.0, -80.0),
                (-45.0, -80.0),
                (45.0, -80.0),
                (135.0, -80.0),
            ],
            holes: Vec::new(),
        };

        let error = assemble_polygon_component(component)
            .expect_err("a polar cap cannot use the planar close-ring path");
        assert!(error.to_string().contains("polar SHP polygons"));
    }

    #[test]
    fn shapefile_records_union_while_crossing_parts_in_one_record_fail() {
        let outer = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let nested_record = vec![(2.0, 2.0), (8.0, 2.0), (8.0, 8.0), (2.0, 8.0)];
        let union = polygon_components_from_records(
            vec![vec![outer.clone()], vec![nested_record]],
            "records",
        )
        .unwrap();
        assert_eq!(union.len(), 2);
        assert!(union.iter().all(|component| component.holes.is_empty()));

        let crossing = vec![(4.0, -1.0), (8.0, -1.0), (8.0, 3.0), (4.0, 3.0)];
        let error = polygon_components_from_records(vec![vec![outer, crossing]], "crossing")
            .expect_err("crossing parts must not be guessed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("intersect or touch"));
    }

    fn write_test_polygon_shp(path: &Path, ring: &[(f64, f64)]) {
        let mut points = ring.to_vec();
        if points.first() != points.last() {
            points.push(points[0]);
        }
        let bounds = [
            points
                .iter()
                .map(|point| point.0)
                .fold(f64::INFINITY, f64::min),
            points
                .iter()
                .map(|point| point.1)
                .fold(f64::INFINITY, f64::min),
            points
                .iter()
                .map(|point| point.0)
                .fold(f64::NEG_INFINITY, f64::max),
            points
                .iter()
                .map(|point| point.1)
                .fold(f64::NEG_INFINITY, f64::max),
        ];
        let content_bytes = 48 + points.len() * 16;
        let file_bytes = 108 + content_bytes;
        let mut bytes = Vec::with_capacity(file_bytes);
        bytes.extend(9994_i32.to_be_bytes());
        bytes.extend([0_u8; 20]);
        bytes.extend(((file_bytes / 2) as i32).to_be_bytes());
        bytes.extend(1000_i32.to_le_bytes());
        bytes.extend(5_i32.to_le_bytes());
        for value in [bounds, [0.0; 4]].concat() {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend(1_i32.to_be_bytes());
        bytes.extend(((content_bytes / 2) as i32).to_be_bytes());
        bytes.extend(5_i32.to_le_bytes());
        for value in bounds {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend(1_i32.to_le_bytes());
        bytes.extend((points.len() as i32).to_le_bytes());
        bytes.extend(0_i32.to_le_bytes());
        for (x, y) in points {
            bytes.extend(x.to_le_bytes());
            bytes.extend(y.to_le_bytes());
        }
        fs::write(path, bytes).unwrap();
    }
}
