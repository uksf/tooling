use crate::{
    axis_common::lod_index,
    lod_insert::{self, HITPOINTS, ROADWAY},
    lod_names::resolution,
    odol::{Lod, Selection, lod_edit},
    op_json::{edit, latin1_lossy, required_int, same, string, strings},
    point_ops::{empty_bone, point},
    section_faces::weight,
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

fn add_quads(
    lod: &mut Lod,
    op: &Value,
    faces: &mut Vec<Vec<i32>>,
    named: &mut Vec<Selection>,
) -> Result<(), String> {
    let Some(quads) = op.get("quads") else {
        return Ok(());
    };
    let quads = quads.as_array().ok_or("quads must be an array")?;
    let bones = if lod.bones.len() == lod.vertices.len() {
        std::mem::take(&mut lod.bones)
    } else {
        vec![empty_bone(); lod.vertices.len()]
    };
    lod_edit::set_bones(lod, bones)?;
    lod_edit::refresh_rest(lod)?;
    for quad in quads {
        let name = string(quad, "name")?;
        let points = quad
            .get("points")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("quad '{name}' needs four x, y, z points"))?
            .iter()
            .map(|p| point(p).ok_or_else(|| format!("quad '{name}' needs finite points")))
            .collect::<Result<Vec<_>, _>>()?;
        if points.len() != 4 || points.iter().any(|p| p.len() != 3) {
            return Err(format!("quad '{name}' needs four x, y, z points"));
        }
        let clip = *lod.clip.first().ok_or("LOD has no clip flags")?;
        let normal: [u8; 4] = lod_edit::normals(lod)?
            .get(..4)
            .and_then(|n| n.try_into().ok())
            .ok_or("LOD has no normals")?;
        let added = points
            .iter()
            .map(|p| lod_edit::append_vertex(lod, [p[0], p[1], p[2]], clip, normal, empty_bone()))
            .collect::<Result<Vec<_>, _>>()?;
        let n = i32::try_from(faces.len()).map_err(|_| "face count overflow")?;
        named.push(lod_edit::new_selection(
            latin1_lossy(name),
            false,
            &[],
            &[n, n + 1],
            &added,
        )?);
        faces.push(added.clone());
        faces.push(added.into_iter().rev().collect());
    }
    Ok(())
}

pub fn add_selection_lod(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, true, |model| {
        let lod_name = string(op, "lod")?;
        let slot = if lod_name.eq_ignore_ascii_case("roadway") {
            ROADWAY
        } else if lod_name.eq_ignore_ascii_case("hitpoints") {
            HITPOINTS
        } else {
            return Err(format!(
                "add-selection-lod supports roadway or hitpoints, not '{lod_name}'"
            ));
        };
        let from_name = string(op, "fromLod")?;
        let from = lod_index(model, from_name)?;
        let wanted = strings(op, "selections")?;
        let expected = required_int(op, "faces")?;
        let source = &model.lods[from];
        let selections = wanted
            .iter()
            .map(|name| {
                let hits: Vec<_> = source
                    .selections
                    .iter()
                    .filter(|s| same(&s.name, name))
                    .collect();
                match hits.as_slice() {
                    [one] => Ok(*one),
                    [] => Err(format!("{from_name} has no selection '{name}'")),
                    _ => Err(format!("{from_name} has more than one selection '{name}'")),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let verts: HashSet<i32> = selections
            .iter()
            .flat_map(|s| s.vertices.iter().copied())
            .collect();
        let kept: Vec<usize> = (0..source.faces.len())
            .filter(|&i| {
                let v = &source.faces[i];
                !v.is_empty() && v.iter().all(|x| verts.contains(x))
            })
            .collect();
        if kept.len() as i64 != i64::from(expected) {
            return Err(format!(
                "selections {} have {} faces, the patch expects {expected}",
                wanted.join(", "),
                kept.len()
            ));
        }
        if source.sections.is_empty() {
            return Err(format!("{from_name} has no sections"));
        }
        let remap: HashMap<usize, i32> = kept
            .iter()
            .enumerate()
            .map(|(i, &f)| (f, i as i32))
            .collect();
        let mut faces: Vec<Vec<i32>> = kept.iter().map(|&i| source.faces[i].clone()).collect();
        let mut named = selections
            .iter()
            .map(|s| {
                let own: HashSet<i32> = s.vertices.iter().copied().collect();
                let listed: Vec<i32> = kept
                    .iter()
                    .filter(|&&i| source.faces[i].iter().all(|v| own.contains(v)))
                    .map(|i| remap[i])
                    .collect();
                lod_edit::new_selection(s.name.clone(), false, &[], &listed, &s.vertices)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let at = lod_insert::insert_clone(model, from, resolution(lod_name)?, slot)?;
        let lod = &mut model.lods[at];
        add_quads(lod, op, &mut faces, &mut named)?;
        let bytes: i32 = faces.iter().map(|f| weight(f)).sum();
        lod.faces = faces;
        lod.face_bytes = bytes as u32;
        let mut section = lod.sections[0].clone();
        section.face_lower = 0;
        section.face_upper = bytes;
        lod.sections = vec![section];
        lod.proxies.clear();
        lod.selections = named;
        Ok(())
    })
}
