use crate::{
    axis_common::lod_index,
    odol::{Lod, lod_edit},
    op_json::{edit, floats, same, string},
    section_faces::bone_names,
};
use serde_json::Value;
use std::collections::HashSet;

fn bone_vertices(lod: &Lod, sub: usize) -> HashSet<i32> {
    lod.bones
        .iter()
        .enumerate()
        .filter(|(_, w)| (0..w.count.clamp(0, 4) as usize).any(|k| w.data[k * 2] as usize == sub))
        .map(|(v, _)| v as i32)
        .collect()
}

pub fn offset_uv(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let li = lod_index(model, string(op, "lod")?)?;
        let bone = string(op, "bone")?;
        let offset = floats(op, "offset", 2)?;
        let skeleton = bone_names(model)
            .iter()
            .position(|n| same(n, bone))
            .ok_or_else(|| format!("model has no bone '{bone}'"))?;
        let lod = &mut model.lods[li];
        let sub = lod
            .sub_to_skeleton
            .iter()
            .position(|&b| b as usize == skeleton)
            .ok_or_else(|| format!("LOD does not use bone '{bone}'"))?;
        let verts = bone_vertices(lod, sub);
        if verts.is_empty() {
            return Err(format!("no vertex is weighted to '{bone}'"));
        }
        let mixed = lod
            .faces
            .iter()
            .filter(|f| f.iter().any(|v| verts.contains(v)) && !f.iter().all(|v| verts.contains(v)))
            .count();
        if mixed > 0 {
            return Err(format!(
                "{mixed} faces join vertices of '{bone}' to other vertices"
            ));
        }
        let set = lod.uv.first_mut().ok_or("LOD has no UV set")?;
        let mut uv = set.coords.clone();
        for &v in &verts {
            let p = uv
                .get_mut(v as usize)
                .ok_or_else(|| format!("vertex {v} has no UV"))?;
            p[0] += offset[0];
            p[1] += offset[1];
        }
        lod_edit::write_uv(set, &uv)?;
        lod_edit::reload_uv(set)
    })
}
