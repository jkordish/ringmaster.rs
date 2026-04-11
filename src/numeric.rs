use num_traits::ToPrimitive;

#[must_use]
pub fn usize_to_f64(value: usize) -> f64 {
    value.to_f64().unwrap_or_else(|| f64::from(u32::MAX))
}

#[must_use]
pub fn i64_to_f64(value: i64) -> f64 {
    value.to_f64().unwrap_or_else(|| {
        if value.is_negative() {
            f64::from(i32::MIN)
        } else {
            f64::from(i32::MAX)
        }
    })
}

#[must_use]
pub fn usize_to_i64(value: usize) -> i64 {
    value.to_i64().unwrap_or(i64::MAX)
}

#[must_use]
pub fn i64_to_usize(value: i64) -> usize {
    value.to_usize().unwrap_or_default()
}

#[must_use]
pub fn rounded_nonnegative_f64_to_u64(value: f64) -> u64 {
    if !value.is_finite() || value <= 0.0 {
        0
    } else {
        value.round().to_u64().unwrap_or(u64::MAX)
    }
}
