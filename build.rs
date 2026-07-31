use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // you can include bytes directly, but this just makes the binary a little smaller
    // it also allows inspecting the json file, otherwise its also possible to include bytes zstd
    // directly
    let input = Path::new("assets/llama-tokenizer.json");
    println!("cargo:rerun-if-changed={}", input.display());

    let out_dir = env::var("OUT_DIR").unwrap();
    let json = fs::read(input).unwrap();
    let compressed = zstd::encode_all(&json[..], 22).unwrap();
    fs::write(format!("{out_dir}/llama-tokenizer.json.zst"), compressed).unwrap();
}
