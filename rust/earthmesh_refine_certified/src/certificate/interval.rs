#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    pub lo: f64,
    pub hi: f64,
}

impl Interval {
    pub fn point(value: f64) -> Self {
        Self {
            lo: next_down(value),
            hi: next_up(value),
        }
    }

    pub fn around(value: f64, radius: f64) -> Self {
        Self {
            lo: next_down(value - radius),
            hi: next_up(value + radius),
        }
    }

    pub fn add_out(self, rhs: Self) -> Self {
        Self {
            lo: next_down(self.lo + rhs.lo),
            hi: next_up(self.hi + rhs.hi),
        }
    }

    pub fn sub_out(self, rhs: Self) -> Self {
        Self {
            lo: next_down(self.lo - rhs.hi),
            hi: next_up(self.hi - rhs.lo),
        }
    }

    pub fn mul_out(self, rhs: Self) -> Self {
        let p = [
            self.lo * rhs.lo,
            self.lo * rhs.hi,
            self.hi * rhs.lo,
            self.hi * rhs.hi,
        ];
        Self {
            lo: next_down(p.iter().copied().fold(f64::INFINITY, f64::min)),
            hi: next_up(p.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
        }
    }

    pub fn contains(self, value: f64) -> bool {
        self.lo <= value && value <= self.hi
    }
}

pub fn next_up(value: f64) -> f64 {
    if value.is_nan() || value == f64::INFINITY {
        return value;
    }
    if value == 0.0 {
        return f64::from_bits(1);
    }
    let bits = value.to_bits();
    f64::from_bits(if value >= 0.0 { bits + 1 } else { bits - 1 })
}

pub fn next_down(value: f64) -> f64 {
    if value.is_nan() || value == f64::NEG_INFINITY {
        return value;
    }
    if value == 0.0 {
        return -f64::from_bits(1);
    }
    let bits = value.to_bits();
    f64::from_bits(if value > 0.0 { bits - 1 } else { bits + 1 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outward_ops_contain_exact_representative_values() {
        let a = Interval::point(0.1);
        let b = Interval::point(0.2);
        assert!(a.add_out(b).contains(0.3));
        assert!(b.sub_out(a).contains(0.1));
        assert!(a.mul_out(b).contains(0.02));
    }
}
