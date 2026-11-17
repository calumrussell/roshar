//! Decoder for Paradex SBE (Simple Binary Encoding) WebSocket frames.
//!
//! Paradex delivers public channel payloads (and, depending on the account,
//! the private `orders`/`fills`/`positions`/`account` payloads) as binary SBE
//! frames when the connection negotiates `?sbeSchemaId=1&sbeSchemaVersion=N`.
//! JSON-RPC control frames stay JSON; see [`crate::ws_control`].
//!
//! Schema: `schema/paradex_1_0.xml` in this crate, schema id 1, version 3
//! (released 2026-10-15), copied verbatim from paradex-py
//! `paradex_py/api/sbe/paradex_1_0.xml` at commit
//! `62c0c3187773abad80f0db9573c34654197b00da` (v0.9.0), sha256
//! `a66f2f48e5f43c6c032f4a2b3cf7b76aa6232375b3736f4c19671133dc8ef166`.
//!
//! Wire rules implemented here (all little-endian):
//! - 8-byte message header: blockLength, templateId, schemaId, version (u16 each).
//! - The root block is read from the header's blockLength. Fields appended in
//!   later schema versions (`sinceVersion`) are decoded only when the block is
//!   long enough to hold them; bytes beyond the known fields are skipped, so
//!   frames from newer schema versions still decode.
//! - Repeating groups: u16 blockLength + u16 numInGroup, entries advance by
//!   the group blockLength.
//! - Var-data: u8 length + UTF-8 bytes. Var-data appended in version 2 is read
//!   only for header version >= 2 and only when bytes remain (the schema
//!   documents two layouts of version 2 for TradeEvent and OrderEvent).
//! - For frames at a known version (<= [`SCHEMA_VERSION`]) trailing bytes after
//!   the last known var-data field are an error.
//!
//! Numeric fields are fixed-point int64 mantissas with a schema-constant
//! exponent (-8, or -12 for `Rate12`). They are exposed as [`Decimal`], whose
//! `Display` renders the exact value with all exponent digits
//! (e.g. `42000.50000000`), the same string paradex-py produces. The int64
//! null sentinel (`i64::MIN`) maps to `None` on fields the schema marks
//! `presence="optional"` and is an error on required fields.
//!
//! Timestamps are `i64` microseconds since the Unix epoch, as on the wire
//! (paradex-py divides them to milliseconds; this crate does not).

use anyhow::{anyhow, bail, Result};
use std::fmt;

/// Schema id this decoder understands.
pub const SCHEMA_ID: u16 = 1;
/// Highest schema version whose layout this decoder knows in full.
pub const SCHEMA_VERSION: u16 = 3;

const HEADER_LEN: usize = 8;
const NULL_I64: i64 = i64::MIN;

/// SBE message header that starts every frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageHeader {
    pub block_length: u16,
    pub template_id: u16,
    pub schema_id: u16,
    pub version: u16,
}

/// Exact fixed-point decimal: `mantissa * 10^exponent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Decimal {
    pub mantissa: i64,
    pub exponent: i8,
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.mantissa < 0 { "-" } else { "" };
        let abs = self.mantissa.unsigned_abs();
        if self.exponent >= 0 {
            let mul = 10u128.pow(self.exponent as u32);
            return write!(f, "{}{}", sign, abs as u128 * mul);
        }
        let scale = self.exponent.unsigned_abs() as u32;
        let div = 10u128.pow(scale);
        let abs = abs as u128;
        write!(
            f,
            "{}{}.{:0width$}",
            sign,
            abs / div,
            abs % div,
            width = scale as usize
        )
    }
}

/// Paradex account address: 32 bytes, big-endian Starknet field element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccountAddress(pub [u8; 32]);

impl fmt::Display for AccountAddress {
    /// `0x`-prefixed lower-case hex without leading zeros (`0x0` for zero).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("0x")?;
        let mut started = false;
        for b in self.0.iter() {
            if started {
                write!(f, "{:02x}", b)?;
            } else if *b != 0 {
                write!(f, "{:x}", b)?;
                started = true;
            }
        }
        if !started {
            f.write_str("0")?;
        }
        Ok(())
    }
}

macro_rules! sbe_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident = $value:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            fn from_u8(raw: u8) -> Result<Self> {
                match raw {
                    $($value => Ok($name::$variant),)+
                    other => Err(anyhow!(concat!("unknown ", stringify!($name), " value {}"), other)),
                }
            }
        }
    };
}

