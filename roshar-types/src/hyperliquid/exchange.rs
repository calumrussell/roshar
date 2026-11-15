//! Hyperliquid exchange (L1 action) wire types and the parsed `l2Book` snapshot.
//! Field layout and hashing order are fixed by the exchange: see roshar-hyperliquid's signer.

/// Hyperliquid book side: "B" (bid) / "A" (ask).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Side {
    Bid,
    Ask,
}

#[derive(Clone, Copy, Debug)]
pub struct Level {
    pub px: f64,
    pub sz: f64,
    pub n: u32,
}

/// Hyperliquid info `l2Book`. `time_ms` equals the block_time (ms) of the last block it includes, so
/// blocks with `block_time_ms > time_ms` must be applied on top (verified exact on PUMP).
#[derive(Clone, Debug)]
pub struct L2Snapshot {
    pub time_ms: u64,
    /// Best first.
    pub bids: Vec<Level>,
    /// Best first.
    pub asks: Vec<Level>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tif {
    Alo,
    Ioc,
    Gtc,
}

pub type Cloid = [u8; 16];

pub fn cloid_hex(c: &Cloid) -> String {
    let mut s = String::with_capacity(34);
    s.push_str("0x");
    for b in c {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[derive(Clone, Debug)]
pub struct OrderReq {
    pub asset: u32,
    pub is_buy: bool,
    pub px: f64,
    pub sz: f64,
    pub reduce_only: bool,
    pub tif: Tif,
    pub cloid: Option<Cloid>,
}

#[derive(Clone, Debug)]
pub enum OrderRef {
    Oid(u64),
    Cloid(Cloid),
}

/// Exchange L1 actions the taker sends. Wire/hash formats must match the official Python SDK exactly.
#[derive(Clone, Debug)]
pub enum Action {
    /// `{"type":"order","orders":[...],"grouping":"na" | {"p": n}}`; `priority = Some(n)` -> grouping `{"p": n}`
    /// where the fee rate is n / 1e8 (1 bp = 10_000).
    Order {
        orders: Vec<OrderReq>,
        priority: Option<u64>,
    },
    /// `{"type":"batchModify","modifies":[{"oid": <int | cloid hex>, "order": {...}}]}`
    BatchModify { modifies: Vec<(OrderRef, OrderReq)> },
    /// `{"type":"modify","oid": <int | cloid hex>,"order":{...},"a":true}`; `a` (always_place) omitted when false
    /// (actions hashed with `a: false` are rejected). With always_place false the new order must be ALO (or a
    /// non-executable GTC, turned into ALO) and is placed only if the cancel succeeded.
    Modify {
        oid: OrderRef,
        order: OrderReq,
        always_place: bool,
    },
    /// `{"type":"cancel","cancels":[{"a":asset,"o":oid}],"f":true}`; `f` omitted when false.
    Cancel {
        cancels: Vec<(u32, u64)>,
        fast: bool,
    },
    /// `{"type":"cancelByCloid","cancels":[{"asset":asset,"cloid":hex}],"f":true}`; `f` omitted when false.
    CancelByCloid {
        cancels: Vec<(u32, Cloid)>,
        fast: bool,
    },
}

/// First status of an exchange response.
#[derive(Clone, Debug, PartialEq)]
pub enum OrderStatus {
    Resting {
        oid: u64,
    },
    Filled {
        total_sz: f64,
        avg_px: f64,
        oid: u64,
    },
    /// Cancel / modify acknowledged ("success" or `{"type":"default"}`).
    Success,
    /// Exchange-level rejection text, e.g. "Order could not immediately match against any resting orders."
    Rejected(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloid_hex_format() {
        let mut c = [0u8; 16];
        c[15] = 0xab;
        assert_eq!(cloid_hex(&c), "0x000000000000000000000000000000ab");
    }
}
