//! Rust-native core constants and typed configuration migrated from
//! `src/consts_coms.F90`.
//!
//! The goal of this crate is to remove hidden Fortran module-global state from
//! downstream mesh kernels while preserving the exact defaults and formulas that
//! existing EarthMesh workflows rely on.

/// Maximum number of remote send/receive processes in the original Fortran module.
pub const MAX_REMOTE: usize = 30;

/// Maximum path length used by the original Fortran character buffers.
pub const PATH_LEN: usize = 256;

/// Earth radius used by `mkgrd.F90:init_consts`, matching MPAS.
pub const EARTH_RADIUS_METERS: f64 = 6_371_229.0;

/// Maximum number of non-parallel M/V/W loops in `mem_ijtabs`.
pub const MLOOPS: usize = 7;
pub const NLOOPS_M: usize = MLOOPS + MAX_REMOTE;
pub const NLOOPS_V: usize = MLOOPS + MAX_REMOTE;
pub const NLOOPS_W: usize = MLOOPS + MAX_REMOTE;

pub const JTM_GRID: usize = 1;
pub const JTU_GRID: usize = 1;
pub const JTV_GRID: usize = 1;
pub const JTW_GRID: usize = 1;
pub const JTM_INIT: usize = 2;
pub const JTU_INIT: usize = 2;
pub const JTV_INIT: usize = 2;
pub const JTW_INIT: usize = 2;
pub const JTM_PROG: usize = 3;
pub const JTU_PROG: usize = 3;
pub const JTV_PROG: usize = 3;
pub const JTW_PROG: usize = 3;
pub const JTM_WADJ: usize = 4;
pub const JTU_WADJ: usize = 4;
pub const JTV_WADJ: usize = 4;
pub const JTW_WADJ: usize = 4;
pub const JTM_WSTN: usize = 5;
pub const JTU_WSTN: usize = 5;
pub const JTV_WSTN: usize = 5;
pub const JTW_WSTN: usize = 5;
pub const JTM_LBCP: usize = 6;
pub const JTU_LBCP: usize = 6;
pub const JTV_LBCP: usize = 6;
pub const JTW_LBCP: usize = 6;
pub const JTM_VADJ: usize = 7;
pub const JTU_WALL: usize = 7;
pub const JTV_WALL: usize = 7;
pub const JTW_VADJ: usize = 7;

/// Radians per degree: `atan(1.0_r8) / 45.0_r8` in Fortran.
pub const PIO180: f64 = std::f64::consts::PI / 180.0;

/// Degrees per radian: `45.0_r8 / atan(1.0_r8)` in Fortran.
pub const PIU180: f64 = 180.0 / std::f64::consts::PI;

/// Full turn in radians: `8.0_r8 * atan(1.0_r8)` in Fortran.
pub const PI2: f64 = 2.0 * std::f64::consts::PI;

/// Convert degrees to radians using the migrated Fortran conversion constant.
#[inline]
pub fn deg_to_rad(degrees: f64) -> f64 {
    degrees * PIO180
}

/// Convert radians to degrees using the migrated Fortran conversion constant.
#[inline]
pub fn rad_to_deg(radians: f64) -> f64 {
    radians * PIU180
}

/// Rust-owned replacement for `mem_grid` coordinate arrays.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GridMemory {
    pub nma: usize,
    pub nua: usize,
    pub nva: usize,
    pub nwa: usize,
    pub mma: usize,
    pub mua: usize,
    pub mva: usize,
    pub mwa: usize,
    pub xem: Vec<f32>,
    pub yem: Vec<f32>,
    pub zem: Vec<f32>,
    pub xew: Vec<f32>,
    pub yew: Vec<f32>,
    pub zew: Vec<f32>,
    pub glatm: Vec<f32>,
    pub glonm: Vec<f32>,
    pub glatw: Vec<f32>,
    pub glonw: Vec<f32>,
}

impl GridMemory {
    /// Match `mem_grid:alloc_xyzem`: allocate M-point Cartesian arrays and zero-fill.
    pub fn allocate_xyzem(&mut self, lma: usize) {
        self.xem = vec![0.0; lma];
        self.yem = vec![0.0; lma];
        self.zem = vec![0.0; lma];
    }

    /// Match `mem_grid:alloc_xyzew`: allocate W-point Cartesian arrays and zero-fill.
    pub fn allocate_xyzew(&mut self, lwa: usize) {
        self.xew = vec![0.0; lwa];
        self.yew = vec![0.0; lwa];
        self.zew = vec![0.0; lwa];
    }

    /// Match `mem_grid:alloc_grid_lonlatmw`: allocate M/W lon-lat arrays and zero-fill.
    pub fn allocate_grid_lonlatmw(&mut self, lma: usize, _lva: usize, lwa: usize) {
        self.glatw = vec![0.0; lwa];
        self.glonw = vec![0.0; lwa];
        self.glatm = vec![0.0; lma];
        self.glonm = vec![0.0; lma];
    }
}

/// Rust equivalent of `mem_ijtabs:itab_m_vars`.
#[derive(Debug, Clone, PartialEq)]
pub struct ItabM {
    pub loop_flags: Vec<bool>,
    pub npoly: i32,
    pub imp: i32,
    pub imglobe: i32,
    pub mrlm: i32,
    pub mrlm_orig: i32,
    pub mrow: i32,
    pub ngr: i32,
    pub iv: [i32; 3],
    pub iw: [i32; 3],
}

