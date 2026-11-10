use crate::odol::Header;

#[derive(Debug, PartialEq)]
pub struct Skeleton {
    pub name: Vec<u8>,
    pub bones: Vec<(Vec<u8>, Vec<u8>)>,
    pub end: usize,
}

fn string(bytes: &[u8], pos: &mut usize) -> Result<Vec<u8>, String> {
    let tail = bytes.get(*pos..).ok_or("truncated skeleton")?;
    let len = tail
        .iter()
        .position(|b| *b == 0)
        .ok_or("unterminated skeleton string")?;
    let result = tail[..len].to_vec();
    *pos += len + 1;
    Ok(result)
}

impl Skeleton {
    pub fn parse(bytes: &[u8], header: &Header) -> Result<Self, String> {
        let mut pos = header
            .model_info_offset
            .checked_add(216)
            .ok_or("model info overflow")?;
        let name = string(bytes, &mut pos)?;
        let mut bones = Vec::new();
        if !name.is_empty() {
            let count_at = pos.checked_add(1).ok_or("skeleton overflow")?;
            let end = count_at.checked_add(4).ok_or("skeleton overflow")?;
            let count_bytes = bytes.get(count_at..end).ok_or("truncated skeleton")?;
            let count =
                u32::from_le_bytes(count_bytes.try_into().map_err(|_| "truncated skeleton")?)
                    as usize;
            pos = end;
            if count > bytes.len().saturating_sub(pos) / 2 {
                return Err("truncated skeleton bones".into());
            }
            for _ in 0..count {
                bones.push((string(bytes, &mut pos)?, string(bytes, &mut pos)?));
            }
            string(bytes, &mut pos)?;
        }
        Ok(Self {
            name,
            bones,
            end: pos,
        })
    }
}