sbe_enum!(
    /// Order/trade side. On `PositionEvent`, `Buy` = long and `Sell` = short.
    Side { Buy = 1, Sell = 2, NonRepresentable = 254 }
);
sbe_enum!(OrderType {
    Limit = 1,
    Market = 2,
    StopLimit = 3,
    StopMarket = 4,
    TakeProfitLimit = 5,
    TakeProfitMarket = 6,
    StopLossLimit = 7,
    StopLossMarket = 8,
    NonRepresentable = 254,
});
sbe_enum!(OrderStatus {
    New = 1,
    Untriggered = 2,
    Open = 3,
    Closed = 4,
    NonRepresentable = 254,
});
sbe_enum!(TimeInForce {
    Gtc = 1,
    Ioc = 2,
    PostOnly = 3,
    Rpi = 4,
    NonRepresentable = 254,
});
sbe_enum!(FillType {
    Fill = 1,
    Liquidation = 2,
    UnwindTransfer = 3,
    SettleMarket = 4,
    Rpi = 5,
    BlockTrade = 6,
    NonRepresentable = 254,
});
sbe_enum!(
    /// Order book package type.
    PkgType { Snapshot = 0, Delta = 1 }
);
sbe_enum!(LiquiditySide {
    Maker = 1,
    Taker = 2,
    NonRepresentable = 254,
});
sbe_enum!(
    /// Self-trade prevention mode.
    StpMode { None = 0, ExpireMaker = 1, ExpireTaker = 2, ExpireBoth = 3, NonRepresentable = 254 }
);
sbe_enum!(PositionStatus {
    Open = 1,
    Closed = 2,
    Unspecified = 3,
    NonRepresentable = 254,
});
sbe_enum!(AccountStatus {
    Active = 1,
    Liquidation = 2,
    NonRepresentable = 254,
});
sbe_enum!(RequestStatus {
    Unspecified = 0,
    Pending = 1,
    Processed = 2,
    Rejected = 3,
    Success = 4,
    NonRepresentable = 254,
});
sbe_enum!(RequestType {
    Unspecified = 0,
    ModifyOrder = 1,
    NonRepresentable = 254,
});

/// Market asset kind. The schema requires decoders to map values they do not
/// know to `Unspecified` rather than fail, so this enum does exactly that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetKind {
    Unspecified,
    PerpetualFuture,
    Option,
    Spot,
}

impl AssetKind {
    fn from_u8(raw: u8) -> Self {
        match raw {
            1 => AssetKind::PerpetualFuture,
            2 => AssetKind::Option,
            3 => AssetKind::Spot,
            _ => AssetKind::Unspecified,
        }
    }
}

/// Funding-rate source venue. The schema appends venues as a non-breaking
/// change and requires decoders to map unknown values to `Unspecified`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FundingRateSource {
    Unspecified,
    Paradex,
    Binance,
    Bybit,
    Okx,
    Deribit,
    Derive,
    Hyperliquid,
    Lighter,
    Databento,
    Publisher,
    Pyth,
    Stork,
}

impl FundingRateSource {
    fn from_u8(raw: u8) -> Self {
        match raw {
            1 => FundingRateSource::Paradex,
            2 => FundingRateSource::Binance,
            3 => FundingRateSource::Bybit,
            4 => FundingRateSource::Okx,
            5 => FundingRateSource::Deribit,
            6 => FundingRateSource::Derive,
            7 => FundingRateSource::Hyperliquid,
            8 => FundingRateSource::Lighter,
            9 => FundingRateSource::Databento,
            10 => FundingRateSource::Publisher,
            11 => FundingRateSource::Pyth,
            12 => FundingRateSource::Stork,
            _ => FundingRateSource::Unspecified,
        }
    }
}

/// `OrderFlags` bitset. Bits the schema does not define are an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct OrderFlags {
    pub reduce_only: bool,
    pub stop_condition_below_trigger: bool,
    pub stop_condition_above_trigger: bool,
    pub interactive: bool,
    pub target_strategy_vwap: bool,
    /// Bit 5, schema version 2+.
    pub mmp: bool,
}

impl OrderFlags {
    fn from_u8(raw: u8) -> Result<Self> {
        if raw & 0b1100_0000 != 0 {
            bail!("unknown OrderFlags bits in {:#04x}", raw);
        }
        Ok(OrderFlags {
            reduce_only: raw & 1 != 0,
            stop_condition_below_trigger: raw & (1 << 1) != 0,
            stop_condition_above_trigger: raw & (1 << 2) != 0,
            interactive: raw & (1 << 3) != 0,
            target_strategy_vwap: raw & (1 << 4) != 0,
            mmp: raw & (1 << 5) != 0,
        })
    }
}

