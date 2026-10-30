//! Numbers in answers are cut to three decimals: milliseconds, or a pixel of a 1000-pixel frame.

pub(crate) fn thousandths(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}
