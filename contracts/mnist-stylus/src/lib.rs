// Allow cargo stylus export-abi to generate a main function.
#![cfg_attr(not(feature = "export-abi"), no_main)]
extern crate alloc;

use std::cmp::Ordering;
use stylus_sdk::{alloy_primitives::U256, prelude::*};

#[derive(Default, Copy, Clone)]
#[repr(transparent)]
pub struct SoftF64(pub f64);

type FInt = u64;
type FSignedInt = i64;

const UNORDERED: Option<Ordering> = None;
const EQUAL: Option<Ordering> = Some(Ordering::Equal);
const GREATER: Option<Ordering> = Some(Ordering::Greater);
const LESS: Option<Ordering> = Some(Ordering::Less);

const fn u64_widen_mul(a: u64, b: u64) -> (u64, u64) {
    let x = u128::wrapping_mul(a as _, b as _);
    (x as u64, (x >> 64) as u64)
}

impl SoftF64 {
    pub const fn from_bits(a: u64) -> Self {
        Self(unsafe { core::mem::transmute(a) })
    }

    pub const fn to_bits(self) -> u64 {
        unsafe { core::mem::transmute(self.0) }
    }

    pub const fn add(self, rhs: Self) -> Self {
        let one: FInt = 1;
        let zero: FInt = 0;

        let bits = Self::BITS as FInt;
        let significand_bits = Self::SIGNIFICAND_BITS;
        let max_exponent = Self::EXPONENT_MAX;

        let implicit_bit = Self::IMPLICIT_BIT;
        let significand_mask = Self::SIGNIFICAND_MASK;
        let sign_bit = Self::SIGN_MASK as FInt;
        let abs_mask = sign_bit - one;
        let exponent_mask = Self::EXPONENT_MASK;
        let inf_rep = exponent_mask;
        let quiet_bit = implicit_bit >> 1;
        let qnan_rep = exponent_mask | quiet_bit;

        let mut a_rep = self.repr();
        let mut b_rep = rhs.repr();
        let a_abs = a_rep & abs_mask;
        let b_abs = b_rep & abs_mask;

        // Detect if a or b is zero, infinity, or NaN.
        if a_abs.wrapping_sub(one) >= inf_rep - one || b_abs.wrapping_sub(one) >= inf_rep - one {
            // NaN + anything = qNaN
            if a_abs > inf_rep {
                return Self::from_repr(a_abs | quiet_bit);
            }
            // anything + NaN = qNaN
            if b_abs > inf_rep {
                return Self::from_repr(b_abs | quiet_bit);
            }

            if a_abs == inf_rep {
                // +/-infinity + -/+infinity = qNaN
                if (self.repr() ^ rhs.repr()) == sign_bit {
                    return Self::from_repr(qnan_rep);
                } else {
                    // +/-infinity + anything remaining = +/- infinity
                    return self;
                }
            }

            // anything remaining + +/-infinity = +/-infinity
            if b_abs == inf_rep {
                return rhs;
            }

            // zero + anything = anything
            if a_abs == 0 {
                // but we need to get the sign right for zero + zero
                if b_abs == 0 {
                    return Self::from_repr(self.repr() & rhs.repr());
                } else {
                    return rhs;
                }
            }

            // anything + zero = anything
            if b_abs == 0 {
                return rhs;
            }
        }

        // Swap a and b if necessary so that a has the larger absolute value.
        if b_abs > a_abs {
            // Don't use mem::swap because it may generate references to memcpy in unoptimized code.
            let tmp = a_rep;
            a_rep = b_rep;
            b_rep = tmp;
        }

        // Extract the exponent and significand from the (possibly swapped) a and b.
        let mut a_exponent: i32 = ((a_rep & exponent_mask) >> significand_bits) as _;
        let mut b_exponent: i32 = ((b_rep & exponent_mask) >> significand_bits) as _;
        let mut a_significand = a_rep & significand_mask;
        let mut b_significand = b_rep & significand_mask;

        // normalize any denormals, and adjust the exponent accordingly.
        if a_exponent == 0 {
            let (exponent, significand) = Self::normalize(a_significand);
            a_exponent = exponent;
            a_significand = significand;
        }
        if b_exponent == 0 {
            let (exponent, significand) = Self::normalize(b_significand);
            b_exponent = exponent;
            b_significand = significand;
        }

        // The sign of the result is the sign of the larger operand, a.  If they
        // have opposite signs, we are performing a subtraction; otherwise addition.
        let result_sign = a_rep & sign_bit;
        let subtraction = ((a_rep ^ b_rep) & sign_bit) != zero;

        // Shift the significands to give us round, guard and sticky, and or in the
        // implicit significand bit.  (If we fell through from the denormal path it
        // was already set by normalize(), but setting it twice won't hurt
        // anything.)
        a_significand = (a_significand | implicit_bit) << 3;
        b_significand = (b_significand | implicit_bit) << 3;

        // Shift the significand of b by the difference in exponents, with a sticky
        // bottom bit to get rounding correct.
        let align = a_exponent.wrapping_sub(b_exponent) as _;
        if align != 0 {
            if align < bits {
                let sticky = (b_significand << bits.wrapping_sub(align) != 0) as FInt;
                b_significand = (b_significand >> align) | sticky;
            } else {
                b_significand = one; // sticky; b is known to be non-zero.
            }
        }
        if subtraction {
            a_significand = a_significand.wrapping_sub(b_significand);
            // If a == -b, return +zero.
            if a_significand == 0 {
                return Self::from_repr(0);
            }

            // If partial cancellation occured, we need to left-shift the result
            // and adjust the exponent:
            if a_significand < implicit_bit << 3 {
                let shift =
                    a_significand.leading_zeros() as i32 - (implicit_bit << 3).leading_zeros() as i32;
                a_significand <<= shift;
                a_exponent -= shift;
            }
        } else {
            // addition
            a_significand += b_significand;

            // If the addition carried up, we need to right-shift the result and
            // adjust the exponent:
            if a_significand & implicit_bit << 4 != 0 {
                let sticky = (a_significand & one != 0) as FInt;
                a_significand = a_significand >> 1 | sticky;
                a_exponent += 1;
            }
        }

        // If we have overflowed the type, return +/- infinity:
        if a_exponent >= max_exponent as i32 {
            return Self::from_repr(inf_rep | result_sign);
        }

        if a_exponent <= 0 {
            // Result is denormal before rounding; the exponent is zero and we
            // need to shift the significand.
            let shift = (1 - a_exponent) as _;
            let sticky = ((a_significand << bits.wrapping_sub(shift)) != 0) as FInt;
            a_significand = a_significand >> shift | sticky;
            a_exponent = 0;
        }

        // Low three bits are round, guard, and sticky.
        let a_significand_i32: i32 = a_significand as _;
        let round_guard_sticky: i32 = a_significand_i32 & 0x7;

        // Shift the significand into place, and mask off the implicit bit.
        let mut result = a_significand >> 3 & significand_mask;

        // Insert the exponent and sign.
        result |= (a_exponent as FInt) << significand_bits;
        result |= result_sign;

        // Final rounding.  The result may overflow to infinity, but that is the
        // correct result in that case.
        if round_guard_sticky > 0x4 {
            result += one;
        }
        if round_guard_sticky == 0x4 {
            result += result & one;
        }

        Self::from_repr(result)
    }

    pub const fn mul(self, rhs: Self) -> Self {
        let one: FInt = 1;
        let zero: FInt = 0;

        let bits = Self::BITS;
        let significand_bits = Self::SIGNIFICAND_BITS;
        let max_exponent = Self::EXPONENT_MAX;

        let exponent_bias = Self::EXPONENT_BIAS;

        let implicit_bit = Self::IMPLICIT_BIT;
        let significand_mask = Self::SIGNIFICAND_MASK;
        let sign_bit = Self::SIGN_MASK as FInt;
        let abs_mask = sign_bit - one;
        let exponent_mask = Self::EXPONENT_MASK;
        let inf_rep = exponent_mask;
        let quiet_bit = implicit_bit >> 1;
        let qnan_rep = exponent_mask | quiet_bit;
        let exponent_bits = Self::EXPONENT_BITS;

        let a_rep = self.repr();
        let b_rep = rhs.repr();

        let a_exponent = (a_rep >> significand_bits) & max_exponent as FInt;
        let b_exponent = (b_rep >> significand_bits) & max_exponent as FInt;
        let product_sign = (a_rep ^ b_rep) & sign_bit;

        let mut a_significand = a_rep & significand_mask;
        let mut b_significand = b_rep & significand_mask;
        let mut scale = 0;

        // Detect if a or b is zero, denormal, infinity, or NaN.
        if a_exponent.wrapping_sub(one) >= (max_exponent - 1) as FInt
            || b_exponent.wrapping_sub(one) >= (max_exponent - 1) as FInt
        {
            let a_abs = a_rep & abs_mask;
            let b_abs = b_rep & abs_mask;

            // NaN + anything = qNaN
            if a_abs > inf_rep {
                return Self::from_repr(a_rep | quiet_bit);
            }
            // anything + NaN = qNaN
            if b_abs > inf_rep {
                return Self::from_repr(b_rep | quiet_bit);
            }

            if a_abs == inf_rep {
                if b_abs != zero {
                    // infinity * non-zero = +/- infinity
                    return Self::from_repr(a_abs | product_sign);
                } else {
                    // infinity * zero = NaN
                    return Self::from_repr(qnan_rep);
                }
            }

            if b_abs == inf_rep {
                if a_abs != zero {
                    // infinity * non-zero = +/- infinity
                    return Self::from_repr(b_abs | product_sign);
                } else {
                    // infinity * zero = NaN
                    return Self::from_repr(qnan_rep);
                }
            }

            // zero * anything = +/- zero
            if a_abs == zero {
                return Self::from_repr(product_sign);
            }

            // anything * zero = +/- zero
            if b_abs == zero {
                return Self::from_repr(product_sign);
            }

            // one or both of a or b is denormal, the other (if applicable) is a
            // normal number.  Renormalize one or both of a and b, and set scale to
            // include the necessary exponent adjustment.
            if a_abs < implicit_bit {
                let (exponent, significand) = Self::normalize(a_significand);
                scale += exponent;
                a_significand = significand;
            }

            if b_abs < implicit_bit {
                let (exponent, significand) = Self::normalize(b_significand);
                scale += exponent;
                b_significand = significand;
            }
        }

        // Or in the implicit significand bit.  (If we fell through from the
        // denormal path it was already set by normalize( ), but setting it twice
        // won't hurt anything.)
        a_significand |= implicit_bit;
        b_significand |= implicit_bit;

        // Get the significand of a*b.  Before multiplying the significands, shift
        // one of them left to left-align it in the field.  Thus, the product will
        // have (exponentBits + 2) integral digits, all but two of which must be
        // zero.  Normalizing this result is just a conditional left-shift by one
        // and bumping the exponent accordingly.
        let (mut product_low, mut product_high) =
            u64_widen_mul(a_significand, b_significand << exponent_bits);

        let a_exponent_i32: i32 = a_exponent as _;
        let b_exponent_i32: i32 = b_exponent as _;
        let mut product_exponent: i32 = a_exponent_i32
            .wrapping_add(b_exponent_i32)
            .wrapping_add(scale)
            .wrapping_sub(exponent_bias as i32);

        // Normalize the significand, adjust exponent if needed.
        if (product_high & implicit_bit) != zero {
            product_exponent = product_exponent.wrapping_add(1);
        } else {
            product_high = (product_high << 1) | (product_low >> (bits - 1));
            product_low <<= 1;
        }

        // If we have overflowed the type, return +/- infinity.
        if product_exponent >= max_exponent as i32 {
            return Self::from_repr(inf_rep | product_sign);
        }

        if product_exponent <= 0 {
            // Result is denormal before rounding
            //
            // If the result is so small that it just underflows to zero, return
            // a zero of the appropriate sign.  Mathematically there is no need to
            // handle this case separately, but we make it a special case to
            // simplify the shift logic.
            let shift = one.wrapping_sub(product_exponent as FInt) as u32;
            if shift >= bits {
                return Self::from_repr(product_sign);
            }

            // Otherwise, shift the significand of the result so that the round
            // bit is the high bit of productLo.
            if shift < bits {
                let sticky = product_low << (bits - shift);
                product_low = product_high << (bits - shift) | product_low >> shift | sticky;
                product_high >>= shift;
            } else if shift < (2 * bits) {
                let sticky = product_high << (2 * bits - shift) | product_low;
                product_low = product_high >> (shift - bits) | sticky;
                product_high = zero;
            } else {
                product_high = zero;
            }
        } else {
            // Result is normal before rounding; insert the exponent.
            product_high &= significand_mask;
            product_high |= (product_exponent as FInt) << significand_bits;
        }

        // Insert the sign of the result:
        product_high |= product_sign;

        // Final rounding.  The final result may overflow to infinity, or underflow
        // to zero, but those are the correct results in those cases.  We use the
        // default IEEE-754 round-to-nearest, ties-to-even rounding mode.
        if product_low > sign_bit {
            product_high += one;
        }

        if product_low == sign_bit {
            product_high += product_high & one;
        }
        Self::from_repr(product_high)
    }

