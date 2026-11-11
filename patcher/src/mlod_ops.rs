use crate::lod_names::resolution;
use serde_json::Value;

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(n).ok_or("MLOD overflow")?;
        let result = self.bytes.get(self.pos..end).ok_or("truncated MLOD")?;
        self.pos = end;
        Ok(result)
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| "truncated MLOD")?,
        ))
    }
    fn count(&mut self) -> Result<usize, String> {
        let n = self.u32()? as usize;
        if n > self.bytes.len() {
            return Err("invalid MLOD count".into());
        }
        Ok(n)
    }
    fn limit(&self, count: usize, elem: usize) -> Result<(), String> {
        crate::bound::count_fits(self.bytes.len().saturating_sub(self.pos), count, elem)
    }
    fn string(&mut self) -> Result<&'a [u8], String> {
        let n = self
            .bytes
            .get(self.pos..)
            .ok_or("truncated MLOD")?
            .iter()
            .position(|&x| x == 0)
            .ok_or("unterminated MLOD string")?;
        let s = self.take(n)?;
        self.take(1)?;
        Ok(s)
    }
}
fn number(b: &[u8]) -> f32 {
    f32::from_le_bytes(b.try_into().expect("four bytes"))
}
fn word(b: &[u8]) -> usize {
    u32::from_le_bytes(b.try_into().expect("four bytes")) as usize
}
fn put(out: &mut Vec<u8>, n: usize) -> Result<(), String> {
    out.extend_from_slice(
        &u32::try_from(n)
            .map_err(|_| "MLOD too large")?
            .to_le_bytes(),
    );
    Ok(())
}
fn area(points: &[[f32; 3]], face: &[u8], count: usize) -> Result<f64, String> {
    let mut normal = [0.0; 3];
    for i in 0..count {
        let a = points
            .get(word(&face[4 + i * 16..8 + i * 16]))
            .ok_or("invalid point index")?;
        let j = (i + 1) % count;
        let b = points
            .get(word(&face[4 + j * 16..8 + j * 16]))
            .ok_or("invalid point index")?;
        for (axis, value) in normal.iter_mut().enumerate() {
            let x = (axis + 1) % 3;
            let y = (axis + 2) % 3;
            *value += (f64::from(a[x]) - f64::from(b[x])) * (f64::from(a[y]) + f64::from(b[y]));
        }
    }
    Ok((normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2]).sqrt() / 2.0)
}
fn lod(
    r: &mut Reader<'_>,
    min: f64,
    filters: &Option<Vec<f32>>,
) -> Result<(Vec<u8>, usize), String> {
    let start = r.pos;
    if r.take(4)? != b"P3DM" || r.u32()? != 28 || r.u32()? != 256 {
        return Err("invalid P3DM header".into());
    }
    let np = r.count()?;
    let nn = r.count()?;
    let nf = r.count()?;
    r.take(4)?;
    let raw_points = r.take(np.checked_mul(16).ok_or("MLOD overflow")?)?;
    let points: Vec<[f32; 3]> = raw_points
        .as_chunks::<16>()
        .0
        .iter()
        .map(|p| [number(&p[0..4]), number(&p[4..8]), number(&p[8..12])])
        .collect();
    r.take(nn.checked_mul(12).ok_or("MLOD overflow")?)?;
    let prefix_end = r.pos;
    r.limit(nf, 74)?;
    let mut faces = Vec::with_capacity(nf);
    let mut counts = Vec::with_capacity(nf);
    let mut areas = Vec::with_capacity(nf);
    for _ in 0..nf {
        let begin = r.pos;
        let count = r.u32()? as usize;
        if count > 4 {
            return Err("invalid face vertex count".into());
        }
        r.take(68)?;
        r.string()?;
        r.string()?;
        let face = &r.bytes[begin..r.pos];
        areas.push(area(&points, face, count)?);
        counts.push(count);
        faces.push(face);
    }
    if r.take(4)? != b"TAGG" {
        return Err("TAGG expected".into());
    }
    let mut tags = Vec::new();
    loop {
        if r.take(1)? != [1] {
            return Err("deactivated tagg".into());
        }
        let name = r.string()?;
        let size = r.count()?;
        let data = r.take(size)?;
        crate::mlod_tags::validate(name, data, np, &counts)?;
        let end = name == b"#EndOfFile#";
        tags.push((name, data));
        if end {
            break;
        }
    }
    let res = number(r.take(4)?);
    let selected = filters.as_ref().is_none_or(|f| {
        f.iter()
            .any(|&v| v == res || (v > 1000.0 && (res - v).abs() <= v * 1e-6))
    });
    let keep: Vec<bool> = areas.iter().map(|&a| !selected || a >= min).collect();
    let removed = keep.iter().filter(|&&k| !k).count();
    if removed == 0 {
        return Ok((r.bytes[start..r.pos].to_vec(), 0));
    }
    let mut out = r.bytes[start..prefix_end].to_vec();
    out[20..24].copy_from_slice(
        &u32::try_from(nf - removed)
            .map_err(|_| "MLOD too large")?
            .to_le_bytes(),
    );
    for (face, &k) in faces.iter().zip(&keep) {
        if k {
            out.extend_from_slice(face);
        }
    }
    out.extend_from_slice(b"TAGG");
    for (name, data) in tags {
        let mut new = Vec::new();
        if name == b"#UVSet#" {
            if data.len() < 4 {
                return Err("invalid UV tagg".into());
            }
            new.extend_from_slice(&data[..4]);
            let mut offset = 4;
            for (&count, &k) in counts.iter().zip(&keep) {
                let end = offset + count * 8;
                let item = data.get(offset..end).ok_or("invalid UV tagg")?;
                if k {
                    new.extend_from_slice(item);
                }
                offset = end;
            }
            if offset != data.len() {
                return Err("invalid UV tagg size".into());
            }
        } else if !matches!(
            name,
            b"#SharpEdges#" | b"#Property#" | b"#Mass#" | b"#EndOfFile#"
        ) {
            if data.len() != np + nf {
                return Err("invalid selection tagg size".into());
            }
            new.extend_from_slice(&data[..np]);
            for (&v, &k) in data[np..].iter().zip(&keep) {
                if k {
                    new.push(if name == b"#Lock#" {
                        u8::from(v != 0)
                    } else {
                        v
                    });
                }
            }
        } else {
            new.extend_from_slice(data);
        }
        out.push(1);
        out.extend_from_slice(name);
        out.push(0);
        put(&mut out, new.len())?;
        out.extend_from_slice(&new);
    }
    out.extend_from_slice(&res.to_le_bytes());
    Ok((out, removed))
}
pub fn remove_degenerate_faces(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    let run = || -> Result<Vec<u8>, String> {
        let mut r = Reader {
            bytes: data,
            pos: 0,
        };
        if r.take(4)? != b"MLOD" {
            return Err("remove-degenerate-faces needs an unbinarised (MLOD) P3D".into());
        }
        if r.u32()? != 257 {
            return Err("invalid MLOD version".into());
        }
        let n = r.count()?;
        let min = op
            .get("minArea")
            .map(|v| v.as_f64().ok_or("invalid minArea"))
            .transpose()?
            .unwrap_or(1e-8);
        let filters = op
            .get("lods")
            .map(|v| {
                v.as_array()
                    .ok_or("invalid lods")?
                    .iter()
                    .map(|s| resolution(s.as_str().ok_or("invalid lod name")?))
                    .collect::<Result<Vec<_>, String>>()
            })
            .transpose()?;
        let mut out = data[..12].to_vec();
        let mut removed = 0;
        for _ in 0..n {
            let (bytes, count) = lod(&mut r, min, &filters)?;
            out.extend_from_slice(&bytes);
            removed += count;
        }
        if r.pos != data.len() {
            return Err("MLOD has trailing data".into());
        }
        if let Some(expected) = op.get("expectedRemoved") {
            let expected =
                i64::from(crate::op_json::int32(expected).ok_or("invalid expectedRemoved")?);
            if expected != removed as i64 {
                return Err(format!(
                    "expected to remove {expected} degenerate faces, found {removed}"
                ));
            }
        }
        if removed == 0 {
            return Err("no degenerate faces to remove".into());
        }
        Ok(out)
    };
    run().map_err(|e| format!("{label}: {e}"))
}
