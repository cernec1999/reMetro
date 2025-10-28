use core::fmt;

use serde::{Deserialize, Serialize};
use crate::{MetroString, MetroVec};

/// Represents the Metro Line
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Line {
    Red,
    Orange,
    Blue,
    Green,
    Yellow,
    Silver
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Line::Red => "RD",
            Line::Orange => "OR",
            Line::Blue => "BL",
            Line::Green => "GR",
            Line::Yellow => "YL",
            Line::Silver => "SV",
        })
    }
}

/// Represents the different timings for the train arrival prediction
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TrainPrediction {
    /// If the train is not at the station, how many minutes away is it?
    Min(u8),
    /// Arriving, Boarding, or Delayed?
    Other(OtherStatus),
    /// Unknown status
    Unknown
}

impl fmt::Display for TrainPrediction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrainPrediction::Min(n) => write!(f, "{n}"),
            TrainPrediction::Other(s) => write!(f, "{s}"),
            TrainPrediction::Unknown => f.write_str("---"),
        }
    }
}

/// Represents the status of a train that isn't a numeric time
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum OtherStatus {
    Arriving,
    Boarding,
    Delayed
}

impl fmt::Display for OtherStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            OtherStatus::Arriving => "ARR",
            OtherStatus::Boarding => "BRD",
            OtherStatus::Delayed => "DLY",
        })
    }
}

/// Represents how many cars are attached to this train
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum Cars {
    Num(u8),
    Unknown
}

impl fmt::Display for Cars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Cars::Num(n) => write!(f, "{n}"),
            Cars::Unknown => write!(f, "---"),
        }
    }
}

/// Represents the prediction from one train
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Train {
    /// The metro line
    pub line: Line,
    /// How many cars attached to this train
    pub cars: Cars,
    /// The terminal station (final destination)
    pub destination: MetroString<10>,
    /// How many minutes until the train arrives
    pub min: TrainPrediction
}


/// Represents the request from the ESP32
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct PredictionsRequest {
    /// The station code
    pub destination: MetroString<3>,
}

/// Represents the response from the web server
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct PredictionsResponse {
    /// The train response
    pub trains: MetroVec<Train, 3>,
}