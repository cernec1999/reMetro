use std::{
    collections::{BTreeSet, HashMap, HashSet},
    str::FromStr,
    sync::{Arc, RwLock},
};

use remetro_common::{
    Address, Location, Station, WMATAStationCode, WMATATrackCode,
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
    types::{
        WMATAAliases, WMATAStationInfo, WMATAStationInfoResponse, WMATATrainPrediction,
        WMATATrainPredictionResponse,
    },
    utils::SortedVec,
};

/// Normalize a single raw prediction into the `TrainPrediction` type.
fn to_train(
    p: &WMATATrainPrediction,
    track: WMATATrackCode,
) -> Result<TrainPrediction, StationDirectoryError> {
    let cars = match &p.car {
        Some(s) => TrainCars::from_str(s)?,
        None => TrainCars::Unknown,
    };

    Ok(TrainPrediction {
        track,
        line: TrainLine::from_str(&p.line)?,
        cars,
        destination: p.destination.clone(),
        min: TrainMins::from_str(&p.min)?,
    })
}

/// If line_code is empty, do nothing. Otherwise, insert the line into the set.
fn insert_line_if_present(
    lines: &mut HashSet<TrainLine>,
    line_code: &Option<String>,
) -> Result<(), StationDirectoryError> {
    if let Some(code) = line_code
        && !code.is_empty()
    {
        let line = TrainLine::from_str(code)?;
        lines.insert(line);
    }
    Ok(())
}

/// Normalize a single station from WMATA response into `Station`.
fn to_station(station: &WMATAStationInfo) -> Result<Station, StationDirectoryError> {
    let mut lines = HashSet::new();
    insert_line_if_present(&mut lines, &station.line_code_1)?;
    insert_line_if_present(&mut lines, &station.line_code_2)?;
    insert_line_if_present(&mut lines, &station.line_code_3)?;
    insert_line_if_present(&mut lines, &station.line_code_4)?;

    let mut linked_stations = HashSet::new();
    if let Some(s) = &station.station_together_1
        && !s.is_empty()
    {
        linked_stations.insert(s.clone());
    }
    if let Some(s) = &station.station_together_2
        && !s.is_empty()
    {
        linked_stations.insert(s.clone());
    }

    Ok(Station {
        code: station.code.clone(),
        name: station.name.clone(),
        aliases: HashSet::new(),
        linked_stations,
        lines,
        location: Location {
            latitude: station.lat,
            longitude: station.lon,
            address: Address {
                street: station.address.street.clone(),
                city: station.address.city.clone(),
                state: station.address.state.clone(),
                zip: station.address.zip.clone(),
            },
        },
    })
}

/// Represents a directory event, which is emitted when predictions change.
#[derive(Debug, Clone)]
pub struct DirectoryEvent {
    pub key: TrainPredictionsRequest,
    pub update: FullTrainUpdate,
}

/// A thread-safe directory of WMATA station information plus fast prediction lookups.
///
/// Single-writer, multi-reader semantics via RwLocks.
/// Use `StationDirectory::new_shared()` for cross-thread sharing.
#[derive(Debug)]
pub struct StationDirectory {
    station_by_code: RwLock<HashMap<WMATAStationCode, Station>>,
    tracks_by_code: RwLock<HashMap<WMATAStationCode, BTreeSet<WMATATrackCode>>>,
    trains_by_track: RwLock<HashMap<TrainPredictionsRequest, Vec<TrainPrediction>>>,
    /// Global alias set for warnings + dedupe.
    aliases: RwLock<HashSet<String>>,
    /// Unknown aliases seen so far.
    unknown_aliases: RwLock<HashSet<String>>,
}

/// Thread-safe, shared reference to a StationDirectory.
pub type SharedStationDirectory = Arc<StationDirectory>;

impl Default for StationDirectory {
    fn default() -> Self {
        Self {
            station_by_code: RwLock::new(HashMap::new()),
            tracks_by_code: RwLock::new(HashMap::new()),
            trains_by_track: RwLock::new(HashMap::new()),
            aliases: RwLock::new(HashSet::new()),
            unknown_aliases: RwLock::new(HashSet::new()),
        }
    }
}

