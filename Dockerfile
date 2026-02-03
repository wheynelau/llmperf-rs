# Stage 1: Build
FROM debian:bookworm-slim AS builder

ARG VERSION
ARG TARGETARCH

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    xz-utils \
    && rm -rf /var/lib/apt/lists/*

# Rather than building again, use the pre-built binaries from GitHub Releases
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
    mv /tmp/llmperf-${RUST_TARGET}/llmperf /llmperf; \
    chmod +x /llmperf

# Final Stage: Minimal runtime image
FROM gcr.io/distroless/cc-debian13:nonroot

COPY --from=builder /llmperf /usr/local/bin/llmperf

ENTRYPOINT ["llmperf"]