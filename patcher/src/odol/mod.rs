mod anim;
mod compress;
mod decode;
pub(crate) mod edit_arrays;
mod info;
mod io;
mod lod;
mod lzo;
mod material;
mod mesh;
pub(crate) mod mesh_arrays;
mod skeleton_data;
pub mod strip;
mod uv;
mod write;

pub use anim::{Animation, Animations};
pub use info::ModelInfo;
pub use lod::{Loadable, Lod};
pub use mesh::{Proxy, Section, Selection};
pub use skeleton_data::{Bone, Skeleton};
pub use uv::UvSet;

use io::Reader;
use write::Writer;

#[derive(Debug, PartialEq)]
pub struct Header {
    pub version: u32,
    pub app_id: u32,
    pub muzzle_flash: Vec<u8>,
    pub resolutions: Vec<f32>,
    pub model_info_offset: usize,
}

pub struct BoneWeight {
    pub count: i32,
    pub data: [u8; 8],
}

pub struct Model {
    pub version: u32,
    pub app_id: u32,
    pub muzzle_flash: Vec<u8>,
    pub info: ModelInfo,
    pub animations: Option<Animations>,
    pub has_animations: u8,
    pub lods: Vec<Lod>,
    pub permanent: Vec<u8>,
    pub loadable: Vec<Option<Loadable>>,
    pub gaps: Vec<Vec<u8>>,
    pub extra: Vec<u8>,
    order: Vec<usize>,
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

impl Model {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let header = Header::parse(bytes)?;
        let mut reader = Reader::new(bytes);
        reader.pos = header.model_info_offset;
        let lods_n = header.resolutions.len();
        let info = ModelInfo::read(&mut reader, lods_n)?;
        let has_animations = reader.u8()?;
        let animations = if has_animations != 0 {
            Some(Animations::read(&mut reader)?)
        } else {
            None
        };
        let starts = reader.u32s(lods_n)?;
        let ends = reader.u32s(lods_n)?;
        let permanent = reader.bytes(lods_n)?.to_vec();
        let mut loadable = Vec::with_capacity(lods_n);
        for flag in &permanent {
            if *flag == 0 {
                loadable.push(Some(Loadable::read(&mut reader)?));
            } else {
                loadable.push(None);
            }
        }
        let after = reader.pos;
        let mut lods = Vec::with_capacity(lods_n);
        for index in 0..lods_n {
            let start = starts[index] as usize;
            let end = ends[index] as usize;
            if start > end || end > bytes.len() {
                return Err(format!(
                    "LOD {index} range {start}..{end} is outside the file"
                ));
            }
            reader.pos = start;
            let mut lod = Lod::read(&mut reader, header.resolutions[index])?;
            if reader.pos > end {
                return Err(format!(
                    "LOD {index} parsed past its end address ({} > {end})",
                    reader.pos
                ));
            }
            lod.tail = bytes[reader.pos..end].to_vec();
            lods.push(lod);
        }
        let mut order: Vec<usize> = (0..lods_n).collect();
        order.sort_by_key(|&index| starts[index]);
        let mut gaps = Vec::with_capacity(lods_n);
        let mut prev = after;
        for &index in &order {
            let start = starts[index] as usize;
            if start < prev {
                return Err(format!("LOD ranges overlap at {start}"));
            }
            gaps.push(bytes[prev..start].to_vec());
            prev = ends[index] as usize;
        }
        Ok(Self {
            version: header.version,
            app_id: header.app_id,
            muzzle_flash: header.muzzle_flash,
            info,
            animations,
            has_animations,
            lods,
            permanent,
            loadable,
            gaps,
            extra: bytes[prev..].to_vec(),
            order,
        })
    }

    pub fn write(&self) -> Result<Vec<u8>, String> {
        let mut w = Writer::new();
        w.bytes(b"ODOL");
        w.u32(self.version);
        if self.version >= 75 {
            w.bytes(&[0; 8]);
        }
        w.u32(self.app_id);
        w.asciiz(&self.muzzle_flash);
        w.f32s(
            &self
                .lods
                .iter()
                .map(|lod| lod.resolution)
                .collect::<Vec<_>>(),
        )?;
        self.info.write(&mut w)?;
        w.u8(self.has_animations);
        if let Some(animations) = &self.animations {
            animations.write(&mut w)?;
        }
        let addresses = w.pos();
        for _ in 0..self.lods.len() * 2 {
            w.u32(0);
        }
        w.bytes(&self.permanent);
        for info in self.loadable.iter().flatten() {
            info.write(&mut w);
        }
        let mut starts = vec![0u32; self.lods.len()];
        let mut ends = vec![0u32; self.lods.len()];
        for (gap, &index) in self.gaps.iter().zip(&self.order) {
            w.bytes(gap);
            starts[index] = w.here()?;
            self.lods[index].write(&mut w)?;
            w.bytes(&self.lods[index].tail);
            ends[index] = w.here()?;
        }
        w.bytes(&self.extra);
        let end = w.pos();
        w.set_pos(addresses);
        for start in starts {
            w.u32(start);
        }
        for end_at in ends {
            w.u32(end_at);
        }
        w.set_pos(end);
        Ok(w.into_vec())
    }

    pub fn memory_lod(&self) -> Option<&Lod> {
        self.lods.get(usize::from(self.info.memory()))
    }
}

pub fn first_mismatch(left: &[u8], right: &[u8]) -> Option<usize> {
    left.iter()
        .zip(right)
        .position(|(a, b)| a != b)
        .or_else(|| (left.len() != right.len()).then(|| left.len().min(right.len())))
}
