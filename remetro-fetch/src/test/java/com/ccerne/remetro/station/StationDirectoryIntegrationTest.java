package com.ccerne.remetro.station;

import com.ccerne.remetro.proto.PredictionItem;
import com.ccerne.remetro.proto.TrainPredictionResponse;
import com.ccerne.remetro.wmata.models.StationInfoResponse;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.test.annotation.DirtiesContext;
import org.springframework.test.context.ContextConfiguration;
import org.springframework.test.context.junit.jupiter.SpringExtension;

import java.io.InputStream;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Integration test for StationDirectory using real WMATA JSON fixtures.
 *
 * Fixtures (from src/test/resources/wmata/):
 *   stations.json    — full WMATA jStations response
 *   predictions.json — large snapshot (540 trains, including no-passenger entries)
 *   predictions2.json — smaller snapshot for change-detection testing
 */
@ExtendWith(SpringExtension.class)
@ContextConfiguration(classes = {StationAliasService.class, StationDirectory.class})
@DirtiesContext(classMode = DirtiesContext.ClassMode.BEFORE_EACH_TEST_METHOD)
class StationDirectoryIntegrationTest {

    @Autowired StationDirectory directory;
    @Autowired StationAliasService aliases;

    private final ObjectMapper mapper = new ObjectMapper();
    private TestListener listener;

    @BeforeEach
    void setUp() {
        listener = new TestListener();
        directory.addListener(listener);
    }

    // ── Station ingestion ─────────────────────────────────────────────────────

    @Test
    void ingestStations_buildsDirectory() throws Exception {
        ingestStations();
        assertTrue(directory.isInitialized());
    }

    @Test
    void metroCenterLinkedStations_mergedIntoOne() throws Exception {
        ingestStations();

        // Both A01 and C01 should resolve to the same NormalizedStation
        var byA01 = directory.getStation("A01");
        var byC01 = directory.getStation("C01");

        assertTrue(byA01.isPresent(), "A01 should be registered");
        assertTrue(byC01.isPresent(), "C01 should be registered");
        assertSame(byA01.get(), byC01.get(), "A01 and C01 should map to the same NormalizedStation");

        NormalizedStation station = byC01.get();
        assertEquals("Metro Center", station.name());
        assertEquals("C01", station.primaryCode());
        assertTrue(station.allWmataCodes().contains("A01"));
        assertTrue(station.allWmataCodes().contains("C01"));

        // 4 platforms: C01 group 1, C01 group 2, A01 group 1, A01 group 2
        assertEquals(4, station.platforms().size());
    }

    @Test
    void unlinkedStation_hasTwoPlatforms() throws Exception {
        ingestStations();
        // A02 = Farragut North, not linked to anything
        var station = directory.getStation("A02");
        assertTrue(station.isPresent());
        assertEquals(2, station.get().platforms().size());
    }

    // ── Prediction ingestion ──────────────────────────────────────────────────

    @Test
    void ingestPredictions_firesChangeEvents() throws Exception {
        ingestStations();
        ingestPredictions("predictions.json");

        // predictions.json has 540 trains across many stations — events must have fired
        assertFalse(listener.changedPlatforms.isEmpty(), "Expected platform change events");
        assertFalse(listener.changedStations.isEmpty(), "Expected station change events");
    }

    @Test
    void noPassengerTrains_appearsAsNoPassengerItem() throws Exception {
        ingestStations();
        ingestPredictions("predictions.json");

        // K06 (West Falls Church) has a no-passenger train in group 2
        var platformItems = directory.getPlatformPredictions(new PlatformKey("K06", 2));
        assertTrue(platformItems.isPresent(), "K06/group2 should have predictions");

        boolean hasNoPassenger = platformItems.get().stream()
                .anyMatch(PredictionItem::hasNoPassenger);
        assertTrue(hasNoPassenger, "Expected at least one NoPassenger item at K06/group2");
    }

