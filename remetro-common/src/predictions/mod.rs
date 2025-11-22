use core::{
    cmp::Ordering,
    fmt::{self},
};

use crate::{
    WMATAPlatformCode,
    predictions::{train_cars::TrainCars, train_line::TrainLine, train_mins::TrainMins},
};
use serde::{Deserialize, Serialize};

pub mod api;
pub mod train_cars;
pub mod train_line;
pub mod train_mins;
pub mod train_update;

/// Represents the prediction from one train
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct TrainPrediction {
    pub platform: WMATAPlatformCode,
    pub line: TrainLine,
    pub cars: TrainCars,
    pub destination: String,
    pub min: TrainMins,
}

/// Train should sort by minutes, then by platform, then by line, then by cars, then by destination
impl TrainPrediction {
    #[inline]
    pub fn sort_key(&self) -> (TrainMins, WMATAPlatformCode, TrainLine, TrainCars, String) {
        (
            self.min,
            self.platform,
            self.line,
            self.cars,
            self.destination.clone(),
        )
    }
}

impl PartialOrd for TrainPrediction {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TrainPrediction {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.sort_key().cmp(&other.sort_key())
    }
}

impl fmt::Display for TrainPrediction {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Line: {}, Cars: {}, Destination: {}, Min: {}",
            self.line, self.cars, self.destination, self.min
        )
    }
}
