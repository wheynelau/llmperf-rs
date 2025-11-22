use log::warn;
use rand;
use rand::seq::SliceRandom;
use rand_distr::{Distribution, Normal};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::Once;

pub fn sample_random_positive_int(mean: u32, stddev: u32, prompt_token_length: u32) -> u32 {
    if stddev == 0 {
        return mean;
    }
    let mut rng = rand::rng();

    let normal = Normal::new(mean as f64, stddev as f64).unwrap();

    loop {
        let sample_f64 = normal.sample(&mut rng);
        let sample_u32 = sample_f64.round() as u32;

        if sample_u32 >= prompt_token_length {
            return sample_u32;
        }
    }
}

pub fn randomly_sample_sonnet_lines_prompt(
    prompt_tokens_mean: u32,
    prompt_tokens_stddev: u32,
    expect_output_tokens: u32,
    tokenizer: &tokenizers::Tokenizer,
    sonnet_lines: &[(String, u32)],
) -> (String, u32) {
    let prompt = format!(
        "Repeat lines indefinitely from the following text with {expect_output_tokens} output tokens. Don't generate eos tokens:\n\n"
    );

    let prompt_token_len = get_token_length(tokenizer, &prompt);

    // Set a safe mean in the event a low mean was set with stddev, potentially creating an infinite loop
    let safe_mean = std::cmp::max(prompt_tokens_mean, prompt_token_len);

    if prompt_tokens_mean < prompt_token_len {
        static WARN_ONCE: Once = Once::new();
        WARN_ONCE.call_once(|| {
            warn!(
                "prompt_tokens_mean ({}) is less than base prompt length ({}). \
                Adjusting mean to ({})\n\
                This warning will only show once.",
                prompt_tokens_mean, prompt_token_len, prompt_token_len
            );
        });
    }

    let num_prompt_tokens =
        sample_random_positive_int(safe_mean, prompt_tokens_stddev, prompt_token_len);

    let remaining_prompt_tokens = num_prompt_tokens - prompt_token_len;

    let prompt = create_prompt(prompt, sonnet_lines, tokenizer, remaining_prompt_tokens);

    (prompt, num_prompt_tokens)
}
fn create_prompt(
    mut prompt: String,
    sonnet_lines: &[(String, u32)],
    tokenizer: &tokenizers::Tokenizer,
    mut remaining_prompt_tokens: u32,
) -> String {
    // Create a mutable copy to shuffle for each call to maintain randomness
    let mut shuffled_lines = sonnet_lines.to_vec();
    let mut rng = rand::rng();
    shuffled_lines.shuffle(&mut rng);

    // Use cycle to replicate the while , for
    for (line, line_len) in shuffled_lines.iter().cycle() {
        if remaining_prompt_tokens < *line_len {
            // Partial line: encode, truncate, and decode
            // Original code truncates by character, but that seems unusual. Truncate by tokens for a more accurate representation
            let encoding = tokenizer.encode(line.to_string(), false).unwrap();
            let ids = encoding.get_ids();
            let truncated = tokenizer
                .decode(&ids[..remaining_prompt_tokens as usize], false)
                .unwrap();
            prompt.push_str(&truncated);
            break;
        }

        // Full line fits
        prompt.push_str(line);
        remaining_prompt_tokens -= line_len;
    }
    prompt
}

pub fn read_sonnet_file(
    tokenizer: &tokenizers::Tokenizer,
    sonnet_path: &str,
) -> Vec<(String, u32)> {
    let file = File::open(sonnet_path).expect("Failed to open sonnet file");
    let reader = BufReader::new(file);

    let lines: Vec<String> = reader.lines().map_while(Result::ok).collect();

    let line_refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
    let encodings = tokenizer
        .encode_batch(line_refs, false)
        .expect("Failed to encode batch");

    let lines_with_counts: Vec<(String, u32)> = lines
        .into_iter()
        .zip(encodings.iter().map(|e| e.len() as u32))
        .collect();

    // Note: We no longer shuffle here since we shuffle in create_prompt for each call
    lines_with_counts
}

fn get_token_length(tokenizer: &tokenizers::Tokenizer, text: &str) -> u32 {
    tokenizer
        .encode(text, true)
        .expect("Failed to get token length")
        .len() as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_sample() {
        let sample = sample_random_positive_int(10, 5, 12);
        // Can't really prove the random distribution
        assert!(sample > 12);
    }
}
