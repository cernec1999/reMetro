use std::time::Duration;

use remetro_common::predictions::api::TrainPredictionsRequest;

use crate::{errors::PublisherError, station_directory::DirectoryEvent};

pub struct ReMetroPublisher {
    client: rumqttc::AsyncClient,
}

impl ReMetroPublisher {
    pub fn new(broker: &str, port: u16, client_id: &str) -> Self {
        let mut mqttoptions = rumqttc::MqttOptions::new(client_id, broker, port);
        mqttoptions.set_keep_alive(Duration::from_secs(5));

        let (client, mut eventloop) = rumqttc::AsyncClient::new(mqttoptions, 10);

        // Spawn a task to handle the event loop
        tokio::spawn(async move {
            loop {
                let _ = eventloop.poll().await;
            }
        });

        ReMetroPublisher { client }
    }

    pub async fn handle_update(&self, update: DirectoryEvent) -> Result<(), PublisherError> {
        let topic = match update.key {
            TrainPredictionsRequest::Station(station_code) => {
                format!("reMetro/v1/predictions/station/{}", station_code)
            }
            TrainPredictionsRequest::StationPlatform(station_code, platform_code) => {
                format!(
                    "reMetro/v1/predictions/station/{}/platform/{}",
                    station_code, platform_code
                )
            }
        };

        let payload = serde_json::to_vec(&update.update)?;

        Ok(self
            .client
            .publish(topic, rumqttc::QoS::AtLeastOnce, false, payload)
            .await?)
    }
}
