package com.ccerne.remetro.station;

import com.fasterxml.jackson.annotation.JsonProperty;
import com.fasterxml.jackson.databind.ObjectMapper;
import jakarta.annotation.PostConstruct;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

import java.io.InputStream;
import java.util.*;

/**
 * Loads aliases.json from the classpath and provides lookup helpers used by
 * StationDirectory and TrainPredictionNormalizer.
 *
 * Responsibilities:
 *  - Resolve a destination name/alias → WMATA station code
 *  - Identify no-passenger destination strings (e.g. "No Passenger", "Train")
 *  - Report which WMATA codes are linked under a primary code
 *  - Validate alias-defined links against WMATA's own StationTogether fields
 */
@Component
public class StationAliasService {

    private static final Logger log = LoggerFactory.getLogger(StationAliasService.class);

    // ── JSON data model ───────────────────────────────────────────────────────

    record AliasData(
            @JsonProperty("linked_stations")   Map<String, List<String>> linkedStations,
            @JsonProperty("station_aliases")   Map<String, List<String>> stationAliases,
            @JsonProperty("no_passenger_aliases") List<String> noPassengerAliases,
            @JsonProperty("last_train_aliases") List<String> lastTrainAliases
    ) {}

    // ── Derived lookup tables (built at @PostConstruct) ───────────────────────

    /** alias string (lower-cased) → WMATA station code */
    private Map<String, String> aliasToCode = Map.of();

    /** set of lower-cased no-passenger destination strings */
    private Set<String> noPassengerAliasSet = Set.of();

    /** set of lower-cased last-train destination strings */
    private Set<String> lastTrainAliasSet = Set.of();

    /** primaryCode → list of secondary codes */
    private Map<String, List<String>> linkedStations = Map.of();

    /** secondaryCode → primaryCode (reverse of linkedStations) */
    private Map<String, String> secondaryToPrimary = Map.of();

    @PostConstruct
    void load() {
        try (InputStream is = getClass().getResourceAsStream("/aliases.json")) {
            if (is == null) {
                log.warn("aliases.json not found on classpath; alias resolution disabled");
                return;
            }
            AliasData data = new ObjectMapper().readValue(is, AliasData.class);
            buildLookups(data);
            log.info("Loaded aliases: {} destination aliases, {} linked station groups, {} no-passenger aliases",
                    aliasToCode.size(), linkedStations.size(), noPassengerAliasSet.size());
        } catch (Exception e) {
            log.error("Failed to load aliases.json", e);
        }
    }

    private void buildLookups(AliasData data) {
        // Destination aliases: stationCode -> [alias, alias, ...] → invert to alias -> code
        Map<String, String> toCode = new HashMap<>();
        if (data.stationAliases() != null) {
            data.stationAliases().forEach((code, aliases) ->
                    aliases.forEach(alias -> toCode.put(alias.toLowerCase(), code)));
        }
        aliasToCode = Collections.unmodifiableMap(toCode);

        // No-passenger aliases
        if (data.noPassengerAliases() != null) {
            Set<String> npSet = new HashSet<>();
            data.noPassengerAliases().forEach(s -> npSet.add(s.toLowerCase()));
            noPassengerAliasSet = Collections.unmodifiableSet(npSet);
        }

        // Last-train aliases
        if (data.lastTrainAliases() != null) {
            Set<String> ltSet = new HashSet<>();
            data.lastTrainAliases().forEach(s -> ltSet.add(s.toLowerCase()));
            lastTrainAliasSet = Collections.unmodifiableSet(ltSet);
        }

        // Linked stations
        if (data.linkedStations() != null) {
            Map<String, List<String>> linked = new HashMap<>(data.linkedStations());
            Map<String, String> reverseMap = new HashMap<>();
            linked.forEach((primary, secondaries) ->
                    secondaries.forEach(secondary -> reverseMap.put(secondary, primary)));
            linkedStations = Collections.unmodifiableMap(linked);
            secondaryToPrimary = Collections.unmodifiableMap(reverseMap);
        }
    }

    // ── Public API ────────────────────────────────────────────────────────────

    /**
     * Returns true if destinationName indicates a no-passenger train
     * (supplements the Line == "No" / "--" check in the normalizer).
     */
    public boolean isNoPassengerAlias(String destinationName) {
        if (destinationName == null) return false;
        return noPassengerAliasSet.contains(destinationName.toLowerCase());
    }

    /** Returns true if destinationName indicates the last passenger train of the night. */
    public boolean isLastTrainAlias(String destinationName) {
        if (destinationName == null) return false;
        return lastTrainAliasSet.contains(destinationName.toLowerCase());
    }

    /**
     * Attempts to resolve a destination name to a WMATA station code.
     * Tries destinationName first, then the short Destination form.
     * Returns empty if not found (caller should track as unknown).
     */
    public Optional<String> resolveDestinationCode(String destinationName, String destination) {
        if (destinationName != null) {
            String code = aliasToCode.get(destinationName.toLowerCase());
            if (code != null) return Optional.of(code);
        }
        if (destination != null) {
            String code = aliasToCode.get(destination.toLowerCase());
            if (code != null) return Optional.of(code);
        }
        return Optional.empty();
    }

    /**
     * Returns the primary WMATA code for a given code.
     * If code is a secondary (e.g. "A01"), returns the primary ("C01").
     * If code is already primary or unlinked, returns it unchanged.
     */
    public String getPrimaryCode(String wmataCode) {
        return secondaryToPrimary.getOrDefault(wmataCode, wmataCode);
    }

    /** Returns true if wmataCode is a secondary in a linked-station group. */
    public boolean isSecondaryCode(String wmataCode) {
        return secondaryToPrimary.containsKey(wmataCode);
    }

    /** Returns the secondary codes linked to a primary, or an empty list. */
    public List<String> getLinkedCodes(String primaryCode) {
        return linkedStations.getOrDefault(primaryCode, List.of());
    }

    /**
     * Validates our alias file's linked_stations against WMATA's StationTogether fields.
     * Logs a warning for any mismatch so the alias file can be corrected.
     *
     * @param wmataCode        The WMATA station code being validated.
     * @param stationTogether1 Value of StationTogether1 from WMATA's jStations response.
     * @param stationTogether2 Value of StationTogether2 from WMATA's jStations response.
     */
    public void validateLinkedStation(String wmataCode, String stationTogether1, String stationTogether2) {
        String primary = getPrimaryCode(wmataCode);
        List<String> ourLinked = getLinkedCodes(primary);

        Set<String> wmataLinked = new HashSet<>();
        if (stationTogether1 != null && !stationTogether1.isBlank()) wmataLinked.add(stationTogether1);
        if (stationTogether2 != null && !stationTogether2.isBlank()) wmataLinked.add(stationTogether2);
        wmataLinked.remove(wmataCode); // a station doesn't link to itself

        Set<String> ourSet = new HashSet<>(ourLinked);
        ourSet.remove(wmataCode);

        if (!ourSet.equals(wmataLinked)) {
            log.warn("Linked-station mismatch for {}: aliases.json says {} but WMATA says {}",
                    wmataCode, ourSet, wmataLinked);
        }
    }
}
