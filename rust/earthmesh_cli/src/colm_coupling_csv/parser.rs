use std::fs;
use std::io;
use std::path::Path;

use super::row::ColmCouplingCsvRow;

pub(crate) fn read_colm_coupling_csv_rows(
    input_csv: impl AsRef<Path>,
) -> io::Result<Vec<ColmCouplingCsvRow>> {
    let text = fs::read_to_string(input_csv)?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "CoLM coupling CSV is empty"))?;
    let columns = split_simple_csv_line(header);
    let mut rows = Vec::new();
    for (line_index, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let values = split_simple_csv_line(line);
        rows.push(ColmCouplingCsvRow {
            cell_id: csv_field(&columns, &values, "cell_id").to_string(),
            cell_index: parse_csv_i32(&columns, &values, "cell_index", line_index + 2)?,
            center_lon: parse_csv_f64(&columns, &values, "center_lon", line_index + 2)?,
            center_lat: parse_csv_f64(&columns, &values, "center_lat", line_index + 2)?,
            surface_class: csv_field(&columns, &values, "surface_class").to_string(),
            has_river: parse_csv_bool(csv_field(&columns, &values, "has_river")),
            river_class: csv_field(&columns, &values, "river_class").to_string(),
            river_fraction: parse_csv_f64(&columns, &values, "river_fraction", line_index + 2)?,
            estimated_river_area_m2: parse_csv_f64(
                &columns,
                &values,
                "estimated_river_area_m2",
                line_index + 2,
            )?,
            has_coast: parse_csv_bool(csv_field(&columns, &values, "has_coast")),
            coast_class: csv_field(&columns, &values, "coast_class").to_string(),
            coastal_fraction: parse_csv_f64(&columns, &values, "coastal_fraction", line_index + 2)?,
            normalized_cell_area_m2: parse_csv_f64(
                &columns,
                &values,
                "normalized_cell_area_m2",
                line_index + 2,
            )?,
            source_area_cell: parse_csv_f64(&columns, &values, "source_areaCell", line_index + 2)?,
        });
    }
    Ok(rows)
}

fn split_simple_csv_line(line: &str) -> Vec<String> {
    line.split(',')
        .map(|part| part.trim().to_string())
        .collect()
}

fn csv_field<'a>(columns: &[String], values: &'a [String], name: &str) -> &'a str {
    columns
        .iter()
        .position(|column| column == name)
        .and_then(|index| values.get(index))
        .map(String::as_str)
        .unwrap_or("")
}

fn parse_csv_i32(
    columns: &[String],
    values: &[String],
    name: &str,
    line_number: usize,
) -> io::Result<i32> {
    csv_field(columns, values, name)
        .parse::<i32>()
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{name} on CSV line {line_number} must be an integer"),
            )
        })
}

fn parse_csv_f64(
    columns: &[String],
    values: &[String],
    name: &str,
    line_number: usize,
) -> io::Result<f64> {
    let value = csv_field(columns, values, name);
    if value.is_empty() {
        return Ok(f64::NAN);
    }
    value.parse::<f64>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} on CSV line {line_number} must be numeric"),
        )
    })
}

fn parse_csv_bool(value: &str) -> bool {
    value.eq_ignore_ascii_case("true") || value == "1"
}
