use core::fmt;
use std::{fmt::Display, num::ParseIntError, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

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
pub struct WMATAPlatformCode(pub u8);

impl FromStr for WMATAPlatformCode {
    type Err = ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse::<u8>()?))
    }
}

impl Display for WMATAPlatformCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub type WMATAStationCode = String;
