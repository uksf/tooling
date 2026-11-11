pub const ABSOLUTE_CAP: usize = 512 * 1024 * 1024;
pub const LZSS_RATIO: usize = 16;
pub const LZO_RATIO: usize = 1024;

pub fn count_fits(remaining: usize, count: usize, elem: usize) -> Result<(), String> {
    let need = count.checked_mul(elem).ok_or_else(|| {
        format!("count {count} of {elem}-byte elements exceeds the remaining input")
    })?;
    if need > remaining {
        return Err(format!(
            "count {count} of {elem}-byte elements exceeds the remaining {remaining} bytes"
        ));
    }
    Ok(())
}

pub fn expand_fits(declared: usize, compressed: usize, ratio: usize) -> Result<(), String> {
    if declared > ABSOLUTE_CAP {
        return Err(format!("declared size {declared} exceeds the 512 MiB cap"));
    }
    let limit = compressed.saturating_mul(ratio);
    if declared > limit {
        return Err(format!(
            "declared size {declared} exceeds {ratio}x the {compressed}-byte compressed input"
        ));
    }
    Ok(())
}

pub fn repeat_fits(bytes: usize) -> Result<(), String> {
    if bytes > ABSOLUTE_CAP {
        return Err(format!("declared size {bytes} exceeds the 512 MiB cap"));
    }
    Ok(())
}
