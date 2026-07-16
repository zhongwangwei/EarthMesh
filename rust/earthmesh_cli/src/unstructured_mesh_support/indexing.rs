use super::types::GridfileMeshPoints;
use crate::LonLatPoint;

/// Gridfile row identity has two independent dimensions: canonical-id mapping
/// and the first physical row. Compact Method-C output stores canonical id 1 in
/// row 0 as a sentinel, while older files can retain two explicit placeholder
/// rows where canonical ids equal row numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GridfileRowLayout {
    pub(crate) first_physical_row: usize,
    pub(crate) has_two_placeholder_rows: bool,
}

impl GridfileRowLayout {
    const fn compact(first_physical_row: usize) -> Self {
        Self {
            first_physical_row,
            has_two_placeholder_rows: false,
        }
    }

    const fn two_explicit_placeholders() -> Self {
        Self {
            first_physical_row: 2,
            has_two_placeholder_rows: true,
        }
    }

    pub(crate) fn is_physical_row(self, row: usize) -> bool {
        row >= self.first_physical_row
    }

    pub(crate) fn physical_row_for_canonical_id(self, id: i32, rows: usize) -> Option<usize> {
        let row = mesh_row_for_canonical_id(id, rows, self.has_two_placeholder_rows)?;
        self.is_physical_row(row).then_some(row)
    }

    pub(crate) fn canonical_id_for_physical_row(self, row: usize) -> Option<i32> {
        self.is_physical_row(row)
            .then(|| mesh_canonical_id_for_row(row, self.has_two_placeholder_rows))?
    }
}

pub(crate) fn gridfile_m_row_layout(mesh: &GridfileMeshPoints) -> GridfileRowLayout {
    let first_is_origin = coordinate_row_is_origin(&mesh.m_lon, &mesh.m_lat, 0);
    let first_is_sentinel = m_row_is_sentinel(&mesh.m_to_w, 0);
    let second_is_sentinel = m_row_is_sentinel(&mesh.m_to_w, 1);
    let two_origin_rows = gridfile_lonlat_has_two_placeholders(&mesh.m_lon, &mesh.m_lat);
    if two_origin_rows && first_is_sentinel && second_is_sentinel {
        return GridfileRowLayout::two_explicit_placeholders();
    }
    let compact_connectivity = first_is_sentinel
        || (mesh.m_to_w.is_empty()
            && authoritative_w_connectivity_references_id(mesh, mesh.m_lon.len()));
    if first_is_origin && compact_connectivity {
        return GridfileRowLayout::compact(1);
    }
    if two_origin_rows && mesh.m_to_w.is_empty() {
        return GridfileRowLayout::two_explicit_placeholders();
    }
    GridfileRowLayout::compact(0)
}

pub(crate) fn gridfile_w_row_layout(mesh: &GridfileMeshPoints) -> GridfileRowLayout {
    let first_is_origin = coordinate_row_is_origin(&mesh.w_lon, &mesh.w_lat, 0);
    let first_is_sentinel = w_row_is_sentinel(mesh, 0);
    let second_is_sentinel = w_row_is_sentinel(mesh, 1);
    let two_origin_rows = gridfile_lonlat_has_two_placeholders(&mesh.w_lon, &mesh.w_lat);
    if two_origin_rows && first_is_sentinel && second_is_sentinel {
        return GridfileRowLayout::two_explicit_placeholders();
    }
    let compact_connectivity = first_is_sentinel
        || (mesh.w_to_m.is_empty()
            && m_row_is_sentinel(&mesh.m_to_w, 0)
            && mesh
                .m_to_w
                .iter()
                .any(|&id| usize::try_from(id).ok() == Some(mesh.w_lon.len())));
    if first_is_origin && compact_connectivity {
        return GridfileRowLayout::compact(1);
    }
    if two_origin_rows {
        return GridfileRowLayout::two_explicit_placeholders();
    }
    GridfileRowLayout::compact(0)
}

pub(crate) fn gridfile_lonlat_has_two_placeholders(lon: &[f64], lat: &[f64]) -> bool {
    coordinate_row_is_origin(lon, lat, 0) && coordinate_row_is_origin(lon, lat, 1)
}

fn coordinate_row_is_origin(lon: &[f64], lat: &[f64], row: usize) -> bool {
    lon.get(row) == Some(&0.0) && lat.get(row) == Some(&0.0)
}

fn row_is_constant(row: &[i32]) -> bool {
    row.windows(2).all(|pair| pair[0] == pair[1])
}

fn m_row_is_sentinel(m_to_w: &[i32], row: usize) -> bool {
    let Some(start) = row.checked_mul(3) else {
        return false;
    };
    m_to_w.get(start..start + 3).is_some_and(row_is_constant)
}

fn w_row_is_sentinel(mesh: &GridfileMeshPoints, row: usize) -> bool {
    if !coordinate_row_is_origin(&mesh.w_lon, &mesh.w_lat, row) || mesh.w_to_m_width == 0 {
        return false;
    }
    let Some(count) = mesh
        .n_w
        .get(row)
        .and_then(|&value| usize::try_from(value).ok())
    else {
        return false;
    };
    let Some(start) = row.checked_mul(mesh.w_to_m_width) else {
        return false;
    };
    count <= 1
        && mesh
            .w_to_m
            .get(start..start + mesh.w_to_m_width)
            .is_some_and(row_is_constant)
}

fn authoritative_w_connectivity_references_id(mesh: &GridfileMeshPoints, id: usize) -> bool {
    let width = mesh.w_to_m_width;
    if width == 0
        || mesh.n_w.len() != mesh.w_lon.len()
        || mesh.w_to_m.len() != mesh.w_lon.len().saturating_mul(width)
    {
        return false;
    }
    mesh.n_w.iter().enumerate().any(|(row, &count)| {
        let Ok(count) = usize::try_from(count) else {
            return false;
        };
        count <= width
            && mesh.w_to_m[row * width..row * width + count]
                .iter()
                .any(|&candidate| usize::try_from(candidate).ok() == Some(id))
    })
}

pub(crate) fn mesh_row_for_canonical_id(
    id: i32,
    rows: usize,
    has_two_placeholder_rows: bool,
) -> Option<usize> {
    if id < 1 {
        return None;
    }
    let id = usize::try_from(id).ok()?;
    let row = if has_two_placeholder_rows {
        if id < 2 {
            return None;
        }
        id
    } else {
        id.checked_sub(1)?
    };
    (row < rows).then_some(row)
}

pub(crate) fn mesh_canonical_id_for_row(row: usize, has_two_placeholder_rows: bool) -> Option<i32> {
    if has_two_placeholder_rows {
        if row < 2 {
            return None;
        }
        i32::try_from(row).ok()
    } else {
        i32::try_from(row.checked_add(1)?).ok()
    }
}

pub(crate) fn mesh_points_have_two_placeholder_rows(points: &[LonLatPoint]) -> bool {
    points.len() > 2
        && points[0].lon == 0.0
        && points[0].lat == 0.0
        && points[1].lon == 0.0
        && points[1].lat == 0.0
}