/// `FillFlags` bitset (schema version 2+). Bits 3-6 are undefined and are an
/// error; bit 7 (`unknown`) is the server's marker for a REST flag the schema
/// cannot name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct FillFlags {
    pub interactive: bool,
    pub rpi: bool,
    pub fastfill: bool,
    pub unknown: bool,
}

impl FillFlags {
    fn from_u8(raw: u8) -> Result<Self> {
        if raw & 0b0111_1000 != 0 {
            bail!("unknown FillFlags bits in {:#04x}", raw);
        }
        Ok(FillFlags {
            interactive: raw & 1 != 0,
            rpi: raw & (1 << 1) != 0,
            fastfill: raw & (1 << 2) != 0,
            unknown: raw & (1 << 7) != 0,
        })
    }
}

/// templateId 1, channel `trades.{market}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeEvent {
    /// Feed publish time, μs.
    pub ts: i64,
    /// Orderbook sequence after execution; 0 for block trade legs and
    /// position transfers.
    pub seq: i64,
    /// DEPRECATED by Paradex: low 64 bits of the real trade id, may be
    /// negative. Use `trade_id_str`.
    pub trade_id: i64,
    /// Aggressor side.
    pub side: Side,
    pub price: Decimal,
    pub size: Decimal,
    /// Matching engine time, μs.
    pub created_at: i64,
    pub trade_type: FillType,
    pub market: String,
    /// Full trade id (schema version 2+); `None` on frames that predate it.
    pub trade_id_str: Option<String>,
}

/// templateId 2, channel `bbo.{market}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BboEvent {
    pub ts: i64,
    pub seq: i64,
    pub bid_price: Option<Decimal>,
    pub bid_size: Option<Decimal>,
    pub ask_price: Option<Decimal>,
    pub ask_size: Option<Decimal>,
    pub market: String,
}

/// One order book level. In a delta, `size` zero removes the level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BookLevel {
    pub price: Decimal,
    pub size: Decimal,
}

/// templateId 3, channel `order_book.{market}.*` (snapshot and delta feeds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookEvent {
    pub ts: i64,
    pub seq: i64,
    pub pkg_type: PkgType,
    /// Interactive feeds only (including RPI).
    pub best_bid_price: Option<Decimal>,
    pub best_bid_size: Option<Decimal>,
    pub best_ask_price: Option<Decimal>,
    pub best_ask_size: Option<Decimal>,
    pub relative_spread: Option<Decimal>,
    /// Best prices from API orders (excluding RPI).
    pub best_bid_api_price: Option<Decimal>,
    pub best_bid_api_size: Option<Decimal>,
    pub best_ask_api_price: Option<Decimal>,
    pub best_ask_api_size: Option<Decimal>,
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
    pub market: String,
}

/// templateId 4, channel `markets_summary[.{market}]`.
///
/// Fields appended in later schema versions are `None` both when the frame
/// predates them and when the server sends the null sentinel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketSummaryEvent {
    pub ts: i64,
    pub seq: i64,
    pub mark_price: Decimal,
    /// JSON `underlying_price`.
    pub index_price: Decimal,
    /// JSON `last_traded_price`.
    pub last_price: Decimal,
    pub volume_usd_24h: Decimal,
    pub open_interest: Decimal,
    /// Truncated to 8dp; prefer `funding_rate_precise`.
    pub funding_rate: Decimal,
    pub bid: Option<Decimal>,
    pub ask: Option<Decimal>,
    /// Saturates at i64::MAX mantissa (a lower bound).
    pub total_volume: Decimal,
    pub price_change_rate_24h: Decimal,
    pub mark_iv: Option<Decimal>,
    pub bid_iv: Option<Decimal>,
    pub ask_iv: Option<Decimal>,
    pub last_iv: Option<Decimal>,
    pub delta: Option<Decimal>,
    pub gamma: Option<Decimal>,
    pub vega: Option<Decimal>,
    pub theta: Option<Decimal>,
    pub rho: Option<Decimal>,
    pub volga: Option<Decimal>,
    pub vanna: Option<Decimal>,
    pub future_funding_rate: Option<Decimal>,
    pub external_fair_price: Option<Decimal>,
    pub bid_size: Option<Decimal>,
    pub ask_size: Option<Decimal>,
    /// Schema version 1+.
    pub forward_rate: Option<Decimal>,
    /// Schema version 1+.
    pub risk_free_rate: Option<Decimal>,
    /// Schema version 1+, exponent -12.
    pub funding_rate_precise: Option<Decimal>,
    /// Schema version 3+.
    pub settlement_twap_estimate: Option<Decimal>,
    pub market: String,
}

