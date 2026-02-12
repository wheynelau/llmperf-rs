FROM gcr.io/distroless/cc-debian13:nonroot

ARG REPO=wheynelau/llmperf
ARG TARGETARCH

COPY artifacts/${TARGETARCH}/llmperf /usr/local/bin/llmperf

USER nonroot:nonroot

ENTRYPOINT ["llmperf"]
