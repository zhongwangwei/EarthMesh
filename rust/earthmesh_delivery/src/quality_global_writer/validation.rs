use std::io;

use super::types::{GlobalQualityMesh, QualityClassMetrics};

pub(super) fn validate_global_quality_mesh(quality: &GlobalQualityMesh) -> io::Result<()> {
    validate_quality_class_metrics("sjx", &quality.sjx, 3)?;
    validate_quality_class_metrics("wbx", &quality.wbx, 5)?;
    validate_quality_class_metrics("lbx", &quality.lbx, 6)?;
    if let Some(qbx) = &quality.qbx {
        validate_quality_class_metrics("qbx", qbx, 7)?;
    }
    Ok(())
}

fn validate_quality_class_metrics(
    class_name: &str,
    metrics: &QualityClassMetrics,
    width: usize,
) -> io::Result<()> {
    let rows = metrics.length.len();
    if metrics.angle.len() != rows {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{class_name} angle row count {} must match length row count {rows}",
                metrics.angle.len()
            ),
        ));
    }
    for (name, actual) in [("less", metrics.less.len()), ("more", metrics.more.len())] {
        if actual != rows {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{class_name} {name} length {actual} must match row count {rows}"),
            ));
        }
    }
    for (idx, row) in metrics.length.iter().enumerate() {
        if row.len() != width {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{class_name} length row {idx} width {} must match required {width}",
                    row.len()
                ),
            ));
        }
    }
    for (idx, row) in metrics.angle.iter().enumerate() {
        if row.len() != width {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{class_name} angle row {idx} width {} must match required {width}",
                    row.len()
                ),
            ));
        }
    }
    Ok(())
}
