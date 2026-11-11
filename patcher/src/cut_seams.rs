use crate::{
    axis_common::lod_index,
    odol::{Lod, edit_arrays::set_vertices, lod_edit},
    op_json::{edit, string},
};
use serde_json::Value;
use std::{collections::HashMap, path::Path};

pub fn source_json(op: &Value, base: &Path) -> Result<Value, String> {
    let source = string(op, "source")?;
    let bytes = std::fs::read(base.join(source))
        .map_err(|_| format!("source '{source}' does not exist"))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("source '{source}': {e}"))
}

fn index(v: &Value) -> Result<i32, String> {
    v.as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| format!("expected an integer, found {v}"))
}

fn grow<T: Clone>(items: &[T], copies: &[usize]) -> Result<Vec<T>, String> {
    let mut out = items.to_vec();
    for &c in copies {
        out.push(
            out.get(c)
                .ok_or("cut-seams copies a missing entry")?
                .clone(),
        );
    }
    Ok(out)
}

fn grow_raw(raw: &[u8], width: usize, copies: &[usize]) -> Result<Vec<u8>, String> {
    let rows: Vec<&[u8]> = raw.chunks_exact(width).collect();
    Ok(grow(&rows, copies)?.concat())
}

fn join_selections(lod: &mut Lod, n0: usize, copies: &[usize]) -> Result<(), String> {
    let mut sources: HashMap<i32, Vec<i32>> = HashMap::new();
    for (i, &c) in copies.iter().enumerate() {
        sources.entry(c as i32).or_default().push((n0 + i) as i32);
    }
    for sel in &mut lod.selections {
        let mut verts = sel.vertices.clone();
        let mut weights = sel.weights.clone();
        let weighted = weights.len() == verts.len();
        let original = verts.len();
        for k in 0..original {
            for &c in sources.get(&verts[k]).map_or(&[][..], Vec::as_slice) {
                verts.push(c);
                if weighted {
                    weights.push(weights[k]);
                }
            }
        }
        if verts.len() == original {
            continue;
        }
        lod_edit::set_selection_vertices(sel, verts)?;
        if weighted {
            lod_edit::set_selection_weights(sel, weights)?;
        }
    }
    Ok(())
}

fn repoint_faces(lod: &mut Lod, plan: &Value, n0: usize, copies: &[usize]) -> Result<(), String> {
    let by_verts: HashMap<Vec<i32>, usize> = lod
        .faces
        .iter()
        .enumerate()
        .map(|(f, v)| (v.clone(), f))
        .collect();
    for e in plan
        .get("faces")
        .and_then(Value::as_array)
        .ok_or("cut-seams plan needs a \"faces\" array")?
    {
        let key = e
            .get("verts")
            .and_then(Value::as_array)
            .ok_or("cut-seams face needs \"verts\"")?
            .iter()
            .map(index)
            .collect::<Result<Vec<_>, _>>()?;
        let name = key.iter().map(i32::to_string).collect::<Vec<_>>().join(",");
        let f = *by_verts
            .get(&key)
            .ok_or_else(|| format!("cut-seams finds no face with vertices {name}"))?;
        let map = e
            .get("map")
            .and_then(Value::as_object)
            .ok_or("cut-seams face needs a \"map\"")?;
        for (from, to) in map {
            let from: i32 = from
                .parse()
                .map_err(|_| format!("cut-seams map key '{from}' is not a vertex"))?;
            let to = index(to)?;
            let k = lod.faces[f]
                .iter()
                .position(|&v| v == from)
                .ok_or_else(|| format!("cut-seams face {name} has no vertex {from}"))?;
            let copy = usize::try_from(to).ok().and_then(|t| t.checked_sub(n0));
            if copy
                .and_then(|c| copies.get(c))
                .is_none_or(|&c| c as i32 != from)
            {
                return Err(format!("cut-seams vertex {to} is not a copy of {from}"));
            }
            lod.faces[f][k] = to;
        }
    }
    Ok(())
}

pub fn cut_seams(data: &[u8], op: &Value, label: &str, base: &Path) -> Result<Vec<u8>, String> {
    edit(data, label, true, |model| {
        let li = lod_index(model, string(op, "lod")?)?;
        let plan = source_json(op, base)?;
        let lod = &mut model.lods[li];
        let n0 = lod.vertices.len();
        let copies = plan
            .get("copies")
            .and_then(Value::as_array)
            .ok_or("cut-seams plan needs a \"copies\" array")?
            .iter()
            .map(|v| index(v).map(|c| usize::try_from(c).unwrap_or(usize::MAX)))
            .collect::<Result<Vec<_>, _>>()?;
        if copies.iter().any(|&c| c >= n0) {
            return Err(format!("cut-seams copies a vertex outside the LOD's {n0}"));
        }
        if !lod.frames.is_empty() {
            return Err("cut-seams does not support LODs with keyframes".into());
        }
        set_vertices(lod, grow(&lod.vertices, &copies)?)?;
        let normals = grow_raw(&lod_edit::normals(lod)?, 4, &copies)?;
        lod_edit::set_normals(lod, &normals)?;
        let st = grow_raw(&crate::odol::mesh_arrays::st(lod)?, 8, &copies)?;
        lod_edit::set_st(lod, &st)?;
        if lod.clip.len() == n0 {
            lod_edit::set_clip(lod, grow(&lod.clip, &copies)?)?;
        }
        if lod.vertex_to_point.len() == n0 {
            lod_edit::set_vertex_to_point(lod, grow(&lod.vertex_to_point, &copies)?)?;
        }
        if lod.bones.len() == n0 {
            lod_edit::set_bones(lod, grow(&lod.bones, &copies)?)?;
        }
        let neighbours = lod_edit::neighbours(lod)?;
        if !neighbours.is_empty() {
            if neighbours.len() != n0 * 32 {
                return Err(format!(
                    "NeighborBoneRef has {} entries for {n0} vertices",
                    neighbours.len() / 32
                ));
            }
            lod_edit::set_neighbours(lod, &grow_raw(&neighbours, 32, &copies)?)?;
        }
        let count = u32::try_from(n0 + copies.len()).map_err(|_| "vertex count overflow")?;
        for set in &mut lod.uv {
            let uv = grow(&set.coords, &copies)?;
            lod_edit::write_uv(set, &uv)?;
            set.vertices = count;
        }
        lod.vertex_count = count;
        join_selections(lod, n0, &copies)?;
        repoint_faces(lod, &plan, n0, &copies)?;
        lod_edit::refresh_rest(lod)
    })
}
