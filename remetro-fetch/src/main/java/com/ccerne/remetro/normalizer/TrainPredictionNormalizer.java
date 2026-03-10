package com.ccerne.remetro.normalizer;

import com.ccerne.remetro.proto.*;
import com.ccerne.remetro.station.StationAliasService;
import com.ccerne.remetro.wmata.models.TrainPrediction;

/**
 * Converts a single raw WMATA TrainPrediction into a proto PredictionItem.
 * Stateless — all methods are static. StationDirectory calls normalizeItem()
 * per-train while grouping predictions by platform.
 */
public class TrainPredictionNormalizer {

    private TrainPredictionNormalizer() {}

    /**
     * Normalizes one WMATA prediction into a proto PredictionItem.
     * No-passenger trains (detected via Line code or DestinationName alias)
     * become a NoPassenger signal; all others become a TrainPrediction.
     */
    public static PredictionItem normalizeItem(TrainPrediction train, StationAliasService aliases) {
        if (isNoPassengerTrain(train, aliases)) {
            return PredictionItem.newBuilder()
                    .setNoPassenger(NoPassenger.getDefaultInstance())
                    .build();
        }
        return PredictionItem.newBuilder()
                .setTrain(buildTrainPrediction(train, aliases))
                .build();
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    private static boolean isNoPassengerTrain(TrainPrediction train, StationAliasService aliases) {
        String line = train.getLine();
        if ("No".equals(line) || "--".equals(line)) return true;
        String destName = train.getDestinationName();
        return destName != null && aliases.isNoPassengerAlias(destName);
    }

    private static com.ccerne.remetro.proto.TrainPrediction buildTrainPrediction(
            TrainPrediction train, StationAliasService aliases) {
        var b = com.ccerne.remetro.proto.TrainPrediction.newBuilder();

        b.setLine(mapLine(train.getLine()));

        // cars is optional — only set when a real value is present
        String carStr = train.getCar();
        if (carStr != null && !isDashed(carStr) && !carStr.isBlank()) {
            try {
                b.setCars(Integer.parseInt(carStr));
            } catch (NumberFormatException ignored) {}
        }

        b.setTrainMins(mapMins(train.getMin()));

        String destName = train.getDestinationName();
        if (aliases.isLastTrainAlias(destName)) {
            b.setLastTrain(true);
        } else if (destName != null) {
            b.setDestination(destName);
        }

        return b.build();
    }

    private static Line mapLine(String line) {
        if (line == null) return Line.LINE_UNSPECIFIED;
        return switch (line) {
            case "OR" -> Line.ORANGE;
            case "YL" -> Line.YELLOW;
            case "GR" -> Line.GREEN;
            case "BL" -> Line.BLUE;
            case "SV" -> Line.SILVER;
            case "RD" -> Line.RED;
            default   -> Line.LINE_UNSPECIFIED;
        };
    }

    private static TrainMins mapMins(String min) {
        var b = TrainMins.newBuilder();
        if (min == null || min.isBlank() || isDashed(min)) return b.build();
        return switch (min) {
            case "ARR" -> b.setStatus(OtherStatus.ARRIVING).build();
            case "BRD" -> b.setStatus(OtherStatus.BOARDING).build();
            case "DLY" -> b.setStatus(OtherStatus.DELAYED).build();
            default -> {
                try {
                    yield b.setMinutes(Integer.parseInt(min)).build();
                } catch (NumberFormatException e) {
                    yield b.build();
                }
            }
        };
    }

    /** Matches WMATA dash placeholders: "-", "--", "---" */
    static boolean isDashed(String s) {
        return "-".equals(s) || "--".equals(s) || "---".equals(s);
    }
}
