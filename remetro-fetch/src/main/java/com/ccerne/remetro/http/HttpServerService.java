package com.ccerne.remetro.http;

import com.sun.net.httpserver.HttpExchange;
import com.sun.net.httpserver.HttpHandler;
import com.sun.net.httpserver.HttpServer;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.util.concurrent.Executors;
import java.util.concurrent.atomic.AtomicReference;

public class HttpServerService {
    private static final Logger logger = LoggerFactory.getLogger(HttpServerService.class);

    private final HttpServer server;
    private final AtomicReference<String> lastPayload;

    public HttpServerService(String bindAddress, AtomicReference<String> lastPayload) throws IOException {
        this.lastPayload = lastPayload;
        String host = "0.0.0.0";
        int port = 3000;
        if (bindAddress != null && !bindAddress.isBlank()) {
            String[] parts = bindAddress.split(":");
            if (parts.length >= 1 && !parts[0].isBlank()) host = parts[0];
            if (parts.length >= 2) {
                try { port = Integer.parseInt(parts[1]); } catch (NumberFormatException ignored) {}
            }
        }
        server = HttpServer.create(new InetSocketAddress(host, port), 0);
        server.createContext("/health", new HealthHandler());
        server.createContext("/last", new LastHandler());
        server.setExecutor(Executors.newCachedThreadPool());
    }

    public void start() {
        logger.info("Starting HTTP server on {}", server.getAddress());
        server.start();
    }

    public void stop() {
        logger.info("Stopping HTTP server");
        server.stop(0);
    }

    class HealthHandler implements HttpHandler {
        @Override
        public void handle(HttpExchange exchange) throws IOException {
            byte[] resp = "OK".getBytes();
            exchange.sendResponseHeaders(200, resp.length);
            try (OutputStream os = exchange.getResponseBody()) {
                os.write(resp);
            }
        }
    }

    class LastHandler implements HttpHandler {
        @Override
        public void handle(HttpExchange exchange) throws IOException {
            String value = lastPayload.get();
            if (value == null) value = "";
            byte[] resp = value.getBytes();
            exchange.getResponseHeaders().add("Content-Type", "application/json; charset=utf-8");
            exchange.sendResponseHeaders(200, resp.length);
            try (OutputStream os = exchange.getResponseBody()) {
                os.write(resp);
            }
        }
    }
}
