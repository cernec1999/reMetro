use tokio_util::sync::CancellationToken;

use crate::{
    client::WMATAClient, publisher::ReMetroPublisher, router::create_router,
    station_directory::StationDirectory,
};

pub mod client;
pub mod config;
pub mod errors;
pub mod publisher;
pub mod router;
pub mod station_directory;
pub mod types;
pub mod utils;

/// Wait for SIGTERM signal (Unix-only)
#[cfg(unix)]
async fn wait_for_sigterm() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to register SIGTERM handler");
    sigterm.recv().await;
}

/// Fallback for non-Unix systems (Windows)
#[cfg(not(unix))]
async fn wait_for_sigterm() {
    // On Windows, we'll just wait indefinitely since SIGTERM isn't available
    std::future::pending::<()>().await;
}

#[tokio::main]
async fn main() {
    // Create a cancellation token that will be shared between tasks; it will trigger on SIGTERM
    // and SIGINT
    let cancel_token = CancellationToken::new();

    // Read config and gracefully exit on error
    let config = match config::read_env_vars() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!(
                "Error reading configuration from environment variables: {}",
                e
            );
            std::process::exit(1);
        }
    };

    // Listen for SIGTERM and SIGINT to trigger cancellation
    let cancel_token_for_signal = cancel_token.clone();
    tokio::spawn(async move {
        tokio::select! {
            _ = wait_for_sigterm() => {
                eprintln!("Received termination signal (SIGTERM). Initiating graceful shutdown...");
            }
            _ = tokio::signal::ctrl_c() => {
                eprintln!("Received interrupt signal (SIGINT). Initiating graceful shutdown...");
            }
        }
        cancel_token_for_signal.cancel();
    });

    // Create the shared station directory
    let station_directory = StationDirectory::new_shared();

    // Create a worker thread that fetches and processes data periodically
    let station_directory_for_fetch = station_directory.clone();
    let station_directory_for_server = station_directory.clone();

    let fetch_handle_cancel_token = cancel_token.clone();
    let fetch_handle = tokio::spawn(async move {
        // Initialize WMATA client
        let client = match WMATAClient::new(
            config.wmata_api_base_url.clone(),
            &config.wmata_api_key,
            "reMetro/1.0 (+https://github.com/cernec1999/reMetro)",
            config.wmata_api_timeout,
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error initializing WMATA client: {}", e);
                return;
            }
        };

        // Populate station directory initially
        let stations = match client.get_stations_raw().await {
            Ok(sts) => sts,
            Err(e) => {
                eprintln!("Error fetching WMATA stations: {}", e);
                return;
            }
        };

        if let Err(e) = station_directory_for_fetch.ingest_stations(&stations) {
            eprintln!("Error populating station directory: {}", e);
            return;
        }

        // Initialize MQTT publisher
        let mqtt_publisher = ReMetroPublisher::new(
            &config.mqtt_broker,
            config.mqtt_port,
            &config.mqtt_client_id,
        );

        // Periodically fetch and process data until Ctrl-C
        loop {
            let predictions = match client.get_predictions_raw().await {
                Ok(preds) => preds,
                Err(e) => {
                    eprintln!("Error fetching train predictions: {}", e);
                    tokio::time::sleep(config.fetch_interval).await;
                    return;
                }
            };

            match station_directory_for_fetch.ingest_predictions(&predictions) {
                Ok(events) => {
                    for event in events {
                        if let Err(e) = mqtt_publisher.handle_update(event).await {
                            eprintln!("Error publishing update: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error normalizing train predictions: {}", e);
                }
            }

            // Gracefully exit if cancellation is requested.
            if fetch_handle_cancel_token.is_cancelled() {
                eprintln!("Stopping fetch loop due to shutdown signal.");
                break;
            }

            tokio::time::sleep(config.fetch_interval).await;
        }
    });

    let cancel_token_for_axum = cancel_token.clone();
    let server_handle = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(config.web_bind_address.clone())
            .await
            .expect("failed to bind server port");

        axum::serve(listener, create_router(station_directory_for_server))
            .with_graceful_shutdown(async move {
                cancel_token_for_axum.cancelled().await;
            })
            .await
            .expect("server crashed");
    });

    let _ = tokio::join!(fetch_handle, server_handle);
}
