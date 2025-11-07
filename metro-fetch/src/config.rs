use std::time::Duration;
use url::Url;
use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
pub struct TrainFetchConfig {
    /// Base URL for the WMATA API
    #[serde(default = "default_base_url")]
    pub base_url: Url,
    /// API key for WMATA
    #[serde(default)]
    pub key: String,
    /// Request timeout duration
    #[serde(default = "default_timeout", deserialize_with = "deserialize_duration")]
    pub timeout: Duration,
    /// Interval between data fetches in seconds
    #[serde(default = "default_fetch_interval", deserialize_with = "deserialize_duration")]
    pub fetch_interval: Duration,
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
            "WMATA_API_TIMEOUT must be an integer representing seconds: {:?}",
            raw_timeout
        )))
    }
}

pub fn read_env_vars() -> Result<TrainFetchConfig, envy::Error> {
    dotenvy::dotenv().ok();
    envy::prefixed("WMATA_API_").from_env::<TrainFetchConfig>()
}