impl StationDirectory {
    /// Create a new shared StationDirectory wrapped in Arc for thread-safe sharing.
    pub fn new_shared() -> SharedStationDirectory {
        Arc::new(Self::default())
    }

    fn load_static_aliases() -> Result<WMATAAliases, StationDirectoryError> {
        Ok(serde_json::from_str(include_str!("../res/aliases.json"))?)
    }

    /// Ingest raw WMATA stations and update the directory.
    pub fn ingest_stations(
        &self,
        resp: &WMATAStationInfoResponse,
    ) -> Result<(), StationDirectoryError> {
        let station_aliases = Self::load_static_aliases()?;

        let mut names_map = self
            .station_by_code
            .write()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;

        let mut aliases_global = self
            .aliases
            .write()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;

        for station in &resp.stations {
            let mut normalized = to_station(station)?;

            // Always include the official name as an alias (and global alias)
            normalized.aliases.insert(normalized.name.clone());
            aliases_global.insert(normalized.name.clone());

            // Add any aliases from the static mapping
            if let Some(aliases) = station_aliases.station_aliases.get(&station.code) {
                for alias in aliases {
                    if !alias.is_empty() {
                        normalized.aliases.insert(alias.clone());
                        aliases_global.insert(alias.clone());
                    }
                }
            }

            names_map.insert(normalized.code.clone(), normalized);
        }

        // Add the no-passenger aliases to the global set
        for alias in &station_aliases.no_passenger_aliases {
            if !alias.is_empty() {
                aliases_global.insert(alias.clone());
            }
        }

        Ok(())
    }

    /// Ingest raw WMATA train prediction response and update directory.
    ///
    /// Returns a list of DirectoryEvents for keys whose prediction lists changed.
    pub fn ingest_predictions(
        &self,
        resp: &WMATATrainPredictionResponse,
    ) -> Result<Vec<DirectoryEvent>, StationDirectoryError> {
        // Take all needed write locks up front in a consistent order to avoid deadlocks.
        let mut aliases_global = self
            .aliases
            .write()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;

        let mut names_map = self
            .station_by_code
            .write()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;

        let mut tracks_map = self
            .tracks_by_code
            .write()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;

        let mut trains_by_track = self
            .trains_by_track
            .write()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;

        // Snapshot old state for diffing.
        let old_trains_by_track = trains_by_track.clone();

        trains_by_track.clear();
        let mut station_trains: HashMap<WMATAStationCode, Vec<TrainPrediction>> = HashMap::new();

        for p in &resp.trains {
            // Enrich destination aliases if we have a code.
            if let Some(dest_code) = &p.destination_code {
                if let Some(station) = names_map.get_mut(dest_code) {
                    if let Some(dest_name) = &p.destination_name
                        && !dest_name.is_empty()
                    {
                        station.aliases.insert(dest_name.clone());
                        aliases_global.insert(dest_name.clone());
                    }
                } else {
                    eprintln!(
                        "Warning: destination_code '{}' not found in station directory",
                        dest_code
                    );
                }
            } else if let Some(dest_name) = &p.destination_name
                && !dest_name.is_empty()
                && !aliases_global.contains(dest_name)
            {
                eprintln!(
                    "Warning: destination_name '{}' has no associated destination_code",
                    dest_name
                );
                self.unknown_aliases
                    .write()
                    .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?
                    .insert(dest_name.clone());
            }

            let track = WMATATrackCode::from_str(&p.group)?;
            tracks_map
                .entry(p.location_code.clone())
                .or_default()
                .insert(track);

            let train = to_train(p, track)?;

            // Station-level collection
            station_trains
                .entry(p.location_code.clone())
                .or_default()
                .insert_sorted(train.clone());

            // Track-level lookup
            let track_key = TrainPredictionsRequest::StationTrack(p.location_code.clone(), track);
            trains_by_track
                .entry(track_key)
                .or_default()
                .insert_sorted(train);
        }

        // Populate station-level aggregated keys
        for (station_code, trains) in station_trains {
            let station_key = TrainPredictionsRequest::Station(station_code);
            trains_by_track.insert(station_key, trains);
        }

        // Drop write locks before diff/notify (read-only from here on).
        drop(trains_by_track);
        drop(tracks_map);
        drop(names_map);
        drop(aliases_global);

        self.check_and_notify_prediction_changes(resp, old_trains_by_track)
    }

