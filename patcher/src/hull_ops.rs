use crate::{
    axis_common::lod_index,
    convex_cache, convex_hull,
    face_ops::ranges,
    mesh_math::V,
    odol::{Lod, edit_arrays},
    op_json::{edit_unlabelled, same, single, string, strings, text},
    section_faces::weight,
};
use serde_json::Value;
use std::collections::HashSet;

fn is_component(name: &[u8]) -> bool {
    name.get(..9).is_some_and(|n| same(n, "component"))
}

fn position(lod: &Lod, v: i32) -> Result<V, String> {
    usize::try_from(v)
        .ok()
        .and_then(|v| lod.vertices.get(v))
        .map(|&p| V::from(p))
        .ok_or_else(|| format!("vertex {v} is outside the LOD"))
}

fn faces_point_outward(lod: &Lod, except: usize) -> Result<bool, String> {
    let mut votes = 0i64;
    for (i, c) in lod.selections.iter().enumerate() {
        if i == except || !is_component(&c.name) {
            continue;
        }
        let mut sum = V(0.0, 0.0, 0.0);
        for &v in &c.vertices {
            sum = sum + position(lod, v)?;
        }
        let centre = sum / c.vertices.len().max(1) as f32;
        for &f in &c.faces {
            let face = usize::try_from(f)
                .ok()
                .and_then(|f| lod.faces.get(f))
                .filter(|f| f.len() >= 3)
                .ok_or("component face is not a polygon of the LOD")?;
            let a = position(lod, face[0])?;
            let n = (position(lod, face[1])? - a).cross(position(lod, face[2])? - a);
            let side = n.dot(a - centre);
            votes += if side > 0.0 {
                1
            } else if side < 0.0 {
                -1
            } else {
                0
            };
        }
    }
    Ok(votes >= 0)
}

fn shear_vertices(
    lod: &mut Lod,
    index: usize,
    ids: &[i32],
    z_mid: f32,
    shear: f32,
) -> Result<(), String> {
    let shared: HashSet<i32> = lod
        .selections
        .iter()
        .enumerate()
        .filter(|(i, s)| *i != index && is_component(&s.name))
        .flat_map(|(_, s)| s.vertices.iter().copied())
        .collect();
    if ids.iter().any(|v| shared.contains(v)) {
        return Err("shear would move a vertex shared with another component".into());
    }
    let mut vertices = lod.vertices.clone();
    for &id in ids {
        let p = usize::try_from(id)
            .ok()
            .and_then(|v| vertices.get_mut(v))
            .ok_or("component vertex is outside the LOD")?;
        p[1] += shear * (p[2] - z_mid);
    }
    edit_arrays::set_vertices(lod, vertices)
}

