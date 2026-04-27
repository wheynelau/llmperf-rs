use token_benchmark::prompt::{self, create_prompt, randomly_sample_sonnet_lines_prompt};

#[test]
fn create_prompt_functional() {
    // This test will fail if it is unable to download
    // TODO: Download json offline to keep in the repo
    let tokenizer =
        tokenizers::Tokenizer::from_pretrained("hf-internal-testing/llama-tokenizer", None)
            .unwrap();

    let raw_lines = ["Hello world\n", "Test line\n", "Another line\n"];

    let lines: Vec<tokenizers::Encoding> = raw_lines
        .iter()
        .map(|line| tokenizer.encode_fast(*line, false).unwrap())
        .collect();

    let prompt = tokenizer.encode_fast("Prompt here:", false).unwrap();
    let prompt_ids = prompt.get_ids();
    let prompt_len = prompt.len();

    // noticed that there was a chance of failture, need to monitor, else increase range
    for target_token in (5..100).step_by(1) {
        let remaining_prompt_tokens = target_token - prompt_len;
        let result_ids = create_prompt(prompt_ids, &lines, remaining_prompt_tokens as u32);
        let prompt = tokenizer.decode(&result_ids, false).unwrap();
        let output_tokens = result_ids.len() as u32;
        let actual_tokens = tokenizer.encode_fast(prompt.as_str(), false).unwrap().len();

        assert_eq!(actual_tokens as u32, output_tokens);
        let diff = (actual_tokens as i64 - target_token as i64).abs();
        assert!(diff <= 1, "expected {target_token} got {actual_tokens}");
    }
}

macro_rules! test_tokenizer {
    ($name:ident, $tokenizer:expr) => {
        #[test]
        fn $name() {
            test_tokenizer_impl($tokenizer);
        }
    };
}

fn test_tokenizer_impl(tokenizer_name: &str) {
    let tokenizer = match tokenizers::Tokenizer::from_pretrained(tokenizer_name, None) {
        Ok(t) => t,
        Err(e) => {
            println!("Skipping tokenizer {tokenizer_name} due to error: {e:?}",);
            return;
        }
    };
    let sonnet_lines = prompt::parse_sonnet_text(&tokenizer, prompt::SONNET_TEXT)
        .expect("Should be able to parse sonnet text");

    let prompt_encoding = tokenizer.encode_fast(prompt::PROMPT_TEXT, false).unwrap();

    for mean in (1000..=10000).step_by(100) {
        let (prompt, _returned_token_count) = randomly_sample_sonnet_lines_prompt(
            mean,
            0,
            &prompt_encoding,
            &tokenizer,
            &sonnet_lines,
        );

        // The reason for encoding here to validate that encode(decode(ids)) may not be == ids
        let actual_token_count = tokenizer.encode_fast(prompt.as_str(), false).unwrap().len();
        let diff = (actual_token_count as i64 - mean as i64).abs();
        assert!(
            diff <= 1,
            "Tokenizer: {tokenizer_name} failed at mean: {mean} (actual {actual_token_count} diff {diff})",
        );
        assert!(prompt.contains("Repeat lines indefinitely"));
    }
}

// These tests are non exhaustive, as there are too many tokenizers to test.
test_tokenizer!(llama_tokenizer, "hf-internal-testing/llama-tokenizer");
test_tokenizer!(qwen_tokenizer, "Qwen/Qwen3-0.6B");
test_tokenizer!(phi_tokenizer, "microsoft/phi-4");
test_tokenizer!(
    devstral_tokenizer,
    "mistralai/Devstral-Small-2-24B-Instruct-2512"
);
test_tokenizer!(glm_tokenizer, "zai-org/GLM-4.7");
test_tokenizer!(minimax_tokenizer, "MiniMaxAI/MiniMax-M2.1");
test_tokenizer!(openaioss_tokenizer, "openai/gpt-oss-20b");

test_tokenizer!(
    nemotron_tokenizer,
    "nvidia/NVIDIA-Nemotron-3-Nano-30B-A3B-BF16"
);

test_tokenizer!(deepseek_tokenizer, "deepseek-ai/DeepSeek-V3.2");

test_tokenizer!(gemma_tokenizer, "OpenMeditron/Meditron3-Gemma2-2B");

test_tokenizer!(baichuan_tokenizer, "baichuan-inc/Baichuan-M2-32B");

// This tokenizer fails occasionally.
// They have tokens for "'OR'" and "O","R"
// And due to the tokenization, this sequence "'O'", "'Repeat'",
// Becomes "'OR'", "'e'", "'peat'",
test_tokenizer!(ouro_tokenizer, "ByteDance/Ouro-1.4B");
