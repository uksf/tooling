use crate::{
    axis_common::{self, Frame},
    cylinder_uv::Cylinder,
    face_projection, mesh_insert,
    mesh_math::V,
    odol::{self, Lod, Model},
    source_mesh::{self, Mesh},
    stretch_ops,
};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};
fn band_faces(
    lod: &Lod,
    frame: &Frame,
    bound: &[bool],
    band: &Value,
) -> Result<Vec<usize>, String> {
    let min = axis_common::number(band, "alongMin")?;
    let max = axis_common::number(band, "alongMax")?;
    let radius = axis_common::number(band, "radius")?;
    let tol = axis_common::number(band, "tolerance")?;
    Ok(lod
        .faces
        .iter()
        .enumerate()
        .filter(|(_, f)| {
            f.iter().all(|&v| {
                let v = v as usize;
                let p = lod.vertices[v];
                let a = frame.along(p);
                bound[v] && a > min - tol && a < max + tol && (frame.radius(p) - radius).abs() < tol
            })
        })
        .map(|(i, _)| i)
        .collect())
}
fn packed(v: V) -> u32 {
    let c = |x: f32| ((-x * 511.).round_ties_even().clamp(-511., 511.) as i32 & 0x3ff) as u32;
    c(v.0) | (c(v.1) << 10) | (c(v.2) << 20)
}
fn append(
    lod: &mut Lod,
    mesh: &Mesh,
    fits: &[Option<Cylinder<'_>>],
    op: &Value,
    band: &[usize],
    ring: &[usize],
    bound: &[bool],
) -> Result<(), String> {
    let mut seen = HashSet::new();
    let targets: Vec<_> = lod
        .faces
        .iter()
        .flatten()
        .map(|&v| v as usize)
        .filter(|&v| seen.insert(v) && bound[v])
        .collect();
    let st = odol::mesh_arrays::st(lod)?;
    let has_tangents = st.len() == lod.vertices.len() * 8;
    let tangent_fit = fits.iter().position(Option::is_some);
    let old = lod.vertices.len();
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut tangents = Vec::new();
    let mut uvs: Vec<_> = lod
        .uv
        .iter()
        .zip(fits)
        .map(|(u, fit)| fit.as_ref().map(|_| u.coords.clone()))
        .collect();
    let mut keyed = HashMap::new();
    let mut faces = Vec::new();
    let fold = axis_common::number(op, "attachAlong")?;
    for face in &mesh.faces {
        let mut pts = Vec::new();
        for c in face {
            let p = mesh.points[c.point];
            if mesh.weld[c.point] {
                let nearest = targets
                    .iter()
                    .map(|&v| V::from(lod.vertices[v]))
                    .min_by(|a, b| (*a - p).len().total_cmp(&(*b - p).len()))
                    .ok_or("no weld targets")?;
                if (nearest - p).len() >= 0.006 {
                    return Err(format!(
                        "no barrel vertex within 6 mm of weld point {}",
                        crate::dotnet_fmt::vector3([p.0, p.1, p.2])
                    ));
                }
                pts.push(nearest);
            } else {
                pts.push(p);
            }
        }
        let projected: Vec<_> = fits
            .iter()
            .map(|f| {
                f.as_ref()
                    .map(|fit| face_projection::project(fit, &pts, fold))
            })
            .collect();
        let mut idx = Vec::new();
        for (k, c) in face.iter().enumerate() {
            let uv: Vec<_> = projected
                .iter()
                .map(|s| s.as_ref().map(|s| s[k].uv).unwrap_or([0., 0.]))
                .collect();
            let normal = packed(-c.normal);
            let key = (
                c.point,
                normal,
                (uv[0][0] * 4096.).round_ties_even() as i32,
                (uv[0][1] * 4096.).round_ties_even() as i32,
            );
            let v = if let Some(&v) = keyed.get(&key) {
                v
            } else {
                let v = old + positions.len();
                keyed.insert(key, v);
                positions.push(pts[k].array());
                normals.push(normal);
                if has_tangents {
                    let t = if let Some(i) = tangent_fit {
                        let corner = &projected[i].as_ref().unwrap()[k];
                        let (s, t) = fits[i]
                            .as_ref()
                            .unwrap()
                            .tangents(corner.dt, corner.dl, c.normal);
                        [packed(s), packed(t)]
                    } else {
                        let b = &st[ring[0] * 8..ring[0] * 8 + 8];
                        [
                            u32::from_le_bytes(b[..4].try_into().unwrap()),
                            u32::from_le_bytes(b[4..].try_into().unwrap()),
                        ]
                    };
                    tangents.push(t);
                }
                for (u, p) in uvs.iter_mut().zip(uv) {
                    if let Some(u) = u {
                        u.push(p);
                    }
                }
                v
            };
            idx.push(v as i32);
        }
        faces.push(idx);
    }
    let added = positions.len();
    odol::mesh_arrays::append(
        lod,
        odol::mesh_arrays::Appended {
            positions,
            normals,
            tangents,
            uvs,
            donor: ring[0],
        },
    )?;
    mesh_insert::insert(lod, faces, band, ring, old, added)?;
    Ok(())
}
fn edit(model: &mut Model, op: &Value, base: &Path) -> Result<(), String> {
    let source = axis_common::string(op, "source")?;
    let data = fs::read(base.join(source))
        .map_err(|_| format!("add-mesh source '{source}' does not exist"))?;
    let frame = Frame::from(op)?;
    let mesh = source_mesh::load(&data, op, &frame)?;
    let band = op.get("uvFrom").ok_or("missing uvFrom")?;
    for name in op
        .get("lods")
        .and_then(Value::as_array)
        .ok_or("missing lods")?
    {
        let name = name.as_str().ok_or("invalid lod")?;
        let index = axis_common::lod_index(model, name)?;
        let bound_op = serde_json::json!({"bones":[axis_common::string(op, "bone")?]});
        let bound = axis_common::bound(model, &model.lods[index], &bound_op)?;
        let faces = band_faces(&model.lods[index], &frame, &bound, band)?;
        if faces.is_empty() {
            return Err(format!("LOD {name} has no barrel faces in the uvFrom band"));
        }
        let max = axis_common::number(band, "alongMax")?;
        let tol = axis_common::number(band, "tolerance")?;
        let mut seen = HashSet::new();
        let ring: Vec<_> = faces
            .iter()
            .flat_map(|&f| model.lods[index].faces[f].iter().copied())
            .map(|v| v as usize)
            .filter(|&v| {
                seen.insert(v) && (frame.along(model.lods[index].vertices[v]) - max).abs() < tol
            })
            .collect();
        if ring.is_empty() {
            return Err(format!("LOD {name} has no barrel ring at the attach point"));
        }
        let (fit_index, fit_faces) = if let Some(fit) = band.get("fit") {
            let i = axis_common::lod_index(model, axis_common::string(fit, "lod")?)?;
            let b = axis_common::bound(model, &model.lods[i], &bound_op)?;
            (i, band_faces(&model.lods[i], &frame, &b, fit)?)
        } else {
            (index, faces.clone())
        };
        let lod = &model.lods[index];
        let from = &model.lods[fit_index];
        let fits = lod
            .uv
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if s.fill != 0 || i >= from.uv.len() || from.uv[i].fill != 0 {
                    Ok(None)
                } else {
                    Cylinder::fit(&frame, from, &from.uv[i].coords, &fit_faces).map(Some)
                }
            })
            .collect::<Result<Vec<_>, String>>()?;
        append(
            &mut model.lods[index],
            &mesh,
            &fits,
            op,
            &faces,
            &ring,
            &bound,
        )?;
        stretch_ops::grow(model, index);
    }
    Ok(())
}
pub fn add(data: &[u8], op: &Value, label: &str, base: &Path) -> Result<Vec<u8>, String> {
    let run = || -> Result<_, String> {
        let mut model = Model::parse(data)?;
        if model.write()? != data {
            return Err("the ODOL library does not re-serialise this model byte-identically; refusing to patch it".into());
        }
        edit(&mut model, op, base)?;
        let out = model.write()?;
        let parsed = Model::parse(&out)?;
        for l in &parsed.lods {
            crate::face_ops::ranges(l)?;
        }
        Ok(out)
    };
    run().map_err(|e| format!("{label}: {e}"))
}
