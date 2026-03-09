package com.ccerne.remetro.station;

/**
 * Uniquely identifies a single physical platform: a WMATA station code + group number.
 * Group 1 and 2 correspond to the two directions of service at a station.
 */
public record PlatformKey(String wmataCode, int group) {

    @Override
    public String toString() {
        return wmataCode + "/group/" + group;
    }
}
