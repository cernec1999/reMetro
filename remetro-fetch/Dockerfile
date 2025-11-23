FROM rust:latest

# Copy the entire project directory to the container
# We want to go up one level to include `remetro-common`
WORKDIR /app
COPY . .

# Now build remetro-fetch
WORKDIR /app/remetro-fetch
RUN cargo build --release

CMD ["./target/release/remetro-fetch"]