use serde::{Deserialize, Deserializer};
use std::time::Duration;
use url::Url;

#[derive(Deserialize)]
pub struct TrainFetchConfig {
    /// Base URL for the WMATA API
    #[serde(default = "default_base_url")]
    pub wmata_api_base_url: Url,
    /// API key for WMATA
    #[serde(default)]
    pub wmata_api_key: String,
    /// Request timeout duration
    #[serde(default = "default_timeout", deserialize_with = "deserialize_duration")]
    pub wmata_api_timeout: Duration,
    /// Interval between data fetches in seconds
    #[serde(
        default = "default_fetch_interval",
        deserialize_with = "deserialize_duration"
    )]
    pub fetch_interval: Duration,
    /// MQTT broker address
    #[serde()]
    pub mqtt_broker: String,
    /// MQTT broker port
    #[serde(default = "default_mqtt_port")]
    pub mqtt_port: u16,
    /// MQTT client ID
    #[serde(default = "default_mqtt_client_id")]
    pub mqtt_client_id: String,
    /// Web server bind address
    #[serde(default = "default_web_bind_address")]
    pub web_bind_address: String,
}

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_FETCH_INTERVAL: Duration = Duration::from_secs(5);

fn default_fetch_interval() -> Duration {
    DEFAULT_FETCH_INTERVAL
}

fn default_base_url() -> Url {
    Url::parse("https://api.wmata.com/").expect("Failed to parse default base URL")
}

fn default_timeout() -> Duration {
    DEFAULT_TIMEOUT
}

fn default_mqtt_port() -> u16 {
    1883
}

fn default_mqtt_client_id() -> String {
    "reMetroClient".to_string()
}

fn deserialize_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    let raw_timeout: String = Deserialize::deserialize(deserializer)?;
    let raw_timeout = raw_timeout.trim();

    if raw_timeout.is_empty() {
        return Ok(DEFAULT_TIMEOUT);
    }

    // Accept an integer number of seconds only
    if let Ok(secs) = raw_timeout.parse::<u64>() {
        Ok(Duration::from_secs(secs))
    } else {
        Err(serde::de::Error::custom(format!(
            "REMETRO_API_TIMEOUT must be an integer representing seconds: {:?}",
            raw_timeout
        )))
    }
}

fn default_web_bind_address() -> String {
    "0.0.0.0:3000".to_string()
}

pub fn read_env_vars() -> Result<TrainFetchConfig, envy::Error> {
    dotenvy::dotenv().ok();
    envy::prefixed("REMETRO_").from_env::<TrainFetchConfig>()
}
