use crate::{
    axis_common::lod_index,
    odol::{BoneWeight, Lod, Model, lod_edit},
    op_json::{edit, floats, latin1, same, single, string},
};
use serde_json::Value;

pub fn empty_bone() -> BoneWeight {
    BoneWeight {
        count: 0,
        data: [0; 8],
    }
}

pub fn bound_to(sub: usize) -> BoneWeight {
    let mut data = [0; 8];
    data[0] = sub as u8;
    data[1] = 255;
    BoneWeight { count: 1, data }
}

pub fn point(v: &Value) -> Option<Vec<f32>> {
    v.as_array()?.iter().map(single).collect()
}

pub fn optional_bone(op: &Value) -> Option<&str> {
    op.get("bone")
        .and_then(Value::as_str)
        .filter(|b| !b.is_empty())
}

pub fn check_loose(target: &Lod, name: &str) -> Result<(), String> {
    if !target.faces.is_empty() {
        return Err("target LOD has faces; only loose points can be added".into());
    }
    if target.vertices.is_empty() {
        return Err("target LOD has no vertex to copy point flags from".into());
    }
    if target.selections.iter().any(|s| same(&s.name, name)) {
        return Err(format!("target LOD already has selection '{name}'"));
    }
    Ok(())
}

pub fn add_loose(target: &mut Lod, name: &str, points: Vec<[f32; 3]>) -> Result<(), String> {
    let clip = *target.clip.first().ok_or("target LOD has no clip flags")?;
    let normal: [u8; 4] = lod_edit::normals(target)?
        .get(..4)
        .and_then(|n| n.try_into().ok())
        .ok_or("target LOD has no normals")?;
    let bone = target
        .bones
        .first()
        .ok_or("target LOD has no bone weights")?
        .clone();
    let added = points
        .into_iter()
        .map(|p| lod_edit::append_vertex(target, p, clip, normal, bone.clone()))
        .collect::<Result<Vec<_>, _>>()?;
    let selection = lod_edit::new_selection(latin1(name)?, false, &[], &[], &added)?;
    target.selections.push(selection);
    Ok(())
}

pub fn add_selection_points(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let target = lod_index(model, string(op, "lod")?)?;
        let name = string(op, "name")?;
        let lod = &mut model.lods[target];
        check_loose(lod, name)?;
        let points = op
            .get("points")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("add-selection-points '{name}' needs a points array"))?
            .iter()
            .map(|p| {
                point(p)
                    .filter(|c| c.len() == 3)
                    .map(|c| [c[0], c[1], c[2]])
                    .ok_or_else(|| {
                        format!("add-selection-points '{name}' needs [x, y, z] for every point")
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if points.is_empty() {
            return Err(format!("add-selection-points '{name}' has no points"));
        }
        add_loose(lod, name, points)
    })
}

fn sub_bone(model: &mut Model, li: usize, bone: &str) -> Result<usize, String> {
    let skel = model
        .info
        .skeleton
        .bones
        .iter()
        .position(|b| same(&b.name, bone))
        .ok_or_else(|| format!("skeleton has no bone '{bone}'"))?;
    let lod = &mut model.lods[li];
    if let Some(existing) = lod.sub_to_skeleton.iter().position(|&b| b == skel as i32) {
        return Ok(existing);
    }
    let sub = lod.sub_to_skeleton.len();
    if sub > usize::from(u8::MAX) {
        return Err(format!(
            "bone '{bone}' would get sub-skeleton index {sub}, which a vertex weight cannot hold"
        ));
    }
    let set = lod
        .skeleton_to_sub
        .get_mut(skel)
        .ok_or("skeleton bone is outside the LOD's sub-skeleton map")?;
    *set = vec![sub as i32];
    lod.sub_to_skeleton.push(skel as i32);
    Ok(sub)
}

pub fn isolate_memory_selection(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let name = string(op, "name")?;
        let position = floats(op, "position", 3)?;
        let li = lod_index(model, "memory")?;
        let memory = &mut model.lods[li];
        let hits: Vec<usize> = (0..memory.selections.len())
            .filter(|&i| same(&memory.selections[i].name, name))
            .collect();
        let index = match hits.as_slice() {
            [] => return Err(format!("memory has no selection '{name}'")),
            [one] => *one,
            _ => return Err(format!("memory has more than one selection '{name}'")),
        };
        let s = &memory.selections[index];
        if s.vertices.is_empty() || !s.faces.is_empty() || s.is_sectional != 0 {
            return Err(format!("'{name}' is not a point selection"));
        }
        if memory.bones.len() != memory.vertices.len() {
            return Err("memory bone-ref count does not match the vertex count".into());
        }
        let donor = usize::try_from(s.vertices[0]).map_err(|_| "invalid vertex")?;
        let clip = *memory.clip.get(donor).ok_or("invalid donor clip")?;
        let normal: [u8; 4] = lod_edit::normals(memory)?
            .get(donor * 4..donor * 4 + 4)
            .and_then(|n| n.try_into().ok())
            .ok_or("invalid donor normal")?;
        let p = [position[0], position[1], position[2]];
        let added = lod_edit::append_vertex(memory, p, clip, normal, empty_bone())?;
        if let Some(bone) = optional_bone(op) {
            let sub = sub_bone(model, li, bone)?;
            let memory = &mut model.lods[li];
            let mut bones = std::mem::take(&mut memory.bones);
            *bones.last_mut().ok_or("memory has no bone weights")? = bound_to(sub);
            lod_edit::set_bones(memory, bones)?;
            lod_edit::refresh_rest(memory)?;
        }
        let s = &mut model.lods[li].selections[index];
        lod_edit::set_selection_vertices(s, vec![added])?;
        lod_edit::set_selection_faces(s, Vec::new())
    })
}
