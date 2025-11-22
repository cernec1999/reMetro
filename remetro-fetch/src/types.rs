use serde::{Deserialize, Serialize};

/// Represents a single train prediction in the WMATA API.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct WMATATrainPrediction {
    /// Might return "-" or null.
    #[serde(alias = "Car")]
    pub(crate) car: Option<String>,
    /// Abbreviated destination for a train.
    #[serde(alias = "Destination")]
    pub(crate) destination: String,
    /// Final destination station code.
    #[serde(alias = "DestinationCode")]
    pub(crate) destination_code: Option<String>,
    /// Final destination station name.
    #[serde(alias = "DestinationName")]
    pub(crate) destination_name: String,
    /// The track the train is on.
    #[serde(alias = "Group")]
    pub(crate) group: String,
    /// Two-letter abbreviation for the line.
    #[serde(alias = "Line")]
    pub(crate) line: String,
    /// Station code where the train is about to arrive.
    #[serde(alias = "LocationCode")]
    pub(crate) location_code: String,
    /// Station name where the train is about to arrive.
    #[serde(alias = "LocationName")]
    pub(crate) location_name: String,
    /// Minutes until arrival. Can be a numeric value, ARR, BRD, ---, or empty.
    #[serde(alias = "Min")]
    pub(crate) min: String,
}

/// Represents a full WMATA train prediction response.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct WMATATrainPredictionResponse {
    /// A vector of train predictions.
    #[serde(alias = "Trains")]
    pub(crate) trains: Vec<WMATATrainPrediction>,
}