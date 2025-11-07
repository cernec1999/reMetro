use std::{
    collections::{BTreeSet, HashMap},
    str::FromStr,
    sync::{Arc, RwLock},
};

use tokio::sync::watch;

use metro_common::predictions::{Train, TrainCars, TrainLine, TrainMins, TrainMinuteChange, TrainUpdate, WMATAPlatformCode, WMATAStationCode};

use crate::{
    errors::StationDirectoryError,
    types::{WMATATrainPrediction, WMATATrainPredictionResponse}, utils::SortedVec,
};

/// Subscription key for monitoring specific station/platform combinations
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SubscriptionKey {
    /// Monitor all platforms for a specific station
    Station(WMATAStationCode),
    /// Monitor a specific platform at a specific station
    StationPlatform(WMATAStationCode, WMATAPlatformCode),
}

impl std::fmt::Display for SubscriptionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubscriptionKey::Station(station) => write!(f, "Station({})", station),
            SubscriptionKey::StationPlatform(station, platform) => {
                write!(f, "StationPlatform({}, {})", station, platform)
            }
        }
    }
}

/// Normalize a single raw prediction into the `Train` type.
pub fn to_train(p: &WMATATrainPrediction, platform: WMATAPlatformCode) -> Result<Train, StationDirectoryError> {
    let cars = match &p.car {
        Some(s) if s == "---" || s == "-" || s.is_empty() => TrainCars::Unknown,
        Some(s) => TrainCars::from_str(s)?,
        None => TrainCars::Unknown,
    };

    Ok(Train {
        platform: platform,
        line: TrainLine::from_str(&p.line)?,
        cars,
        destination: p.destination.clone(),
        min: TrainMins::from_str(&p.min)?,
    })
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
    // Individual watch channels for specific subscriptions
    subscription_channels: RwLock<HashMap<SubscriptionKey, watch::Sender<TrainUpdate>>>,
    // Store previous prediction state for change comparison
    previous_predictions: RwLock<HashMap<SubscriptionKey, Vec<Train>>>,
    // Efficient O(1) lookup structure for normalized train data
    trains_by_platform: RwLock<HashMap<SubscriptionKey, Vec<Train>>>,
}

/// Thread-safe, shared reference to a StationDirectory.
/// This is the recommended way to share a StationDirectory across threads.
pub type SharedStationDirectory = Arc<StationDirectory>;

impl Default for StationDirectory {
    fn default() -> Self {
        Self {
            names_by_code: RwLock::new(HashMap::new()),
            platforms_by_code: RwLock::new(HashMap::new()),
            subscription_channels: RwLock::new(HashMap::new()),
            previous_predictions: RwLock::new(HashMap::new()),
            trains_by_platform: RwLock::new(HashMap::new()),
        }
    }
}

impl StationDirectory {
    /// Create a new shared StationDirectory wrapped in Arc for thread-safe sharing.
    pub fn new_shared() -> SharedStationDirectory {
        Arc::new(Self::default())
    }

    /// Subscribe to data change events for a specific station or platform.
    /// Returns a receiver that will be notified when train prediction data changes
    /// for the specified subscription key.
    /// 
    /// # Example
    /// ```rust,no_run
    /// let station_dir = StationDirectory::new_shared();
    /// 
    /// // Subscribe to all platforms for station A01
    /// let mut station_receiver = station_dir.subscribe(SubscriptionKey::Station("A01".to_string())).unwrap();
    /// 
    /// // Subscribe to specific platform for station A01
    /// let platform_key = SubscriptionKey::StationPlatform("A01".to_string(), WMATAPlatformCode(1));
    /// let mut platform_receiver = station_dir.subscribe(platform_key).unwrap();
    /// 
    /// // In an MQTT client task:
    /// tokio::spawn(async move {
    ///     while station_receiver.changed().await.is_ok() {
    ///         let update = station_receiver.borrow().clone();
    ///         match update {
    ///             TrainUpdate::FullRefresh { new_train_count } => {
    ///                 println!("Station A01: Full refresh needed, {} trains", new_train_count);
    ///                 // Trigger complete display update
    ///                 let trains = station_dir.get_top_trains("A01".to_string(), None, 3)?;
    ///                 // Update display with new trains
    ///             },
    ///             TrainUpdate::MinutesChanged { changes } => {
    ///                 println!("Station A01: {} trains had minute changes", changes.len());
    ///                 // Update only the minute displays for specific trains
    ///                 for change in changes {
    ///                     println!("Train {}: {} → {}", change.train_index, change.old_minutes, change.new_minutes);
    ///                 }
    ///             }
    ///         }
    ///     }
    /// });
    /// ```
    pub fn subscribe(&self, key: SubscriptionKey) -> Result<watch::Receiver<TrainUpdate>, StationDirectoryError> {
        let mut channels = self.subscription_channels.write()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;
        
        // Create a new channel if it doesn't exist, or clone the receiver from existing channel
        let sender = channels.entry(key).or_insert_with(|| {
            let (sender, _receiver) = watch::channel(TrainUpdate::Full {
                trains: Vec::new(),
            });
            sender
        });
        
        Ok(sender.subscribe())
    }

