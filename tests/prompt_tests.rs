use token_benchmark::prompt::{
    create_prompt, randomly_sample_sonnet_lines_prompt, read_sonnet_file,
};

#[test]
fn test_create_prompt_functional() {
    let tokenizer =
        tokenizers::Tokenizer::from_pretrained("hf-internal-testing/llama-tokenizer", None)
            .unwrap();

    let raw_lines = vec!["Hello world\n", "Test line\n", "Another line\n"];

    let lines: Vec<(tokenizers::Encoding, u32)> = raw_lines
        .iter()
        .map(|line| {
            let encoding = tokenizer.encode_fast(*line, false).unwrap();
            let len = encoding.len() as u32;
            (encoding, len)
        })
        .collect();

    for target_tokens in [5, 10] {
        let (prompt, output_tokens) = create_prompt(Vec::new(), &lines, &tokenizer, target_tokens);
        let actual_tokens = tokenizer.encode_fast(prompt.as_str(), false).unwrap().len();

        assert_eq!(actual_tokens as u32, output_tokens);
        assert_eq!(actual_tokens, target_tokens as usize);
    }
}

#[test]
fn test_randomly_sample_sonnet_lines_prompt_with_file() {
    let tokenizer = tokenizers::Tokenizer::from_pretrained("Qwen/Qwen3-0.6B-FP8", None).unwrap();

    let sonnet_lines =
        read_sonnet_file(&tokenizer, "sonnet.txt").expect("Should be able to read sonnet.txt file");

    assert!(!sonnet_lines.is_empty(), "Sonnet file should contain lines");

    let stddev = 0;

    for mean in (1000..=10000).step_by(100) {
        // returned token count is the number of tokens from concatenated Vec of Ids
        // That is [1,2,3] + [4,5,6] = len of 6
        let (prompt, returned_token_count) =
            randomly_sample_sonnet_lines_prompt(mean, stddev, mean, &tokenizer, &sonnet_lines);

        // The reason for encoding here to validate that encode(decode(ids)) may not be == ids
        let actual_token_count = tokenizer.encode_fast(prompt.as_str(), false).unwrap().len();

        assert_eq!(actual_token_count, mean as usize);
        assert_eq!(returned_token_count, mean);
        assert!(prompt.contains("Repeat lines indefinitely"));
    }
}
