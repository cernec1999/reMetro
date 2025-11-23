use std::num::ParseIntError;

use remetro_common::{ReMetroError, predictions::api::TrainPredictionsRequest};
use reqwest::{StatusCode, header::InvalidHeaderValue};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WMATAClientError {
    /// An invalid header value was set
    #[error("Invalid header value: {0}")]
    InvalidHeaderValue(#[from] InvalidHeaderValue),
    /// The HTTP client failed to build or send
    #[error("HTTP client error: {0}")]
    Client(#[from] reqwest::Error),
    /// Base URL join error
    #[error("Failed to join URL path: {0}")]
    UrlJoin(#[from] url::ParseError),
    /// If status code is not OK
    #[error("The API returned status code {0}: {1}")]
    StatusCode(StatusCode, String),
    /// If an error occurs while deserializing the response text
    #[error("Error deserializing the response text: {0}")]
    Deserialize(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum StationDirectoryError {
    /// If a conversion (e.g., Line/Cars/Min) failed
    #[error("Normalization error: {0}")]
    Normalize(#[from] ReMetroError),
    /// If we couldn't turn the platform / group into a number.
    #[error("Could not convert platform into a number: {0}")]
    PlatformParseError(#[from] ParseIntError),
    /// If a station code is not found
    #[error("Station and/or platform code not found: {0}")]
    InvalidStationOrPlatform(TrainPredictionsRequest),
    /// If a RwLock is poisoned
    #[error("RwLock poisoned: {0}")]
    RwLockPoisonError(String),
    /// If we encountered an error while deserializing station aliases
    #[error("Deserialization error: {0}")]
    Deserialization(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum PublisherError {
    /// If an error occurs while publishing to MQTT
    #[error("Publisher error: {0}")]
    MqttClient(#[from] rumqttc::ClientError),
    /// If an error occurs while serializing the payload
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
