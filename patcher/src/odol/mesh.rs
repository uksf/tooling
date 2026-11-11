use super::decode::i32s;
use super::io::{Blob, Reader};
use super::material::Material;
use super::write::Writer;

#[derive(Clone)]
pub struct Proxy {
    pub model: Vec<u8>,
    pub transform: [f32; 12],
    pub sequence: i32,
    pub selection: i32,
    pub bone: i32,
    pub section: i32,
}

#[derive(Clone)]
pub struct Section {
    pub face_lower: i32,
    pub face_upper: i32,
    pub min_bone: i32,
    pub bones: i32,
    pub unused: u32,
    pub texture: i16,
    pub special: u32,
    pub material: i32,
    pub material_name: Vec<u8>,
    pub area_over_tex: Vec<f32>,
    pub flag: i32,
    pub extra: Vec<f32>,
}

pub struct Selection {
    pub name: Vec<u8>,
    pub faces: Vec<i32>,
    pub sections: Vec<i32>,
    pub vertices: Vec<i32>,
    pub weights: Vec<u8>,
    pub unused: i32,
    pub is_sectional: u8,
    pub weight_size: i32,
    pub(crate) faces_origin: Blob,
    pub(crate) sections_origin: Blob,
    pub(crate) vertices_origin: Blob,
    pub(crate) weights_origin: Blob,
}

pub struct Keyframe {
    pub time: f32,
    pub points: Vec<[f32; 3]>,
}

impl Proxy {
    pub fn read(r: &mut Reader<'_>) -> Result<Self, String> {
        let model = r.asciiz()?;
        let mut transform = [0.0; 12];
        for value in &mut transform {
            *value = r.f32()?;
        }
        Ok(Self {
            model,
            transform,
            sequence: r.i32()?,
            selection: r.i32()?,
            bone: r.i32()?,
            section: r.i32()?,
        })
    }

    pub fn write(&self, w: &mut Writer) {
        w.asciiz(&self.model);
        for value in self.transform {
            w.f32(value);
        }
        w.i32(self.sequence);
        w.i32(self.selection);
        w.i32(self.bone);
        w.i32(self.section);
    }
}

impl Section {
    pub fn read(r: &mut Reader<'_>) -> Result<Self, String> {
        let face_lower = r.i32()?;
        let face_upper = r.i32()?;
        let min_bone = r.i32()?;
        let bones = r.i32()?;
        let unused = r.u32()?;
        let texture = r.i16()?;
        let special = r.u32()?;
        let material = r.i32()?;
        let material_name = if material == -1 {
            r.asciiz()?
        } else {
            Vec::new()
        };
        let area_count = r.count()?;
        let mut area_over_tex = Vec::with_capacity(area_count);
        for _ in 0..area_count {
            area_over_tex.push(r.f32()?);
        }
        let flag = r.i32()?;
        let mut extra = Vec::new();
        if flag >= 1 {
            for _ in 0..11 {
                extra.push(r.f32()?);
            }
        }
        Ok(Self {
            face_lower,
            face_upper,
            min_bone,
            bones,
            unused,
            texture,
            special,
            material,
            material_name,
            area_over_tex,
            flag,
            extra,
        })
    }

    pub fn write(&self, w: &mut Writer) -> Result<(), String> {
        w.i32(self.face_lower);
        w.i32(self.face_upper);
        w.i32(self.min_bone);
        w.i32(self.bones);
        w.u32(self.unused);
        w.u16(self.texture as u16);
        w.u32(self.special);
        w.i32(self.material);
        if self.material == -1 {
            w.asciiz(&self.material_name);
        }
        w.f32s(&self.area_over_tex)?;
        w.i32(self.flag);
        if self.flag >= 1 {
            for value in &self.extra {
                w.f32(*value);
            }
        }
        Ok(())
    }
}

impl Selection {
    pub fn read(r: &mut Reader<'_>) -> Result<Self, String> {
        let name = r.asciiz()?;
        let (faces_origin, faces_raw) = r.packed_array(4)?;
        let unused = r.i32()?;
        let is_sectional = r.u8()?;
        let (sections_origin, sections_raw) = r.packed_array(4)?;
        let (vertices_origin, vertices_raw) = r.packed_array(4)?;
        let weight_size = r.i32()?;
        let expected =
            usize::try_from(weight_size).map_err(|_| r.err("negative selection weights"))?;
        let (weights_origin, weights) = r.packed_payload(expected)?;
        Ok(Self {
            name,
            faces: i32s(&faces_raw)?,
            sections: i32s(&sections_raw)?,
            vertices: i32s(&vertices_raw)?,
            weights,
            unused,
            is_sectional,
            weight_size,
            faces_origin,
            sections_origin,
            vertices_origin,
            weights_origin,
        })
    }

    pub fn write(&self, w: &mut Writer) {
        w.asciiz(&self.name);
        w.blob(&self.faces_origin);
        w.i32(self.unused);
        w.u8(self.is_sectional);
        w.blob(&self.sections_origin);
        w.blob(&self.vertices_origin);
        w.i32(self.weight_size);
        w.blob(&self.weights_origin);
    }
}

impl Keyframe {
    pub fn read(r: &mut Reader<'_>) -> Result<Self, String> {
        let time = r.f32()?;
        let count = r.count()?;
        let mut points = Vec::with_capacity(count);
        for _ in 0..count {
            points.push(r.vec3()?);
        }
        Ok(Self { time, points })
    }

    pub fn write(&self, w: &mut Writer) -> Result<(), String> {
        w.f32(self.time);
        w.count(self.points.len())?;
        for point in &self.points {
            w.vec3(point);
        }
        Ok(())
    }
}

pub fn faces(r: &mut Reader<'_>) -> Result<(u32, u16, Vec<Vec<i32>>), String> {
    let count = r.count()?;
    let unused1 = r.u32()?;
    let unused2 = r.u16()?;
    let mut faces = Vec::with_capacity(count);
    for _ in 0..count {
        let n = usize::from(r.u8()?);
        faces.push(r.i32s(n)?);
    }
    Ok((unused1, unused2, faces))
}

pub fn materials(r: &mut Reader<'_>) -> Result<Vec<Material>, String> {
    let count = r.count()?;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        out.push(Material::read(r)?);
    }
    Ok(out)
}
