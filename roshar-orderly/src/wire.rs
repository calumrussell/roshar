//! Request encoding shared by signing and transport.
//!
//! Orderly signs `{timestamp}{METHOD}{path?query}{body}` and checks it against the bytes it
//! receives, so the crate builds the path and body text once and both signs and sends that text.
//! Bodies are written the way the official Python SDK writes them (`json.dumps` defaults: `", "`
//! and `": "` separators, ASCII escaping, insertion order), so signatures are byte-identical to
//! the SDK's.

use anyhow::{bail, Result};
use std::fmt::Write as _;

/// Checks that `s` is a JSON number literal (RFC 8259 grammar). Prices and sizes are passed in as
/// decimal text and written verbatim as JSON numbers, so the caller controls precision.
pub(crate) fn check_number(field: &str, s: &str) -> Result<()> {
    let b = s.as_bytes();
    let mut i = 0;
    if b.get(i) == Some(&b'-') {
        i += 1;
    }
    match b.get(i) {
        Some(b'0') => i += 1,
        Some(c) if c.is_ascii_digit() => {
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }
        _ => bail!("{field}: {s:?} is not a JSON number"),
    }
    if b.get(i) == Some(&b'.') {
        i += 1;
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            bail!("{field}: {s:?} is not a JSON number");
        }
    }
    if matches!(b.get(i), Some(b'e') | Some(b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+') | Some(b'-')) {
            i += 1;
        }
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        if i == start {
            bail!("{field}: {s:?} is not a JSON number");
        }
    }
    if i != b.len() {
        bail!("{field}: {s:?} is not a JSON number");
    }
    Ok(())
}

/// Appends `s` as a JSON string with Python `json.dumps(ensure_ascii=True)` escaping.
pub(crate) fn push_py_str(buf: &mut String, s: &str) {
    buf.push('"');
    for c in s.chars() {
        match c {
            '"' => buf.push_str("\\\""),
            '\\' => buf.push_str("\\\\"),
            '\n' => buf.push_str("\\n"),
            '\r' => buf.push_str("\\r"),
            '\t' => buf.push_str("\\t"),
            '\u{8}' => buf.push_str("\\b"),
            '\u{c}' => buf.push_str("\\f"),
            ' '..='~' => buf.push(c),
            _ => {
                let mut units = [0u16; 2];
                for u in c.encode_utf16(&mut units) {
                    let _ = write!(buf, "\\u{u:04x}");
                }
            }
        }
    }
    buf.push('"');
}

/// A JSON object written in Python `json.dumps` default style, keys in insertion order.
pub(crate) struct PyObject {
    buf: String,
    empty: bool,
}

impl PyObject {
    pub(crate) fn new() -> Self {
        Self {
            buf: String::from("{"),
            empty: true,
        }
    }

    fn key(&mut self, key: &str) {
        if !self.empty {
            self.buf.push_str(", ");
        }
        self.empty = false;
        push_py_str(&mut self.buf, key);
        self.buf.push_str(": ");
    }

    pub(crate) fn str(&mut self, key: &str, value: &str) -> &mut Self {
        self.key(key);
        push_py_str(&mut self.buf, value);
        self
    }

    pub(crate) fn opt_str(&mut self, key: &str, value: Option<&str>) -> &mut Self {
        if let Some(v) = value {
            self.str(key, v);
        }
        self
    }

    /// Writes an already-encoded JSON value (number literal, nested object or array).
    pub(crate) fn raw(&mut self, key: &str, value: &str) -> &mut Self {
        self.key(key);
        self.buf.push_str(value);
        self
    }

    /// Writes a decimal literal as a JSON number after checking it.
    pub(crate) fn number(&mut self, key: &str, value: &str) -> Result<&mut Self> {
        check_number(key, value)?;
        Ok(self.raw(key, value))
    }

    pub(crate) fn opt_number(&mut self, key: &str, value: Option<&str>) -> Result<&mut Self> {
        if let Some(v) = value {
            self.number(key, v)?;
        }
        Ok(self)
    }

