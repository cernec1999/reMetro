use core::{
    cmp::Ordering,
    fmt::{self},
    str::FromStr,
};

use crate::{ReMetroError, utils::is_dashed};
use serde::{Deserialize, Serialize};

/// Represents how many cars a train has
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TrainCars {
    Num(u8),
    Unknown,
}

impl TrainCars {
    #[inline]
    pub fn from_display_str(s: &str) -> Result<Self, ReMetroError> {
        if is_dashed(s) || s.is_empty() {
            return Ok(TrainCars::Unknown);
        }
        if let Ok(n) = u8::from_str(s) {
            return Ok(TrainCars::Num(n));
        }
        Err(ReMetroError::CarsConversion)
    }

    #[inline]
    pub fn sort_key(&self) -> u8 {
        match self {
            TrainCars::Num(n) => *n,
            TrainCars::Unknown => u8::MAX,
        }
    }
}

impl Ord for TrainCars {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.sort_key().cmp(&other.sort_key())
    }
}

impl PartialOrd for TrainCars {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for TrainCars {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrainCars::Num(n) => write!(f, "{n}"),
            TrainCars::Unknown => f.write_str("-"),
        }
    }
}

impl FromStr for TrainCars {
    type Err = ReMetroError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TrainCars::from_display_str(s)
    }
}
