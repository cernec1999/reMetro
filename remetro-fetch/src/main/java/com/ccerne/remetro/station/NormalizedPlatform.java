package com.ccerne.remetro.station;

/**
 * A single physical platform within a normalized station.
 * index is 1-based and assigned when the station directory is built.
 */
public record NormalizedPlatform(int index, String wmataCode, int group) {

    public PlatformKey key() {
        return new PlatformKey(wmataCode, group);
    }
}
