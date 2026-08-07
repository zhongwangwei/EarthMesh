//! Topological decisions that do not turn on a guessed epsilon.
//!
//! An incremental triangulation asks the same few questions over and over:
//! which side of this plane is that point, is this point inside that
//! circumcircle. Answered with `if determinant.abs() < 1e-10 { pick a branch }`
//! they are answered differently on different machines, and a mesh built on
//! them cannot be reproduced or audited.
//!
//! The shape here is the one the specification asks for. A fast f64 evaluation
//! with a rigorous error bound decides whenever the determinant is far enough
//! from zero, which is nearly always. When it is not, the same determinant is
//! recomputed in roughly twice the precision. Only if *that* cannot separate
//! the value from zero does the predicate say so, as [`Ambiguous`], rather than
//! choosing.
//!
//! Exactly zero is a different answer from ambiguous, and both are returned.
//! Four points genuinely on one plane is a fact about the input; being unable
//! to tell is a fact about the arithmetic.

use crate::CartesianPoint;

/// Which side, or neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sign {
    Positive,
    Negative,
    /// The determinant is exactly zero: the points really are coplanar or
    /// cocircular.
    Zero,
}

/// The predicate could not decide, even at extended precision.
///
/// Deliberately not convertible into a sign. A caller that reaches this has a
/// genuinely degenerate configuration and has to do something about it --
/// perturb the candidate, widen the patch, refuse the transaction -- rather
/// than proceed on a coin toss.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ambiguous {
    /// The extended-precision value that could not be separated from zero.
    pub residual: f64,
    /// The bound it had to clear.
    pub bound: f64,
}

impl std::fmt::Display for Ambiguous {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "geometric predicate is degenerate: |{:e}| does not clear {:e}",
            self.residual, self.bound
        )
    }
}

impl std::error::Error for Ambiguous {}

/// Dekker's splitting constant, `2^27 + 1`.
const SPLITTER: f64 = 134_217_729.0;

/// Split a double into two halves whose product with another split double is
/// exact.
fn split(value: f64) -> (f64, f64) {
    let scaled = SPLITTER * value;
    let high = scaled - (scaled - value);
    (high, value - high)
}

/// The product, and the part of it the product lost.
///
/// Dekker's algorithm rather than a fused multiply-add. `mul_add` gives the
/// same answer only where it is genuinely fused, and whether it is depends on
/// the target; a predicate that decides differently on two machines is the
/// thing this module exists to prevent.
fn two_product(left: f64, right: f64) -> (f64, f64) {
    let product = left * right;
    let (left_high, left_low) = split(left);
    let (right_high, right_low) = split(right);
    let error =
        ((left_high * right_high - product) + left_high * right_low + left_low * right_high)
            + left_low * right_low;
    (product, error)
}

/// The sum, and the part of it the sum lost. Valid for any two doubles.
fn two_sum(left: f64, right: f64) -> (f64, f64) {
    let sum = left + right;
    let left_virtual = sum - right;
    let right_virtual = sum - left_virtual;
    (sum, (left - left_virtual) + (right - right_virtual))
}

/// A number held as a leading double plus its correction.
#[derive(Clone, Copy, Debug, Default)]
struct DoubleDouble {
    high: f64,
    low: f64,
}

impl DoubleDouble {
    fn value(self) -> f64 {
        self.high + self.low
    }

    fn add(self, other: f64) -> Self {
        let (sum, error) = two_sum(self.high, other);
        let low = self.low + error;
        let (high, error) = two_sum(sum, low);
        Self { high, low: error }
    }

    /// Add a product without losing what the product lost.
    fn add_product(self, left: f64, right: f64) -> Self {
        let (product, error) = two_product(left, right);
        self.add(product).add(error)
    }
}

/// `1 + eps` rounding, used to size the fast path's error bound.
const EPSILON: f64 = f64::EPSILON / 2.0;

