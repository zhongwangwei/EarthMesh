use std::io;

use crate::{
    validate_lonlat, validate_positive_distance, MethodCRefinementRegion,
    METHOD_C_MIN_GRID_SPACING_METERS,
};

impl MethodCRefinementRegion {
    pub fn validate(&self) -> io::Result<()> {
        let level = self.level();
        if !(1..=5).contains(&level) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Method-C refinement level {level} must be in 1..=5"),
            ));
        }
        match self {
            Self::Circle {
                center,
                radius_meters,
                ..
            } => {
                validate_lonlat(*center)?;
                validate_method_c_radius("circle radius", *radius_meters)?;
            }
            Self::Bbox {
                west_degrees,
                east_degrees,
                south_degrees,
                north_degrees,
                ..
            } => {
                if !west_degrees.is_finite()
                    || !east_degrees.is_finite()
                    || !south_degrees.is_finite()
                    || !north_degrees.is_finite()
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "bbox coordinates must be finite",
                    ));
                }
                if *south_degrees < -90.0
                    || *south_degrees > 90.0
                    || *north_degrees < -90.0
                    || *north_degrees > 90.0
                    || south_degrees > north_degrees
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "bbox latitude bounds are invalid",
                    ));
                }
            }
            Self::Corridor {
                points,
                radius_meters,
                ..
            } => {
                if points.len() < 2 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "corridor refinement requires at least two points",
                    ));
                }
                if radius_meters.len() != points.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "corridor refinement requires one radius per point",
                    ));
                }
                for &point in points {
                    validate_lonlat(point)?;
                }
                for &radius in radius_meters {
                    validate_method_c_radius("corridor radius", radius)?;
                }
            }
            Self::Polygon { points, .. } => {
                if points.len() < 3 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "polygon refinement requires at least three points",
                    ));
                }
                for &point in points {
                    validate_lonlat(point)?;
                }
            }
        }
        Ok(())
    }

    pub fn validate_cartesian_xy(&self) -> io::Result<()> {
        let level = self.level();
        if !(1..=5).contains(&level) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Method-C refinement level {level} must be in 1..=5"),
            ));
        }
        match self {
            Self::Circle {
                center,
                radius_meters,
                ..
            } => {
                if !center.lon_degrees.is_finite() || !center.lat_degrees.is_finite() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "circle Cartesian coordinates must be finite",
                    ));
                }
                validate_method_c_radius("circle radius", *radius_meters)?;
            }
            Self::Corridor {
                points,
                radius_meters,
                ..
            } => {
                if points.len() < 2 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "corridor refinement requires at least two points",
                    ));
                }
                if radius_meters.len() != points.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "corridor refinement requires one radius per point",
                    ));
                }
                for point in points {
                    if !point.lon_degrees.is_finite() || !point.lat_degrees.is_finite() {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "corridor Cartesian coordinates must be finite",
                        ));
                    }
                }
                for &radius in radius_meters {
                    validate_method_c_radius("corridor radius", radius)?;
                }
            }
            Self::Bbox {
                west_degrees,
                east_degrees,
                south_degrees,
                north_degrees,
                ..
            } => {
                if !west_degrees.is_finite()
                    || !east_degrees.is_finite()
                    || !south_degrees.is_finite()
                    || !north_degrees.is_finite()
                    || west_degrees > east_degrees
                    || south_degrees > north_degrees
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "Cartesian bbox bounds must be finite and ordered",
                    ));
                }
            }
            Self::Polygon { points, .. } => {
                if points.len() < 3 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "polygon refinement requires at least three points",
                    ));
                }
                if points
                    .iter()
                    .any(|point| !point.lon_degrees.is_finite() || !point.lat_degrees.is_finite())
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "polygon Cartesian coordinates must be finite",
                    ));
                }
            }
        }
        Ok(())
    }
}

pub(crate) fn validate_method_c_radius(name: &str, value: f64) -> io::Result<()> {
    validate_positive_distance(name, value)?;
    if value < METHOD_C_MIN_GRID_SPACING_METERS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{name} must be at least {METHOD_C_MIN_GRID_SPACING_METERS} to match Canonical Method-C dzxmin"
            ),
        ));
    }
    Ok(())
}
