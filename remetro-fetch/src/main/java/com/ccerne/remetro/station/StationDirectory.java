package com.ccerne.remetro.station;

import com.ccerne.remetro.normalizer.TrainPredictionNormalizer;
import com.ccerne.remetro.proto.PredictionItem;
import com.ccerne.remetro.proto.TrainMins;
import com.ccerne.remetro.proto.TrainPredictionResponse;
import com.ccerne.remetro.wmata.models.StationInfo;
import com.ccerne.remetro.wmata.models.TrainPrediction;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

import java.util.*;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.stream.Collectors;
import java.util.stream.Stream;

/**
 * Thread-safe store for normalized station data and live train predictions.
 *
 * Lifecycle:
 *  1. ingestStations() — called once at startup (or on retry) to build the station map.
 *  2. ingestPredictions() — called on every poll cycle. Diffs per-platform and per-station;
 *     fires PredictionChangeListener events only when data actually changes.
 *
 * MQTT topic naming handled by MqttPublisher; this class is transport-agnostic.
 */
@Component
public class StationDirectory {

    private static final Logger log = LoggerFactory.getLogger(StationDirectory.class);

    private final StationAliasService aliases;
    private final List<PredictionChangeListener> listeners = new CopyOnWriteArrayList<>();

    // wmataCode → NormalizedStation (all codes, primary + secondary, point to same object)
    private final Map<String, NormalizedStation> stationsByWmataCode = new ConcurrentHashMap<>();

    // Current sorted predictions per platform
    private final Map<PlatformKey, List<PredictionItem>> platformPredictions = new ConcurrentHashMap<>();

    // Current sorted predictions per station (all platforms combined)
    private final Map<String, List<PredictionItem>> stationPredictions = new ConcurrentHashMap<>();

    // Unknown destination names seen — exposed via debug endpoint
    private final Set<String> unknownAliases = ConcurrentHashMap.newKeySet();

    public StationDirectory(StationAliasService aliases) {
        this.aliases = aliases;
    }

    public void addListener(PredictionChangeListener listener) {
        listeners.add(listener);
    }

    public boolean isInitialized() {
        return !stationsByWmataCode.isEmpty();
    }

    // ── Station ingestion ─────────────────────────────────────────────────────

    /**
     * Builds the station directory from WMATA's jStations response.
     * Merges linked stations (e.g. Metro Center C01 + A01) into one NormalizedStation.
     * Validates our aliases.json linked_stations against WMATA's StationTogether fields.
     */
    public synchronized void ingestStations(List<StationInfo> wmataStations) {
        if (wmataStations == null || wmataStations.isEmpty()) {
            log.warn("ingestStations called with empty list");
            return;
        }

        Map<String, StationInfo> byCode = new LinkedHashMap<>();
        for (StationInfo s : wmataStations) {
            if (s.getCode() != null) byCode.put(s.getCode(), s);
        }

        Map<String, NormalizedStation> built = new LinkedHashMap<>();
        Set<String> processed = new HashSet<>();

        for (StationInfo info : wmataStations) {
            String code = info.getCode();
            if (code == null || processed.contains(code)) continue;

            // Skip secondary codes — they'll be part of their primary's station
            if (aliases.isSecondaryCode(code)) {
                processed.add(code);
                continue;
            }

            String primaryCode = code; // this IS the primary
            List<String> linkedCodes = aliases.getLinkedCodes(primaryCode);

            // Validate links against WMATA's StationTogether fields
            aliases.validateLinkedStation(code,
                    info.getStationTogether1(), info.getStationTogether2());

            // Collect all WMATA codes for this station complex
            Set<String> allCodes = new LinkedHashSet<>();
            allCodes.add(primaryCode);
            allCodes.addAll(linkedCodes);

            // Collect line codes across all WMATA codes in this complex
            List<String> lineCodes = allCodes.stream()
                    .map(byCode::get)
                    .filter(Objects::nonNull)
                    .flatMap(si -> Stream.of(si.getLineCode1(), si.getLineCode2(),
                            si.getLineCode3(), si.getLineCode4()))
                    .filter(lc -> lc != null && !lc.isBlank())
                    .distinct()
                    .collect(Collectors.toList());

            // Build platform list: for each WMATA code, two platforms (group 1 + 2)
            List<NormalizedPlatform> platforms = new ArrayList<>();
            int idx = 1;
            for (String wmataCode : allCodes) {
                platforms.add(new NormalizedPlatform(idx++, wmataCode, 1));
                platforms.add(new NormalizedPlatform(idx++, wmataCode, 2));
            }

            // Use the primary station's name
            String name = info.getName() != null ? info.getName() : primaryCode;

            NormalizedStation station = new NormalizedStation(
                    primaryCode, name,
                    Collections.unmodifiableSet(allCodes),
                    Collections.unmodifiableList(lineCodes),
                    Collections.unmodifiableList(platforms));

            // Register every WMATA code → same station object
            for (String c : allCodes) {
                built.put(c, station);
                processed.add(c);
            }
        }

        stationsByWmataCode.putAll(built);
        long distinct = stationsByWmataCode.values().stream().distinct().count();
        log.info("Station directory built: {} station complexes across {} WMATA codes",
                distinct, stationsByWmataCode.size());
    }

