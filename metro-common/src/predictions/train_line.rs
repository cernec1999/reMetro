use core::fmt;
use std::{cmp::Ordering, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::ReMetroError;

/// Represents the Metro Line
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum TrainLine {
    Red,
    Orange,
    Blue,
    Green,
    Yellow,
    Silver,
    NoPassengers,
}

impl TrainLine {
    #[inline]
    pub const fn code(self) -> &'static str {
        match self {
            TrainLine::Red => "RD",
            TrainLine::Orange => "OR",
            TrainLine::Blue => "BL",
            TrainLine::Green => "GR",
            TrainLine::Yellow => "YL",
            TrainLine::Silver => "SV",
            TrainLine::NoPassengers => "--",
        }
    }

    #[inline]
    pub fn from_code(s: &str) -> Result<Self, ReMetroError> {
        match s {
            "RD" => Ok(TrainLine::Red),
            "OR" => Ok(TrainLine::Orange),
            "BL" => Ok(TrainLine::Blue),
            "GR" => Ok(TrainLine::Green),
            "YL" => Ok(TrainLine::Yellow),
            "SV" => Ok(TrainLine::Silver),
            "No" | "--" => Ok(TrainLine::NoPassengers),
            _ => Err(ReMetroError::LineConversion),
        }
    }
}

impl Ord for TrainLine {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.code().cmp(other.code())
    }
}

impl PartialOrd for TrainLine {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for TrainLine {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl FromStr for TrainLine {
    type Err = ReMetroError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TrainLine::from_code(s)
    }
}
