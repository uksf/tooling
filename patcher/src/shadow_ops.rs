use crate::{
    axis_common, face_ops,
    odol::{self, Model},
    stretch_ops,
};
use serde_json::Value;
use std::collections::HashMap;

fn key(p: [f32; 3]) -> [u32; 3] {
    p.map(|x| if x == 0.0 { 0 } else { x.to_bits() })
}
fn edit(model: &mut Model, op: &Value) -> Result<(), String> {
    let from = axis_common::lod_index(model, axis_common::string(op, "from")?)?;
    let to = axis_common::lod_index(model, axis_common::string(op, "lod")?)?;
    if from == to {
        return Err("source and volume must differ".into());
    }
    let source = &model.lods[from];
    let volume = &model.lods[to];
    let problem = if volume.faces.is_empty() {
        Some("the target LOD is empty")
    } else if volume.uv.len() != 1 || volume.uv[0].fill == 0 {
        Some("the target LOD has real UVs")
    } else if !volume.vertex_to_point.is_empty() || !source.vertex_to_point.is_empty() {
        Some("a LOD has a vertex-to-point map")
    } else if !volume.frames.is_empty() || !source.frames.is_empty() {
        Some("a LOD has keyframes")
    } else if volume.sub_to_skeleton != source.sub_to_skeleton {
        Some("the LODs map bones differently")
    } else if !(source.sub_to_skeleton.is_empty()
        && source.bones.is_empty()
        && volume.bones.is_empty())
        && source.bones.len() != source.vertices.len()
    {
        Some("the source LOD lacks per-vertex bones")
    } else if source.clip.len() != source.vertices.len() {
        Some("the source LOD lacks per-vertex clip flags")
    } else if volume.sections.len() != source.sections.len() {
        Some("the LODs have different section counts")
    } else if volume
        .selections
        .iter()
        .chain(&source.selections)
        .any(|s| !s.faces.is_empty() || !s.vertices.is_empty())
    {
        Some("a named selection holds faces or vertices")
    } else {
        None
    };
    if let Some(p) = problem {
        return Err(format!(
            "rebuild-shadow-volume does not support this model: {p}"
        ));
    }
    if source.faces.iter().any(|f| f.len() != 3) {
        return Err("rebuild-shadow-volume needs a triangulated source LOD".into());
    }
    let origin: Vec<usize> = source.faces.iter().flatten().map(|&v| v as usize).collect();
    if origin.len() > u16::MAX as usize {
        return Err("shadow volume overflows the 16-bit neighbour index".into());
    }
    type Edge = ([u32; 3], [u32; 3]);
    let mut edges: HashMap<Edge, Vec<(usize, usize)>> = HashMap::new();
    for (f, face) in source.faces.iter().enumerate() {
        for k in 0..3 {
            let a = key(source.vertices[face[k] as usize]);
            let b = key(source.vertices[face[(k + 1) % 3] as usize]);
            edges.entry((a, b)).or_default().push((f, k));
        }
    }
    let ranges = face_ops::ranges(source)?;
    let mut faces = Vec::new();
    let mut new_ranges = Vec::new();
    for (first, end) in ranges {
        let start = faces.len();
        for f in first..end {
            let c = [3 * f, 3 * f + 1, 3 * f + 2];
            faces.push(c.iter().map(|&x| x as i32).collect::<Vec<_>>());
            for k in [2, 0, 1] {
                let a = key(source.vertices[origin[c[(k + 1) % 3]]]);
                let b = key(source.vertices[origin[c[k]]]);
                if let Some(twins) = edges.get(&(a, b)) {
                    if twins.len() > 1 {
                        return Err("non-manifold edge".into());
                    }
                    let (tf, tk) = twins[0];
                    faces.push(vec![
                        c[k] as i32,
                        (tf * 3 + (tk + 1) % 3) as i32,
                        c[(k + 1) % 3] as i32,
                    ]);
                }
            }
        }
        new_ranges.push((start, faces.len()));
    }
    let (source, volume) = if from < to {
        let (a, b) = model.lods.split_at_mut(to);
        (&a[from], &mut b[0])
    } else {
        let (a, b) = model.lods.split_at_mut(from);
        (&b[0], &mut a[to])
    };
    odol::edit_arrays::shadow_arrays(source, volume, &origin)?;
    let mut offsets = vec![0i32];
    for f in &faces {
        offsets.push(offsets.last().unwrap() + 4 + 4 * f.len() as i32);
    }
    for (s, (a, b)) in volume.sections.iter_mut().zip(new_ranges) {
        s.face_lower = offsets[a];
        s.face_upper = offsets[b];
    }
    volume.face_bytes = *offsets.last().unwrap() as u32;
    volume.faces = faces;
    stretch_ops::grow(model, to);
    Ok(())
}
pub fn rebuild(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    let run = || -> Result<_, String> {
        let mut model = Model::parse(data)?;
        if model.write()? != data {
            return Err("the ODOL library does not re-serialise this model byte-identically; refusing to patch it".into());
        }
        edit(&mut model, op)?;
        let out = model.write()?;
        let parsed = Model::parse(&out)?;
        for l in &parsed.lods {
            face_ops::ranges(l)?;
        }
        Ok(out)
    };
    run().map_err(|e| format!("{label}: {e}"))
}
