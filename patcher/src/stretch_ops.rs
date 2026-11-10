use crate::{
    axis_common::{self, Frame},
    odol::{self, Model},
};
use serde_json::Value;
use std::collections::HashSet;

pub(crate) fn grow(model: &mut Model, i: usize) {
    let l = &mut model.lods[i];
    for p in &l.vertices {
        for k in 0..3 {
            l.bounds_min[k] = l.bounds_min[k].min(p[k]);
            l.bounds_max[k] = l.bounds_max[k].max(p[k]);
        }
        let d = [
            p[0] - l.bounds_center[0],
            p[1] - l.bounds_center[1],
            p[2] - l.bounds_center[2],
        ];
        l.bounds_radius = l
            .bounds_radius
            .max((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt());
        model.info.bounding_sphere = model
            .info
            .bounding_sphere
            .max((p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt());
    }
    if l.vertices.is_empty() {
        return;
    }
    for k in 0..3 {
        model.info.bbox_min[k] = model.info.bbox_min[k].min(l.bounds_min[k]);
        model.info.bbox_max[k] = model.info.bbox_max[k].max(l.bounds_max[k]);
        if l.resolution < 1e13 {
            model.info.bbox_min_visual[k] = model.info.bbox_min_visual[k].min(l.bounds_min[k]);
            model.info.bbox_max_visual[k] = model.info.bbox_max_visual[k].max(l.bounds_max[k]);
        }
    }
}
fn edit(model: &mut Model, op: &Value) -> Result<(), String> {
    let frame = Frame::from(op)?;
    let max_radius = if op.get("maxRadius").is_some() {
        axis_common::number(op, "maxRadius")?
    } else {
        f32::INFINITY
    };
    let cut = axis_common::number(op, "cut")?;
    let length = axis_common::number(op, "length")?;
    let shift = frame.direction.map(|x| x * length);
    let lods = op
        .get("lods")
        .and_then(Value::as_array)
        .ok_or("missing lods")?;
    let mut moved = 0;
    for name in lods {
        let index = axis_common::lod_index(model, name.as_str().ok_or("invalid lod")?)?;
        let bound = axis_common::bound(model, &model.lods[index], op)?;
        let l = &mut model.lods[index];
        let mut vertices = l.vertices.clone();
        for (v, p) in vertices.iter_mut().enumerate() {
            if bound[v] && frame.along(*p) > cut && frame.radius(*p) <= max_radius {
                for k in 0..3 {
                    p[k] += shift[k];
                }
                moved += 1;
            }
        }
        odol::edit_arrays::set_vertices(l, vertices)?;
        if let Some(bones) = op.get("bones").and_then(Value::as_array) {
            for proxy in &mut l.proxies {
                let name = model.info.skeleton.bones.get(proxy.bone as usize);
                let pos = [proxy.transform[9], proxy.transform[10], proxy.transform[11]];
                if name.is_some_and(|n| {
                    bones
                        .iter()
                        .filter_map(Value::as_str)
                        .any(|b| n.name.eq_ignore_ascii_case(b.as_bytes()))
                }) && frame.along(pos) > cut
                {
                    for k in 0..3 {
                        proxy.transform[9 + k] += shift[k];
                    }
                }
            }
        }
        grow(model, index);
    }
    let expected = op.get("vertices").and_then(Value::as_i64).unwrap_or(-1);
    if moved != expected {
        return Err(format!(
            "stretch-along-axis moved {moved} vertices, the patch expects {expected}"
        ));
    }
    if let Some(names) = op.get("memoryPoints") {
        let index = axis_common::lod_index(model, "memory")?;
        let lod = &mut model.lods[index];
        let mut indices = HashSet::new();
        for n in names.as_array().ok_or("invalid memoryPoints")? {
            let name = n.as_str().ok_or("invalid memory point")?;
            let s = lod
                .selections
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(name.as_bytes()))
                .ok_or_else(|| format!("memory LOD has no selection '{name}'"))?;
            indices.extend(s.vertices.iter().copied());
        }
        let mut vertices = lod.vertices.clone();
        for v in indices {
            let p = vertices
                .get_mut(v as usize)
                .ok_or("invalid memory vertex")?;
            for k in 0..3 {
                p[k] += shift[k];
            }
        }
        odol::edit_arrays::set_vertices(lod, vertices)?;
    }
    Ok(())
}
pub fn stretch(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    let run = || -> Result<_, String> {
        let mut model = Model::parse(data)?;
        if model.write()? != data {
            return Err("the ODOL library does not re-serialise this model byte-identically; refusing to patch it".into());
        }
        edit(&mut model, op)?;
        let out = model.write()?;
        Model::parse(&out)?;
        Ok(out)
    };
    run().map_err(|e| format!("{label}: {e}"))
}
