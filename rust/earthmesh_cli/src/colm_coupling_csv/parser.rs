use std::fs;
use std::io;
use std::path::Path;

use super::row::ColmCouplingCsvRow;

const REQUIRED_COLUMNS: [&str; 14] = [
    "cell_id",
    "cell_index",
    "center_lon",
    "center_lat",
    "surface_class",
    "has_river",
    "river_class",
    "river_fraction",
    "estimated_river_area_m2",
    "has_coast",
    "coast_class",
    "coastal_fraction",
    "normalized_cell_area_m2",
    "source_areaCell",
];

pub(crate) fn read_colm_coupling_csv_rows(
    input_csv: impl AsRef<Path>,
) -> io::Result<Vec<ColmCouplingCsvRow>> {
    let text = fs::read_to_string(input_csv)?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "CoLM coupling CSV is empty"))?;
    let columns = split_simple_csv_line(header);
    validate_required_columns(&columns)?;
    let mut rows = Vec::new();
    for (line_index, line) in lines.enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let values = split_simple_csv_line(line);
        let line_number = line_index + 2;
        if values.len() != columns.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "CoLM coupling CSV line {line_number} has {} fields, expected {}",
                    values.len(),
                    columns.len()
                ),
            ));
        }
        rows.push(ColmCouplingCsvRow {
            cell_id: parse_csv_text(&columns, &values, "cell_id", line_number)?.to_string(),
            cell_index: parse_csv_i32(&columns, &values, "cell_index", line_number)?,
            center_lon: parse_csv_f64(&columns, &values, "center_lon", line_number)?,
            center_lat: parse_csv_f64(&columns, &values, "center_lat", line_number)?,
            surface_class: parse_csv_text(&columns, &values, "surface_class", line_number)?
                .to_string(),
            has_river: parse_csv_bool(&columns, &values, "has_river", line_number)?,
            river_class: csv_field(&columns, &values, "river_class", line_number)?.to_string(),
            river_fraction: parse_csv_f64(&columns, &values, "river_fraction", line_number)?,
            estimated_river_area_m2: parse_csv_optional_f64(
                &columns,
                &values,
                "estimated_river_area_m2",
                line_number,
            )?,
            has_coast: parse_csv_bool(&columns, &values, "has_coast", line_number)?,
            coast_class: csv_field(&columns, &values, "coast_class", line_number)?.to_string(),
            coastal_fraction: parse_csv_f64(&columns, &values, "coastal_fraction", line_number)?,
            normalized_cell_area_m2: parse_csv_f64(
                &columns,
                &values,
                "normalized_cell_area_m2",
                line_number,
            )?,
            source_area_cell: parse_csv_f64(&columns, &values, "source_areaCell", line_number)?,
        });
    }
    Ok(rows)
}

fn split_simple_csv_line(line: &str) -> Vec<String> {
    line.split(',')
        .map(|part| part.trim().to_string())
        .collect()
}

fn validate_required_columns(columns: &[String]) -> io::Result<()> {
    for required in REQUIRED_COLUMNS {
        let count = columns.iter().filter(|column| column == &required).count();
        match count {
            1 => {}
            0 => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("CoLM coupling CSV is missing required column '{required}'"),
                ))
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("CoLM coupling CSV repeats required column '{required}'"),
                ))
            }
        }
    }
    Ok(())
}

fn csv_field<'a>(
    columns: &[String],
    values: &'a [String],
    name: &str,
    line_number: usize,
) -> io::Result<&'a str> {
    columns
        .iter()
        .position(|column| column == name)
        .and_then(|index| values.get(index))
        .map(String::as_str)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{name} is missing on CSV line {line_number}"),
            )
        })
}

fn parse_csv_text<'a>(
    columns: &[String],
    values: &'a [String],
    name: &str,
    line_number: usize,
) -> io::Result<&'a str> {
    let value = csv_field(columns, values, name, line_number)?;
    if value.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} is empty on CSV line {line_number}"),
        ));
    }
    Ok(value)
}

fn parse_csv_i32(
    columns: &[String],
    values: &[String],
    name: &str,
    line_number: usize,
) -> io::Result<i32> {
    csv_field(columns, values, name, line_number)?
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
    let value = csv_field(columns, values, name, line_number)?;
    let parsed = value.parse::<f64>().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} on CSV line {line_number} must be numeric"),
        )
    })?;
    if !parsed.is_finite() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} on CSV line {line_number} must be finite"),
        ));
    }
    Ok(parsed)
}

fn parse_csv_optional_f64(
    columns: &[String],
    values: &[String],
    name: &str,
    line_number: usize,
) -> io::Result<f64> {
    if csv_field(columns, values, name, line_number)?.is_empty() {
        Ok(f64::NAN)
    } else {
        parse_csv_f64(columns, values, name, line_number)
    }
}

fn parse_csv_bool(
    columns: &[String],
    values: &[String],
    name: &str,
    line_number: usize,
) -> io::Result<bool> {
    match csv_field(columns, values, name, line_number)? {
        value if value.eq_ignore_ascii_case("true") || value == "1" => Ok(true),
        value if value.eq_ignore_ascii_case("false") || value == "0" => Ok(false),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{name} on CSV line {line_number} must be true/false or 1/0"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{read_colm_coupling_csv_rows, REQUIRED_COLUMNS};
    use std::fs;

    fn write_temp(name: &str, contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "earthmesh_colm_csv_{name}_{}_{}.csv",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn coupling_csv_rejects_missing_columns_instead_of_synthesizing_nan() {
        let path = write_temp("missing", "cell_id,cell_index\nc1,1\n");
        let error = read_colm_coupling_csv_rows(&path).unwrap_err();
        assert!(
            error.to_string().contains("missing required column"),
            "{error}"
        );
        let _ = fs::remove_file(path);
    }

    #[test]
    fn coupling_csv_rejects_empty_nonfinite_and_invalid_boolean_values() {
        let header = REQUIRED_COLUMNS.join(",");
        let valid = "c1,1,100,20,LAND,false,none,0,0,false,none,0,1,1";
        for (label, row, expected) in [
            (
                "empty",
                valid.replace(",100,", ",,"),
                "center_lon on CSV line 2 must be numeric",
            ),
            (
                "nan",
                valid.replace(",100,", ",NaN,"),
                "center_lon on CSV line 2 must be finite",
            ),
            (
                "bool",
                valid.replace(",false,none,0,0,", ",maybe,none,0,0,"),
                "has_river on CSV line 2 must be true/false",
            ),
        ] {
            let path = write_temp(label, &format!("{header}\n{row}\n"));
            let error = read_colm_coupling_csv_rows(&path).unwrap_err();
            assert!(error.to_string().contains(expected), "{error}");
            let _ = fs::remove_file(path);
        }
    }
}
