use crate::predictions::{TrainPrediction, train_mins::TrainMins};
use serde::{Deserialize, Serialize};

pub const MAX_TRAINS_PER_UPDATE: usize = 3;

/// Represents a change in arrival time for a specific train
#[derive(Serialize, Deserialize, Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct TrainMinuteChange {
    /// Index of the train in the current lineup (0-based)
    pub train_index: usize,
    /// New arrival time/status
    pub new_minutes: TrainMins,
    /// Previous arrival time/status for comparison
    pub old_minutes: TrainMins,
}

/// Subscription update indicating trains for a station/platform have changed
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct FullTrainUpdate {
    /// Full update with the current list of trains
    pub trains: Vec<TrainPrediction>,
}
