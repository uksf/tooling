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

pub fn shadow_arrays(source: &Lod, volume: &mut Lod, origin: &[usize]) -> Result<(), String> {
    let (_, normals) = Reader::new(&source.normals_origin.origin).condensed(4)?;
    let (_, bones) = Reader::new(&source.bones_origin.origin).packed_array(12)?;
    let (_, source_st) = Reader::new(&source.st_origin.origin).packed_array(8)?;
    let (_, volume_st) = Reader::new(&volume.st_origin.origin).packed_array(8)?;
    if !source_st.is_empty() || !volume_st.is_empty() {
        return Err("a LOD has tangents".into());
    }
    let vertices: Vec<_> = origin.iter().map(|&i| source.vertices[i]).collect();
    set_vertices(volume, vertices)?;
    let old = volume.normals_origin.origin.len()
        + volume.clip_origin.origin.len()
        + volume.bones_origin.origin.len()
        + volume.neighbors_origin.origin.len();
    let normals_new: Vec<_> = origin
        .iter()
        .flat_map(|&i| normals[i * 4..i * 4 + 4].iter().copied())
        .collect();
    let clips: Vec<_> = origin
        .iter()
        .flat_map(|&i| source.clip[i].to_le_bytes())
        .collect();
    volume.normals_origin = condensed(&normals_new, 4)?;
    volume.clip_origin = condensed(&clips, 4)?;
    let mut neighbours = Vec::new();
    if !bones.is_empty() {
        let new_bones: Vec<_> = origin
            .iter()
            .flat_map(|&i| bones[i * 12..i * 12 + 12].iter().copied())
            .collect();
        volume.bones_origin = array(&new_bones, origin.len())?;
        for f in 0..origin.len() / 3 {
            for k in 0..3 {
                for n in [(k + 1) % 3, (k + 2) % 3] {
                    let corner = f * 3 + n;
                    neighbours.extend((corner as u16).to_le_bytes());
                    neighbours.extend([0, 0]);
                    neighbours
                        .extend_from_slice(&bones[origin[corner] * 12..origin[corner] * 12 + 12]);
                }
            }
        }
    }
    volume.neighbors_origin = array(&neighbours, neighbours.len() / 32)?;
    let new = volume.normals_origin.origin.len()
        + volume.clip_origin.origin.len()
        + volume.bones_origin.origin.len()
        + volume.neighbors_origin.origin.len();
    volume.rest_size = u32::try_from(i64::from(volume.rest_size) + new as i64 - old as i64)
        .map_err(|_| "rest size overflow")?;
    volume.vertex_count = u32::try_from(origin.len()).map_err(|_| "too many vertices")?;
    volume.uv[0].vertices = volume.vertex_count;
    Ok(())
}

pub fn set_vertices(lod: &mut Lod, vertices: Vec<[f32; 3]>) -> Result<(), String> {
    let raw: Vec<u8> = vertices
        .iter()
        .flatten()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let old = lod.vertices_origin.origin.len();
    lod.vertices_origin = array(&raw, vertices.len())?;
    lod.rest_size = u32::try_from(
        i64::from(lod.rest_size) + lod.vertices_origin.origin.len() as i64 - old as i64,
    )
    .map_err(|_| "rest size overflow")?;
    lod.vertices = vertices;
    Ok(())
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
