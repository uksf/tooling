use crate::{
    axis_common::{self, Frame},
    odol::{self, Lod, Model},
};
use serde_json::Value;
use std::collections::HashMap;

fn root(parent: &mut [usize], mut n: usize) -> usize {
    while parent[n] != n {
        parent[n] = parent[parent[n]];
        n = parent[n];
    }
    n
}
fn parts(lod: &Lod, bound: &[bool]) -> Vec<Vec<usize>> {
    let mut parent: Vec<_> = (0..lod.vertices.len()).collect();
    let mut positions = HashMap::new();
    for (i, p) in lod.vertices.iter().enumerate() {
        if !bound[i] {
            continue;
        }
        let key = p.map(|x| if x == 0.0 { 0 } else { x.to_bits() });
        if let Some(&old) = positions.get(&key) {
            let a = root(&mut parent, i);
            let b = root(&mut parent, old);
            parent[a] = b;
        } else {
            positions.insert(key, i);
        }
    }
    for f in &lod.faces {
        let vertices = f.iter().map(|&v| v as usize).collect::<Vec<_>>();
        if vertices.iter().all(|&v| bound[v]) {
            for &v in vertices.iter().skip(1) {
                let a = root(&mut parent, v);
                let b = root(&mut parent, vertices[0]);
                parent[a] = b;
            }
        }
    }
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..parent.len() {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    (0..parent.len())
        .map(|i| groups[&root(&mut parent, i)].clone())
        .collect()
}
fn ranges(lod: &Lod) -> Result<Vec<(usize, usize)>, String> {
    let mut offsets = vec![0i32];
    for face in &lod.faces {
        offsets.push(offsets.last().unwrap() + 4 + 4 * face.len() as i32);
    }
    if offsets.last().copied() != Some(lod.face_bytes as i32) {
        return Err("face data size does not match its faces".into());
    }
    let mut result = Vec::new();
    let mut expected = 0;
    for section in &lod.sections {
        let a = offsets
            .iter()
            .position(|&n| n == section.face_lower)
            .ok_or("invalid section start")?;
        let b = offsets
            .iter()
            .position(|&n| n == section.face_upper)
            .ok_or("invalid section end")?;
        if a != expected || b < a {
            return Err("sections do not partition faces".into());
        }
        result.push((a, b));
        expected = b;
    }
    if !lod.sections.is_empty() && expected != lod.faces.len() {
        return Err("sections do not cover faces".into());
    }
    Ok(result)
}
fn edit(model: &mut Model, op: &Value) -> Result<(), String> {
    let name = axis_common::string(op, "lod")?;
    let index = axis_common::lod_index(model, name)?;
    let frame = Frame::from(op)?;
    let bound = axis_common::bound(model, &model.lods[index], op)?;
    let all = axis_common::number(op, "allBeyond")?;
    let any = axis_common::number(op, "anyBeyond")?;
    let expected = op.get("faces").and_then(Value::as_i64).unwrap_or(-1);
    let lod = &mut model.lods[index];
    let along = lod
        .vertices
        .iter()
        .map(|&p| frame.along(p))
        .collect::<Vec<_>>();
    let parts = op
        .get("wholeParts")
        .and_then(Value::as_bool)
        .unwrap_or(false)
        .then(|| parts(lod, &bound));
    let old_ranges = ranges(lod)?;
    let mut kept = Vec::new();
    let mut indices = vec![-1; lod.faces.len()];
    let mut new_ranges = Vec::new();
    for (a, b) in old_ranges {
        let start = kept.len();
        for (f, slot) in indices.iter_mut().enumerate().take(b).skip(a) {
            let face = &lod.faces[f];
            let vs = face.iter().map(|&v| v as usize).collect::<Vec<_>>();
            let test = parts.as_ref().map(|p| p[vs[0]].as_slice()).unwrap_or(&vs);
            let doomed = vs.iter().all(|&v| bound[v])
                && test.iter().all(|&v| along[v] > all)
                && test.iter().any(|&v| along[v] > any);
            if !doomed {
                *slot = kept.len() as i32;
                kept.push(face.clone());
            }
        }
        new_ranges.push((start, kept.len()));
    }
    let removed = lod.faces.len() - kept.len();
    if removed as i64 != expected {
        return Err(format!(
            "remove-faces matches {removed} faces in LOD {name}, the patch expects {expected}"
        ));
    }
    for s in &mut lod.selections {
        if !s.faces.is_empty() {
            let faces = s
                .faces
                .iter()
                .filter_map(|&f| indices.get(f as usize).copied().filter(|&x| x >= 0))
                .collect();
            odol::edit_arrays::set_faces(s, faces)?;
        }
    }
    let mut offsets = vec![0i32];
    for face in &kept {
        offsets.push(offsets.last().unwrap() + 4 + 4 * face.len() as i32);
    }
    for (s, (a, b)) in lod.sections.iter_mut().zip(new_ranges) {
        s.face_lower = offsets[a];
        s.face_upper = offsets[b];
    }
    lod.face_bytes = *offsets.last().unwrap() as u32;
    lod.faces = kept;
    Ok(())
}
pub fn remove_faces(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    let run = || -> Result<_, String> {
        let mut model = Model::parse(data)?;
        if model.write()? != data {
            return Err("the ODOL library does not re-serialise this model byte-identically; refusing to patch it".into());
        }
        edit(&mut model, op)?;
        let out = model.write()?;
        let parsed = Model::parse(&out)?;
        for l in &parsed.lods {
            ranges(l)?;
        }
        Ok(out)
    };
    run().map_err(|e| format!("{label}: {e}"))
}