    /// Emit events only for keys whose train lists changed.
    fn check_and_notify_prediction_changes(
        &self,
        resp: &WMATATrainPredictionResponse,
        old_trains_by_track: HashMap<TrainPredictionsRequest, Vec<TrainPrediction>>,
    ) -> Result<Vec<DirectoryEvent>, StationDirectoryError> {
        let mut events: Vec<DirectoryEvent> = Vec::new();

        let current_trains = self
            .trains_by_track
            .read()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;

        let mut affected_keys = HashSet::new();
        for prediction in &resp.trains {
            affected_keys.insert(TrainPredictionsRequest::Station(
                prediction.location_code.clone(),
            ));
            if let Ok(track) = WMATATrackCode::from_str(&prediction.group) {
                affected_keys.insert(TrainPredictionsRequest::StationTrack(
                    prediction.location_code.clone(),
                    track,
                ));
            }
        }

        for key in affected_keys {
            if let Some(trains) = current_trains.get(&key) {
                let changed = old_trains_by_track
                    .get(&key)
                    .map(|old| old != trains)
                    .unwrap_or(true);

                if changed {
                    let update = FullTrainUpdate {
                        trains: trains.iter().take(MAX_TRAINS_PER_UPDATE).cloned().collect(),
                    };
                    events.push(DirectoryEvent { key, update });
                }
            }
        }

        Ok(events)
    }

    /// Return the list unknown aliases.
    pub fn unknown_aliases(&self) -> Result<Vec<String>, StationDirectoryError> {
        let unknown = self
            .unknown_aliases
            .read()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?;
        Ok(unknown.iter().cloned().collect())
    }

    /// Get all stations as records.
    pub fn all_stations(&self) -> Vec<Station> {
        self.station_by_code
            .read()
            .map(|map| map.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get a station from its station code.
    pub fn station(&self, code: WMATAStationCode) -> Result<Station, StationDirectoryError> {
        self.station_by_code
            .read()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?
            .get(&code)
            .cloned()
            .ok_or_else(|| {
                StationDirectoryError::InvalidStationOrTrack(TrainPredictionsRequest::Station(
                    code.to_string(),
                ))
            })
    }

    /// Get a list of track codes associated with a station code.
    pub fn station_tracks(
        &self,
        code: WMATAStationCode,
    ) -> Result<BTreeSet<WMATATrackCode>, StationDirectoryError> {
        self.tracks_by_code
            .read()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?
            .get(&code)
            .cloned()
            .ok_or(StationDirectoryError::InvalidStationOrTrack(
                TrainPredictionsRequest::Station(code),
            ))
    }

    /// Get predictions for a given station or station+track.
    pub fn predictions(
        &self,
        key: &TrainPredictionsRequest,
    ) -> Result<Vec<TrainPrediction>, StationDirectoryError> {
        self.trains_by_track
            .read()
            .map_err(|e| StationDirectoryError::RwLockPoisonError(e.to_string()))?
            .get(key)
            .cloned()
            .ok_or_else(|| StationDirectoryError::InvalidStationOrTrack(key.clone()))
    }
}

/// PIMS "key" is station_code + track/group.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PimsKey {
    pub station_code: WMATAStationCode,
    pub track: WMATATrackCode,
}

/// Build a PIMS key if data is consistent.
pub fn pims_key_from_prediction(
    p: &WMATATrainPrediction,
) -> Result<PimsKey, StationDirectoryError> {
    Ok(PimsKey {
        station_code: p.location_code.clone(),
        track: WMATATrackCode::from_str(&p.group)?,
    })
}