    pub(crate) fn uint(&mut self, key: &str, value: u64) -> &mut Self {
        self.key(key);
        let _ = write!(self.buf, "{value}");
        self
    }

    pub(crate) fn bool(&mut self, key: &str, value: bool) -> &mut Self {
        self.key(key);
        self.buf.push_str(if value { "true" } else { "false" });
        self
    }

    pub(crate) fn opt_bool(&mut self, key: &str, value: Option<bool>) -> &mut Self {
        if let Some(v) = value {
            self.bool(key, v);
        }
        self
    }

    pub(crate) fn finish(mut self) -> String {
        self.buf.push('}');
        self.buf
    }
}

/// Python `json.dumps` of a list of already-encoded values.
pub(crate) fn py_array(items: &[String]) -> String {
    let mut out = String::from("[");
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(item);
    }
    out.push(']');
    out
}

/// Rejects text that would change meaning, or be re-encoded by the HTTP stack, inside a URL path
/// segment or query value. The crate does not percent-encode: the signed path must be the exact
/// path sent.
fn check_url_text(field: &str, value: &str, allow_slash: bool) -> Result<()> {
    if value.is_empty() {
        bail!("{field}: empty value");
    }
    for c in value.chars() {
        let ok = c.is_ascii_graphic()
            && !matches!(c, '&' | '=' | '#' | '?' | '%' | '+')
            && (allow_slash || c != '/');
        if !ok {
            bail!("{field}: {value:?} contains {c:?}, which cannot be sent unencoded in a URL");
        }
    }
    Ok(())
}

pub(crate) fn path_segment(field: &str, value: &str) -> Result<()> {
    check_url_text(field, value, false)
}

/// Builds `path?k=v&k=v` in call order, skipping unset parameters (the SDK's `cleanNoneValue`).
pub(crate) struct PathQuery {
    url: String,
    has_params: bool,
}

impl PathQuery {
    pub(crate) fn new(path: impl Into<String>) -> Self {
        Self {
            url: path.into(),
            has_params: false,
        }
    }

    pub(crate) fn param(mut self, key: &str, value: impl std::fmt::Display) -> Result<Self> {
        let value = value.to_string();
        check_url_text(key, &value, true)?;
        self.url.push(if self.has_params { '&' } else { '?' });
        self.url.push_str(key);
        self.url.push('=');
        self.url.push_str(&value);
        self.has_params = true;
        Ok(self)
    }

    pub(crate) fn opt(self, key: &str, value: Option<impl std::fmt::Display>) -> Result<Self> {
        match value {
            Some(v) => self.param(key, v),
            None => Ok(self),
        }
    }

    pub(crate) fn finish(self) -> String {
        self.url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        for ok in [
            "0", "1", "-1", "65000.5", "1e-05", "0.00001", "1E+3", "12.0",
        ] {
            check_number("x", ok).unwrap();
        }
        for bad in [
            "", "-", "01", "1.", ".5", "1e", "NaN", "inf", "1,5", " 1", "0x10",
        ] {
            assert!(check_number("x", bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn python_escaping() {
        let mut s = String::new();
        push_py_str(&mut s, "a\"b\\c\n\u{7f}é😀");
        assert_eq!(s, "\"a\\\"b\\\\c\\n\\u007f\\u00e9\\ud83d\\ude00\"");
    }

    #[test]
    fn object_layout() {
        let mut o = PyObject::new();
        o.str("a", "x").uint("b", 1).bool("c", false);
        assert_eq!(o.finish(), r#"{"a": "x", "b": 1, "c": false}"#);
    }

    #[test]
    fn query_rejects_reserved() {
        assert!(PathQuery::new("/v1/x").param("s", "a&b").is_err());
        assert!(PathQuery::new("/v1/x").param("s", "a b").is_err());
        let q = PathQuery::new("/v1/x")
            .param("ids", "1,2")
            .unwrap()
            .opt("n", None::<u32>)
            .unwrap()
            .finish();
        assert_eq!(q, "/v1/x?ids=1,2");
    }
}
