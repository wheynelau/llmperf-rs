# Metrics

## How Tokens Are Counted

This benchmark tool gets token counts directly from the API responses:

```rust
// Check if usage is provided, some endpoints will send their usage.  
// This will correctly reflect the tokens that were received, so metrics are more accurate.  
if let Some(usage) = response.usage {
    metrics.number_input_tokens = usage.prompt_tokens;
    metrics.number_output_tokens = usage.completion_tokens;
    metrics.number_total_tokens = usage.total_tokens;
}
```

If the endpoint does not provide, it uses the calculated values, based on the arguments provided.  

## For correctness

If you need to get the exact token count, you can pass in a tokenizer to the tool. 
```
llmperf --tokenizer <hf tokenizer path>
```

Even then, the input tokens may not be the same due to chat templates, but from some testing it was less than 20 tokens off. 

The `output tokens` are <= `max_tokens`, more on this below. 

## Per Response Metrics

Stored in `*_individual_responses.jsonl.zst`.

```bash
zstdcat results/<filename>_individual_responses.jsonl.zst | head -n1 | jq
```

```json
{
  "itl_ms_mean": 23.81,
  "itl_ms_stddev": 13.74,
  "itl_ms_vec": [0.001, 29.03, 25.61, ...],
  "ttft_s": 0.125,
  "end_to_end_latency_s": 0.363,
  "number_input_tokens": 112,
  "number_output_tokens": 10,
  "number_total_tokens": 122,
  "prefill_throughput_tps": 893.58,
  "error_msg": null,
  "error_code": null,
  "decode_throughput_tps": 41.99,
  "content": "With beauty's treasure, ere it be self-",
  "reasoning": "",
  "finish_reason": "length"
}
```

Python:

Review the code in [postprocess/compare_results.py](/postprocess/compare_results.py). It briefly shows how to read and process the individual responses. You will need uv.

```bash
uv run postprocess/compare_results.py
```

## Summary Metrics

Key user-facing metrics: TTFT, ITL, RPM.

### ITL (Inter Token Latency)

Latency between tokens. Each `itl_vec` has N-1 elements where N is output tokens.

Aggregated across all responses:
```
Response 1: [100, 200, 300]
Response 2: [400, 500, 600]
Aggregated: [100, 200, 300, 400, 500, 600]
```

### TTFT (Time To First Token)

TTFT calculated as the difference in timing from the POST request to the first token. 

### End to End Latency

End to end latency is the total time from the POST request to the final token. To be precise, the time stops when the response returns with `finish_reason`,   
rather than end of stream. 

### Prefill Throughput Tokens Per Second (TPS)

Prefill Throughput is the number of tokens processed per second during the prefill phase.  

$`\frac{\mathrm{number\_input\_tokens}}{\mathrm{ttft}}`$

This explains why the correct input tokens are needed.  

A caveat is that this includes the queue time. 

### Decode Throughput Tokens Per Second (TPS)

Decode Throughput is the number of tokens processed per second during the decode phase.  

$`\frac{\mathrm{number\_output\_tokens}}{\mathrm{final\_time} - \mathrm{decode\_start\_time}}`$


### Finish Reason

This is for confirming that all runs end with the max tokens. Occasionally, due to prompts or model behavior, the run may not end with the max tokens. 

Then you would encounter Finish Reason = stop, rather than Finish Reason = length. For an accurate metric, all runs should end with Finish Reason = length.

However for the current codebase, finish reason stop is included. The variance will occur in RPM and E2E latency. It is left to the user to decide if they want to aggregate with or without the stopped sequences. 

## Conclusion

The role of llmperf is to help with LLM configuration, selection of models should still be done via other evaluations. A faster model does not mean a better model. 
