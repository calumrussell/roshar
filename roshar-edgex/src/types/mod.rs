//! Wire types for edgeX V2, named and shaped as the exchange returns them. Numbers the exchange
//! sends as strings stay strings.

mod market;
mod trading;
mod ws;

pub use market::*;
pub use trading::*;
pub use ws::*;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Every REST reply: `{"code","data","msg","errorParam","requestTime","responseTime","traceId"}`.
/// `code` is `"SUCCESS"` or an error code; `data` is `null` on errors and on void endpoints.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexResponse<T> {
    pub code: String,
    pub data: Option<T>,
    pub msg: Option<String>,
    pub error_param: Option<Value>,
    pub request_time: String,
    pub response_time: String,
    pub trace_id: String,
}

pub const EDGEX_SUCCESS: &str = "SUCCESS";

impl<T> EdgexResponse<T> {
    /// `Ok(data)` when `code == "SUCCESS"`, else the exchange's error. `data` may be `None` on
    /// success only for void endpoints.
    pub fn into_result(self) -> Result<Option<T>, EdgexApiError> {
        if self.code == EDGEX_SUCCESS {
            Ok(self.data)
        } else {
            Err(EdgexApiError {
                code: self.code,
                msg: self.msg,
                error_param: self.error_param,
                trace_id: self.trace_id,
            })
        }
    }
}

/// An error envelope (`code != "SUCCESS"`), e.g. `INVALID_API_KEY`, `INVALID_DEPTH_LEVEL`,
/// `GATEWAY_HEADER_REQUIRED`.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgexApiError {
    pub code: String,
    pub msg: Option<String>,
    pub error_param: Option<Value>,
    pub trace_id: String,
}

impl std::fmt::Display for EdgexApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "edgeX error {}", self.code)?;
        if let Some(msg) = &self.msg {
            write!(f, ": {msg}")?;
        }
        if let Some(p) = &self.error_param {
            write!(f, " ({p})")?;
        }
        Ok(())
    }
}

impl std::error::Error for EdgexApiError {}

/// Cursor page: `{"dataList": [...], "nextPageOffsetData": ""}`; an empty offset is the last page.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgexPage<T> {
    pub data_list: Vec<T>,
    pub next_page_offset_data: String,
}

macro_rules! wire_enum {
    ($(#[$m:meta])* $name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum $name {
            $(#[serde(rename = $wire)] $variant),+
        }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $wire),+
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }
    };
}

wire_enum!(
    /// `klineType` (REST) / interval (WS channel).
    EdgexKlineType {
        Minute1 => "MINUTE_1",
        Minute5 => "MINUTE_5",
        Minute15 => "MINUTE_15",
        Minute30 => "MINUTE_30",
        Hour1 => "HOUR_1",
        Hour2 => "HOUR_2",
        Hour4 => "HOUR_4",
        Hour6 => "HOUR_6",
        Hour8 => "HOUR_8",
        Hour12 => "HOUR_12",
        Day1 => "DAY_1",
        Week1 => "WEEK_1",
        Month1 => "MONTH_1",
    }
);

wire_enum!(
    /// `priceType` of a K-line.
    EdgexPriceType {
        OraclePrice => "ORACLE_PRICE",
        IndexPrice => "INDEX_PRICE",
        LastPrice => "LAST_PRICE",
        Ask1Price => "ASK1_PRICE",
        Bid1Price => "BID1_PRICE",
        OpenInterest => "OPEN_INTEREST",
    }
);

wire_enum!(EdgexSide { Buy => "BUY", Sell => "SELL" });

wire_enum!(EdgexOrderType {
    Limit => "LIMIT",
    Market => "MARKET",
    StopLimit => "STOP_LIMIT",
    StopMarket => "STOP_MARKET",
    TakeProfitLimit => "TAKE_PROFIT_LIMIT",
    TakeProfitMarket => "TAKE_PROFIT_MARKET",
});

wire_enum!(
    /// `POST_ONLY` is the maker-only (ALO) flag. Market orders must be `IMMEDIATE_OR_CANCEL`.
    EdgexTimeInForce {
        GoodTilCancel => "GOOD_TIL_CANCEL",
        FillOrKill => "FILL_OR_KILL",
        ImmediateOrCancel => "IMMEDIATE_OR_CANCEL",
        PostOnly => "POST_ONLY",
    }
);

wire_enum!(EdgexTriggerPriceType {
    LastPrice => "LAST_PRICE",
    IndexPrice => "INDEX_PRICE",
    OraclePrice => "ORACLE_PRICE",
    Ask1Price => "ASK1_PRICE",
    Bid1Price => "BID1_PRICE",
});

wire_enum!(
    /// Order status filter values.
    EdgexOrderStatus {
        Pending => "PENDING",
        Open => "OPEN",
        Filled => "FILLED",
        Canceling => "CANCELING",
        Canceled => "CANCELED",
        Untriggered => "UNTRIGGERED",
    }
);

/// `getDepth` / `depth.{contractId}.{level}`: the gateway serves only 15 and 200 levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgexDepthLevel {
    L15,
    L200,
}

impl EdgexDepthLevel {
    pub fn as_u32(self) -> u32 {
        match self {
            Self::L15 => 15,
            Self::L200 => 200,
        }
    }
}

/// `setMarginMode` `marginMode`: `0` cross, `1` isolated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgexMarginMode {
    Cross,
    Isolated,
}

impl EdgexMarginMode {
    pub fn as_u8(self) -> u8 {
        match self {
            Self::Cross => 0,
            Self::Isolated => 1,
        }
    }
}