impl Default for ItabM {
    fn default() -> Self {
        Self {
            loop_flags: vec![false; MLOOPS],
            npoly: 0,
            imp: 1,
            imglobe: 1,
            mrlm: 0,
            mrlm_orig: 0,
            mrow: 0,
            ngr: 0,
            iv: [1; 3],
            iw: [1; 3],
        }
    }
}

/// Rust equivalent of `mem_ijtabs:itab_v_vars`.
#[derive(Debug, Clone, PartialEq)]
pub struct ItabV {
    pub loop_flags: Vec<bool>,
    pub ivp: i32,
    pub irank: i32,
    pub ivglobe: i32,
    pub mrlv: i32,
    pub im: [i32; 6],
    pub iw: [i32; 4],
    pub iv: [i32; 4],
}

impl Default for ItabV {
    fn default() -> Self {
        Self {
            loop_flags: vec![false; MLOOPS],
            ivp: 1,
            irank: -1,
            ivglobe: 1,
            mrlv: 0,
            im: [1; 6],
            iw: [1; 4],
            iv: [1; 4],
        }
    }
}

/// Rust equivalent of `mem_ijtabs:itab_w_vars`.
#[derive(Debug, Clone, PartialEq)]
pub struct ItabW {
    pub loop_flags: Vec<bool>,
    pub npoly: i32,
    pub iwp: i32,
    pub irank: i32,
    pub iwglobe: i32,
    pub mrlw: i32,
    pub mrlw_orig: i32,
    pub ngr: i32,
    pub im: [i32; 7],
    pub iv: [i32; 7],
    pub iw: [i32; 7],
    pub dirv: [f32; 7],
}

impl Default for ItabW {
    fn default() -> Self {
        Self {
            loop_flags: vec![false; MLOOPS],
            npoly: 0,
            iwp: 1,
            irank: -1,
            iwglobe: 1,
            mrlw: 0,
            mrlw_orig: 0,
            ngr: 0,
            im: [1; 7],
            iv: [1; 7],
            iw: [1; 7],
            dirv: [0.0; 7],
        }
    }
}

/// Allocated `mem_ijtabs` state.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IjTabs {
    pub m: Vec<ItabM>,
    pub v: Vec<ItabV>,
    pub w: Vec<ItabW>,
}

impl IjTabs {
    /// Match `mem_ijtabs:alloc_itabs`: allocate records and false loop flags.
    pub fn allocate(mma: usize, mva: usize, mwa: usize) -> Self {
        Self {
            m: vec![ItabM::default(); mma],
            v: vec![ItabV::default(); mva],
            w: vec![ItabW::default(); mwa],
        }
    }
}

/// Rust equivalent of `mem_delaunay:itab_md_vars`.
#[derive(Debug, Clone, PartialEq)]
pub struct ItabMd {
    pub loop_flags: [bool; MLOOPS],
    pub npoly: i32,
    pub imp: i32,
    pub mrlm: i32,
    pub mrlm_orig: i32,
    pub ngr: i32,
    pub im: [i32; 7],
    pub iu: [i32; 7],
    pub iw: [i32; 7],
}

impl Default for ItabMd {
    fn default() -> Self {
        Self {
            loop_flags: [false; MLOOPS],
            npoly: 0,
            imp: 1,
            mrlm: 0,
            mrlm_orig: 0,
            ngr: 0,
            im: [1; 7],
            iu: [1; 7],
            iw: [1; 7],
        }
    }
}

/// Rust equivalent of `mem_delaunay:itab_ud_vars`.
#[derive(Debug, Clone, PartialEq)]
pub struct ItabUd {
    pub loop_flags: [bool; MLOOPS],
    pub iup: i32,
    pub mrlu: i32,
    pub im: [i32; 2],
    pub iu: [i32; 12],
    pub iw: [i32; 6],
}

impl Default for ItabUd {
    fn default() -> Self {
        Self {
            loop_flags: [false; MLOOPS],
            iup: 1,
            mrlu: 0,
            im: [1; 2],
            iu: [1; 12],
            iw: [1; 6],
        }
    }
}

/// Rust equivalent of `mem_delaunay:itab_wd_vars`.
#[derive(Debug, Clone, PartialEq)]
pub struct ItabWd {
    pub loop_flags: [bool; MLOOPS],
    pub npoly: i32,
    pub iwp: i32,
    pub mrlw: i32,
    pub mrlw_orig: i32,
    pub mrow: i32,
    pub ngr: i32,
    pub im: [i32; 3],
    pub iu: [i32; 3],
    pub iw: [i32; 9],
}

impl Default for ItabWd {
    fn default() -> Self {
        Self {
            loop_flags: [false; MLOOPS],
            npoly: 0,
            iwp: 1,
            mrlw: 0,
            mrlw_orig: 0,
            mrow: 0,
            ngr: 0,
            im: [1; 3],
            iu: [1; 3],
            iw: [1; 9],
        }
    }
}

/// Rust equivalent of `mem_delaunay:nest_ud_vars`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NestUd {
    pub im: i32,
    pub iu: i32,
}

/// Rust equivalent of `mem_delaunay:nest_wd_vars`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NestWd {
    pub iu: [i32; 3],
    pub iw: [i32; 3],
}

