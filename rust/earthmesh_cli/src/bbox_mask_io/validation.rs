use std::io;

use super::types::{BBoxMask, BBoxMesh};

pub(super) fn validate_bbox_mesh(mesh: &BBoxMesh) -> io::Result<()> {
    for (index, point) in mesh.points.iter().enumerate() {
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
        if point.west > point.east {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bbox point {} west must be <= east", index + 1),
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

pub(crate) fn validate_bbox_mask(mask: &BBoxMask) -> io::Result<()> {
    validate_bbox_mesh(&BBoxMesh {
        points: mask.points.clone(),
    })
}
