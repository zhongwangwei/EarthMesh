use std::io;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct OlamMethodCNestUd {
    pub(crate) im: usize,
    pub(crate) iu: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct OlamMethodCNestWd {
    pub(crate) iu: [usize; 3],
    pub(crate) iw: [isize; 3],
}

impl OlamMethodCNestWd {
    pub(crate) fn flag(self) -> isize {
        self.iw[2]
    }

    pub(crate) fn is_subdivided(self) -> bool {
        self.flag() > 0
    }

    pub(crate) fn is_suppressed(self) -> bool {
        self.flag() < 0
    }

    pub(crate) fn child_iw(self, slot: usize) -> io::Result<usize> {
        let value = self.iw[slot];
        if value <= 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("OLAM Method-C child W slot {slot} is not allocated"),
            ));
        }
        Ok(value as usize)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OlamMethodCPerimeterPoint {
    pub(crate) im: usize,
    pub(crate) iu: usize,
    pub(crate) npoly: usize,
    pub(crate) nwdiv: usize,
    pub(crate) near_pentagon: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OlamTriangleSeed {
    pub(crate) im: [usize; 3],
    pub(crate) mrlw: usize,
    pub(crate) mrlw_orig: usize,
    pub(crate) ngr: usize,
    pub(crate) mrow: isize,
    pub(crate) target_iw: usize,
    pub(crate) target_iu: [usize; 3],
}

impl OlamTriangleSeed {
    pub(crate) fn new(im: [usize; 3], metadata: (usize, usize, usize)) -> Self {
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

    pub(crate) fn with_mrow(mut self, mrow: isize) -> Self {
        self.mrow = mrow;
        self
    }

    pub(crate) fn with_target_iw(mut self, target_iw: usize) -> Self {
        self.target_iw = target_iw;
        self
    }
}