/// Allocated `mem_delaunay` state and copy/original buffers.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DelaunayMemory {
    pub md: Vec<ItabMd>,
    pub ud: Vec<ItabUd>,
    pub wd: Vec<ItabWd>,
    pub md_copy: Vec<ItabMd>,
    pub ud_copy: Vec<ItabUd>,
    pub wd_copy: Vec<ItabWd>,
    pub xemd: Vec<f32>,
    pub yemd: Vec<f32>,
    pub zemd: Vec<f32>,
    pub xemd_copy: Vec<f32>,
    pub yemd_copy: Vec<f32>,
    pub zemd_copy: Vec<f32>,
    pub nmd: usize,
    pub nud: usize,
    pub nwd: usize,
    pub nmd_copy: usize,
    pub nud_copy: usize,
    pub nwd_copy: usize,
    pub iwdorig: Vec<i32>,
    pub iwdorig_temp: Vec<i32>,
}

impl DelaunayMemory {
    /// Match `mem_delaunay:alloc_itabsd`: allocate Delaunay records and
    /// zero-filled M-point Cartesian arrays.
    pub fn allocate_itabsd(&mut self, mma: usize, mua: usize, mwa: usize) {
        self.md = vec![ItabMd::default(); mma];
        self.ud = vec![ItabUd::default(); mua];
        self.wd = vec![ItabWd::default(); mwa];
        self.xemd = vec![0.0; mma];
        self.yemd = vec![0.0; mma];
        self.zemd = vec![0.0; mma];
    }
}

/// Derived Earth radius values initialized by `mkgrd.F90:init_consts`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EarthRadii {
    pub radius_meters: f64,
    pub double_radius_meters: f64,
    pub radius_over_sqrt_five_meters: f64,
    pub inverse_radius_meters: f64,
    pub double_radius_squared_meters: f64,
}

impl EarthRadii {
    /// Build the same secondary radius values that Fortran initializes from `erad`.
    pub fn from_radius_meters(radius_meters: f64) -> Self {
        let double_radius_meters = radius_meters * 2.0;
        Self {
            radius_meters,
            double_radius_meters,
            radius_over_sqrt_five_meters: radius_meters / 5.0_f64.sqrt(),
            inverse_radius_meters: 1.0 / radius_meters,
            double_radius_squared_meters: double_radius_meters * double_radius_meters,
        }
    }
}

impl Default for EarthRadii {
    fn default() -> Self {
        Self::from_radius_meters(EARTH_RADIUS_METERS)
    }
}

/// Typed equivalent of `consts_coms:oname_vars` defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct EarthmeshConfig {
    pub experiment_name: String,
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
    pub beta: f32,
    pub relax: f32,
    pub isolated_ocean: bool,
    pub mask_restart: bool,
    pub mask_domain_type: String,
    pub landtype_file: String,
    pub mask_domain_fprefix: String,
    pub mask_domain_global: bool,
    pub mask_patch_on: bool,
    pub mask_patch_type: String,
    pub mask_patch_fprefix: String,
    pub output_format: String,
}

impl Default for EarthmeshConfig {
    fn default() -> Self {
        Self {
            experiment_name: "/tmp".to_string(),
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
            landtype_file: "/tmp".to_string(),
            mask_domain_fprefix: "/tmp".to_string(),
            mask_domain_global: true,
            mask_patch_on: false,
            mask_patch_type: "/tmp".to_string(),
            mask_patch_fprefix: "/tmp".to_string(),
            output_format: "/tmp".to_string(),
        }
    }
}

/// Non-destructive representation of a Fortran `Mask_make(...)` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskOperation {
    pub mask_select: String,
    pub type_select: String,
    pub mask_fprefix: String,
}

impl MaskOperation {
    pub fn new(mask_select: &str, type_select: &str, mask_fprefix: &str) -> Self {
        Self {
            mask_select: mask_select.to_string(),
            type_select: type_select.to_string(),
            mask_fprefix: mask_fprefix.to_string(),
        }
    }
}

