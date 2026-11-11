use super::BoneWeight;

pub fn i32s(raw: &[u8]) -> Result<Vec<i32>, String> {
    chunks(raw, "misaligned ODOL int array")
        .map(|rows| rows.iter().map(|c| i32::from_le_bytes(*c)).collect())
}

pub fn f32s(raw: &[u8]) -> Result<Vec<f32>, String> {
    chunks(raw, "misaligned ODOL float array")
        .map(|rows| rows.iter().map(|c| f32::from_le_bytes(*c)).collect())
}

pub fn vec3s(raw: &[u8]) -> Result<Vec<[f32; 3]>, String> {
    if !raw.len().is_multiple_of(12) {
        return Err("misaligned ODOL vector array".into());
    }
    Ok(raw
        .as_chunks::<12>()
        .0
        .iter()
        .map(|c| {
            [
                f32::from_le_bytes([c[0], c[1], c[2], c[3]]),
                f32::from_le_bytes([c[4], c[5], c[6], c[7]]),
                f32::from_le_bytes([c[8], c[9], c[10], c[11]]),
            ]
        })
        .collect())
}

pub fn normals(raw: &[u8]) -> Result<Vec<[f32; 3]>, String> {
    Ok(i32s(raw)?.into_iter().map(normal).collect())
}

fn normal(value: i32) -> [f32; 3] {
    const SCALE: f64 = -1.0 / 511.0;
    let mut parts = [value & 0x3ff, (value >> 10) & 0x3ff, (value >> 20) & 0x3ff];
    for part in &mut parts {
        if *part > 511 {
            *part -= 1024;
        }
    }
    [
        (f64::from(parts[0]) * SCALE) as f32,
        (f64::from(parts[1]) * SCALE) as f32,
        (f64::from(parts[2]) * SCALE) as f32,
    ]
}

pub fn bone_refs(raw: &[u8]) -> Result<Vec<BoneWeight>, String> {
    if !raw.len().is_multiple_of(12) {
        return Err("misaligned ODOL bone refs".into());
    }
    Ok(raw
        .as_chunks::<12>()
        .0
        .iter()
        .map(|c| BoneWeight {
            count: i32::from_le_bytes([c[0], c[1], c[2], c[3]]),
            data: [c[4], c[5], c[6], c[7], c[8], c[9], c[10], c[11]],
        })
        .collect())
}

pub fn scale_uv(value: i16, span: f64, min: f32) -> f32 {
    (1.52587890625e-5 * (f64::from(value) + f64::from(i16::MAX)) * span) as f32 + min
}

fn chunks<'a>(raw: &'a [u8], msg: &str) -> Result<&'a [[u8; 4]], String> {
    if !raw.len().is_multiple_of(4) {
        return Err(msg.into());
    }
    Ok(raw.as_chunks::<4>().0)
}
