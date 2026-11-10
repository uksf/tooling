use super::{
    Lod,
    edit_arrays::{self, array, condensed, payload},
    io::Reader,
};

pub struct Appended {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<u32>,
    pub tangents: Vec<[u32; 2]>,
    pub uvs: Vec<Vec<[f32; 2]>>,
    pub donor: usize,
}
pub fn append(lod: &mut Lod, extra: Appended) -> Result<(), String> {
    let old_count = lod.vertices.len();
    let count = extra.positions.len();
    let old_size = lod.normals_origin.origin.len()
        + lod.st_origin.origin.len()
        + lod.bones_origin.origin.len()
        + lod.clip_origin.origin.len()
        + lod.uv.iter().map(|s| s.data.origin.len()).sum::<usize>();
    let mut vertices = lod.vertices.clone();
    vertices.extend(extra.positions);
    edit_arrays::set_vertices(lod, vertices)?;
    let (_, mut normals) = Reader::new(&lod.normals_origin.origin).condensed(4)?;
    for n in extra.normals {
        normals.extend(n.to_le_bytes());
    }
    lod.normals_origin = condensed(&normals, 4)?;
    let (_, mut st) = Reader::new(&lod.st_origin.origin).packed_array(8)?;
    for t in extra.tangents {
        for x in t {
            st.extend(x.to_le_bytes());
        }
    }
    lod.st_origin = array(&st, st.len() / 8)?;
    let (_, mut bones) = Reader::new(&lod.bones_origin.origin).packed_array(12)?;
    let bone = bones
        .get(extra.donor * 12..extra.donor * 12 + 12)
        .ok_or("invalid donor bone")?
        .to_vec();
    for _ in 0..count {
        bones.extend_from_slice(&bone);
    }
    lod.bones_origin = array(&bones, old_count + count)?;
    let (_, mut clips) = Reader::new(&lod.clip_origin.origin).condensed(4)?;
    let clip = clips
        .get(extra.donor * 4..extra.donor * 4 + 4)
        .ok_or("invalid donor clip")?
        .to_vec();
    for _ in 0..count {
        clips.extend_from_slice(&clip);
    }
    lod.clip_origin = condensed(&clips, 4)?;
    for (set, uv) in lod.uv.iter_mut().zip(extra.uvs) {
        if set.fill == 0 {
            let mut min = [f32::INFINITY; 2];
            let mut max = [f32::NEG_INFINITY; 2];
            for p in &uv {
                for k in 0..2 {
                    min[k] = min[k].min(p[k]);
                    max[k] = max[k].max(p[k]);
                }
            }
            for k in 0..2 {
                max[k] = min[k] + (max[k] - min[k]) * 65536. / 65534.;
            }
            let mut raw = Vec::new();
            for p in &uv {
                for k in 0..2 {
                    let q = (((p[k] - min[k]) / (max[k] - min[k])) as f64 * 65536. - 32767.)
                        .round_ties_even()
                        .clamp(-32767., 32767.) as i16;
                    raw.extend(q.to_le_bytes());
                }
            }
            set.min = min;
            set.max = max;
            set.data.origin = payload(&raw);
        }
        set.vertices = (old_count + count) as u32;
    }
    let new_size = lod.normals_origin.origin.len()
        + lod.st_origin.origin.len()
        + lod.bones_origin.origin.len()
        + lod.clip_origin.origin.len()
        + lod.uv.iter().map(|s| s.data.origin.len()).sum::<usize>();
    lod.rest_size = u32::try_from(i64::from(lod.rest_size) + new_size as i64 - old_size as i64)
        .map_err(|_| "rest size overflow")?;
    lod.vertex_count = (old_count + count) as u32;
    Ok(())
}
pub fn st(lod: &Lod) -> Result<Vec<u8>, String> {
    Ok(Reader::new(&lod.st_origin.origin).packed_array(8)?.1)
}
pub fn selection_vertices(
    s: &mut super::Selection,
    new: &[i32],
    donor: usize,
) -> Result<(), String> {
    let weighted = s.weights.len() == s.vertices.len();
    let full = if weighted { s.weights[donor] } else { 0 };
    s.vertices.extend_from_slice(new);
    let raw: Vec<_> = s.vertices.iter().flat_map(|v| v.to_le_bytes()).collect();
    s.vertices_origin = array(&raw, s.vertices.len())?;
    if weighted {
        s.weights.extend(std::iter::repeat_n(full, new.len()));
        s.weight_size = s.weights.len() as i32;
        s.weights_origin.origin = payload(&s.weights);
    }
    Ok(())
}
