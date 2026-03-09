package com.ccerne.remetro.config;

import io.github.cdimascio.dotenv.Dotenv;

import java.net.URI;
import java.time.Duration;
import java.util.Objects;
import java.util.Optional;
import java.util.function.Function;

/**
 * Configuration read from environment variables (prefixed with REMETRO_). Minimal parsing and
 * sensible defaults. This class uses {@link Optional} for values that may be absent and enforces
 * required values at construction time (for example, the WMATA API key).
 */
public final class TrainFetchConfig {
    public final URI wmataApiBaseUrl;
    public final String wmataApiKey; // required, non-null
    public final Duration wmataApiTimeout;
    public final Duration fetchInterval;

    public final Optional<String> mqttBroker;
    public final Optional<Integer> mqttPort;
    public final String mqttClientId;

    public final Optional<String> webBindAddress; // host:port

    private TrainFetchConfig(
            URI wmataApiBaseUrl,
            String wmataApiKey,
            Duration wmataApiTimeout,
            Duration fetchInterval,
            Optional<String> mqttBroker,
            Optional<Integer> mqttPort,
            String mqttClientId,
            Optional<String> webBindAddress) {
        this.wmataApiBaseUrl = Objects.requireNonNull(wmataApiBaseUrl, "wmataApiBaseUrl");
        this.wmataApiKey = Objects.requireNonNull(wmataApiKey, "wmataApiKey");
        this.wmataApiTimeout = Objects.requireNonNull(wmataApiTimeout, "wmataApiTimeout");
        this.fetchInterval = Objects.requireNonNull(fetchInterval, "fetchInterval");
        this.mqttBroker = mqttBroker != null ? mqttBroker : Optional.empty();
        this.mqttPort = mqttPort != null ? mqttPort : Optional.empty();
        this.mqttClientId = Objects.requireNonNull(mqttClientId, "mqttClientId");
        this.webBindAddress = webBindAddress != null ? webBindAddress : Optional.empty();
    }

    private static Duration parseDurationOrDefault(String raw, Duration def) {
        if (raw == null || raw.trim().isEmpty()) return def;
        String t = raw.trim();
        try {
            // Accept an integer number of seconds
            long secs = Long.parseLong(t);
            return Duration.ofSeconds(secs);
        } catch (NumberFormatException ex) {
            // Try ISO-8601 duration
            try {
                return Duration.parse(t);
            } catch (Exception e) {
                return def;
            }
        }
    }

    public static TrainFetchConfig fromEnv() {
        Dotenv dotenv = Dotenv.configure().ignoreIfMissing().load();
        // Prefer explicit REMETRO_ prefixed variables; fallback to plain env
        Function<String, String> get = (k) -> {
            String v = dotenv.get("REMETRO_" + k);
            if (v == null) v = System.getenv("REMETRO_" + k);
            if (v == null) v = dotenv.get(k);
            if (v == null) v = System.getenv(k);
            return v;
        };

        try {
            String base = get.apply("WMATA_API_BASE_URL");
            if (base == null || base.isBlank()) {
                base = "https://api.wmata.com";
            }
            URI baseUri = URI.create(base);


            String key = get.apply("WMATA_API_KEY");
            if (key == null || key.isBlank()) {
                throw new IllegalStateException("WMATA_API_KEY (REMETRO_WMATA_API_KEY) is required");
            }

            Duration timeout = parseDurationOrDefault(get.apply("WMATA_API_TIMEOUT"), Duration.ofSeconds(5));
            Duration fetchInterval = parseDurationOrDefault(get.apply("FETCH_INTERVAL"), Duration.ofSeconds(5));

            String mqttBrokerRaw = get.apply("MQTT_BROKER");
            Optional<String> mqttBroker = (mqttBrokerRaw == null || mqttBrokerRaw.isBlank()) ? Optional.empty() : Optional.of(mqttBrokerRaw.trim());

            String mqttPortRaw = get.apply("MQTT_PORT");
            Optional<Integer> mqttPort = Optional.empty();
            if (mqttPortRaw != null && !mqttPortRaw.isBlank()) {
                try { mqttPort = Optional.of(Integer.parseInt(mqttPortRaw.trim())); } catch (NumberFormatException ignored) {}
            }

            String mqttClientId = get.apply("MQTT_CLIENT_ID");
            if (mqttClientId == null || mqttClientId.isBlank()) mqttClientId = "reMetroClient";

            String webBind = get.apply("WEB_BIND_ADDRESS");
            Optional<String> webBindOpt = (webBind == null || webBind.isBlank()) ? Optional.empty() : Optional.of(webBind.trim());

            return new TrainFetchConfig(baseUri, key.trim(), timeout, fetchInterval, mqttBroker, mqttPort, mqttClientId, webBindOpt);
        } catch (Exception ex) {
            throw new RuntimeException("Failed to read configuration from environment", ex);
        }
    }
}
