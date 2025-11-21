use crate::{
    client::WMATAClient, publisher::ReMetroPublisher, station_directory::SharedStationDirectory,
};

pub mod client;
pub mod config;
pub mod errors;
pub mod publisher;
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
    let station_directory = SharedStationDirectory::default();

    // Create a worker thread that fetches and processes data periodically
    let fetch_handle = tokio::spawn(async move {
        // Initialize WMATA client
        let client = match WMATAClient::new(
            config.api_base_url.clone(),
            &config.api_key.clone(),
            "reMetro/1.0 (+https://github.com/cernec1999/reMetro)",
            config.api_timeout,
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error initializing WMATA client: {}", e);
                return;
            }
        };

        // Initialize MQTT publisher
        let mqtt_publisher = ReMetroPublisher::new(
            &config.mqtt_broker,
            config.mqtt_port,
            &config.mqtt_client_id,
        );

        // Periodically fetch and process data
        loop {
            let predictions = match client.get_predictions_raw().await {
                Ok(preds) => preds,
                Err(e) => {
                    eprintln!("Error fetching train predictions: {}", e);
                    tokio::time::sleep(config.fetch_interval).await;
                    continue;
                }
            };

            let events = station_directory.ingest(&predictions);

            if let Err(e) = events {
                eprintln!("Error normalizing train predictions: {}", e);
            } else {
                let events = events.unwrap();

                for event in events {
                    mqtt_publisher.handle_update(event).await.unwrap();
                }
            }

            tokio::time::sleep(config.fetch_interval).await;
        }
    });

    // Create a worker thread that listens to updates from station K04.
    // let listen_handle = tokio::spawn(async move {
    //     let key = TrainPredictionsRequest::Station("K04".to_string());
    //     if let Ok(mut receiver) = station_directory_listener.subscribe(key) {
    //         // Listen for updates
    //         while receiver.changed().await.is_ok() {
    //             let update = receiver.borrow().clone();
    //             match update {
    //                 TrainUpdate::Full { trains } => {
    //                     // print trains
    //                     println!("Station K04: Full refresh needed, {} trains", trains.len());
    //                     for train in trains {
    //                         println!("\t{}", train);
    //                     }
    //                 }
    //                 TrainUpdate::Incremental { changes } => {
    //                     println!("Station K04: {} trains had minute changes", changes.len());
    //                     for change in changes {
    //                         println!(
    //                             "\tTrain {}: {} → {}",
    //                             change.train_index, change.old_minutes, change.new_minutes
    //                         );
    //                     }
    //                 }
    //             }
    //         }
    //     } else {
    //         eprintln!("Failed to subscribe to station K04");
    //     }
    // });

    // Wait for both tasks to complete (they won't, but this keeps the main thread alive)
    let _ = tokio::join!(fetch_handle);
}