    // ── Prediction ingestion ──────────────────────────────────────────────────

    /**
     * Processes a raw WMATA prediction response.
     * Groups by platform, normalizes, sorts, diffs, and fires change events.
     */
    public void ingestPredictions(
            com.ccerne.remetro.wmata.models.TrainPredictionResponse rawResponse) {

        List<TrainPrediction> trains = rawResponse.getTrains();
        if (trains == null || trains.isEmpty()) return;

        // ── Step 1: group raw predictions by PlatformKey ──────────────────────
        Map<PlatformKey, List<TrainPrediction>> grouped = new LinkedHashMap<>();

        for (TrainPrediction raw : trains) {
            String locationCode = raw.getLocationCode();
            String groupStr = raw.getGroup();
            if (locationCode == null || groupStr == null) continue;

            if (!stationsByWmataCode.containsKey(locationCode)) {
                log.warn("Prediction for unknown station code '{}' — directory may not be initialized",
                        locationCode);
                continue;
            }

            // Track unresolved destinations
            if (raw.getDestinationCode() == null && !isNoPassenger(raw) && !isLastTrain(raw)) {
                Optional<String> resolved =
                        aliases.resolveDestinationCode(raw.getDestinationName(), raw.getDestination());
                if (resolved.isEmpty()) {
                    String unknown = raw.getDestinationName();
                    if (unknown != null && !unknown.isBlank() && unknownAliases.add(unknown)) {
                        log.warn("Unknown destination alias: '{}' at station {}",
                                unknown, locationCode);
                    }
                }
            }

            try {
                int group = Integer.parseInt(groupStr.trim());
                grouped.computeIfAbsent(new PlatformKey(locationCode, group), k -> new ArrayList<>())
                        .add(raw);
            } catch (NumberFormatException e) {
                log.warn("Invalid Group value '{}' for station {}", groupStr, locationCode);
            }
        }

        // ── Step 2: normalize, sort, diff per platform ────────────────────────
        Set<String> changedPrimaryStations = new HashSet<>();

        for (Map.Entry<PlatformKey, List<TrainPrediction>> entry : grouped.entrySet()) {
            PlatformKey key = entry.getKey();

            List<PredictionItem> newItems = entry.getValue().stream()
                    .map(t -> TrainPredictionNormalizer.normalizeItem(t, aliases))
                    .sorted(PREDICTION_ORDER)
                    .toList();

            List<PredictionItem> oldItems = platformPredictions.get(key);
            if (newItems.equals(oldItems)) continue;

            platformPredictions.put(key, newItems);

            TrainPredictionResponse proto = TrainPredictionResponse.newBuilder()
                    .addAllItems(newItems).build();
            listeners.forEach(l -> l.onPlatformChanged(key, proto));

            // Mark this station for station-level aggregation
            NormalizedStation station = stationsByWmataCode.get(key.wmataCode());
            if (station != null) changedPrimaryStations.add(station.primaryCode());
        }

        // ── Step 3: aggregate and diff per station ────────────────────────────
        for (String primaryCode : changedPrimaryStations) {
            NormalizedStation station = stationsByWmataCode.get(primaryCode);
            if (station == null) continue;

            List<PredictionItem> allItems = station.platforms().stream()
                    .flatMap(p -> platformPredictions
                            .getOrDefault(p.key(), List.of()).stream())
                    .sorted(PREDICTION_ORDER)
                    .toList();

            List<PredictionItem> oldItems = stationPredictions.get(primaryCode);
            if (allItems.equals(oldItems)) continue;

            stationPredictions.put(primaryCode, allItems);

            TrainPredictionResponse proto = TrainPredictionResponse.newBuilder()
                    .addAllItems(allItems).build();
            listeners.forEach(l -> l.onStationChanged(primaryCode, proto));
        }
    }

