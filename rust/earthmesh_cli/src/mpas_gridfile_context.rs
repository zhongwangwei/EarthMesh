//! Exact producer-owned MPAS density inputs; no inferred or uniform fallback.
use std::{io, path::Path};

use crate::netcdf_to_io_error;

const WIDTH: &str = "earthmesh_w_cellwidth_km";
const NXP: &str = "earthmesh_mpas_base_nxp";
const STEP: &str = "earthmesh_mpas_step";
const REFERENCE: &str = "earthmesh_mpas_density_reference_width_km";
const SOURCE: &str = "earthmesh_mpas_cellwidth_source";
pub(crate) const HFIELD_QUANTIZED_DEMAND_V1: &str = "method_c_hfield_quantized_w_demand_v1";

#[derive(Clone, Debug, PartialEq)]
pub struct MpasGridfileContext {
    /// Same W row order as GLONW, including positive placeholder values.
    pub cellwidth_km: Vec<f64>,
    pub base_nxp: usize,
    pub step: usize,
    /// Original producer minimum, retained even if a crop removes the finest cells.
    pub density_reference_width_km: f64,
    /// Describes the actual producer, not a claim of equivalent backend levels.
    pub source: String,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

impl MpasGridfileContext {
    /// Nominal generation demand evaluated at final W sites, not realized
    /// geometry, birth levels or the legacy Spring U-edge targets. The complete
    /// field defines the reference even when no W site reaches its finest level.
    pub fn from_hfield_quantized_demand(
        mesh: &crate::UnstructuredMesh,
        hfield: &crate::hfield_gridfile_context::HfieldGridfileContext,
        base_nxp: usize,
    ) -> io::Result<Self> {
        hfield.validate()?;
        let finest = hfield
            .field
            .level_map(hfield.base_m, hfield.max_level)?
            .into_iter()
            .max()
            .ok_or_else(|| invalid("HField demand is empty"))?;
        let width_km = |level: u8| hfield.base_m / 2.0_f64.powi(i32::from(level)) / 1000.0;
        let reference = width_km(finest);
        let first =
            crate::unstructured_mesh_support::unstructured_w_row_layout(mesh).first_physical_row;
        if first >= mesh.w_points.len() {
            return Err(invalid("HField MPAS demand requires physical W cells"));
        }
        let mut cellwidth_km = vec![reference; mesh.w_points.len()];
        for (width, point) in cellwidth_km[first..]
            .iter_mut()
            .zip(&mesh.w_points[first..])
        {
            *width = width_km(hfield.field.try_level_at(
                point.lon,
                point.lat,
                hfield.base_m,
                hfield.max_level,
            )?);
        }
        let context = Self {
            cellwidth_km,
            base_nxp,
            step: usize::from(finest) + 1,
            density_reference_width_km: reference,
            source: HFIELD_QUANTIZED_DEMAND_V1.to_string(),
        };
        context.validate(mesh.w_points.len())?;
        Ok(context)
    }

    pub(crate) fn from_producer(
        mesh: &crate::UnstructuredMesh,
        cellwidth_km: Vec<f64>,
        base_nxp: usize,
        step: usize,
        source: &str,
    ) -> io::Result<Self> {
        let first =
            crate::unstructured_mesh_support::unstructured_w_row_layout(mesh).first_physical_row;
        let density_reference_width_km = cellwidth_km
            .get(first..)
            .unwrap_or(&[])
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let context = Self {
            cellwidth_km,
            base_nxp,
            step,
            density_reference_width_km,
            source: source.to_string(),
        };
        context.validate(mesh.w_points.len())?;
        Ok(context)
    }

    pub(crate) fn validate(&self, rows: usize) -> io::Result<()> {
        if self
            .source
            .starts_with("method_c_hfield_quantized_w_demand_")
            && (self.source != HFIELD_QUANTIZED_DEMAND_V1 || self.step > 6)
        {
            return Err(invalid(
                "unsupported HField MPAS demand version or level cap",
            ));
        }
        if self.cellwidth_km.len() != rows || rows == 0 {
            return Err(invalid("MPAS cellwidth must match every native W row"));
        }
        if self.base_nxp == 0
            || i32::try_from(self.base_nxp).is_err()
            || self.step == 0
            || self.step > usize::BITS as usize
        {
            return Err(invalid(
                "MPAS context requires positive i32 NXP and a representable step",
            ));
        }
        if self.source.trim().is_empty()
            || !self.density_reference_width_km.is_finite()
            || self.density_reference_width_km <= 0.0
            || self.cellwidth_km.iter().any(|&width| {
                !width.is_finite() || width <= 0.0 || width < self.density_reference_width_km
            })
        {
            return Err(invalid("MPAS context requires a source and finite positive widths not below the density reference"));
        }
        Ok(())
    }

