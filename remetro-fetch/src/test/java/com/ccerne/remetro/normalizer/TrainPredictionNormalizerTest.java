package com.ccerne.remetro.normalizer;

import com.ccerne.remetro.proto.*;
import com.ccerne.remetro.station.StationAliasService;
import com.ccerne.remetro.wmata.models.TrainPrediction;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.extension.ExtendWith;
import org.mockito.Mock;
import org.mockito.junit.jupiter.MockitoExtension;
import org.mockito.junit.jupiter.MockitoSettings;
import org.mockito.quality.Strictness;

import static org.junit.jupiter.api.Assertions.*;
import static org.mockito.ArgumentMatchers.any;
import static org.mockito.Mockito.when;

@ExtendWith(MockitoExtension.class)
@MockitoSettings(strictness = Strictness.LENIENT)
class TrainPredictionNormalizerTest {

    @Mock
    StationAliasService aliases;

    @BeforeEach
    void defaultAliasStubs() {
        // By default: no-passenger alias check returns false, resolution returns empty
        when(aliases.isNoPassengerAlias(any())).thenReturn(false);
        when(aliases.isLastTrainAlias(any())).thenReturn(false);
    }

    // ── Last train detection ──────────────────────────────────────────────────

    @Test
    void lastTrainAlias_setsLastTrainFlag() {
        when(aliases.isLastTrainAlias("LAST TRAIN")).thenReturn(true);
        com.ccerne.remetro.proto.TrainPrediction t = trainFrom("SV", null, "18", "LAST TRAIN");
        assertTrue(t.getLastTrain(), "Expected last_train flag to be set");
        assertFalse(t.hasCars(), "Last train has no car count");
        assertEquals(18, t.getTrainMins().getMinutes());
        assertEquals("", t.getDestination(), "Destination should be empty for last trains");
    }

    @Test
    void normalTrain_doesNotSetLastTrainFlag() {
        com.ccerne.remetro.proto.TrainPrediction t = trainFrom("RD", "8", "5", "Glenmont");
        assertFalse(t.getLastTrain());
    }

    // ── No-passenger detection ────────────────────────────────────────────────

    @Test
    void lineCodeNo_producesNoPassenger() {
        PredictionItem item = normalizeItem("No", "8", "5", "Glenmont");
        assertTrue(item.hasNoPassenger());
        assertFalse(item.hasTrain());
    }

    @Test
    void lineCodeDash_producesNoPassenger() {
        PredictionItem item = normalizeItem("--", null, "---", "No Passenger");
        assertTrue(item.hasNoPassenger());
    }

    @Test
    void destinationNameAlias_producesNoPassenger() {
        when(aliases.isNoPassengerAlias("Train")).thenReturn(true);
        PredictionItem item = normalizeItem("RD", null, "ARR", "Train");
        assertTrue(item.hasNoPassenger());
    }

    // ── Line mapping ──────────────────────────────────────────────────────────

    @Test
    void lineMapping_allCodes() {
        assertEquals(Line.RED,    trainLine("RD"));
        assertEquals(Line.ORANGE, trainLine("OR"));
        assertEquals(Line.BLUE,   trainLine("BL"));
        assertEquals(Line.GREEN,  trainLine("GR"));
        assertEquals(Line.YELLOW, trainLine("YL"));
        assertEquals(Line.SILVER, trainLine("SV"));
    }

    @Test
    void unknownLineCode_producesUnspecified() {
        assertEquals(Line.LINE_UNSPECIFIED, trainLine("XX"));
    }

    // ── Cars ─────────────────────────────────────────────────────────────────

    @Test
    void numericCar_setsOptionalField() {
        com.ccerne.remetro.proto.TrainPrediction t = trainFrom("RD", "8", "5", "Glenmont");
        assertTrue(t.hasCars());
        assertEquals(8, t.getCars());
    }

    @Test
    void nullCar_leavesOptionalUnset() {
        com.ccerne.remetro.proto.TrainPrediction t = trainFrom("RD", null, "5", "Glenmont");
        assertFalse(t.hasCars());
    }

    @Test
    void dashedCar_leavesOptionalUnset() {
        assertFalse(trainFrom("RD", "-",   "5", "Glenmont").hasCars());
        assertFalse(trainFrom("RD", "--",  "5", "Glenmont").hasCars());
        assertFalse(trainFrom("RD", "---", "5", "Glenmont").hasCars());
    }

    @Test
    void blankCar_leavesOptionalUnset() {
        assertFalse(trainFrom("RD", "", "5", "Glenmont").hasCars());
        assertFalse(trainFrom("RD", "  ", "5", "Glenmont").hasCars());
    }

    // ── TrainMins ─────────────────────────────────────────────────────────────

    @Test
    void boardingStatus() {
        TrainMins mins = minsFrom("BRD");
        assertEquals(TrainMins.PredictionCase.STATUS, mins.getPredictionCase());
        assertEquals(OtherStatus.BOARDING, mins.getStatus());
    }

    @Test
    void arrivingStatus() {
        assertEquals(OtherStatus.ARRIVING, minsFrom("ARR").getStatus());
    }

    @Test
    void delayedStatus() {
        assertEquals(OtherStatus.DELAYED, minsFrom("DLY").getStatus());
    }

    @Test
    void numericMinutes() {
        TrainMins mins = minsFrom("7");
        assertEquals(TrainMins.PredictionCase.MINUTES, mins.getPredictionCase());
        assertEquals(7, mins.getMinutes());
    }

    @Test
    void dashedMin_producesUnknown() {
        assertUnknownMins(minsFrom("-"));
        assertUnknownMins(minsFrom("--"));
        assertUnknownMins(minsFrom("---"));
    }

    @Test
    void emptyMin_producesUnknown() {
        assertUnknownMins(minsFrom(""));
        assertUnknownMins(minsFrom("   "));
    }

    @Test
    void nullMin_producesUnknown() {
        com.ccerne.remetro.proto.TrainPrediction t = trainFrom("RD", "8", null, "Glenmont");
        assertUnknownMins(t.getTrainMins());
    }

    // ── Destination ───────────────────────────────────────────────────────────

    @Test
    void destinationName_passedThrough() {
        assertEquals("Glenmont", trainFrom("RD", "8", "5", "Glenmont").getDestination());
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    private PredictionItem normalizeItem(String line, String car, String min, String destName) {
        return TrainPredictionNormalizer.normalizeItem(raw(line, car, min, destName), aliases);
    }

    private com.ccerne.remetro.proto.TrainPrediction trainFrom(
            String line, String car, String min, String destName) {
        PredictionItem item = normalizeItem(line, car, min, destName);
        assertFalse(item.hasNoPassenger(), "Expected a train item, got NoPassenger");
        return item.getTrain();
    }

    private Line trainLine(String lineCode) {
        return trainFrom(lineCode, "8", "5", "Test Dest").getLine();
    }

    private TrainMins minsFrom(String min) {
        return trainFrom("RD", "8", min, "Glenmont").getTrainMins();
    }

    private void assertUnknownMins(TrainMins mins) {
        assertEquals(TrainMins.PredictionCase.PREDICTION_NOT_SET, mins.getPredictionCase(),
                "Expected PREDICTION_NOT_SET (unknown mins) but got " + mins.getPredictionCase());
    }

    private static TrainPrediction raw(String line, String car, String min, String destName) {
        TrainPrediction t = new TrainPrediction();
        t.setLine(line);
        t.setCar(car);
        t.setMin(min);
        t.setDestinationName(destName);
        t.setDestinationCode(null);
        t.setLocationCode("C01");
        t.setGroup("1");
        return t;
    }
}
