# Build stage
FROM eclipse-temurin:21-jdk AS build

WORKDIR /workspace

# Copy only what Maven needs (protobuf sources + the fetch project)
COPY remetro-protobuf/ ../remetro-protobuf/
COPY remetro-fetch/pom.xml .
COPY remetro-fetch/src/ src/

# Package the fat JAR (tests already ran in CI)
RUN apt-get update -qq && apt-get install -y -qq maven \
    && mvn --no-transfer-progress package -DskipTests

# Runtime stage
FROM eclipse-temurin:21-jre

WORKDIR /app

COPY --from=build /workspace/target/*.jar app.jar

# JVM tuned for container resource limits:
#  - UseContainerSupport: honour cgroup CPU/memory limits
#  - MaxRAMPercentage: use up to 75% of container memory for heap
ENTRYPOINT ["java", \
    "-XX:+UseContainerSupport", \
    "-XX:MaxRAMPercentage=75.0", \
    "-jar", "app.jar"]
