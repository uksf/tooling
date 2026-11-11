use super::{
    BoneWeight, Lod, Selection, UvSet, decode,
    edit_arrays::{array, condensed, payload},
    io::{Blob, Reader},
    write::Writer,
};

fn ints(values: &[i32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

pub fn refresh_rest(lod: &mut Lod) -> Result<(), String> {
    let mut w = Writer::new();
    w.blob(&lod.clip_origin);
    lod.uv
        .first()
        .ok_or("LOD is missing its UV set")?
        .write(&mut w);
    w.u32(lod.uv_count);
    for set in lod.uv.iter().skip(1) {
        set.write(&mut w);
    }
    for blob in [
        &lod.vertices_origin,
        &lod.normals_origin,
        &lod.st_origin,
        &lod.bones_origin,
        &lod.neighbors_origin,
    ] {
        w.blob(blob);
    }
    w.u32(lod.unknown);
    lod.rest_size = w.here()?;
    Ok(())
}

pub fn normals(lod: &Lod) -> Result<Vec<u8>, String> {
    Ok(Reader::new(&lod.normals_origin.origin).condensed(4)?.1)
}

pub fn set_normals(lod: &mut Lod, raw: &[u8]) -> Result<(), String> {
    lod.normals_origin = condensed(raw, 4)?;
    lod.normals = decode::normals(raw)?;
    Ok(())
}

pub fn st(lod: &Lod) -> Result<Vec<u8>, String> {
    Ok(Reader::new(&lod.st_origin.origin).packed_array(8)?.1)
}

pub fn set_st(lod: &mut Lod, raw: &[u8]) -> Result<(), String> {
    lod.st_origin = array(raw, raw.len() / 8)?;
    Ok(())
}

pub fn neighbours(lod: &Lod) -> Result<Vec<u8>, String> {
    Ok(Reader::new(&lod.neighbors_origin.origin)
        .packed_array(32)?
        .1)
}

pub fn set_neighbours(lod: &mut Lod, raw: &[u8]) -> Result<(), String> {
    lod.neighbors_origin = array(raw, raw.len() / 32)?;
    Ok(())
}

pub fn set_clip(lod: &mut Lod, clip: Vec<i32>) -> Result<(), String> {
    lod.clip_origin = condensed(&ints(&clip), 4)?;
    lod.clip = clip;
    Ok(())
}

pub fn set_vertex_to_point(lod: &mut Lod, map: Vec<i32>) -> Result<(), String> {
    lod.vertex_to_point_origin = array(&ints(&map), map.len())?;
    lod.vertex_to_point = map;
    Ok(())
}

pub fn set_bones(lod: &mut Lod, bones: Vec<BoneWeight>) -> Result<(), String> {
    let raw: Vec<u8> = bones
        .iter()
        .flat_map(|b| b.count.to_le_bytes().into_iter().chain(b.data))
        .collect();
    lod.bones_origin = array(&raw, bones.len())?;
    lod.bones = bones;
    Ok(())
}

pub fn append_vertex(
    lod: &mut Lod,
    position: [f32; 3],
    clip: i32,
    normal: [u8; 4],
    bone: BoneWeight,
) -> Result<i32, String> {
    if lod.uv.iter().any(|s| s.fill == 0) {
        return Err("memory LOD has per-vertex UV data, which memory edits do not support".into());
    }
    let v = lod.vertices.len();
    let mut vertices = lod.vertices.clone();
    vertices.push(position);
    super::edit_arrays::set_vertices(lod, vertices)?;
    let mut clips = lod.clip.clone();
    clips.push(clip);
    set_clip(lod, clips)?;
    let mut raw = normals(lod)?;
    raw.extend(normal);
    set_normals(lod, &raw)?;
    let mut bones = std::mem::take(&mut lod.bones);
    bones.push(bone);
    set_bones(lod, bones)?;
    lod.vertex_count = u32::try_from(v + 1).map_err(|_| "vertex count overflow")?;
    for set in &mut lod.uv {
        set.vertices = lod.vertex_count;
    }
    refresh_rest(lod)?;
    i32::try_from(v).map_err(|_| "vertex count overflow".into())
}

pub fn new_selection(
    name: Vec<u8>,
    sectional: bool,
    sections: &[i32],
    faces: &[i32],
    vertices: &[i32],
) -> Result<Selection, String> {
    Ok(Selection {
        name,
        faces: faces.to_vec(),
        sections: sections.to_vec(),
        vertices: vertices.to_vec(),
        weights: Vec::new(),
        unused: 0,
        is_sectional: u8::from(sectional),
        weight_size: 0,
        faces_origin: array(&ints(faces), faces.len())?,
        sections_origin: array(&ints(sections), sections.len())?,
        vertices_origin: array(&ints(vertices), vertices.len())?,
        weights_origin: Blob { origin: Vec::new() },
    })
}

pub fn set_selection_faces(s: &mut Selection, faces: Vec<i32>) -> Result<(), String> {
    s.faces_origin = array(&ints(&faces), faces.len())?;
    s.faces = faces;
    Ok(())
}

pub fn set_selection_sections(s: &mut Selection, sections: Vec<i32>) -> Result<(), String> {
    s.sections_origin = array(&ints(&sections), sections.len())?;
    s.sections = sections;
    Ok(())
}

pub fn set_selection_vertices(s: &mut Selection, vertices: Vec<i32>) -> Result<(), String> {
    s.vertices_origin = array(&ints(&vertices), vertices.len())?;
    s.vertices = vertices;
    Ok(())
}

pub fn set_selection_weights(s: &mut Selection, weights: Vec<u8>) -> Result<(), String> {
    s.weights_origin = Blob {
        origin: payload(&weights),
    };
    s.weight_size = i32::try_from(weights.len()).map_err(|_| "weights overflow")?;
    s.weights = weights;
    Ok(())
}

pub fn write_uv(set: &mut UvSet, uv: &[[f32; 2]]) -> Result<(), String> {
    if set.fill != 0 {
        return Err("UV set uses a default fill, which UV edits do not support".into());
    }
    let first = uv.first().ok_or("UV set has no vertices")?;
    let mut min = *first;
    let mut max = *first;
    for p in uv {
        for k in 0..2 {
            if p[k] < min[k] {
                min[k] = p[k];
            }
            if p[k] > max[k] {
                max[k] = p[k];
            }
        }
    }
    for k in 0..2 {
        max[k] = min[k] + (max[k] - min[k]) * 65536. / 65534.;
    }
    if min.iter().chain(&max).any(|x| !x.is_finite()) {
        return Err(format!(
            "UV bounds overflow ({},{})..({},{})",
            crate::dotnet_fmt::float(min[0]),
            crate::dotnet_fmt::float(min[1]),
            crate::dotnet_fmt::float(max[0]),
            crate::dotnet_fmt::float(max[1])
        ));
    }
    let mut raw = Vec::with_capacity(uv.len() * 4);
    for p in uv {
        for k in 0..2 {
            let q = (f64::from((p[k] - min[k]) / (max[k] - min[k])) * 65536. - 32767.)
                .round_ties_even()
                .clamp(-32767., 32767.) as i16;
            raw.extend(q.to_le_bytes());
        }
    }
    set.min = min;
    set.max = max;
    set.data.origin = payload(&raw);
    Ok(())
}

pub fn reload_uv(set: &mut UvSet) -> Result<(), String> {
    let mut w = Writer::new();
    set.write(&mut w);
    let bytes = w.into_vec();
    *set = UvSet::read(&mut Reader::new(&bytes))?;
    Ok(())
}

impl Lod {
    pub fn duplicate(&self, resolution: f32) -> Result<Self, String> {
        let mut w = Writer::new();
        self.write(&mut w)?;
        let bytes = w.into_vec();
        Self::read(&mut Reader::new(&bytes), resolution)
    }
}
