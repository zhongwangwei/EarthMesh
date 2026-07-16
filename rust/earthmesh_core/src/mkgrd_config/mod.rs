use crate::{
    canonical_quote, namelist_assignments, parse_canonical_bool, parse_canonical_string, parse_f64,
    parse_i32,
};

/// Typed equivalent of `consts_coms:oname_vars` defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct EarthmeshConfig {
    pub experiment_name: String,
    pub runtype: String,
    pub nxp: i32,
    pub base_dir: String,
    pub mesh_type: String,
    pub mode_grid: String,
    pub mode_file_description: String,
    pub mode_file: String,
    pub refine: bool,
    pub openmp: i32,
    pub niter: i32,
    pub gridnum_perdegree: i32,
    pub mask_sea_ratio: f64,
    pub beta: f64,
    pub relax: f64,
    pub isolated_ocean: bool,
    pub mask_restart: bool,
    pub mask_domain_type: String,
    /// Optional close-boundary preprocessing carried as a compact engine spec.
    /// `polyline` preserves compatibility behavior and is omitted by the writer.
    pub mask_domain_close_boundary: String,
    pub landtype_file: String,
    pub mask_domain_fprefix: String,
    pub mask_domain_global: bool,
    pub mask_patch_on: bool,
    pub mask_patch_type: String,
    pub mask_patch_fprefix: String,
    pub output_format: String,
    pub coupling_fraction_method: String,
    pub coupling_identify_coastline: bool,
    pub coupling_identify_river_mouth: bool,
    pub coupling_cama_root: String,
}

impl Default for EarthmeshConfig {
    fn default() -> Self {
        Self {
            experiment_name: "/tmp".to_string(),
            runtype: " ".to_string(),
            nxp: 0,
            base_dir: " /tmp".to_string(),
            mesh_type: "/tmp".to_string(),
            mode_grid: "/tmp".to_string(),
            mode_file_description: "/tmp".to_string(),
            mode_file: " /tmp".to_string(),
            refine: false,
            openmp: 16,
            niter: 5000,
            gridnum_perdegree: 120,
            mask_sea_ratio: 0.5,
            beta: 1.2,
            relax: 0.04,
            isolated_ocean: false,
            mask_restart: false,
            mask_domain_type: "/tmp".to_string(),
            mask_domain_close_boundary: "polyline".to_string(),
            landtype_file: "/tmp".to_string(),
            mask_domain_fprefix: "/tmp".to_string(),
            mask_domain_global: true,
            mask_patch_on: false,
            mask_patch_type: "/tmp".to_string(),
            mask_patch_fprefix: "/tmp".to_string(),
            output_format: "/tmp".to_string(),
            coupling_fraction_method: "point_sample".to_string(),
            coupling_identify_coastline: false,
            coupling_identify_river_mouth: false,
            coupling_cama_root: String::new(),
        }
    }
}

impl EarthmeshConfig {
    /// Derive `file_dir = trim(base_dir) // trim(expnme) // '/'` as in
    /// `mkgrd.F90:read_nl`.
    pub fn file_dir(&self) -> String {
        format!(
            "{}{}/",
            self.base_dir.trim_end(),
            self.experiment_name.trim()
        )
    }