    // ── Query methods (for debug HTTP endpoints) ──────────────────────────────

    public Collection<NormalizedStation> getAllStations() {
        return stationsByWmataCode.values().stream().distinct().toList();
    }

    public Optional<NormalizedStation> getStation(String wmataCode) {
        return Optional.ofNullable(stationsByWmataCode.get(wmataCode));
    }

    public Optional<List<PredictionItem>> getPlatformPredictions(PlatformKey key) {
        return Optional.ofNullable(platformPredictions.get(key));
    }

    public Optional<List<PredictionItem>> getStationPredictions(String primaryCode) {
        return Optional.ofNullable(stationPredictions.get(primaryCode));
    }

    public Set<String> getUnknownAliases() {
        return Collections.unmodifiableSet(unknownAliases);
    }

    // ── Sorting ───────────────────────────────────────────────────────────────

    /**
     * Canonical prediction order used for diffing and wire format.
     * BRD → ARR → Min(ascending) → DLY → Unknown → NoPassenger
     * Ties broken by line → cars (unknown last) → destination.
     */
    private static final Comparator<PredictionItem> PREDICTION_ORDER = (a, b) -> {
        int c = compareMins(a, b);
        if (c != 0) return c;
        c = Integer.compare(lineNumber(a), lineNumber(b));
        if (c != 0) return c;
        c = Integer.compare(carsKey(a), carsKey(b));
        if (c != 0) return c;
        return destination(a).compareTo(destination(b));
    };

    /** Primary sort key: BRD=0, ARR=1, Min(n)=2+n, DLY=3000, Unknown=4000, NoPassenger=9999 */
    private static int compareMins(PredictionItem a, PredictionItem b) {
        return Integer.compare(minsKey(a), minsKey(b));
    }

    private static int minsKey(PredictionItem item) {
        if (item.hasNoPassenger()) return 9999;
        TrainMins mins = item.getTrain().getTrainMins();
        return switch (mins.getPredictionCase()) {
            case STATUS -> switch (mins.getStatus()) {
                case BOARDING -> 0;
                case ARRIVING -> 1;
                case DELAYED  -> 3000;
                default       -> 4000;
            };
            case MINUTES            -> 2 + (int) mins.getMinutes();
            case PREDICTION_NOT_SET -> 4000;
        };
    }

    private static int lineNumber(PredictionItem item) {
        return item.hasNoPassenger() ? Integer.MAX_VALUE : item.getTrain().getLine().getNumber();
    }

    private static int carsKey(PredictionItem item) {
        if (item.hasNoPassenger()) return Integer.MAX_VALUE;
        com.ccerne.remetro.proto.TrainPrediction t = item.getTrain();
        return t.hasCars() ? (int) t.getCars() : Integer.MAX_VALUE;
    }

    private static String destination(PredictionItem item) {
        return item.hasNoPassenger() ? "" : item.getTrain().getDestination();
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    private boolean isNoPassenger(TrainPrediction raw) {
        String line = raw.getLine();
        if ("No".equals(line) || "--".equals(line)) return true;
        return raw.getDestinationName() != null && aliases.isNoPassengerAlias(raw.getDestinationName());
    }

    private boolean isLastTrain(TrainPrediction raw) {
        return raw.getDestinationName() != null && aliases.isLastTrainAlias(raw.getDestinationName());
    }
}