/// templateId 5, channel `funding_data.{market}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FundingDataEvent {
    pub ts: i64,
    pub seq: i64,
    pub funding_rate: Decimal,
    pub funding_index: Decimal,
    pub created_at: i64,
    pub funding_premium: Decimal,
    pub funding_rate_8h: Decimal,
    pub funding_period_hours: i16,
    /// Schema version 1+; `None` when absent or null.
    pub impact_premium_rate: Option<Decimal>,
    pub market: String,
}

/// templateId 6, channel `funding_rate_comparison.ALL@{refresh_rate}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FundingRateComparisonEvent {
    pub ts: i64,
    pub last_updated_at: i64,
    /// Exponent -12.
    pub hourly_funding_rate: Decimal,
    pub asset_kind: AssetKind,
    pub source: FundingRateSource,
    /// JSON `symbol`.
    pub market: String,
}

/// templateId 20, private channel `orders.{market}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderEvent {
    pub ts: i64,
    pub seq: i64,
    pub status: OrderStatus,
    pub side: Side,
    pub order_type: OrderType,
    pub time_in_force: TimeInForce,
    pub price: Option<Decimal>,
    pub trigger_price: Option<Decimal>,
    pub size: Decimal,
    /// JSON `remaining_size`.
    pub size_open: Decimal,
    pub avg_fill_price: Option<Decimal>,
    pub created_at: i64,
    pub updated_at: i64,
    pub account: AccountAddress,
    pub received_at: i64,
    pub published_at: i64,
    pub stp: StpMode,
    pub flags: OrderFlags,
    /// Schema version 2+; `Unspecified` together with `request_type`
    /// `Unspecified` means JSON `request_info` was absent.
    pub request_status: Option<RequestStatus>,
    /// Schema version 2+.
    pub request_type: Option<RequestType>,
    pub order_id: String,
    pub client_order_id: String,
    pub market: String,
    /// Empty if not cancelled.
    pub cancel_reason: String,
    /// Schema version 2+; empty if no request.
    pub request_id: Option<String>,
    /// Schema version 2+; empty if no request.
    pub request_message: Option<String>,
}

/// templateId 21, private channel `fills.{market}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillEvent {
    pub ts: i64,
    /// `None` for position transfers.
    pub seq: Option<i64>,
    pub fill_type: FillType,
    pub side: Side,
    pub liquidity: LiquiditySide,
    pub price: Decimal,
    pub size: Decimal,
    pub fee: Decimal,
    pub realized_pnl: Option<Decimal>,
    pub created_at: i64,
    pub account: AccountAddress,
    pub underlying_price: Decimal,
    pub realized_funding: Decimal,
    /// Schema version 2+.
    pub flags: Option<FillFlags>,
    /// Schema version 2+; 0 for position transfers.
    pub orderbook_seq_no: Option<u64>,
    pub fill_id: String,
    pub order_id: String,
    pub client_order_id: String,
    pub trade_id: String,
    pub market: String,
    /// Schema version 2+.
    pub fee_currency: Option<String>,
}

/// templateId 22, private channel `positions`.
///
/// `size` is the absolute size as on the wire; the direction is `side`
/// (`Buy` = long, `Sell` = short). The JSON channel signs the size instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionEvent {
    pub ts: i64,
    pub seq: i64,
    pub side: Side,
    pub size: Decimal,
    pub avg_entry_price: Decimal,
    pub unrealized_pnl: Decimal,
    pub realized_pnl: Decimal,
    /// Always null per the schema.
    pub mark_price: Option<Decimal>,
    pub liquidation_price: Option<Decimal>,
    pub leverage: Decimal,
    pub updated_at: i64,
    pub account: AccountAddress,
    pub avg_entry_price_usd: Decimal,
    pub cost: Decimal,
    pub cost_usd: Decimal,
    pub cached_funding_index: Decimal,
    pub unrealized_funding_pnl: Decimal,
    pub status: PositionStatus,
    pub market: String,
    pub last_fill_id: String,
}

/// templateId 23, private channel `account`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountEvent {
    pub ts: i64,
    pub seq: i64,
    pub total_collateral: Decimal,
    pub free_collateral: Decimal,
    pub initial_margin_req: Decimal,
    pub maintenance_margin_req: Decimal,
    pub account_value: Decimal,
    /// Always null per the schema.
    pub unrealized_pnl: Option<Decimal>,
    pub updated_at: i64,
    pub account: AccountAddress,
    pub margin_cushion: Decimal,
    pub status: AccountStatus,
    /// Schema version 2+, μs; 0 = never acknowledged.
    pub last_seen_notification: Option<i64>,
    pub settlement_asset: String,
}

