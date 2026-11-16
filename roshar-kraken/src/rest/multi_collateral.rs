use roshar_http::RateLimitedClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::auth::{AuthApi, KrakenCredentials};
use super::parse_response;

const BASE_URL: &str = "https://futures.kraken.com";

/// Parameters of `PUT /leveragepreferences`, sent form-encoded in the body. Without
/// `maxLeverage` the symbol returns to plain cross margin; `marginMode` defaults to `isolated`.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenLeverageSettingRequest {
    pub symbol: String,
    #[serde(rename = "maxLeverage", skip_serializing_if = "Option::is_none")]
    pub max_leverage: Option<String>,
    /// `isolated` or `cross`.
    #[serde(rename = "marginMode", skip_serializing_if = "Option::is_none")]
    pub margin_mode: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenLeverageSettingResponse {
    pub result: String,
    #[serde(rename = "serverTime")]
    pub server_time: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenLeveragePreference {
    pub symbol: String,
    #[serde(rename = "maxLeverage")]
    pub max_leverage: f64,
    /// `isolated` or `cross`.
    #[serde(rename = "marginMode")]
    pub margin_mode: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenGetLeverageResponse {
    pub result: String,
    #[serde(rename = "leveragePreferences")]
    pub leverage_preferences: Vec<KrakenLeveragePreference>,
    #[serde(rename = "serverTime")]
    pub server_time: String,
}

/// Kraken Multi-Collateral API
pub struct MultiCollateralApi {
    client: Arc<RateLimitedClient>,
    credentials: KrakenCredentials,
}

impl MultiCollateralApi {
    pub fn new_with_client(client: Arc<RateLimitedClient>, credentials: KrakenCredentials) -> Self {
        Self {
            client,
            credentials,
        }
    }

    async fn make_get_request<R>(&self, endpoint: &str) -> Result<R, Box<dyn std::error::Error>>
    where
        R: for<'de> Deserialize<'de>,
    {
        let headers = AuthApi::create_headers(&self.credentials, endpoint, "")?;

        let mut request = self
            .client
            .request(reqwest::Method::GET, &format!("{BASE_URL}{endpoint}"))
            .await;

        for (key, value) in headers {
            request = request.header(key, value);
        }

        let text = request.send().await?.text().await?;
        parse_response(&text)
    }

    async fn make_put_request<T, R>(
        &self,
        endpoint: &str,
        request_data: &T,
    ) -> Result<R, Box<dyn std::error::Error>>
    where
        T: Serialize,
        R: for<'de> Deserialize<'de>,
    {
        let post_data = serde_urlencoded::to_string(request_data)?;
        let headers = AuthApi::create_headers(&self.credentials, endpoint, &post_data)?;

        let mut request = self
            .client
            .request(reqwest::Method::PUT, &format!("{BASE_URL}{endpoint}"))
            .await;

        for (key, value) in headers {
            request = request.header(key, value);
        }

        request = request
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(post_data);

        let text = request.send().await?.text().await?;
        parse_response(&text)
    }

    pub async fn get_leverage(
        &self,
    ) -> Result<KrakenGetLeverageResponse, Box<dyn std::error::Error>> {
        self.make_get_request("/derivatives/api/v3/leveragepreferences")
            .await
    }

    /// `PUT /leveragepreferences`. `max_leverage` is a decimal string; `None` restores plain
    /// cross margin. `margin_mode` is `isolated` (the exchange default) or `cross`.
    pub async fn set_leverage(
        &self,
        symbol: &str,
        max_leverage: Option<&str>,
        margin_mode: Option<&str>,
    ) -> Result<KrakenLeverageSettingResponse, Box<dyn std::error::Error>> {
        let leverage_request = KrakenLeverageSettingRequest {
            symbol: symbol.to_string(),
            max_leverage: max_leverage.map(str::to_string),
            margin_mode: margin_mode.map(str::to_string),
        };

        self.make_put_request("/derivatives/api/v3/leveragepreferences", &leverage_request)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_leverage_preferences() {
        // Example from https://docs.kraken.com/openapi/futures-rest.yaml
        let text = r#"{"result":"success","serverTime":"2022-06-28T15:01:12.762Z","leveragePreferences":[{"symbol":"PF_XBTUSD","maxLeverage":10,"marginMode":"isolated"}]}"#;
        let r: KrakenGetLeverageResponse = parse_response(text).unwrap();
        assert_eq!(r.leverage_preferences[0].margin_mode, "isolated");
        assert_eq!(r.leverage_preferences[0].max_leverage, 10.0);
    }

    #[test]
    fn leverage_request_encoding() {
        let req = KrakenLeverageSettingRequest {
            symbol: "PF_XBTUSD".to_string(),
            max_leverage: Some("5".to_string()),
            margin_mode: Some("cross".to_string()),
        };
        assert_eq!(
            serde_urlencoded::to_string(&req).unwrap(),
            "symbol=PF_XBTUSD&maxLeverage=5&marginMode=cross"
        );
    }

    fn create_test_client() -> Arc<RateLimitedClient> {
        Arc::new(RateLimitedClient::new(10, 1))
    }

    // Live tests only: the library itself never reads the environment.
    fn test_credentials() -> KrakenCredentials {
        KrakenCredentials::new(
            std::env::var("KRAKEN_PUBLIC_KEY").expect("KRAKEN_PUBLIC_KEY"),
            std::env::var("KRAKEN_PRIVATE_KEY").expect("KRAKEN_PRIVATE_KEY"),
        )
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_leverage() -> Result<(), Box<dyn std::error::Error>> {
        let api = MultiCollateralApi::new_with_client(create_test_client(), test_credentials());
        let leverage_response = api.get_leverage().await?;

        assert_eq!(leverage_response.result, "success");
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_set_leverage() -> Result<(), Box<dyn std::error::Error>> {
        let api = MultiCollateralApi::new_with_client(create_test_client(), test_credentials());

        let set_response = api
            .set_leverage("PF_XBTUSD", Some("5"), Some("cross"))
            .await?;

        assert_eq!(set_response.result, "success");

        let reset_response = api.set_leverage("PF_XBTUSD", None, None).await?;

        assert_eq!(reset_response.result, "success");
        Ok(())
    }
}
