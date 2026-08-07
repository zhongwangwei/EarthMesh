//! One triangle, as something that builds a mesh rather than something a mesh
//! holds.
//!
//! Every construction that produces faces goes through this: the global 2x and
//! 3x expansions, the Cartesian-hex build, reading a mesh back from gridfile
//! tables, and Method-C's emit. It was declared inside the nesting, which is
//! why splitting the crate found four shared callers pointing at it.
//!
//! `mrow` is Method-C's, and is carried rather than acted on -- a shared
//! builder passing a field through for whichever backend cares.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MethodCTriangleSeed {
    pub im: [usize; 3],
    pub mrlw: usize,
    pub mrlw_orig: usize,
    pub ngr: usize,
    pub mrow: isize,
    pub target_iw: usize,
    pub target_iu: [usize; 3],
}

impl MethodCTriangleSeed {
    pub fn new(im: [usize; 3], metadata: (usize, usize, usize)) -> Self {
        Self {
            im,
            mrlw: metadata.0,
            mrlw_orig: metadata.1,
            ngr: metadata.2,
            mrow: 0,
            target_iw: 0,
            target_iu: [0; 3],
        }
    }

    pub fn with_mrow(mut self, mrow: isize) -> Self {
        self.mrow = mrow;
        self
    }

    pub fn with_target_iw(mut self, target_iw: usize) -> Self {
        self.target_iw = target_iw;
        self
    }
}
