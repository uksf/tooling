use super::io::Blob;

pub struct Writer {
    buf: Vec<u8>,
    pos: usize,
}

impl Writer {
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
            pos: 0,
        }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn set_pos(&mut self, pos: usize) {
        self.pos = pos;
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.buf
    }

    pub fn bytes(&mut self, data: &[u8]) {
        let end = self.pos + data.len();
        if end > self.buf.len() {
            self.buf.resize(end, 0);
        }
        self.buf[self.pos..end].copy_from_slice(data);
        self.pos = end;
    }

    pub fn u8(&mut self, value: u8) {
        self.bytes(&[value]);
    }

    pub fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    pub fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    pub fn i32(&mut self, value: i32) {
        self.u32(value as u32);
    }

    pub fn f32(&mut self, value: f32) {
        self.bytes(&value.to_le_bytes());
    }

    pub fn vec3(&mut self, value: &[f32; 3]) {
        for item in value {
            self.f32(*item);
        }
    }

    pub fn count(&mut self, len: usize) -> Result<(), String> {
        self.i32(i32::try_from(len).map_err(|_| "ODOL array too large")?);
        Ok(())
    }

    pub fn asciiz(&mut self, value: &[u8]) {
        self.bytes(value);
        self.u8(0);
    }

    pub fn strings(&mut self, values: &[Vec<u8>]) -> Result<(), String> {
        self.count(values.len())?;
        for value in values {
            self.asciiz(value);
        }
        Ok(())
    }

    pub fn i32s(&mut self, values: &[i32]) -> Result<(), String> {
        self.count(values.len())?;
        for value in values {
            self.i32(*value);
        }
        Ok(())
    }

    pub fn f32s(&mut self, values: &[f32]) -> Result<(), String> {
        self.count(values.len())?;
        for value in values {
            self.f32(*value);
        }
        Ok(())
    }

    pub fn blob(&mut self, blob: &Blob) {
        self.bytes(&blob.origin);
    }

    pub fn here(&self) -> Result<u32, String> {
        u32::try_from(self.pos).map_err(|_| "ODOL exceeds 4 GiB".into())
    }
}
