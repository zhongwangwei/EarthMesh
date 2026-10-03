//! Exact producer-owned MPAS density inputs; no inferred or uniform fallback.
use std::{io, path::Path};

use crate::netcdf_to_io_error;

const WIDTH: &str = "earthmesh_w_cellwidth_km";
const NXP: &str = "earthmesh_mpas_base_nxp";
const STEP: &str = "earthmesh_mpas_step";
const REFERENCE: &str = "earthmesh_mpas_density_reference_width_km";
const SOURCE: &str = "earthmesh_mpas_cellwidth_source";
pub const LEPP_RESOLVED_REGION_DEMAND_V1: &str = "lepp_resolved_region_w_demand_v1";
pub const ADAPTIVE_REGION_PASS_DEMAND_V1: &str = "adaptive_region_pass_w_demand_v1";
pub const HFIELD_QUANTIZED_DEMAND_V1: &str = "method_c_hfield_quantized_w_demand_v1";
const GRIDINIT_UNIFORM_BASE: &str = "gridinit_uniform_base";
const SPRING_GLOBAL_DISTANCE_LAYERS: &str = "spring_global_distance_layers";
const CERTIFIED_DELIVERED_LEVELS: &str = "cmrc_delivered_w_levels";

/// Where a mesh's MPAS cell widths came from, as the gridfile records it in
/// `earthmesh_mpas_cellwidth_source`.
///
/// The output layer reads widths from any producer the same way; what it needs
/// to know about the producer is only whether the widths are a nominal demand
/// (then the MPAS mesh takes the producer's reference as `nominalMinDc`) and,
/// for a versioned demand, whether this version reads it. A file written by
/// another version or tool keeps what it recorded, verbatim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MpasWidthSource {
    /// The uniform base mesh: every cell the base NXP's width.
    GridinitUniformBase,
    /// The global spring's distance layers.
    SpringGlobalDistanceLayers,
    /// Method-C's h-field, quantized to levels (version 1).
    HfieldQuantizedDemandV1,
    /// The point+radius route's per-pass regions (version 1).
    AdaptiveRegionPassDemandV1,
    /// LEPP's resolved region targets (version 1).
    LeppResolvedRegionDemandV1,
    /// CMRC's delivered W levels.
    CertifiedDeliveredLevels,
    /// A source this version does not produce, kept as recorded.
    Recorded(String),
}

