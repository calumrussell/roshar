//! Price rounding to Hyperliquid's tick rules. The caller picks the direction.
//!
//! Hyperliquid also allows any integer price regardless of significant figures
//! (https://hyperliquid.gitbook.io/hyperliquid-docs/for-developers/api/tick-and-lot-size, checked
//! 2026-11-15). That exemption is not applied: prices >= 100000 are rounded to 5 significant figures,
//! which is valid but coarser than necessary. Applying it changes the pinned `to_tick` golden vectors.

/// Price rounding to a valid Hyperliquid perp price in one direction:
/// <= (6 - sz_decimals) decimals and <= 5 significant figures.
pub fn to_tick(px: f64, sz_decimals: u32, up: bool) -> f64 {
    let dec_tick = 10f64.powi(-(6 - sz_decimals as i32));
    let sig_tick = 10f64.powi(px.log10().floor() as i32 - 4);
    let tick = dec_tick.max(sig_tick);
    let n = (px / tick * 1e6).round() / 1e6;
    let n = if up { n.ceil() } else { n.floor() };
    let decimals = 6 - sz_decimals as i32;
    let p = 10f64.powi(decimals);
    (n * tick * p).round() / p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_tick_matches_python() {
        // values from hl_recorder.trader.to_tick
        assert_eq!(to_tick(0.0043605, 0, false), 0.00436);
        assert_eq!(to_tick(0.0043605, 0, true), 0.004361);
        assert_eq!(to_tick(187.456789, 2, false), 187.45);
    }
}
