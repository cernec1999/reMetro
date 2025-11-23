use serde::{Deserialize, Serialize};

use crate::{WMATAStationCode, WMATATrackCode};

/// Subscription key for monitoring specific station/platform combinations
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TrainPredictionsRequest {
    /// Monitor all platforms for a specific station
    Station(WMATAStationCode),
    /// Monitor a specific platform at a specific station
    StationPlatform(WMATAStationCode, WMATATrackCode),
}

impl std::fmt::Display for TrainPredictionsRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrainPredictionsRequest::Station(station) => write!(f, "Station({})", station),
            TrainPredictionsRequest::StationPlatform(station, platform) => {
                write!(f, "StationPlatform({}, {})", station, platform)
            }
        }
    }
}

/// Represents the response from the web server
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct PredictionsResponse {
    pub trains: String,
}
