use core::{
    cmp::Ordering,
    fmt::{self},
    str::FromStr,
};

use crate::{ReMetroError, utils::is_dashed};
use serde::{Deserialize, Serialize};

/// Represents statuses like ARR / BRD / DLY
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum OtherStatus {
    Arriving,
    Boarding,
    Delayed,
}

impl OtherStatus {
    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            OtherStatus::Arriving => "ARR",
            OtherStatus::Boarding => "BRD",
            OtherStatus::Delayed => "DLY",
        }
    }

    #[inline]
    pub fn from_code(s: &str) -> Result<Self, ReMetroError> {
        match s {
            "ARR" => Ok(OtherStatus::Arriving),
            "BRD" => Ok(OtherStatus::Boarding),
            "DLY" => Ok(OtherStatus::Delayed),
            _ => Err(ReMetroError::OtherStatusConversion),
        }
    }
}

impl fmt::Display for OtherStatus {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl FromStr for OtherStatus {
    type Err = ReMetroError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        OtherStatus::from_code(s)
    }
}

/// Represents timing prediction
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TrainMins {
    Min(u8),
    Other(OtherStatus),
    Unknown,
}

impl TrainMins {
    #[inline]
    fn sort_key(&self) -> (u8, u8) {
        match *self {
            TrainMins::Other(OtherStatus::Boarding) => (0, 0),
            TrainMins::Other(OtherStatus::Arriving) => (1, 0),
            TrainMins::Min(n) => (2, n),
            TrainMins::Other(OtherStatus::Delayed) => (3, 0),
            TrainMins::Unknown => (4, 0),
        }
    }
}

impl Ord for TrainMins {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.sort_key().cmp(&other.sort_key())
    }
}

impl PartialOrd for TrainMins {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl TrainMins {
    #[inline]
    pub fn from_display_str(s: &str) -> Result<Self, ReMetroError> {
        if s.is_empty() || is_dashed(s) {
            return Ok(TrainMins::Unknown);
        }

        if let Ok(n) = u8::from_str(s) {
            return Ok(TrainMins::Min(n));
        }

        match OtherStatus::from_code(s) {
            Ok(s) => Ok(TrainMins::Other(s)),
            Err(_) => Err(ReMetroError::TrainPredictionConversion),
        }
    }
}

impl fmt::Display for TrainMins {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrainMins::Min(n) => write!(f, "{n}"),
            TrainMins::Other(s) => f.write_str(s.code()),
            TrainMins::Unknown => f.write_str("---"),
        }
    }
}

impl FromStr for TrainMins {
    type Err = ReMetroError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TrainMins::from_display_str(s)
    }
}
