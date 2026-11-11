use crate::{
    odol::{Lod, Model, Section},
    op_json::same,
};
use std::collections::HashSet;

pub const MAX_BONES_PER_SECTION: i32 = 56;

pub fn weight(face: &[i32]) -> i32 {
    4 + 4 * face.len() as i32
}

pub fn in_section(lod: &Lod, s: &Section) -> Vec<usize> {
    let mut faces = Vec::new();
    let mut offset = 0;
    for (f, face) in lod.faces.iter().enumerate() {
        if offset >= s.face_lower && offset < s.face_upper {
            faces.push(f);
        }
        offset += weight(face);
    }
    faces
}

pub fn selection(lod: &Lod, name: &str) -> Result<usize, String> {
    lod.selections
        .iter()
        .position(|s| same(&s.name, name))
        .ok_or_else(|| format!("LOD has no selection '{name}'"))
}

pub fn sections(lod: &Lod, name: &str) -> Result<Vec<usize>, String> {
    if let Some(index) = name.strip_prefix('#') {
        let index: usize = index
            .parse()
            .map_err(|_| format!("'{name}' is not a section index"))?;
        if index >= lod.sections.len() {
            return Err(format!("LOD has no section {index}"));
        }
        return Ok(vec![index]);
    }
    let s = &lod.selections[selection(lod, name)?];
    if s.is_sectional == 0 || s.sections.is_empty() {
        return Err(format!("'{name}' is not a sectional selection"));
    }
    Ok(s.sections.iter().map(|&x| x as usize).collect())
}

pub fn shared(lod: &Lod, own: &HashSet<usize>, verts: &HashSet<i32>) -> usize {
    let mut shared: HashSet<i32> = HashSet::new();
    for (f, face) in lod.faces.iter().enumerate() {
        if !own.contains(&f) {
            shared.extend(face.iter().filter(|v| verts.contains(v)));
        }
    }
    shared.len()
}

pub fn inside(p: [f32; 3], b: &[f32]) -> bool {
    p[0] >= b[0] && p[0] <= b[1] && p[1] >= b[2] && p[1] <= b[3] && p[2] >= b[4] && p[2] <= b[5]
}

pub fn bone_names(model: &Model) -> Vec<&[u8]> {
    model
        .info
        .skeleton
        .bones
        .iter()
        .map(|b| b.name.as_slice())
        .collect()
}

pub fn fit_bone_window(lod: &mut Lod, section: usize) -> Result<(), String> {
    let mut used = Vec::new();
    for f in in_section(lod, &lod.sections[section]) {
        for &v in &lod.faces[f] {
            let b = lod
                .bones
                .get(v as usize)
                .ok_or("vertex has no bone weight")?;
            let n = usize::try_from(b.count).unwrap_or(0).min(4);
            used.extend((0..n).map(|k| i32::from(b.data[2 * k])));
        }
    }
    let (Some(&lo), Some(&hi)) = (used.iter().min(), used.iter().max()) else {
        return Ok(());
    };
    let sec = &mut lod.sections[section];
    if lo >= sec.min_bone && hi < sec.min_bone + sec.bones {
        return Ok(());
    }
    let count = sec.bones.max(hi - lo + 1);
    if count > MAX_BONES_PER_SECTION {
        return Err(format!(
            "a section would need bones {lo}..{hi}, more than {MAX_BONES_PER_SECTION}"
        ));
    }
    sec.min_bone = lo.min((hi - count + 1).max(0));
    sec.bones = count;
    Ok(())
}
