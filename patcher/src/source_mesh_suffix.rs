use super::Reader;

pub fn skip(r: &mut Reader<'_>, points: usize, counts: &[i32]) -> Result<(), String> {
    if r.take(4)? != b"TAGG" {
        return Err("TAGG expected in the add-mesh source".into());
    }
    loop {
        if r.take(1)?[0] == 0 {
            return Err("deactivated tagg in the add-mesh source".into());
        }
        let start = r.pos;
        r.string()?;
        let name = &r.data[start..r.pos - 1];
        let size = r.u32()?;
        let len = match name {
            b"#EndOfFile#" => break,
            b"#SharpEdges#" => size / 8 * 8,
            b"#Property#" => 128,
            b"#Mass#" => size / 4 * 4,
            b"#Animation#" => 4 + (size as u32).wrapping_sub(4) as usize / 12 * 12,
            b"#UVSet#" => {
                let mut len = 4usize;
                for &c in counts {
                    let c = usize::try_from(c).map_err(|_| "negative face vertex count")?;
                    len = len.checked_add(c * 8).ok_or("MLOD overflow")?;
                }
                len
            }
            _ => points + counts.len(),
        };
        r.take(len)?;
    }
    r.take(4)?;
    Ok(())
}
