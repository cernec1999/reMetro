use std::{
    collections::{BTreeSet, HashMap},
    str::FromStr,
    sync::{Arc, RwLock},
};

use metro_common::{
    WMATAPlatformCode, WMATAStationCode,
    predictions::{
        TrainPrediction,
        api::TrainPredictionsRequest,
        train_cars::TrainCars,
        train_line::TrainLine,
        train_mins::TrainMins,
        train_update::{FullTrainUpdate, MAX_TRAINS_PER_UPDATE},
    },
};

use crate::{
    errors::StationDirectoryError,
    types::{WMATATrainPrediction, WMATATrainPredictionResponse},
    utils::SortedVec,
};

/// Normalize a single raw prediction into the `Train` type.
pub fn to_train(
    p: &WMATATrainPrediction,
    platform: WMATAPlatformCode,
) -> Result<TrainPrediction, StationDirectoryError> {
    let cars = match &p.car {
        Some(s) => TrainCars::from_str(s)?,
        None => TrainCars::Unknown,
    };

    Ok(TrainPrediction {
        platform,
        line: TrainLine::from_str(&p.line)?,
        cars,
        destination: p.destination.clone(),
        min: TrainMins::from_str(&p.min)?,
    })
}

/// Represents a directory event, which will be the return type for ingestion.
#[derive(Debug, Clone)]
pub struct DirectoryEvent {
    pub key: TrainPredictionsRequest,
    pub update: FullTrainUpdate,
}

/// A thread-safe directory of WMATA station information with granular change notifications.
///
/// This structure provides thread-safe access with single-writer, multiple-reader semantics:
/// - Only one thread can modify the directory at a time (via `ingest`)
/// - Multiple threads can read from the directory concurrently (via `station_name`, `platforms`, `as_records`)
/// - All operations are protected by `RwLock` for safe concurrent access
/// - Subscribers can be notified of changes for specific stations/platforms via individual `watch` channels
///
/// For sharing across threads, use `StationDirectory::new_shared()` or wrap in `Arc`.
#[derive(Debug)]
pub struct StationDirectory {
    names_by_code: RwLock<HashMap<WMATAStationCode, String>>,
    platforms_by_code: RwLock<HashMap<WMATAStationCode, BTreeSet<WMATAPlatformCode>>>,
    // Efficient O(1) lookup structure for normalized train data
    trains_by_platform: RwLock<HashMap<TrainPredictionsRequest, Vec<TrainPrediction>>>,
}

/// Thread-safe, shared reference to a StationDirectory.
/// This is the recommended way to share a StationDirectory across threads.
pub type SharedStationDirectory = Arc<StationDirectory>;

impl Default for StationDirectory {
    fn default() -> Self {
        Self {
            names_by_code: RwLock::new(HashMap::new()),
            platforms_by_code: RwLock::new(HashMap::new()),
            trains_by_platform: RwLock::new(HashMap::new()),
        }
    }
}

impl StationDirectory {
    /// Create a new shared StationDirectory wrapped in Arc for thread-safe sharing.
    pub fn new_shared() -> SharedStationDirectory {
        Arc::new(Self::default())
    }

    pub fn ingest(
        &self,
        resp: &WMATATrainPredictionResponse,
    ) -> Result<Vec<DirectoryEvent>, StationDirectoryError> {
        // Update basic station/platform directory and build efficient lookup structures
        {
            let mut names_map = self
                .names_by_code
                .write()
                .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;
            let mut platforms_map = self
                .platforms_by_code
                .write()
                .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;
            let mut trains_by_platform = self
                .trains_by_platform
                .write()
                .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;

            // Clear previous train data
            trains_by_platform.clear();

            // Group trains by station for station-level subscriptions
            let mut station_trains: HashMap<WMATAStationCode, Vec<TrainPrediction>> =
                HashMap::new();

            for p in &resp.trains {
                names_map
                    .entry(p.location_code.clone())
                    .or_insert_with(|| p.location_name.clone());

                let platform = WMATAPlatformCode::from_str(&p.group)?;
                platforms_map
                    .entry(p.location_code.clone())
                    .or_default()
                    .insert(platform);

                // Normalize the train
                let train = to_train(p, platform)?;

                // Add to station-level collection (for aggregation)
                station_trains
                    .entry(p.location_code.clone())
                    .or_default()
                    .insert_sorted(train.clone());

                // Add to platform-level lookup
                let platform_key =
                    TrainPredictionsRequest::StationPlatform(p.location_code.clone(), platform);
                trains_by_platform
                    .entry(platform_key)
                    .or_default()
                    .insert_sorted(train);
            }

            // Now populate station-level subscription keys with aggregated data
            for (station_code, trains) in station_trains {
                let station_key = TrainPredictionsRequest::Station(station_code);
                trains_by_platform.insert(station_key, trains);
            }
        }

        // Check for prediction changes and notify subscribers with detailed change information
        self.check_and_notify_prediction_changes(resp)
    }