/// templateId 40.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeartbeatEvent {
    pub ts: i64,
    pub seq: i64,
}

/// templateId 41 (reserved by Paradex; acks currently arrive as JSON).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubscribedEvent {
    pub ts: i64,
    pub seq: i64,
    /// 0 = ok, 1 = error.
    pub status: u8,
    pub channel: String,
}

/// A decoded Paradex SBE frame, one variant per schema template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParadexSbeMessage {
    Trade(TradeEvent),
    Bbo(BboEvent),
    Book(BookEvent),
    MarketSummary(MarketSummaryEvent),
    FundingData(FundingDataEvent),
    FundingRateComparison(FundingRateComparisonEvent),
    Order(OrderEvent),
    Fill(FillEvent),
    Position(PositionEvent),
    Account(AccountEvent),
    Heartbeat(HeartbeatEvent),
    Subscribed(SubscribedEvent),
}

/// Bounds-checked little-endian cursor.
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    fn take(&mut self, n: usize, what: &str) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| anyhow!("SBE frame truncated reading {}", what))?;
        let out = self.buf.get(self.pos..end).ok_or_else(|| {
            anyhow!(
                "SBE frame truncated reading {}: need {} bytes at offset {}, have {}",
                what,
                n,
                self.pos,
                self.buf.len()
            )
        })?;
        self.pos = end;
        Ok(out)
    }

    fn array<const N: usize>(&mut self, what: &str) -> Result<[u8; N]> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N, what)?);
        Ok(out)
    }

    fn u8(&mut self, what: &str) -> Result<u8> {
        Ok(self.take(1, what)?[0])
    }

    fn u16(&mut self, what: &str) -> Result<u16> {
        Ok(u16::from_le_bytes(self.array(what)?))
    }

    fn i16(&mut self, what: &str) -> Result<i16> {
        Ok(i16::from_le_bytes(self.array(what)?))
    }

    fn i64(&mut self, what: &str) -> Result<i64> {
        Ok(i64::from_le_bytes(self.array(what)?))
    }

    fn u64(&mut self, what: &str) -> Result<u64> {
        Ok(u64::from_le_bytes(self.array(what)?))
    }

    /// Required fixed-point field; the null sentinel is an error.
    fn dec(&mut self, exponent: i8, what: &str) -> Result<Decimal> {
        let mantissa = self.i64(what)?;
        if mantissa == NULL_I64 {
            bail!("SBE required field {} is null", what);
        }
        Ok(Decimal { mantissa, exponent })
    }

    /// Optional fixed-point field (`presence="optional"`).
    fn dec_opt(&mut self, exponent: i8, what: &str) -> Result<Option<Decimal>> {
        let mantissa = self.i64(what)?;
        Ok((mantissa != NULL_I64).then_some(Decimal { mantissa, exponent }))
    }

    fn d8(&mut self, what: &str) -> Result<Decimal> {
        self.dec(-8, what)
    }

    fn d8_opt(&mut self, what: &str) -> Result<Option<Decimal>> {
        self.dec_opt(-8, what)
    }

    fn address(&mut self, what: &str) -> Result<AccountAddress> {
        Ok(AccountAddress(self.array(what)?))
    }

    /// `varString8`: u8 length + UTF-8 bytes.
    fn var_string(&mut self, what: &str) -> Result<String> {
        let len = self.u8(what)? as usize;
        let bytes = self.take(len, what)?;
        let s = std::str::from_utf8(bytes)
            .map_err(|e| anyhow!("SBE field {} is not UTF-8: {}", what, e))?;
        Ok(s.to_string())
    }

    /// Var-data appended in `since` version: read only when the frame's
    /// version includes it and bytes remain.
    fn var_string_since(&mut self, version: u16, since: u16, what: &str) -> Result<Option<String>> {
        if version >= since && self.remaining() > 0 {
            Ok(Some(self.var_string(what)?))
        } else {
            Ok(None)
        }
    }
}

/// Read the 8-byte message header.
pub fn decode_header(frame: &[u8]) -> Result<MessageHeader> {
    let mut r = Reader::new(frame);
    Ok(MessageHeader {
        block_length: r.u16("header.blockLength")?,
        template_id: r.u16("header.templateId")?,
        schema_id: r.u16("header.schemaId")?,
        version: r.u16("header.version")?,
    })
}