    pub(crate) fn write_delivery_provenance(&self, path: &Path) -> io::Result<()> {
        let mut file = netcdf::append(path).map_err(netcdf_to_io_error)?;
        file.add_attribute(SOURCE, self.source.as_str())
            .map_err(netcdf_to_io_error)?;
        file.add_attribute(REFERENCE, self.density_reference_width_km)
            .map_err(netcdf_to_io_error)?;
        file.close().map_err(netcdf_to_io_error)
    }

    pub(crate) fn write(&self, file: &mut netcdf::FileMut) -> io::Result<()> {
        let mut var = file
            .add_variable::<f64>(WIDTH, &["lbx_points"])
            .map_err(netcdf_to_io_error)?;
        var.put_attribute("units", "km")
            .map_err(netcdf_to_io_error)?;
        var.put_values(&self.cellwidth_km, ..)
            .map_err(netcdf_to_io_error)?;
        file.add_attribute(NXP, self.base_nxp as i32)
            .map_err(netcdf_to_io_error)?;
        file.add_attribute(STEP, self.step as i32)
            .map_err(netcdf_to_io_error)?;
        file.add_attribute(REFERENCE, self.density_reference_width_km)
            .map_err(netcdf_to_io_error)?;
        file.add_attribute(SOURCE, self.source.as_str())
            .map_err(netcdf_to_io_error)?;
        Ok(())
    }
}

/// Missing context is distinct from malformed/partial context. Never consult sidecars.
pub fn read_mpas_gridfile_context(
    gridfile: impl AsRef<Path>,
) -> io::Result<Option<MpasGridfileContext>> {
    let file = crate::open_netcdf(gridfile.as_ref()).map_err(netcdf_to_io_error)?;
    let Some(var) = file.variable(WIDTH) else {
        if [NXP, STEP, REFERENCE, SOURCE]
            .iter()
            .any(|name| file.attribute(name).is_some())
        {
            return Err(invalid("MPAS context attributes exist without W cellwidth"));
        }
        return Ok(None);
    };
    if var.dimensions().len() != 1
        || var.dimensions()[0].name() != "lbx_points"
        || var.vartype() != netcdf::types::NcVariableType::Float(netcdf::types::FloatType::F64)
    {
        return Err(invalid(
            "MPAS cellwidth must be f64 on the native lbx_points dimension",
        ));
    }
    if !matches!(var.attribute_value("units").transpose().map_err(netcdf_to_io_error)?, Some(netcdf::AttributeValue::Str(unit)) if unit == "km")
    {
        return Err(invalid("MPAS cellwidth units must be km"));
    }
    let attribute = |name| -> io::Result<netcdf::AttributeValue> {
        file.attribute(name)
            .ok_or_else(|| invalid(format!("MPAS context missing {name}")))?
            .value()
            .map_err(netcdf_to_io_error)
    };
    let positive_int = |name| -> io::Result<usize> {
        match attribute(name)? {
            netcdf::AttributeValue::Int(value) if value > 0 => Ok(value as usize),
            _ => Err(invalid(format!(
                "MPAS context {name} must be a positive integer"
            ))),
        }
    };
    let context = MpasGridfileContext {
        cellwidth_km: var.get_values(..).map_err(netcdf_to_io_error)?,
        base_nxp: positive_int(NXP)?,
        step: positive_int(STEP)?,
        density_reference_width_km: match attribute(REFERENCE)? {
            netcdf::AttributeValue::Double(value) => value,
            _ => return Err(invalid("MPAS density reference must be f64")),
        },
        source: match attribute(SOURCE)? {
            netcdf::AttributeValue::Str(value) => value,
            _ => return Err(invalid("MPAS cellwidth source must be text")),
        },
    };
    context.validate(var.dimensions()[0].len())?;
    Ok(Some(context))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LonLatPoint, UnstructuredMesh};

    #[test]
    fn producer_reference_excludes_only_topological_placeholders() {
        for placeholders in 0..=2 {
            let mut mesh = UnstructuredMesh {
                m_points: vec![],
                m_to_w: vec![],
                w_points: vec![LonLatPoint { lon: 0.0, lat: 0.0 }; placeholders],
                w_to_m: vec![vec![0; 3]; placeholders],
                n_w_to_m: vec![0; placeholders],
            };
            // A real polygon at the origin must still contribute its minimum width.
            mesh.w_points.extend([
                LonLatPoint { lon: 0.0, lat: 0.0 },
                LonLatPoint {
                    lon: 10.0,
                    lat: 10.0,
                },
            ]);
            mesh.w_to_m.extend([vec![1, 2, 3], vec![2, 3, 4]]);
            mesh.n_w_to_m.extend([3, 3]);
            let mut widths = vec![100.0; placeholders];
            widths.extend([25.0, 50.0]);
            let context = MpasGridfileContext::from_producer(&mesh, widths, 4, 2, "test").unwrap();
            assert_eq!(
                crate::unstructured_mesh_support::unstructured_w_row_layout(&mesh)
                    .first_physical_row,
                placeholders,
            );
            assert_eq!(context.density_reference_width_km, 25.0);
        }
    }
}
