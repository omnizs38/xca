use std::{env, fs, process, time::Instant};

fn usage(program: &str) {
    eprintln!(
        "Usage:\n  {program} compress <input> <output> [level: 1-9]\n  {program} decompress <input> <output>\n  {program} check <input>\n  {program} info <input>\n  {program} bench <input> [iterations] [level]"
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
        Some("bench") if (3..=5).contains(&args.len()) => {
            let input = fs::read(&args[2])?;
            let iterations: usize = args.get(3).map(|v| v.parse()).transpose()?.unwrap_or(7);
            let level: u8 = args.get(4).map(|v| v.parse()).transpose()?.unwrap_or(5);
            if iterations == 0 {
                return Err("iterations must be greater than zero".into());
            }
            let archive = xca::compress_with_level(&input, level)?;
            let mut encode_times = Vec::with_capacity(iterations);
            for _ in 0..iterations {
                let started = Instant::now();
                let result = xca::compress_with_level(&input, level)?;
                encode_times.push(started.elapsed().as_secs_f64());
                std::hint::black_box(result);
            }
            let mut decode_times = Vec::with_capacity(iterations);
            for _ in 0..iterations {
                let started = Instant::now();
                let result = xca::decompress(&archive)?;
                decode_times.push(started.elapsed().as_secs_f64());
                std::hint::black_box(result);
            }
            encode_times.sort_by(f64::total_cmp);
            decode_times.sort_by(f64::total_cmp);
            let encode = encode_times[iterations / 2];
            let decode = decode_times[iterations / 2];
            let mib = input.len() as f64 / (1024.0 * 1024.0);
            println!("input_bytes: {}", input.len());
            println!("archive_bytes: {}", archive.len());
            println!(
                "ratio_percent: {:.4}",
                archive.len() as f64 * 100.0 / input.len().max(1) as f64
            );
            println!("encode_median_ms: {:.3}", encode * 1000.0);
            println!("encode_mib_s: {:.2}", mib / encode);
            println!("decode_median_ms: {:.3}", decode * 1000.0);
            println!("decode_mib_s: {:.2}", mib / decode);
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
