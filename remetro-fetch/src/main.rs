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

#[tokio::main]
async fn main() {
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

    // Create the shared station directory
    let station_directory = StationDirectory::new_shared();

    // Create a worker thread that fetches and processes data periodically
    let station_directory_for_fetch = station_directory.clone();
    let station_directory_for_server = station_directory.clone();

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
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    eprintln!("Ctrl-C received, stopping fetch loop.");
                    break;
                }

                _ = async {
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

                    tokio::time::sleep(config.fetch_interval).await;
                } => {}
            }
        }
    });

    let server_handle = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
            .await
            .expect("failed to bind server port");

        axum::serve(listener, create_router(station_directory_for_server))
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
                eprintln!("Ctrl-C received, shutting down server.");
            })
            .await
            .expect("server crashed");
    });

    let _ = tokio::join!(fetch_handle, server_handle);
}