impl MpasWidthSource {
    pub fn parse(text: &str) -> Self {
        match text {
            GRIDINIT_UNIFORM_BASE => Self::GridinitUniformBase,
            SPRING_GLOBAL_DISTANCE_LAYERS => Self::SpringGlobalDistanceLayers,
            HFIELD_QUANTIZED_DEMAND_V1 => Self::HfieldQuantizedDemandV1,
            ADAPTIVE_REGION_PASS_DEMAND_V1 => Self::AdaptiveRegionPassDemandV1,
            LEPP_RESOLVED_REGION_DEMAND_V1 => Self::LeppResolvedRegionDemandV1,
            CERTIFIED_DELIVERED_LEVELS => Self::CertifiedDeliveredLevels,
            other => Self::Recorded(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::GridinitUniformBase => GRIDINIT_UNIFORM_BASE,
            Self::SpringGlobalDistanceLayers => SPRING_GLOBAL_DISTANCE_LAYERS,
            Self::HfieldQuantizedDemandV1 => HFIELD_QUANTIZED_DEMAND_V1,
            Self::AdaptiveRegionPassDemandV1 => ADAPTIVE_REGION_PASS_DEMAND_V1,
            Self::LeppResolvedRegionDemandV1 => LEPP_RESOLVED_REGION_DEMAND_V1,
            Self::CertifiedDeliveredLevels => CERTIFIED_DELIVERED_LEVELS,
            Self::Recorded(text) => text,
        }
    }

    /// Widths that are a nominal demand: the MPAS mesh takes the producer's
    /// reference width as its `nominalMinDc`.
    pub fn is_nominal_demand(&self) -> bool {
        matches!(
            self,
            Self::HfieldQuantizedDemandV1
                | Self::AdaptiveRegionPassDemandV1
                | Self::LeppResolvedRegionDemandV1
        )
    }

    /// Why this version cannot read the widths, if it cannot: a versioned
    /// demand it does not know, or a known one at a level beyond its cap.
    fn unsupported(&self, step: usize) -> Option<&'static str> {
        const HFIELD: &str = "unsupported HField MPAS demand version or level cap";
        const ADAPTIVE: &str = "unsupported adaptive region-pass MPAS demand version or level cap";
        const LEPP: &str = "unsupported LEPP resolved-region MPAS demand version or level cap";
        match self {
            Self::HfieldQuantizedDemandV1 => (step > 6).then_some(HFIELD),
            Self::AdaptiveRegionPassDemandV1 => (step > 6).then_some(ADAPTIVE),
            Self::LeppResolvedRegionDemandV1 => (step > 6).then_some(LEPP),
            Self::Recorded(text) if text.starts_with("method_c_hfield_quantized_w_demand_") => {
                Some(HFIELD)
            }
            Self::Recorded(text) if text.starts_with("adaptive_region_pass_w_demand_") => {
                Some(ADAPTIVE)
            }
            Self::Recorded(text) if text.starts_with("lepp_resolved_region_w_demand_") => {
                Some(LEPP)
            }
            _ => None,
        }
    }
}

impl From<&str> for MpasWidthSource {
    fn from(text: &str) -> Self {
        Self::parse(text)
    }
}

impl From<String> for MpasWidthSource {
    fn from(text: String) -> Self {
        Self::parse(&text)
    }
}

impl PartialEq<&str> for MpasWidthSource {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl std::fmt::Display for MpasWidthSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MpasGridfileContext {
    /// Same W row order as GLONW, including positive placeholder values.
    pub cellwidth_km: Vec<f64>,
    pub base_nxp: usize,
    pub step: usize,
    /// Original producer minimum, retained even if a crop removes the finest cells.
    pub density_reference_width_km: f64,
    /// Describes the actual producer, not a claim of equivalent backend levels.
    pub source: MpasWidthSource,
}

pub fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

impl MpasGridfileContext {
    pub fn from_producer(
        mesh: &crate::UnstructuredMesh,
        cellwidth_km: Vec<f64>,
        base_nxp: usize,
        step: usize,
        source: MpasWidthSource,
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
            source,
        };
        context.validate(mesh.w_points.len())?;
        Ok(context)
    }

    pub fn validate(&self, rows: usize) -> io::Result<()> {
        if let Some(why) = self.source.unsupported(self.step) {
            return Err(invalid(why));
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
        if self.source.as_str().trim().is_empty()
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

    pub fn write_delivery_provenance(&self, path: &Path) -> io::Result<()> {
        let mut file = netcdf::append(path).map_err(netcdf_to_io_error)?;
        file.add_attribute(SOURCE, self.source.as_str())
            .map_err(netcdf_to_io_error)?;
        file.add_attribute(REFERENCE, self.density_reference_width_km)
            .map_err(netcdf_to_io_error)?;
        file.close().map_err(netcdf_to_io_error)
    }

    pub fn write(&self, file: &mut netcdf::FileMut) -> io::Result<()> {
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
            netcdf::AttributeValue::Str(value) => MpasWidthSource::parse(&value),
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
            let context =
                MpasGridfileContext::from_producer(&mesh, widths, 4, 2, "test".into()).unwrap();
            assert_eq!(
                crate::unstructured_mesh_support::unstructured_w_row_layout(&mesh)
                    .first_physical_row,
                placeholders,
            );
            assert_eq!(context.density_reference_width_km, 25.0);
        }
    }
}
