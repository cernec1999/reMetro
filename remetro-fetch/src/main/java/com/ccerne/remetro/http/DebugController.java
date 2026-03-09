package com.ccerne.remetro.http;

import com.ccerne.remetro.proto.PredictionItem;
import com.ccerne.remetro.station.NormalizedStation;
import com.ccerne.remetro.station.PlatformKey;
import com.ccerne.remetro.station.StationDirectory;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.*;

import java.util.*;

/**
 * Read-only HTTP endpoints for inspecting internal state during development.
 * Not part of the public API — intended for browser debugging only.
 *
 * Endpoints:
 *   GET /debug/stations                               — all known station complexes
 *   GET /debug/stations/{code}                        — one station by WMATA code
 *   GET /debug/predictions/{code}                     — all-platform predictions for a station
 *   GET /debug/predictions/{code}/platform/{group}    — per-platform predictions
 *   GET /debug/unknown-aliases                        — destination names not in aliases.json
 *   GET /health                                        — liveness probe
 */
@RestController
public class DebugController {

    private final StationDirectory directory;

    public DebugController(StationDirectory directory) {
        this.directory = directory;
    }

    @GetMapping("/health")
    public String health() {
        return "OK";
    }

    @GetMapping("/debug/stations")
    public Collection<StationView> allStations() {
        return directory.getAllStations().stream()
                .map(StationView::from)
                .toList();
    }

    @GetMapping("/debug/stations/{code}")
    public ResponseEntity<StationView> station(@PathVariable String code) {
        return directory.getStation(code)
                .map(StationView::from)
                .map(ResponseEntity::ok)
                .orElse(ResponseEntity.notFound().build());
    }

    @GetMapping("/debug/predictions/{code}")
    public ResponseEntity<List<PredictionView>> stationPredictions(@PathVariable String code) {
        return directory.getStationPredictions(code)
                .map(items -> items.stream().map(PredictionView::from).toList())
                .map(ResponseEntity::ok)
                .orElse(ResponseEntity.notFound().build());
    }

    @GetMapping("/debug/predictions/{code}/platform/{group}")
    public ResponseEntity<List<PredictionView>> platformPredictions(
            @PathVariable String code,
            @PathVariable int group) {
        return directory.getPlatformPredictions(new PlatformKey(code, group))
                .map(items -> items.stream().map(PredictionView::from).toList())
                .map(ResponseEntity::ok)
                .orElse(ResponseEntity.notFound().build());
    }

    @GetMapping("/debug/unknown-aliases")
    public Set<String> unknownAliases() {
        return directory.getUnknownAliases();
    }

    // ── JSON view records ─────────────────────────────────────────────────────

    record StationView(
            String primaryCode,
            String name,
            Set<String> allWmataCodes,
            List<String> lineCodes,
            List<PlatformSummary> platforms) {

        static StationView from(NormalizedStation s) {
            var platforms = s.platforms().stream()
                    .map(p -> new PlatformSummary(p.index(), p.wmataCode(), p.group()))
                    .toList();
            return new StationView(s.primaryCode(), s.name(),
                    s.allWmataCodes(), s.lineCodes(), platforms);
        }
    }

    record PlatformSummary(int index, String wmataCode, int group) {}

    record PredictionView(String type, String line, Integer cars,
                          String destination, String minutes, boolean lastTrain) {

        static PredictionView from(PredictionItem item) {
            if (item.hasNoPassenger()) {
                return new PredictionView("NO_PASSENGER", null, null, null, null, false);
            }
            var t = item.getTrain();
            String mins = switch (t.getTrainMins().getPredictionCase()) {
                case MINUTES            -> String.valueOf(t.getTrainMins().getMinutes());
                case STATUS             -> t.getTrainMins().getStatus().name();
                case PREDICTION_NOT_SET -> "---";
            };
            return new PredictionView(
                    "TRAIN",
                    t.getLine().name(),
                    t.hasCars() ? (int) t.getCars() : null,
                    t.getDestination().isEmpty() ? null : t.getDestination(),
                    mins,
                    t.getLastTrain());
        }
    }
}
