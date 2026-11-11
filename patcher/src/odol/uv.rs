use super::decode::scale_uv;
use super::io::{Blob, Reader};
use super::write::Writer;

pub struct UvSet {
    pub min: [f32; 2],
    pub max: [f32; 2],
    pub vertices: u32,
    pub fill: u8,
    pub default_value: Vec<u8>,
    pub coords: Vec<[f32; 2]>,
    pub(crate) data: Blob,
}

impl UvSet {
    pub fn read(r: &mut Reader<'_>) -> Result<Self, String> {
        let min = [r.f32()?, r.f32()?];
        let max = [r.f32()?, r.f32()?];
        let vertices = r.u32()?;
        let fill = r.u8()?;
        let (default_value, data, raw) = if fill != 0 {
            (
                r.bytes(4)?.to_vec(),
                Blob { origin: Vec::new() },
                Vec::new(),
            )
        } else {
            let expected = (vertices as usize)
                .checked_mul(4)
                .ok_or_else(|| r.err("UV overflow"))?;
            let (data, raw) = r.packed_payload(expected)?;
            (Vec::new(), data, raw)
        };
        let coords = coords(&raw, &default_value, vertices, fill, min, max)?;
        Ok(Self {
            min,
            max,
            vertices,
            fill,
            default_value,
            coords,
            data,
        })
    }

    pub fn write(&self, w: &mut Writer) {
        w.f32(self.min[0]);
        w.f32(self.min[1]);
        w.f32(self.max[0]);
        w.f32(self.max[1]);
        w.u32(self.vertices);
        w.u8(self.fill);
        if self.fill != 0 {
            w.bytes(&self.default_value);
        } else {
            w.blob(&self.data);
        }
    }
}

fn coords(
    raw: &[u8],
    default_value: &[u8],
    vertices: u32,
    fill: u8,
    min: [f32; 2],
    max: [f32; 2],
) -> Result<Vec<[f32; 2]>, String> {
    let source = if fill != 0 { default_value } else { raw };
    if fill != 0 && source.len() != 4 {
        return Err("UV default size mismatch".into());
    }
    if fill == 0 && source.len() != vertices as usize * 4 {
        return Err("UV data size mismatch".into());
    }
    let du = f64::from(max[0] - min[0]);
    let dv = f64::from(max[1] - min[1]);
    let one = |chunk: &[u8; 4]| {
        [
            scale_uv(i16::from_le_bytes([chunk[0], chunk[1]]), du, min[0]),
            scale_uv(i16::from_le_bytes([chunk[2], chunk[3]]), dv, min[1]),
        ]
    };
    if fill != 0 {
        let chunk = source
            .as_chunks::<4>()
            .0
            .first()
            .ok_or("UV default size mismatch")?;
        let n = vertices as usize;
        let bytes = n
            .checked_mul(size_of::<[f32; 2]>())
            .ok_or("UV repeat overflow")?;
        crate::bound::repeat_fits(bytes)?;
        return Ok(vec![one(chunk); n]);
    }
    Ok(source.as_chunks::<4>().0.iter().map(one).collect())
}
