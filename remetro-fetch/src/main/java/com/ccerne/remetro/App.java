package com.ccerne.remetro;

import com.ccerne.remetro.config.TrainFetchConfig;
import com.ccerne.remetro.fetcher.TrainFetcher;
import com.ccerne.remetro.http.HttpServerService;
import com.ccerne.remetro.mqtt.MqttClientService;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.util.concurrent.atomic.AtomicReference;

public class App {
    private static final Logger logger = LoggerFactory.getLogger(App.class);

    public static void main(String[] args) throws Exception {
    TrainFetchConfig cfg = TrainFetchConfig.fromEnv();

    String mqttHost = cfg.mqttBroker.orElse("localhost");
    int mqttPort = cfg.mqttPort.orElse(1883);
    String webBind = cfg.webBindAddress.orElse("0.0.0.0:3000");

    logger.info("Loaded configuration: mqtt={}:{} bind={}", mqttHost, mqttPort, webBind);

    MqttClientService mqtt = new MqttClientService(mqttHost, mqttPort, cfg.mqttClientId);
        mqtt.start();

        AtomicReference<String> lastPayload = new AtomicReference<>();

        AtomicReference<HttpServerService> httpRef = new AtomicReference<>();
        try {
            HttpServerService http = new HttpServerService(webBind, lastPayload);
            http.start();
            httpRef.set(http);
        } catch (IOException e) {
            logger.warn("Failed to start HTTP server", e);
        }

        TrainFetcher fetcher = new TrainFetcher(cfg, mqtt, lastPayload);
        Thread fetchThread = new Thread(fetcher, "train-fetcher");
        fetchThread.start();

        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            logger.info("Shutdown requested");
            fetchThread.interrupt();
            try { fetchThread.join(2000); } catch (InterruptedException ignored) {}
            HttpServerService http = httpRef.get();
            if (http != null) http.stop();
            mqtt.close();
        }));

        // Keep main alive while fetcher runs
        try {
            fetchThread.join();
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
    }
}

