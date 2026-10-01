use std::{env, fs, process, time::Instant};

fn usage(program: &str) {
    eprintln!(
        "Usage:\n  {program} --version\n  {program} compress <input> <output>\n  {program} decompress <input> <output>\n  {program} check <input>\n  {program} info <input>\n  {program} analyze <archive>\n  {program} bench <input> [iterations]"
    );
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(value) = env::var_os("XCA_THREADS") {
        let value = value
            .to_str()
            .ok_or("XCA_THREADS must be valid UTF-8")?
            .parse::<usize>()?;
        if value == 0 {
            return Err("XCA_THREADS must be greater than zero".into());
        }
        xca::set_thread_limit(value);
    }
    let args: Vec<String> = env::args().collect();
    let program = args.first().map(String::as_str).unwrap_or("xca");
    match args.get(1).map(String::as_str) {
        Some("--version" | "-V") if args.len() == 2 => {
            println!("xca {}", xca::VERSION);
        }
        Some("compress") if args.len() == 4 => {
            let stats = xca::compress_file(&args[2], &args[3])?;
            eprintln!(
                "{} -> {} bytes ({:.2}%)",
                stats.input_bytes,
                stats.output_bytes,
                ratio(stats.input_bytes as usize, stats.output_bytes as usize)
            );
        }
        Some("decompress") if args.len() == 4 => {
            let stats = xca::decompress_file(&args[2], &args[3])?;
            eprintln!("{} -> {} bytes", stats.input_bytes, stats.output_bytes);
        }
        Some("check") if args.len() == 3 => {
            let input = fs::read(&args[2])?;
            let decoded_size = xca::check(&input)?;
            eprintln!("valid XCA stream; decoded size: {decoded_size} bytes");
        }
        Some("info") if args.len() == 3 => {
            let input = fs::read(&args[2])?;
            let info = xca::frame_info(&input)?;
            println!("version: XCA{}", info.version);
            println!("method: {:?}", info.method);
            if let Some(level) = info.legacy_level {
                println!("profile: legacy");
                println!("legacy level: {level}");
            } else {
                println!("profile: unified");
            }
            println!("original size: {}", info.original_size);
            println!("frame size: {}", info.frame_size);
            println!("checksum: {}", info.checksum);
        }
        Some("analyze") if args.len() == 3 => {
            let archive = fs::read(&args[2])?;
            let a = xca::analyze_archive(&archive)?;
            let match_commands = a.short_match_commands + a.long_match_commands;
            println!("original_bytes: {}", a.original_bytes);
            println!("archive_bytes: {}", a.archive_bytes);
            println!("blocks: {}", a.blocks);
            println!("stored_blocks: {}", a.stored_blocks);
            println!("pulse_blocks: {}", a.pulse_blocks);
            println!("split_pulse_blocks: {}", a.split_pulse_blocks);
            println!("entropy_blocks: {}", a.entropy_blocks);
            println!("reference_blocks: {}", a.reference_blocks);
            println!("referenced_bytes: {}", a.referenced_bytes);
            println!(
                "referenced_percent: {:.4}",
                a.referenced_bytes as f64 * 100.0 / a.original_bytes.max(1) as f64
            );
            println!("predictor_none_blocks: {}", a.predictor_none_blocks);
            println!("predictor_delta_blocks: {}", a.predictor_delta_blocks);
            println!("predictor_xor_blocks: {}", a.predictor_xor_blocks);
            println!("literal_commands: {}", a.literal_commands);
            println!("literal_bytes: {}", a.literal_bytes);
            println!(
                "literal_percent: {:.4}",
                a.literal_bytes as f64 * 100.0 / a.original_bytes.max(1) as f64
            );
            println!("match_commands: {}", match_commands);
            println!("short_match_commands: {}", a.short_match_commands);
            println!("long_match_commands: {}", a.long_match_commands);
            println!("matched_bytes: {}", a.matched_bytes);
            println!(
                "matched_percent: {:.4}",
                a.matched_bytes as f64 * 100.0 / a.original_bytes.max(1) as f64
            );
            println!(
                "average_match_length: {:.2}",
                a.matched_bytes as f64 / match_commands.max(1) as f64
            );
            println!(
                "average_match_distance: {:.2}",
                a.distance_total as f64 / match_commands.max(1) as f64
            );
        }
        Some("bench") if (3..=4).contains(&args.len()) => {
            let input = fs::read(&args[2])?;
            let iterations: usize = args.get(3).map(|v| v.parse()).transpose()?.unwrap_or(7);
            if iterations == 0 {
                return Err("iterations must be greater than zero".into());
            }
            let archive = xca::compress(&input);
            let mut encode_times = Vec::with_capacity(iterations);
            for _ in 0..iterations {
                let started = Instant::now();
                let result = xca::compress(&input);
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
