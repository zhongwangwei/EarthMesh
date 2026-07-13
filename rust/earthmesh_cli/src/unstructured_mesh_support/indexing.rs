use crate::LonLatPoint;

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

pub(super) fn mesh_points_have_two_placeholder_rows(points: &[LonLatPoint]) -> bool {
    points.len() > 2
        && points[0].lon == 0.0
        && points[0].lat == 0.0
        && points[1].lon == 0.0
        && points[1].lat == 0.0
}
