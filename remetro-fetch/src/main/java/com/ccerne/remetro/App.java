package com.ccerne.remetro;

import com.ccerne.remetro.mqtt.MqttPublisher;
import com.ccerne.remetro.station.StationDirectory;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.context.ConfigurableApplicationContext;
import org.springframework.scheduling.annotation.EnableScheduling;

/**
 * Application entry point.
 *
 * Spring Boot manages:
 *  - Embedded HTTP server (Spring MVC) for debug endpoints
 *  - @Scheduled polling via TrainFetcher
 *  - Dependency injection for all @Component beans
 *
 * Wiring order on startup:
 *  1. AppConfig creates TrainFetchConfig bean
 *  2. StationAliasService loads aliases.json (@PostConstruct)
 *  3. MqttPublisher connects to broker (if configured)
 *  4. StationDirectory registers MqttPublisher as a change listener
 *  5. TrainFetcher fetches stations then starts the prediction poll loop
 */
@SpringBootApplication
@EnableScheduling
public class App {

    public static void main(String[] args) {
        ConfigurableApplicationContext ctx = SpringApplication.run(App.class, args);

        // Register MqttPublisher as a StationDirectory listener after the context is up
        StationDirectory directory = ctx.getBean(StationDirectory.class);
        MqttPublisher publisher = ctx.getBean(MqttPublisher.class);
        directory.addListener(publisher);
    }
}
