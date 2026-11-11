use crate::{
    axis_common::lod_index,
    odol::{Lod, lod_edit},
    op_json::{edit, float, int, latin1, same, string, strings},
    section_faces::selection,
};
use serde_json::Value;
use std::collections::HashSet;

fn single(lod: &Lod, name: &str) -> Result<usize, String> {
    let hits: Vec<_> = (0..lod.selections.len())
        .filter(|&i| same(&lod.selections[i].name, name))
        .collect();
    match hits.as_slice() {
        [one] => Ok(*one),
        _ => Err(format!("LOD has no single selection '{name}'")),
    }
}

fn require_new(lod: &Lod, name: &str) -> Result<(), String> {
    if lod.selections.iter().any(|s| same(&s.name, name)) {
        return Err(format!("LOD already has selection '{name}'"));
    }
    Ok(())
}

pub fn centre_and_shrink(positions: &[[f32; 3]], shrink: f32) -> Vec<[f32; 3]> {
    let mut sum = [0f32; 3];
    for p in positions {
        for k in 0..3 {
            sum[k] += p[k];
        }
    }
    let n = positions.len() as f32;
    let centre = sum.map(|s| s / n);
    std::iter::once(centre)
        .chain(
            positions
                .iter()
                .map(|p| std::array::from_fn(|k| centre[k] + (p[k] - centre[k]) * shrink)),
        )
        .collect()
}

pub fn copy_selection_points(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let target = lod_index(model, string(op, "lod")?)?;
        let name = string(op, "name")?;
        let from_lod = lod_index(model, string(op, "fromLod")?)?;
        let from_name = string(op, "from")?;
        let shrink = float(op, "shrink")?;
        let expected = int(op, "points", -1)?;
        if !(0.0..=1.0).contains(&shrink) {
            return Err("shrink must be between 0 and 1".into());
        }
        let lod = &model.lods[target];
        if !lod.faces.is_empty() {
            return Err(
                "target LOD has faces; copy-selection-points only adds loose points".into(),
            );
        }
        if lod.vertices.is_empty() {
            return Err("target LOD has no vertex to copy point flags from".into());
        }
        require_new(lod, name)?;
        let source = &model.lods[from_lod];
        let mut seen = HashSet::new();
        let positions = source.selections[single(source, from_name)?]
            .vertices
            .iter()
            .filter(|v| seen.insert(**v))
            .map(|&v| source.vertices.get(v as usize).copied())
            .collect::<Option<Vec<_>>>()
            .ok_or("source selection has a vertex outside its LOD")?;
        if positions.is_empty() {
            return Err(format!("source selection '{from_name}' has no vertices"));
        }
        let points = centre_and_shrink(&positions, shrink);
        if expected >= 0 && points.len() as i64 != i64::from(expected) {
            return Err(format!(
                "copy-selection-points '{name}' made {} points, manifest expects {expected}",
                points.len()
            ));
        }
        let lod = &mut model.lods[target];
        let clip = *lod.clip.first().ok_or("target LOD has no clip flags")?;
        let normal: [u8; 4] = lod_edit::normals(lod)?
            .get(..4)
            .and_then(|n| n.try_into().ok())
            .ok_or("target LOD has no normals")?;
        let bone = lod
            .bones
            .first()
            .ok_or("target LOD has no bone weights")?
            .clone();
        let added = points
            .into_iter()
            .map(|p| lod_edit::append_vertex(lod, p, clip, normal, bone.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        let selection = lod_edit::new_selection(latin1(name)?, false, &[], &[], &added)?;
        lod.selections.push(selection);
        Ok(())
    })
}

pub fn split_selection(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let li = lod_index(model, string(op, "lod")?)?;
        let name = string(op, "selection")?;
        let into = string(op, "into")?;
        let axis = int(op, "axis", -1)?;
        let keep_below = float(op, "keepBelow")?;
        let axis = usize::try_from(axis)
            .ok()
            .filter(|&a| a < 3)
            .ok_or("split-selection axis must be 0, 1 or 2")?;
        let lod = &mut model.lods[li];
        require_new(lod, into)?;
        let index = single(lod, name)?;
        let s = &lod.selections[index];
        for (what, empty) in [
            ("faces", s.faces.is_empty()),
            ("sections", s.sections.is_empty()),
            ("vertex weights", s.weights.is_empty()),
        ] {
            if !empty {
                return Err(format!(
                    "selection '{name}' has {what}; split-selection only splits loose unweighted points"
                ));
            }
        }
        let mut below = Vec::new();
        let mut above = Vec::new();
        for &v in &s.vertices {
            let p = lod
                .vertices
                .get(v as usize)
                .ok_or("selection vertex is outside the LOD")?;
            if p[axis] < keep_below {
                below.push(v);
            } else if p[axis] >= keep_below {
                above.push(v);
            }
        }
        if below.is_empty() || above.is_empty() {
            return Err(format!(
                "split of '{name}' at {keep_below} on axis {axis} leaves one side empty"
            ));
        }
        lod_edit::set_selection_vertices(&mut lod.selections[index], below)?;
        let selection = lod_edit::new_selection(latin1(into)?, false, &[], &[], &above)?;
        lod.selections.push(selection);
        Ok(())
    })
}

pub fn move_sections(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let li = lod_index(model, string(op, "lod")?)?;
        let texture = string(op, "texture")?;
        let new_selection = string(op, "newSelection")?;
        let from = strings(op, "from")?;
        let lod = &mut model.lods[li];
        let mut secs = Vec::new();
        for (s, section) in lod.sections.iter().enumerate() {
            let t = section.texture;
            if t >= 0 {
                let name = lod.textures.get(t as usize).ok_or_else(|| {
                    format!("section {s} references texture {t}, outside the LOD")
                })?;
                if same(name, texture) {
                    secs.push(s as i32);
                }
            }
        }
        if secs.is_empty() {
            return Err(format!("LOD has no sections using '{texture}'"));
        }
        require_new(lod, new_selection)?;
        for name in from {
            let i = selection(lod, name)?;
            let n = &mut lod.selections[i];
            let kept = n
                .sections
                .iter()
                .copied()
                .filter(|s| !secs.contains(s))
                .collect();
            lod_edit::set_selection_sections(n, kept)?;
        }
        let selection = lod_edit::new_selection(latin1(new_selection)?, true, &secs, &[], &[])?;
        lod.selections.push(selection);
        Ok(())
    })
}

pub fn retexture(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let from = string(op, "from")?;
        let to = latin1(string(op, "to")?)?;
        let mut changed = 0;
        for t in model.lods.iter_mut().flat_map(|l| &mut l.textures) {
            if same(t, from) {
                changed += usize::from(*t != to);
                t.clone_from(&to);
            }
        }
        if changed == 0 {
            return Err(format!("no LOD uses texture '{from}'"));
        }
        Ok(())
    })
}

pub fn set_section_texture(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let li = lod_index(model, string(op, "lod")?)?;
        let section = int(op, "section", -1)?;
        let texture = string(op, "texture")?;
        let lod = &mut model.lods[li];
        let section = usize::try_from(section)
            .ok()
            .filter(|&s| s < lod.sections.len())
            .ok_or_else(|| format!("LOD has no section {section}"))?;
        let index = match lod.textures.iter().position(|t| same(t, texture)) {
            Some(i) => i,
            None => {
                lod.textures.push(latin1(texture)?);
                lod.textures.len() - 1
            }
        };
        lod.sections[section].texture =
            i16::try_from(index).map_err(|_| "texture index overflow")?;
        Ok(())
    })
}
