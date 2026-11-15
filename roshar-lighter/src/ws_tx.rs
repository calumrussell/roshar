//! Messages for the trading side of the Lighter websocket (`LIGHTER_WSS_URL`): transaction
//! submission (`jsonapi/sendtx`, `jsonapi/sendtxbatch`) and the authenticated account channels.
//! Builders only; the caller owns the connection. Replies to a send carry the request `id`.

use anyhow::Result;
use serde_json::json;

use crate::exchange::batch_fields;
use crate::signer::SignedTx;

/// `{"type":"jsonapi/sendtx","data":{"id":..,"tx_type":..,"tx_info":{..}}}`. `tx_info` is embedded
/// as the signer produced it (not re-serialised).
pub fn send_tx(id: &str, tx: &SignedTx) -> String {
    format!(
        "{{\"type\":\"jsonapi/sendtx\",\"data\":{{\"id\":{},\"tx_type\":{},\"tx_info\":{}}}}}",
        serde_json::Value::String(id.to_string()),
        tx.tx_type,
        tx.tx_info
    )
}

/// `{"type":"jsonapi/sendtxbatch","data":{"id":..,"tx_types":"[..]","tx_infos":"[..]"}}`, with the
/// lists serialised to strings as Lighter expects.
pub fn send_tx_batch(id: &str, txs: &[SignedTx]) -> Result<String> {
    let (types, infos) = batch_fields(txs)?;
    Ok(json!({"type": "jsonapi/sendtxbatch", "data": {"id": id, "tx_types": types, "tx_infos": infos}}).to_string())
}

/// `account_orders/{market}/{account}`: our orders on one market (Order JSON keyed by market id).
pub fn subscribe_account_orders(market_id: i64, account_index: i64, auth: &str) -> String {
    subscribe_auth(&format!("account_orders/{market_id}/{account_index}"), auth)
}

/// `account_market/{market}/{account}`: position, orders and trades of one market.
pub fn subscribe_account_market(market_id: i64, account_index: i64, auth: &str) -> String {
    subscribe_auth(&format!("account_market/{market_id}/{account_index}"), auth)
}

/// `account_tx/{account}`: the account's executed transactions.
pub fn subscribe_account_tx(account_index: i64, auth: &str) -> String {
    subscribe_auth(&format!("account_tx/{account_index}"), auth)
}

/// Subscribe to any authenticated channel.
pub fn subscribe_auth(channel: &str, auth: &str) -> String {
    json!({"type": "subscribe", "channel": channel, "auth": auth}).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_tx_embeds_tx_info_verbatim() {
        let t = SignedTx {
            tx_type: 15,
            tx_info: r#"{"Z":1,"A":2}"#.into(),
            tx_hash: "h".into(),
        };
        let m = send_tx("b1", &t);
        assert_eq!(
            m,
            r#"{"type":"jsonapi/sendtx","data":{"id":"b1","tx_type":15,"tx_info":{"Z":1,"A":2}}}"#
        );
        let v: serde_json::Value =
            serde_json::from_str(&send_tx_batch("b2", &[t.clone(), t]).unwrap()).unwrap();
        assert_eq!(v["data"]["tx_types"], "[15,15]");
    }
}
