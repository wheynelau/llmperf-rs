FROM debian:bookworm-slim

ARG VERSION
ARG TARGETARCH

# Install minimal dependencies for downloading and extracting
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Map Docker's TARGETARCH to Rust target triples
RUN set -eux; \
    case "${TARGETARCH}" in \
        amd64) RUST_TARGET="x86_64-unknown-linux-gnu" ;; \
        arm64) RUST_TARGET="aarch64-unknown-linux-gnu" ;; \
        *) echo "Unsupported architecture: ${TARGETARCH}" && exit 1 ;; \
    esac; \
    DOWNLOAD_URL="https://github.com/wheynelau/llmperf-rs/releases/download/${VERSION}/llmperf-${RUST_TARGET}.tar.xz"; \
    echo "Downloading from: ${DOWNLOAD_URL}"; \
    curl -fsSL "${DOWNLOAD_URL}" -o /tmp/llmperf.tar.xz; \
    tar -xJf /tmp/llmperf.tar.xz -C /tmp; \
    mv /tmp/llmperf-${RUST_TARGET}/llmperf /usr/local/bin/llmperf; \
    chmod +x /usr/local/bin/llmperf; \
    rm -rf /tmp/llmperf.tar.xz /tmp/llmperf-*

ENTRYPOINT ["llmperf"]