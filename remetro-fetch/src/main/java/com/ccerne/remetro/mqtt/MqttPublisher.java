package com.ccerne.remetro.mqtt;

import com.ccerne.remetro.config.TrainFetchConfig;
import com.ccerne.remetro.proto.TrainPredictionResponse;
import com.ccerne.remetro.station.PlatformKey;
import com.ccerne.remetro.station.PredictionChangeListener;
import jakarta.annotation.PreDestroy;
import org.eclipse.paho.client.mqttv3.*;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

/**
 * Publishes protobuf-encoded prediction payloads to MQTT whenever the
 * StationDirectory detects a change.
 *
 * Topic structure:
 *   reMetro/v1/wmata/station/{wmataCode}/platform/{group}  — per-platform
 *   reMetro/v1/wmata/station/{primaryCode}/all             — whole station complex
 *
 * If MQTT_BROKER is not configured the publisher is a no-op (dev-mode friendly).
 */
@Component
public class MqttPublisher implements PredictionChangeListener {

    private static final Logger log = LoggerFactory.getLogger(MqttPublisher.class);

    private static final int QOS = 1;
    private static final String TOPIC_PREFIX = "reMetro/v1/wmata/station/";

    private final IMqttClient client; // null when broker not configured

    public MqttPublisher(TrainFetchConfig config) {
        IMqttClient c = null;
        if (config.mqttBroker.isPresent()) {
            String brokerUri = "tcp://%s:%d".formatted(
                    config.mqttBroker.get(),
                    config.mqttPort.orElse(1883));
            try {
                c = new MqttClient(brokerUri, config.mqttClientId);
                MqttConnectOptions opts = new MqttConnectOptions();
                opts.setAutomaticReconnect(true);
                opts.setCleanSession(true);
                opts.setConnectionTimeout(5);
                c.connect(opts);
                log.info("MQTT publisher connected to {}", brokerUri);
            } catch (MqttException e) {
                log.error("Failed to connect to MQTT broker {}: {}", brokerUri, e.getMessage());
                c = null;
            }
        } else {
            log.warn("REMETRO_MQTT_BROKER not set — MQTT publishing disabled");
        }
        this.client = c;
    }

    @Override
    public void onPlatformChanged(PlatformKey key, TrainPredictionResponse response) {
        String topic = TOPIC_PREFIX + key.wmataCode() + "/platform/" + key.group();
        publish(topic, response.toByteArray());
    }

    @Override
    public void onStationChanged(String primaryCode, TrainPredictionResponse response) {
        String topic = TOPIC_PREFIX + primaryCode + "/all";
        publish(topic, response.toByteArray());
    }

    private void publish(String topic, byte[] payload) {
        if (client == null) return;
        try {
            MqttMessage msg = new MqttMessage(payload);
            msg.setQos(QOS);
            msg.setRetained(false);
            client.publish(topic, msg);
            log.debug("Published {} bytes to {}", payload.length, topic);
        } catch (MqttException e) {
            log.error("Failed to publish to {}: {}", topic, e.getMessage());
        }
    }

    @PreDestroy
    void shutdown() {
        if (client != null && client.isConnected()) {
            try {
                client.disconnect();
                client.close();
                log.info("MQTT publisher disconnected");
            } catch (MqttException e) {
                log.warn("Error disconnecting MQTT client", e);
            }
        }
    }
}
