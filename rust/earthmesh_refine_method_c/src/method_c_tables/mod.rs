use std::io;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MethodCNestUd {
    pub(crate) im: usize,
    pub(crate) iu: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MethodCNestWd {
    pub(crate) iu: [usize; 3],
    pub(crate) iw: [isize; 3],
}

impl MethodCNestWd {
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
                format!("Method-C child W slot {slot} is not allocated"),
            ));
        }
        Ok(value as usize)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MethodCPerimeterPoint {
    pub(crate) im: usize,
    pub(crate) iu: usize,
    pub(crate) npoly: usize,
    pub(crate) nwdiv: usize,
    pub(crate) near_pentagon: bool,
}
