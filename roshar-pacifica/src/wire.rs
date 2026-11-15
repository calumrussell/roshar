//! Exact decimal strings for prices and amounts sent to Pacifica.

/// Largest absolute change rounding may make before the value is rejected.
const MAX_ROUNDING_ERROR: f64 = 1e-12;

/// Formats `x` as a decimal string with at most 8 decimal places and no trailing zeros, e.g.
/// `0.1 + 0.2` gives `"0.3"` where `f64::to_string` gives `"0.30000000000000004"`.
///
/// Returns an error for NaN, infinity, or a value that 8 decimal places cannot represent
/// (e.g. `1e-9`), rather than silently sending a different number.
pub fn float_to_wire(x: f64) -> Result<String, String> {
    if !x.is_finite() {
        return Err(format!("cannot send non-finite value {x}"));
    }
    let rounded = format!("{x:.8}");
    let back: f64 = rounded
        .parse()
        .map_err(|e| format!("cannot reparse {rounded:?}: {e}"))?;
    if (back - x).abs() >= MAX_ROUNDING_ERROR {
        return Err(format!(
            "{x} has more than 8 decimal places (rounds to {rounded})"
        ));
    }
    let trimmed = rounded.trim_end_matches('0').trim_end_matches('.');
    Ok(match trimmed {
        "-0" | "" => "0".to_string(),
        s => s.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::float_to_wire;

    #[test]
    fn exact_decimals() {
        assert_eq!(float_to_wire(0.1 + 0.2).unwrap(), "0.3");
        assert_eq!(float_to_wire(100.0).unwrap(), "100");
        assert_eq!(float_to_wire(1e-8).unwrap(), "0.00000001");
        assert_eq!(float_to_wire(123.456).unwrap(), "123.456");
        assert_eq!(float_to_wire(-1.5).unwrap(), "-1.5");
        assert_eq!(float_to_wire(0.0).unwrap(), "0");
        assert_eq!(float_to_wire(-0.0).unwrap(), "0");
        assert_eq!(float_to_wire(65432.1).unwrap(), "65432.1");
    }

    #[test]
    fn rejects_unrepresentable() {
        assert!(float_to_wire(1e-9).is_err());
        assert!(float_to_wire(0.123456789).is_err());
        assert!(float_to_wire(f64::NAN).is_err());
        assert!(float_to_wire(f64::INFINITY).is_err());
        assert!(float_to_wire(f64::NEG_INFINITY).is_err());
    }
}
