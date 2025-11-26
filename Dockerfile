# Multi-stage build for minimal final image
FROM rust:1.89-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create a new empty project
WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src
COPY examples ./examples
COPY tests ./tests

# Build the application in release mode
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN useradd -m -u 1000 appuser

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/api-check /usr/local/bin/api-check

# Change ownership
RUN chown -R appuser:appuser /app

# Switch to non-root user
USER appuser

# Set default environment variables
ENV API_CHECK_SERVER_HOST=0.0.0.0
ENV API_CHECK_SERVER_PORT=3000

# Expose the server port
EXPOSE 3000

# Set the default command
ENTRYPOINT ["api-check"]
CMD ["server"]
