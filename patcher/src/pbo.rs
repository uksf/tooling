use sha1::{Digest, Sha1};
use std::collections::{HashMap, HashSet};

const VERSION: u32 = 0x5665_7273;
const COMPRESSED: u32 = 0x4370_7273;

#[derive(Debug)]
pub struct Entry {
    pub name: Vec<u8>,
    pub fields: usize,
    pub packing: u32,
    pub original_size: u32,
    pub timestamp: u32,
    pub size: u32,
    pub offset: usize,
}

#[derive(Debug)]
pub struct Pbo {
    pub bytes: Vec<u8>,
    pub entries: Vec<Entry>,
    pub prefix: Vec<u8>,
    pub header_len: usize,
}

pub struct Addition {
    pub name: Vec<u8>,
    pub timestamp: u32,
    pub data: Vec<u8>,
}

fn zero_string(bytes: &[u8], pos: &mut usize) -> Result<Vec<u8>, String> {
    let tail = bytes.get(*pos..).ok_or("unterminated name in header")?;
    let len = tail
        .iter()
        .position(|b| *b == 0)
        .ok_or("unterminated name in header")?;
    let result = tail[..len].to_vec();
    *pos += len + 1;
    Ok(result)
}

fn word(bytes: &[u8], pos: usize) -> Result<u32, String> {
    let value = bytes
        .get(pos..pos + 4)
        .ok_or("header runs past the end of the file")?;
    Ok(u32::from_le_bytes(
        value.try_into().map_err(|_| "invalid word")?,
    ))
}

fn normalise(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .skip_while(|b| **b == 92 || **b == b'/')
        .map(|b| match b {
            b'/' => 92,
            0xc0..=0xd6 | 0xd8..=0xde => b + 32,
            _ => b.to_ascii_lowercase(),
        })
        .collect()
}

impl Pbo {
    pub fn parse(bytes: Vec<u8>) -> Result<Self, String> {
        let mut entries = Vec::new();
        let mut prefix = Vec::new();
        let mut pos = 0;
        loop {
            let name = zero_string(&bytes, &mut pos)?;
            let fields = pos;
            let packing = word(&bytes, pos)?;
            let original_size = word(&bytes, pos + 4)?;
            let timestamp = word(&bytes, pos + 12)?;
            let size = word(&bytes, pos + 16)?;
            pos += 20;
            if name.is_empty() && packing == VERSION {
                loop {
                    let key = zero_string(&bytes, &mut pos)?;
                    if key.is_empty() {
                        break;
                    }
                    let value = zero_string(&bytes, &mut pos)?;
                    if key.eq_ignore_ascii_case(b"prefix") {
                        prefix = value;
                    }
                }
            } else if name.is_empty() {
                break;
            } else {
                entries.push(Entry {
                    name,
                    fields,
                    packing,
                    original_size,
                    timestamp,
                    size,
                    offset: 0,
                });
            }
        }
        let header_len = pos;
        for entry in &mut entries {
            entry.offset = pos;
            pos = pos
                .checked_add(entry.size as usize)
                .ok_or("PBO data size overflow")?;
        }
        if pos.checked_add(21) != Some(bytes.len()) {
            return Err("data blocks and trailer do not fill the file".into());
        }
        if bytes[pos] != 0 {
            return Err("trailer does not start with a zero byte".into());
        }
        Ok(Self {
            bytes,
            entries,
            prefix,
            header_len,
        })
    }

    pub fn trailer_valid(&self) -> bool {
        let end = self.bytes.len() - 21;
        Sha1::digest(&self.bytes[..end]).as_slice() == &self.bytes[end + 1..]
    }

    pub fn find(&self, path: &[u8]) -> Result<Option<usize>, String> {
        let mut wanted = normalise(path);
        let prefix = normalise(&self.prefix);
        if !prefix.is_empty()
            && wanted.starts_with(&prefix)
            && wanted.get(prefix.len()) == Some(&92)
        {
            wanted.drain(..prefix.len() + 1);
        }
        let matches: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| normalise(&e.name) == wanted)
            .map(|(i, _)| i)
            .collect();
        match matches.len() {
            0 => Ok(None),
            1 => Ok(Some(matches[0])),
            n => Err(format!(
                "{n} entries are named '{}'",
                String::from_utf8_lossy(path)
            )),
        }
    }

    pub fn data(&self, index: usize) -> Result<Vec<u8>, String> {
        let e = self.entries.get(index).ok_or("entry index out of range")?;
        let data = &self.bytes[e.offset..e.offset + e.size as usize];
        if e.packing != COMPRESSED {
            return Ok(data.to_vec());
        }
        let mut output = vec![0; e.original_size as usize];
        if output.is_empty() {
            return Ok(output);
        }
        let consumed = hemtt_lzo::lz77::decompress(data, &mut output).map_err(str::to_owned)?;
        let checksum = u32::from_le_bytes(
            data[consumed - 4..consumed]
                .try_into()
                .map_err(|_| "invalid checksum")?,
        );
        let actual = output
            .iter()
            .fold(0u32, |sum, byte| sum.wrapping_add(u32::from(*byte)));
        if checksum != actual {
            return Err("Checksum mismatch".into());
        }
        Ok(output)
    }

    pub fn rebuild(
        &self,
        replacements: &HashMap<usize, Vec<u8>>,
        additions: &[Addition],
        removals: &HashSet<usize>,
    ) -> Result<Vec<u8>, String> {
        if replacements
            .keys()
            .chain(removals.iter())
            .any(|i| *i >= self.entries.len())
        {
            return Err("entry index out of range".into());
        }
        let mut header = self.bytes[..self.header_len].to_vec();
        for (&index, data) in replacements {
            let size = u32::try_from(data.len())
                .map_err(|_| "entry too large")?
                .to_le_bytes();
            let at = self.entries[index].fields;
            header[at..at + 4].copy_from_slice(&0u32.to_le_bytes());
            header[at + 4..at + 8].copy_from_slice(&size);
            header[at + 16..at + 20].copy_from_slice(&size);
        }
        for index in (0..self.entries.len())
            .rev()
            .filter(|i| removals.contains(i))
        {
            let e = &self.entries[index];
            header.drain(e.fields - e.name.len() - 1..e.fields + 20);
        }
        let terminator = header.len() - 21;
        let mut extra = Vec::new();
        for addition in additions {
            let size = u32::try_from(addition.data.len()).map_err(|_| "entry too large")?;
            extra.extend_from_slice(&addition.name);
            extra.push(0);
            extra.extend_from_slice(&0u32.to_le_bytes());
            extra.extend_from_slice(&size.to_le_bytes());
            extra.extend_from_slice(&0u32.to_le_bytes());
            extra.extend_from_slice(&addition.timestamp.to_le_bytes());
            extra.extend_from_slice(&size.to_le_bytes());
        }
        header.splice(terminator..terminator, extra);
        for (index, e) in self.entries.iter().enumerate() {
            if removals.contains(&index) {
                continue;
            }
            if let Some(data) = replacements.get(&index) {
                header.extend_from_slice(data);
            } else {
                header.extend_from_slice(&self.bytes[e.offset..e.offset + e.size as usize]);
            }
        }
        for addition in additions {
            header.extend_from_slice(&addition.data);
        }
        let hash = Sha1::digest(&header);
        header.push(0);
        header.extend_from_slice(&hash);
        Ok(header)
    }
}
