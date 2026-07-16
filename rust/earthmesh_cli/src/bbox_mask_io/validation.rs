use std::io;

use super::types::{BBoxMask, BBoxMesh, BBoxPoint};

pub(super) fn validate_bbox_mesh(mesh: &BBoxMesh) -> io::Result<()> {
    validate_bbox_points(&mesh.points)
}

pub(crate) fn validate_bbox_mask(mask: &BBoxMask) -> io::Result<()> {
    validate_bbox_points(&mask.points)
}

pub(crate) fn validate_bbox_mask_geographic(mask: &BBoxMask) -> io::Result<()> {
    validate_bbox_points(&mask.points)?;
    for (index, point) in mask.points.iter().enumerate() {
        if !(-90.0..=90.0).contains(&point.north) || !(-90.0..=90.0).contains(&point.south) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "bbox point {} latitudes must be within [-90, 90] degrees",
                    index + 1
                ),
            ));
        }
    }
    Ok(())
}

fn validate_bbox_points(points: &[BBoxPoint]) -> io::Result<()> {
    for (index, point) in points.iter().enumerate() {
        if !point.west.is_finite()
            || !point.east.is_finite()
            || !point.north.is_finite()
            || !point.south.is_finite()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bbox point {} coordinates must be finite", index + 1),
            ));
        }
        if point.north < point.south {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bbox point {} north must be >= south", index + 1),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bbox_mask_defers_latitude_range_to_geographic_context() {
        let mask = BBoxMask {
            refine_degree: 1,
            points: vec![BBoxPoint {
                west: 170.0,
                east: -170.0,
                north: 91.0,
                south: -10.0,
            }],
        };

        validate_bbox_mask(&mask).expect("Cartesian bbox coordinates are not latitudes");
        let error = validate_bbox_mask_geographic(&mask)
            .expect_err("invalid geographic latitude must fail");
        assert!(error.to_string().contains("[-90, 90]"));
    }
}
