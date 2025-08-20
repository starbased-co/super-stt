# Cross compilation Docker image for Super STT
# This is used by the cross tool for cross-compilation
FROM nvidia/cuda:13.0.0-cudnn9-devel-ubuntu24.04

# Install Zig for cross-compilation
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Install Zig 0.13.0 (compatible with cargo-zigbuild 0.19.7)
RUN curl -L https://ziglang.org/download/0.13.0/zig-linux-x86_64-0.13.0.tar.xz | tar -xJ -C /opt && \
    ln -s /opt/zig-linux-x86_64-0.13.0/zig /usr/local/bin/zig

# The cross tool will handle the rest via pre-build commands
# The dependencies will be installed based on the target architecture