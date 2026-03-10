package com.ccerne.remetro.wmata;

import com.ccerne.remetro.config.TrainFetchConfig;
import com.ccerne.remetro.wmata.api.DefaultApi;
import com.ccerne.remetro.wmata.models.StationInfoResponse;
import com.ccerne.remetro.wmata.models.TrainPredictionResponse;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

/**
 * Thin wrapper around the OpenAPI-generated DefaultApi.
 * Configures the HTTP client once and exposes typed methods for each WMATA endpoint.
 */
@Component
public final class WmataClient {

    private static final Logger log = LoggerFactory.getLogger(WmataClient.class);
    private final DefaultApi api;

    public WmataClient(TrainFetchConfig config) {
        ApiClient client = new ApiClient();
        client.setRequestInterceptor(builder -> builder.header("api_key", config.wmataApiKey));
        client.updateBaseUri(config.wmataApiBaseUrl.toString());
        client.setConnectTimeout(config.wmataApiTimeout);
        this.api = new DefaultApi(client);
        log.info("WMATA client initialized — base URI: {}", client.getBaseUri());
    }

    /** Fetches real-time train predictions. Pass "All" to get every station at once. */
    public TrainPredictionResponse getPredictions(String stationCodes) throws ApiException {
        return api.getPredictionJson(stationCodes);
    }

    /** Fetches static station information (names, line codes, linked stations). */
    public StationInfoResponse getStations() throws ApiException {
        return api.getStationsJson();
    }
}
