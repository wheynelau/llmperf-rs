# Deployment

## Docker

Docker images are available on ghcr.io/wheynelau/llmperf-rs:latest. You can pull the latest image with:

```bash
docker pull ghcr.io/wheynelau/llmperf-rs:latest
```

## Kubernetes

If you want to save the files, you should handle the volume mounts accordingly.

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
          args:
            - --model=Qwen/Qwen3-0.6B
            - --tokenizer=Qwen/Qwen3-0.6B
```