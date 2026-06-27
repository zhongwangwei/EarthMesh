pub(super) fn flatten_m_to_w(m_to_w: &[[i32; 3]]) -> Vec<i32> {
    let mut values = Vec::with_capacity(m_to_w.len() * 3);
    for row in m_to_w {
        values.extend_from_slice(row);
    }
    values
}

pub(super) fn flatten_w_to_m(w_to_m: &[Vec<i32>], dimc: usize) -> Vec<i32> {
    let mut values = Vec::with_capacity(w_to_m.len() * dimc);
    for row in w_to_m {
        values.extend(row.iter().copied().take(dimc));
        values.resize(values.len() + dimc.saturating_sub(row.len().min(dimc)), 0);
    }
    values
}

pub(super) fn trim_trailing_zero_connectivity(row: &[i32]) -> Vec<i32> {
    let end = row
        .iter()
        .rposition(|&value| value != 0)
        .map(|idx| idx + 1)
        .unwrap_or(row.len());
    row[..end].to_vec()
}