    pub const fn div(self, rhs: Self) -> Self {
        const NUMBER_OF_HALF_ITERATIONS: usize = 3;
        const NUMBER_OF_FULL_ITERATIONS: usize = 1;
        const USE_NATIVE_FULL_ITERATIONS: bool = false;

        let one = 1;
        let zero = 0;
        let hw = Self::BITS / 2;
        let lo_mask = u64::MAX >> hw;

        let significand_bits = Self::SIGNIFICAND_BITS;
        let max_exponent = Self::EXPONENT_MAX;

        let exponent_bias = Self::EXPONENT_BIAS;

        let implicit_bit = Self::IMPLICIT_BIT;
        let significand_mask = Self::SIGNIFICAND_MASK;
        let sign_bit = Self::SIGN_MASK;
        let abs_mask = sign_bit - one;
        let exponent_mask = Self::EXPONENT_MASK;
        let inf_rep = exponent_mask;
        let quiet_bit = implicit_bit >> 1;
        let qnan_rep = exponent_mask | quiet_bit;

        #[inline(always)]
        const fn negate_u64(a: u64) -> u64 {
            (<i64>::wrapping_neg(a as i64)) as u64
        }

        let a_rep = self.repr();
        let b_rep = rhs.repr();

        let a_exponent = (a_rep >> significand_bits) & max_exponent as u64;
        let b_exponent = (b_rep >> significand_bits) & max_exponent as u64;
        let quotient_sign = (a_rep ^ b_rep) & sign_bit;

        let mut a_significand = a_rep & significand_mask;
        let mut b_significand = b_rep & significand_mask;
        let mut scale = 0;

        // Detect if a or b is zero, denormal, infinity, or NaN.
        if a_exponent.wrapping_sub(one) >= (max_exponent - 1) as u64
            || b_exponent.wrapping_sub(one) >= (max_exponent - 1) as u64
        {
            let a_abs = a_rep & abs_mask;
            let b_abs = b_rep & abs_mask;

            // NaN / anything = qNaN
            if a_abs > inf_rep {
                return Self::from_repr(a_rep | quiet_bit);
            }
            // anything / NaN = qNaN
            if b_abs > inf_rep {
                return Self::from_repr(b_rep | quiet_bit);
            }

            if a_abs == inf_rep {
                if b_abs == inf_rep {
                    // infinity / infinity = NaN
                    return Self::from_repr(qnan_rep);
                } else {
                    // infinity / anything else = +/- infinity
                    return Self::from_repr(a_abs | quotient_sign);
                }
            }

            // anything else / infinity = +/- 0
            if b_abs == inf_rep {
                return Self::from_repr(quotient_sign);
            }

            if a_abs == zero {
                if b_abs == zero {
                    // zero / zero = NaN
                    return Self::from_repr(qnan_rep);
                } else {
                    // zero / anything else = +/- zero
                    return Self::from_repr(quotient_sign);
                }
            }

            // anything else / zero = +/- infinity
            if b_abs == zero {
                return Self::from_repr(inf_rep | quotient_sign);
            }

            // one or both of a or b is denormal, the other (if applicable) is a
            // normal number.  Renormalize one or both of a and b, and set scale to
            // include the necessary exponent adjustment.
            if a_abs < implicit_bit {
                let (exponent, significand) = Self::normalize(a_significand);
                scale += exponent;
                a_significand = significand;
            }

            if b_abs < implicit_bit {
                let (exponent, significand) = Self::normalize(b_significand);
                scale -= exponent;
                b_significand = significand;
            }
        }

        // Set the implicit significand bit.  If we fell through from the
        // denormal path it was already set by normalize( ), but setting it twice
        // won't hurt anything.
        a_significand |= implicit_bit;
        b_significand |= implicit_bit;

        let written_exponent: i64 = a_exponent
            .wrapping_sub(b_exponent)
            .wrapping_add(scale as u64)
            .wrapping_add(exponent_bias as u64) as i64;
        let b_uq1 = b_significand << (Self::BITS - significand_bits - 1);

        // Align the significand of b as a UQ1.(n-1) fixed-point number in the range
        // [1.0, 2.0) and get a UQ0.n approximate reciprocal using a small minimax
        // polynomial approximation: x0 = 3/4 + 1/sqrt(2) - b/2.
        // The max error for this approximation is achieved at endpoints, so
        //   abs(x0(b) - 1/b) <= abs(x0(1) - 1/1) = 3/4 - 1/sqrt(2) = 0.04289...,
        // which is about 4.5 bits.
        // The initial approximation is between x0(1.0) = 0.9571... and x0(2.0) = 0.4571...

        // Then, refine the reciprocal estimate using a quadratically converging
        // Newton-Raphson iteration:
        //     x_{n+1} = x_n * (2 - x_n * b)
        //
        // Let b be the original divisor considered "in infinite precision" and
        // obtained from IEEE754 representation of function argument (with the
        // implicit bit set). Corresponds to rep_t-sized b_UQ1 represented in
        // UQ1.(W-1).
        //
        // Let b_hw be an infinitely precise number obtained from the highest (HW-1)
        // bits of divisor significand (with the implicit bit set). Corresponds to
        // half_rep_t-sized b_UQ1_hw represented in UQ1.(HW-1) that is a **truncated**
        // version of b_UQ1.
        //
        // Let e_n := x_n - 1/b_hw
        //     E_n := x_n - 1/b
        // abs(E_n) <= abs(e_n) + (1/b_hw - 1/b)
        //           = abs(e_n) + (b - b_hw) / (b*b_hw)
        //          <= abs(e_n) + 2 * 2^-HW

        // rep_t-sized iterations may be slower than the corresponding half-width
        // variant depending on the handware and whether single/double/quad precision
        // is selected.
        // NB: Using half-width iterations increases computation errors due to
        // rounding, so error estimations have to be computed taking the selected
        // mode into account!

        let mut x_uq0 = if NUMBER_OF_HALF_ITERATIONS > 0 {
            // Starting with (n-1) half-width iterations
            let b_uq1_hw: u32 = (b_significand >> (significand_bits + 1 - hw)) as u32;

            // C is (3/4 + 1/sqrt(2)) - 1 truncated to W0 fractional bits as UQ0.HW
            // with W0 being either 16 or 32 and W0 <= HW.
            // That is, C is the aforementioned 3/4 + 1/sqrt(2) constant (from which
            // b/2 is subtracted to obtain x0) wrapped to [0, 1) range.

            // HW is at least 32. Shifting into the highest bits if needed.
            let c_hw = (0x7504F333_u64 as u32).wrapping_shl(hw.wrapping_sub(32));

            // b >= 1, thus an upper bound for 3/4 + 1/sqrt(2) - b/2 is about 0.9572,
            // so x0 fits to UQ0.HW without wrapping.
            let x_uq0_hw: u32 = {
                let mut x_uq0_hw: u32 = c_hw.wrapping_sub(b_uq1_hw /* exact b_hw/2 as UQ0.HW */);
                // dbg!(x_uq0_hw);
                // An e_0 error is comprised of errors due to
                // * x0 being an inherently imprecise first approximation of 1/b_hw
                // * C_hw being some (irrational) number **truncated** to W0 bits
                // Please note that e_0 is calculated against the infinitely precise
                // reciprocal of b_hw (that is, **truncated** version of b).
                //
                // e_0 <= 3/4 - 1/sqrt(2) + 2^-W0

                // By construction, 1 <= b < 2
                // f(x)  = x * (2 - b*x) = 2*x - b*x^2
                // f'(x) = 2 * (1 - b*x)
                //
                // On the [0, 1] interval, f(0)   = 0,
                // then it increses until  f(1/b) = 1 / b, maximum on (0, 1),
                // then it decreses to     f(1)   = 2 - b
                //
                // Let g(x) = x - f(x) = b*x^2 - x.
                // On (0, 1/b), g(x) < 0 <=> f(x) > x
                // On (1/b, 1], g(x) > 0 <=> f(x) < x
                //
                // For half-width iterations, b_hw is used instead of b.
                let mut idx = 0;
                while idx < NUMBER_OF_HALF_ITERATIONS {
                    // corr_UQ1_hw can be **larger** than 2 - b_hw*x by at most 1*Ulp
                    // of corr_UQ1_hw.
                    // "0.0 - (...)" is equivalent to "2.0 - (...)" in UQ1.(HW-1).
                    // On the other hand, corr_UQ1_hw should not overflow from 2.0 to 0.0 provided
                    // no overflow occurred earlier: ((rep_t)x_UQ0_hw * b_UQ1_hw >> HW) is
                    // expected to be strictly positive because b_UQ1_hw has its highest bit set
                    // and x_UQ0_hw should be rather large (it converges to 1/2 < 1/b_hw <= 1).
                    let corr_uq1_hw: u32 = 0_u64
                        .wrapping_sub(((x_uq0_hw as u64).wrapping_mul(b_uq1_hw as u64)) >> hw)
                        as u32;
                    // dbg!(corr_uq1_hw);

                    // Now, we should multiply UQ0.HW and UQ1.(HW-1) numbers, naturally
                    // obtaining an UQ1.(HW-1) number and proving its highest bit could be
                    // considered to be 0 to be able to represent it in UQ0.HW.
                    // From the above analysis of f(x), if corr_UQ1_hw would be represented
                    // without any intermediate loss of precision (that is, in twice_rep_t)
                    // x_UQ0_hw could be at most [1.]000... if b_hw is exactly 1.0 and strictly
                    // less otherwise. On the other hand, to obtain [1.]000..., one have to pass
                    // 1/b_hw == 1.0 to f(x), so this cannot occur at all without overflow (due
                    // to 1.0 being not representable as UQ0.HW).
                    // The fact corr_UQ1_hw was virtually round up (due to result of
                    // multiplication being **first** truncated, then negated - to improve
                    // error estimations) can increase x_UQ0_hw by up to 2*Ulp of x_UQ0_hw.
                    x_uq0_hw = ((x_uq0_hw as u64).wrapping_mul(corr_uq1_hw as u64) >> (hw - 1)) as u32;
                    // dbg!(x_uq0_hw);
                    // Now, either no overflow occurred or x_UQ0_hw is 0 or 1 in its half_rep_t
                    // representation. In the latter case, x_UQ0_hw will be either 0 or 1 after
                    // any number of iterations, so just subtract 2 from the reciprocal
                    // approximation after last iteration.

                    // In infinite precision, with 0 <= eps1, eps2 <= U = 2^-HW:
                    // corr_UQ1_hw = 2 - (1/b_hw + e_n) * b_hw + 2*eps1
                    //             = 1 - e_n * b_hw + 2*eps1
                    // x_UQ0_hw = (1/b_hw + e_n) * (1 - e_n*b_hw + 2*eps1) - eps2
                    //          = 1/b_hw - e_n + 2*eps1/b_hw + e_n - e_n^2*b_hw + 2*e_n*eps1 - eps2
                    //          = 1/b_hw + 2*eps1/b_hw - e_n^2*b_hw + 2*e_n*eps1 - eps2
                    // e_{n+1} = -e_n^2*b_hw + 2*eps1/b_hw + 2*e_n*eps1 - eps2
                    //         = 2*e_n*eps1 - (e_n^2*b_hw + eps2) + 2*eps1/b_hw
                    //                        \------ >0 -------/   \-- >0 ---/
                    // abs(e_{n+1}) <= 2*abs(e_n)*U + max(2*e_n^2 + U, 2 * U)
                    idx += 1;
                }
                // For initial half-width iterations, U = 2^-HW
                // Let  abs(e_n)     <= u_n * U,
                // then abs(e_{n+1}) <= 2 * u_n * U^2 + max(2 * u_n^2 * U^2 + U, 2 * U)
                // u_{n+1} <= 2 * u_n * U + max(2 * u_n^2 * U + 1, 2)

                // Account for possible overflow (see above). For an overflow to occur for the
                // first time, for "ideal" corr_UQ1_hw (that is, without intermediate
                // truncation), the result of x_UQ0_hw * corr_UQ1_hw should be either maximum
                // value representable in UQ0.HW or less by 1. This means that 1/b_hw have to
                // be not below that value (see g(x) above), so it is safe to decrement just
                // once after the final iteration. On the other hand, an effective value of
                // divisor changes after this point (from b_hw to b), so adjust here.
                x_uq0_hw.wrapping_sub(1_u32)
            };

            // Error estimations for full-precision iterations are calculated just
            // as above, but with U := 2^-W and taking extra decrementing into account.
            // We need at least one such iteration.

            // Simulating operations on a twice_rep_t to perform a single final full-width
            // iteration. Using ad-hoc multiplication implementations to take advantage
            // of particular structure of operands.
            let blo: u64 = b_uq1 & lo_mask;
            // x_UQ0 = x_UQ0_hw * 2^HW - 1
            // x_UQ0 * b_UQ1 = (x_UQ0_hw * 2^HW) * (b_UQ1_hw * 2^HW + blo) - b_UQ1
            //
            //   <--- higher half ---><--- lower half --->
            //   [x_UQ0_hw * b_UQ1_hw]
            // +            [  x_UQ0_hw *  blo  ]
            // -                      [      b_UQ1       ]
            // = [      result       ][.... discarded ...]
            let corr_uq1 = negate_u64(
                (x_uq0_hw as u64) * (b_uq1_hw as u64) + (((x_uq0_hw as u64) * (blo)) >> hw) - 1,
            ); // account for *possible* carry
            let lo_corr = corr_uq1 & lo_mask;
            let hi_corr = corr_uq1 >> hw;
            // x_UQ0 * corr_UQ1 = (x_UQ0_hw * 2^HW) * (hi_corr * 2^HW + lo_corr) - corr_UQ1
            let mut x_uq0: FInt = (((x_uq0_hw as u64) * hi_corr) << 1)
                .wrapping_add(((x_uq0_hw as u64) * lo_corr) >> (hw - 1))
                .wrapping_sub(2); // 1 to account for the highest bit of corr_UQ1 can be 1
            // 1 to account for possible carry
            // Just like the case of half-width iterations but with possibility
            // of overflowing by one extra Ulp of x_UQ0.
            x_uq0 -= one;
            // ... and then traditional fixup by 2 should work

            // On error estimation:
            // abs(E_{N-1}) <=   (u_{N-1} + 2 /* due to conversion e_n -> E_n */) * 2^-HW
            //                 + (2^-HW + 2^-W))
            // abs(E_{N-1}) <= (u_{N-1} + 3.01) * 2^-HW

            // Then like for the half-width iterations:
            // With 0 <= eps1, eps2 < 2^-W
            // E_N  = 4 * E_{N-1} * eps1 - (E_{N-1}^2 * b + 4 * eps2) + 4 * eps1 / b
            // abs(E_N) <= 2^-W * [ 4 * abs(E_{N-1}) + max(2 * abs(E_{N-1})^2 * 2^W + 4, 8)) ]
            // abs(E_N) <= 2^-W * [ 4 * (u_{N-1} + 3.01) * 2^-HW + max(4 + 2 * (u_{N-1} + 3.01)^2, 8) ]
            x_uq0
        } else {
            // C is (3/4 + 1/sqrt(2)) - 1 truncated to 64 fractional bits as UQ0.n
            let c: FInt = 0x7504F333 << (Self::BITS - 32);
            let x_uq0: FInt = c.wrapping_sub(b_uq1);
            // E_0 <= 3/4 - 1/sqrt(2) + 2 * 2^-64
            x_uq0
        };

        let mut x_uq0 = {
            // not using native full iterations
            x_uq0
        };

        // Finally, account for possible overflow, as explained above.
        x_uq0 = x_uq0.wrapping_sub(2);

        // u_n for different precisions (with N-1 half-width iterations):
        // W0 is the precision of C
        //   u_0 = (3/4 - 1/sqrt(2) + 2^-W0) * 2^HW

        // Estimated with bc:
        //   define half1(un) { return 2.0 * (un + un^2) / 2.0^hw + 1.0; }
        //   define half2(un) { return 2.0 * un / 2.0^hw + 2.0; }
        //   define full1(un) { return 4.0 * (un + 3.01) / 2.0^hw + 2.0 * (un + 3.01)^2 + 4.0; }
        //   define full2(un) { return 4.0 * (un + 3.01) / 2.0^hw + 8.0; }

        //             | f32 (0 + 3) | f32 (2 + 1)  | f64 (3 + 1)  | f128 (4 + 1)
        // u_0         | < 184224974 | < 2812.1     | < 184224974  | < 791240234244348797
        // u_1         | < 15804007  | < 242.7      | < 15804007   | < 67877681371350440
        // u_2         | < 116308    | < 2.81       | < 116308     | < 499533100252317
        // u_3         | < 7.31      |              | < 7.31       | < 27054456580
        // u_4         |             |              |              | < 80.4
        // Final (U_N) | same as u_3 | < 72         | < 218        | < 13920

        // Add 2 to U_N due to final decrement.

        let reciprocal_precision: FInt = 220;

        // Suppose 1/b - P * 2^-W < x < 1/b + P * 2^-W
        let x_uq0 = x_uq0 - reciprocal_precision;
        // Now 1/b - (2*P) * 2^-W < x < 1/b
        // FIXME Is x_UQ0 still >= 0.5?

        let mut quotient: FInt = u64_widen_mul(x_uq0, a_significand << 1).1;
        // Now, a/b - 4*P * 2^-W < q < a/b for q=<quotient_UQ1:dummy> in UQ1.(SB+1+W).

        // quotient_UQ1 is in [0.5, 2.0) as UQ1.(SB+1),
        // adjust it to be in [1.0, 2.0) as UQ1.SB.
        let (mut residual, written_exponent) = if quotient < (implicit_bit << 1) {
            // Highest bit is 0, so just reinterpret quotient_UQ1 as UQ1.SB,
            // effectively doubling its value as well as its error estimation.
            let residual_lo = (a_significand << (significand_bits + 1))
                .wrapping_sub(quotient.wrapping_mul(b_significand));
            a_significand <<= 1;
            (residual_lo, written_exponent.wrapping_sub(1))
        } else {
            // Highest bit is 1 (the UQ1.(SB+1) value is in [1, 2)), convert it
            // to UQ1.SB by right shifting by 1. Least significant bit is omitted.
            quotient >>= 1;
            let residual_lo =
                (a_significand << significand_bits).wrapping_sub(quotient.wrapping_mul(b_significand));
            (residual_lo, written_exponent)
        };

        //drop mutability
        let quotient = quotient;

        // NB: residualLo is calculated above for the normal result case.
        //     It is re-computed on denormal path that is expected to be not so
        //     performance-sensitive.

        // Now, q cannot be greater than a/b and can differ by at most 8*P * 2^-W + 2^-SB
        // Each NextAfter() increments the floating point value by at least 2^-SB
        // (more, if exponent was incremented).
        // Different cases (<---> is of 2^-SB length, * = a/b that is shown as a midpoint):
        //   q
        //   |   | * |   |   |       |       |
        //       <--->      2^t
        //   |   |   |   |   |   *   |       |
        //               q
        // To require at most one NextAfter(), an error should be less than 1.5 * 2^-SB.
        //   (8*P) * 2^-W + 2^-SB < 1.5 * 2^-SB
        //   (8*P) * 2^-W         < 0.5 * 2^-SB
        //   P < 2^(W-4-SB)
        // Generally, for at most R NextAfter() to be enough,
        //   P < (2*R - 1) * 2^(W-4-SB)
        // For f32 (0+3): 10 < 32 (OK)
        // For f32 (2+1): 32 < 74 < 32 * 3, so two NextAfter() are required
        // For f64: 220 < 256 (OK)
        // For f128: 4096 * 3 < 13922 < 4096 * 5 (three NextAfter() are required)

        // If we have overflowed the exponent, return infinity
        if written_exponent >= max_exponent as i64 {
            return Self::from_repr(inf_rep | quotient_sign);
        }

        // Now, quotient <= the correctly-rounded result
        // and may need taking NextAfter() up to 3 times (see error estimates above)
        // r = a - b * q
        let abs_result = if written_exponent > 0 {
            let mut ret = quotient & significand_mask;
            ret |= (written_exponent as u64) << significand_bits;
            residual <<= 1;
            ret
        } else {
            if (significand_bits as i64 + written_exponent) < 0 {
                return Self::from_repr(quotient_sign);
            }
            let ret = quotient.wrapping_shr((negate_u64(written_exponent as u64) + 1) as u32);
            residual = a_significand
                .wrapping_shl(significand_bits.wrapping_add(written_exponent as u32))
                .wrapping_sub(((ret).wrapping_mul(b_significand)) << 1);
            ret
        };
        // Round
        let abs_result = {
            residual += abs_result & one; // tie to even
            // conditionally turns the below LT comparison into LTE
            if residual > b_significand {
                abs_result + one
            } else {
                abs_result
            }
        };
        Self::from_repr(abs_result | quotient_sign)
    }

    pub const fn sub(self, rhs: Self) -> Self {
        self.add(rhs.neg())
    }

    pub const fn neg(self) -> Self {
        Self::from_repr(self.repr() ^ Self::SIGN_MASK)
    }
}

type SelfInt = u64;
type SelfSignedInt = i64;

impl SoftF64 {
    pub const BITS: u32 = 64;
    pub const SIGNIFICAND_BITS: u32 = 52;
    pub const EXPONENT_BITS: u32 = Self::BITS - Self::SIGNIFICAND_BITS - 1;
    pub const EXPONENT_MAX: u32 = (1 << Self::EXPONENT_BITS) - 1;
    pub const EXPONENT_BIAS: u32 = Self::EXPONENT_MAX >> 1;
    pub const SIGN_MASK: SelfInt = 1 << (Self::BITS - 1);
    pub const SIGNIFICAND_MASK: SelfInt = (1 << Self::SIGNIFICAND_BITS) - 1;
    pub const IMPLICIT_BIT: SelfInt = 1 << Self::SIGNIFICAND_BITS;
    pub const EXPONENT_MASK: SelfInt = !(Self::SIGN_MASK | Self::SIGNIFICAND_MASK);

    pub const fn repr(self) -> SelfInt {
        self.to_bits()
    }
    pub const fn signed_repr(self) -> SelfSignedInt {
        self.to_bits() as SelfSignedInt
    }
    const fn from_repr(a: SelfInt) -> Self {
        Self::from_bits(a)
    }
    const fn normalize(significand: SelfInt) -> (i32, SelfInt) {
        let shift = significand
            .leading_zeros()
            .wrapping_sub((1u64 << Self::SIGNIFICAND_BITS).leading_zeros());
        (
            1i32.wrapping_sub(shift as i32),
            significand << shift as SelfInt,
        )
    }
}

const ROWS1: usize = 10;
const COLS1: usize = 784;

