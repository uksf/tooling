use crate::{
    axis_common::lod_index,
    odol::{Lod, edit_arrays::set_vertices, lod_edit},
    op_json::{edit, floats, int, same, string},
    section_faces::{bone_names, fit_bone_window, in_section, inside, sections, shared},
};
use serde_json::Value;
use std::collections::HashSet;

fn ordered<T: Copy + Eq + std::hash::Hash>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut seen = HashSet::new();
    items.into_iter().filter(|x| seen.insert(*x)).collect()
}

fn position(lod: &Lod, v: i32) -> Result<[f32; 3], String> {
    lod.vertices
        .get(v as usize)
        .copied()
        .ok_or_else(|| format!("face vertex {v} is outside the LOD"))
}

fn finish(lod: &mut Lod, refs: Vec<crate::odol::BoneWeight>, secs: &[usize]) -> Result<(), String> {
    lod_edit::set_bones(lod, refs)?;
    for &s in secs {
        fit_bone_window(lod, s)?;
    }
    lod_edit::refresh_rest(lod)
}

pub fn bind_selection(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let li = lod_index(model, string(op, "lod")?)?;
        let selection = string(op, "selection")?;
        let bone = string(op, "bone")?;
        let bounds = op.get("box").map(|_| floats(op, "box", 6)).transpose()?;
        let names = bone_names(model);
        let lod = &model.lods[li];
        let sub = lod
            .sub_to_skeleton
            .iter()
            .position(|&b| names.get(b as usize).is_some_and(|n| same(n, bone)))
            .ok_or_else(|| format!("LOD has no bone '{bone}'"))?;
        let sub = u8::try_from(sub).map_err(|_| {
            format!("bone '{bone}' has sub-skeleton index {sub}, which a vertex weight cannot hold")
        })?;
        let secs = sections(lod, selection)?;
        let mut faces = Vec::new();
        for &s in &secs {
            for f in in_section(lod, &lod.sections[s]) {
                let mut all = true;
                for &v in &lod.faces[f] {
                    if let Some(b) = &bounds {
                        all &= inside(position(lod, v)?, b);
                    }
                }
                if all {
                    faces.push(f);
                }
            }
        }
        let own: HashSet<usize> = faces.iter().copied().collect();
        if own.is_empty() {
            return Err(format!("no faces of '{selection}' match"));
        }
        let verts = ordered(
            ordered(faces)
                .into_iter()
                .flat_map(|f| lod.faces[f].clone()),
        );
        let set: HashSet<i32> = verts.iter().copied().collect();
        let count = shared(lod, &own, &set);
        if count > 0 {
            return Err(format!(
                "{count} vertices of '{selection}' are shared with other faces"
            ));
        }
        let mut refs = lod.bones.clone();
        if verts
            .iter()
            .any(|&v| refs.get(v as usize).is_none_or(|r| r.count != 0))
        {
            return Err(format!("'{selection}' already has bone weights"));
        }
        let template = refs
            .iter()
            .find(|r| r.count == 1)
            .ok_or("the LOD has no single-bone weight to copy")?
            .clone();
        for &v in &verts {
            let mut r = template.clone();
            r.data[0] = sub;
            refs[v as usize] = r;
        }
        finish(&mut model.lods[li], refs, &secs)
    })
}

fn sub_bone(lod: &mut Lod, names: &[&[u8]], bone: &str, add: bool) -> Result<u8, String> {
    let skel = names
        .iter()
        .position(|n| same(n, bone))
        .ok_or_else(|| format!("skeleton has no bone '{bone}'"))?;
    let set = lod
        .skeleton_to_sub
        .get(skel)
        .ok_or("skeleton bone is outside the LOD's table")?;
    if let Some(&first) = set.first() {
        return u8::try_from(first).map_err(|_| format!("bone '{bone}' does not fit a weight"));
    }
    if !add {
        return Err(format!("LOD has no bone '{bone}'"));
    }
    let sub = u8::try_from(lod.sub_to_skeleton.len()).map_err(|_| {
        format!(
            "bone '{bone}' would get sub-skeleton index {}, which a vertex weight cannot hold",
            lod.sub_to_skeleton.len()
        )
    })?;
    lod.sub_to_skeleton.push(skel as i32);
    lod.skeleton_to_sub[skel] = vec![i32::from(sub)];
    Ok(sub)
}

pub fn rebind_vertices(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let li = lod_index(model, string(op, "lod")?)?;
        let section = int(op, "section", -1)?;
        let bounds = floats(op, "box", 6)?;
        let expected = int(op, "vertices", -1)?;
        let section = usize::try_from(section)
            .ok()
            .filter(|&s| s < model.lods[li].sections.len())
            .ok_or_else(|| format!("LOD has no section {section}"))?;
        let names: Vec<Vec<u8>> = bone_names(model).into_iter().map(<[u8]>::to_vec).collect();
        let names: Vec<&[u8]> = names.iter().map(Vec::as_slice).collect();
        let lod = &mut model.lods[li];
        let from = sub_bone(lod, &names, string(op, "from")?, false)?;
        let to = sub_bone(lod, &names, string(op, "to")?, true)?;
        let own_faces = in_section(lod, &lod.sections[section]);
        let own: HashSet<usize> = own_faces.iter().copied().collect();
        let verts: Vec<i32> = ordered(own_faces.iter().flat_map(|&f| lod.faces[f].clone()))
            .into_iter()
            .map(|v| Ok((v, position(lod, v)?)))
            .collect::<Result<Vec<_>, String>>()?
            .into_iter()
            .filter(|(_, p)| inside(*p, &bounds))
            .map(|(v, _)| v)
            .collect();
        if verts.len() as i64 != i64::from(expected) {
            return Err(format!(
                "the box holds {} vertices of section {section}, the patch expects {expected}",
                verts.len()
            ));
        }
        let set: HashSet<i32> = verts.iter().copied().collect();
        let count = shared(lod, &own, &set);
        if count > 0 {
            return Err(format!(
                "{count} of the vertices are shared with faces outside section {section}"
            ));
        }
        let mut refs = lod.bones.clone();
        for &v in &verts {
            let r = refs
                .get_mut(v as usize)
                .filter(|r| r.count == 1 && r.data[0] == from)
                .ok_or_else(|| format!("vertex {v} is not weighted to the 'from' bone alone"))?;
            r.data[0] = to;
        }
        finish(lod, refs, &[section])
    })
}

pub fn translate_selection(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let li = lod_index(model, string(op, "lod")?)?;
        let selection = string(op, "selection")?;
        let offset = floats(op, "offset", 3)?;
        let lod = &mut model.lods[li];
        let secs = sections(lod, selection)?;
        let own: HashSet<usize> = secs
            .iter()
            .flat_map(|&s| in_section(lod, &lod.sections[s]))
            .collect();
        if own.is_empty() {
            return Err(format!("'{selection}' has no faces"));
        }
        let verts: HashSet<i32> = own.iter().flat_map(|&f| lod.faces[f].clone()).collect();
        let count = shared(lod, &own, &verts);
        if count > 0 {
            return Err(format!(
                "{count} vertices of '{selection}' are shared with other faces"
            ));
        }
        let moved = lod
            .vertices
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if verts.contains(&(i as i32)) {
                    [p[0] + offset[0], p[1] + offset[1], p[2] + offset[2]]
                } else {
                    *p
                }
            })
            .collect();
        set_vertices(lod, moved)?;
        lod_edit::refresh_rest(lod)
    })
}
