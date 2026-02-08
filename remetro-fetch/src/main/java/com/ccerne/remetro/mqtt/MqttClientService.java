package com.ccerne.remetro.mqtt;

import org.eclipse.paho.client.mqttv3.IMqttDeliveryToken;
import org.eclipse.paho.client.mqttv3.MqttCallback;
import org.eclipse.paho.client.mqttv3.MqttClient;
import org.eclipse.paho.client.mqttv3.MqttConnectOptions;
import org.eclipse.paho.client.mqttv3.MqttException;
import org.eclipse.paho.client.mqttv3.MqttMessage;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.nio.charset.StandardCharsets;

public class MqttClientService implements AutoCloseable {
    private static final Logger logger = LoggerFactory.getLogger(MqttClientService.class);

    private final String brokerUri; // e.g. tcp://host:1883
    private final String clientId;
    private MqttClient client;

    public MqttClientService(String host, int port, String clientId) {
        this.brokerUri = String.format("tcp://%s:%d", host, port);
        this.clientId = clientId;
    }

    public void start() throws MqttException {
        logger.info("Starting MQTT client connecting to {} as {}", brokerUri, clientId);
        client = new MqttClient(brokerUri, clientId);
        MqttConnectOptions opts = new MqttConnectOptions();
        opts.setAutomaticReconnect(true);
        opts.setCleanSession(true);
        client.setCallback(new MqttCallback() {
            @Override
            public void connectionLost(Throwable cause) {
                logger.warn("MQTT connection lost", cause);
            }

            @Override
            public void messageArrived(String topic, org.eclipse.paho.client.mqttv3.MqttMessage message) {
                logger.info("Received message on {}: {}", topic, new String(message.getPayload(), StandardCharsets.UTF_8));
            }

            @Override
            public void deliveryComplete(IMqttDeliveryToken token) {
                // no-op
            }
        });
        client.connect(opts);
    }

    public void publish(String topic, String payload) {
        if (client == null || !client.isConnected()) {
            logger.warn("MQTT client not connected; dropping message to topic {}", topic);
            return;
        }
        try {
            MqttMessage msg = new MqttMessage(payload.getBytes(StandardCharsets.UTF_8));
            msg.setQos(0);
            client.publish(topic, msg);
        } catch (MqttException e) {
            logger.error("Failed to publish mqtt message", e);
        }
    }

    @Override
    public void close() {
        if (client != null && client.isConnected()) {
            try {
                client.disconnect();
            } catch (MqttException e) {
                logger.warn("Error while disconnecting MQTT client", e);
            }
        }
    }
}