/// Decode one binary WebSocket frame.
///
/// Errors on an unknown schema id or template id, a frame shorter than its
/// header and advertised block, a block shorter than the template's base
/// layout, a null sentinel in a required field, an unknown enum value, or
/// (for known versions) trailing bytes.
pub fn decode(frame: &[u8]) -> Result<ParadexSbeMessage> {
    let header = decode_header(frame)?;
    if header.schema_id != SCHEMA_ID {
        bail!(
            "unsupported SBE schemaId {}, expected {}",
            header.schema_id,
            SCHEMA_ID
        );
    }
    let mut body = Reader::new(&frame[HEADER_LEN..]);
    let mut block = Reader::new(body.take(header.block_length as usize, "root block")?);
    let v = header.version;
    let b = &mut block;
    let r = &mut body;
    let msg = match header.template_id {
        1 => ParadexSbeMessage::Trade(TradeEvent {
            ts: b.i64("ts")?,
            seq: b.i64("seq")?,
            trade_id: b.i64("tradeId")?,
            side: Side::from_u8(b.u8("side")?)?,
            price: b.d8("price")?,
            size: b.d8("size")?,
            created_at: b.i64("createdAt")?,
            trade_type: FillType::from_u8(b.u8("tradeType")?)?,
            market: r.var_string("market")?,
            trade_id_str: r.var_string_since(v, 2, "tradeIdStr")?,
        }),
        2 => ParadexSbeMessage::Bbo(BboEvent {
            ts: b.i64("ts")?,
            seq: b.i64("seq")?,
            bid_price: b.d8_opt("bidPrice")?,
            bid_size: b.d8_opt("bidSize")?,
            ask_price: b.d8_opt("askPrice")?,
            ask_size: b.d8_opt("askSize")?,
            market: r.var_string("market")?,
        }),
        3 => ParadexSbeMessage::Book(BookEvent {
            ts: b.i64("ts")?,
            seq: b.i64("seq")?,
            pkg_type: PkgType::from_u8(b.u8("pkgType")?)?,
            best_bid_price: b.d8_opt("bestBidPrice")?,
            best_bid_size: b.d8_opt("bestBidSize")?,
            best_ask_price: b.d8_opt("bestAskPrice")?,
            best_ask_size: b.d8_opt("bestAskSize")?,
            relative_spread: b.d8_opt("relativeSpread")?,
            best_bid_api_price: b.d8_opt("bestBidApiPrice")?,
            best_bid_api_size: b.d8_opt("bestBidApiSize")?,
            best_ask_api_price: b.d8_opt("bestAskApiPrice")?,
            best_ask_api_size: b.d8_opt("bestAskApiSize")?,
            bids: book_levels(r, "bids")?,
            asks: book_levels(r, "asks")?,
            market: r.var_string("market")?,
        }),
        4 => ParadexSbeMessage::MarketSummary(MarketSummaryEvent {
            ts: b.i64("ts")?,
            seq: b.i64("seq")?,
            mark_price: b.d8("markPrice")?,
            index_price: b.d8("indexPrice")?,
            last_price: b.d8("lastPrice")?,
            volume_usd_24h: b.d8("volumeUsd24h")?,
            open_interest: b.d8("openInterest")?,
            funding_rate: b.d8("fundingRate")?,
            bid: b.d8_opt("bid")?,
            ask: b.d8_opt("ask")?,
            total_volume: b.d8("totalVolume")?,
            price_change_rate_24h: b.d8("priceChangeRate24h")?,
            mark_iv: b.d8_opt("markIv")?,
            bid_iv: b.d8_opt("bidIv")?,
            ask_iv: b.d8_opt("askIv")?,
            last_iv: b.d8_opt("lastIv")?,
            delta: b.d8_opt("delta")?,
            gamma: b.d8_opt("gamma")?,
            vega: b.d8_opt("vega")?,
            theta: b.d8_opt("theta")?,
            rho: b.d8_opt("rho")?,
            volga: b.d8_opt("volga")?,
            vanna: b.d8_opt("vanna")?,
            future_funding_rate: b.d8_opt("futureFundingRate")?,
            external_fair_price: b.d8_opt("externalFairPrice")?,
            bid_size: b.d8_opt("bidSize")?,
            ask_size: b.d8_opt("askSize")?,
            // sinceVersion=1 fields occupy 24 bytes together.
            forward_rate: appended(b, 24, |b| b.d8_opt("forwardRate"))?.flatten(),
            risk_free_rate: appended(b, 16, |b| b.d8_opt("riskFreeRate"))?.flatten(),
            funding_rate_precise: appended(b, 8, |b| b.dec_opt(-12, "fundingRatePrecise"))?
                .flatten(),
            settlement_twap_estimate: appended(b, 8, |b| b.d8_opt("settlementTwapEstimate"))?
                .flatten(),
            market: r.var_string("market")?,
        }),
        5 => ParadexSbeMessage::FundingData(FundingDataEvent {
            ts: b.i64("ts")?,
            seq: b.i64("seq")?,
            funding_rate: b.d8("fundingRate")?,
            funding_index: b.d8("fundingIndex")?,
            created_at: b.i64("createdAt")?,
            funding_premium: b.d8("fundingPremium")?,
            funding_rate_8h: b.d8("fundingRate8h")?,
            funding_period_hours: b.i16("fundingPeriodHours")?,
            impact_premium_rate: appended(b, 8, |b| b.d8_opt("impactPremiumRate"))?.flatten(),
            market: r.var_string("market")?,
        }),
        6 => ParadexSbeMessage::FundingRateComparison(FundingRateComparisonEvent {
            ts: b.i64("ts")?,
            last_updated_at: b.i64("lastUpdatedAt")?,
            hourly_funding_rate: b.dec(-12, "hourlyFundingRate")?,
            asset_kind: AssetKind::from_u8(b.u8("assetKind")?),
            source: FundingRateSource::from_u8(b.u8("source")?),
            market: r.var_string("market")?,
        }),
        20 => {
            let ts = b.i64("ts")?;
            let seq = b.i64("seq")?;
            let status = OrderStatus::from_u8(b.u8("status")?)?;
            let side = Side::from_u8(b.u8("side")?)?;
            let order_type = OrderType::from_u8(b.u8("orderType")?)?;
            let time_in_force = TimeInForce::from_u8(b.u8("timeInForce")?)?;
            let price = b.d8_opt("price")?;
            let trigger_price = b.d8_opt("triggerPrice")?;
            let size = b.d8("size")?;
            let size_open = b.d8("sizeOpen")?;
            let avg_fill_price = b.d8_opt("avgFillPrice")?;
            let created_at = b.i64("createdAt")?;
            let updated_at = b.i64("updatedAt")?;
            let account = b.address("account")?;
            let received_at = b.i64("receivedAt")?;
            let published_at = b.i64("publishedAt")?;
            let stp = StpMode::from_u8(b.u8("stp")?)?;
            let flags = OrderFlags::from_u8(b.u8("flags")?)?;
            // requestStatus + requestType (sinceVersion=2) occupy 2 bytes.
            let request = appended(b, 2, |b| {
                Ok((
                    RequestStatus::from_u8(b.u8("requestStatus")?)?,
                    RequestType::from_u8(b.u8("requestType")?)?,
                ))
            })?;
            ParadexSbeMessage::Order(OrderEvent {
                ts,
                seq,
                status,
                side,
                order_type,
                time_in_force,
                price,
                trigger_price,
                size,
                size_open,
                avg_fill_price,
                created_at,
                updated_at,
                account,
                received_at,
                published_at,
                stp,
                flags,
                request_status: request.map(|(s, _)| s),
                request_type: request.map(|(_, t)| t),
                order_id: r.var_string("orderId")?,
                client_order_id: r.var_string("clientOrderId")?,
                market: r.var_string("market")?,
                cancel_reason: r.var_string("cancelReason")?,
                request_id: r.var_string_since(v, 2, "requestId")?,
                request_message: r.var_string_since(v, 2, "requestMessage")?,
            })
        }
        21 => {
            let ts = b.i64("ts")?;
            let seq = b.i64("seq")?;
            let fill_type = FillType::from_u8(b.u8("fillType")?)?;
            let side = Side::from_u8(b.u8("side")?)?;
            let liquidity = LiquiditySide::from_u8(b.u8("liquidity")?)?;
            let price = b.d8("price")?;
            let size = b.d8("size")?;
            let fee = b.d8("fee")?;
            let realized_pnl = b.d8_opt("realizedPnl")?;
            let created_at = b.i64("createdAt")?;
            let account = b.address("account")?;
            let underlying_price = b.d8("underlyingPrice")?;
            let realized_funding = b.d8("realizedFunding")?;
            // flags (u8) + orderbookSeqNo (u64), sinceVersion=2.
            let appended_v2 = appended(b, 9, |b| {
                Ok((
                    FillFlags::from_u8(b.u8("flags")?)?,
                    b.u64("orderbookSeqNo")?,
                ))
            })?;
            ParadexSbeMessage::Fill(FillEvent {
                ts,
                seq: (seq != NULL_I64).then_some(seq),
                fill_type,
                side,
                liquidity,
                price,
                size,
                fee,
                realized_pnl,
                created_at,
                account,
                underlying_price,
                realized_funding,
                flags: appended_v2.map(|(f, _)| f),
                orderbook_seq_no: appended_v2.map(|(_, s)| s),
                fill_id: r.var_string("fillId")?,
                order_id: r.var_string("orderId")?,
                client_order_id: r.var_string("clientOrderId")?,
                trade_id: r.var_string("tradeId")?,
                market: r.var_string("market")?,
                fee_currency: r.var_string_since(v, 2, "feeCurrency")?,
            })
        }
        22 => ParadexSbeMessage::Position(PositionEvent {
            ts: b.i64("ts")?,
            seq: b.i64("seq")?,
            side: Side::from_u8(b.u8("side")?)?,
            size: b.d8("size")?,
            avg_entry_price: b.d8("avgEntryPrice")?,
            unrealized_pnl: b.d8("unrealizedPnl")?,
            realized_pnl: b.d8("realizedPnl")?,
            mark_price: b.d8_opt("markPrice")?,
            liquidation_price: b.d8_opt("liquidationPrice")?,
            leverage: b.d8("leverage")?,
            updated_at: b.i64("updatedAt")?,
            account: b.address("account")?,
            avg_entry_price_usd: b.d8("avgEntryPriceUsd")?,
            cost: b.d8("cost")?,
            cost_usd: b.d8("costUsd")?,
            cached_funding_index: b.d8("cachedFundingIndex")?,
            unrealized_funding_pnl: b.d8("unrealizedFundingPnl")?,
            status: PositionStatus::from_u8(b.u8("status")?)?,
            market: r.var_string("market")?,
            last_fill_id: r.var_string("lastFillId")?,
        }),
        23 => ParadexSbeMessage::Account(AccountEvent {
            ts: b.i64("ts")?,
            seq: b.i64("seq")?,
            total_collateral: b.d8("totalCollateral")?,
            free_collateral: b.d8("freeCollateral")?,
            initial_margin_req: b.d8("initialMarginReq")?,
            maintenance_margin_req: b.d8("maintenanceMarginReq")?,
            account_value: b.d8("accountValue")?,
            unrealized_pnl: b.d8_opt("unrealizedPnl")?,
            updated_at: b.i64("updatedAt")?,
            account: b.address("account")?,
            margin_cushion: b.d8("marginCushion")?,
            status: AccountStatus::from_u8(b.u8("status")?)?,
            last_seen_notification: appended(b, 8, |b| b.i64("lastSeenNotification"))?,
            settlement_asset: r.var_string("settlementAsset")?,
        }),
        40 => ParadexSbeMessage::Heartbeat(HeartbeatEvent {
            ts: b.i64("ts")?,
            seq: b.i64("seq")?,
        }),
        41 => ParadexSbeMessage::Subscribed(SubscribedEvent {
            ts: b.i64("ts")?,
            seq: b.i64("seq")?,
            status: b.u8("status")?,
            channel: r.var_string("channel")?,
        }),
        other => bail!("unknown SBE templateId {}", other),
    };
    if v <= SCHEMA_VERSION && r.remaining() != 0 {
        bail!(
            "SBE frame templateId {} version {} has {} trailing bytes",
            header.template_id,
            v,
            r.remaining()
        );
    }
    Ok(msg)
}

/// Decode fields appended in a later schema version: present only when the
/// root block still holds `len` bytes at the current offset.
fn appended<'a, T>(
    block: &mut Reader<'a>,
    len: usize,
    read: impl FnOnce(&mut Reader<'a>) -> Result<T>,
) -> Result<Option<T>> {
    if block.remaining() >= len {
        read(block).map(Some)
    } else {
        Ok(None)
    }
}

/// Repeating group of `{price: Price8, size: Qty8}` entries.
fn book_levels(r: &mut Reader<'_>, what: &str) -> Result<Vec<BookLevel>> {
    let entry_len = r.u16(what)? as usize;
    let count = r.u16(what)? as usize;
    if entry_len < 16 {
        bail!("SBE group {} blockLength {} below 16", what, entry_len);
    }
    let mut levels = Vec::with_capacity(count);
    for _ in 0..count {
        let mut entry = Reader::new(r.take(entry_len, what)?);
        levels.push(BookLevel {
            price: entry.d8("price")?,
            size: entry.d8("size")?,
        });
    }
    Ok(levels)
}
