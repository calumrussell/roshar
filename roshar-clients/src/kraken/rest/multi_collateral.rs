use serde::{Deserialize, Serialize};

use super::auth::AuthApi;

const BASE_URL: &str = "https://futures.kraken.com";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KrakenLeverageSettingRequest {
    pub symbol: String,
    #[serde(rename = "marginType")]
    pub margin_type: String, // "isolated" or "cross"
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
    #[serde(rename = "marginType")]
    pub margin_type: String,
    #[serde(rename = "maxLeverage")]
    pub max_leverage: f64,
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
pub struct MultiCollateralApi;

impl MultiCollateralApi {
    async fn make_get_request<R>(endpoint: &str) -> Result<R, Box<dyn std::error::Error>>
    where
        R: for<'de> Deserialize<'de>,
    {
        let client = crate::http::get_http_client();
        let headers = AuthApi::create_headers(endpoint, "")?;

        let mut request = client.get(format!("{BASE_URL}{endpoint}"));

        for (key, value) in headers {
            request = request.header(key, value);
        }

        let response = request.send().await?;
        let text = response.text().await?;
        let parsed: R = serde_json::from_str(&text)?;
        Ok(parsed)
    }

    async fn make_put_request<T, R>(
        endpoint: &str,
        request_data: &T,
    ) -> Result<R, Box<dyn std::error::Error>>
    where
        T: Serialize,
        R: for<'de> Deserialize<'de>,
    {
        let client = crate::http::get_http_client();
        let post_data = serde_urlencoded::to_string(request_data)?;
        let headers = AuthApi::create_headers(endpoint, &post_data)?;

        let mut request = client.put(format!("{BASE_URL}{endpoint}"));

        for (key, value) in headers {
            request = request.header(key, value);
        }

        request = request
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(post_data);

        let response = request.send().await?;
        let text = response.text().await?;

        let parsed: R = serde_json::from_str(&text)?;
        Ok(parsed)
    }

    pub async fn get_leverage() -> Result<KrakenGetLeverageResponse, Box<dyn std::error::Error>> {
        Self::make_get_request("/derivatives/api/v3/leveragepreferences").await
    }

    pub async fn set_leverage(
        symbol: &str,
        margin_type: &str,
    ) -> Result<KrakenLeverageSettingResponse, Box<dyn std::error::Error>> {
        let leverage_request = KrakenLeverageSettingRequest {
            symbol: symbol.to_string(),
            margin_type: margin_type.to_string(),
        };

        Self::make_put_request("/derivatives/api/v3/leveragepreferences", &leverage_request).await
    }
}

#[cfg(test)]
mod tests {
    use super::MultiCollateralApi;

    #[tokio::test]
    #[ignore]
    async fn test_get_leverage() -> Result<(), Box<dyn std::error::Error>> {
        use std::{thread::sleep, time::Duration};

        sleep(Duration::from_millis(110));

        let leverage_response = MultiCollateralApi::get_leverage().await?;

        assert_eq!(leverage_response.result, "success");
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_set_leverage() -> Result<(), Box<dyn std::error::Error>> {
        use std::{thread::sleep, time::Duration};

        sleep(Duration::from_millis(200));

        let set_response = MultiCollateralApi::set_leverage("PF_XBTUSD", "cross").await?;

        assert_eq!(set_response.result, "success");

        sleep(Duration::from_millis(100));

        let reset_response = MultiCollateralApi::set_leverage("PF_XBTUSD", "isolated").await?;

        assert_eq!(reset_response.result, "success");
        Ok(())
    }
}
