# CLI Reference

```bash
llmperf --model <MODEL> [OPTIONS]
```

## Options

| Option | Default | Description |
|--------|---------|-------------|
| `--model` | (required) | Model to benchmark |
| `--tokenizer` | `hf-internal-testing/llama-tokenizer` | HuggingFace model ID or local path |
| `--mean-input-tokens` | 550 | Mean input tokens |
| `--stddev-input-tokens` | 150 | Stddev of input tokens |
| `--mean-output-tokens` | 150 | Mean output tokens (max_tokens) |
| `--stddev-output-tokens` | 80 | Stddev of output tokens |
| `--num-concurrent-requests` | 10 | Concurrent requests |
| `--max-num-completed-requests` | 10 | Requests to complete before finishing |
| `--timeout` | 90 | Hard timeout in seconds (0 for no timeout) |
| `--results-dir` | (none) | Directory to save results |
| `--metadata <K=V>` | (none) | Metadata key-value pairs (repeatable) |
| `--no-check-endpoint` | false | Skip `/models` sanity check |
| `--no-thinking` | true | Disable reasoning (set to disable) |
| `--multi-turn` | 1 | Number of conversation turns |
| `--llm-api` | `openai` | LLM API to use |
| `--additional-sampling-params` | `{}` | Additional sampling params (unused) |
| `--db-url` | (none) | Database URL for reporting |

## Examples

```bash
# Basic
export OPENAI_API_BASE=http://localhost:8000/v1
llmperf --model Qwen/Qwen3-4B --results-dir results/

# High inputs, good for RAG or agentic
llmperf --model Qwen/Qwen3-4B \
  --num-concurrent-requests 50 \
  --max-num-completed-requests 100 \
  --mean-input-tokens 8192 \
  --results-dir results/

# Multi-turn conversation benchmark
llmperf --model Qwen/Qwen3-4B \
  --multi-turn 3 \
  --mean-input-tokens 1024 \
  --mean-output-tokens 512 \
  --max-num-completed-requests 50 \
  --results-dir results/

# Default with metadata
llmperf --model Qwen/Qwen3-4B \
  --metadata experiment=baseline \
  --metadata gpu=A100 \
  --results-dir results/
```

## Output Files

Two files are generated when `--results-dir` is set:

**Filename format**: `YYYYMMDD_HHMMSS-TZ_<model>_<input>_<output>_<type>.<ext>`

Example: `20260203_113938-0800_Qwen-Qwen3-0-6B_550_150_summary.json`

### `*_summary.json`

Aggregated statistics with `mean`, `median`, `stddev`, `min`, `max`, and percentiles (`p25`-`p99`) for:

| Metric | Unit |
|--------|------|
| `ttft_s_*` | seconds |
| `itl_ms_*` | milliseconds |
| `end_to_end_latency_s_*` | seconds |
| `prefill_throughput_tps_*` | tokens/sec |
| `decode_throughput_tps_*` | tokens/sec |
| `input_tokens_*` | tokens |
| `output_tokens_*` | tokens |

Also includes: `num_completed_requests`, `num_completed_requests_per_min`, `error_rate`, `error_code_frequency`, `finish_reasons`.

### `*_individual_responses.jsonl.zst`

Per-request metrics. See [metrics.md](metrics.md).
