# Deployment

## Docker

Docker images are available on ghcr.io/wheynelau/llmperf-rs:latest. You can pull the latest image with:

```bash
docker pull ghcr.io/wheynelau/llmperf-rs:latest
```

Run the container with environment variables:

```bash
docker run --rm \
  -e OPENAI_API_BASE=http://localhost:8080/v1 \
  -e OPENAI_API_KEY=sk-xxx \
  ghcr.io/wheynelau/llmperf-rs:latest \
  --model=Qwen/Qwen3-0.6B \
  --tokenizer=Qwen/Qwen3-0.6B
```

## Kubernetes

If you want to save the files, you should handle the volume mounts accordingly.

Since version 0.6: You can specify all values in `env`.

```yaml
apiVersion: batch/v1
kind: Job
metadata:
  name: llmperf-benchmark
spec:
  backoffLimit: 0
  template:
    spec:
      restartPolicy: Never
      containers:
        - name: llmperf
          image: ghcr.io/wheynelau/llmperf-rs:latest
          env:
            - name: OPENAI_API_BASE
              value: "http://myapp:8080/v1"
            - name: DB_URL
              valueFrom:
                secretKeyRef:
                  name: llmperf-secrets
                  key: db-url
          # Or load all env vars from a secret (like .env):
          # envFrom:
          #   - secretRef:
          #       name: llmperf-secrets
          args:
            - --model=Qwen/Qwen3-0.6B
            - --tokenizer=Qwen/Qwen3-0.6B
```

Since version 0.6