/// Non-destructive execution plan for the filesystem and mask-preprocess side
/// effects triggered by `mkgrd.F90:read_nl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MkgrdWorkspacePlan {
    pub file_dir: String,
    pub remove_existing_file_dir: bool,
    pub remove_filelists: bool,
    pub directories_to_create: Vec<String>,
    pub namelist_save_path: String,
    pub mask_operations: Vec<MaskOperation>,
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

    /// Parse the Fortran `/mkgrd/ NL` namelist shape consumed by
    /// `mkgrd.F90:read_nl` into the typed Rust configuration.
    ///
    /// This is intentionally non-destructive: it mirrors assignment parsing and
    /// validation, but does not create/remove the working directories that the
    /// Fortran driver manages after `read_nl`.
    pub fn from_mkgrd_namelist(input: &str) -> Result<Self, String> {
        let mut config = Self::default();
        let mut in_mkgrd = false;

        for raw_line in input.lines() {
            let line = strip_fortran_comment(raw_line).trim().trim_end_matches(',');
            if line.is_empty() {
                continue;
            }
            if line.starts_with('&') {
                in_mkgrd = line.eq_ignore_ascii_case("&mkgrd");
                continue;
            }
            if line == "/" {
                in_mkgrd = false;
                continue;
            }
            if !in_mkgrd {
                continue;
            }

            let Some((left, right)) = line.split_once('=') else {
                continue;
            };
            let Some(field) = left.trim().split_once('%').map(|(_, field)| field.trim()) else {
                continue;
            };
            let value = right.trim().trim_end_matches(',');

            match field.to_ascii_lowercase().as_str() {
                "expnme" => config.experiment_name = parse_fortran_string(value),
                "nxp" => config.nxp = parse_i32(field, value)?,
                "base_dir" => config.base_dir = parse_fortran_string(value),
                "mesh_type" => config.mesh_type = parse_fortran_string(value),
                "mode_grid" => config.mode_grid = parse_fortran_string(value),
                "mode_file_description" => {
                    config.mode_file_description = parse_fortran_string(value)
                }
                "mode_file" => config.mode_file = parse_fortran_string(value),
                "refine" => config.refine = parse_fortran_bool(field, value)?,
                "openmp" => config.openmp = parse_i32(field, value)?,
                "niter" => config.niter = parse_i32(field, value)?,
                "gridnum_perdegree" => config.gridnum_perdegree = parse_i32(field, value)?,
                "mask_sea_ratio" => config.mask_sea_ratio = parse_f64(field, value)?,
                "beta" => config.beta = parse_f32(field, value)?,
                "relax" => config.relax = parse_f32(field, value)?,
                "isolated_ocean" => config.isolated_ocean = parse_fortran_bool(field, value)?,
                "mask_restart" => config.mask_restart = parse_fortran_bool(field, value)?,
                "mask_domain_type" => config.mask_domain_type = parse_fortran_string(value),
                "landtype_file" => config.landtype_file = parse_fortran_string(value),
                "mask_domain_fprefix" => config.mask_domain_fprefix = parse_fortran_string(value),
                "mask_domain_global" => {
                    config.mask_domain_global = parse_fortran_bool(field, value)?
                }
                "mask_patch_on" => config.mask_patch_on = parse_fortran_bool(field, value)?,
                "mask_patch_type" => config.mask_patch_type = parse_fortran_string(value),
                "mask_patch_fprefix" => config.mask_patch_fprefix = parse_fortran_string(value),
                "output_format" => config.output_format = parse_fortran_string(value),
                _ => {}
            }
        }

        config.validate_like_read_nl()?;
        Ok(config)
    }

    /// Build the side-effect plan implied by `read_nl` without executing shell
    /// commands or touching the filesystem.
    pub fn read_nl_workspace_plan(
        &self,
        refine_config: Option<&RefineConfig>,
    ) -> MkgrdWorkspacePlan {
        let file_dir = self.file_dir();
        let mut plan = MkgrdWorkspacePlan {
            namelist_save_path: format!("{file_dir}result/namelist.save"),
            file_dir: file_dir.clone(),
            remove_existing_file_dir: false,
            remove_filelists: false,
            directories_to_create: Vec::new(),
            mask_operations: Vec::new(),
        };

        if self.mask_restart {
            if self.mask_patch_on {
                plan.mask_operations.push(MaskOperation::new(
                    "mask_patch",
                    &self.mask_patch_type,
                    &self.mask_patch_fprefix,
                ));
            }
            return plan;
        }

        plan.remove_existing_file_dir = true;
        plan.remove_filelists = true;
        for subdir in ["contain", "gridfile", "patchtype", "result", "tmpfile"] {
            plan.directories_to_create
                .push(format!("{file_dir}{subdir}/"));
        }

        if !self.mask_domain_global {
            plan.mask_operations.push(MaskOperation::new(
                "mask_domain",
                &self.mask_domain_type,
                &self.mask_domain_fprefix,
            ));
        }
        if self.mask_patch_on {
            plan.mask_operations.push(MaskOperation::new(
                "mask_patch",
                &self.mask_patch_type,
                &self.mask_patch_fprefix,
            ));
        }

        if self.refine {
            plan.directories_to_create
                .push(format!("{file_dir}threshold/"));
            if let Some(refine) = refine_config {
                if refine.refine_setting == "specified" || refine.refine_setting == "mixed" {
                    plan.mask_operations.push(MaskOperation::new(
                        "mask_refine",
                        &refine.mask_refine_spc_type,
                        &refine.mask_refine_spc_fprefix,
                    ));
                }
                if refine.refine_setting == "calculate" || refine.refine_setting == "mixed" {
                    plan.mask_operations.push(MaskOperation::new(
                        "mask_refine",
                        &refine.mask_refine_cal_type,
                        &refine.mask_refine_cal_fprefix,
                    ));
                }
            }
        }

        plan
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
            | ("oceanmesh", "FVCOM")
            | ("atmosmesh", "MPAS")
            | ("atmosmesh", "MPAS-Simple")
            | ("LOCmesh", "CoLM") => Ok(()),
            ("landmesh", _) => Err(format!(
                "landmesh output_format must be CoLM like mkgrd.F90:read_nl, got {}",
                self.output_format
            )),
            ("oceanmesh", _) => Err(format!(
                "oceanmesh output_format must be FVCOM like mkgrd.F90:read_nl, got {}",
                self.output_format
            )),
            ("atmosmesh", _) => Err(format!(
                "atmosmesh output_format must be MPAS or MPAS-Simple like mkgrd.F90:read_nl, got {}",
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

fn strip_fortran_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    for (index, ch) in line.char_indices() {
        match ch {
            '\'' | '"' if quote == Some(ch) => quote = None,
            '\'' | '"' if quote.is_none() => quote = Some(ch),
            '!' if quote.is_none() => return &line[..index],
            _ => {}
        }
    }
    line
}

fn parse_fortran_string(value: &str) -> String {
    value
        .trim()
        .trim_end_matches(',')
        .trim()
        .trim_matches(|ch| ch == '\'' || ch == '"')
        .to_string()
}

fn parse_i32(field: &str, value: &str) -> Result<i32, String> {
    value
        .trim()
        .trim_end_matches(',')
        .parse()
        .map_err(|err| format!("invalid integer for {field}: {value} ({err})"))
}

fn parse_f32(field: &str, value: &str) -> Result<f32, String> {
    value
        .trim()
        .trim_end_matches(',')
        .parse()
        .map_err(|err| format!("invalid real for {field}: {value} ({err})"))
}

fn parse_f64(field: &str, value: &str) -> Result<f64, String> {
    value
        .trim()
        .trim_end_matches(',')
        .parse()
        .map_err(|err| format!("invalid real for {field}: {value} ({err})"))
}

fn parse_fortran_bool(field: &str, value: &str) -> Result<bool, String> {
    match value
        .trim()
        .trim_end_matches(',')
        .to_ascii_lowercase()
        .as_str()
    {
        ".true." | "true" | "t" => Ok(true),
        ".false." | "false" | "f" => Ok(false),
        other => Err(format!("invalid logical for {field}: {other}")),
    }
}

fn parse_i32_array<const N: usize>(field: &str, value: &str) -> Result<[i32; N], String> {
    let values = value
        .split(',')
        .map(|part| parse_i32(field, part.trim()))
        .collect::<Result<Vec<_>, _>>()?;
    values.try_into().map_err(|values: Vec<i32>| {
        format!(
            "invalid integer array length for {field}: expected {N}, got {}",
            values.len()
        )
    })
}

fn parse_f64_array<const N: usize>(field: &str, value: &str) -> Result<[f64; N], String> {
    let values = value
        .split(',')
        .map(|part| parse_f64(field, part.trim()))
        .collect::<Result<Vec<_>, _>>()?;
    values.try_into().map_err(|values: Vec<f64>| {
        format!(
            "invalid real array length for {field}: expected {N}, got {}",
            values.len()
        )
    })
}

/// Typed equivalent of the operational `refine_vars` module state.
#[derive(Debug, Clone, PartialEq)]
pub struct RefineConfig {
    pub refine_setting: String,
    pub mask_refine_spc_type: String,
    pub mask_refine_spc_fprefix: String,
    pub mask_refine_cal_type: String,
    pub mask_refine_cal_fprefix: String,
    pub threshold_dir: String,
    pub set_dis_type: String,
    pub mask_refine_ndm: [i32; 10],
    pub max_iter: i32,
    pub max_iter_spc: i32,
    pub max_iter_cal: i32,
    pub halo: [i32; 10],
    pub max_transition_row: [i32; 10],
    pub spring_global_type: i32,
    pub spring_regional_type: i32,
    pub num_rc: i32,
    pub vertex_pretect_layers: i32,
    pub niter_refine: i32,
    pub th_num_landtypes: i32,
    pub th_area_mainland: f64,
    pub th_sea_ratio: [f64; 2],
    pub th_onelayer_lnd: [f64; 4],
    pub th_onelayer_ocn: [f64; 8],
    pub th_onelayer_atmos: [f64; 2],
    pub th_twolayer_lnd: [[f64; 2]; 10],
    pub weak_concav_eliminate: bool,
    pub is_transition: bool,
    pub iter_d: bool,
    pub refine_spc: bool,
    pub refine_cal: bool,
    pub refine_num_landtypes: bool,
    pub refine_area_mainland: bool,
    pub refine_sea_ratio: bool,
    pub refine_onelayer_lnd: [bool; 4],
    pub refine_onelayer_ocn: [bool; 8],
    pub refine_onelayer_atmos: [bool; 2],
    pub refine_twolayer_lnd: [bool; 10],
    pub exit_loop_step: [bool; 10],
}

impl Default for RefineConfig {
    fn default() -> Self {
        Self {
            refine_setting: "/tmp".to_string(),
            mask_refine_spc_type: "/tmp".to_string(),
            mask_refine_spc_fprefix: "/tmp".to_string(),
            mask_refine_cal_type: "/tmp".to_string(),
            mask_refine_cal_fprefix: "/tmp".to_string(),
            threshold_dir: "/tmp".to_string(),
            set_dis_type: "/tmp".to_string(),
            mask_refine_ndm: [0; 10],
            max_iter: 0,
            max_iter_spc: 0,
            max_iter_cal: 0,
            halo: [0; 10],
            max_transition_row: [0; 10],
            spring_global_type: 1,
            spring_regional_type: 1,
            num_rc: 0,
            vertex_pretect_layers: 1,
            niter_refine: 100,
            th_num_landtypes: 12,
            th_area_mainland: 0.6,
            th_sea_ratio: [0.5; 2],
            th_onelayer_lnd: [999.0; 4],
            th_onelayer_ocn: [999.0; 8],
            th_onelayer_atmos: [999.0; 2],
            th_twolayer_lnd: [[999.0; 2]; 10],
            weak_concav_eliminate: false,
            is_transition: false,
            iter_d: false,
            refine_spc: false,
            refine_cal: false,
            refine_num_landtypes: false,
            refine_area_mainland: false,
            refine_sea_ratio: false,
            refine_onelayer_lnd: [false; 4],
            refine_onelayer_ocn: [false; 8],
            refine_onelayer_atmos: [false; 2],
            refine_twolayer_lnd: [false; 10],
            exit_loop_step: [false; 10],
        }
    }
}

impl RefineConfig {
    /// Parse the `/mkrefine/ RL` namelist and apply the non-I/O validation and
    /// derived mode changes performed by `mkgrd.F90:read_nl`.
    pub fn from_mkrefine_namelist(
        input: &str,
        mesh_type: &str,
        mode_grid: &str,
    ) -> Result<Self, String> {
        let mut config = Self::default();
        let mut in_mkrefine = false;

        for raw_line in input.lines() {
            let line = strip_fortran_comment(raw_line).trim().trim_end_matches(',');
            if line.is_empty() {
                continue;
            }
            if line.starts_with('&') {
                in_mkrefine = line.eq_ignore_ascii_case("&mkrefine");
                continue;
            }
            if line == "/" {
                in_mkrefine = false;
                continue;
            }
            if !in_mkrefine {
                continue;
            }

            let Some((left, right)) = line.split_once('=') else {
                continue;
            };
            let Some(field) = left.trim().split_once('%').map(|(_, field)| field.trim()) else {
                continue;
            };
            let value = right.trim().trim_end_matches(',');

            match field.to_ascii_lowercase().as_str() {
                "weak_concav_eliminate" => {
                    config.weak_concav_eliminate = parse_fortran_bool(field, value)?
                }
                "istransition" => config.is_transition = parse_fortran_bool(field, value)?,
                "iterd" => config.iter_d = parse_fortran_bool(field, value)?,
                "halo" => config.halo = parse_i32_array(field, value)?,
                "max_transition_row" => config.max_transition_row = parse_i32_array(field, value)?,
                "springglobal_type" => config.spring_global_type = parse_i32(field, value)?,
                "springregional_type" => config.spring_regional_type = parse_i32(field, value)?,
                "num_rc" => config.num_rc = parse_i32(field, value)?,
                "set_dis_type" => config.set_dis_type = parse_fortran_string(value),
                "vertex_pretect_layers" => config.vertex_pretect_layers = parse_i32(field, value)?,
                "niter_refine" => config.niter_refine = parse_i32(field, value)?,
                "refine_spc" => config.refine_spc = parse_fortran_bool(field, value)?,
                "refine_cal" => config.refine_cal = parse_fortran_bool(field, value)?,
                "max_iter_spc" => config.max_iter_spc = parse_i32(field, value)?,
                "max_iter_cal" => config.max_iter_cal = parse_i32(field, value)?,
                "mask_refine_spc_type" => config.mask_refine_spc_type = parse_fortran_string(value),
                "mask_refine_spc_fprefix" => {
                    config.mask_refine_spc_fprefix = parse_fortran_string(value)
                }
                "mask_refine_cal_type" => config.mask_refine_cal_type = parse_fortran_string(value),
                "mask_refine_cal_fprefix" => {
                    config.mask_refine_cal_fprefix = parse_fortran_string(value)
                }
                "threshold_dir" => config.threshold_dir = parse_fortran_string(value),
                "refine_num_landtypes" => {
                    config.refine_num_landtypes = parse_fortran_bool(field, value)?
                }
                "refine_area_mainland" => {
                    config.refine_area_mainland = parse_fortran_bool(field, value)?
                }
                "refine_sea_ratio" => config.refine_sea_ratio = parse_fortran_bool(field, value)?,
                "refine_lai_m" => config.refine_onelayer_lnd[0] = parse_fortran_bool(field, value)?,
                "refine_lai_s" => config.refine_onelayer_lnd[1] = parse_fortran_bool(field, value)?,
                "refine_slope_m" => {
                    config.refine_onelayer_lnd[2] = parse_fortran_bool(field, value)?
                }
                "refine_slope_s" => {
                    config.refine_onelayer_lnd[3] = parse_fortran_bool(field, value)?
                }
                "refine_k_s_m" => config.refine_twolayer_lnd[0] = parse_fortran_bool(field, value)?,
                "refine_k_s_s" => config.refine_twolayer_lnd[1] = parse_fortran_bool(field, value)?,
                "refine_k_solids_m" => {
                    config.refine_twolayer_lnd[2] = parse_fortran_bool(field, value)?
                }
                "refine_k_solids_s" => {
                    config.refine_twolayer_lnd[3] = parse_fortran_bool(field, value)?
                }
                "refine_tkdry_m" => {
                    config.refine_twolayer_lnd[4] = parse_fortran_bool(field, value)?
                }
                "refine_tkdry_s" => {
                    config.refine_twolayer_lnd[5] = parse_fortran_bool(field, value)?
                }
                "refine_tksatf_m" => {
                    config.refine_twolayer_lnd[6] = parse_fortran_bool(field, value)?
                }
                "refine_tksatf_s" => {
                    config.refine_twolayer_lnd[7] = parse_fortran_bool(field, value)?
                }
                "refine_tksatu_m" => {
                    config.refine_twolayer_lnd[8] = parse_fortran_bool(field, value)?
                }
                "refine_tksatu_s" => {
                    config.refine_twolayer_lnd[9] = parse_fortran_bool(field, value)?
                }
                "refine_sst_m" => config.refine_onelayer_ocn[0] = parse_fortran_bool(field, value)?,
                "refine_sst_s" => config.refine_onelayer_ocn[1] = parse_fortran_bool(field, value)?,
                "refine_ssh_m" => config.refine_onelayer_ocn[2] = parse_fortran_bool(field, value)?,
                "refine_ssh_s" => config.refine_onelayer_ocn[3] = parse_fortran_bool(field, value)?,
                "refine_eke_m" => config.refine_onelayer_ocn[4] = parse_fortran_bool(field, value)?,
                "refine_eke_s" => config.refine_onelayer_ocn[5] = parse_fortran_bool(field, value)?,
                "refine_sea_slope_m" => {
                    config.refine_onelayer_ocn[6] = parse_fortran_bool(field, value)?
                }
                "refine_sea_slope_s" => {
                    config.refine_onelayer_ocn[7] = parse_fortran_bool(field, value)?
                }
                "refine_typhoon_m" => {
                    config.refine_onelayer_atmos[0] = parse_fortran_bool(field, value)?
                }
                "refine_typhoon_s" => {
                    config.refine_onelayer_atmos[1] = parse_fortran_bool(field, value)?
                }
                "th_num_landtypes" => config.th_num_landtypes = parse_i32(field, value)?,
                "th_area_mainland" => config.th_area_mainland = parse_f64(field, value)?,
                "th_lai_m" => config.th_onelayer_lnd[0] = parse_f64(field, value)?,
                "th_lai_s" => config.th_onelayer_lnd[1] = parse_f64(field, value)?,
                "th_slope_m" => config.th_onelayer_lnd[2] = parse_f64(field, value)?,
                "th_slope_s" => config.th_onelayer_lnd[3] = parse_f64(field, value)?,
                "th_k_s_m" => config.th_twolayer_lnd[0] = parse_f64_array(field, value)?,
                "th_k_s_s" => config.th_twolayer_lnd[1] = parse_f64_array(field, value)?,
                "th_k_solids_m" => config.th_twolayer_lnd[2] = parse_f64_array(field, value)?,
                "th_k_solids_s" => config.th_twolayer_lnd[3] = parse_f64_array(field, value)?,
                "th_tkdry_m" => config.th_twolayer_lnd[4] = parse_f64_array(field, value)?,
                "th_tkdry_s" => config.th_twolayer_lnd[5] = parse_f64_array(field, value)?,
                "th_tksatf_m" => config.th_twolayer_lnd[6] = parse_f64_array(field, value)?,
                "th_tksatf_s" => config.th_twolayer_lnd[7] = parse_f64_array(field, value)?,
                "th_tksatu_m" => config.th_twolayer_lnd[8] = parse_f64_array(field, value)?,
                "th_tksatu_s" => config.th_twolayer_lnd[9] = parse_f64_array(field, value)?,
                "th_sea_ratio" => config.th_sea_ratio = parse_f64_array(field, value)?,
                "th_sst_m" => config.th_onelayer_ocn[0] = parse_f64(field, value)?,
                "th_sst_s" => config.th_onelayer_ocn[1] = parse_f64(field, value)?,
                "th_ssh_m" => config.th_onelayer_ocn[2] = parse_f64(field, value)?,
                "th_ssh_s" => config.th_onelayer_ocn[3] = parse_f64(field, value)?,
                "th_eke_m" => config.th_onelayer_ocn[4] = parse_f64(field, value)?,
                "th_eke_s" => config.th_onelayer_ocn[5] = parse_f64(field, value)?,
                "th_sea_slope_m" => config.th_onelayer_ocn[6] = parse_f64(field, value)?,
                "th_sea_slope_s" => config.th_onelayer_ocn[7] = parse_f64(field, value)?,
                "th_typhoon_m" => config.th_onelayer_atmos[0] = parse_f64(field, value)?,
                "th_typhoon_s" => config.th_onelayer_atmos[1] = parse_f64(field, value)?,
                _ => {}
            }
        }

        config.validate_like_read_nl(mesh_type, mode_grid)?;
        Ok(config)
    }

    fn validate_like_read_nl(&mut self, mesh_type: &str, mode_grid: &str) -> Result<(), String> {
        if !self.is_transition {
            if mode_grid != "tri" {
                return Err("not Istransition can only use in the tri".to_string());
            }
            self.spring_global_type = 0;
            self.spring_regional_type = 0;
        } else {
            if !(0..=1).contains(&self.spring_global_type) {
                return Err("SpringGlobal_type must 0,1".to_string());
            }
            if !(0..=2).contains(&self.spring_regional_type) {
                return Err("SpringRegional_type must 0,1,2".to_string());
            }
            if self.spring_global_type > 0 && self.spring_regional_type > 0 {
                return Err(
                    "only one of (SpringGlobal_type and SpringRegional_type) can larger than zero"
                        .to_string(),
                );
            }
        }

        if self.spring_global_type > 0 {
            self.vertex_pretect_layers = 0;
        }
        if self.vertex_pretect_layers < 0 {
            return Err("vertex_pretect_layers must >= 0".to_string());
        }

        if self.refine_cal && mesh_type == "atmosmesh" {
            return Err("atmosmesh can not use in refine_cal".to_string());
        }

        self.refine_setting = match (self.refine_spc, self.refine_cal) {
            (true, true) => "mixed".to_string(),
            (true, false) => "specified".to_string(),
            (false, true) => "calculate".to_string(),
            (false, false) => {
                return Err(
                    "Must one of TRUE in the refine_spc and refine_cal when refine is TRUE"
                        .to_string(),
                );
            }
        };

        if self.refine_setting == "calculate" || self.refine_setting == "mixed" {
            self.validate_threshold_switches_for_mesh(mesh_type)?;
        }
        self.validate_enabled_threshold_values()?;

        Ok(())
    }

    fn validate_threshold_switches_for_mesh(&self, mesh_type: &str) -> Result<(), String> {
        let has_land = self.refine_num_landtypes
            || self.refine_area_mainland
            || self.refine_onelayer_lnd.iter().any(|enabled| *enabled)
            || self.refine_twolayer_lnd.iter().any(|enabled| *enabled);
        let has_ocean =
            self.refine_sea_ratio || self.refine_onelayer_ocn.iter().any(|enabled| *enabled);
        let has_atmos = self.refine_onelayer_atmos.iter().any(|enabled| *enabled);

        match mesh_type {
            "landmesh" if !has_land => Err(
                "Must one of TRUE in the refine_num_landtypes or refine_area_mainland or refine_onelayer_Lnd or refine_twolayer_Lnd when refine is TRUE and meshtype = landmesh"
                    .to_string(),
            ),
            "oceanmesh" if !has_ocean => Err(
                "Must one of TRUE in the refine_sea_ratio or refine_onelayer_Ocn when refine is TRUE and meshtype = oceanmesh"
                    .to_string(),
            ),
            "atmosmesh" if !has_atmos => Err(
                "Must one of TRUE in the refine_onelayer_Atmos when refine is TRUE and meshtype = atmosmesh"
                    .to_string(),
            ),
            "LOCmesh" if !(has_land || has_ocean || has_atmos) => Err(
                "Must one threshold switch be TRUE for LOCmesh among land, ocean, or atmos criteria"
                    .to_string(),
            ),
            _ => Ok(()),
        }
    }

    fn validate_enabled_threshold_values(&self) -> Result<(), String> {
        for (index, enabled) in self.refine_onelayer_lnd.iter().enumerate() {
            if *enabled && self.th_onelayer_lnd[index] == 999.0 {
                return Err(format!(
                    "mismatch between refine_onelayer_Lnd({}) and th_onelayer_Lnd({})",
                    index + 1,
                    index + 1
                ));
            }
        }
        for (index, enabled) in self.refine_twolayer_lnd.iter().enumerate() {
            if *enabled && self.th_twolayer_lnd[index].contains(&999.0) {
                return Err(format!(
                    "mismatch between refine_twolayer_Lnd({}) and th_twolayer_Lnd({}, 1:2)",
                    index + 1,
                    index + 1
                ));
            }
        }
        for (index, enabled) in self.refine_onelayer_ocn.iter().enumerate() {
            if *enabled && self.th_onelayer_ocn[index] == 999.0 {
                return Err(format!(
                    "mismatch between refine_onelayer_Ocn({}) and th_onelayer_Ocn({})",
                    index + 1,
                    index + 1
                ));
            }
        }
        for (index, enabled) in self.refine_onelayer_atmos.iter().enumerate() {
            if *enabled && self.th_onelayer_atmos[index] == 999.0 {
                return Err(format!(
                    "mismatch between refine_onelayer_Atmos({}) and th_onelayer_Atmos({})",
                    index + 1,
                    index + 1
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_radii_use_mpas_radius() {
        let radii = EarthRadii::default();
        assert_eq!(radii.radius_meters, EARTH_RADIUS_METERS);
    }
}

/// Typed equivalent of `lonlatmesh_coms:mesh_vars` defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct LonLatMeshConfig {
    pub definition: String,
    pub lon_start: f64,
    pub lon_end: f64,
    pub lon_grid_interval: f64,
    pub lon_points: i32,
    pub lat_start: f64,
    pub lat_end: f64,
    pub lat_grid_interval: f64,
    pub lat_points: i32,
}

impl Default for LonLatMeshConfig {
    fn default() -> Self {
        Self {
            definition: "center".to_string(),
            lon_start: 0.0,
            lon_end: 359.0,
            lon_grid_interval: 0.0625,
            lon_points: 2880,
            lat_start: 0.0,
            lat_end: 0.0,
            lat_grid_interval: 0.0,
            lat_points: 1440,
        }
    }
}

/// Typed equivalent of `fvcommesh_coms:mesh_vars` defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct FvcomMeshConfig {
    pub case_name: String,
    pub dem_file: String,
    pub lon_name: String,
    pub lat_name: String,
    pub depth_name: String,
    pub min_depth: f64,
    pub max_depth: f64,
    pub limit_slope: f64,
}

impl Default for FvcomMeshConfig {
    fn default() -> Self {
        Self {
            case_name: "CASENAME".to_string(),
            dem_file: "/tmp".to_string(),
            lon_name: "/tmp".to_string(),
            lat_name: "/tmp".to_string(),
            depth_name: "/tmp".to_string(),
            min_depth: 1.0,
            max_depth: 300.0,
            limit_slope: 0.02,
        }
    }
}
