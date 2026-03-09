package com.ccerne.remetro.station;

import java.util.List;
import java.util.Set;

/**
 * Our canonical station model. One entry per physical station complex.
 * Linked WMATA codes (e.g. Metro Center = C01 + A01) are merged into a
 * single NormalizedStation so the rest of the system never sees split stations.
 *
 * @param primaryCode   The stable ID we use in MQTT topics and protobuf (primary WMATA code).
 * @param name          Human-readable display name, e.g. "Metro Center".
 * @param allWmataCodes All WMATA codes belonging to this complex (primary + linked).
 * @param lineCodes     WMATA line codes serving this station (e.g. "BL", "OR", "RD").
 * @param platforms     Ordered list of physical platforms across all WMATA codes.
 */
public record NormalizedStation(
        String primaryCode,
        String name,
        Set<String> allWmataCodes,
        List<String> lineCodes,
        List<NormalizedPlatform> platforms
) {}
