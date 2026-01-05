# Prompt

## Description

This doc briefly describes how the prompt is built, and how the exact token count is achieved.

## Steps

1. Sonnet text is loaded
2. Sonnet text is split by "\n", but the "\n" is added back. In the original llmperf, the newline was not added back.
3. The lines are tokenized, so each line is actually a list of token ids.
4. A list of random integers is generated based on the number of lines, for this explanation, [0,1,2,3] will be used.
5. On each iteration, [0,1,2,3] is shuffled and cycled -> [1,3,2,0,1,3,2,0,...]
6. Build the prompt, rough python code:

Note: 
- token_ids here is a list of tokens
- prompt_ids is the tokenized prompt

```python
prompt_ids = [2,5,67,1235,2]
result_ids = []
leftover_tokens = 1000
sonnet_lines = [(token_ids, line_length),(token_ids, line_length),... ]

for idx in infinite_loop:
    if leftover_tokens == 0:
        break

    tokens, line_len = sonnet_lines[idx]

    if line_len < leftover_tokens:
        result_ids.extend(tokens)
    else:
        result_ids.extend(tokens[:remaining])
        break

result_ids.extend(prompt_ids)

prompt = tokenizer.decode(result_ids)

```

```
# prompt will look something like this
sonnet line 1
sonnet line 3
sonnet line 2
...
Repeat lines indefinitely from the above text. Don't generate eos tokens:
```

## Difference between this and original llmperf

1. Original llmperf reads the line as strings, tokenize the strings but index the string based on the tokens
```python
with open(sonnet_path, "r") as f:
    sonnet_lines = f.readlines()
    for line in sonnet_lines:
            line_to_add = line
            if remaining_prompt_tokens - get_token_length(line_to_add) < 0:
                line_to_add = line_to_add[: int(math.ceil(remaining_prompt_tokens))]
```

My way of testing was to do rounds of encode and decode. In my first few tests, the 2nd encode led to lesser tokens, due to the tokenization behaviour.

Eg: [0,1,2,3,4] + [5,6,7,8,9]

token_length = 10

After decoding to a text and encoding, it became like this:

[0,1,2,3,10,6,7,8,9]

2. Add prompt after lines, rather than before

This was to reduce the caching as much as possible