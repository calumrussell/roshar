//! Signed exchange actions over REST (`POST /exchange`) with a keep-alive client, as an alternative to the
//! websocket `post` channel.

use std::time::Duration;

use crate::poster::{now_ns, PostResult};
use serde_json::Value;

use crate::poster::decode_status;

pub const HL_EXCHANGE_URL: &str = "https://api.hyperliquid.xyz/exchange";

#[derive(Clone)]
pub struct RestPoster {
    http: reqwest::Client,
    url: String,
}

impl RestPoster {
    pub fn new(url: &str) -> anyhow::Result<RestPoster> {
        let http = reqwest::Client::builder()
            // Pin rustls: a workspace build can enable reqwest's default-tls, which would otherwise win.
            .use_rustls_tls()
            .timeout(Duration::from_secs(10))
            .connect_timeout(Duration::from_secs(5))
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(4)
            .tcp_keepalive(Duration::from_secs(30))
            .tcp_nodelay(true)
            .build()?;
        Ok(RestPoster {
            http,
            url: url.to_string(),
        })
    }

    /// Post a signed payload (`Signer::sign` output) and wait for the response. Always returns a result:
    /// Err status = transport / HTTP problem or an unparseable body.
    pub async fn post(&self, payload: &Value) -> PostResult {
        let sent_ns = now_ns();
        let status = match self.http.post(&self.url).json(payload).send().await {
            Ok(resp) => {
                let code = resp.status();
                match resp.text().await {
                    Ok(text) if code.is_success() => match serde_json::from_str::<Value>(&text) {
                        Ok(v) => decode_status(&v),
                        Err(e) => Err(format!(
                            "bad json ({e}): {}",
                            text.chars().take(200).collect::<String>()
                        )),
                    },
                    Ok(text) => Err(format!(
                        "HTTP {code}: {}",
                        text.chars().take(200).collect::<String>()
                    )),
                    Err(e) => Err(format!("HTTP {code}: body: {e}")),
                }
            }
            Err(e) => Err(format!("{e}")),
        };
        PostResult {
            id: 0,
            sent_ns,
            recv_ns: now_ns(),
            status,
        }
    }

    /// Open the keep-alive connection ahead of the first timed request (a GET; the response is ignored).
    pub async fn warm(&self) {
        let _ = self.http.get(&self.url).send().await;
    }
}
