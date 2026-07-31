# CLI Reference

```bash
llmperf --model <MODEL> [OPTIONS]
```

All options can also be supplied via environment variables. The environment variable name matches the uppercased flag with hyphens replaced by underscores (e.g. `--mean-input-tokens` → `MEAN_INPUT_TOKENS`, `--no-check-endpoint` → `NO_CHECK_ENDPOINT`). Command-line flags take precedence over environment variables.

## Options

| Option | Env Var | Default | Description |
|--------|---------|---------|-------------|
| `--model` | `MODEL` | (required) | Model to benchmark |
| `--tokenizer` | `TOKENIZER` | `hf-internal-testing/llama-tokenizer` | HuggingFace model ID or local path |
| `--mean-input-tokens` | `MEAN_INPUT_TOKENS` | 550 | Mean input tokens (min: 50) |
| `--stddev-input-tokens` | `STDDEV_INPUT_TOKENS` | 150 | Stddev of input tokens |
| `--mean-output-tokens` | `MEAN_OUTPUT_TOKENS` | 150 | Mean output tokens (`max_tokens`, min: 1) |
| `--stddev-output-tokens` | `STDDEV_OUTPUT_TOKENS` | 80 | Stddev of output tokens |
| `--num-concurrent-requests` | `NUM_CONCURRENT_REQUESTS` | 10 | Concurrent requests |
| `--max-num-completed-requests` | `MAX_NUM_COMPLETED_REQUESTS` | 10 | Requests to complete before finishing |
| `--timeout` | `TIMEOUT` | 90 | Hard timeout (seconds; `0` for no timeout) |
| `--additional-sampling-params` | `ADDITIONAL_SAMPLING_PARAMS` | `{}` | Additional sampling params JSON (currently unused) |
| `--results-dir` | `RESULTS_DIR` | (none) | Directory to save results |
| `--llm-api` | `LLM_API` | `openai` | LLM API type (only `openai` supported) |
| `--metadata <K=V>` (repeatable) | `METADATA` | (none) | Metadata key-value pairs (comma-separated via env, e.g. `METADATA='name=foo,bar=1'`) |
| `--no-check-endpoint` | `NO_CHECK_ENDPOINT` | `false` | Skip `/models` sanity check |
| `--no-thinking` | `NO_THINKING` | `true` | Disable reasoning (sends `thinking: false`/`enable_thinking: false`) |
| `--multi-turn` | `MULTI_TURN` | 1 | Number of conversation turns (min: 1); `1` runs single-turn mode |
| `--db-url` | `DB_URL` | (none) | DB URL to report results (value hidden in help; prefers env due to secrets) |
| `--headers <K=V>` (repeatable) | `HEADERS` | (none) | Extra request headers (e.g. `x-bf-store-raw-request-response=true`); `Content-Type` and `Authorization` cannot be overridden |

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

# Default with metadata
llmperf --model Qwen/Qwen3-4B \
  --metadata experiment=baseline,gpu=A100 \
  --results-dir results/

# Same options via environment variables
export MODEL=Qwen/Qwen3-4B
export NUM_CONCURRENT_REQUESTS=50
export MAX_NUM_COMPLETED_REQUESTS=100
export MEAN_INPUT_TOKENS=8192
export RESULTS_DIR=results/
export METADATA="experiment=baseline,gpu=A100"
llmperf

# Multi-turn benchmark (5 turns)
llmperf --model Qwen/Qwen3-4B --multi-turn 5 --results-dir results/
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

Also includes: `num_completed_requests`, `num_completed_requests_per_min`, `error_rate`, `error_code_frequency`, `finish_reasons`, `cache_hit_rate` (multi-turn runs only).

### `*_individual_responses.jsonl.zst`

Per-request metrics. See [metrics.md](metrics.md).
