pub fn validate(name: &[u8], data: &[u8], points: usize, counts: &[usize]) -> Result<(), String> {
    let len = match name {
        b"#SharpEdges#" => data.len() / 8 * 8,
        b"#Property#" => 128,
        b"#Mass#" => data.len() / 4 * 4,
        b"#UVSet#" => 4 + counts.iter().sum::<usize>() * 8,
        b"#EndOfFile#" => 0,
        b"#Animation#" => return Err("animation tagg is not understood by the MLOD library".into()),
        _ => points + counts.len(),
    };
    if len != data.len() {
        return Err(format!(
            "tagg '{}' is not understood by the MLOD library",
            String::from_utf8_lossy(name)
        ));
    }
    if name == b"#Lock#" && data.iter().any(|&x| x > 1) {
        return Err("the MLOD library does not re-serialise this model byte-identically; refusing to patch it".into());
    }
    Ok(())
}