    pub fn ingest(&self, resp: &WMATATrainPredictionResponse) -> Result<(), StationDirectoryError> {
        // Update basic station/platform directory and build efficient lookup structures
        {
            let mut names_map = self.names_by_code.write()
                .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;
            let mut platforms_map = self.platforms_by_code.write()
                .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;
            let mut trains_by_platform = self.trains_by_platform.write()
                .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;
            
            // Clear previous train data
            trains_by_platform.clear();
            
            // Group trains by station for station-level subscriptions
            let mut station_trains: HashMap<WMATAStationCode, Vec<Train>> = HashMap::new();
            
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
                let platform_key = SubscriptionKey::StationPlatform(p.location_code.clone(), platform);
                trains_by_platform
                    .entry(platform_key)
                    .or_default()
                    .insert_sorted(train);
            }
            
            // Now populate station-level subscription keys with aggregated data
            for (station_code, trains) in station_trains {
                let station_key = SubscriptionKey::Station(station_code);
                trains_by_platform.insert(station_key, trains);
            }
        }

        // Check for prediction changes and notify subscribers with detailed change information
        self.check_and_notify_prediction_changes(resp)?;
        
        Ok(())
    }

    /// Check for prediction changes and notify subscribers with detailed change information
    fn check_and_notify_prediction_changes(&self, resp: &WMATATrainPredictionResponse) -> Result<(), StationDirectoryError> {
        let mut previous_predictions = self.previous_predictions.write()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;
        let current_trains = self.trains_by_platform.read()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;
        let channels = self.subscription_channels.read()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;
        
        // Collect unique stations and station-platform combinations from the response
        let mut affected_keys = std::collections::HashSet::new();
        
        for prediction in &resp.trains {
            affected_keys.insert(SubscriptionKey::Station(prediction.location_code.clone()));
            if let Ok(platform) = WMATAPlatformCode::from_str(&prediction.group) {
                affected_keys.insert(SubscriptionKey::StationPlatform(prediction.location_code.clone(), platform));
            }
        }
        
        // Check each affected subscription key for changes
        for key in affected_keys {
            if let Some(sender) = channels.get(&key) {
                if let Some(new_trains) = current_trains.get(&key) {
                    let old_trains = previous_predictions.get(&key).cloned().unwrap_or_default();
                    
                    let update = self.determine_update_type(&old_trains, new_trains);
                    if let Some(update) = update {
                        let _ = sender.send(update); // Ignore send errors (no active receivers)
                    }

                    // Update stored predictions for next comparison
                    previous_predictions.insert(key, new_trains.clone());
                }
            }
        }
        
        Ok(())
    }

    /// Determine what type of update occurred by comparing old and new train data
    fn determine_update_type(&self, old_trains: &[Train], new_trains: &[Train]) -> Option<TrainUpdate> {
        // If the number of trains changed, it's a full refresh
        if old_trains.len() != new_trains.len() {
            return Some(TrainUpdate::Full {
                trains: new_trains.to_vec(),
            });
        }
        
        let mut minute_changes = Vec::new();
        
        // Check each train for changes
        for (index, (old_train, new_train)) in old_trains.iter().zip(new_trains.iter()).enumerate() {
            // If line, destination, or car count changed, it's a full refresh
            if old_train.line != new_train.line 
                || old_train.destination != new_train.destination 
                || old_train.cars != new_train.cars {
                return Some(TrainUpdate::Full {
                    trains: new_trains.to_vec(),
                });
            }
            
            // If only minutes changed, track the change
            if old_train.min != new_train.min {
                minute_changes.push(TrainMinuteChange {
                    train_index: index,
                    new_minutes: new_train.min,
                    old_minutes: old_train.min,
                });
            }
        }
        
        // If we have minute changes, return incremental update
        if !minute_changes.is_empty() {
            Some(TrainUpdate::Incremental { changes: minute_changes })
        } else {
            // No changes detected (this shouldn't happen if we're called correctly)
            None
        }
    }

    pub fn station_name(&self, code: &str) -> Result<String, StationDirectoryError> {
        self.names_by_code
            .read()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?
            .get(code)
            .cloned()
            .ok_or_else(|| StationDirectoryError::InvalidPlatformKey(SubscriptionKey::Station(code.to_string())))
    }

    pub fn platforms(&self, code: WMATAStationCode) -> Result<BTreeSet<WMATAPlatformCode>, StationDirectoryError> {
        self.platforms_by_code
            .read()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?
            .get(&code)
            .cloned()
            .ok_or_else(|| StationDirectoryError::InvalidPlatformKey(SubscriptionKey::Station(code)))
    }

    /// Flat list of station records to persist/export.
    pub fn as_records(&self) -> Result<Vec<(WMATAStationCode, String, Vec<u8>)>, StationDirectoryError> {
        let names_map = self.names_by_code.read()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;
        let platforms_map = self.platforms_by_code.read()
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
pub fn pims_key_from_prediction(p: &WMATATrainPrediction) -> Result<PimsKey, StationDirectoryError> {
    Ok(PimsKey {
        station_code: p.location_code.clone(),
        platform: WMATAPlatformCode::from_str(&p.group)?,
    })
}
