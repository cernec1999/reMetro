use metro_common::predictions::{TrainUpdate, WMATAPlatformCode};

use crate::{client::WMATAClient, station_directory::{SharedStationDirectory, SubscriptionKey}};

pub mod client;
pub mod config;
pub mod errors;
pub mod station_directory;
pub mod types;
pub mod utils;

#[tokio::main]
async fn main() {
    // Read config and gracefully exit on error
    let config = match config::read_env_vars() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error reading configuration from environment variables: {}", e);
            std::process::exit(1);
        }
    };

    // Create the shared station directory
    let station_directory = SharedStationDirectory::default();
    
    // Clone for the listener task
    let station_directory_listener = station_directory.clone();

    // Create a worker thread that fetches and processes data periodically
    let fetch_handle = tokio::spawn(async move {
        // Initialize WMATA client
        let client = match WMATAClient::new(config.base_url.clone(), &config.key.clone(), "reMetro/1.0 (+https://github.com/cernec1999/reMetro)", config.timeout) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error initializing WMATA client: {}", e);
                return;
            }
        };

        // Periodically fetch and process data
        loop {
            let predictions = match client.get_predictions_raw().await {
                Ok(preds) => preds,
                Err(e) => {
                    eprintln!("Error fetching train predictions: {}", e);
                    continue;
                }
            };

            if let Err(e) = station_directory
                .ingest(&predictions)
            {
                eprintln!("Error normalizing train predictions: {}", e);
            }
            tokio::time::sleep(config.fetch_interval).await;
        }
    });

    // Create a worker thread that listens to updates from station K04.
    let listen_handle = tokio::spawn(async move {
        let key = SubscriptionKey::StationPlatform("K04".to_string(), WMATAPlatformCode(2));
        if let Ok(mut receiver) = station_directory_listener.subscribe(key) {
            // Listen for updates
            while receiver.changed().await.is_ok() {
                let update = receiver.borrow().clone();
                match update {
                    TrainUpdate::Full { trains } => {
                        // print trains
                        println!("Station K04: Full refresh needed, {} trains", trains.len());
                        for train in trains {
                            println!("\t{}", train);
                        }
                    }
                    TrainUpdate::Incremental { changes } => {
                        println!("Station K04: {} trains had minute changes", changes.len());
                        for change in changes {
                            println!("\tTrain {}: {} → {}", change.train_index, change.old_minutes, change.new_minutes);
                        }
                    }
                }
            }
        } else {
            eprintln!("Failed to subscribe to station K04");
        }
    });

    // Wait for both tasks to complete (they won't, but this keeps the main thread alive)
    let _ = tokio::join!(fetch_handle, listen_handle);
}