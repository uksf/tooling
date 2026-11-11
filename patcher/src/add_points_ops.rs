use crate::{
    lod_insert::{self, HITPOINTS},
    lod_names::resolution,
    odol::{Model, lod_edit},
    op_json::{edit, latin1_lossy, same, string},
    point_ops::{bound_to, empty_bone, optional_bone, point},
};
use serde_json::Value;

fn find_or_create(model: &mut Model, lod_name: &str) -> Result<usize, String> {
    let wanted = resolution(lod_name)?;
    let found: Vec<usize> = (0..model.lods.len())
        .filter(|&i| lod_insert::matches(model.lods[i].resolution, wanted))
        .collect();
    match found.as_slice() {
        [one] => return Ok(*one),
        [] => {}
        _ => return Err(format!("more than one {lod_name} LOD")),
    }
    if !lod_name.eq_ignore_ascii_case("hitpoints") {
        return Err(format!("no {lod_name} LOD"));
    }
    let memory = resolution("memory")?;
    let found: Vec<usize> = (0..model.lods.len())
        .filter(|&i| lod_insert::matches(model.lods[i].resolution, memory))
        .collect();
    let [source] = found.as_slice() else {
        return Err(format!("model has no memory LOD to clone as {lod_name}"));
    };
    let at = lod_insert::insert_clone(model, *source, wanted, HITPOINTS)?;
    model.lods[at].selections.clear();
    Ok(at)
}

fn positions(op: &Value) -> Result<Vec<[f32; 3]>, String> {
    let raw = op
        .get("positions")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .ok_or("add-points needs a non-empty positions array")?;
    raw.iter()
        .map(|item| {
            let p = point(item).ok_or("each position needs x, y, z as finite numbers")?;
            match p.as_slice() {
                [x, y, z] => Ok([*x, *y, *z]),
                _ => Err("each position needs x, y, z".to_owned()),
            }
        })
        .collect()
}

pub fn add_points(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let lod_name = string(op, "lod")?;
        let name = string(op, "name")?;
        let positions = positions(op)?;
        let expected = crate::op_json::required_int(op, "points")?;
        if positions.len() as i64 != i64::from(expected) {
            return Err(format!(
                "'{name}' has {} points, the patch expects {expected}",
                positions.len()
            ));
        }
        let li = find_or_create(model, lod_name)?;
        let lod = &model.lods[li];
        if !lod.faces.is_empty() {
            return Err(format!("add-points refuses {lod_name}, which has faces"));
        }
        if lod.selections.iter().any(|s| same(&s.name, name)) {
            return Err(format!("{lod_name} already has selection '{name}'"));
        }
        if lod.bones.len() != lod.vertices.len() && !lod.bones.is_empty() {
            return Err("add-points: bone-ref count does not match the vertex count".into());
        }
        let sub = optional_bone(op)
            .map(|bone| {
                let names = crate::section_faces::bone_names(model);
                for (i, &b) in lod.sub_to_skeleton.iter().enumerate() {
                    let n = usize::try_from(b)
                        .ok()
                        .and_then(|b| names.get(b))
                        .ok_or("sub-skeleton bone is outside the skeleton")?;
                    if same(n, bone) {
                        return Ok(i);
                    }
                }
                Err(format!("this LOD's skeleton has no bone '{bone}'"))
            })
            .transpose();
        let lod = &mut model.lods[li];
        if lod.bones.is_empty() {
            lod_edit::set_bones(lod, vec![empty_bone(); lod.vertices.len()])?;
        }
        let donor = (!lod.vertices.is_empty()).then_some(0);
        let clip = donor.map_or(Ok(63), |d| {
            lod.clip.get(d).copied().ok_or("invalid donor clip")
        })?;
        let normal: [u8; 4] = match donor {
            None => [0; 4],
            Some(_) => lod_edit::normals(lod)?
                .get(..4)
                .and_then(|n| n.try_into().ok())
                .ok_or("invalid donor normal")?,
        };
        let added = positions
            .into_iter()
            .map(|p| lod_edit::append_vertex(lod, p, clip, normal, empty_bone()))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(sub) = sub? {
            let mut bones = std::mem::take(&mut lod.bones);
            for &v in &added {
                bones[v as usize] = bound_to(sub);
            }
            lod_edit::set_bones(lod, bones)?;
            lod_edit::refresh_rest(lod)?;
        }
        let selection = lod_edit::new_selection(latin1_lossy(name), false, &[], &[], &added)?;
        lod.selections.push(selection);
        Ok(())
    })
}
