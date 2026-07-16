use crate::MLOOPS;

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
    pub xem: Vec<f64>,
    pub yem: Vec<f64>,
    pub zem: Vec<f64>,
    pub xew: Vec<f64>,
    pub yew: Vec<f64>,
    pub zew: Vec<f64>,
    pub glatm: Vec<f64>,
    pub glonm: Vec<f64>,
    pub glatw: Vec<f64>,
    pub glonw: Vec<f64>,
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
    pub dirv: [f64; 7],
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
    pub xemd: Vec<f64>,
    pub yemd: Vec<f64>,
    pub zemd: Vec<f64>,
    pub xemd_copy: Vec<f64>,
    pub yemd_copy: Vec<f64>,
    pub zemd_copy: Vec<f64>,
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

/// Mesh-memory allocation sizes used to replace the Canonical `mem_*` module
/// globals with one explicit Rust-owned runtime state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MeshMemoryShape {
    pub nma: usize,
    pub nua: usize,
    pub nva: usize,
    pub nwa: usize,
    pub mma: usize,
    pub mua: usize,
    pub mva: usize,
    pub mwa: usize,
}