    @Test
    void sameDataIngested_producesNoNewEvents() throws Exception {
        ingestStations();
        ingestPredictions("predictions.json");

        int platformEventsBefore = listener.changedPlatforms.size();
        int stationEventsBefore  = listener.changedStations.size();

        // Ingest the exact same data again
        ingestPredictions("predictions.json");

        assertEquals(platformEventsBefore, listener.changedPlatforms.size(),
                "No new platform events expected for identical data");
        assertEquals(stationEventsBefore, listener.changedStations.size(),
                "No new station events expected for identical data");
    }

    @Test
    void differentDataIngested_firesNewEvents() throws Exception {
        ingestStations();
        ingestPredictions("predictions.json");

        int platformEventsBefore = listener.changedPlatforms.size();

        // predictions2.json is a different snapshot — some platforms will differ
        ingestPredictions("predictions2.json");

        assertTrue(listener.changedPlatforms.size() > platformEventsBefore,
                "Expected additional platform events when data changes");
    }

    @Test
    void knownAliasDestination_notTrackedAsUnknown() throws Exception {
        ingestStations();
        ingestPredictions("predictions.json");

        Set<String> unknown = directory.getUnknownAliases();

        // "NewCrlton" is in aliases.json → should NOT appear as unknown
        assertFalse(unknown.contains("NewCrlton"),
                "'NewCrlton' is a known alias and must not appear in unknownAliases");
        // "Branch Av" is in aliases.json → should NOT appear as unknown
        assertFalse(unknown.contains("Branch Av"),
                "'Branch Av' is a known alias and must not appear in unknownAliases");
    }

    @Test
    void unknownDestinationNames_trackedForDiscovery() throws Exception {
        ingestStations();
        // predictions2.json has "Farragut N." with no DestinationCode and not in aliases.json
        ingestPredictions("predictions2.json");

        Set<String> unknown = directory.getUnknownAliases();
        assertFalse(unknown.isEmpty(),
                "Expected at least one unknown alias to be tracked from predictions2.json");
    }

    @Test
    void predictionsSortedCorrectly_boardingBeforeArriving() throws Exception {
        ingestStations();
        ingestPredictions("predictions.json");

        // Find any platform that has both BRD and ARR
        directory.getAllStations().stream()
                .flatMap(s -> s.platforms().stream())
                .map(NormalizedPlatform::key)
                .map(directory::getPlatformPredictions)
                .filter(opt -> opt.isPresent() && opt.get().size() >= 2)
                .map(opt -> opt.get())
                .filter(items -> items.stream().anyMatch(i ->
                        i.hasTrain() && i.getTrain().getTrainMins().getStatus()
                                == com.ccerne.remetro.proto.OtherStatus.BOARDING))
                .findFirst()
                .ifPresent(items -> {
                    // If BRD is present, it must be first
                    assertTrue(items.get(0).hasTrain());
                    assertEquals(com.ccerne.remetro.proto.OtherStatus.BOARDING,
                            items.get(0).getTrain().getTrainMins().getStatus(),
                            "BOARDING should sort first");
                });
    }

    // ── Test helpers ──────────────────────────────────────────────────────────

    private void ingestStations() throws Exception {
        StationInfoResponse resp = load("stations.json", StationInfoResponse.class);
        directory.ingestStations(resp.getStations());
    }

    private void ingestPredictions(String fixture) throws Exception {
        com.ccerne.remetro.wmata.models.TrainPredictionResponse resp =
                load(fixture, com.ccerne.remetro.wmata.models.TrainPredictionResponse.class);
        directory.ingestPredictions(resp);
    }

    private <T> T load(String fixture, Class<T> type) throws Exception {
        String path = "/wmata/" + fixture;
        try (InputStream is = getClass().getResourceAsStream(path)) {
            assertNotNull(is, "Test fixture not found: " + path);
            return mapper.readValue(is, type);
        }
    }

    // ── Event capture ─────────────────────────────────────────────────────────

    static class TestListener implements PredictionChangeListener {
        final List<PlatformKey> changedPlatforms = new ArrayList<>();
        final List<String> changedStations = new ArrayList<>();

        @Override
        public void onPlatformChanged(PlatformKey key, TrainPredictionResponse response) {
            changedPlatforms.add(key);
        }

        @Override
        public void onStationChanged(String primaryCode, TrainPredictionResponse response) {
            changedStations.add(primaryCode);
        }
    }
}
