package com.ccerne.remetro.fetcher;

import com.ccerne.remetro.config.TrainFetchConfig;
import com.ccerne.remetro.station.StationDirectory;
import com.ccerne.remetro.wmata.ApiException;
import com.ccerne.remetro.wmata.WmataClient;
import jakarta.annotation.PostConstruct;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.scheduling.annotation.Scheduled;
import org.springframework.stereotype.Component;

/**
 * Drives the WMATA polling loop via Spring's scheduler.
 *
 * On startup (@PostConstruct) it attempts to initialize the station directory.
 * The scheduled method then polls for predictions at the configured interval.
 * Both operations are resilient — WMATA being temporarily down won't crash the app.
 */
@Component
public class TrainFetcher {

    private static final Logger log = LoggerFactory.getLogger(TrainFetcher.class);

    private final TrainFetchConfig config;
    private final WmataClient wmataClient;
    private final StationDirectory stationDirectory;

    public TrainFetcher(TrainFetchConfig config,
                        WmataClient wmataClient,
                        StationDirectory stationDirectory) {
        this.config = config;
        this.wmataClient = wmataClient;
        this.stationDirectory = stationDirectory;
    }

    /** Attempt station initialization at startup. Retry is handled in fetchPredictions(). */
    @PostConstruct
    void init() {
        log.info("TrainFetcher starting — poll interval: {}s", config.fetchInterval.getSeconds());
        initializeStations();
    }

    /**
     * Runs every fetchInterval milliseconds (fixed delay, so the next execution
     * starts only after the previous one completes, avoiding overlap).
     */
    @Scheduled(fixedDelayString = "#{@trainFetchConfig.fetchInterval.toMillis()}")
    public void fetchPredictions() {
        if (!stationDirectory.isInitialized()) {
            log.info("Station directory not yet initialized — retrying station fetch");
            if (!initializeStations()) return;
        }
        try {
            var response = wmataClient.getPredictions("All");
            stationDirectory.ingestPredictions(response);
        } catch (ApiException e) {
            log.warn("WMATA predictions fetch failed: {}", e.getMessage());
        }
    }

    /** @return true if station directory was successfully initialized */
    private boolean initializeStations() {
        try {
            var response = wmataClient.getStations();
            if (response.getStations() != null) {
                stationDirectory.ingestStations(response.getStations());
                return true;
            }
        } catch (ApiException e) {
            log.warn("WMATA stations fetch failed: {}", e.getMessage());
        }
        return false;
    }
}
