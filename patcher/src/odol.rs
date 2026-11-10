#[derive(Debug, PartialEq)]
pub struct Header {
    pub version: u32,
    pub app_id: u32,
    pub muzzle_flash: Vec<u8>,
    pub resolutions: Vec<f32>,
    pub model_info_offset: usize,
}

fn word(bytes: &[u8], pos: &mut usize) -> Result<u32, String> {
    let end = pos.checked_add(4).ok_or("ODOL header overflow")?;
    let data = bytes.get(*pos..end).ok_or("truncated ODOL header")?;
    *pos = end;
    Ok(u32::from_le_bytes(
        data.try_into().map_err(|_| "truncated ODOL header")?,
    ))
}

impl Header {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.get(..4) != Some(b"ODOL") {
            return Err("not a binarised (ODOL) P3D".into());
        }
        let mut pos = 4;
        let version = word(bytes, &mut pos)?;
        if !(73..=75).contains(&version) {
            return Err(format!("ODOL version {version} is not supported (73-75)"));
        }
        if version == 75 {
            let first = word(bytes, &mut pos)?;
            let second = word(bytes, &mut pos)?;
            if first != 0 || second != 0 {
                return Err("ODOL v75 model is encrypted".into());
            }
        }
        let app_id = word(bytes, &mut pos)?;
        let tail = bytes.get(pos..).ok_or("truncated ODOL header")?;
        let len = tail
            .iter()
            .position(|b| *b == 0)
            .ok_or("unterminated ODOL muzzle flash")?;
        let muzzle_flash = tail[..len].to_vec();
        pos += len + 1;
        let count = word(bytes, &mut pos)? as usize;
        if count > (bytes.len() - pos) / 4 {
            return Err("truncated ODOL resolutions".into());
        }
        let mut resolutions = Vec::with_capacity(count);
        for _ in 0..count {
            resolutions.push(f32::from_bits(word(bytes, &mut pos)?));
        }
        Ok(Self {
            version,
            app_id,
            muzzle_flash,
            resolutions,
            model_info_offset: pos,
        })
    }
}
