//! Exact consumed spherical demand, not realized W widths or MPAS density.
use crate::netcdf_to_io_error;
use earthmesh_hfield::HField;
use std::{io, path::Path};

const TARGET: &str = "earthmesh_hfield_target_m";
const NLON: &str = "earthmesh_hfield_nlon";
const NLAT: &str = "earthmesh_hfield_nlat";
const BASE: &str = "earthmesh_hfield_base_m";
const LEVEL: &str = "earthmesh_hfield_max_level";
const SEMANTICS: &str = "earthmesh_hfield_semantics";
// v1 uses HField's south-to-north cell centres, longitude-fast raster,
// periodic longitude/polar-continuous bilinear sample and level_at quantization.
// Bump the semantic version if these sampling/quantization rules change.
const VERSION: &str = "spherical_effective_demand_v1";

#[derive(Clone, Debug)]
pub struct HfieldGridfileContext {
    pub field: HField,
    pub base_m: f64,
    pub max_level: u8,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

impl HfieldGridfileContext {
    pub(crate) fn validate(&self) -> io::Result<()> {
        if !self.base_m.is_finite() || self.base_m <= 0.0 || !(1..=5).contains(&self.max_level) {
            return Err(invalid(
                "HField demand requires positive finite base_m and max_level in 1..=5",
            ));
        }
        if self
            .field
            .values()
            .iter()
            .any(|v| !v.is_finite() || *v <= 0.0)
        {
            return Err(invalid("HField demand sizes must be positive and finite"));
        }
        Ok(())
    }

    pub(crate) fn write(&self, file: &mut netcdf::FileMut) -> io::Result<()> {
        file.add_dimension(NLON, self.field.nlon())
            .map_err(netcdf_to_io_error)?;
        file.add_dimension(NLAT, self.field.nlat())
            .map_err(netcdf_to_io_error)?;
        let mut var = file
            .add_variable::<f64>(TARGET, &[NLAT, NLON])
            .map_err(netcdf_to_io_error)?;
        var.put_attribute("units", "m")
            .map_err(netcdf_to_io_error)?;
        var.put_values(self.field.values(), (.., ..))
            .map_err(netcdf_to_io_error)?;
        file.add_attribute(BASE, self.base_m)
            .map_err(netcdf_to_io_error)?;
        file.add_attribute(LEVEL, i32::from(self.max_level))
            .map_err(netcdf_to_io_error)?;
        file.add_attribute(SEMANTICS, VERSION)
            .map_err(netcdf_to_io_error)?;
        Ok(())
    }
}

/// Restore the same HField and level_at inputs consumed by the producer. The
/// whole source raster survives crops; it is not compacted with native M/W rows.
pub fn read_hfield_gridfile_context(
    path: impl AsRef<Path>,
) -> io::Result<Option<HfieldGridfileContext>> {
    let file = crate::open_netcdf(path.as_ref()).map_err(netcdf_to_io_error)?;
    let Some(var) = file.variable(TARGET) else {
        if [BASE, LEVEL, SEMANTICS]
            .iter()
            .any(|name| file.attribute(name).is_some())
            || [NLAT, NLON]
                .iter()
                .any(|name| file.dimension(name).is_some())
        {
            return Err(invalid(
                "partial HField demand context without target raster",
            ));
        }
        return Ok(None);
    };
    let dims = var.dimensions();
    if dims.len() != 2
        || dims[0].name() != NLAT
        || dims[1].name() != NLON
        || var.vartype() != netcdf::types::NcVariableType::Float(netcdf::types::FloatType::F64)
    {
        return Err(invalid(
            "HField demand must be f64 on [earthmesh_hfield_nlat, earthmesh_hfield_nlon]",
        ));
    }
    if !matches!(var.attribute_value("units").transpose().map_err(netcdf_to_io_error)?, Some(netcdf::AttributeValue::Str(unit)) if unit == "m")
    {
        return Err(invalid("HField demand units must be m"));
    }
    let attribute = |name| -> io::Result<netcdf::AttributeValue> {
        file.attribute(name)
            .ok_or_else(|| invalid("incomplete HField demand attributes"))?
            .value()
            .map_err(netcdf_to_io_error)
    };
    if !matches!(attribute(SEMANTICS)?, netcdf::AttributeValue::Str(tag) if tag == VERSION) {
        return Err(invalid("unknown HField demand semantics"));
    }
    let base_m = match attribute(BASE)? {
        netcdf::AttributeValue::Double(value) if value.is_finite() && value > 0.0 => value,
        _ => return Err(invalid("HField base_m must be positive finite f64")),
    };
    let max_level = match attribute(LEVEL)? {
        netcdf::AttributeValue::Int(value) if (1..=5).contains(&value) => value as u8,
        _ => return Err(invalid("HField max_level must be an integer in 1..=5")),
    };
    let (nlat, nlon) = (dims[0].len(), dims[1].len());
    if nlon < 4
        || nlat < 2
        || nlon
            .checked_mul(nlat)
            .is_none_or(|len| len > isize::MAX as usize / std::mem::size_of::<f64>())
    {
        return Err(invalid("invalid HField raster dimensions"));
    }
    let context = HfieldGridfileContext {
        field: HField::from_values(
            nlon,
            nlat,
            var.get_values((.., ..)).map_err(netcdf_to_io_error)?,
        )?,
        base_m,
        max_level,
    };
    context.validate()?;
    Ok(Some(context))
}
