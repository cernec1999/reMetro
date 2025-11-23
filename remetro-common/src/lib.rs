use core::fmt;
use std::{collections::HashSet, fmt::Display, num::ParseIntError, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::predictions::train_line::TrainLine;

pub mod predictions;
pub mod utils;

#[derive(Debug, Error)]
pub enum ReMetroError {
    #[error("Error converting line abbreviation to strong metro type")]
    LineConversion,
    #[error("Error converting status abbreviation (e.g. ARR, BRD, DLY) to strong metro type")]
    OtherStatusConversion,
    #[error(
        "Error converting train prediction time (e.g. \"1\", \"ARR\", etc) to strong metro type"
    )]
    TrainPredictionConversion,
    #[error("Error converting number of cars to strong metro type")]
    CarsConversion,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WMATATrackCode(pub u8);

impl FromStr for WMATATrackCode {
    type Err = ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse::<u8>()?))
    }
}

impl Display for WMATATrackCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub type WMATAStationCode = String;

/// Representation of a WMATA station with its code and known names.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Station {
    /// Station code.
    pub code: WMATAStationCode,
    /// Station name.
    pub name: String,
    /// Known aliases for the station.
    pub aliases: HashSet<String>,
    /// Linked stations (for multi-platform stations).
    pub linked_stations: HashSet<WMATAStationCode>,
    /// Lines that serve this station.
    pub lines: HashSet<TrainLine>,
    /// Location information for the station.
    pub location: Location,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Location {
    /// Latitude of the station.
    pub latitude: f64,
    /// Longitude of the station.
    pub longitude: f64,
    /// Address of the station.
    pub address: Address,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Address {
    /// Street address of the station.
    pub street: String,
    /// City where the station is located.
    pub city: String,
    /// State where the station is located.
    pub state: String,
    /// ZIP code of the station's location.
    pub zip: String,
}
