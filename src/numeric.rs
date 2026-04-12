use num_traits::ToPrimitive;

#[must_use]
pub fn usize_to_f64(value: usize) -> f64 {
    let converted = value.to_f64();
    debug_assert!(
        converted.is_some(),
        "usize to f64 conversion should only fail on exotic targets or oversized values"
    );
    converted.unwrap_or(f64::MAX)
}

#[must_use]
pub fn i64_to_f64(value: i64) -> f64 {
    let converted = value.to_f64();
    debug_assert!(
        converted.is_some(),
        "i64 to f64 conversion should only fail on exotic targets or oversized values"
    );
    converted.unwrap_or_else(|| {
        if value.is_negative() {
            f64::MIN
        } else {
            f64::MAX
        }
    })
}

#[must_use]
pub fn usize_to_i64(value: usize) -> i64 {
    value.to_i64().unwrap_or(i64::MAX)
}

#[must_use]
pub fn i64_to_usize(value: i64) -> usize {
    value
        .to_usize()
        .unwrap_or_else(|| if value.is_negative() { 0 } else { usize::MAX })
}

#[must_use]
pub fn rounded_nonnegative_f64_to_u64(value: f64) -> u64 {
    if !value.is_finite() || value <= 0.0 {
        0
    } else {
        value.round().to_u64().unwrap_or(u64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::{i64_to_f64, i64_to_usize, rounded_nonnegative_f64_to_u64, usize_to_f64};

    #[test]
    fn characterizes_basic_integer_to_float_conversions() {
        assert!((usize_to_f64(12) - 12.0).abs() < f64::EPSILON);
        assert!((i64_to_f64(-42) + 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn clamps_negative_signed_values_before_usize_conversion() {
        assert_eq!(i64_to_usize(-7), 0);
    }

    #[test]
    fn rounds_and_saturates_nonnegative_float_to_u64() {
        assert_eq!(rounded_nonnegative_f64_to_u64(12.6), 13);
        assert_eq!(rounded_nonnegative_f64_to_u64(f64::INFINITY), 0);
    }
}