    /// Parse the Canonical `/mkgrd/ NL` namelist shape consumed by
    /// `mkgrd.F90:read_nl` into the typed Rust configuration.
    ///
    /// This is intentionally non-destructive: it mirrors assignment parsing and
    /// validation, but does not create/remove the working directories that the
    /// Canonical driver manages after `read_nl`.
    pub fn from_mkgrd_namelist(input: &str) -> Result<Self, String> {
        let mut config = Self::default();
        for assignment in namelist_assignments(input, "mkgrd")? {
            let field = assignment.field.as_str();
            let value = assignment.value.as_str();

            match field.to_ascii_lowercase().as_str() {
                "expnme" => config.experiment_name = parse_canonical_string(value),
                "runtype" => config.runtype = parse_canonical_string(value),
                "nxp" => config.nxp = parse_i32(field, value)?,
                "base_dir" => config.base_dir = parse_canonical_string(value),
                "mesh_type" => config.mesh_type = parse_canonical_string(value),
                "mode_grid" => config.mode_grid = parse_canonical_string(value),
                "mode_file_description" => {
                    config.mode_file_description = parse_canonical_string(value)
                }
                "mode_file" => config.mode_file = parse_canonical_string(value),
                "refine" => config.refine = parse_canonical_bool(field, value)?,
                "openmp" => config.openmp = parse_i32(field, value)?,
                "niter" => config.niter = parse_i32(field, value)?,
                "gridnum_perdegree" => config.gridnum_perdegree = parse_i32(field, value)?,
                "mask_sea_ratio" => config.mask_sea_ratio = parse_f64(field, value)?,
                "beta" => config.beta = parse_f64(field, value)?,
                "relax" => config.relax = parse_f64(field, value)?,
                "isolated_ocean" => config.isolated_ocean = parse_canonical_bool(field, value)?,
                "mask_restart" => config.mask_restart = parse_canonical_bool(field, value)?,
                "mask_domain_type" => config.mask_domain_type = parse_canonical_string(value),
                "mask_domain_close_boundary" => {
                    config.mask_domain_close_boundary = parse_canonical_string(value)
                }
                "landtype_file" => config.landtype_file = parse_canonical_string(value),
                "mask_domain_fprefix" => config.mask_domain_fprefix = parse_canonical_string(value),
                "mask_domain_global" => {
                    config.mask_domain_global = parse_canonical_bool(field, value)?
                }
                "mask_patch_on" => config.mask_patch_on = parse_canonical_bool(field, value)?,
                "mask_patch_type" => config.mask_patch_type = parse_canonical_string(value),
                "mask_patch_fprefix" => config.mask_patch_fprefix = parse_canonical_string(value),
                "output_format" => config.output_format = parse_canonical_string(value),
                "coupling_fraction_method" => {
                    config.coupling_fraction_method = parse_canonical_string(value)
                }
                "coupling_identify_coastline" => {
                    config.coupling_identify_coastline = parse_canonical_bool(field, value)?
                }
                "coupling_identify_river_mouth" => {
                    config.coupling_identify_river_mouth = parse_canonical_bool(field, value)?
                }
                "coupling_cama_root" => config.coupling_cama_root = parse_canonical_string(value),
                _ if is_native_method_c_field(field) => {}
                _ => return Err(format!("unknown &mkgrd field '{field}'")),
            }
        }

        config.validate_like_read_nl()?;
        Ok(config)
    }

    /// Serialize the configuration back into the `&mkgrd` namelist block that
    /// `from_mkgrd_namelist` consumes. The round-trip
    /// `from_mkgrd_namelist(&x.to_mkgrd_namelist())` reproduces `x`.
    pub fn to_mkgrd_namelist(&self) -> String {
        fn flag(value: bool) -> &'static str {
            if value {
                ".TRUE."
            } else {
                ".FALSE."
            }
        }
        fn q(value: &str) -> String {
            canonical_quote(value)
        }

