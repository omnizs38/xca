fn main() -> Result<(), xca::Error> {
    let source = b"XCA universal integration example".repeat(1000);
    let compressed = xca::compress(&source);
    let restored = xca::decompress(&compressed)?;
    assert_eq!(restored, source);
    println!(
        "XCA {}: {} -> {} -> {} bytes",
        xca::VERSION,
        source.len(),
        compressed.len(),
        restored.len()
    );
    Ok(())
}
