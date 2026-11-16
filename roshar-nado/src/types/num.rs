//! Integers Nado sends as JSON strings (x18 fixed-point values, nonces, timestamps, appendices).
//!
//! Each newtype accepts only a JSON string holding a base-10 integer that fits the type, and
//! serializes back to the same string form.

use serde::de::{self, Deserializer, Visitor};
use serde::{Deserialize, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

macro_rules! string_int {
    ($(#[$doc:meta])* $name:ident, $inner:ty) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
        pub struct $name(pub $inner);

        impl $name {
            pub fn get(self) -> $inner {
                self.0
            }
        }

        impl From<$inner> for $name {
            fn from(v: $inner) -> Self {
                Self(v)
            }
        }

        impl From<$name> for $inner {
            fn from(v: $name) -> Self {
                v.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = std::num::ParseIntError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                s.parse::<$inner>().map(Self)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct V;
                impl<'de> Visitor<'de> for V {
                    type Value = $name;
                    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                        f.write_str(concat!("a string holding a ", stringify!($inner)))
                    }
                    fn visit_str<E: de::Error>(self, v: &str) -> Result<$name, E> {
                        v.parse::<$inner>().map($name).map_err(|e| {
                            E::custom(format_args!(
                                concat!("invalid ", stringify!($inner), " string {:?}: {}"),
                                v, e
                            ))
                        })
                    }
                }
                d.deserialize_str(V)
            }
        }
    };
}

string_int!(
    /// Signed 128-bit integer sent as a string: x18 prices, amounts, balances, fees, rates.
    I128,
    i128
);
string_int!(
    /// Unsigned 64-bit integer sent as a string: nonces, expirations, timestamps, submission
    /// indices.
    U64,
    u64
);
string_int!(
    /// Unsigned 128-bit integer sent as a string: the order appendix.
    U128,
    u128
);