static W1: [[f64; COLS1]; ROWS1] = [
    [-0.006168969, -0.05752615, 0.08154402, -0.071830735, 0.009086631, 0.020130157, 0.04913295, 0.06581616, -0.0029310212, 0.059977584, -0.02657472, 0.021833964, 0.104303725, 0.056618374, 0.07308509, -0.023667267, -0.062276274, 0.039039724, 0.04826323, -0.0015223697, 0.042576455, -0.040982954, 0.031359285, -0.013490178, 0.045184262, -0.010683283, -0.010206513, -0.009955712, -0.005847417, -0.058452144, -0.0013405457, -0.03180899, 0.099006034, 0.03705407, 0.19327286, 0.09013033, 0.19672354, 0.14945886, 0.1268032, 0.12294935, 0.08638903, 0.107681945, -0.003814304, -0.047613148, -0.15862502, 0.038761552, 0.12653296, 0.19065587, 0.1341134, 0.1483005, 0.08588752, 0.07307288, -0.011184633, 0.013623446, -0.0803237, -0.022818156, -0.030745357, -0.025885075, -0.04673775, -0.13602991, -0.066872016, 0.17688617, 0.324631, 0.25683883, 0.19529082, 0.18606618, 0.23114504, 0.259873, 0.19514695, 0.20809625, 0.22186555, 0.19170961, 0.18561277, 0.27864587, 0.1941973, 0.2005177, 0.1949056, 0.29394495, 0.18242711, 0.1303171, -0.06728843, -0.024239631, 0.03931559, -0.07160251, -0.019762449, -0.029644396, -0.045513302, -0.072843716, 0.11647452, 0.09806379, 0.22446002, 0.26640838, 0.08586332, 0.18226342, 0.11270505, 0.10893084, 0.1490669, 0.059773102, 0.035801377, 0.16423032, 0.06562325, 0.11496318, 0.0939242, 0.24563815, 0.16838628, 0.19989757, 0.18460904, 0.1539351, 0.048558626, 0.0150439795, 0.1399403, 0.0681572, -0.040960073, -0.09663496, -0.041607313, -0.015109213, 0.10242061, 0.13100778, 0.053697184, 0.05277823, -0.11193349, -0.05605494, -0.08321865, -0.10764798, -0.08776773, -0.04414613, -0.026300093, -0.051774234, 0.09496987, 0.057103973, 0.122814395, 0.19214728, 0.22500457, 0.21931735, 0.32780787, 0.3785932, 0.15057962, -0.013523997, -0.03934093, 0.049242012, -0.0035560876, -0.030920614, 0.06437841, -0.10860066, -0.023144199, 0.14214468, -0.030125992, 0.02798964, -0.0126809, 0.02774372, -0.0773749, -0.07542236, -0.11540543, -0.099806875, -0.060673885, -0.050605785, -0.09609128, -0.053375218, 0.12935938, 0.01964965, 0.17115216, 0.1583666, 0.13587706, 0.32927924, 0.29396597, 0.07742357, 0.14171408, 0.11696029, 0.003191188, -0.009506481, 0.16753504, 0.09423797, 0.049256083, 0.07871162, -0.024270546, 0.040257946, 0.033199333, 0.028942004, 0.07583334, 0.050829682, 0.02995852, -0.12208241, -0.11940163, -0.04653826, -0.12984025, -0.027560972, -0.024409395, -0.010967224, -0.030566165, 0.04623445, 0.10201593, 0.1876753, 0.32026672, 0.24827343, 0.19168809, 0.13608836, 0.08261959, -0.046526026, 0.06697132, 0.114047855, -0.01240698, 0.10350071, -0.06609108, 0.08704923, 0.050292615, 0.070254505, 0.06318958, 0.0755781, 0.04125158, -0.028012166, 0.04650585, -0.14741693, -0.016184958, 0.003368544, -0.08807404, -0.0001155801, 0.06751947, -0.019561548, -0.013475323, -0.028950052, 0.17031473, 0.18879151, 0.09450491, 0.111993276, -0.10159183, 0.25040358, 0.20545557, 0.050352108, 0.0073774, -0.06042228, 0.037162174, 0.068662986, 0.06682354, 0.101148866, 0.010581613, 0.07570422, 0.04155908, 0.06653241, 0.041731518, -0.052988164, -0.119897015, -0.095386215, -0.05270055, -0.12652515, -0.04209609, -0.010964308, -0.16054669, -0.05391322, -0.061077386, 0.14040555, 0.005871368, -0.10577564, 0.06445759, 0.12350299, 0.044228803, 0.02783498, 0.051356383, -0.100274295, -0.07003683, -0.018389935, 0.018630395, -0.03824817, 0.017831445, 0.112547874, 0.14783664, -0.030335102, 0.06520351, 0.10083056, -0.06616246, -0.16468132, -0.101799965, -0.13798214, -0.13876462, -0.07241693, -0.08110529, -0.12508003, -0.024470026, 0.22110365, 0.0040712054, 0.08614396, 0.0061285743, 0.1937742, 0.15607661, -0.054247703, -0.03857618, -0.030336246, 0.002859441, 0.02076356, 0.04083658, 0.115363695, 0.08667296, 0.043132856, 0.14576405, 0.16061126, 0.10847961, 0.1795086, 0.08685402, -0.08312287, -0.24299246, -0.163233, -0.06373841, -0.095179036, -0.09404077, -0.25867075, -0.07770179, 0.3181613, 0.12042953, -0.013463029, 0.0583466, 0.08170815, 0.31083912, 0.1204034, 0.13757257, 0.04859293, 0.038981456, 0.17319249, 0.18086208, 0.098147035, 0.20723891, 0.14906818, 0.14470582, 0.20603058, 0.11153424, 0.24497916, 0.20425396, 0.026131853, 0.026507784, -0.101601936, -0.13188307, -0.13544884, -0.07483005, -0.11139153, -0.21999565, 0.07534481, -0.055741332, -0.0417721, 0.04398557, 0.25858703, 0.35528353, 0.111497805, 0.22225294, 0.051420696, 0.036665734, 0.23251434, 0.18607217, 0.14510345, 0.14516507, 0.060257256, 0.1301188, 0.086763695, 0.106372036, 0.34286118, 0.33152708, 0.066104256, 0.039652713, -0.23315841, -0.09276939, -0.09505235, -0.12801501, -0.1727942, -0.30893397, -0.18384163, -0.21051483, 0.04911844, -0.0860315, 0.06560639, 0.20277037, 0.1737594, 0.100806504, 0.015155082, 0.13079727, 0.0714915, 0.11573376, 0.037697647, 0.09931833, 0.0022122988, 0.015819613, 0.1184602, 0.15391368, 0.30929366, 0.20450795, 0.12396448, -0.03046068, -0.034146577, 0.07931333, 0.007692081, 0.05970679, 0.0033985367, -0.11395274, -0.15307575, -0.2613573, -0.24241337, 0.026933657, 0.10857684, 0.1440409, 0.10374375, 0.11583764, 0.08908949, 0.08630515, 0.041898564, 0.038690396, -0.04056408, 0.07938646, -0.016276576, -0.11473283, 0.08848567, 0.2313747, 0.24113889, 0.25864515, 0.12293939, 0.014886334, 0.10042251, 0.062099136, 0.03540983, 0.15975893, 0.051496074, -0.053641777, -0.104248844, -0.3888088, -0.013480014, 0.08618764, -0.084611766, 0.0064999405, 0.124219835, -0.039877657, -0.08850474, -0.0053067403, -0.0053366357, -0.090530895, 0.013670719, -0.017690804, 0.073034, -0.03538619, 0.13143058, 0.27350402, 0.34508276, 0.16883157, 0.11098658, 0.06442858, 0.1694586, 0.115781926, 0.13284582, 0.093690395, 0.039159786, -0.062023696, -0.039269276, -0.3264613, -0.21059278, 0.078751184, -0.15758368, -0.11484878, -0.04257938, -0.21953295, -0.11588004, -0.065288976, 0.044088855, -0.0026755587, -0.100593716, -0.09454724, -0.04875209, 0.035255913, 0.26020184, 0.23985162, 0.21970735, 0.07630588, 0.042536296, 0.14810991, 0.07293415, 0.024083463, 0.014757566, 0.009875355, -0.04462895, -0.049101353, -0.04271174, -0.26046067, -0.05291655, -0.0047706217, -0.043402355, -0.0821633, -0.035853233, -0.059866138, -0.14229737, 0.008282545, -0.042920318, -0.09591111, -0.10166687, 0.02324543, -0.03688106, 0.12044049, 0.28065795, 0.32811716, 0.31607503, 0.006455475, 0.08084914, 0.067246415, 0.058691088, 0.09136144, 0.07977659, -0.0024296506, 0.043132607, 0.0045649423, 0.06515488, -0.23303488, -0.19784074, 0.030137155, -0.07554462, 0.08154171, -0.018083207, -0.14281595, -0.09578726, -0.11016467, -0.122824654, -0.057221666, -0.05311673, -0.12049317, 0.0064213346, 0.1545532, 0.25079772, 0.24803746, 0.16459414, 0.16805515, 0.10689783, 0.09118101, 0.07763714, 0.087320685, -0.056962896, -0.12211107, 0.004536711, -0.012164857, 0.11756991, 0.15038115, 0.04312399, 0.13003534, 0.038320795, 0.15204006, -0.07411953, -0.10845742, -0.23615974, -0.0286135, 0.030358983, -0.024777584, 0.010602572, -0.03212498, 0.08981482, 0.23005623, 0.26251057, 0.3284708, 0.16500108, 0.17375942, 0.05784902, 0.111294664, 0.05071866, 0.075194746, 0.004075885, -0.11117416, -0.008025049, 0.059803203, 0.05662587, -0.043453693, -0.0327503, 0.0072481483, -0.074985325, -0.041212495, -0.11260403, -0.1651016, -0.27059683, -0.07897692, -0.13512215, 0.004880246, 0.020238055, 0.0035091056, 0.14445797, 0.11781453, 0.13258202, 0.23695916, 0.18688212, 0.14055817, 0.15426114, 0.08357248, 0.035764016, 0.0473067, -0.14021178, -0.15255615, -0.13563722, 0.022609867, 0.06528822, 0.00068150467, 0.11312965, 0.09806565, 0.05474759, -0.06781008, -0.09110031, -0.07279407, -0.091785386, -0.18086855, -0.11242953, 0.06933622, 0.14699543, 0.072607376, 0.11203804, 0.1989728, 0.15335463, 0.17236134, 0.18234356, 0.15058076, 0.23206304, 0.13211589, 0.1264841, 0.04993527, 0.0032314206, -0.10991037, 0.031296868, 0.25867867, 0.04267671, -0.13848981, -0.0055490443, 0.005661957, -0.020904126, -0.21041924, -0.14528662, 0.078465454, -0.06051177, -0.05204137, -0.131553, 0.0070179044, 0.05808288, 0.016965536, 0.036477588, -0.013406123, -0.03175294, 0.014387733, 0.1712565, 0.07482484, 0.043913107, 0.10312969, -0.045247976, 0.026038952, -0.09788208, -0.1019832, -0.21046521, 0.06312278, -0.014968031, -0.0016936562, 0.04681027, 0.066696055, 0.0399232, -0.20546815, -0.20823793, 0.174538, 0.084956154, -0.003560865, -0.03993343, -0.15369622, -0.059864923, -0.1309808, -0.18782799, -0.13447483, -0.1602702, 8.560171e-05, 0.011795118, 0.047508918, 0.036315568, -0.05657721, 0.00051477045, -0.04926437, -0.15491647, -0.038516954, -0.15651381, -0.07523153, 0.035590366, 0.13289611, -0.057052113, -0.03338411, 0.010423981, 0.0075638373, -0.17005177, 0.10793373, 0.22465006, 0.0737915, 0.09392377, -0.1266016, -0.019209307, 0.00043034498, -0.16972047, -0.042644463, -0.05986658, -0.07614786, -0.063694865, -0.0999835, -0.03929004, -0.04541126, -0.10544209, -0.105637275, -0.0587913, -0.056222267, -0.24742372, -0.063858286, -0.18828522, 0.12458736, 0.0693116, 0.012471937, 0.010310471, -0.1333423, 0.08392649, 0.25291163, 0.1327368, 0.09276067, 0.14370672, 0.0020727122, 0.13931686, 0.018944701, 0.15425232, 0.120407194, 0.11230986, 0.06679558, 0.18248011, 0.0022875075, 0.03024418, 0.100454934, 0.0008158074, 0.015184446, 0.07413292, 0.062021136, -0.013585922, 0.085578606, 0.020892363, 0.02196028, 0.084279515, -0.08145088, 0.015130684, -0.044377297, -0.08353474, -0.1598479, 0.028445216, 0.07839706, 0.048068844, 0.14106497, 0.21324015, 0.12559307, 0.18195607, 0.17197464, 0.20731208, 0.1633131, 0.3268652, 0.22707489, 0.23470554, 0.12187753, 0.13305894, -0.0009563129, 0.19297472, 0.17637618, -0.014281721, -0.13386165, 0.022160618, -0.0061487406, 0.015718833, -0.018924788, -0.032394115, -0.078284375, 0.0018645748, -0.0319548, 0.025189962, 0.12465944, -0.030831242, 0.040127046, 0.067141004, 0.20291413, 0.16508032, 0.017832166, 0.24007383, 0.26579025, 0.16255818, 0.24652778, 0.30464733, 0.16429575, 0.23919551, -0.015367727, 0.1834818, 0.16224201, 0.04062588, 0.071410306, -0.04528083, 0.023383215, -0.07740748],
    [-0.015919276, -0.05119815, -0.025012158, 0.008211471, -0.02688272, -0.0343951, 0.048957147, -0.08390292, -0.020495072, -0.030345663, -0.030037913, 0.08173499, -0.08059376, 0.0023434334, 0.008613259, 0.023405686, 0.0848669, -0.07829012, -0.06698513, 0.04626935, -0.021580383, 0.011132963, -0.08023819, 0.021377176, 0.03318564, -0.023987345, -0.03413945, -0.064648576, -0.061640497, 0.0764832, -0.079596385, 0.01319845, 0.06980734, 0.057124026, -0.16408342, -0.08550832, 0.038686205, 0.19958864, 0.17805675, 0.19985202, 0.14568773, 0.028386852, -0.027143052, -0.14555807, -0.12133228, 0.022245716, 0.12313384, -0.022898443, -0.12768084, -0.09153153, -0.118213795, -0.17728178, -0.07629696, -0.05662886, 0.033986375, 0.08017094, -0.05601132, 0.05882656, -0.11780522, 0.043249395, 0.114441834, 0.18221594, 0.0732945, 0.09073475, -0.032615162, -0.051696256, -0.20366228, -0.14048913, -0.096591346, -0.19320802, -0.12586844, -0.19615291, -0.09523556, -0.080211185, -0.027266318, 0.09875424, -0.03729457, -0.024871672, -0.09705333, -0.1368803, -0.087722585, -0.13196605, -0.0758106, -0.04790385, 0.01883237, -0.08612436, -0.13052088, -0.029414514, 0.13089325, 0.079809, 0.08047771, 0.011738985, -0.01741192, -0.2156319, -0.08143917, -0.08118458, -0.10265546, -0.057120387, -0.00999606, -0.07618374, 0.027031725, 0.012871061, 0.00907955, -0.007313355, 0.06805007, 0.18027192, 0.06527353, 0.09242681, -0.09280947, -0.12859358, 0.018519785, -0.024348095, -0.003548354, 0.1001482, 0.11112249, 0.01373468, 0.11104468, -0.087727115, -0.16286528, 0.012605378, -0.035788175, -0.11042142, -0.12774858, -0.051277347, -0.02195795, -0.0304132, -0.0050429325, 0.032301296, 0.04450535, 0.024246678, 0.10536945, 0.055569433, 0.026949853, 0.14114654, 0.10455819, 0.07461558, 0.21162319, 0.08231413, 0.14066611, -0.01566804, -0.054623168, -0.040671762, -0.09492794, 0.023006922, -0.11119569, -0.08048008, -0.07041705, -0.071312934, -0.11459616, -0.08316723, -0.01093525, -0.0010418125, -0.04999442, -0.022538, 0.054086722, -0.008595231, -0.039642923, 0.05453767, 0.057984408, 0.057610985, 0.116207175, -0.042148277, 0.074135646, 0.05736475, 0.15371399, 0.18086042, 0.18865483, -0.1069811, 0.009091765, 0.07160561, -0.064489365, -0.06936258, -0.07295782, -0.019823955, 0.010221245, -0.05083701, -0.076639324, -0.0634169, -0.0010141985, 0.03283212, 0.09155159, 0.030993605, 0.11031079, 0.14031184, 0.08262013, 0.13496706, 0.17238106, 0.12702954, 0.13803732, 0.13768995, 0.11691015, 0.23756713, 0.17729568, 0.15200682, 0.087427266, 0.046982683, -0.06187882, 0.044093315, -0.07986931, -0.24324217, -0.31818044, -0.033688985, -0.05857071, -0.12010362, 0.023667056, -0.007732304, -0.0053909128, 0.08646651, 0.12673151, 0.1056155, 0.09817939, 0.15619227, 0.21848382, 0.21066523, 0.17630742, 0.1003245, 0.15404798, 0.10504716, 0.030253444, 0.23996873, 0.27961758, 0.24057198, 0.058533154, 0.051781744, 0.0716757, -0.21633612, -0.09768179, -0.29498783, -0.36616534, -0.17242444, -0.05977198, -0.15398806, 0.02849736, 0.0019982848, 0.029026879, 0.032385565, 0.08297467, 0.14990373, 0.14777775, 0.08676893, 0.18388505, 0.11145342, 0.20579667, 0.080448635, 0.097827934, 0.045738712, 0.04507003, 0.26396146, 0.4628568, 0.4214829, 0.14312693, 0.1569384, -0.13139914, -0.12091535, -0.08292659, -0.2465902, -0.28230134, -0.11461109, -0.058169458, -0.025142523, 0.0886454, 0.06400447, -0.044076785, 0.021051407, 0.06355412, 0.04071169, 0.052033227, 0.027387083, 0.03745903, 0.025270563, -0.02271293, 0.09190979, 0.03277787, 0.0913622, 0.0498511, 0.11942511, 0.6352708, 0.48349172, 0.2999507, 0.03471998, -0.091305204, -0.10950605, -0.21737501, -0.23844482, -0.06749787, -0.21346417, -0.03234052, -0.045738257, 0.08516907, 0.069048, 0.09906141, 0.07111393, 0.20993853, 0.20192768, 0.21941108, 0.08763265, 0.023707248, 0.047645558, 0.012393569, 0.04355468, 0.0783901, 0.11484553, -0.002125093, 0.17825313, 0.6443442, 0.5408902, 0.2904845, 0.00832223, 0.009102171, -0.12935397, -0.20127632, -0.15778705, -0.10301166, -0.028387228, 0.16119084, 0.1540459, 0.24853595, 0.2136661, 0.31308207, 0.3095867, 0.2801453, 0.42680505, 0.260511, 0.12134625, 0.022570385, 0.089714415, 0.0902618, -0.00920603, -0.02750868, -0.08383815, -0.015628468, 0.11453677, 0.21495539, 0.21335454, 0.22277735, 0.1073118, -0.019026607, -0.12761603, -0.23422828, -0.20625085, -0.019801944, 0.1590265, 0.2654171, 0.381995, 0.22786051, 0.22615582, 0.2961614, 0.33905166, 0.37113068, 0.3402767, 0.3565724, 0.17960052, 0.095779724, 0.0971205, 0.11158481, -0.047712825, -0.032633875, 0.013954252, 0.006769432, 0.10883266, -0.11560283, -0.07312202, -0.111012466, -0.13645747, 0.012797214, 0.014329585, -0.25094593, -0.04006145, 0.13182217, 0.34040564, 0.3145541, 0.38618377, 0.30626717, 0.16422662, 0.2461299, 0.19756222, 0.16888358, 0.19125143, 0.1332006, 0.14130332, 0.065709874, 0.07829132, 0.14992666, 0.0665401, -0.07615791, -0.03863543, 0.021889346, -0.013070944, -0.22600819, -0.4491069, -0.33193868, -0.21411993, 0.051189214, 0.0008786144, -0.12164747, -0.06938479, 0.070186555, 0.20057982, 0.14577939, 0.19028956, 0.09516639, 0.12423214, 0.11137466, 0.1371619, 0.10674651, 0.006429583, 0.0089474255, 0.047976553, 0.047983788, 0.0982412, 0.0043973383, -0.0023921148, -0.03974207, 0.043348085, 0.044562418, 0.06181585, -0.14517301, -0.4872544, -0.41878235, -0.15699278, 0.1033062, 0.07724368, 0.10443036, -0.18986125, 0.04231604, 0.012135791, -0.07340257, -0.028477928, 0.0014452076, 0.15985425, 0.025491897, 0.024348432, 0.06358574, -0.11244895, -0.049091328, -0.08898505, -0.038788535, -0.030822873, 0.04717054, 0.050502222, 0.0551754, 0.023965623, 0.13001476, 0.07538713, -0.12706979, -0.48750862, -0.46871203, -0.24378459, -0.10476859, 0.03030976, 0.001826142, -0.077220604, -0.13626048, -0.07611156, -0.16299, -0.07369551, -0.023413075, -0.027032183, -0.13466147, -0.09083847, -0.08838208, -0.23356289, -0.13605079, -0.02644033, -0.016088678, -0.032754675, 0.050846532, 0.06998941, 0.054140635, 0.009627706, 0.049090177, 0.057040844, -0.20494957, -0.67492616, -0.527879, -0.13186963, 0.08088619, 0.03032865, -0.088214755, -0.043756418, -0.09324678, -0.024211721, 0.031407353, -0.00439626, -0.096113615, -0.08261165, -0.16680814, -0.30082035, -0.21517901, -0.22370526, -0.1555185, -0.07715736, 0.042461086, 0.00364602, -0.012205725, 0.08780637, 0.0022852535, 0.052965727, 0.023417383, -0.008059915, -0.10492601, -0.73283404, -0.2905373, -0.077696346, -0.029058103, -0.048458662, 0.009317027, -0.025086824, -0.031791285, -0.050115738, 0.06254519, -0.0020543386, -0.0020661014, -0.07500767, -0.08835824, -0.08603357, -0.31668526, -0.24221528, -0.06163438, 0.08668703, 0.043479618, 0.09399842, 0.10856586, 0.027158488, 0.12484757, -0.01788601, 0.008450245, -0.10411085, -0.24057512, -0.5924929, -0.1740827, -0.18904988, 0.073772885, -0.063441485, 0.1239645, 0.036687735, -0.056267407, 0.06362957, 0.13101149, 0.010132818, 0.066185206, 0.032046143, -0.08636321, -0.13514598, -0.1509747, -0.050060995, -0.08539411, 0.062850855, 0.03963272, 0.04486083, 0.071220234, 0.119015984, 0.113035135, 0.02037117, 0.010909782, -0.13152698, -0.10619571, -0.4253279, -0.2109207, -0.14270692, 0.047394894, 0.061715435, 0.0882075, 0.0035848587, -0.048591647, 0.02891264, 0.034403168, -0.0891347, 0.0444534, 0.05580107, 0.05561474, -0.0016895488, -0.016888475, 0.006740334, 0.052824546, 0.10800968, 0.06379879, 0.12100612, 0.059777405, 0.009065013, -0.07086516, -0.014529807, -0.094434805, -0.15513709, -0.18932249, -0.28110933, -0.12683992, 0.11843937, -0.0030739494, 0.0065977047, -0.1047438, 0.0009689088, -0.024769932, 0.04844063, -0.012076991, 0.045031004, 0.03987894, 0.06377309, -0.08041015, 0.039697148, 0.077966884, 0.016224222, 0.043709915, 0.15433826, 0.015151114, 0.024824506, 0.039954323, -0.1045199, -0.018148854, -0.030839832, -0.07915105, -0.19381851, -0.23699309, -0.24543, -0.05460513, 0.020627147, -0.0690374, 0.014902076, -0.10438361, -0.040194433, -0.06691971, -0.07276443, -0.026367985, -0.026094396, -0.031580906, -0.011000645, 0.072850615, 0.12617172, 0.1161594, 0.17490293, 0.15768689, 0.08558562, 0.16628069, 0.103341624, -0.020543301, -0.02484354, -0.066644974, -0.16340446, -0.10272678, -0.11206386, -0.16743176, -0.098587975, -0.16171916, -0.03243108, 0.026436627, -0.08196531, 0.046105143, 0.21711136, 0.04317399, 0.053709958, -0.07471185, -0.07417699, 0.06811487, 0.082559645, 0.21112005, 0.059325144, 0.13016681, 0.15282206, 0.1902938, 0.18798813, 0.04549477, 0.08792941, -0.044549868, -0.17414927, -0.12279166, -0.16503145, -0.14691047, -0.2549118, -0.17297255, -0.2229812, -0.06743464, 0.031187221, 0.05879978, -0.0377243, 0.0157057, 0.4475961, 0.29577604, 0.1270686, 0.24359769, 0.22354296, 0.20296036, 0.20938894, 0.2202524, 0.20404403, 0.1773365, 0.24951136, 0.087663524, 0.086385876, 0.1637215, 0.028452728, -0.011664334, 0.0326202, 0.0002879572, -0.21897526, -0.032775607, -0.055873536, -0.23863721, -0.22059491, -0.17604603, 0.056920536, 0.059367158, -0.021417193, 0.16165404, 0.08838442, 0.18181463, 0.21300547, 0.21961989, 0.2545247, 0.19439176, 0.17702073, 0.24910098, 0.16781697, 0.12530334, 0.19336791, 0.009501387, 0.15036915, 0.05772127, 0.1316285, 0.24158502, 0.2166791, 0.20685036, 0.2865495, 0.34738725, 0.41240633, 0.18904948, 0.0132578695, -0.16207652, -0.010915138, -0.0073199198, -0.00027662516, -0.014678687, 0.027571091, 0.34831816, 0.120789334, 0.08610065, -0.07163601, -0.0069199386, -0.078656785, 0.015141448, 0.0435742, 0.08172963, -0.011583045, 0.074416794, -0.004849444, 0.011161545, -0.00018588618, 0.045541722, 0.01309186, 0.14634874, -0.07089379, -0.019939104, -0.041721098, 0.18249941, -0.008812696, -0.027947765, -0.072224066, -0.04532715, -0.066880554, -0.06813345, -0.0756832, -0.1061456, -0.13427742, 0.063226886, 0.055501267, -0.025225552, -0.102814704, -0.12700659, -0.025710562, 0.008064753, -0.13321736, -0.13806997, -0.1289029, -0.0863163, -0.13438603, -0.05215101, -0.09013594, -0.14322068, -0.10372842, -0.21367362, -0.19469751, -0.05545082, -0.015176535, -0.039358757, 0.041543014],
    [-0.0819605, -0.011573754, -0.061372124, -0.0047696456, -0.008219287, -0.051244203, 0.016364314, 0.01284226, 0.039010175, -0.06809405, -0.011986256, -0.05251449, -0.05224754, -0.03151746, -0.050002214, -0.046603452, 0.017954744, -0.0705557, -0.058297306, -0.0531009, -0.07302733, -0.012518048, -0.037333753, -0.015414506, -0.032230485, -0.021340758, 0.011263929, 0.04882083, -0.02378504, -0.008065686, 0.0005822852, 0.073278226, -0.0071513206, 0.0015700608, -0.021416536, -0.0061051785, 0.10956591, 0.091227904, 0.053426664, 0.14576156, 0.018917436, 0.07151635, -0.14127722, -0.14139779, -0.2823225, -0.09494468, 0.01442208, -0.105132654, -0.12295414, -0.08058874, -0.0798355, 0.03042283, 0.071386255, 0.045741625, -0.04530721, -0.037586894, 0.07567387, -0.060488634, 0.04449647, 0.106582604, 0.102099635, -0.04791057, -0.0069958204, 0.08028986, 0.15329279, 0.1588981, -0.0041451324, -0.030717494, 0.005952991, -0.1600858, -0.25157458, -0.3296298, -0.08083091, -0.014233187, 0.02300046, -0.11729442, -0.007936565, -0.1638633, -0.04731898, -0.18205567, 0.09658304, 0.05343307, 0.06415025, -0.017225124, 0.04938156, 0.005431786, 0.075121574, 0.1034111, 0.13067973, -0.2583943, -0.11897546, -0.10527095, 0.018267918, -0.017576061, 0.120253965, 0.031294122, 0.15845704, 0.039044213, 0.048319448, 0.09699021, 0.03235233, 0.029574415, 0.06589428, -0.017796395, 0.051305056, 0.08771605, -0.040443715, -0.0809673, 0.0022176574, 0.109472945, 0.04010301, 0.041145362, 0.043516643, 0.016506322, -0.14642157, -0.0030156844, -0.17373838, -0.22185396, -0.12795979, -0.14678872, -0.19801037, -0.07035778, -0.042009946, 0.10052798, 0.056726314, -0.03141359, 0.067540005, 0.16945821, 0.090195164, 0.012325221, 0.07919054, -0.011393327, -0.09115778, -0.022557503, -0.06892431, 0.034609713, 0.14016268, 0.15246019, 0.20957433, 0.006088338, 0.039961122, -0.049146283, -0.15119113, -0.05094597, -0.309845, -0.164977, -0.070365004, -0.069827534, -0.078474104, 0.04800795, 0.03386941, 0.04571066, 0.009990057, 0.10910065, -0.016615627, -0.07310299, 0.09651296, 0.07803924, 0.04823862, -0.022035627, 0.061143376, 0.015303554, 0.07439362, 0.011255638, 0.050851163, 0.30185312, 0.07456288, -0.00496491, 0.046080984, 0.12649451, 0.090707175, -0.0638161, -0.0031150247, 0.007891323, 0.0515134, 0.089937955, 0.08655685, -0.007266615, 0.16451383, 0.12623768, 0.11213284, 0.09481924, 0.10237891, 0.09257155, 0.08829533, 0.08710415, 0.099045165, 0.08328079, 0.07384256, 0.09462618, 0.14561109, 0.013860192, 0.13589114, 0.25996247, 0.12784418, -0.026690803, -0.03728143, -0.046540994, 0.17147459, 0.13520798, -0.15927278, 0.01745271, 0.08872481, 0.14352159, 0.24115011, 0.19523397, 0.21250775, 0.19596559, 0.16738455, 0.09635496, 0.049451858, 0.033099886, 0.14082141, 0.16924307, 0.20129168, 0.15948701, 0.16044623, 0.088932335, 0.08986306, 0.23556106, 0.2703049, 0.30769575, 0.05326436, 0.08657826, 0.044806264, 0.16333261, 0.20755741, 0.073804125, -0.06658151, 0.18752465, 0.070145816, 0.167973, 0.2177388, 0.24286047, 0.32727352, 0.2796214, 0.28163442, 0.11548528, 0.08324212, 0.11377584, 0.14367233, 0.16370697, 0.27066192, 0.20377432, 0.22024156, 0.22693853, 0.20656377, 0.3622853, 0.3922834, 0.3824035, 0.24786349, 0.111070625, 0.061178952, 0.10323188, 0.24652861, 0.066625684, -0.010986425, 0.039632548, 0.030332753, 0.07817153, 0.20079692, 0.17746532, 0.2400885, 0.28421664, 0.31572908, 0.17505932, 0.20123212, 0.07325645, 0.08393079, 0.29033098, 0.14103273, 0.1401718, 0.21494323, 0.22892529, 0.23953964, 0.42914698, 0.6339889, 0.65015227, 0.42163673, 0.10179209, 0.008156121, 0.113563694, 0.31043813, 0.2082955, 0.10995966, 0.043610148, 0.004996585, 0.008532776, 0.12768903, 0.15623355, 0.11905897, 0.21947621, 0.12918909, 0.26182008, 0.11237063, 0.10257827, 0.09346475, 0.055749424, 0.12387371, 0.069823876, 0.084637254, 0.054017004, 0.18092598, 0.25556684, 0.5909596, 0.72457796, 0.53150296, 0.13765176, -0.0071138293, 0.19430447, 0.266616, 0.055936895, 0.07050763, -0.081220634, 0.035111412, 0.10271979, 0.013850213, -0.04180965, 0.039210614, 0.012959529, 0.08360727, 0.13992769, 0.010103916, -0.21559197, -0.092818305, -0.1474948, -0.11467899, -0.17183381, -0.26646098, -0.20653126, -0.15534419, -0.29211327, -0.05473473, 0.47417286, 0.54084355, -0.17872468, -0.05401645, 0.11472019, 0.18349357, -0.124473065, 0.04484456, 0.01856866, -0.07609704, -0.040730156, -0.11132911, -0.09144252, -0.12481453, -0.12307373, -0.17643414, -0.13028032, -0.23192272, -0.34202987, -0.2785431, -0.17761333, -0.19969946, -0.25859755, -0.1752024, -0.36383414, -0.4801773, -0.63049364, -0.48091283, -0.003187193, 0.32075202, -0.06684693, 0.059532978, 0.023449246, 0.07411561, -0.09447322, -0.16949956, 0.038847275, -0.114323854, -0.016979825, -0.15173712, -0.05783972, -0.049167603, -0.06606714, -0.15465374, -0.2027536, -0.20977823, -0.23779348, -0.27427605, -0.103009686, -0.033420656, -0.11937383, -0.08536184, -0.25558275, -0.23805495, -0.3339042, -0.27126735, -0.082253136, 0.17186572, 0.15040255, 0.03681268, -0.057359375, 0.094903834, -0.15036474, -0.0071810745, -0.08705039, -0.16071956, -0.09284821, -0.02846951, -0.032203145, -0.084333256, -0.14118877, -0.2706421, -0.15778838, -0.18143344, -0.25994855, -0.22129603, -0.21860793, -0.006173371, -0.044870175, -0.059026938, -0.08395635, -0.093726225, 0.014659999, 0.12302411, 0.009424735, 0.09321284, 0.14839555, 0.19766888, -0.12493207, 0.118638106, -0.03669542, -0.033982344, -0.22434686, -0.11436523, -0.109666936, -0.18587102, -0.032205407, -0.10457983, -0.21609206, -0.17996803, -0.12844639, -0.22008105, -0.24451466, -0.21249923, -0.08424421, 0.017083734, -0.013024138, 0.034746755, 0.05419162, -0.024293398, 0.06504067, 0.17697965, 0.059543032, 0.18397325, 0.10197269, 0.08241545, -0.039603595, -0.03392973, 0.053411786, 0.048400924, -0.037718475, -0.14751647, -0.17980811, -0.2867739, -0.17034298, -0.105201416, -0.16883379, -0.041488376, -0.09091316, -0.2219354, -0.14116357, -0.15062876, 0.016736634, 0.033878997, 0.058647238, -0.0022355882, 0.10110563, 0.010555891, 0.11135588, 0.24068192, 0.14576995, 0.19099373, 0.11645193, -0.018717818, -0.04078968, -0.00066471176, 0.09236613, 0.21195348, 0.0870289, -0.011930882, -0.19029914, -0.2420605, -0.20507914, -0.094540276, -0.1066423, -0.10892444, -0.12913024, -0.23544209, -0.14998762, 0.03728622, -0.023131244, 0.020700729, -0.05462434, 0.027226262, -0.0046043224, 0.08234487, 0.14250442, 0.25898814, 0.2810899, 0.22205344, 0.10277365, 0.032548986, -0.09172433, -0.05007669, 0.04231427, 0.17996323, 0.14510529, 0.2042589, -0.0067888265, -0.16911416, -0.11440895, -0.13866368, -0.113180414, -0.047982745, -0.06946419, 0.022166008, -0.053864937, -0.014290237, 0.038864076, 0.009021433, -0.0038113224, 0.013412398, 0.028368529, 0.04320208, 0.076482356, 0.17858987, 0.254453, 0.14389956, 0.16560875, -0.053354744, 0.09244923, 0.1501466, -0.0007251858, 0.32167244, 0.22819582, 0.21875697, 0.17655377, 0.16214876, 0.01896725, 0.12321061, 0.043976486, 0.17325398, 0.06689662, 0.034983918, 0.049145263, -0.0678216, -0.11094866, -0.07926111, -0.025301922, 0.09764065, 0.035456274, 0.0952084, 0.13696237, 0.3110545, 0.06828966, 0.050838295, 0.02685602, 0.050909363, 0.007907592, 0.065607175, 0.062474806, 0.20991865, 0.31876767, 0.14959925, 0.078258544, 0.23262249, 0.18176657, 0.10572047, 0.11278409, 0.166731, 0.07688408, 0.06457229, -0.019572504, -0.01676701, -0.0067648017, -0.02485705, -0.023041625, -0.05745993, 0.033062387, 0.17666206, 0.24839091, 0.016138611, 0.009831886, -0.026958764, -0.009177421, -0.08007308, -0.087117195, -0.14592654, 0.10834102, 0.19602716, 0.07068223, 0.16707778, 0.19856478, 0.11697625, 0.10638037, 0.116467215, 0.08080947, 0.0690055, 0.090239756, -0.02553605, 0.004730719, -0.03533992, -0.008675242, -0.07335842, 0.01591294, -0.091325805, 0.18149933, 0.22095174, 0.20556311, 0.10016117, 0.013185025, 0.15990396, 0.06611137, -0.003591268, -0.051252343, -0.13185956, -0.0762248, 0.10990728, 0.05708442, 0.01693093, 0.17226742, 0.04116833, 0.1481531, 0.0012467654, 0.11781845, 0.07360378, 0.15060587, 0.11794826, 0.1059106, 0.11489152, -0.017328389, 0.008534902, -0.012596292, 0.06062495, -0.025450606, 0.17636357, 0.216073, 0.056458518, -0.05928262, 0.09082146, -0.008972442, -0.0058823973, -0.029346302, -0.21955955, -0.076763056, 0.09986057, 0.14767227, 0.08058379, 0.1412366, 0.067557916, 0.100887924, 0.1830397, 0.14823501, 0.20246239, 0.14151873, 0.15751827, 0.108965866, 0.041492328, 0.08594809, -0.07422541, -0.030762525, -0.04374778, -0.04248467, -0.021917932, 0.064360626, -0.021558084, -0.04687503, 0.059565336, 0.05756899, 0.0347647, 0.07183092, -0.00023495543, 0.06717605, 0.20846626, 0.08875603, 0.12797262, 0.006959774, 0.0891113, 0.045217924, 0.17295069, 0.13422391, 0.25297913, 0.21555443, 0.25922942, 0.2456141, 0.13733374, 0.1272178, 0.13890009, 0.051840726, -0.07030488, -0.10993817, -0.07133416, -0.038919933, 0.002603789, -0.14600179, -0.09002613, -0.06064105, -0.084034115, 0.06515833, -0.18676563, 0.13445616, 0.15635191, -0.01933532, 0.008886653, 0.03302313, 0.036268312, 0.1072318, 0.07702747, 0.15938427, 0.27403045, 0.15110083, 0.18768899, 0.24350347, 0.21441899, 0.274199, 0.13153166, 0.18529765, -0.094212, -0.034604326, 0.2155987, 0.12559368, 0.10526102, -0.112563655, -0.071603775, -0.07324919, -0.05016921, 0.046593763, -0.012790218, -0.037253805, -0.1475311, -0.07549488, -0.2306348, -0.11922552, -0.034697726, -0.0035891791, 0.002387756, 0.06570462, 0.040016282, 0.09176476, 0.118908234, 0.15913244, 0.13851662, 0.13620706, 0.11602756, 0.27983323, 0.11041048, 0.022012921, 0.2246104, 0.013355308, -0.0625318, 0.010850402, 0.04068274, 0.058750592, -0.06829231, -0.03942135, -0.04890663, -0.045651253, 0.096178584, 0.18129191, 0.026905641, 0.114308126, 0.22966816, 0.07870371, 0.22946982, 0.17214093, 0.15710391, 0.43316185, 0.18511167, 0.004125713, 0.27883285, 0.25749537, 0.1506476, 0.08413077, 0.050059307, -0.0067911106, 0.09333823, 0.073699854, 0.052081965, 0.015028946, 0.07163546, -0.0020574406],
    [-0.08169022, -0.0075861216, -0.03547013, -0.07632019, 0.062915556, 0.046935298, 0.05383935, -0.06790759, 0.032000102, -0.021491326, 0.027242102, 0.027256899, -0.093579434, -0.07729731, 0.08382439, -0.027495356, -0.0047109947, -0.057700552, -0.04315352, -0.002519101, 0.012006752, -0.006807342, 0.03752312, -0.021227472, 0.053176664, -0.049704276, -0.07837988, -0.04504746, 0.05198752, -0.06626718, 0.07922817, 0.037698604, -0.075834006, -0.069846325, -0.10534507, -0.1827173, -0.0060112104, -0.043569915, -0.08700418, -0.20572124, -0.24557677, -0.2111694, -0.079470396, -0.21498644, -0.18649457, -0.07258948, -0.28379866, -0.20850788, -0.31528905, -0.18579811, -0.179897, -0.0708477, 0.055667467, 0.042247854, -0.083146006, -0.07733478, -0.015298754, 0.07871137, -0.03213888, -0.19662684, -0.16079672, -0.040267807, -0.15329961, -0.24792646, -0.045176413, 0.07016528, -0.18989621, -0.07560538, -0.29376304, -0.31411526, -0.381469, -0.32903886, -0.45154437, -0.28673145, -0.051473696, -0.07841386, -0.045660026, -0.16740103, -0.114108734, -0.238112, -0.0060839546, 0.04475706, 0.06456479, 0.085534595, 0.06407983, -0.06375189, -0.10956764, -0.12247509, -0.028975548, 0.0741547, 0.1009005, -0.03715102, -0.19329391, -0.08000399, 0.033814132, -0.07790169, -0.09279767, -0.22071631, -0.2535924, -0.25396883, -0.2059747, -0.042826544, 0.06543214, -0.009322498, 0.059452143, 0.067682154, 0.21768923, 0.17851841, 0.21033418, 0.16214165, 0.06735318, 0.006044969, 0.07171162, -0.11559223, -0.050215572, -0.12909834, -0.16975228, -0.20396295, -0.05699215, -0.10895943, -0.14663492, -0.0030616096, -0.090549245, -0.005582484, 0.047337696, -0.070097186, -0.09008747, -0.06892288, -0.04954784, -0.012055115, -0.12510431, -0.07194262, -0.095916405, -0.19218238, -0.17968811, -0.07843051, 0.15607826, 0.3201592, 0.08309424, 0.021712026, -0.08172689, -0.06829478, -0.1304479, -0.0562077, -0.30405977, -0.28405863, -0.265698, -0.05545462, -0.0569126, -0.05901449, -0.074913494, 0.1927474, 0.101629116, 0.111642405, 0.04212619, 0.03741688, -0.06856304, -0.017418839, -0.085623376, -0.038145814, -0.11060148, -0.17301163, -0.28772387, -0.21434872, -0.10117708, 0.22994228, 0.15455502, -0.15082103, 0.018615618, -0.18103676, -0.051328305, -0.24624762, -0.1782754, -0.20852877, -0.25363645, -0.09632532, 0.04570157, 0.070624664, 0.07824815, 0.087929785, 0.22795007, 0.20435709, 0.20353206, 0.2817208, 0.29325762, 0.20838138, 0.13093308, 0.1236846, 0.049819432, -0.12250622, -0.10340224, -0.1241766, -0.005091154, 0.43471044, 0.100586325, -0.07627336, -0.053853124, -0.19007337, -0.18859744, -0.34100145, -0.28751165, -0.25316966, -0.106890425, -0.036802027, 0.10012644, 0.038429257, 0.10119653, 0.11629416, 0.16670385, 0.25668272, 0.26000765, 0.30256045, 0.30638605, 0.18403602, 0.0910438, 0.022302274, 0.09575419, 0.027483359, -0.08037387, 0.011912416, 0.11942276, 0.36752725, 0.051034052, -0.06282901, -0.097464934, -0.24754272, -0.15536606, -0.28476644, -0.15337628, -0.12573676, -0.016209152, -0.024567612, 0.16105859, 0.10640182, 0.18715957, 0.09235635, 0.052451503, 0.18651518, 0.16563143, 0.16293046, 0.06240686, 0.0070288046, 0.07318848, 0.002467837, 0.049638156, 0.14407763, 0.0066702613, 0.114552006, 0.26702386, 0.35284594, 0.2222591, 0.2537112, -0.055838075, -0.09594455, -0.29431945, -0.21376698, -0.07729443, 0.009106845, 0.15485671, 0.007875097, 0.071840815, 0.18507707, 0.19779088, 0.16261679, 0.17995292, 0.02468232, -0.05598272, -0.11569402, -0.08178995, -0.03304511, -0.061947946, 0.025876218, 0.07382415, 0.0711252, 0.038076688, 0.13219671, 0.13642561, 0.34839714, 0.31028143, -0.09733761, -0.035500534, -0.07091466, -0.23871407, -0.16294433, -0.06591094, 0.07119948, 0.17539917, 0.15158758, 0.19819975, 0.16772115, 0.22646056, 0.12333339, 0.057152163, -0.06341269, -0.14517424, -0.23839732, -0.23155779, -0.11420474, -0.005891831, -0.02096224, -0.04420228, 0.0021997083, -0.0724547, 0.124251574, 0.07339582, 0.23164122, 0.2774767, 0.020678958, -0.14750494, -0.23268509, -0.3765904, -0.30258557, 0.045305554, 0.12129102, 0.11628267, 0.20316641, 0.14098263, 0.12737794, 0.19966638, 0.08768785, 0.026021576, -0.037812117, -0.06518363, -0.070114225, 0.009696296, -0.017053686, 0.04549151, 0.00460259, 0.1392923, -0.02236816, 0.06505655, 0.087591775, 0.18890084, 0.13386565, 0.3304935, 0.007795621, -0.09444105, -0.15641509, -0.22300944, -0.05916217, 0.028461246, 0.10971303, 0.10835137, 0.10184937, -0.015194913, 0.12246166, 0.09731897, -0.025632858, -0.00032105902, 0.0441505, -0.05044397, 0.08629819, 0.10564608, 0.0790878, 0.08947073, 0.13930975, 0.07516677, 0.14092442, -0.070046574, -0.05370152, -0.04931251, 0.1117612, 0.23670258, -0.0819824, 0.005333667, -0.13138162, -0.21699819, -0.04340074, 0.12586583, 0.015241288, -0.14414437, -0.061410956, -0.03467364, -0.034680773, 0.0016682696, 0.094595015, 0.084261596, 0.13660881, 0.15418543, 0.17316493, 0.25812832, 0.22832865, 0.105290964, 0.14853145, 0.06387028, -0.01924965, 0.00974515, -0.18829322, -0.24381126, -0.094442986, 0.049065717, 0.13243759, -0.09640286, -0.11621967, -0.30290875, 0.04439294, -0.005931149, -0.1948208, -0.035226654, -0.1697907, -0.034944735, -0.041505, 0.10682216, 0.084328614, 0.14640976, 0.101715036, 0.063765995, 0.11618664, 0.2734819, 0.2144789, 0.04898103, -0.019837849, -0.06420789, -0.08990897, -0.23792641, -0.23479863, -0.23371531, -0.04526966, 0.15735082, 0.06618559, -0.15901741, -0.08288336, -0.31539476, 0.03852153, -0.048170187, -0.1712326, -0.12093065, -0.15254904, -0.07864684, -0.057612937, 0.10812965, 0.07958796, 0.17250644, 0.045955084, -0.04757495, 0.11400692, 0.18507588, 0.117040545, -0.04132786, -0.012315738, -0.0870823, -0.23036596, -0.2389921, -0.26846224, -0.23568282, 0.031273223, 0.013490077, 0.020873394, 0.03580843, -0.13867407, -0.1700815, 0.06322756, -0.07876626, -0.10088867, -0.0891094, -0.11686675, -0.051982593, -0.0587027, 0.08224667, 0.08984132, 0.16771694, 0.031571865, -0.043857846, 0.094118915, 0.17421491, 0.14945775, -0.043971613, -0.10140149, -0.20201686, -0.22099453, -0.12397434, -0.24669461, -0.10849918, 0.051628277, 0.030469105, 0.094247736, 0.013731137, -0.15748417, -0.24337314, -0.06480692, -0.04517718, -0.06999608, 3.1846146e-05, -0.0077798693, 0.016139992, -0.07393517, 0.07080956, 0.14285503, 0.21531661, -0.058158822, -0.06041396, 0.025536362, 0.118163384, 0.053430893, 0.021990653, -0.054803256, 0.02551161, -0.076076604, -0.08699144, -0.09361229, 0.01991773, -0.005095365, 0.0275425, -0.07255138, 0.043079276, -0.086394265, -0.06359476, -0.16607821, 0.036156878, 0.19576672, 0.3068748, 0.21132623, 0.06499428, 0.03505203, 0.13207315, 0.053564183, 0.055606216, -0.061488762, -0.018870953, 0.086791724, 0.04838794, -0.0006079336, 0.08513244, 0.094327666, 0.055135187, -0.055457667, -0.09734772, 0.05343343, 0.0990739, -0.054733966, -0.031594507, 0.0396609, -0.120148234, -0.24593008, -0.0030991407, -0.096453466, 0.08821921, 0.26647478, 0.20670849, 0.08800138, 0.061203454, -0.05577455, 0.03612656, -0.05719024, -0.09340814, -0.0676361, -0.12458891, -0.0619473, -0.0028064705, -0.01475222, -0.017715063, 0.08664232, 0.12006194, -0.015437068, 0.061007705, 0.04819398, 0.16355877, -0.0643382, -0.054003693, -0.050799645, 0.031266876, -0.13702385, -0.02022409, -0.23215903, 0.13439079, 0.14593227, 0.007844996, 0.1035348, 0.06535183, 0.0005679161, -0.080610044, -0.25790682, -0.12138631, -0.15249522, -0.08694788, -0.099134594, -0.14627498, -0.057240818, -0.02291084, 0.12357341, 0.049009874, 0.07002113, 0.11317586, 0.1412867, 0.1739143, 0.14176567, -0.13411129, -0.19871314, 0.010789135, -0.0057750577, -0.032027856, -0.12863685, 0.005373118, 0.013299163, -0.021395147, 0.061962318, -0.038348977, 0.024611324, -0.13211545, -0.15501378, 0.015809307, -0.056833453, -0.07203316, -0.07323772, -0.14849037, -0.06348307, -0.058736607, -0.013032114, 0.112982124, 0.09882649, 0.24206275, 0.2193777, 0.16132113, -0.026273161, -0.13997288, -0.043957572, 0.0037168483, 0.0048252614, -0.017424187, -0.07706751, 0.14077695, -0.18692264, -0.08118895, -0.10589876, 0.05273781, -0.044903368, 0.07186088, 0.06411188, 0.071201965, 0.10758112, -0.031144058, -0.12102331, -0.025720494, -0.11294181, -0.09521406, -0.033938255, -0.00074026914, 0.12111559, 0.32745036, 0.31173518, 0.2746065, 0.11890895, -0.021806426, 0.04883454, 0.03166257, 0.053338848, -0.06721595, -0.049866367, 0.018193664, -0.21439172, -0.19371752, -0.09838442, 0.052638803, -0.037561834, -0.014069682, 0.11643967, 0.11325797, 0.10390669, 0.07935936, 0.027834404, -0.070998676, 0.024817998, -0.050449975, 0.12207146, 0.17458177, 0.26586106, 0.3752751, 0.34662923, 0.16633826, -0.057450276, -0.10028511, 0.017887764, -0.037965693, 0.030893832, -0.10541459, -0.15948914, -0.14279138, -0.12600309, -0.06250576, 0.053234436, 0.11625729, 0.09272131, 0.12935786, 0.09091293, 0.063515455, 0.07446874, 0.007484556, 0.06320044, 0.23327906, 0.10248941, 0.21348926, 0.29729718, 0.3474096, 0.22718498, 0.29182592, 0.36539608, 0.034535456, -0.26912677, -0.07594068, -0.015768781, -0.027567182, 0.024595827, 0.14361873, 0.07528944, -0.04800416, 0.093109325, 0.2136983, 0.06617432, 0.17053346, 0.06694453, 0.15874368, 0.100185946, 0.22949256, 0.1865239, 0.27956685, 0.2855643, 0.27535096, 0.33906454, 0.30855614, 0.3045906, 0.45651138, 0.3608351, 0.07797216, 0.20044172, 0.054419592, 0.011944555, -0.17327839, 0.013411321, -0.0141284615, -0.06704573, 0.063783415, 0.068615355, 0.17373142, 0.1876905, 0.2438285, 0.26548085, 0.27524003, 0.20444646, 0.2782789, 0.24094732, 0.26159927, 0.31389663, 0.37727684, 0.18939699, 0.22720878, 0.27136648, 0.24788067, 0.11894169, 0.29368314, 0.10852988, 0.07986728, 0.039462782, 0.13833813, -0.034302305, 0.067190744, 0.021663807, -0.086908594, -0.054469403, -0.022469364, 0.017928384, -0.040867925, -0.11866968, 0.000990185, 0.028196467, 0.07623117, -0.023758462, 0.071791105, 0.048059274, 0.094064884, -0.11941894, -0.06678111, -0.10455606, -0.07949118, -0.24075621, -0.0017225067, -0.10847291, 0.017549451, -0.10132176, -0.10217311, -0.1268979, -0.031605713, -0.005545199, 0.041851334, -0.03241828],
    [-0.05596867, -0.086147614, 0.03375358, -0.07512429, 0.043734096, 0.07407331, -0.03261227, -0.078329556, -0.053207636, 0.03545183, 0.008162767, -0.05666185, -0.07733937, -0.1456716, 0.11089884, 0.03561655, 0.023630738, 0.00649482, -0.054833055, -0.067856684, 0.06724375, 0.05027727, -0.0034825727, -0.00029601902, 0.03437902, -0.0026758537, 0.047711454, 0.042511605, 0.053120114, 0.059885018, 0.063447766, 0.046954043, -0.091100805, -0.03781544, -0.15257765, -0.10955006, -0.11091914, -0.14084668, -0.19872355, -0.16713093, -0.26239437, -0.20311779, -0.038276345, -0.2642859, -0.13351941, -0.19332801, -0.31506458, -0.29675126, -0.20428888, -0.18279687, -0.07177379, -0.19548061, -0.007708654, -0.0834829, -0.03678776, -0.03764057, -0.06734887, -0.062024437, -0.1031288, -0.12995595, -0.23120387, -0.15993486, -0.16756378, -0.34610346, -0.15234633, -0.16214246, -0.34977394, -0.24456006, -0.23232064, -0.09526742, -0.13441825, -0.1153788, -0.087484, -0.048622705, -0.036952913, -0.10359866, -0.22049035, -0.15205893, -0.15826182, -0.19915794, 0.0201938, 0.16946532, -0.07803505, 0.054931767, -0.0101824105, 0.08432595, 0.007376379, -0.1626422, -0.122610755, 0.121158056, -0.0020199553, -0.14635196, -0.080760054, -0.012418972, -0.05906297, 0.12041622, 0.117870785, -0.023941517, -0.03893931, 0.018835125, 0.050066262, -0.022284392, 0.046361938, -0.03375731, 0.05987412, -0.012260084, -0.072681375, -0.010872837, -0.122728065, 0.014476368, -0.04696697, 0.071346305, 0.035461225, -0.04918064, -0.041179314, 0.07987311, -0.010480061, 0.21765651, 0.056436125, 0.13297082, 0.08540499, 0.14479609, 0.15750113, 0.20838393, 0.22456305, 0.19574057, 0.2129191, 0.2256888, 0.28131068, 0.18008922, 0.12926412, 0.19058894, 0.10146546, 0.07111476, -0.05793378, 0.017012838, 0.22289208, 0.20793451, 0.04809671, -0.12046816, -0.026764896, 0.07027496, -0.023104105, 0.13259237, 0.014526048, 0.06330597, 0.171915, 0.102382496, 0.038707558, 0.13718797, 0.03611753, 0.029207872, 0.10002134, -0.004965371, 0.12592855, 0.1504728, 0.25997177, 0.26466662, 0.2328552, 0.15996498, 0.16723074, 0.14816658, 0.19999051, 0.100970864, 0.10266994, 0.29513702, 0.07265032, -0.074669935, 0.038945533, 0.08978555, -0.005884986, 0.009173811, 0.13876578, 0.0648609, 0.06795136, -0.016620321, 0.043647625, 0.003895913, 0.0013970939, -0.0048181484, -0.046541117, -0.0304643, -0.054002527, -0.061537806, -0.0019710227, -0.0574395, -0.04026008, 0.037100296, 0.14839906, -0.003966994, 0.19936907, 0.32119447, 0.24708234, 0.41866952, 0.2487688, -0.020825123, -0.038055208, 0.18361448, -0.11546866, -0.052321225, 0.1672396, 0.24006842, 0.11892453, -0.0076709245, 0.10216326, -0.009323278, 0.018184843, -0.040369052, -0.069691695, -0.07878355, -0.15577611, -0.2003312, -0.15257902, -0.08184783, -0.0030876356, -0.035510946, 0.0145741785, 0.029127866, 0.13686523, 0.25388786, 0.28095913, 0.38066897, 0.36730218, 0.06997572, -0.0071555325, -0.12456566, -0.087366074, 0.0889547, 0.16164756, 0.05310848, 0.013100194, 0.097965844, -0.08191928, 0.028755078, 0.06976545, -0.02744052, 0.041302733, -0.014607884, -0.074463926, -0.09203758, -0.095077045, 0.028403426, -0.03689563, 0.08341981, 0.09196553, 0.06541345, 0.029454123, 0.07582969, 0.22810411, 0.4272393, 0.40216064, 0.105836086, -0.073161624, -0.181215, -0.025852134, -0.041585527, 0.07579431, 0.02123111, -0.07863671, 0.018855805, -0.05271025, 0.09896768, -0.04450886, 0.025672037, -0.027500404, -0.10211124, -0.15226118, -0.17794608, -0.022795968, -0.051982313, 0.056759905, 0.024834396, 0.010979481, 0.16220504, 0.070416816, 0.14209905, 0.22136475, 0.39202031, 0.2483184, 0.025547836, -0.13340887, -0.17589176, -0.0955238, 0.06566774, 0.044986516, -0.040370144, -0.1280397, -0.09985412, -0.005352064, -0.07931745, 0.0015850503, -0.073021, 0.0008834041, -0.049898624, -0.12186739, -0.06278296, 0.026089272, -0.07039651, -0.027324963, -0.0021806187, 0.007938681, -0.07744354, -0.008139062, 0.031119341, 0.12600814, 0.39395434, 0.37764665, 0.022169542, 0.008955919, -0.08210471, -0.13897622, -0.07328458, -0.0021040114, -0.15605913, -0.14218576, -0.10258196, -0.07317953, -0.020549215, -0.053452138, 0.013658421, 0.01865484, 0.021365676, -0.09750303, 0.04236277, -0.032476123, -0.03704644, -0.10754032, -0.07566496, -0.16619946, -0.27425814, -0.21321547, -0.14144455, 0.19602178, 0.4181325, 0.39213547, -0.006669037, -0.00464912, -0.13925183, -0.118631974, -0.07390671, -0.24975489, -0.24254216, -0.2106425, -0.15568052, -0.14651188, -0.040995378, 0.074270085, 0.07049984, 0.2622386, 0.13720712, 0.07636469, 0.0762483, 0.08193825, -0.068268485, -0.10888733, -0.11129653, -0.16546148, -0.24993353, -0.26812977, -0.35342115, -0.09771661, 0.3323227, 0.28836843, -0.09474986, 0.06979739, -0.06902407, -0.20296521, -0.1109084, -0.16646197, -0.219953, -0.111095205, -0.1470576, -0.009536767, -0.0010591501, 0.08506797, 0.25182953, 0.40759462, 0.25976142, 0.14967933, 0.04029025, 0.048112437, 0.015460453, 0.057395805, -0.12186986, -0.085470684, -0.13543428, -0.1858325, -0.13629094, 0.0077394713, 0.22399203, 0.4050727, 0.123417266, 0.043799598, 0.058361806, -0.037444733, -0.05427124, -0.027569555, -0.12515569, -0.14992356, -0.095398486, -0.050825194, 0.038510695, 0.17787525, 0.18603353, 0.2599065, 0.29128402, 0.2130372, 0.11576625, 0.123940624, 0.031091494, -0.045625042, -0.03037423, -0.09577213, -0.1578992, -0.114376456, -0.20827572, 0.005596899, 0.060413055, 0.3904725, 0.22932674, 0.14312145, -0.016832776, 0.22394161, 0.02635677, 0.028314892, -0.1315704, 0.006640486, -0.029306091, -0.010351395, 0.12893705, 0.031631432, 0.10283996, 0.31220543, 0.27492848, 0.28015202, 0.13526614, 0.11868321, 0.022059608, -0.030306123, -0.024684565, -0.14646783, -0.13360873, -0.07834598, -0.06716501, -0.02742662, 0.08842738, 0.42957574, 0.2257279, -0.028107904, 0.04387323, 0.2226713, 0.13811351, -0.099812135, 0.032753978, 0.05167012, -0.0018789165, -0.05685891, -0.044062275, -0.053827446, 0.07395669, 0.18477842, 0.24888995, 0.20902622, 0.22865845, 0.054210875, 0.043506328, 0.06601853, 0.036347777, -0.0555125, 0.0364052, 0.034590866, 0.10456631, 0.21625909, 0.27080938, 0.3007814, 0.15109546, 0.07959955, 0.08944077, 0.11791944, 0.20679441, 0.12612581, 0.016282037, 0.054479, 0.054751396, -0.048553314, -0.045735814, -0.20309204, -0.06437898, 0.0745675, 0.14888358, 0.12749848, 0.1483512, 0.122324444, 0.049772494, 0.10811556, 0.034840144, 0.051398486, 0.15020515, 0.08762757, 0.16702464, 0.14369376, 0.023424542, 0.07456492, -0.0801864, -0.17021309, 0.086998455, 0.09348897, 0.25177255, 0.2480863, 0.28382087, 0.2019901, 0.12113028, 0.059584193, -0.0037821194, -0.19313517, -0.11063208, 0.02194557, 0.09537288, 0.06644706, 0.14701721, 0.15826924, 0.12725885, 0.01311783, 0.06293517, 0.104699135, 0.12320866, 0.13297477, 0.21525453, 0.27650473, 0.13881432, 0.068493515, 0.22300048, 0.0066341953, -0.10328346, 0.078272626, 0.28245595, 0.23174259, 0.27985895, 0.22905073, 0.0652502, 0.19544965, 0.07860406, -0.0866413, -0.03486879, -0.060390927, 0.08537514, 0.035445224, 0.123538375, 0.21208763, 0.20156549, 0.2061492, 0.15837814, 0.15929845, 0.1298426, 0.11856865, 0.22275275, 0.16816019, 0.22839855, 0.2840346, 0.15697205, -0.038085382, -0.09622031, 0.106339715, 0.39712897, 0.2936842, 0.23124346, 0.15987413, 0.1961103, 0.1401447, 0.11681074, 0.036430135, 0.007781196, -0.026688457, -0.121684805, -0.013854728, 0.043013923, 0.048330504, 0.22406358, 0.16273709, 0.19918105, 0.27817208, 0.21516202, 0.20842403, 0.24073924, 0.008043052, 0.11330465, 0.08430935, -0.083666, -0.06910676, -0.02869153, 0.26847935, 0.23552276, 0.23446882, 0.07043028, 0.09021211, 0.1090604, 0.07966118, 0.05108061, 0.010786743, 0.07149072, -0.055948988, 0.03083518, 0.0062988736, 0.072483875, 0.033967976, 0.15946737, 0.27152365, 0.24153884, 0.18623058, 0.15369447, 0.22037575, 0.24191721, -0.07410599, 0.069926456, 0.08474663, -0.06938825, -0.05381124, 0.019269979, 0.23969136, 0.24176358, 0.21564198, 0.0064297873, 0.07631075, 0.054202694, 0.054056074, 0.028955039, 0.07873668, 0.088227145, 0.061953504, 0.03053099, 0.006909168, 0.008830289, 0.087340266, 0.07096732, 0.26635858, 0.23419458, 0.2894743, 0.20813443, 0.22835054, 0.12207607, -0.09621082, -0.055143487, 0.06560725, -0.05313941, 0.020544812, -0.013028108, 0.2673728, 0.40571785, 0.1477544, 0.034953035, -0.040859863, -0.004331572, 0.0021216434, 0.117804594, 0.1087274, 0.077736445, 0.036643215, 0.08269614, 0.0524776, 0.12784405, 0.13351762, 0.17124058, 0.15769245, 0.1870253, 0.18100464, 0.17532347, 0.09355559, -0.011581453, -0.03578302, 0.06464494, -0.0070517934, 0.064500265, 0.08222065, -0.02608373, 0.09393003, 0.30020168, 0.036546897, -0.13570842, -0.120510645, -0.0044284486, 0.025517173, 0.09299551, 0.09538559, 0.15340307, 0.1918749, 0.15850928, 0.20982014, 0.082293995, 0.07096079, 0.05557602, 0.112637475, 0.10016622, 0.09376704, -0.035048112, -0.0825979, 0.003669348, 0.12265059, 0.09297586, 0.09800199, -0.06154854, -0.06998942, -0.07542164, 0.058934346, -0.084467694, -0.12070461, -0.30211857, -0.08843226, -0.16378154, -0.13928397, -0.06859676, 0.0393802, -0.014065971, -0.081802726, 0.060002122, 0.031050928, -0.024538381, -0.06998651, -0.07280076, -0.14104833, -0.1158188, -0.15416524, -0.19537745, 0.03762877, -0.0386015, 0.068883665, 0.10456634, 0.13057117, -0.05382082, 0.024107657, -0.0555553, -0.0035982803, -0.02322606, -0.10325572, -0.21767713, -0.30382252, -0.346913, -0.2998303, -0.35500348, -0.3994006, -0.27591366, -0.37719423, -0.4295506, -0.6782517, -0.42386946, -0.31795478, -0.3042229, -0.3216681, -0.29395878, -0.19792095, -0.08652113, -0.03911981, -0.015065305, -0.14193812, 0.04889355, 0.007767655, -0.038782984, -0.036825582, -0.04451046, -0.052436087, 0.003442444, -0.028477259, -0.012312226, -0.1267494, -0.0072935326, -0.20395029, -0.1628852, -0.11133744, -0.104471244, -0.12547407, -0.18202423, -0.30576137, -0.19636303, -0.1892542, -0.14192048, -0.15789062, -0.2437583, -0.12836608, -0.17475545, -0.08532904, -0.006183115, 0.06553886, 0.07430143, 0.07018884, -0.06133492],
    [0.01774463, 0.04770901, 0.011473127, 0.02110067, -0.08140029, -0.041100655, 0.023484416, 0.08523818, 0.07495438, -0.020602927, -0.027948987, 0.03225542, -0.0029517075, -0.14360681, -0.03468843, 0.0942445, -0.06737581, -0.0053938776, -0.046417624, 0.06142933, -0.08032186, 0.058714785, -0.07973608, 0.042560227, 0.06961473, -0.017239429, -0.028534215, 0.05560466, 0.006347172, 0.06415705, 0.034443244, 0.042511754, -0.02436912, -0.024129467, -0.14355092, -0.16551413, -0.19007324, -0.15825158, -0.15908729, -0.26529232, -0.26472571, -0.26741868, 0.07388418, 0.071859084, -0.117353894, -0.08440306, -0.2894945, -0.14120576, -0.19858828, -0.22500612, -0.12823927, -0.16361956, 0.05587227, -0.0069081485, 0.028561138, -0.036770575, 0.057907127, 0.02409029, 0.03901812, -0.18730648, -0.21188395, -0.21095534, -0.27375215, -0.31062305, -0.27507216, -0.34601903, -0.43555018, -0.43287492, -0.31304857, -0.17527124, -0.10743482, -0.12406924, -0.016235285, -0.06540224, -0.16261894, -0.32049802, -0.25952357, -0.27451524, -0.17262536, -0.16095035, 0.073167756, 0.073532715, -0.035709742, -0.042539276, -0.023880176, -0.019500643, 0.13193098, -0.12636784, -0.18032037, -0.044719, -0.26212105, -0.28257447, -0.40370485, -0.29589626, -0.31351492, -0.2702375, -0.2550482, -0.18596669, -0.1678818, -0.13418028, -0.1310312, -0.12675744, -0.24032034, -0.23801477, -0.14857416, -0.17846891, -0.24803653, -0.13254477, -0.17985731, -0.020080568, -0.13235, 0.04663851, 0.060351558, 0.021305466, 0.16851577, 0.04089449, 0.010477829, 0.12662314, -0.108455576, -0.06473307, -0.107048504, -0.044399038, -0.15679772, 0.053526096, 0.16796204, 0.13531053, 0.19849779, 0.22379819, 0.022891585, -0.020275382, -0.0061573, 0.0349344, 0.054107636, -0.013770403, -0.04540979, -0.26771277, -0.122194596, 0.026242264, -0.010635031, -0.13562974, -0.029797412, 0.049015872, 0.1701385, 0.22993767, 0.11058439, 0.18695736, 0.17480205, 0.038973898, -0.026510164, 0.023761267, -0.060428426, 0.11304282, 0.1293733, 0.08225061, 0.19180448, 0.09495181, 0.029410465, 0.02420345, -0.12037609, -0.062695585, -0.07178477, -0.058878053, -0.06729893, -0.13045298, -0.08167143, -0.17123087, -0.04765551, -0.110778, -0.08597694, 0.10362554, 0.12507528, 0.37095556, 0.20605345, 0.16356006, 0.23457195, 0.1477975, 0.165499, 0.091248125, 0.12295675, 0.15323174, 0.11668386, 0.11053022, 0.030098965, 0.10254639, 0.069350116, 0.07506292, -0.023879673, 0.015492739, -0.006261828, 0.0500262, -0.11406809, -0.048920564, -0.17965093, -0.097265884, -0.028059954, -0.11024544, 0.029182335, 0.019666094, 0.119543485, 0.48005748, 0.32259518, 0.28411213, 0.32713044, 0.20971128, 0.25006366, 0.2760904, 0.14579383, 0.067637965, 0.11544515, 0.071659215, -0.020515852, 0.052320164, 0.06149036, 0.038869493, 0.18463229, 0.109815344, 0.09735505, 0.0053899395, -0.02629946, -0.1335142, -0.17841467, -0.16741048, -0.04634566, -0.079138316, -0.08027591, 0.20004977, 0.13424422, 0.5309615, 0.5319123, 0.33036903, 0.33183822, 0.40082228, 0.1783526, 0.21525556, 0.16013142, 0.1578111, 0.13096334, 0.24474731, 0.13360035, 0.14142077, 0.23588464, 0.2612834, 0.3626338, 0.35898343, 0.15706156, 0.14424652, 0.16125655, -0.04096886, -0.2745937, -0.15211387, 0.036524758, 0.16326399, 0.12707892, 0.23900001, 0.11793674, 0.39362428, 0.44106734, 0.36250183, 0.24040566, 0.18988404, 0.1899665, 0.14921375, 0.15379053, 0.11317169, 0.062794246, 0.20468418, 0.16904438, 0.22255073, 0.33692244, 0.4421122, 0.39303496, 0.39464113, 0.26158231, 0.2754149, 0.17767264, -0.14199017, -0.44598353, -0.23293893, 0.114463635, 0.07050089, 0.1309287, 0.2551953, 0.3106634, 0.3421213, 0.3607543, 0.14885853, 0.23619625, 0.12007468, 0.113778956, 0.104204476, 0.09359482, 0.06444232, 0.012047695, 0.09727361, 0.2624102, 0.35863993, 0.38906702, 0.3155794, 0.380831, 0.38598007, 0.31585583, 0.3451431, 0.18078183, -0.18991427, -0.52459705, -0.2987774, -0.09501453, 0.009050812, 0.06277155, 0.17221248, 0.35306093, 0.5283264, 0.26704276, 0.2234798, 0.07224226, 0.19876193, 0.115938425, 0.12108805, 0.0652375, 0.030687854, 0.06306025, 0.024255479, 0.18315051, 0.31069055, 0.20926686, 0.38098767, 0.41368276, 0.3181721, 0.31491712, 0.26608777, 0.23147337, -0.07808007, -0.30328125, -0.09973581, 0.14866933, 0.11595576, 0.10257192, 0.18942195, 0.32712084, 0.45423716, 0.2897895, 0.06782758, -0.0062958533, 0.044237304, 0.08924527, 0.16928579, -0.020738624, -0.12339392, -0.27657616, -0.10551665, 0.16366515, 0.33191094, 0.27662164, 0.23599623, 0.2856135, 0.22873059, 0.28427693, 0.3045961, 0.20261973, -0.04302753, -0.17961611, 0.116560735, 0.15099694, 0.22003067, 0.0758835, 0.11007933, 0.3707209, 0.33550122, 0.16075471, -0.034255102, -0.10351716, 0.0030982408, 0.0067200335, -0.12648296, -0.2668781, -0.2843664, -0.40713674, -0.13287166, 0.15065144, 0.13446724, 0.3101955, 0.2842671, 0.22028752, 0.15051745, 0.28299344, 0.085535385, -0.10068829, -0.20127696, -0.2949648, 0.079001956, 0.23612861, 0.25405708, -0.14571287, 0.10850378, 0.32319677, 0.168192, 0.23067299, -0.04084538, -0.20952031, -0.20233718, -0.20806399, -0.3254932, -0.37085244, -0.29017884, -0.3705059, 0.011355495, 0.17452192, 0.25515103, 0.1492072, 0.2852654, 0.29079697, 0.3139937, 0.10301532, 0.048911568, -0.06700345, -0.2862251, -0.3172442, 0.097807325, 0.2365262, 0.16366296, -0.15736437, 0.016839743, 0.18954782, 0.20595057, 0.096256495, -0.1324604, -0.09121342, -0.20134528, -0.18286724, -0.20775554, -0.30157232, -0.2941158, -0.070238665, 0.12305257, 0.17589378, 0.2692598, 0.13693051, 0.21974863, 0.26621386, 0.069257244, -0.02140348, -0.087007105, -0.07191855, -0.20004481, -0.21199895, -0.05570235, 0.09753588, 0.23178875, 0.13755073, 0.10511888, 0.25078914, 0.02269798, 0.09761486, -0.13436002, -0.1709378, -0.110404484, 0.009314673, -0.07620188, -0.23074257, -0.24456063, 0.18262458, 0.2513123, 0.23255835, 0.17538951, 0.2386268, 0.2096754, 0.017533345, -0.029142506, -0.06914367, -0.019569311, -0.051924556, -0.06972348, -0.037425667, -0.088873304, 0.18198709, 0.12696098, -0.06590351, 0.11597484, 0.14334138, 0.020134298, 0.04169848, -0.18023397, -0.31423783, -0.03784435, -0.037164662, 0.00012571341, -0.036983978, -0.0057046954, 0.30140057, 0.29118225, 0.22244258, 0.20002387, 0.11799169, -0.095063515, -0.11284889, -0.13558318, -0.048110764, 0.014951582, 0.030973831, 0.0048638657, -0.069545455, -0.18978237, 0.25241494, -0.07400302, 0.09485221, 0.13060862, 0.15580572, 0.17819828, -0.02240124, -0.17438334, -0.23242748, -0.31506532, -0.024772579, 0.008807479, -0.04560638, -0.01621664, 0.1410848, 0.25484362, 0.13955817, 0.052975737, -0.057736896, -0.13102959, -0.026580732, 0.035203073, -0.020338958, -0.09588279, -0.06332084, -0.13010095, -0.18278515, -0.11237892, 0.07471343, -0.012568148, 0.117698886, 0.019898454, 0.02327209, 0.05394739, -0.14366286, -0.12964046, -0.19635776, -0.09758728, -0.14593174, -0.15961854, -0.1737573, -0.095042385, -0.047492906, 0.11185422, 0.003958012, 0.017568996, 0.004564625, -0.055774312, -0.075598575, -0.036340103, -0.05916677, -0.031170938, -0.15622391, -0.095378704, -0.07978082, 0.017933734, 0.17293426, 0.10420162, -0.07347284, -0.09624719, -0.1492825, -0.05878537, -0.22554226, -0.07642619, 0.036100697, 0.085534036, -0.000839319, -0.01138907, -0.06700668, -0.187218, -0.13799047, 0.04099337, 0.04671658, 0.0009020084, 0.010419096, -0.00084246224, -0.03150881, -0.06726804, -0.056302812, 0.00039665186, -0.08563151, -0.05183803, 0.06498254, -0.01643882, 0.19658034, -0.0020119164, 0.02599539, 0.053972565, -0.021812936, -0.05202178, -0.15274273, 0.0009525688, 0.07384242, 0.058945492, 0.0008267171, 0.059904534, 0.024451988, 0.0027395752, -0.0961399, 0.0049241097, 0.035615277, 0.03065834, 0.03263585, -0.065042205, -0.03553038, -0.104756884, -0.16393812, -0.16320911, -0.21267653, -0.28031728, -0.08026522, -0.09390505, -0.1630155, -0.009893145, -0.04944548, -0.0056802277, -0.015222444, -0.1093668, -0.06993734, 0.14371891, 0.25221366, 0.13353513, 0.016835708, -0.005389642, 0.06665725, 0.03879307, -0.016658075, 0.060401585, 0.09417082, 0.06701312, -0.010968184, -0.013779241, -0.10698644, -0.105370335, -0.10863957, -0.1787905, -0.18344763, -0.30381253, -0.15223272, 0.027003512, -0.09860883, 0.05000526, 0.032005638, -0.008873589, -0.15441054, -0.14481963, 0.0120975245, 0.056448728, 0.07919057, 0.16694933, 0.02037849, 0.10900054, 0.055385366, 0.11154544, 0.11060105, 0.20294492, 0.14870518, 0.215672, 0.08716391, 0.15069953, 0.06749486, -0.12885714, -0.09609603, -0.1107399, -0.28406534, -0.25150782, -0.22152276, -0.15860552, -0.03403729, -0.025573488, 0.001729168, -0.08019353, 0.010257246, -0.018288359, 0.033093695, 0.13962603, 0.16000703, 0.069872916, 0.16665833, 0.01948329, 0.14247958, 0.22304597, 0.2656003, 0.1523613, 0.1783601, 0.13514039, 0.16011934, -0.037803903, -0.09277599, 0.043274883, -0.1830702, -0.18663414, -0.29291466, -0.2546526, -0.31042185, -0.0836458, -0.01963292, -0.052886225, 0.032639377, -0.070984185, -0.11407449, -0.08070724, -0.0032019692, -0.076680526, 0.08763491, 0.12499471, 0.18647794, 0.26021054, 0.16317156, 0.17111604, 0.16771953, 0.14513527, 0.27410987, 0.1457353, 0.05999419, -0.026681153, 0.05889267, -0.028264944, -0.09910751, -0.1497192, -0.1806835, -0.31583372, -0.2464698, -0.15680888, 0.007174374, 0.074962266, -0.06678032, -0.063479185, 0.019625537, -0.070102066, -0.15633146, -0.13370617, 0.070946716, 0.18739796, 0.23914734, 0.18278365, 0.09972083, 0.05510608, -0.02365762, 0.123842515, 0.0356399, 0.08631236, 0.026587978, 0.18724026, 0.07745708, 0.12334739, -0.08800476, 0.069880046, 0.11498722, 0.06280222, 0.10886315, -0.031103723, 0.0041899756, -0.013846703, 0.08614171, 0.0038062856, -0.01734434, 0.085549496, -0.03292514, 0.0952624, 0.12537977, 0.091302454, 0.11898734, 0.16302128, 0.13740398, 0.07188477, 0.09312229, 0.3540248, 0.24601503, 0.24934836, 0.27975348, 0.34510964, 0.153098, 0.2989256, -0.021335902, 0.057679743, 0.18476315, 0.18583749, 0.053398065, -0.004103653, 0.03473776, -0.0818685],
    [-0.072958745, 0.07244388, -0.058840707, 0.08409826, 0.07596826, 0.017708465, -0.0054156855, 0.07836992, -0.00077807903, 0.009963669, 0.036531173, -0.0490645, -9.3815e-05, -0.032771554, -0.090524994, -0.056399688, -0.028589714, 0.03695269, -0.034401298, -0.041103184, -0.017316528, 0.07982496, 0.057923444, 0.056049876, 0.07175296, 0.027560882, -0.08578917, 0.009212434, 0.08025902, -0.01356639, 0.073870696, 0.032662757, -0.008905607, 0.18059583, 0.1261477, 0.2647918, 0.16543816, 0.18998311, 0.15171392, 0.23339169, 0.31698608, 0.28260472, 0.035312675, 0.13514863, 0.113055326, 0.17930575, 0.15167864, 0.19650766, 0.25411734, 0.21192715, 0.18123008, 0.071532264, -0.035462484, 0.04104335, 0.026854701, -0.007882059, -0.042266715, 0.038906254, 0.01717532, 0.11412827, 0.22920446, 0.098395765, 0.14301959, 0.20649377, 0.28437406, 0.24556184, 0.33952954, 0.3569796, 0.38035178, 0.17194262, 0.039784487, 0.024024438, 0.085642844, 0.14772668, 0.20963916, 0.2835536, 0.15833886, 0.23204781, 0.34161913, 0.16418222, -0.06259319, -0.12320025, -0.030956488, -0.0022397041, 0.05135166, -0.043841545, -0.10587309, 0.13563995, 0.202553, 9.252255e-05, 0.07290469, 0.14362395, 0.26362473, 0.20459394, 0.36407748, 0.22754447, 0.26130822, 0.28526524, 0.118931964, 0.2073946, 0.08265517, 0.14504053, 0.21087973, 0.17116776, 0.20490272, 0.25294465, 0.279053, 0.24333102, 0.20099624, 0.018823894, 0.08226456, 0.03548207, 0.04570871, -0.09122879, -0.0622632, -0.095072255, -0.08570737, -0.037817903, 0.007323446, 0.04474777, 0.12292174, 0.14669217, 0.2346807, 0.08320759, 0.046538487, 0.12356054, 0.15998212, 0.1869898, 0.20700924, 0.25186777, 0.14368607, 0.07902811, 0.073847994, 0.11057558, 0.0793288, 0.069442295, 0.0113286525, 0.15263256, 0.08229482, 0.19499415, 0.06390118, 0.033749476, -0.111115284, -0.24924748, -0.057108145, -0.31849518, -0.17396288, -0.029679967, 0.15674585, 0.13313462, 0.10686719, 0.0774428, 0.10329319, 0.010023697, 0.19206968, 0.021416847, 0.18236083, 0.35295165, 0.21773359, 0.14760053, 0.21689314, 0.1053812, 0.043011013, -0.051469766, -0.052874178, 0.17399065, 0.20067799, 0.119273156, 0.07825621, -0.037174646, 0.034896806, -0.1673899, -0.102117285, -0.0688604, -0.09842055, 0.10296779, -0.024607994, 0.002272251, -0.01127206, 0.037472166, 0.06709857, -0.055394698, 0.07160601, 0.033430785, 0.032138653, 0.027193133, -0.010397913, 0.031251255, -0.020358473, 0.010764654, 0.0146399, 0.112869725, 0.113477945, 0.26723614, 0.009045669, 0.0057980716, -0.021631576, 0.06066341, 0.036919367, -0.09905183, -0.14613706, -0.14011025, 0.035718188, 0.07927397, 0.17650333, 0.09361305, 0.025773596, 0.0696394, -0.04746081, -0.07275099, -0.07385066, -0.02610451, -0.10518884, -0.08269937, -0.12725125, -0.060818754, -0.14071086, -0.009845035, 0.03849833, 0.06370803, 0.25382292, 0.28036338, 0.09675524, 0.12515058, 0.13257755, 0.016940316, 0.057289913, -0.108881935, -0.23032553, -0.04102665, -0.008521374, 0.08498028, 0.09747519, 0.17634748, 0.077244386, 0.050806057, -0.009593613, 0.015054021, -0.079571106, -0.08854257, -0.117166385, -0.062238507, -0.17317, -0.095206186, -0.086846575, -0.06320132, 0.0032108512, 0.086410895, 0.2093009, 0.21534961, 0.14038053, 0.105334125, -0.037817173, -0.20142415, -0.050303873, -0.03104996, -0.13429615, -0.05273125, 0.08490709, 0.03325041, 0.07715445, 0.026941435, 0.13921688, 0.117422774, 0.03311495, -0.02169814, -0.06466951, -0.3331657, -0.17244759, -0.22632244, -0.0775278, 0.029951368, 0.052313868, 0.06533828, 0.019021466, 0.19428563, 0.32931083, 0.101541534, -0.05783627, -0.058367785, 0.031363413, -0.024075475, -0.10102546, -0.09172086, 0.039455477, -0.015009911, 0.12480636, 0.13975312, 0.18600774, 0.17045105, 0.07722923, 0.004382342, 0.012190034, -0.02301468, -0.13651356, -0.34080225, -0.1927628, -0.1775344, -0.09927707, -0.10933642, -0.037009742, 0.088454634, -0.045971133, 0.10403845, 0.27967522, -0.021827932, -0.09372879, -0.15580533, 0.0029210127, 0.06390397, -0.052628785, -0.11293649, -0.12743703, -0.008133655, 0.11684173, 0.055363715, -0.008428274, 0.072857045, -0.017769052, -0.05757803, 0.021988055, 0.01682909, -0.28968608, -0.43312174, -0.21996412, -0.15656467, -0.07695772, 0.06696351, -0.063001804, 0.039586958, -0.039037675, 0.25912988, 0.42397237, 0.00974642, -0.0630442, -0.014494262, -0.03332035, -0.018744789, -0.06783938, 0.026580613, -0.19591126, 0.09147928, -0.01821219, 0.00908196, -0.11241767, -0.0071029654, -0.023509631, -0.0108984625, 0.13714781, 0.058801007, -0.30111033, -0.35673785, -0.13687015, -0.07726602, -0.028861606, 0.00013505205, 0.05221447, 0.11270314, 0.15513195, 0.19708444, 0.5355911, 0.012126593, -0.27785063, -0.2009049, -0.059317686, 0.06076879, 0.03691643, 0.015891349, -0.110434204, -0.07181746, 0.025506182, 0.031495012, 0.03954625, 0.010634041, 0.0134043535, 0.18454438, 0.17004724, 0.050614566, -0.29047757, -0.12994882, -0.09327501, -0.09602477, -0.012045499, -0.024207242, 0.04135958, -0.020375727, 0.20000955, 0.32977772, 0.37653118, -0.031925928, -0.16146792, -0.19864397, 0.18725865, -0.02570542, -0.07166301, -0.0023366495, -0.26921776, -0.108888045, -0.10540959, -0.054103114, -0.05338115, 0.055673923, 0.16188927, 0.3099843, 0.17063138, -0.086504206, -0.058695257, -0.029821804, -0.09653214, -0.09817989, -0.03788718, -0.016565358, 0.013289185, -0.045901105, 0.04271349, 0.20779578, 0.29669037, 0.10660493, -0.23436823, -0.17808193, 0.14464785, -0.042027514, -0.052242555, -0.07995963, -0.230928, -0.064221226, -0.118264504, 0.08043484, 0.10528051, 0.17365035, 0.16202615, 0.23836677, 0.10281193, 0.043831605, -0.004161213, -0.041785937, 0.013690455, -0.1293744, -0.038632035, -0.08821861, -0.020485185, 0.052302867, -0.026794918, 0.08031527, 0.22514623, 0.20016901, -0.21983422, -0.21582547, 0.0027854799, -0.026948255, -0.15058136, -0.049547803, -0.13182761, -0.044602625, 0.03387378, -0.028555406, 0.04867675, 0.18124989, 0.19346406, 0.26173657, 0.069228984, -0.022938903, -0.08211252, -0.026103014, -0.10028185, -0.025741601, 0.07466698, -0.00983171, -0.0026895585, -0.013166137, -0.016958963, 0.1625827, 0.15301716, 0.19746879, -0.090643086, -0.14834654, -0.04703686, -0.12155721, -0.072020896, -0.011658863, -0.13866705, 0.079589024, 0.09501919, 0.12719622, 0.11704016, 0.2410504, 0.36217672, 0.34390214, 0.20750538, 0.022971882, -0.0013407163, -0.01679032, 0.030386997, 0.0067580747, 0.050532285, 0.065959126, 0.14274797, 0.121607766, 0.1774585, 0.2502629, 0.23717335, 0.053464502, -0.14801668, -0.0012885544, 0.035490613, -0.14272892, -0.09607535, -0.15035646, -0.025175843, 0.051507544, 0.1761844, 0.19249783, 0.22605518, 0.21937554, 0.31705782, 0.41616306, 0.269266, 0.070422515, -0.040230304, -0.025330419, 0.060129758, 0.090793654, 0.17242236, 0.16583343, 0.058970027, 0.111862905, 0.19415589, 0.24367821, 0.23336719, 0.037483595, -0.1832518, -0.17579828, -0.054740638, -0.15990223, 0.039718285, -0.14073968, -0.013381293, 0.14138041, 0.15268517, 0.14904979, 0.24029663, 0.29569656, 0.32735875, 0.44664666, 0.41482797, 0.2592772, 0.07250869, 0.20396698, 0.14681756, 0.20815043, 0.20600273, 0.09958541, 0.16546263, 0.14080417, 0.29841954, 0.12576483, 0.11272891, -0.18954909, -0.16860108, -0.15550344, 0.017783284, 0.015618704, 0.21081935, -0.14735106, -0.031371694, 0.09397222, 0.0683564, 0.17501666, 0.19006044, 0.24805713, 0.2934317, 0.32705036, 0.33199438, 0.370389, 0.21718329, 0.21689312, 0.2001206, 0.042556364, 0.16534479, 0.11647681, 0.11837731, 0.17021173, 0.25019196, 0.13379794, -0.032675814, -0.17842393, -0.18823421, -0.018704517, -0.038614262, -0.059044596, 0.053065225, -0.05187119, -0.02040988, -0.10716847, -0.045638494, 0.102893084, 0.13556883, 0.22410123, 0.19338073, 0.3893442, 0.30785075, 0.43915167, 0.333072, 0.29633185, 0.14832419, 0.13245629, 0.06656323, 0.05997226, 0.025876468, 0.058445524, 0.23017795, 0.1225657, -0.142171, -0.10181011, 0.19479284, 0.09811868, 0.0067441193, -0.039381977, 0.025380949, 0.10136112, -0.055578314, -0.20750774, -0.10800855, -0.06445653, 0.02911938, 0.033366863, 0.17128138, 0.20415911, 0.34956053, 0.40367144, 0.17710085, 0.17325924, 0.0878478, 0.05907886, 0.052506723, -0.012146102, -0.012830898, 0.13367592, 0.15992276, 0.0098623, -0.030983267, -0.025645295, 0.046514265, 0.10861068, 0.039675258, -0.07513288, 0.055970874, 0.013251583, -0.20712142, -0.16441996, -0.1794266, -0.17915063, -0.014724744, 0.03186032, 0.037074525, -0.011808662, 0.08679241, 0.15659374, 0.14113055, 0.15215029, -0.0348316, -0.08577769, -0.07356471, 0.022683568, -0.034786478, 0.06589205, 0.10140509, 0.1150341, -0.094139405, 0.07754205, -0.14578192, 0.021105625, 0.038899027, 0.06819422, -0.15065853, -0.21198171, -0.33522254, -0.3714025, -0.3716036, -0.1732065, -0.1340907, -0.10290362, -0.13506044, -0.14851344, 0.056485962, 0.04625837, -0.0014512669, 0.06518799, 0.0032343944, -0.06391608, 0.049436238, -0.08220387, -0.061756007, -0.27451262, -0.040987227, -0.06572079, 0.20925, 0.06358348, 0.027476748, 0.026737764, -0.053147946, 0.07047672, 0.05309561, -0.09719214, -0.16110887, -0.31219956, -0.3854937, -0.26820588, -0.41556975, -0.26969078, -0.20746046, -0.258652, -0.31415823, -0.19529882, -0.27020726, -0.055034973, 0.023298936, -0.076855585, -0.08300207, -0.04671243, -0.12480236, -0.17862038, -0.13439964, -0.07716843, 0.024806459, -0.037741352, 0.01949576, -0.040438436, 0.040861778, -0.08395382, 0.061503254, 0.026725143, 0.013939006, -0.13377146, -0.13839705, -0.27898136, -0.24299532, -0.1800119, -0.10319614, -0.068324454, 0.09383178, -0.010600936, -0.24415246, -0.25630286, -0.15279022, -0.047008585, -0.030965425, -0.12995514, -0.06542477, -0.09701353, -0.18035665, -0.056668885, -0.10078259, -0.020969316, 0.08048109, -0.073608756, -0.07949402, -0.026588377, 0.03604676, 0.08641059, -0.047187842, -0.0032469246, -0.0885241, 0.026008993, -0.119590215, -0.088951975, -0.040713023, 0.07872902, 0.04059491, -0.03291365, -0.03460162, -0.12405727, -0.12236049, -0.07343052, -0.063101634, -0.061368752, 0.009416026, -0.14820227, -0.14520626, -0.016762357, -0.07237035, -0.08611344, -0.038096804, 0.020493418],
    [0.08156311, 0.0844348, 0.06678536, -0.008327514, -0.053998914, 0.08691364, 0.047899596, -0.068815775, -0.013713807, 0.026817039, 0.04945465, -0.06374097, 0.14557083, 0.093295775, -0.023431597, -0.006449864, -0.025827937, -0.07547099, -0.07899509, 0.027864322, 0.0024151653, -0.007011093, -0.049685415, 0.060007803, 0.05986216, -0.018863127, -0.0039265156, -0.06161575, 0.031513177, -0.019375436, 0.0064150468, 0.06395406, -0.07863828, 0.05986909, 0.13272652, 0.19505173, 0.11785248, 0.025207175, 0.14703159, 0.044579558, 0.12612696, 0.21542059, 0.08404813, 0.18997863, 0.23043802, 0.14604919, 0.04730799, 0.15894152, 0.0648326, -0.04636459, 0.11470649, 0.15402646, 0.030226268, -0.05299093, -0.05237047, -0.012779668, -0.046808466, 0.05570882, 0.11510917, 0.086913176, 0.15073495, -0.08417753, -0.07059352, 0.070127256, 0.20562339, 0.15736206, 0.312949, 0.22188383, 0.38587794, 0.2293114, 0.1948013, 0.16133703, 0.1616467, -0.0010974808, -0.0051863366, -0.16297583, 0.010653201, 0.055884264, 0.015805773, 0.13962318, 0.047581807, 0.012370973, -0.05723881, 0.0046776906, 0.051346235, 0.009687893, -0.008418208, 0.13158973, -0.21961021, 0.031491973, 0.082843296, 0.12032828, 0.2658361, 0.40503743, 0.27906775, 0.5354937, 0.45167306, 0.48269868, 0.34168902, 0.29808158, 0.30899376, 0.2687083, 0.12221218, -0.038396813, 0.058498807, -0.09757512, -0.22765027, -0.19795537, -0.05879394, -0.044671368, 0.039851293, 0.049580343, -0.08285342, -0.0012296076, -0.02499408, 0.12776546, -0.05846449, 0.112189524, 0.29365385, 0.2168549, 0.23874104, 0.2500521, 0.3013958, 0.19110341, 0.28061417, 0.1483943, 0.2095339, 0.14791673, 0.18003598, 0.060173538, 0.06413108, 0.08979832, -0.056858838, -0.2166528, -0.22789846, -0.35635352, -0.26969144, -0.10900354, -0.041174646, 0.09283078, 0.027206786, -0.0664009, 0.13850868, 0.08536869, 0.07053896, 0.22047068, 0.1010943, 0.06927068, 0.036801465, 0.024874682, 0.07313465, 0.20265281, 0.0831666, 0.19440177, 0.14220089, 0.13647868, 0.1851601, 0.09224375, 0.109884515, 0.10885386, -0.06012673, -0.06737444, -0.04401035, -0.30231228, -0.19592464, -0.17625916, -0.21622063, -0.06915173, 0.056249313, 0.11941577, 0.051904697, 0.26511127, 0.0559947, 0.060603753, 0.05722229, 0.08354517, 0.06504229, 0.04799908, 0.08144251, 0.21246673, 0.1584333, 0.1921002, 0.23366071, 0.2179978, 0.2847062, 0.22642945, 0.1875112, 0.08707978, 0.032890074, 0.078922026, -0.11064411, -0.17526998, -0.33197546, -0.31619158, -0.23702917, 0.025857804, -0.07526976, 0.2182296, 0.0408568, 0.15009728, 0.25374103, -0.021804802, -0.021861413, -0.06041616, -0.03697386, 0.109002, 0.07826631, 0.12763421, 0.28817979, 0.34071177, 0.31776184, 0.27847132, 0.21730734, 0.20675737, 0.073398665, 0.08091458, 0.052574467, 0.005196328, -0.0072210846, -0.14950825, -0.34446648, -0.36190358, -0.17539918, 0.015294682, 0.022291198, 0.18365379, -0.04689821, 0.058147695, 0.095838726, -0.04022784, -0.034638185, -0.041105174, 0.016866902, 0.017105488, 0.1510876, 0.12403602, 0.14529906, 0.24861605, 0.3917822, 0.30034697, 0.17110734, 0.19262291, 0.1131204, 0.11794893, 0.018437956, -0.018219307, 0.0658889, -0.22496194, -0.45352846, -0.4079064, -0.45758465, -0.26870233, 0.027584527, 0.15863115, 0.003910919, 0.1343386, 0.16126388, 0.05991122, 0.09092895, 0.0048277937, -0.041077428, 0.0758488, -0.034745038, -0.03339381, 0.0015185047, 0.09914745, 0.38999018, 0.42935997, 0.2339315, 0.10704897, 0.14294302, 0.0046811923, 0.15368405, -0.02720214, -0.04720487, -0.015320876, -0.2944269, -0.459669, -0.3168325, 0.017499818, 0.029596176, 0.14000148, 0.07726636, 0.25528455, -0.022462074, -0.058535237, -0.006481107, -0.101191595, -0.04757302, -0.22200878, -0.16923623, -0.25002176, -0.18209274, -0.056169774, 0.2311359, 0.08308851, 0.124126256, 0.04099414, 0.120881, 0.03346376, 0.12891424, 0.03261407, 0.08234984, 0.015866017, -0.23931572, -0.14508562, -0.2646005, 0.04272539, 0.050057903, 0.16318433, 0.03514734, 0.20695813, -0.16060564, -0.18350384, -0.16432865, -0.29423088, -0.24450049, -0.31533313, -0.23365405, -0.20063287, -0.25918463, -0.034069218, -0.016634293, 0.035871934, -0.060108434, -0.030801032, 0.057863783, 0.102858864, 0.16690366, 0.11938853, 0.1050947, 0.102468766, -0.012680719, -0.20339325, -0.36526334, -0.036193576, -0.029322885, -0.08271875, 0.031750202, -0.05229029, -0.22125319, -0.24345194, -0.36781684, -0.24149434, -0.27529225, -0.20431612, -0.11230148, -0.21400806, -0.13863319, -0.12405529, 0.026723886, -0.029678302, -0.137118, -0.032207325, -0.015937893, 0.034520473, -0.10537934, -0.10640544, -0.02788024, -0.004050025, 0.14284168, -0.004154746, -0.042710666, -0.0030131633, 0.0863726, -0.090727344, -0.0057157986, -0.15828685, -0.28061435, -0.34569633, -0.31643665, -0.23995298, -0.11509376, -0.18482517, -0.09795277, -0.13204533, -0.022816949, 0.16461055, 0.10385985, -0.041709572, 0.04911999, 0.034118626, 0.024611937, -0.1569084, -0.27869475, -0.10150442, -0.09299138, -0.017382314, 0.07238731, 0.034458127, 0.034288887, 0.10373614, -0.036800504, -0.103850745, -0.025551248, -0.09839303, -0.17764468, -0.19322814, -0.07784319, -0.13359855, -0.018441059, -0.026614375, -0.027435798, -0.10893549, -0.036571942, 0.17900826, 0.0952041, -0.03277465, 0.012166638, -0.026214013, -0.054429054, -0.05584976, -0.0890896, -0.17567648, -0.10156892, -0.080937944, 0.13743727, 0.11710831, 0.39706293, 0.15153725, -0.14898852, 0.016869906, -0.091097295, 0.050311003, -0.015997084, 0.03471436, 0.05124979, 0.015134098, -0.0787504, -0.09406039, -0.004352975, 0.05978947, 0.059898555, 0.053217594, 0.11745305, -0.058603983, -0.068262644, -0.07376681, -0.060936816, -0.05292235, 0.055443328, 0.057945356, -0.043431245, 0.0978319, 0.15021051, 0.18320222, 0.26638302, 0.282607, 0.096047334, -0.015947253, 0.17727478, 0.26429167, 0.256271, 0.22612385, 0.056433037, 0.020970806, -0.19327262, -0.09257251, 0.103279635, 0.029281562, -0.005036656, 0.06141524, -0.14611915, -0.13843282, -0.0674398, -0.040641256, 0.09839929, 0.06809891, -0.055872746, 0.103230566, 0.12929273, 0.093246624, 0.19625154, 0.3957336, 0.36821985, 0.18199718, 0.03128358, 0.10428929, 0.012325448, 0.27367637, 0.25326577, 0.16128966, 0.10174015, 0.01821238, -0.09355721, -0.102284156, -0.042537477, -0.07861375, -0.00063670275, -0.0918288, -0.11495731, -0.08748216, -0.06379943, 0.0289092, 0.06392375, 0.14881751, 0.026898835, 0.045247465, 0.17770568, 0.18050258, 0.20732449, 0.51710254, 0.41318235, 0.28153667, 0.06455095, 0.036671553, 0.11435294, 0.3717402, 0.24243991, 0.1621252, 0.18820693, 0.10378703, 0.0149970045, 0.013308225, 0.1003576, -0.082417056, -0.18152498, -0.08447481, -0.1115625, 0.023393665, 0.1525787, 0.20906968, 0.19529296, 0.13382505, 0.12370831, 0.20871851, 0.19963688, 0.14457247, 0.17292465, 0.42528972, 0.16648789, 0.216052, -0.07937481, 0.2228326, -0.07815052, 0.21050544, 0.22129941, 0.18628274, 0.19600666, 0.203606, 0.19073333, 0.16398336, 0.12775187, 0.17982225, -0.007697878, 0.09051248, 0.014885063, 0.06479056, 0.21126983, 0.27705312, 0.19656046, 0.20588987, 0.17736788, 0.20024577, 0.26922756, 0.14193293, 0.25486252, 0.5152834, 0.2387415, 0.0139934495, 0.06857737, 0.03444585, 0.04254651, 0.19809993, 0.29316798, 0.23355135, 0.2210379, 0.27114156, 0.27018958, 0.26134574, 0.22211167, 0.09998661, 0.13448659, 0.09378425, 0.22681554, 0.08342202, 0.13114649, 0.100132205, 0.14915478, 0.22300601, 0.20775528, 0.19838586, 0.047802787, 0.046977326, 0.20426102, 0.34645626, 0.119482666, -0.07512898, -0.09020904, -0.016132927, 0.18781473, 0.26898354, 0.16068986, 0.093633756, 0.1648248, 0.17079695, 0.11298578, 0.20909505, 0.1436601, 0.10771851, 0.12583525, 0.09236908, 0.1304369, 0.097095706, 0.09259565, 0.162785, 0.14500214, 0.25041336, 0.07904096, 0.027081769, 0.04392678, 0.13804351, 0.20821488, 0.29568923, 0.22807805, -0.046775665, -0.070677504, 0.08412367, 0.1503932, 0.23287767, 0.07735321, 0.11627669, 0.1838466, 0.10996063, 0.1959326, 0.114092804, 0.11338181, 0.16243121, 0.14449695, 0.032707613, 0.035520133, 0.09447449, 0.12626082, 0.07154252, 0.16889152, 0.05080031, 0.14508928, 0.06111541, 0.11751628, 0.02464158, 0.14769714, 0.17135666, -0.09159509, 0.035865437, 0.02762571, 0.022588827, 0.1371523, 0.24652955, 0.042262513, 0.14754266, 0.1191967, 0.07554328, 0.0038572613, 0.13796233, 0.13469557, 0.09159116, 0.05493333, 0.066756986, 0.10431767, -0.022919582, -0.061336398, 0.02660589, 0.060567826, 0.016253207, 0.033189982, 0.02836695, 0.1385043, 0.0033143926, 0.19162348, 0.021599766, -0.008129769, 0.056396015, 0.019446425, -0.065378055, 0.062330637, 0.18394215, 0.17467214, 0.11390484, 0.1251521, 0.17222637, 0.030417496, 0.022550056, 0.0715369, 0.01305342, 0.05157342, 0.12559593, 0.053058706, 0.036500894, 0.047596462, 0.06316666, 0.041432668, -0.15991189, 0.0009892613, 0.087297685, 0.047031738, 0.06608983, 0.36057928, 0.22885966, 0.10664244, 0.026624396, -0.04145759, -0.016153723, 0.0787704, -0.12104164, -0.02543689, 0.26739866, 0.29317203, 0.27624637, 0.29988712, 0.15485592, 0.14051357, 0.10146613, 0.25553706, 0.14301987, 0.0904313, -0.0119745815, 0.11647292, 0.055754308, 0.132123, 0.036582805, 0.1204077, -0.00026978893, -0.12145733, -0.117888525, -0.07305649, 0.19232209, 0.12615745, -0.030113168, 0.06142325, 0.064803936, -0.03531185, -0.020820774, -0.0056128954, 0.05741773, 0.13470079, 0.3092817, 0.3427361, 0.37720358, 0.303207, 0.26263517, 0.29909304, 0.32618058, 0.42187652, 0.36096236, 0.40189847, 0.4347345, 0.3222329, 0.35932744, 0.22373238, 0.13469888, 0.18721649, 0.1986572, 0.18026364, 0.019583771, -0.0012837574, -0.023005843, -0.0071130395, -0.0078054145, 0.02540557, 0.06992737, -0.0069329944, 0.0714641, -0.037567053, 0.0078016357, 0.122117475, 0.09710231, 0.1969791, 0.07039953, 0.11644787, 0.05826474, 0.3327689, 0.14013186, 0.18847287, 0.18537074, 0.2599924, 0.2540493, 0.12765698, 0.19030839, -0.005724312, 0.11647343, -0.064784996, -0.06742047, -0.06371046, -0.016462013],
    [-0.07998826, 0.013400167, -0.05781073, 0.07939536, -0.04039688, 0.021427102, -0.048890322, -0.0532644, -0.013533063, 0.022226356, -0.02931393, -0.028420743, 0.027610596, 0.12297865, -0.036298517, -0.08092291, 0.02055543, -0.008450724, -0.015808083, 0.051199637, 0.02936247, -0.049678557, 0.010803781, 0.060616456, 0.0015429556, -0.028271869, -0.022376016, -0.037070163, 0.0025865436, 0.014165498, 0.0791043, -0.017719202, 0.051558405, -0.0074589397, 0.054458138, 0.13089274, 0.04318659, 0.02255517, 0.13256402, 0.20044693, 0.25424802, 0.11484543, 0.07074045, 0.17780456, 0.11159939, 0.17206405, 0.12235251, 0.21479517, 0.21222259, 0.2108665, 0.1710404, 0.21024103, -0.052003026, 0.0048399046, -0.004242599, -0.055520397, 0.004898995, -0.05356196, 0.014684747, 0.1853283, 0.15226458, 0.039585944, 0.021211933, 0.0872976, -0.056893513, 0.03359006, -0.0644349, 0.024509735, 0.10833994, 0.13079964, 0.13010325, 0.27042094, 0.14935629, -0.023540787, 0.031000573, -0.062650874, -0.0074643567, 0.20240478, 0.23265521, 0.08000132, -0.08477093, -0.09141131, 0.033573538, 0.0024622306, 0.04061178, 0.04299099, 0.027851636, 0.17316861, -0.033905026, -0.19974253, -0.08091802, -0.23361762, -0.18341053, -0.15070404, -0.26105073, -0.34563866, -0.25563204, -0.030845061, -0.1663769, -0.034267105, -0.10238162, -0.16857432, -0.19144304, -0.3450281, -0.2926007, -0.18972643, 0.03595667, 0.13864455, 0.070771925, -0.04661556, -0.095269874, 0.008726791, -0.062734365, -0.014873065, 0.08856243, -0.107616685, -0.11179895, -0.14901692, -0.3145953, -0.27334464, -0.13835129, -0.013740017, -0.08706168, -0.087740146, -0.025770335, 0.048306827, 0.117257364, 0.16523328, 0.008990256, -0.004834446, -0.021743705, 0.03762292, 0.09955199, 0.08928289, -0.04047367, 0.054888155, -0.24096918, -0.1151983, 0.08358449, 0.095211156, -0.016206115, -0.054446213, 0.09351422, -0.1414671, -0.22747426, -0.21970107, -0.13016541, -0.20015724, -0.28146017, -0.1954831, -0.12780268, -0.06775895, 0.007196329, 0.09468754, 0.078764565, 0.15577953, 0.045859627, 0.08423092, 0.08018276, 0.13620202, 0.14676005, 0.1950977, 0.16423863, 0.019032033, -0.07789834, -0.12285776, -0.096439764, -0.042920336, -0.042209327, -0.01820463, -0.062043685, -0.2944893, -0.1608133, -0.1938311, -0.18265764, -0.08322781, -0.15289706, -0.1842974, -0.0775431, -0.10558259, 0.03916874, 0.07024787, -0.043276243, 0.029663302, -0.0068889214, -0.016178096, 0.03646683, 0.04618042, 0.06211504, 0.139723, 0.16796044, 0.1739367, 0.012693472, -0.12656951, -0.021943243, -0.08308853, 0.016019955, -0.24531356, -0.05004962, -0.16708685, -0.16550833, -0.16091149, -0.22202006, -0.06381091, -0.13326988, -0.14949176, -0.10610766, 0.012258823, -0.03537229, 0.020659298, 0.015008141, 0.040182192, 0.040530663, 0.09086273, 0.06724861, 0.115658775, 0.05755026, 0.14002067, 0.1534032, 0.14034297, -0.016223479, -0.25659147, -0.059390385, -0.05278303, 0.17595209, -0.11837042, -0.04386648, -0.062008545, -0.21206798, -0.093588874, -0.12308144, -0.13484482, -0.092741676, 0.04028208, -0.026978334, -0.0072515616, 0.118114956, 0.17395398, 0.029832492, 0.064959586, 0.056589477, 0.168631, 0.062540404, 0.037968405, 0.013560886, 0.052219052, -0.031182941, 0.02021161, -0.09734968, -0.16703011, 0.051435303, 0.16328721, 0.050696496, -0.060305636, -0.18658555, 0.09836611, -0.16646266, -0.08815716, -0.036798354, -0.0027154267, -0.053877626, 0.069289796, 0.1204474, 0.09446918, 0.20443207, 0.118916616, 0.20070595, 0.08276975, 0.042580307, -0.013793929, 0.10814186, -0.061286904, -0.031065276, -0.015829332, -0.05195249, 0.048178084, -0.05944089, -0.3519809, -0.0890019, -0.07113984, -0.08181112, -0.10294782, -0.14489363, -0.10886637, -0.108814925, -0.03409546, -0.062506475, 0.10405277, 0.052745737, 0.11470138, 0.16833587, 0.2543588, 0.1776808, 0.1667295, 0.26766032, 0.076052114, -0.103456475, -0.12910658, -0.079019845, -0.03093422, -0.0847255, -0.09204616, 0.077102035, 0.086993955, -0.2624022, -0.38029736, -0.21103677, -0.014654842, -0.06045121, 0.07933701, -0.14614117, -0.31477398, -0.26020238, -0.0034733335, 0.0022374927, 0.15924516, 0.09126757, 0.118846476, 0.044856958, 0.018086554, 0.09758908, 0.16442384, 0.31134018, 0.17105311, -0.115060404, -0.2141179, -0.122972295, -0.10470613, 0.0025217636, 0.07112401, 0.2278757, 0.1888091, 0.24331008, -0.25564396, -0.024862828, -0.029485123, 0.022375239, -0.07162024, -0.19104356, 0.008301475, -0.072353624, 0.02984243, 0.08479973, 0.15464133, 0.0562685, 0.08956804, -0.0638396, -0.119372524, 0.024352297, 0.13699651, 0.30253622, 0.062235057, -0.05898443, -0.1510789, -0.08035702, -0.018601468, 0.06363901, 0.27075773, 0.34989154, 0.47519168, 0.4331644, -0.08901152, -0.1460334, -0.058298036, 0.15977624, 0.037370827, -0.13742034, -0.043545224, -0.08275014, 0.00040199404, 0.051188316, -0.05213217, -0.048987053, -0.11005039, -0.14378694, -0.17735565, 0.0110212965, 0.23975602, 0.13805501, 0.22451442, -0.119530454, -0.16768761, -0.16342066, -0.08382904, -0.029512225, 0.10265338, 0.22214113, 0.31482032, 0.18115655, -0.0112548405, -0.21959352, 0.036874678, 0.019255586, -0.15293024, -0.020993512, -0.104899585, -0.14405099, -0.09006388, -0.03825777, -0.024018481, -0.009438053, -0.047506165, -0.12609838, -0.09451285, -0.007751165, 0.24696964, 0.26876485, 0.13816704, -0.062299702, -0.16071472, -0.1223365, -0.2039707, -0.21193616, -0.08574144, -0.18668742, -0.003187351, 0.07170503, -0.03802786, 0.08319597, 0.1230627, -0.09814668, -0.07874623, -0.092743106, -0.07913111, -0.0057615214, -0.031861965, -0.031682003, 0.0392, -0.0070471177, -0.033980303, -0.077026665, 0.03369717, 0.02787678, 0.26461235, 0.21975835, 0.015362471, -0.11633083, -0.16681215, -0.16143638, -0.23563561, -0.2942729, -0.23680219, -0.27242658, -0.18126602, -0.007292224, 0.17114548, -0.15537785, -0.15909433, -0.004229875, -0.11810261, -0.10471873, -0.004672175, -0.14373714, -0.068031594, -0.03593827, 0.10361714, 0.12378223, 0.04561939, 0.07949511, 0.071679294, -0.033595897, 0.19903302, 0.24354717, -0.0043454496, -0.13932802, -0.1870631, -0.19076918, -0.30563933, -0.24343185, -0.25479785, -0.28602746, -0.112390414, -0.18250674, -0.0446629, -0.10319121, -0.07379981, 0.068282716, -0.06286747, -0.19958608, -0.12603718, -0.25246853, -0.22305888, -0.060827795, -0.014229967, 0.07255238, 0.24568984, 0.2743211, 0.18842243, 0.14988627, 0.23082975, 0.122592755, -0.06403167, -0.047850624, -0.14292285, -0.31253237, -0.19966434, -0.12782703, -0.11341074, -0.15123062, 0.0072101895, -0.019714722, -0.05913895, -0.15856, -0.05149674, 0.062210772, 0.011626487, -0.19376723, -0.34615573, -0.28679812, -0.2455207, -0.089569144, -0.021521604, 0.12671812, 0.24661337, 0.21909748, 0.19642861, 0.20364854, 0.14861272, 0.07954335, -0.019343596, -0.2095473, -0.19146505, -0.16073236, -0.12760134, -0.08413624, -0.12709336, 0.053523958, 0.048154864, 0.07101853, 0.0698852, -0.011693973, 0.018857041, 0.049749535, -0.004715626, -0.042234197, -0.33956364, -0.22706012, -0.1726399, -0.17093863, 0.0862706, 0.08883192, 0.23035751, 0.19305603, 0.1772663, 0.09398789, 0.03310541, 0.05941199, -0.03292059, -0.010653906, 0.011010513, -0.010003526, 0.014339718, 0.01952551, 0.12545724, 0.0799564, 0.1606031, 0.038624194, 0.08880125, -0.116730295, -0.07285734, -0.07194875, 0.060091175, -0.18721782, -0.2627738, -0.24628374, -0.21421778, -0.012825112, 0.06079321, 0.0947938, 0.03357982, 0.22875628, 0.09675793, 0.20575055, 0.040710572, 0.092057824, 0.104772694, -0.014473753, 0.09343232, 0.06651933, 0.096583, 0.102383345, 0.10590809, 0.2102987, 0.08927102, 0.053837564, 0.102832995, -0.03711393, -0.08297252, 0.10018413, -0.085690774, -0.111101285, -0.1851447, -0.17795947, 0.14192416, 0.18586351, 0.17649513, 0.06974516, 0.18225493, 0.13227253, 0.118234836, 0.093139656, 0.018651986, 0.033965714, 0.008218105, 0.08830589, 0.041866563, 0.21528126, 0.065706335, 0.17014465, 0.080049515, 0.03582194, 0.09827532, -0.027545039, 0.08970343, 0.0067227497, 0.00894708, -0.025795845, 0.031006789, -0.12791215, -0.065119065, 0.06778229, 0.15438765, 0.21795714, 0.085357115, 0.16171068, 0.013885581, 0.020977844, -0.03651419, -0.014107119, 0.05145884, 0.09610188, 0.070010655, 0.12513587, 0.11506451, 0.119666934, 0.08990999, 0.08964679, -0.014606763, -0.06368981, 0.02604123, 0.08501976, 0.07581352, 0.023619836, 0.044476066, -0.08335734, 0.08168436, -0.10088853, -0.3184325, -0.28783926, -0.05975404, -0.031200089, -0.07735072, 0.07553293, 0.052618228, 0.01290703, -0.05319033, 0.1584077, 0.08191264, 0.16469821, 0.112340316, 0.16855285, 0.2553544, 0.0985631, 0.061011117, 0.037234236, 0.021548998, -0.017680554, 0.0033980855, -0.07659879, 0.02918029, -0.09437282, -0.046832297, -0.0380979, -0.048271313, -0.07981603, -0.34155568, -0.39753884, -0.4339522, -0.28020957, -0.25917265, -0.12347085, -0.110260695, -0.119726345, 0.0051177703, 0.007821904, -0.10427155, 0.07582625, 0.061310608, 0.007001551, -0.010503294, 0.11798704, 0.13564321, -0.012210375, -0.018688634, -0.1627207, 0.03912753, 0.023529163, 0.13838334, 0.09544465, -0.043297995, -0.016645595, 0.026075192, 0.1062701, -0.24606705, -0.33765388, -0.34045362, -0.3080438, -0.43418, -0.3800932, -0.34190246, -0.27440804, -0.25746042, -0.19304465, -0.19104873, -0.17217894, -0.057087816, -0.011421568, -0.0798042, 0.028244926, -0.07356789, 0.07189404, -0.07463182, -0.11977243, -0.039144352, -0.11466957, -0.01811904, -0.07810496, -0.05354544, 0.03356479, -0.06429185, -0.024842791, 0.01834427, -0.045522366, -0.07985492, -0.109240234, -0.20699404, 0.015645145, -0.06386413, -0.11791882, -0.0386246, 0.15606926, -0.06567291, 0.0074172625, -0.07376158, 0.0055710236, -0.166385, -0.021026244, -0.06575546, -0.096578725, -0.117154665, -0.16583318, -0.10680973, -0.074907295, 0.031705663, -0.031386375, -0.025770154, 0.019706137, 0.018122025, 0.07441359, -0.06594202, -0.16533165, -0.10972214, -0.042192195, 0.020604512, -0.1063372, -0.016491622, -0.17054893, 0.036574025, 0.04238768, -0.2537972, -0.16919385, -0.029870985, -0.07075624, -0.30910626, -0.15219809, -0.1737524, 0.021521533, -0.03476055, -0.11613308, -0.06925754, 0.08322691, -0.041483458, -0.034092527, -0.03678496],
    [-0.038426857, 0.08576115, 0.039211176, 0.032674015, 0.083360486, -0.010007463, -0.011702746, -0.002599001, -0.08309174, -0.00711675, 0.05650363, -0.060272053, -0.11142928, 0.0355054, -0.0519832, 0.00078580476, 0.075450845, -0.05796551, -0.04027348, 0.056729577, 0.02339188, 0.055251487, 0.06451339, -0.0061224625, 0.009986654, 0.039654844, -0.078297, 0.041503124, -0.06308911, -0.018692806, 0.07611311, 0.017490909, 0.056793403, -0.11042448, -0.13077322, -0.13880767, -0.09828736, -0.09331853, -0.12798369, -0.14414671, -0.23444238, -0.1993986, -0.009111954, -0.19856778, 0.013651943, -0.0275366, -0.17507523, -0.19681968, -0.16624928, -0.17936419, -0.13870105, -0.11066684, -0.038354483, -0.053575058, -0.013719797, -0.003054984, -0.009075873, -0.013662718, 0.045770746, -0.017442476, -0.0637795, -0.13697039, -0.22809775, -0.2678276, -0.10891064, -0.053272657, -0.14780182, -0.25158545, -0.2773978, -0.27077004, -0.19789688, -0.15062653, -0.16035311, -0.23083058, -0.24691318, -0.116106436, -0.20164703, -0.0394892, -0.13135615, -0.17456765, 0.011412165, 0.1533824, 0.009803981, -0.060491577, -0.061362445, 0.042308457, 0.0056325323, -0.07970806, -0.0959869, -0.09859686, -0.110843875, -0.22940643, -0.1635129, -0.04810195, -0.10348953, -0.07809169, -0.20113827, -0.23785181, -0.32494247, -0.33455744, -0.24065767, -0.2426817, -0.33029008, -0.26304373, -0.30823556, -0.1959537, -0.13384004, -0.16365781, -0.13499327, -0.057270665, -0.10132811, 0.059447534, 0.037632465, -0.042155944, -0.1423831, -0.19463317, -0.09602449, -0.20290022, -0.13693298, 0.02277011, -0.17413507, -0.17877555, -0.2081257, -0.24725685, -0.11688376, -0.12425939, -0.22147156, -0.1566354, -0.16449231, -0.049374267, -0.18650019, -0.050966352, -0.01719646, -0.03253386, -0.011839683, 0.030756442, 0.06851233, 0.00048049874, -0.10585447, 0.0047983346, 0.021553546, -0.04937277, -0.034173544, -0.10241838, -0.006197747, -0.08538282, -0.015288841, -0.02577592, -0.0018003804, -0.067436144, -0.06854778, 0.045659408, -0.00021223558, -0.05416226, 0.1005359, -0.023049425, 0.012268075, 0.10864431, 0.042346843, 0.0048589227, 0.012199401, -0.032780927, 0.10924058, 0.17350104, -0.044495154, -0.16554086, -0.23209862, 0.075796254, 0.042577095, -0.14570126, -0.00803404, -0.024829164, 0.056439843, 0.070620395, 0.020305702, 0.028651232, -0.09599595, 0.012904373, -0.07481105, -0.08241438, -0.025040474, 0.048217826, 0.020067157, -0.0027509695, 0.033233825, 0.054076865, 0.09016293, 0.05037615, 0.07114398, 0.091510646, 0.10352199, 0.107399575, -0.052175496, -0.15719563, -0.17029637, 0.05410476, 0.025560318, -0.15449148, 0.013803276, 0.002837287, 0.07914474, -0.0054793702, -0.031247567, 0.020165015, -0.0063717407, -0.030748645, 0.048189282, 0.02637318, -0.040239397, -0.00087111717, -0.004629268, 0.003055188, 0.08960191, 0.07786131, 0.029632665, 0.04126278, 0.01401573, 0.023310924, 0.09919246, 0.13726708, 0.075271696, -0.07316765, 0.025716137, -0.062589355, 0.17412744, -0.061521415, 0.015490752, -0.12946072, -0.13497344, 0.04095999, -0.12636381, -0.12807028, -0.029614866, 0.09354149, 0.018088633, 0.09594874, 0.029662447, 0.012366068, -0.005804758, 0.003034445, 0.014638889, 0.08728344, 0.14636482, 0.0523649, 0.03303917, 0.09897151, -0.010348966, 0.18671885, -0.02137638, -0.16690819, -0.10347198, -0.21861649, -0.010121652, 0.010488179, -0.046565138, -0.18970767, -0.11600396, -0.06394462, -0.039276708, -0.103103645, 0.029490305, 0.044009205, 0.06768686, 0.031012045, 0.10908537, 0.05002146, 0.015074228, -0.034520943, 0.13935138, 0.11811892, 0.21628197, 0.071052946, 0.05885067, -0.07872635, -0.07418017, 0.009591536, -0.095967375, -0.25324568, -0.3908064, -0.091051556, -0.019617364, -0.09897615, -0.16143858, -0.21244283, -0.20657194, -0.07383756, -0.052940726, -0.04190696, 0.08879979, 0.16674262, 0.041274056, 0.14721575, 0.10799595, -0.099566534, -0.2239594, -0.07930747, 0.07167388, 0.16114615, 0.0669091, 0.050873287, -0.028614596, -0.0022034983, -0.016181864, -0.070648655, -0.15619814, -0.40327635, -0.3478635, -0.03459747, -0.012171734, -0.08661149, -0.16560729, -0.3052502, -0.24972402, 0.057947956, 0.050454326, -0.038719397, 0.14242476, 0.110341825, 0.22572961, 0.096984714, -0.02677584, -0.33285758, -0.42487007, -0.15612906, 0.12449292, 0.022337183, 0.10115622, 0.0870801, 0.07873168, -0.030808138, -0.09388234, -0.2164369, -0.22673209, -0.26906717, -0.23237145, -0.09335487, 0.048955448, -0.08391992, -0.117750555, -0.26671353, -0.04580828, -0.07175274, 0.0135953175, 0.12642275, 0.11846696, 0.10781633, 0.1360729, 0.22962223, 0.06049919, -0.24612993, -0.3725562, -0.17306417, 0.024298646, 0.05694121, 0.07092535, 0.01775074, 0.05443176, 0.18502171, 0.12598875, -0.02026559, 0.016871657, -0.040651243, -0.1381532, 0.109932184, -0.07735608, 0.045536432, -0.12939388, -0.22380362, -0.03822509, 0.048232958, 0.15512364, 0.18966597, 0.12751159, 0.14312854, 0.2754419, 0.29657805, 0.04872517, -0.23724872, -0.1079302, -0.055140365, 0.02659306, 0.1735088, 0.12781292, 0.14836593, 0.06428402, 0.2017757, 0.19553827, 0.10126335, 0.14686657, 0.21221541, 0.14232777, 0.046309326, -0.063999064, -0.08345268, -0.18469785, -0.10028037, 0.16172837, 0.24634655, 0.14294405, 0.30454418, 0.30727592, 0.22187902, 0.1598778, 0.20249996, -0.056022987, -0.087726615, -0.14856885, -0.06570432, 0.035122216, 0.24985728, 0.093774736, 0.08783556, 0.16043718, 0.08401653, 0.14104448, 0.11417822, 0.016321963, 0.109494045, 0.17640926, 0.054814912, -0.20084389, -0.13420472, -0.124961406, -0.0031378784, 0.21851823, 0.22028069, 0.26465365, 0.2229883, 0.27228156, 0.16700871, 0.27616987, 0.15018576, -0.02898986, -0.20366295, -0.18379769, 0.048257694, 0.18356001, 0.19122544, 0.1751509, 0.14724839, 0.020834096, 0.06704181, 0.08694761, 0.007527763, -0.06327138, 0.054132495, 0.24975774, 0.2323109, -0.035700437, -0.17875713, -0.07367599, -0.13155915, 0.07786, 0.17859408, 0.13985297, 0.28336, 0.25424868, 0.18594469, 0.14654006, 0.05085678, 0.006249455, -0.17125252, 0.04210459, 0.1402758, 0.31379163, 0.21798396, 0.12548359, 0.014392022, 0.09081221, 0.10570472, -0.026450824, -0.07665679, -0.07417713, 0.1978674, 0.28733754, 0.13113198, 0.029774487, -0.16358605, -0.087043606, -0.14222392, 0.026130565, 0.09172731, 0.17640373, 0.12863559, 0.18039563, 0.17695124, 0.099234454, 0.05330788, 0.03873019, -0.04107332, 0.18903103, 0.2814278, 0.23056431, 0.09566821, 0.106712595, 0.014651643, 0.04551265, 0.122416675, -0.039783675, -0.13571215, -0.039970174, 0.3782854, 0.1538621, 0.21222927, -0.08749741, -0.05378812, 0.008622499, -0.20274928, 0.0463425, 0.13112628, 0.06323003, 0.18394984, 0.05336617, 0.027774503, 0.1380984, 0.07469095, 0.079151206, 0.103160635, 0.16286404, 0.14653286, 0.07202218, 0.08780207, 0.0152715705, -0.023231782, 0.046357084, -0.050454214, -0.13086222, 0.0012163552, 0.14533837, 0.24114406, 0.14568985, 0.01759025, -0.08017066, -0.06171064, 0.020117806, -0.1823453, -0.028027616, 0.0701927, 0.029134704, 0.07017532, 0.011927094, -0.060764506, -0.033940822, 0.008350631, -0.07398411, 0.044128608, 0.14270917, 0.045375403, 0.05682602, -0.07850807, -0.0245481, -0.032654714, -0.07971098, 0.004328526, -0.06932907, -0.05580926, 0.024266938, 0.19344904, 0.1480497, 0.0052285204, 0.018848248, 0.14713807, -0.004720059, -0.15506865, 0.036541585, 0.03345889, 0.011102442, -0.0056830086, 0.0009855748, -0.03177815, -0.04410713, 0.014768718, 0.039278734, -0.050068237, -0.053844128, 0.038008, 0.017805753, -0.03312851, -0.08304788, -0.05371244, -0.119698614, -0.078340515, -0.093021795, -0.05534991, 0.09611831, 0.052170273, 0.006118202, -0.0014739955, -0.12064447, -0.046628978, 0.072960146, -0.09487198, -0.068337366, 0.047073063, -0.12296031, -0.14344479, -0.025461858, -0.0041371672, 0.0056481147, 0.09416417, -0.053161632, -0.016134791, -0.047172587, -0.06206256, -0.072318986, -0.018601608, -0.08375164, 0.0035192124, -0.020886825, 0.00025076204, 0.036028627, 0.07958578, 0.07798331, -0.018196577, 0.2309556, -0.028678656, -0.08350914, 0.004715571, 0.053177502, -0.18026677, -0.061869647, -0.12311045, -0.07356235, -0.04650966, 0.08294997, 0.120507, 0.12871546, 0.1072069, 0.07722238, 0.007919039, -0.03986744, 0.025507234, -0.04531437, 0.044426184, 0.025280448, 0.0686526, 0.085944414, 0.15076911, 0.072989754, 0.031188767, 0.08055819, -0.05585016, -0.029030472, 0.052831724, 0.027295634, -0.012572058, -0.20115818, -0.1858582, -0.1941269, -0.09424879, -0.045097545, -0.022496592, 0.03284324, 0.061371684, 0.15837733, 0.11267602, 0.097618535, 0.109287865, 0.057855867, 0.05782363, 0.05295945, 0.08103134, -0.010900975, 0.0071518114, 0.008227218, 0.22149678, 0.13323559, 0.00071484345, 0.08088333, 0.08902094, -0.12081692, 0.018402934, 0.04918232, -0.083634295, -0.17204875, -0.2554248, -0.17074047, -0.18774302, -0.09680438, -0.1281804, -0.09777654, -0.07650161, -0.001939463, -0.048671734, 0.05037785, -0.07661275, 0.06847263, -0.05372994, 0.025222268, -0.0048362133, 0.01252201, 0.06497949, 0.032784093, 0.17586766, 0.15603618, 0.08143262, 0.12635887, 0.16254003, 0.15158902, 0.006986864, 0.024752572, 0.031168714, 0.02326422, 0.015857037, 0.044109393, 0.085653454, -0.15490222, -0.2558903, -0.27035773, -0.28211334, -0.21600252, -0.28430307, -0.2933045, -0.30882478, -0.21540755, -0.25172666, -0.20559944, -0.19917364, -0.15132423, -0.18284564, -0.03463188, -0.09841826, -0.0576677, -0.11654417, -0.14718229, 0.18303274, 0.042108584, -0.005296886, -0.07505693, -0.040131096, 0.0048105195, 0.09858414, 0.0995753, 0.043677416, 0.18558088, -0.020062206, -0.083483264, -0.089955926, 0.0076240026, -0.028124833, -0.008713862, -0.09566562, -0.0062104347, -0.14992535, -0.17362761, -0.09533432, -0.17029466, -0.20316602, -0.016001284, -0.18828535, -0.14893788, 0.019897124, -0.081878364, -0.07103603, -0.023612626, -0.08436988, 0.06310689, -0.02058573, 0.038824596, -0.0824106, -0.05942255, -0.049455296, -0.059800263, -0.03204529, -0.036551375, -0.07549339, -0.15626334, -0.079428084, -0.02468015, -0.1719436, -0.038216237, -0.13731202, -0.040409017, -0.12956066, -0.08955298, -0.087184094, 0.053014785, -0.1282517, -0.06794646, -0.105690695, 0.01765918, 0.0060942993, 0.045892216, 0.062137045],
];

