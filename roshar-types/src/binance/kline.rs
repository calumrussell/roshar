use serde::{Deserialize, Deserializer, Serialize};

/// Historical kline/candlestick data from Binance REST API.
///
/// Binance returns klines as arrays in the format:
/// [open_time, open, high, low, close, volume, close_time, quote_volume, num_trades, taker_buy_base_volume, taker_buy_quote_volume, ignore]
///
/// Prices and volumes are returned as strings and need to be converted to f64.
#[derive(Debug, Clone, Serialize)]
pub struct BinanceKline {
    pub open_time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub close_time: i64,
    pub quote_volume: f64,
    pub num_trades: u64,
    pub taker_buy_base_volume: f64,
    pub taker_buy_quote_volume: f64,
}

impl<'de> Deserialize<'de> for BinanceKline {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{Error, SeqAccess, Visitor};
        use std::fmt;

        struct BinanceKlineVisitor;

        impl<'de> Visitor<'de> for BinanceKlineVisitor {
            type Value = BinanceKline;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a sequence of 12 elements representing a Binance kline")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let open_time: i64 = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("open_time"))?;

                let open_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("open"))?;
                let open = open_str
                    .parse::<f64>()
                    .map_err(|_| Error::custom("failed to parse open as f64"))?;

                let high_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("high"))?;
                let high = high_str
                    .parse::<f64>()
                    .map_err(|_| Error::custom("failed to parse high as f64"))?;

                let low_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("low"))?;
                let low = low_str
                    .parse::<f64>()
                    .map_err(|_| Error::custom("failed to parse low as f64"))?;

                let close_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("close"))?;
                let close = close_str
                    .parse::<f64>()
                    .map_err(|_| Error::custom("failed to parse close as f64"))?;

                let volume_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("volume"))?;
                let volume = volume_str
                    .parse::<f64>()
                    .map_err(|_| Error::custom("failed to parse volume as f64"))?;

                let close_time: i64 = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("close_time"))?;

                let quote_volume_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("quote_volume"))?;
                let quote_volume = quote_volume_str
                    .parse::<f64>()
                    .map_err(|_| Error::custom("failed to parse quote_volume as f64"))?;

                let num_trades: u64 = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("num_trades"))?;

                let taker_buy_base_volume_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("taker_buy_base_volume"))?;
                let taker_buy_base_volume = taker_buy_base_volume_str
                    .parse::<f64>()
                    .map_err(|_| Error::custom("failed to parse taker_buy_base_volume as f64"))?;

                let taker_buy_quote_volume_str: String = seq
                    .next_element()?
                    .ok_or_else(|| Error::missing_field("taker_buy_quote_volume"))?;
                let taker_buy_quote_volume = taker_buy_quote_volume_str
                    .parse::<f64>()
                    .map_err(|_| Error::custom("failed to parse taker_buy_quote_volume as f64"))?;

                // Skip the "ignore" field (12th element)
                let _ignore: Option<String> = seq.next_element()?;

                Ok(BinanceKline {
                    open_time,
                    open,
                    high,
                    low,
                    close,
                    volume,
                    close_time,
                    quote_volume,
                    num_trades,
                    taker_buy_base_volume,
                    taker_buy_quote_volume,
                })
            }
        }

        deserializer.deserialize_seq(BinanceKlineVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binance_kline_deserialize() {
        let json_str = r#"[
            1499040000000,
            "0.01634000",
            "0.80000000",
            "0.01575800",
            "0.01577100",
            "148976.11427815",
            1499644799999,
            "2434.19055334",
            308,
            "1756.87402397",
            "28.46694368",
            "17928899.62484339"
        ]"#;

        let kline: BinanceKline = serde_json::from_str(json_str).unwrap();

        assert_eq!(kline.open_time, 1499040000000);
        assert!((kline.open - 0.01634).abs() < 1e-10);
        assert!((kline.high - 0.8).abs() < 1e-10);
        assert!((kline.low - 0.015758).abs() < 1e-10);
        assert!((kline.close - 0.015771).abs() < 1e-10);
        assert!((kline.volume - 148976.11427815).abs() < 1e-10);
        assert_eq!(kline.close_time, 1499644799999);
        assert!((kline.quote_volume - 2434.19055334).abs() < 1e-10);
        assert_eq!(kline.num_trades, 308);
        assert!((kline.taker_buy_base_volume - 1756.87402397).abs() < 1e-10);
        assert!((kline.taker_buy_quote_volume - 28.46694368).abs() < 1e-10);
    }

    #[test]
    fn test_binance_kline_deserialize_array() {
        let json_str = r#"[
            [1499040000000, "0.01634", "0.80000", "0.01575", "0.01577", "148976.11", 1499644799999, "2434.19", 308, "1756.87", "28.46", "17928899.62"],
            [1499644800000, "0.01577", "0.01590", "0.01570", "0.01585", "250000.00", 1499731199999, "3950.00", 500, "2000.00", "31.70", "20000000.00"]
        ]"#;

        let klines: Vec<BinanceKline> = serde_json::from_str(json_str).unwrap();

        assert_eq!(klines.len(), 2);
        assert_eq!(klines[0].open_time, 1499040000000);
        assert_eq!(klines[1].open_time, 1499644800000);
        assert_eq!(klines[0].num_trades, 308);
        assert_eq!(klines[1].num_trades, 500);
    }

    #[test]
    fn test_binance_kline_serialize() {
        let kline = BinanceKline {
            open_time: 1499040000000,
            open: 0.01634,
            high: 0.8,
            low: 0.015758,
            close: 0.015771,
            volume: 148976.11427815,
            close_time: 1499644799999,
            quote_volume: 2434.19055334,
            num_trades: 308,
            taker_buy_base_volume: 1756.87402397,
            taker_buy_quote_volume: 28.46694368,
        };

        let json = serde_json::to_string(&kline).unwrap();
        assert!(json.contains("\"open_time\":1499040000000"));
        assert!(json.contains("\"num_trades\":308"));
    }
}