/// Sign of the determinant of the three row vectors.
///
/// Positive when `(row_a, row_b, row_c)` form a right-handed set, which for
/// three points on a unit sphere means they are counter-clockwise seen from
/// outside.
fn determinant_sign(row_a: [f64; 3], row_b: [f64; 3], row_c: [f64; 3]) -> Result<Sign, Ambiguous> {
    let terms = [
        (row_a[0], row_b[1] * row_c[2], row_b[1], row_c[2]),
        (-row_a[0], row_b[2] * row_c[1], row_b[2], row_c[1]),
        (-row_a[1], row_b[0] * row_c[2], row_b[0], row_c[2]),
        (row_a[1], row_b[2] * row_c[0], row_b[2], row_c[0]),
        (row_a[2], row_b[0] * row_c[1], row_b[0], row_c[1]),
        (-row_a[2], row_b[1] * row_c[0], row_b[1], row_c[0]),
    ];
    let approximate: f64 = terms
        .iter()
        .map(|(scale, product, _, _)| scale * product)
        .sum();
    // Sum of magnitudes: the largest the rounding error can be, times the
    // relative bound. Shewchuk's static filter for a three by three
    // determinant of this shape.
    let permanent: f64 = terms
        .iter()
        .map(|(scale, product, _, _)| (scale * product).abs())
        .sum();
    let fast_bound = (7.0 + 56.0 * EPSILON) * EPSILON * permanent;
    if approximate.abs() > fast_bound {
        return Ok(if approximate > 0.0 {
            Sign::Positive
        } else {
            Sign::Negative
        });
    }

    // Too close to call. Redo it carrying the bits the fast path dropped.
    let mut exact = DoubleDouble::default();
    for (scale, _, left, right) in terms {
        let (product, product_error) = two_product(left, right);
        exact = exact.add_product(scale, product);
        exact = exact.add_product(scale, product_error);
    }
    let refined = exact.value();
    let refined_bound = 16.0 * EPSILON * EPSILON * permanent;
    if refined.abs() > refined_bound {
        return Ok(if refined > 0.0 {
            Sign::Positive
        } else {
            Sign::Negative
        });
    }
    if refined == 0.0 && permanent == 0.0 {
        return Ok(Sign::Zero);
    }
    if refined == 0.0 {
        return Ok(Sign::Zero);
    }
    Err(Ambiguous {
        residual: refined,
        bound: refined_bound,
    })
}

/// Which side of the plane through `a`, `b` and `c` the point `d` lies on.
///
/// Positive when `d` is below the plane as seen from the side `a`, `b`, `c`
/// wind counter-clockwise. Zero when the four are coplanar.
pub fn orient3d(
    a: CartesianPoint,
    b: CartesianPoint,
    c: CartesianPoint,
    d: CartesianPoint,
) -> Result<Sign, Ambiguous> {
    determinant_sign(
        [a.x - d.x, a.y - d.y, a.z - d.z],
        [b.x - d.x, b.y - d.y, b.z - d.z],
        [c.x - d.x, c.y - d.y, c.z - d.z],
    )
}

/// Whether `a`, `b` and `c` wind counter-clockwise seen from outside the
/// sphere.
///
/// The triple product, which is `orient3d` against the centre.
pub fn orientation_on_sphere(
    a: CartesianPoint,
    b: CartesianPoint,
    c: CartesianPoint,
) -> Result<Sign, Ambiguous> {
    determinant_sign([a.x, a.y, a.z], [b.x, b.y, b.z], [c.x, c.y, c.z])
}

/// Whether `d` falls inside the circumcircle of the spherical triangle
/// `a`, `b`, `c`.
///
/// On a sphere the circumcircle of three points is the intersection with the
/// plane through them, so "inside" is "on the far side of that plane from the
/// centre" -- which makes this the same determinant as `orient3d`, read
/// against the triangle's own winding. Getting that reading right is what the
/// tests pin.
///
/// `Sign::Zero` means the four points are exactly cocircular, which is a
/// legitimate answer and the case a Delaunay flip is free to break either way.
pub fn in_circle_on_sphere(
    a: CartesianPoint,
    b: CartesianPoint,
    c: CartesianPoint,
    d: CartesianPoint,
) -> Result<Sign, Ambiguous> {
    let winding = orientation_on_sphere(a, b, c)?;
    let side = orient3d(a, b, c, d)?;
    Ok(match (winding, side) {
        (_, Sign::Zero) => Sign::Zero,
        (Sign::Zero, _) => Sign::Zero,
        (Sign::Positive, Sign::Positive) | (Sign::Negative, Sign::Negative) => Sign::Positive,
        (Sign::Positive, Sign::Negative) | (Sign::Negative, Sign::Positive) => Sign::Negative,
    })
}

#[cfg(test)]
mod tests;