fn forward_propagation(x: &[[SoftF64; 28]; 28]) -> U256 {
    let mut X: [SoftF64; 28 * 28] = [SoftF64(0.0_f64); 28 * 28];
    for i in 0..28 {
        for j in 0..28 {
            X[i * 28 + j] = x[i][j];
        }
    }

    let mut z1: [SoftF64; 10] = [SoftF64(0.0_f64); 10];
    for i in 0..784 {
        for j in 0..10 {
            z1[i] = z1[i].add(SoftF64(W1[i][j]).mul(X[j]));
        }
    }
    let mut a1: [SoftF64; 10] = [SoftF64(0.0); 10];
    let mut index = 0;
    U256::from(index)
}

sol_storage! {
    #[entrypoint]
    pub struct Counter {
    }
}

#[public]
impl Counter {
    pub fn classify(&self, mat: Vec<Vec<U256>>) -> U256 {
        let mut matrix: [[SoftF64; 28]; 28] = [[SoftF64(0.0_f64); 28]; 28];
        for i in 0..28 {
            for j in 0..28 {
                match mat[i][j].eq(&U256::from(1)) {
                    true => matrix[i][j] = SoftF64(1.0_f64),
                    false => matrix[i][j] = SoftF64(0.0_f64)
                }
            }
        }

        forward_propagation(&mut matrix)
    }
}