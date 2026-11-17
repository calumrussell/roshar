//! Strict serde helpers for Polymarket's string-encoded numbers. A value that is present but does
//! not parse is an error, never a silent default.

use serde::{de, Deserialize, Deserializer};

#[derive(Deserialize)]
#[serde(untagged)]
enum StrOrNum {
    Str(String),
    Num(serde_json::Number),
}

fn parse_f64<E: de::Error>(v: StrOrNum) -> Result<f64, E> {
    match v {
        StrOrNum::Str(s) => s
            .parse::<f64>()
            .map_err(|_| E::custom(format!("failed to parse '{s}' as f64"))),
        StrOrNum::Num(n) => n
            .as_f64()
            .ok_or_else(|| E::custom(format!("failed to read {n} as f64"))),
    }
}

fn parse_u64<E: de::Error>(v: StrOrNum) -> Result<u64, E> {
    match v {
        StrOrNum::Str(s) => s
            .parse::<u64>()
            .map_err(|_| E::custom(format!("failed to parse '{s}' as u64"))),
        StrOrNum::Num(n) => n
            .as_u64()
            .ok_or_else(|| E::custom(format!("failed to read {n} as u64"))),
    }
}

/// Decimal sent as a JSON string (`"0.52"`) or number.
pub(crate) fn f64_from_str<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    parse_f64(StrOrNum::deserialize(d)?)
}

/// Optional decimal sent as a JSON string or number; `null`/absent is `None` (use with
/// `#[serde(default)]`).
pub(crate) fn opt_f64_from_str<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Option::<StrOrNum>::deserialize(d)?
        .map(parse_f64)
        .transpose()
}

/// Unsigned integer sent as a JSON string (`"1731740000000"`) or number.
pub(crate) fn u64_from_str<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
    parse_u64(StrOrNum::deserialize(d)?)
}

/// Optional decimal string where `""` means "no value" (e.g. a book's `last_trade_price` before
/// the first trade). Any other non-decimal string is an error.
pub(crate) fn opt_f64_from_str_empty_none<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Option<f64>, D::Error> {
    match Option::<StrOrNum>::deserialize(d)? {
        Some(StrOrNum::Str(s)) if s.is_empty() => Ok(None),
        v => v.map(parse_f64).transpose(),
    }
}

/// Key must be present, value may be `null`. Plain `Option<T>` fields default to `None` when the
/// key is missing; `#[serde(deserialize_with = "nullable")]` (without `default`) does not.
pub(crate) fn nullable<'de, D, T>(d: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct T {
        #[serde(deserialize_with = "f64_from_str")]
        a: f64,
        #[serde(default, deserialize_with = "opt_f64_from_str_empty_none")]
        b: Option<f64>,
        #[serde(default, deserialize_with = "u64_from_str")]
        c: u64,
    }

    #[test]
    fn strict_numbers() {
        let t: T = serde_json::from_str(r#"{"a":"0.5","b":"12","c":"7"}"#).unwrap();
        assert_eq!((t.a, t.b, t.c), (0.5, Some(12.0), 7));
        let t: T = serde_json::from_str(r#"{"a":0.25,"b":""}"#).unwrap();
        assert_eq!((t.a, t.b), (0.25, None));
        assert!(serde_json::from_str::<T>(r#"{"a":"x"}"#).is_err());
        assert!(serde_json::from_str::<T>(r#"{"a":"1","b":"x"}"#).is_err());
        assert!(serde_json::from_str::<T>(r#"{"a":"1","c":"-1"}"#).is_err());
        assert!(serde_json::from_str::<T>(r#"{"b":"1"}"#).is_err());
    }
}
