use crate::{
    axis_common::lod_index,
    mesh_math::V,
    odol::Lod,
    op_json::{edit, floats, int, same, single, string},
    section_faces::in_section,
};
use serde_json::Value;

const CORNER_UV: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

pub struct Quad {
    pub corners: [V; 4],
    pub normal: V,
    pub uv: [[f32; 2]; 4],
}

pub struct Label {
    pub selection: String,
    pub quads: Vec<Quad>,
}

pub struct Convention {
    pub cross_right_down_is_out: bool,
    pub stored_normals_point_out: bool,
}

fn elements(v: &Value) -> Result<Vec<f32>, String> {
    v.as_array()
        .ok_or("expected an array of numbers")?
        .iter()
        .map(|x| single(x).ok_or_else(|| "expected a finite number".to_owned()))
        .collect()
}

fn array<'a>(v: &'a Value, key: &str) -> Result<&'a Vec<Value>, String> {
    v.get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("label needs \"{key}\" as an array"))
}

fn uvs(quad: &Value) -> Result<[[f32; 2]; 4], String> {
    let Some(uv) = quad.get("uv") else {
        return Ok(CORNER_UV);
    };
    let bad = || "label quad uv needs 4 finite [u, v] pairs".to_owned();
    let pts = uv
        .as_array()
        .ok_or_else(bad)?
        .iter()
        .map(|c| {
            let f = elements(c)?;
            Ok((f.len() == 2 && (f[0] + f[1]).is_finite()).then(|| [f[0], f[1]]))
        })
        .collect::<Result<Vec<_>, String>>()?;
    match pts.as_slice() {
        [Some(a), Some(b), Some(c), Some(d)] => Ok([*a, *b, *c, *d]),
        _ => Err(bad()),
    }
}

fn quad(q: &Value) -> Result<Quad, String> {
    let corners = array(q, "corners")?
        .iter()
        .map(|c| {
            let f = elements(c)?;
            Ok(if f.len() == 3 {
                V(f[0], f[1], f[2])
            } else {
                V(f32::NAN, f32::NAN, f32::NAN)
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let corners: [V; 4] = corners
        .try_into()
        .ok()
        .filter(|c: &[V; 4]| c.iter().all(|c| (c.0 + c.1 + c.2).is_finite()))
        .ok_or("a label quad needs 4 corners (top-left, top-right, bottom-right, bottom-left as seen from outside)")?;
    let n = floats(q, "normal", 3)?;
    let normal = V(n[0], n[1], n[2]).unit();
    let facing = (corners[1] - corners[0])
        .cross(corners[3] - corners[0])
        .dot(normal);
    if facing.abs() < 1e-6 {
        return Err("label quad corners are degenerate or parallel to its normal".into());
    }
    Ok(Quad {
        corners,
        normal,
        uv: uvs(q)?,
    })
}

fn labels(op: &Value) -> Result<Vec<Label>, String> {
    let list = op
        .get("labels")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .ok_or("add-label-quads needs a non-empty \"labels\" array")?;
    list.iter()
        .map(|l| {
            let quads = array(l, "quads")?
                .iter()
                .map(quad)
                .collect::<Result<_, _>>()?;
            Ok(Label {
                selection: string(l, "selection")?.to_owned(),
                quads,
            })
        })
        .collect()
}

fn sign(x: f32) -> Result<i64, String> {
    if x.is_nan() {
        return Err("a template face has a non-finite normal".into());
    }
    Ok(if x > 0.0 {
        1
    } else if x < 0.0 {
        -1
    } else {
        0
    })
}

fn convention(lod: &Lod, template: usize) -> Result<Convention, String> {
    let centre = (V::from(lod.bounds_min) + V::from(lod.bounds_max)) / 2.0;
    let (mut winding, mut stored) = (0, 0);
    for f in in_section(lod, &lod.sections[template]) {
        let v = &lod.faces[f];
        if v.len() < 3 {
            return Err("a template face has fewer than three vertices".into());
        }
        let p: Vec<V> = v
            .iter()
            .map(|&i| V::from(lod.vertices[i as usize]))
            .collect();
        let sum = p.iter().fold(V(0.0, 0.0, 0.0), |a, &b| a + b);
        let outward = sum / p.len() as f32 - centre;
        let geometric = (p[1] - p[0]).cross(p[2] - p[0]);
        let n = V::from(lod.normals[v[0] as usize]);
        winding += sign(geometric.dot(outward))?;
        stored += sign(n.dot(outward))?;
    }
    Ok(Convention {
        cross_right_down_is_out: winding > 0,
        stored_normals_point_out: stored > 0,
    })
}

pub fn add_label_quads(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, true, |model| {
        let labels = labels(op)?;
        let texture = string(op, "texture")?;
        for name in crate::op_json::strings(op, "lods")? {
            let li = lod_index(model, name)?;
            let lod = &model.lods[li];
            let template = int(op, "templateSection", -1)?;
            let template = usize::try_from(template)
                .ok()
                .filter(|&t| t < lod.sections.len())
                .ok_or_else(|| format!("LOD {name} has no template section {template}"))?;
            for l in &labels {
                if lod.selections.iter().any(|s| same(&s.name, &l.selection)) {
                    return Err(format!(
                        "LOD {name} already has a selection '{}'",
                        l.selection
                    ));
                }
            }
            let anchor = int(op, "anchorSection", template as i32)?;
            let anchor = usize::try_from(anchor)
                .ok()
                .filter(|&t| t < lod.sections.len())
                .ok_or_else(|| format!("LOD {name} has no anchor section {anchor}"))?;
            let convention = convention(lod, template)?;
            let special = match op.get("special") {
                Some(_) => int(op, "special", 0)? as u32,
                None => lod.sections[template].special,
            };
            for l in &labels {
                crate::label_add::add_label(
                    &mut model.lods[li],
                    l,
                    template,
                    anchor,
                    texture,
                    special,
                    &convention,
                )?;
            }
            crate::stretch_ops::grow(model, li);
            let lod = &model.lods[li];
            if let Some(info) = model.loadable[li].as_mut() {
                info.faces = lod.faces.len() as i32;
                info.vertices = lod.vertex_count as i32;
                info.or_hints = lod.or_hints as u32;
            }
        }
        Ok(())
    })
}
