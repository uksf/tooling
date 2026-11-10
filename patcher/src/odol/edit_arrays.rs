use super::{
    Lod, compress,
    io::{Blob, Reader},
};

fn payload(raw: &[u8]) -> Vec<u8> {
    if raw.is_empty() {
        Vec::new()
    } else if raw.len() < 1024 {
        let mut v = vec![0];
        v.extend_from_slice(raw);
        v
    } else {
        let mut v = vec![2];
        v.extend(compress::compress(raw));
        v
    }
}
fn array(raw: &[u8], count: usize) -> Result<Blob, String> {
    let mut v = i32::try_from(count)
        .map_err(|_| "array too large")?
        .to_le_bytes()
        .to_vec();
    v.extend(payload(raw));
    Ok(Blob { origin: v })
}
fn condensed(raw: &[u8], width: usize) -> Result<Blob, String> {
    let count = raw.len() / width;
    let mut v = i32::try_from(count)
        .map_err(|_| "array too large")?
        .to_le_bytes()
        .to_vec();
    if count > 0 && raw.chunks_exact(width).all(|x| x == &raw[..width]) {
        v.push(1);
        v.extend_from_slice(&raw[..width]);
    } else {
        v.push(0);
        v.extend(payload(raw));
    }
    Ok(Blob { origin: v })
}

pub fn set_faces(selection: &mut super::Selection, mut faces: Vec<i32>) -> Result<(), String> {
    faces.sort_unstable();
    let raw: Vec<u8> = faces.iter().flat_map(|f| f.to_le_bytes()).collect();
    selection.faces_origin = array(&raw, faces.len())?;
    selection.faces = faces;
    Ok(())
}

pub fn append_memory(
    lod: &mut Lod,
    donor: usize,
    position: [f32; 3],
    selection: usize,
) -> Result<(), String> {
    if lod.uv.iter().any(|u| u.fill == 0) {
        return Err("memory LOD has per-vertex UV data, which memory edits do not support".into());
    }
    let old_size = lod.vertices_origin.origin.len()
        + lod.normals_origin.origin.len()
        + lod.clip_origin.origin.len()
        + lod.bones_origin.origin.len();
    let old = lod.vertices.len();
    let mut vertices = Vec::with_capacity((old + 1) * 12);
    for p in lod.vertices.iter().chain(std::iter::once(&position)) {
        for x in p {
            vertices.extend(x.to_le_bytes());
        }
    }
    let (_, mut normals) = Reader::new(&lod.normals_origin.origin).condensed(4)?;
    let normal = normals
        .get(donor * 4..donor * 4 + 4)
        .ok_or("invalid donor normal")?
        .to_vec();
    normals.extend(normal);
    let (_, mut bones) = Reader::new(&lod.bones_origin.origin).packed_array(12)?;
    let bone = bones
        .get(donor * 12..donor * 12 + 12)
        .ok_or("invalid donor bone")?
        .to_vec();
    bones.extend(bone);
    let donor_clip = *lod.clip.get(donor).ok_or("invalid donor clip")?;
    lod.clip.push(donor_clip);
    let clips: Vec<u8> = lod.clip.iter().flat_map(|x| x.to_le_bytes()).collect();
    lod.vertices_origin = array(&vertices, old + 1)?;
    lod.normals_origin = condensed(&normals, 4)?;
    lod.clip_origin = condensed(&clips, 4)?;
    lod.bones_origin = array(&bones, old + 1)?;
    let new_size = lod.vertices_origin.origin.len()
        + lod.normals_origin.origin.len()
        + lod.clip_origin.origin.len()
        + lod.bones_origin.origin.len();
    lod.rest_size = u32::try_from(i64::from(lod.rest_size) + new_size as i64 - old_size as i64)
        .map_err(|_| "rest size overflow")?;
    lod.vertex_count = u32::try_from(old + 1).map_err(|_| "vertex count overflow")?;
    for uv in &mut lod.uv {
        uv.vertices = lod.vertex_count;
    }
    let vertex = i32::try_from(old).map_err(|_| "vertex count overflow")?;
    let sel = &mut lod.selections[selection];
    sel.vertices = vec![vertex];
    sel.vertices_origin = array(&vertex.to_le_bytes(), 1)?;
    Ok(())
}
