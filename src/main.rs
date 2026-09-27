use std::{env, fs, process};

fn usage(program: &str) {
    eprintln!(
        "Usage:\n  {program} compress <input> <output> [level: 1-9]\n  {program} decompress <input> <output>\n  {program} check <input>\n  {program} info <input>"
    );
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let program = args.first().map(String::as_str).unwrap_or("xca");
    match args.get(1).map(String::as_str) {
        Some("compress") if args.len() == 4 || args.len() == 5 => {
            let level = args
                .get(4)
                .map(|value| value.parse())
                .transpose()?
                .unwrap_or(6);
            let input = fs::read(&args[2])?;
            let output = xca::compress_with_level(&input, level)?;
            fs::write(&args[3], &output)?;
            eprintln!(
                "{} -> {} bytes ({:.2}%)",
                input.len(),
                output.len(),
                ratio(input.len(), output.len())
            );
        }
        Some("decompress") if args.len() == 4 => {
            let input = fs::read(&args[2])?;
            let output = xca::decompress(&input)?;
            fs::write(&args[3], &output)?;
            eprintln!("{} -> {} bytes", input.len(), output.len());
        }
        Some("check") if args.len() == 3 => {
            let input = fs::read(&args[2])?;
            let output = xca::decompress(&input)?;
            eprintln!("valid XCA stream; decoded size: {} bytes", output.len());
        }
        Some("info") if args.len() == 3 => {
            let input = fs::read(&args[2])?;
            let info = xca::frame_info(&input)?;
            println!("version: XCA{}", info.version);
            println!("method: {:?}", info.method);
            println!("level: {}", info.level);
            println!("original size: {}", info.original_size);
            println!("frame size: {}", info.frame_size);
            println!("checksum: {}", info.checksum);
        }
        _ => {
            usage(program);
            return Err("invalid arguments".into());
        }
    }
    Ok(())
}

fn ratio(input: usize, output: usize) -> f64 {
    if input == 0 {
        0.0
    } else {
        output as f64 * 100.0 / input as f64
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        process::exit(1);
    }
}
