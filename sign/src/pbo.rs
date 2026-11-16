use std::{
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom},
};

use hemtt_common::BISignVersion;
use sha1::{Digest, Sha1};

const VERS: u32 = 0x5665_7273;
const CPRS: u32 = 0x4370_7273;
const MAX_NAME: usize = 4096;

struct Entry {
    name: Vec<u8>,
    mime: u32,
    original: u32,
    size: u32,
}

impl Entry {
    fn is_empty(&self) -> bool {
        if self.mime == CPRS {
            self.original == 0
        } else {
            self.size == 0
        }
    }
}

pub type Hash = [u8; 20];

pub fn dssign_hashes(path: &str, version: BISignVersion) -> Result<[Hash; 3], String> {
    read_hashes(path, version).map_err(|e| format!("{path}: {e}"))
}

fn read_hashes(path: &str, version: BISignVersion) -> io::Result<[Hash; 3]> {
    let file = File::open(path)?;
    let file_length = file.metadata()?.len();
    let mut input = BufReader::new(file);
    let (prefix, entries) = read_headers(&mut input)?;
    let data_start = input.stream_position()?;
    let data_end = data_start + entries.iter().map(|e| u64::from(e.size)).sum::<u64>();
    if data_end + 21 != file_length {
        return Err(invalid(format!(
            "file data ends at {data_end} but the file is {file_length} bytes, expected one zero byte and a 20 byte checksum after the data"
        )));
    }
    input.seek(SeekFrom::Start(data_end))?;
    let mut trailer = [0u8; 21];
    input.read_exact(&mut trailer)?;
    if trailer[0] != 0 {
        return Err(invalid("missing zero byte before the PBO checksum".into()));
    }
    let checksum: Hash = trailer[1..].try_into().expect("20 byte slice");

    let names = names_hash(&entries)?;
    let files = files_hash(&mut input, data_start, &entries, version)?;
    let prefix = prefix_suffix(prefix);
    Ok([
        checksum,
        digest(&[&checksum, &names, &prefix]),
        digest(&[&files, &names, &prefix]),
    ])
}

fn read_headers<R: Read>(input: &mut R) -> io::Result<(Option<Vec<u8>>, Vec<Entry>)> {
    let mut prefix = None;
    let mut entries = Vec::new();
    loop {
        let name = read_cstring(input)?;
        let mut fields = [0u32; 5];
        for field in &mut fields {
            let mut bytes = [0u8; 4];
            input.read_exact(&mut bytes)?;
            *field = u32::from_le_bytes(bytes);
        }
        let [mime, original, _, _, size] = fields;
        if mime == VERS {
            loop {
                let key = read_cstring(input)?;
                if key.is_empty() {
                    break;
                }
                let value = read_cstring(input)?;
                if key == b"prefix" {
                    prefix = Some(value);
                }
            }
        } else if name.is_empty() {
            return Ok((prefix, entries));
        } else {
            entries.push(Entry {
                name,
                mime,
                original,
                size,
            });
        }
    }
}

fn read_cstring<R: Read>(input: &mut R) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        input.read_exact(&mut byte)?;
        if byte[0] == 0 {
            return Ok(bytes);
        }
        if bytes.len() == MAX_NAME {
            return Err(invalid(format!(
                "header string longer than {MAX_NAME} bytes"
            )));
        }
        bytes.push(byte[0]);
    }
}

fn names_hash(entries: &[Entry]) -> io::Result<Hash> {
    let mut names: Vec<Vec<u8>> = entries
        .iter()
        .filter(|e| !e.is_empty())
        .map(|e| e.name.to_ascii_lowercase())
        .collect();
    if names.is_empty() {
        return Err(invalid("PBO has no files".into()));
    }
    names.sort();
    names.dedup();
    let mut hasher = Sha1::new();
    for name in names {
        let name: Vec<u8> = name
            .into_iter()
            .map(|b| if b == b'/' { b'\\' } else { b })
            .collect();
        hasher.update(name);
    }
    Ok(hasher.finalize().into())
}

fn files_hash<R: Read + Seek>(
    input: &mut R,
    data_start: u64,
    entries: &[Entry],
    version: BISignVersion,
) -> io::Result<Hash> {
    let mut hasher = Sha1::new();
    let mut hashed = Vec::new();
    let mut offset = data_start;
    for entry in entries {
        if is_signed_file(&entry.name, version) {
            let name = entry.name.to_ascii_lowercase();
            if hashed.contains(&name) {
                return Err(invalid(format!(
                    "duplicate signed file {}",
                    String::from_utf8_lossy(&entry.name)
                )));
            }
            hashed.push(name);
            input.seek(SeekFrom::Start(offset))?;
            io::copy(&mut input.by_ref().take(u64::from(entry.size)), &mut hasher)?;
        }
        offset += u64::from(entry.size);
    }
    if hashed.is_empty() {
        hasher.update(version.nothing());
    }
    Ok(hasher.finalize().into())
}

fn is_signed_file(name: &[u8], version: BISignVersion) -> bool {
    let file_name = name
        .rsplit(|&b| b == b'\\' || b == b'/')
        .next()
        .unwrap_or(name);
    let Some(dot) = file_name.iter().rposition(|&b| b == b'.') else {
        return false;
    };
    std::str::from_utf8(&file_name[dot + 1..])
        .is_ok_and(|ext| version.should_hash_file(&format!("x.{}", ext.to_ascii_lowercase())))
}

fn prefix_suffix(prefix: Option<Vec<u8>>) -> Vec<u8> {
    match prefix {
        Some(mut prefix) if !prefix.is_empty() => {
            if !prefix.ends_with(b"\\") {
                prefix.push(b'\\');
            }
            prefix
        }
        _ => Vec::new(),
    }
}

fn digest(parts: &[&[u8]]) -> Hash {
    let mut hasher = Sha1::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().into()
}

fn invalid(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use hemtt_common::BISignVersion;

    use super::is_signed_file;

    #[test]
    fn signed_file_extension_comes_from_the_last_path_segment() {
        let v3 = BISignVersion::V3;
        assert!(is_signed_file(b"Alpha\\Upper.SQF", v3));
        assert!(is_signed_file(b".sqf", v3));
        assert!(is_signed_file(b"dir\\\xff\xfe.sqf", v3));
        assert!(!is_signed_file(b"scenes\\intro.Lythium\\noext", v3));
        assert!(!is_signed_file(b"data\\tex.paa", v3));
        assert!(is_signed_file(b"data\\tex.PAA", BISignVersion::V2));
    }
}
