use super::BoneWeight;
use super::decode::{bone_refs, i32s, normals, vec3s};
use super::io::{Blob, Reader};
use super::material::Material;
use super::mesh::{self, Keyframe, Proxy, Section, Selection};
use super::uv::UvSet;
use super::write::Writer;

#[derive(Clone)]
pub struct Loadable {
    pub faces: i32,
    pub color: u32,
    pub special: i32,
    pub or_hints: u32,
    pub skeleton: u8,
    pub vertices: i32,
    pub face_area: f32,
}

pub struct Lod {
    pub resolution: f32,
    pub proxies: Vec<Proxy>,
    pub sub_to_skeleton: Vec<i32>,
    pub skeleton_to_sub: Vec<Vec<i32>>,
    pub vertex_count: u32,
    pub face_area: f32,
    pub or_hints: i32,
    pub and_hints: i32,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
    pub bounds_center: [f32; 3],
    pub bounds_radius: f32,
    pub textures: Vec<Vec<u8>>,
    pub materials: Vec<Material>,
    pub point_to_vertex: Vec<i32>,
    pub vertex_to_point: Vec<i32>,
    pub face_bytes: u32,
    pub face_flags: u16,
    pub faces: Vec<Vec<i32>>,
    pub sections: Vec<Section>,
    pub selections: Vec<Selection>,
    pub properties: Vec<(Vec<u8>, Vec<u8>)>,
    pub frames: Vec<Keyframe>,
    pub color_top: i32,
    pub color: i32,
    pub special: i32,
    pub bone_ref_simple: u8,
    pub rest_size: u32,
    pub clip: Vec<i32>,
    pub uv_count: u32,
    pub uv: Vec<UvSet>,
    pub vertices: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub bones: Vec<BoneWeight>,
    pub unknown: u32,
    pub unknown68: u8,
    pub tail: Vec<u8>,
    pub(crate) point_to_vertex_origin: Blob,
    pub(crate) vertex_to_point_origin: Blob,
    pub(crate) clip_origin: Blob,
    pub(crate) vertices_origin: Blob,
    pub(crate) normals_origin: Blob,
    pub(crate) st_origin: Blob,
    pub(crate) bones_origin: Blob,
    pub(crate) neighbors_origin: Blob,
}

impl Loadable {
    pub fn read(r: &mut Reader<'_>) -> Result<Self, String> {
        Ok(Self {
            faces: r.i32()?,
            color: r.u32()?,
            special: r.i32()?,
            or_hints: r.u32()?,
            skeleton: r.u8()?,
            vertices: r.i32()?,
            face_area: r.f32()?,
        })
    }

    pub fn write(&self, w: &mut Writer) {
        w.i32(self.faces);
        w.u32(self.color);
        w.i32(self.special);
        w.u32(self.or_hints);
        w.u8(self.skeleton);
        w.i32(self.vertices);
        w.f32(self.face_area);
    }
}

