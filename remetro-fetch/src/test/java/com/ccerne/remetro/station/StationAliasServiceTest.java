package com.ccerne.remetro.station;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.test.context.ContextConfiguration;
import org.springframework.test.context.junit.jupiter.SpringExtension;

import java.util.Optional;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Tests StationAliasService against the real aliases.json on the classpath.
 * Uses a minimal Spring context — no web server, no WMATA client, no MQTT.
 */
@ExtendWith(SpringExtension.class)
@ContextConfiguration(classes = StationAliasService.class)
class StationAliasServiceTest {

    @Autowired
    StationAliasService service;

    // ── No-passenger aliases ──────────────────────────────────────────────────

    @Test
    void knownNoPassengerAliases_areRecognized() {
        assertTrue(service.isNoPassengerAlias("No Passenger"));
        assertTrue(service.isNoPassengerAlias("Train"));
    }

    @Test
    void noPassengerCheck_isCaseInsensitive() {
        assertTrue(service.isNoPassengerAlias("no passenger"));
        assertTrue(service.isNoPassengerAlias("NO PASSENGER"));
    }

    @Test
    void normalDestination_isNotNoPassenger() {
        assertFalse(service.isNoPassengerAlias("Glenmont"));
        assertFalse(service.isNoPassengerAlias("Shady Grove"));
        assertFalse(service.isNoPassengerAlias(null));
    }

    // ── Destination alias resolution ──────────────────────────────────────────

    @Test
    void knownAliases_resolveToStationCode() {
        assertEquals(Optional.of("D13"), service.resolveDestinationCode("NewCrlton", null));
        assertEquals(Optional.of("D13"), service.resolveDestinationCode("N Carrollton", null));
        assertEquals(Optional.of("A15"), service.resolveDestinationCode("Shady Grv", null));
        assertEquals(Optional.of("E01"), service.resolveDestinationCode("MtVern Sq", null));
        assertEquals(Optional.of("E01"), service.resolveDestinationCode("Mt Vern Sq", null));
        assertEquals(Optional.of("F11"), service.resolveDestinationCode("Branch Av", null));
    }

    @Test
    void resolution_isCaseInsensitive() {
        assertEquals(Optional.of("D13"), service.resolveDestinationCode("newcrlton", null));
        assertEquals(Optional.of("D13"), service.resolveDestinationCode("NEWCRLTON", null));
    }

    @Test
    void shortFormDestination_usedAsFallback() {
        // DestinationName unknown but short Destination form matches
        assertEquals(Optional.of("A15"),
                service.resolveDestinationCode("Unknown Long Name", "Shady Grv"));
    }

    @Test
    void unknownDestination_returnsEmpty() {
        assertEquals(Optional.empty(),
                service.resolveDestinationCode("Completely Unknown Station", "Unknown"));
        assertEquals(Optional.empty(),
                service.resolveDestinationCode(null, null));
    }

    // ── Linked station lookups ────────────────────────────────────────────────

    @Test
    void secondaryCode_resolvesToPrimary() {
        assertEquals("C01", service.getPrimaryCode("A01")); // A01 is secondary → C01 primary
        assertEquals("B01", service.getPrimaryCode("F01")); // F01 is secondary → B01 primary
        assertEquals("B06", service.getPrimaryCode("E06")); // E06 is secondary → B06 primary
        assertEquals("D03", service.getPrimaryCode("F03")); // F03 is secondary → D03 primary
    }

    @Test
    void primaryOrUnlinkedCode_returnsSelf() {
        assertEquals("C01", service.getPrimaryCode("C01"));
        assertEquals("A02", service.getPrimaryCode("A02")); // Farragut North, unlinked
    }

    @Test
    void isSecondaryCode_correctlyIdentified() {
        assertTrue(service.isSecondaryCode("A01"));
        assertFalse(service.isSecondaryCode("C01"));
        assertFalse(service.isSecondaryCode("A02")); // unlinked station
    }

    @Test
    void getLinkedCodes_returnsSecondaries() {
        assertTrue(service.getLinkedCodes("C01").contains("A01"));
        assertTrue(service.getLinkedCodes("B01").contains("F01"));
    }

    @Test
    void getLinkedCodes_emptyForUnlinked() {
        assertTrue(service.getLinkedCodes("A02").isEmpty());
    }

    // ── Last train aliases ────────────────────────────────────────────────────

    @Test
    void knownLastTrainAliases_areRecognized() {
        assertTrue(service.isLastTrainAlias("LAST TRAIN"));
        assertTrue(service.isLastTrainAlias("LastTrain"));
    }

    @Test
    void lastTrainCheck_isCaseInsensitive() {
        assertTrue(service.isLastTrainAlias("last train"));
        assertTrue(service.isLastTrainAlias("lasttrain"));
    }

    @Test
    void normalDestination_isNotLastTrain() {
        assertFalse(service.isLastTrainAlias("Glenmont"));
        assertFalse(service.isLastTrainAlias(null));
    }

    // ── Linked station validation ─────────────────────────────────────────────

    @Test
    void validateLinkedStation_noExceptionWhenMatching() {
        // Our aliases.json says C01→[A01]. WMATA says A01 has StationTogether1="C01".
        // Validation should pass silently (we can't assert on log output, but no exception).
        assertDoesNotThrow(() -> service.validateLinkedStation("A01", "C01", ""));
        assertDoesNotThrow(() -> service.validateLinkedStation("C01", "A01", ""));
    }
}