    /// Check for prediction changes and notify subscribers with detailed change information
    fn check_and_notify_prediction_changes(
        &self,
        resp: &WMATATrainPredictionResponse,
    ) -> Result<Vec<DirectoryEvent>, StationDirectoryError> {
        let mut events: Vec<DirectoryEvent> = Vec::new();
        let current_trains = self
            .trains_by_platform
            .read()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;

        // Collect unique stations and station-platform combinations from the response
        let mut affected_keys = std::collections::HashSet::new();

        for prediction in &resp.trains {
            affected_keys.insert(TrainPredictionsRequest::Station(
                prediction.location_code.clone(),
            ));
            if let Ok(platform) = WMATAPlatformCode::from_str(&prediction.group) {
                affected_keys.insert(TrainPredictionsRequest::StationPlatform(
                    prediction.location_code.clone(),
                    platform,
                ));
            }
        }

        // Check each affected subscription key for changes
        for key in affected_keys {
            if let Some(trains) = current_trains.get(&key) {
                let update = FullTrainUpdate {
                    // Get the first 3 trains for the update
                    trains: trains.iter().take(MAX_TRAINS_PER_UPDATE).cloned().collect(),
                };
                events.push(DirectoryEvent { key, update });
            }
        }

        Ok(events)
    }

    pub fn station_name(&self, code: &str) -> Result<String, StationDirectoryError> {
        self.names_by_code
            .read()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?
            .get(code)
            .cloned()
            .ok_or_else(|| {
                StationDirectoryError::InvalidStationOrPlatform(TrainPredictionsRequest::Station(
                    code.to_string(),
                ))
            })
    }

    pub fn platforms(
        &self,
        code: WMATAStationCode,
    ) -> Result<BTreeSet<WMATAPlatformCode>, StationDirectoryError> {
        self.platforms_by_code
            .read()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?
            .get(&code)
            .cloned()
            .ok_or(StationDirectoryError::InvalidStationOrPlatform(
                TrainPredictionsRequest::Station(code),
            ))
    }

    /// Flat list of station records to persist/export.
    pub fn as_records(
        &self,
    ) -> Result<Vec<(WMATAStationCode, String, Vec<u8>)>, StationDirectoryError> {
        let names_map = self
            .names_by_code
            .read()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;
        let platforms_map = self
            .platforms_by_code
            .read()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;

        let mut out = Vec::new();
        for (code, name) in names_map.iter() {
            let plats = platforms_map
                .get(code)
                .map(|set| set.iter().map(|p| p.0).collect())
                .unwrap_or_default();
            out.push((code.clone(), name.clone(), plats));
        }
        Ok(out)
    }
}

/// PIMS "key" is station_code + platform/group.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PimsKey {
    pub station_code: WMATAStationCode,
    pub platform: WMATAPlatformCode,
}

/// Build a PIMS key if data is consistent.
pub fn pims_key_from_prediction(
    p: &WMATATrainPrediction,
) -> Result<PimsKey, StationDirectoryError> {
    Ok(PimsKey {
        station_code: p.location_code.clone(),
        platform: WMATAPlatformCode::from_str(&p.group)?,
    })
}