fn rebuild(lod: &mut Lod, name: &str, shear: f32, extra: &mut [u8]) -> Result<(), String> {
    let hits: Vec<usize> = (0..lod.selections.len())
        .filter(|&i| same(&lod.selections[i].name, name))
        .collect();
    let index = match hits.as_slice() {
        [one] => *one,
        [] => return Err("no such selection".into()),
        _ => return Err("more than one such selection".into()),
    };
    if !is_component(&lod.selections[index].name) {
        return Err("not a component selection".into());
    }
    let old_ranges = ranges(lod)?;
    let old_faces: HashSet<i32> = lod.selections[index].faces.iter().copied().collect();
    let ids = lod.selections[index].vertices.clone();
    let before = ids
        .iter()
        .map(|&v| position(lod, v))
        .collect::<Result<Vec<_>, _>>()?;
    let (min, max) = convex_hull::bounds(before.iter().copied());
    if shear != 0.0 {
        shear_vertices(lod, index, &ids, (min.2 + max.2) * 0.5, shear)?;
    }
    let points = ids
        .iter()
        .map(|&v| position(lod, v))
        .collect::<Result<Vec<_>, _>>()?;
    let hull = convex_hull::build(&ids, &points)?;
    let outward = faces_point_outward(lod, index)?;
    let hull_faces: Vec<Vec<i32>> = hull
        .iter()
        .map(|t| {
            if outward {
                vec![t[0], t[1], t[2]]
            } else {
                vec![t[0], t[2], t[1]]
            }
        })
        .collect();
    let insert_at = *old_faces.iter().min().ok_or("the component has no faces")?;
    let n = lod.faces.len();
    let mut faces: Vec<Vec<i32>> = Vec::with_capacity(n + hull_faces.len());
    let mut map = vec![0i32; n];
    let mut hull_indices = Vec::new();
    let mut boundary = vec![0usize; n + 1];
    for i in 0..n {
        boundary[i] = faces.len();
        if i as i64 == i64::from(insert_at) {
            hull_indices.extend((faces.len()..faces.len() + hull_faces.len()).map(|f| f as i32));
            faces.extend(hull_faces.iter().cloned());
        }
        let old = old_faces.contains(&(i as i32));
        map[i] = if old { -1 } else { faces.len() as i32 };
        if !old {
            faces.push(lod.faces[i].clone());
        }
    }
    boundary[n] = faces.len();
    let mapped = |f: i32| -> Result<i32, String> {
        usize::try_from(f)
            .ok()
            .and_then(|f| map.get(f).copied())
            .ok_or_else(|| format!("selection face {f} is outside the LOD"))
    };
    for (i, s) in lod.selections.iter_mut().enumerate() {
        let hit = s.faces.iter().filter(|f| old_faces.contains(f)).count();
        if hit == 0 && i != index {
            let moved = s
                .faces
                .iter()
                .map(|&f| mapped(f))
                .collect::<Result<Vec<_>, _>>()?;
            if moved.iter().zip(&s.faces).any(|(a, b)| a != b) {
                edit_arrays::set_faces(s, moved)?;
            }
            continue;
        }
        if hit != old_faces.len() {
            return Err(format!(
                "selection '{}' holds only some of the component's faces",
                text(&s.name)
            ));
        }
        let mut kept = Vec::new();
        for &f in &s.faces {
            let m = mapped(f)?;
            if m >= 0 {
                kept.push(m);
            }
        }
        kept.extend(&hull_indices);
        edit_arrays::set_faces(s, kept)?;
    }
    let mut offsets = vec![0i32];
    for f in &faces {
        offsets.push(offsets[offsets.len() - 1] + weight(f));
    }
    for (s, (a, b)) in lod.sections.iter_mut().zip(old_ranges) {
        s.face_lower = offsets[boundary[a]];
        s.face_upper = offsets[boundary[b]];
    }
    lod.face_bytes = offsets[offsets.len() - 1] as u32;
    lod.faces = faces;
    let eps = convex_hull::tolerance(&points);
    for &h in &hull_indices {
        let ids = lod.faces[h as usize].clone();
        let t = if outward {
            [ids[0], ids[1], ids[2]]
        } else {
            [ids[0], ids[2], ids[1]]
        };
        let corners = [
            position(lod, t[0])?,
            position(lod, t[1])?,
            position(lod, t[2])?,
        ];
        if convex_hull::max_outside(corners, &points) > eps {
            let list: Vec<String> = ids.iter().map(i32::to_string).collect();
            return Err(format!(
                "hull face [{}] has component points outside it",
                list.join(",")
            ));
        }
    }
    if shear == 0.0 {
        return Ok(());
    }
    if convex_cache::refresh(extra, min, max, shear).map_err(|e| format!("\0{e}"))? == 0 {
        return Err("no cooked convex component matches the sheared hull".into());
    }
    Ok(())
}

pub fn convex_hull_component(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit_unlabelled(data, true, |model| {
        let component = string(op, "component")?;
        let shear = match op.get("shear") {
            Some(v) if v.is_number() => {
                single(v).ok_or("shear must be a finite number of at most 0.2")?
            }
            _ => 0.0,
        };
        if !shear.is_finite() || shear.abs() > 0.2 {
            return Err("shear must be a finite number of at most 0.2".into());
        }
        for lod_name in strings(op, "lods")? {
            let li = lod_index(model, lod_name)?;
            let mut extra = std::mem::take(&mut model.extra);
            let result = rebuild(&mut model.lods[li], component, shear, &mut extra);
            model.extra = extra;
            result.map_err(|e| match e.strip_prefix('\0') {
                Some(plain) => format!("\0{plain}"),
                None => format!("\u{1}{lod_name}:{component}: {e}"),
            })?;
        }
        Ok(())
    })
    .map_err(|e| {
        if let Some(plain) = e.strip_prefix('\0') {
            plain.to_owned()
        } else if let Some(located) = e.strip_prefix('\u{1}') {
            format!("{label}:{located}")
        } else {
            format!("{label}: {e}")
        }
    })
}