impl Lod {
    pub fn read(r: &mut Reader<'_>, resolution: f32) -> Result<Self, String> {
        let proxy_count = r.count()?;
        r.limit(proxy_count, 65)?;
        let mut proxies = Vec::with_capacity(proxy_count);
        for _ in 0..proxy_count {
            proxies.push(Proxy::read(r)?);
        }
        let sub_count = r.count()?;
        let sub_to_skeleton = r.i32s(sub_count)?;
        let map_count = r.count()?;
        r.limit(map_count, 4)?;
        let mut skeleton_to_sub = Vec::with_capacity(map_count);
        for _ in 0..map_count {
            let n = r.count()?;
            skeleton_to_sub.push(r.i32s(n)?);
        }
        let vertex_count = r.u32()?;
        let face_area = r.f32()?;
        let or_hints = r.i32()?;
        let and_hints = r.i32()?;
        let bounds_min = r.vec3()?;
        let bounds_max = r.vec3()?;
        let bounds_center = r.vec3()?;
        let bounds_radius = r.f32()?;
        let textures = r.strings()?;
        let materials = mesh::materials(r)?;
        let (point_to_vertex_origin, point_raw) = r.packed_array(4)?;
        let (vertex_to_point_origin, vertex_raw) = r.packed_array(4)?;
        let (face_bytes, face_flags, faces) = mesh::faces(r)?;
        let section_count = r.count()?;
        r.limit(section_count, 38)?;
        let mut sections = Vec::with_capacity(section_count);
        for _ in 0..section_count {
            sections.push(Section::read(r)?);
        }
        let selection_count = r.count()?;
        r.limit(selection_count, 5)?;
        let mut selections = Vec::with_capacity(selection_count);
        for _ in 0..selection_count {
            selections.push(Selection::read(r)?);
        }
        let property_count = r.count()?;
        r.limit(property_count, 2)?;
        let mut properties = Vec::with_capacity(property_count);
        for _ in 0..property_count {
            properties.push((r.asciiz()?, r.asciiz()?));
        }
        let frame_count = r.count()?;
        r.limit(frame_count, 8)?;
        let mut frames = Vec::with_capacity(frame_count);
        for _ in 0..frame_count {
            frames.push(Keyframe::read(r)?);
        }
        let color_top = r.i32()?;
        let color = r.i32()?;
        let special = r.i32()?;
        let bone_ref_simple = r.u8()?;
        let rest_size = r.u32()?;
        let (clip_origin, clip_raw) = r.condensed(4)?;
        let uv0 = UvSet::read(r)?;
        let uv_count = r.u32()?;
        r.limit((uv_count as usize).saturating_sub(1), 21)?;
        let mut uv = vec![uv0];
        for _ in 1..uv_count.max(1) {
            uv.push(UvSet::read(r)?);
        }
        let (vertices_origin, vertices_raw) = r.packed_array(12)?;
        let (normals_origin, normals_raw) = r.condensed(4)?;
        let (st_origin, _) = r.packed_array(8)?;
        let (bones_origin, bones_raw) = r.packed_array(12)?;
        let (neighbors_origin, _) = r.packed_array(32)?;
        Ok(Self {
            resolution,
            proxies,
            sub_to_skeleton,
            skeleton_to_sub,
            vertex_count,
            face_area,
            or_hints,
            and_hints,
            bounds_min,
            bounds_max,
            bounds_center,
            bounds_radius,
            textures,
            materials,
            point_to_vertex: i32s(&point_raw)?,
            vertex_to_point: i32s(&vertex_raw)?,
            face_bytes,
            face_flags,
            faces,
            sections,
            selections,
            properties,
            frames,
            color_top,
            color,
            special,
            bone_ref_simple,
            rest_size,
            clip: i32s(&clip_raw)?,
            uv_count,
            uv,
            vertices: vec3s(&vertices_raw)?,
            normals: normals(&normals_raw)?,
            bones: bone_refs(&bones_raw)?,
            unknown: r.u32()?,
            unknown68: r.u8()?,
            tail: Vec::new(),
            point_to_vertex_origin,
            vertex_to_point_origin,
            clip_origin,
            vertices_origin,
            normals_origin,
            st_origin,
            bones_origin,
            neighbors_origin,
        })
    }

    pub fn write(&self, w: &mut Writer) -> Result<(), String> {
        w.count(self.proxies.len())?;
        for proxy in &self.proxies {
            proxy.write(w);
        }
        w.i32s(&self.sub_to_skeleton)?;
        w.count(self.skeleton_to_sub.len())?;
        for row in &self.skeleton_to_sub {
            w.i32s(row)?;
        }
        w.u32(self.vertex_count);
        w.f32(self.face_area);
        w.i32(self.or_hints);
        w.i32(self.and_hints);
        w.vec3(&self.bounds_min);
        w.vec3(&self.bounds_max);
        w.vec3(&self.bounds_center);
        w.f32(self.bounds_radius);
        w.strings(&self.textures)?;
        w.count(self.materials.len())?;
        for material in &self.materials {
            material.write(w)?;
        }
        w.blob(&self.point_to_vertex_origin);
        w.blob(&self.vertex_to_point_origin);
        w.i32(i32::try_from(self.faces.len()).map_err(|_| "face count overflow")?);
        w.u32(self.face_bytes);
        w.u16(self.face_flags);
        for face in &self.faces {
            w.u8(u8::try_from(face.len()).map_err(|_| "face vertex overflow")?);
            for index in face {
                w.i32(*index);
            }
        }
        w.count(self.sections.len())?;
        for section in &self.sections {
            section.write(w)?;
        }
        w.count(self.selections.len())?;
        for selection in &self.selections {
            selection.write(w);
        }
        w.count(self.properties.len())?;
        for (key, value) in &self.properties {
            w.asciiz(key);
            w.asciiz(value);
        }
        w.count(self.frames.len())?;
        for frame in &self.frames {
            frame.write(w)?;
        }
        w.i32(self.color_top);
        w.i32(self.color);
        w.i32(self.special);
        w.u8(self.bone_ref_simple);
        w.u32(self.rest_size);
        w.blob(&self.clip_origin);
        let first = self.uv.first().ok_or("LOD is missing its UV set")?;
        first.write(w);
        w.u32(self.uv_count);
        for set in self.uv.iter().skip(1) {
            set.write(w);
        }
        w.blob(&self.vertices_origin);
        w.blob(&self.normals_origin);
        w.blob(&self.st_origin);
        w.blob(&self.bones_origin);
        w.blob(&self.neighbors_origin);
        w.u32(self.unknown);
        w.u8(self.unknown68);
        Ok(())
    }
}
