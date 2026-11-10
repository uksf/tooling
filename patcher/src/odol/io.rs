use super::lzo;

pub struct Reader<'a> {
    data: &'a [u8],
    pub pos: usize,
}

#[derive(Clone)]
pub struct Blob {
    pub origin: Vec<u8>,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn err(&self, msg: &str) -> String {
        format!("{msg} at {}", self.pos)
    }

    pub fn u8(&mut self) -> Result<u8, String> {
        let byte = *self
            .data
            .get(self.pos)
            .ok_or_else(|| self.err("truncated ODOL"))?;
        self.pos += 1;
        Ok(byte)
    }

    pub fn bytes(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| self.err("truncated ODOL"))?;
        let slice = self
            .data
            .get(self.pos..end)
            .ok_or_else(|| self.err("truncated ODOL"))?;
        self.pos = end;
        Ok(slice)
    }

    pub fn u16(&mut self) -> Result<u16, String> {
        let b = self.bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub fn i16(&mut self) -> Result<i16, String> {
        Ok(self.u16()? as i16)
    }

    pub fn u32(&mut self) -> Result<u32, String> {
        let b = self.bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn i32(&mut self) -> Result<i32, String> {
        Ok(self.u32()? as i32)
    }

    pub fn f32(&mut self) -> Result<f32, String> {
        Ok(f32::from_bits(self.u32()?))
    }

    pub fn vec3(&mut self) -> Result<[f32; 3], String> {
        Ok([self.f32()?, self.f32()?, self.f32()?])
    }

    pub fn count(&mut self) -> Result<usize, String> {
        let n = self.i32()?;
        usize::try_from(n).map_err(|_| self.err("negative ODOL array"))
    }

    pub fn u32s(&mut self, n: usize) -> Result<Vec<u32>, String> {
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(self.u32()?);
        }
        Ok(out)
    }

    pub fn i32s(&mut self, n: usize) -> Result<Vec<i32>, String> {
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(self.i32()?);
        }
        Ok(out)
    }

    pub fn asciiz(&mut self) -> Result<Vec<u8>, String> {
        let tail = self
            .data
            .get(self.pos..)
            .ok_or_else(|| self.err("truncated ODOL"))?;
        let len = tail
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| self.err("unterminated ODOL string"))?;
        let value = tail[..len].to_vec();
        self.pos += len + 1;
        Ok(value)
    }

    pub fn strings(&mut self) -> Result<Vec<Vec<u8>>, String> {
        let n = self.count()?;
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(self.asciiz()?);
        }
        Ok(out)
    }

    fn span(&self, start: usize) -> Vec<u8> {
        self.data[start..self.pos].to_vec()
    }

    pub fn payload(&mut self, expected: usize) -> Result<Vec<u8>, String> {
        if expected == 0 {
            return Ok(Vec::new());
        }
        let flag = self.u8()?;
        if flag == 0 {
            return Ok(self.bytes(expected)?.to_vec());
        }
        let (out, consumed) =
            lzo::decompress(&self.data[self.pos..], expected).map_err(|e| self.err(&e))?;
        self.pos += consumed;
        Ok(out)
    }

    pub fn packed_array(&mut self, elem: usize) -> Result<(Blob, Vec<u8>), String> {
        let start = self.pos;
        let count = self.count()?;
        let expected = count
            .checked_mul(elem)
            .ok_or_else(|| self.err("ODOL array overflow"))?;
        let raw = self.payload(expected)?;
        Ok((
            Blob {
                origin: self.span(start),
            },
            raw,
        ))
    }

    pub fn packed_payload(&mut self, expected: usize) -> Result<(Blob, Vec<u8>), String> {
        let start = self.pos;
        let raw = self.payload(expected)?;
        Ok((
            Blob {
                origin: self.span(start),
            },
            raw,
        ))
    }

    pub fn condensed(&mut self, elem: usize) -> Result<(Blob, Vec<u8>), String> {
        let start = self.pos;
        let count = self.count()?;
        let fill = self.u8()?;
        let raw = if fill != 0 {
            let one = self.bytes(elem)?.to_vec();
            let mut raw = Vec::with_capacity(count.saturating_mul(elem));
            for _ in 0..count {
                raw.extend_from_slice(&one);
            }
            raw
        } else {
            let expected = count
                .checked_mul(elem)
                .ok_or_else(|| self.err("ODOL array overflow"))?;
            self.payload(expected)?
        };
        Ok((
            Blob {
                origin: self.span(start),
            },
            raw,
        ))
    }
}
