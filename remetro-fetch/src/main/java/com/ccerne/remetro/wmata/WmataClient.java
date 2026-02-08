package com.ccerne.remetro.wmata;

import com.ccerne.remetro.wmata.api.DefaultApi;
import com.ccerne.remetro.wmata.invoker.ApiClient;
import com.ccerne.remetro.wmata.invoker.ApiException;
import com.ccerne.remetro.wmata.invoker.ApiResponse;

/**
 * Small adapter around the generated WMATA OpenAPI client to provide
 * friendly methods returning raw response bodies (as String) where the
 * generator did not produce typed models.
 */
public final class WmataClient {
    private final DefaultApi api;

    public WmataClient(DefaultApi api) {
        this.api = api;
    }

    /**
     * Call the JSON predictions endpoint and return the raw response body as a String
     * wrapped in {@link ApiResponse} so callers can inspect status and headers.
     */
    public ApiResponse<String> getPredictionJson(String stationCodes) throws ApiException {
        ApiClient client = api.getApiClient();
        okhttp3.Call call = api.getPredictionJsonCall(stationCodes, null);
        return client.execute(call, String.class);
    }

    /**
     * Call the XML predictions endpoint and return the raw response body as a String
     */
    public ApiResponse<String> getPredictionXml(String stationCodes) throws ApiException {
        ApiClient client = api.getApiClient();
        okhttp3.Call call = api.getPredictionXmlCall(stationCodes, null);
        return client.execute(call, String.class);
    }
}
