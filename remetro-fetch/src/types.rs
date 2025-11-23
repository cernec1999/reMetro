use std::collections::HashMap;

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
    pub(crate) destination_name: Option<String>,
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

/// Represents a station address in the WMATA API.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct WMATAStationAddress {
    /// Street
    #[serde(alias = "Street")]
    pub(crate) street: String,
    /// City
    #[serde(alias = "City")]
    pub(crate) city: String,
    /// State
    #[serde(alias = "State")]
    pub(crate) state: String,
    /// Zip code
    #[serde(alias = "Zip")]
    pub(crate) zip: String,
}

/// Represents a single station in the WMATA API.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct WMATAStationInfo {
    /// Station code.
    #[serde(alias = "Code")]
    pub(crate) code: String,
    /// Station name.
    #[serde(alias = "Name")]
    pub(crate) name: String,
    /// For stations with multiple platforms, this indicates a "linked" station. We'll make this optional,
    /// just in case, but WMATA returns an empty string when not applicable.
    #[serde(alias = "StationTogether1")]
    pub(crate) station_together_1: Option<String>,
    /// For stations with multiple platforms, this indicates a "linked" station. We'll make this optional,
    /// just in case, but WMATA returns an empty string when not applicable.
    #[serde(alias = "StationTogether2")]
    pub(crate) station_together_2: Option<String>,
    /// Line code 1 (e.g., RD, BL, etc).
    #[serde(alias = "LineCode1")]
    pub(crate) line_code_1: Option<String>,
    /// Line code 2 (e.g., RD, BL, etc).
    #[serde(alias = "LineCode2")]
    pub(crate) line_code_2: Option<String>,
    /// Line code 3 (e.g., RD, BL, etc).
    #[serde(alias = "LineCode3")]
    pub(crate) line_code_3: Option<String>,
    /// Line code 4 (e.g., RD, BL, etc).
    #[serde(alias = "LineCode4")]
    pub(crate) line_code_4: Option<String>,
    /// Latitude of the station.
    #[serde(alias = "Lat")]
    pub(crate) lat: f64,
    /// Longitude of the station.
    #[serde(alias = "Lon")]
    pub(crate) lon: f64,
    /// Station address.
    #[serde(alias = "Address")]
    pub(crate) address: WMATAStationAddress,
}

/// Represents a full WMATA station information response.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WMATAStationInfoResponse {
    /// A vector of station information.
    #[serde(alias = "Stations")]
    pub(crate) stations: Vec<WMATAStationInfo>,
}

/// Represents aliases from the WMATA API.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WMATAAliases {
    /// Station aliases
    pub(crate) station_aliases: HashMap<String, Vec<String>>,
    /// No passenger aliases
    pub(crate) no_passenger_aliases: Vec<String>,
}
