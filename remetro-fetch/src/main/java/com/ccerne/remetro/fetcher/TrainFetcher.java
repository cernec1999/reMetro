package com.ccerne.remetro.fetcher;

import com.ccerne.remetro.config.TrainFetchConfig;
import com.ccerne.remetro.mqtt.MqttClientService;
import com.ccerne.remetro.wmata.api.DefaultApi;
import com.ccerne.remetro.wmata.invoker.ApiClient;
import com.ccerne.remetro.wmata.invoker.ApiException;
import com.ccerne.remetro.wmata.invoker.ApiResponse;
import com.ccerne.remetro.wmata.invoker.Configuration;
import com.ccerne.remetro.wmata.WmataClient;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.concurrent.atomic.AtomicReference;

/**
 * Runnable that periodically fetches data from WMATA and publishes notifications to MQTT.
 * The actual WMATA-specific parsing/logic is intentionally left as a TODO for the caller.
 */
public class TrainFetcher implements Runnable {
    private static final Logger logger = LoggerFactory.getLogger(TrainFetcher.class);

    private final TrainFetchConfig config;
    private final MqttClientService mqtt;
    private final AtomicReference<String> lastPayload;
    private final HttpClient httpClient;
    // Optional generated OpenAPI client
    private final DefaultApi wmataApi;
    private final WmataClient wmataClient;

    public TrainFetcher(TrainFetchConfig config, MqttClientService mqtt, AtomicReference<String> lastPayload) {
        this.config = config;
        this.mqtt = mqtt;
        this.lastPayload = lastPayload;
        this.httpClient = HttpClient.newBuilder()
                .connectTimeout(config.wmataApiTimeout)
                .build();

        // Initialize the generated OpenAPI client (WMATA API key is required by TrainFetchConfig)
        DefaultApi tmpApi = null;
        WmataClient tmpWmataClient = null;
        try {
            ApiClient client = Configuration.getDefaultApiClient();
            // set base path to configured WMATA base (keep default otherwise)
            if (config.wmataApiBaseUrl != null) {
                client.setBasePath(config.wmataApiBaseUrl.toString());
            }
            // set API key and reasonable timeouts
            client.setApiKey(config.wmataApiKey);
            client.setReadTimeout((int) config.wmataApiTimeout.toMillis());
            tmpApi = new DefaultApi(client);
            tmpWmataClient = new WmataClient(tmpApi);
            logger.info("Initialized OpenAPI WMATA client with base {}", client.getBasePath());
        } catch (Exception e) {
            logger.warn("Failed to initialize generated WMATA client, will fall back to raw HTTP: {}", e.toString());
            tmpApi = null;
            tmpWmataClient = null;
        }
        this.wmataApi = tmpApi;
        this.wmataClient = tmpWmataClient;
    }

    @Override
    public void run() {
        logger.info("Starting TrainFetcher loop with interval {} seconds", config.fetchInterval.getSeconds());
        while (!Thread.currentThread().isInterrupted()) {
            try {
                if (wmataClient != null) {
                    // Use the generated OpenAPI client adapter when available.
                    try {
                        ApiResponse<String> apiResp = wmataClient.getPredictionJson("All");
                        int status = apiResp.getStatusCode();
                        String body = apiResp.getData();
                        if (body == null) {
                            // fallback to raw HTTP if generation did not provide a body
                            body = doRawHttpFetch();
                        } else {
                            // update last payload and publish
                            lastPayload.set(body);
                            mqtt.publish("remetro/predictions", body);
                        }
                        logger.info("Fetched WMATA predictions (status={})", status);
                    } catch (ApiException ae) {
                        logger.warn("WMATA client call failed (will fallback to direct HTTP): {}", ae.getMessage());
                        // fall back to raw HTTP
                        doRawHttpFetch();
                    }
                } else {
                    // No generated client configured: use raw HTTP
                    doRawHttpFetch();
                }

            } catch (Exception e) {
                // InterruptedException is handled where it can be thrown; any other exceptions are logged here.
                logger.warn("Error during fetch loop", e);
            }

            try {
                Thread.sleep(config.fetchInterval.toMillis());
            } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
                break;
            }
        }

        logger.info("TrainFetcher loop exiting");
    }

    private static String escapeJson(String s) {
        if (s == null) return "";
        // escape backslashes first, then quotes, and normalize newlines
        return s.replace("\\", "\\\\").replace("\"", "\\\"").replace('\n', ' ').replace('\r', ' ');
    }

    /**
     * Simple raw HTTP fetch used as a fallback when the generated client is unavailable
     * or when a generated client call fails.
     */
    private String doRawHttpFetch() {
        try {
            URI uri = config.wmataApiBaseUrl.resolve("StationPrediction.svc/json/GetPrediction/ALL");
            HttpRequest req = HttpRequest.newBuilder()
                    .uri(uri)
                    .timeout(config.wmataApiTimeout)
                    .header("User-Agent", "reMetro/1.0 (+https://github.com/cernec1999/reMetro)")
                    .header("api_key", config.wmataApiKey)
                    .GET()
                    .build();

            HttpResponse<String> resp = httpClient.send(req, HttpResponse.BodyHandlers.ofString());
            String body = resp.body();

            // TODO: parse body and create meaningful payloads for MQTT
            String payload = String.format("{\"statusCode\":%d,\"bodyPreview\":\"%s\"}", resp.statusCode(), escapeJson(body));

            // update last payload and publish to MQTT topic
            lastPayload.set(payload);
            mqtt.publish("remetro/predictions", payload);
            return payload;
        } catch (InterruptedException ie) {
            Thread.currentThread().interrupt();
            return null;
        } catch (Exception e) {
            logger.warn("Error during raw HTTP fetch", e);
            return null;
        }
    }
}
