use std::collections::BTreeMap;

/// All `-?\d+` integers on a line (faithful to the regex in refinement_eval.py).
fn line_integers(line: &str) -> Vec<i64> {
    let bytes = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let neg = bytes[i] == b'-';
        let start = i;
        let digit_start = if neg { i + 1 } else { i };
        let mut j = digit_start;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j > digit_start {
            if let Ok(v) = line[start..j].parse::<i64>() {
                out.push(v);
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Faithful port of `refinement_eval.py::parse_refinement_log`.
pub fn parse_refinement_log(log_text: &str) -> BTreeMap<String, BTreeMap<String, i64>> {
    let mut result: BTreeMap<String, BTreeMap<String, i64>> = BTreeMap::new();
    let mut current: Option<String> = None;
    for line in log_text.lines() {
        let nums = line_integers(line);
        let last = nums.last().copied();
        let norm = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if line.contains("refine_degree") {
            if let Some(d) = last {
                current = Some(d.to_string());
                result.entry(d.to_string()).or_default();
            }
        } else if line.contains("需要细化的三角形个数") {
            if let (Some(d), Some(v)) = (&current, last) {
                result
                    .entry(d.clone())
                    .or_default()
                    .insert("selected_triangles".into(), v);
            }
        } else if norm.contains("before num_ref") {
            if let (Some(d), Some(v)) = (&current, last) {
                result
                    .entry(d.clone())
                    .or_default()
                    .insert("before_nested_cleanup_triangles".into(), v);
            }
        } else if norm.contains("after num_ref") {
            if let (Some(d), Some(v)) = (&current, last) {
                result
                    .entry(d.clone())
                    .or_default()
                    .insert("after_nested_cleanup_triangles".into(), v);
            }
        } else if line.contains("去除孤立细化三角形后") {
            if let (Some(d), Some(v)) = (&current, last) {
                result
                    .entry(d.clone())
                    .or_default()
                    .insert("retained_triangles".into(), v);
            }
        }
    }
    result
}
