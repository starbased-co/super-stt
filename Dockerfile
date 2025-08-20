FROM nvidia/cuda:13.0.0-cudnn-devel-ubuntu24.04 AS builder

# Set build arguments for architecture
ARG TARGETARCH
ARG TARGETPLATFORM

# Install system dependencies
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    pkg-config \
    libxkbcommon-dev \
    libwayland-dev \
    libxkbcommon-x11-dev \
    libegl1-mesa-dev \
    libfontconfig1-dev \
    libfreetype6-dev \
    libglib2.0-dev \
    libgtk-4-dev \
    libspeechd-dev \
    libxrandr-dev \
    libxinerama-dev \
    libxcursor-dev \
    libxi-dev \
    libxss-dev \
    libasound2-dev \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install Rust with appropriate target
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Add Rust target based on architecture
RUN case "${TARGETARCH}" in \
        amd64) rustup target add x86_64-unknown-linux-gnu ;; \
        arm64) rustup target add aarch64-unknown-linux-gnu ;; \
        *) echo "Unsupported architecture: ${TARGETARCH}" && exit 1 ;; \
    esac

# Set working directory
WORKDIR /app

# Copy source code
COPY . .

# Build the project with CUDA and cuDNN features based on architecture
RUN case "${TARGETARCH}" in \
        amd64) cargo build --release --target x86_64-unknown-linux-gnu --features cuda,cudnn ;; \
        arm64) cargo build --release --target aarch64-unknown-linux-gnu --features cuda,cudnn ;; \
        *) echo "Unsupported architecture: ${TARGETARCH}" && exit 1 ;; \
    esac

# Create runtime stage
FROM nvidia/cuda:13.0.0-runtime-ubuntu24.04

# Set build arguments for architecture
ARG TARGETARCH

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libxkbcommon0 \
    libwayland-client0 \
    libwayland-cursor0 \
    libwayland-egl1 \
    libegl1 \
    libfontconfig1 \
    libfreetype6 \
    libglib2.0-0 \
    libgtk-4-1 \
    libspeechd2 \
    libxrandr2 \
    libxinerama1 \
    libxcursor1 \
    libxi6 \
    libxss1 \
    libasound2 \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd -r stt && useradd -r -g stt -s /bin/bash stt

# Create directories
RUN mkdir -p /var/run/stt /var/log/stt && \
    chown stt:stt /var/run/stt /var/log/stt

# Copy binaries from builder stage based on architecture
# The binaries are in architecture-specific target directories
COPY --from=builder /app/target/*/release/super-stt /usr/local/bin/
COPY --from=builder /app/target/*/release/super-stt-app /usr/local/bin/
COPY --from=builder /app/target/*/release/super-stt-cosmic-applet /usr/local/bin/

# Copy systemd service file
COPY --from=builder /app/super-stt/systemd/super-stt.service /etc/systemd/user/

# Create wrapper script
RUN echo '#!/bin/bash\nexec /usr/local/bin/super-stt "$@"' > /usr/local/bin/stt && \
    chmod +x /usr/local/bin/stt

# Switch to non-root user
USER stt

# Expose default ports
EXPOSE 8765/udp

# Set environment variables
ENV RUST_LOG=info
ENV CUDA_VISIBLE_DEVICES=all

# Default command
CMD ["/usr/local/bin/super-stt", "--socket", "/var/run/stt/super-stt.sock", "--udp-port", "8765"]
