package com.ccerne.remetro.station;

import com.ccerne.remetro.proto.TrainPredictionResponse;

/**
 * Called by StationDirectory whenever predictions change.
 * Implementations (e.g. MqttPublisher) react by publishing the updated payload.
 */
public interface PredictionChangeListener {

    /**
     * Fired when predictions for a specific platform change.
     *
     * @param key      The platform that changed.
     * @param response The full updated prediction list for that platform.
     */
    void onPlatformChanged(PlatformKey key, TrainPredictionResponse response);

    /**
     * Fired when any platform within a station changes, providing the aggregated
     * view of all platforms at that station.
     *
     * @param primaryCode The station's primary WMATA code (our canonical station ID).
     * @param response    The combined prediction list for the whole station.
     */
    void onStationChanged(String primaryCode, TrainPredictionResponse response);
}
