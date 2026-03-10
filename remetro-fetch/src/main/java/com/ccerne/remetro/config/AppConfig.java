package com.ccerne.remetro.config;

import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Configuration;

/**
 * Root Spring configuration. Exposes TrainFetchConfig as a named bean so it
 * can be referenced in SpEL expressions (e.g. @Scheduled fixedDelayString).
 */
@Configuration
public class AppConfig {

    @Bean
    public TrainFetchConfig trainFetchConfig() {
        return TrainFetchConfig.fromEnv();
    }
}
