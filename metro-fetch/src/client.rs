use std::time::Duration;

use reqwest::{Client, StatusCode, Url, header::HeaderMap};

use crate::{errors::WMATAClientError, types::WMATATrainPredictionResponse};

#[derive(Debug)]
pub struct WMATAClient {
    client: Client,
    base_url: Url,
}

impl WMATAClient {
    /// Creates a new `WMATAClient`.
    ///
    /// # Arguments
    /// - `base_url`: The base URL of the WMATA API.
    /// - `api_key`: The API key to use for the WMATA API.
    /// - `user_agent`: A special user agent string so WMATA can differentiate our client if needed.
    ///
    /// # Returns
    /// - A new `WMATAClient` instance
    ///
    /// # Errors
    /// - `WMATAClientError::InvalidHeaderValue`: If the `api_key` or `user_agent` is not parseable.
    /// - `WMATAClientError::Client`: If an error occurs while creating the client.
    pub fn new(
        base_url: Url,
        api_key: &str,
        user_agent: &str,
        timeout: Duration,
    ) -> Result<WMATAClient, WMATAClientError> {
        let mut header_map = HeaderMap::new();
        header_map.append("User-Agent", user_agent.parse()?);
        header_map.append("api_key", api_key.parse()?);

        let client = Client::builder()
            .default_headers(header_map)
            .timeout(timeout)
            .build()?;
        Ok(WMATAClient { client, base_url })
    }

    /// Joins a path to the base URL
    ///
    /// # Arguments
    /// - `path`: The path to append to the base URL.
    ///
    /// # Returns
    /// - The new `Url`.
    ///
    /// # Errors
    /// - `WMATAClientError::UrlJoin`: If an error occurs while joining the URL.
    fn url_join(&self, path: &str) -> Result<Url, WMATAClientError> {
        Ok(self.base_url.join(path)?)
    }

    /// Gets all of the WMATA train predictions.
    ///
    /// # Returns
    /// - An array of train predictions
    ///
    /// # Errors
    /// - `WMATAClientError::Client`: If an error occurs while sending the request or
    ///   getting the response text.
    /// - `WMATAClientError::UrlJoin`: If an error occurs while joining the URL.
    /// - `WMATAClientError::StatusCode`: If the status code is not OK.
    /// - `WMATAClientError::Deserialize`: If deserializing the result fails.
    pub async fn get_predictions_raw(&self) -> Result<WMATATrainPredictionResponse, WMATAClientError> {
        let url = self.url_join("/StationPrediction.svc/json/GetPrediction/All")?;
        let resp = self.client.get(url).send().await?;
        let status = resp.status();
        let body = resp.text().await?;

        if status != StatusCode::OK {
            return Err(WMATAClientError::StatusCode(status, body));
        }

        let typed: WMATATrainPredictionResponse = serde_json::from_str(&body)?;
        Ok(typed)
    }
}
