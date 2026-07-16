use std::io;

/// CaMa binary grid geometry for native Rust hydro/coast preprocessing readers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CamaBinaryGridSpec {
    pub nx: usize,
    pub ny: usize,
    pub west: f64,
    pub south: f64,
    pub grid_size_deg: f64,
    pub little_endian: bool,
    pub y_reversed_storage: bool,
}

impl CamaBinaryGridSpec {
    pub fn validate(&self) -> io::Result<()> {
        if self.nx == 0 || self.ny == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "CaMa grid dimensions must be positive",
            ));
        }
        if self.nx > i32::MAX as usize || self.ny > i32::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "CaMa grid dimensions exceed the safe nextxy/isize index range",
            ));
        }
        if !self.west.is_finite()
            || !self.south.is_finite()
            || !self.grid_size_deg.is_finite()
            || self.grid_size_deg <= 0.0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "CaMa west, south, and grid_size_deg must be finite and grid_size_deg must be positive",
            ));
        }
        let lon_span = self.nx as f64 * self.grid_size_deg;
        let lat_span = self.ny as f64 * self.grid_size_deg;
        let east = self.west + lon_span;
        let north = self.south + lat_span;
        let tolerance = self.grid_size_deg.abs().max(1.0) * 1.0e-9;
        if !lon_span.is_finite()
            || !lat_span.is_finite()
            || !east.is_finite()
            || !north.is_finite()
            || lon_span > 360.0 + tolerance
            || self.south < -90.0 - tolerance
            || north > 90.0 + tolerance
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "CaMa grid geometry must describe a finite Earth lon/lat extent",
            ));
        }
        Ok(())
    }

    /// Longitude at the center of a zero-based CaMa x index.
    pub fn lon_center(&self, x_index: usize) -> f64 {
        self.west + (x_index as f64 + 0.5) * self.grid_size_deg
    }

    /// Latitude at the center of a zero-based CaMa y index.
    pub fn lat_center(&self, y_index: usize) -> f64 {
        self.south + (y_index as f64 + 0.5) * self.grid_size_deg
    }

    /// Return a logical south-to-north window for a lon/lat bounding box.
    pub fn window_for_bbox(
        &self,
        west: f64,
        east: f64,
        south: f64,
        north: f64,
    ) -> io::Result<CamaBinaryWindow> {
        self.validate()?;
        if !west.is_finite()
            || !east.is_finite()
            || !south.is_finite()
            || !north.is_finite()
            || west >= east
            || south >= north
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "CaMa bbox bounds must be finite and ordered west < east, south < north",
            ));
        }
        let x0 = ((west - self.west) / self.grid_size_deg)
            .floor()
            .clamp(0.0, self.nx as f64) as usize;
        let x1 = ((east - self.west) / self.grid_size_deg)
            .ceil()
            .clamp(0.0, self.nx as f64) as usize;
        let y0 = ((south - self.south) / self.grid_size_deg)
            .floor()
            .clamp(0.0, self.ny as f64) as usize;
        let y1 = ((north - self.south) / self.grid_size_deg)
            .ceil()
            .clamp(0.0, self.ny as f64) as usize;
        if x1 <= x0 || y1 <= y0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "bbox does not overlap grid",
            ));
        }
        Ok(CamaBinaryWindow {
            x_start: x0,
            y_start: y0,
            width: x1 - x0,
            height: y1 - y0,
        })
    }

    pub(crate) fn storage_y_index(&self, logical_y_index: usize) -> usize {
        if self.y_reversed_storage {
            self.ny - 1 - logical_y_index
        } else {
            logical_y_index
        }
    }
}

/// Rectangular logical CaMa binary window in zero-based x/y indices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CamaBinaryWindow {
    pub x_start: usize,
    pub y_start: usize,
    pub width: usize,
    pub height: usize,
}