        let mut out = String::new();
        out.push_str("&mkgrd\n");
        out.push_str(&format!("  NL%EXPNME = {}\n", q(&self.experiment_name)));
        out.push_str(&format!("  NL%runtype = {}\n", q(&self.runtype)));
        out.push_str(&format!("  NL%base_dir = {}\n", q(&self.base_dir)));
        out.push_str(&format!("  NL%mesh_type = {}\n", q(&self.mesh_type)));
        out.push_str(&format!("  NL%mode_grid = {}\n", q(&self.mode_grid)));
        out.push_str(&format!("  NL%mode_file = {}\n", q(&self.mode_file)));
        out.push_str(&format!(
            "  NL%mode_file_description = {}\n",
            q(&self.mode_file_description)
        ));
        out.push_str(&format!("  NL%NXP = {}\n", self.nxp));
        out.push_str(&format!("  NL%refine = {}\n", flag(self.refine)));
        out.push_str(&format!(
            "  NL%gridnum_perdegree = {}\n",
            self.gridnum_perdegree
        ));
        out.push_str(&format!("  NL%niter = {}\n", self.niter));
        out.push_str(&format!("  NL%beta = {}\n", self.beta));
        out.push_str(&format!("  NL%relax = {}\n", self.relax));
        out.push_str(&format!("  NL%openmp = {}\n", self.openmp));
        out.push_str(&format!(
            "  NL%landtype_file = {}\n",
            q(&self.landtype_file)
        ));
        out.push_str(&format!(
            "  NL%mask_domain_global = {}\n",
            flag(self.mask_domain_global)
        ));
        out.push_str(&format!(
            "  NL%mask_domain_type = {}\n",
            q(&self.mask_domain_type)
        ));
        if self.mask_domain_close_boundary.trim() != "polyline" {
            out.push_str(&format!(
                "  NL%mask_domain_close_boundary = {}\n",
                q(&self.mask_domain_close_boundary)
            ));
        }
        out.push_str(&format!(
            "  NL%mask_domain_fprefix = {}\n",
            q(&self.mask_domain_fprefix)
        ));
        out.push_str(&format!(
            "  NL%mask_restart = {}\n",
            flag(self.mask_restart)
        ));
        out.push_str(&format!("  NL%mask_sea_ratio = {}\n", self.mask_sea_ratio));
        out.push_str(&format!(
            "  NL%mask_patch_on = {}\n",
            flag(self.mask_patch_on)
        ));
        out.push_str(&format!(
            "  NL%mask_patch_type = {}\n",
            q(&self.mask_patch_type)
        ));
        out.push_str(&format!(
            "  NL%mask_patch_fprefix = {}\n",
            q(&self.mask_patch_fprefix)
        ));
        out.push_str(&format!(
            "  NL%isolated_ocean = {}\n",
            flag(self.isolated_ocean)
        ));
        out.push_str(&format!(
            "  NL%output_format = {}\n",
            q(&self.output_format)
        ));
        if self.mesh_type.trim() == "LOCmesh" {
            out.push_str(&format!(
                "  NL%coupling_fraction_method = {}\n",
                q(&self.coupling_fraction_method)
            ));
            out.push_str(&format!(
                "  NL%coupling_identify_coastline = {}\n",
                flag(self.coupling_identify_coastline)
            ));
            out.push_str(&format!(
                "  NL%coupling_identify_river_mouth = {}\n",
                flag(self.coupling_identify_river_mouth)
            ));
            if !self.coupling_cama_root.trim().is_empty() {
                out.push_str(&format!(
                    "  NL%coupling_cama_root = {}\n",
                    q(&self.coupling_cama_root)
                ));
            }
        }
        out.push_str("/\n");
        out
    }

    fn validate_like_read_nl(&self) -> Result<(), String> {
        match self.gridnum_perdegree {
            120 | 240 => {}
            other => {
                return Err(format!(
                    "gridnum_perdegree must be 120 or 240 like mkgrd.F90:read_nl, got {other}"
                ));
            }
        }

        match (self.mesh_type.as_str(), self.output_format.as_str()) {
            ("landmesh", "CoLM")
            | ("earthmesh", "CoLM")
            | ("oceanmesh", "FVCOM")
            | ("atmos", "MPAS")
            | ("atmos", "MPAS-Simple")
            | ("atmosmesh", "MPAS")
            | ("atmosmesh", "MPAS-Simple")
            | ("LOCmesh", "CoLM") => Ok(()),
            ("landmesh", _) => Err(format!(
                "landmesh output_format must be CoLM like mkgrd.F90:read_nl, got {}",
                self.output_format
            )),
            ("earthmesh", _) => Err(format!(
                "earthmesh output_format must be CoLM like mkgrd.F90:read_nl, got {}",
                self.output_format
            )),
            ("oceanmesh", _) => Err(format!(
                "oceanmesh output_format must be FVCOM like mkgrd.F90:read_nl, got {}",
                self.output_format
            )),
            ("atmos" | "atmosmesh", _) => Err(format!(
                "atmos/atmosmesh output_format must be MPAS or MPAS-Simple like mkgrd.F90:read_nl, got {}",
                self.output_format
            )),
            ("LOCmesh", _) => Err(format!(
                "LOCmesh output_format must be CoLM like mkgrd.F90:read_nl, got {}",
                self.output_format
            )),
            (mesh_type, _) => Err(format!(
                "unsupported mesh_type {mesh_type} like mkgrd.F90:read_nl"
            )),
        }
    }
}

pub(crate) fn is_native_method_c_field(field: &str) -> bool {
    let base = field
        .split_once('(')
        .map_or(field, |(base, _)| base)
        .trim()
        .to_ascii_lowercase();
    matches!(
        base.as_str(),
        "mdomain"
            | "deltax"
            | "ngrids"
            | "ngrdll"
            | "grdrad"
            | "grdlat"
            | "grdlon"
            | "gridplot_base"
            | "nsfcgrids"
            | "nsfcgrdll"
            | "sfcgrdrad"
            | "sfcgrdlat"
            | "sfcgrdlon"
            | "sfcgridplot_base"
            | "sfcgrid_res_factor"
    )
}
