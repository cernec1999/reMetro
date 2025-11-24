use serde::{Deserialize, Serialize};

use crate::{WMATAStationCode, WMATATrackCode};

/// Subscription key for monitoring specific station/track combinations
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TrainPredictionsRequest {
    /// Monitor all tracks at a specific station
    Station(WMATAStationCode),
    /// Monitor a specific track at a specific station
    StationTrack(WMATAStationCode, WMATATrackCode),
}

impl std::fmt::Display for TrainPredictionsRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrainPredictionsRequest::Station(station) => write!(f, "Station({})", station),
            TrainPredictionsRequest::StationTrack(station, track) => {
                write!(f, "StationTrack({}, {})", station, track)
            }
        }
    }
}

/// Represents the response from the web server
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct PredictionsResponse {
    pub trains: String,
}
