use std::{collections::BTreeSet, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, State},
    response::IntoResponse,
    routing::get,
};
use remetro_common::{Station, WMATATrackCode, predictions::api::TrainPredictionsRequest};

use crate::{errors::StationDirectoryError, station_directory::StationDirectory};

impl IntoResponse for StationDirectoryError {
    fn into_response(self) -> axum::response::Response {
        let status_code = match self {
            StationDirectoryError::InvalidStationOrTrack(_) => axum::http::StatusCode::NOT_FOUND,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = Json(serde_json::json!({
            "error": self.to_string()
        }));
        (status_code, body).into_response()
    }
}

pub fn create_router(station_directory: Arc<StationDirectory>) -> Router {
    Router::new()
        .route("/", get(|| async { "Welcome to reMetro!" }))
        .route("/health", get(health_check_handler))
        .route("/v1/unknown_aliases", get(unknown_aliases_handler))
        .route("/v1/station", get(stations_handler))
        .route("/v1/station/{station_code}", get(single_station_handler))
        .route(
            "/v1/predictions/station/{station_code}",
            get(station_predictions_handler),
        )
        .route(
            "/v1/predictions/station/{station_code}/track",
            get(station_tracks),
        )
        .route(
            "/v1/predictions/station/{station_code}/track/{track_code}",
            get(station_track_predictions_handler),
        )
        .with_state(station_directory.clone())
}

async fn health_check_handler() -> &'static str {
    "OK"
}

async fn unknown_aliases_handler(
    State(station_directory): State<Arc<StationDirectory>>,
) -> Result<Json<Vec<String>>, StationDirectoryError> {
    let aliases = station_directory.unknown_aliases()?;
    Ok(Json(aliases))
}

async fn single_station_handler(
    State(station_directory): State<Arc<StationDirectory>>,
    Path(station_code): Path<String>,
) -> Result<Json<Station>, StationDirectoryError> {
    let station = station_directory.station(station_code)?;
    Ok(Json(station))
}

async fn stations_handler(
    State(station_directory): State<Arc<StationDirectory>>,
) -> Json<Vec<Station>> {
    let stations = station_directory.all_stations();
    Json(stations)
}

async fn station_predictions_handler(
    State(station_directory): State<Arc<StationDirectory>>,
    Path(station_code): Path<String>,
) -> Result<Json<Vec<remetro_common::predictions::TrainPrediction>>, StationDirectoryError> {
    let request = TrainPredictionsRequest::Station(station_code);
    let predictions = station_directory.predictions(&request)?;
    Ok(Json(predictions))
}

async fn station_track_predictions_handler(
    State(station_directory): State<Arc<StationDirectory>>,
    Path((station_code, track_code)): Path<(String, String)>,
) -> Result<Json<Vec<remetro_common::predictions::TrainPrediction>>, StationDirectoryError> {
    // convert track_code to an int
    let track_code = track_code.parse::<u8>().unwrap_or(u8::MAX);
    let request = TrainPredictionsRequest::StationTrack(station_code, WMATATrackCode(track_code));
    let predictions = station_directory.predictions(&request)?;
    Ok(Json(predictions))
}

async fn station_tracks(
    State(station_directory): State<Arc<StationDirectory>>,
    Path(station_code): Path<String>,
) -> Result<Json<BTreeSet<WMATATrackCode>>, StationDirectoryError> {
    let tracks = station_directory.station_tracks(station_code)?;
    Ok(Json(tracks))
}
