use crate::{
    face_ops::ranges,
    label_quads::{Convention, Label},
    mesh_math::V,
    odol::{Lod, edit_arrays, lod_edit},
    op_json::{latin1, same},
    section_faces::{in_section, weight},
    uv_plan::pack,
};
use std::cmp::Ordering;

fn nearest(lod: &Lod, anchors: &[usize], corner: V) -> Result<usize, String> {
    let mut best: Option<(usize, f32)> = None;
    for &a in anchors {
        let d = (V::from(lod.vertices[a]) - corner).len();
        let less = match best {
            None => true,
            Some((_, key)) => {
                d.partial_cmp(&key)
                    .unwrap_or_else(|| key.is_nan().cmp(&d.is_nan()))
                    == Ordering::Less
            }
        };
        if less {
            best = Some((a, d));
        }
    }
    best.map(|b| b.0)
        .ok_or_else(|| "the anchor section has no vertices".into())
}

pub fn add_label(
    lod: &mut Lod,
    label: &Label,
    template: usize,
    anchor_section: usize,
    texture: &str,
    special: u32,
    convention: &Convention,
) -> Result<(), String> {
    let n0 = lod.vertices.len();
    let mut positions = lod.vertices.clone();
    let mut normals = lod_edit::normals(lod)?;
    let mut st = lod_edit::st(lod)?;
    let has_tangents = st.len() / 8 == n0 && st.len() % 8 == 0;
    let mut clip = lod.clip.clone();
    let mut bones = lod.bones.clone();
    let has_bones = bones.len() == n0;
    let mut uvs: Vec<Vec<[f32; 2]>> = lod.uv.iter().map(|s| s.coords.clone()).collect();
    let mut anchors: Vec<usize> = Vec::new();
    for f in in_section(lod, &lod.sections[anchor_section]) {
        for &v in &lod.faces[f] {
            if !anchors.contains(&(v as usize)) {
                anchors.push(v as usize);
            }
        }
    }
    let mut new_faces = Vec::new();
    for quad in &label.quads {
        let c = quad.corners;
        let stored = if convention.stored_normals_point_out {
            quad.normal
        } else {
            -quad.normal
        };
        let right = (c[1] - c[0]).unit();
        let down = (c[3] - c[0]).unit();
        let first = positions.len() as i32;
        for (k, corner) in c.iter().enumerate() {
            let anchor = nearest(lod, &anchors, *corner)?;
            positions.push(corner.array());
            normals.extend(pack(&stored.array()));
            if has_tangents {
                st.extend(pack(&right.array()));
                st.extend(pack(&down.array()));
            }
            clip.push(
                *lod.clip
                    .get(anchor)
                    .ok_or("anchor vertex has no clip flags")?,
            );
            if has_bones {
                bones.push(bones[anchor].clone());
            }
            for set in &mut uvs {
                set.push(quad.uv[k]);
            }
        }
        let out_facing = (c[1] - c[0]).cross(c[3] - c[0]).dot(quad.normal) > 0.0;
        let order = if out_facing == convention.cross_right_down_is_out {
            [0, 1, 2, 3]
        } else {
            [0, 3, 2, 1]
        };
        new_faces.push(order.iter().map(|i| first + i).collect::<Vec<i32>>());
    }
    let count = positions.len();
    edit_arrays::set_vertices(lod, positions)?;
    lod_edit::set_normals(lod, &normals)?;
    lod_edit::set_st(lod, &st)?;
    lod_edit::set_clip(lod, clip)?;
    if has_bones {
        lod_edit::set_bones(lod, bones)?;
    }
    for (set, uv) in lod.uv.iter_mut().zip(&uvs) {
        if set.fill == 0 {
            lod_edit::write_uv(set, uv)?;
        }
        set.vertices = count as u32;
        lod_edit::reload_uv(set)?;
    }
    lod.vertex_count = count as u32;
    lod_edit::refresh_rest(lod)?;
    append_section(lod, &label.selection, new_faces, template, texture, special)
}

fn append_section(
    lod: &mut Lod,
    selection: &str,
    new_faces: Vec<Vec<i32>>,
    template: usize,
    texture: &str,
    special: u32,
) -> Result<(), String> {
    let mut old = ranges(lod)?;
    let end = lod.faces.len();
    let texture_index = match lod.textures.iter().position(|t| same(t, texture)) {
        Some(i) => i,
        None => {
            lod.textures.push(latin1(texture)?);
            lod.textures.len() - 1
        }
    };
    let mut section = lod.sections[template].clone();
    section.texture = texture_index as i16;
    section.special = special;
    lod.sections.push(section);
    let index = (lod.sections.len() - 1) as i32;
    lod.selections.push(lod_edit::new_selection(
        latin1(selection)?,
        true,
        &[index],
        &[],
        &[],
    )?);
    old.push((end, end + new_faces.len()));
    lod.faces.extend(new_faces);
    let mut offsets = vec![0i32];
    for f in &lod.faces {
        offsets.push(offsets[offsets.len() - 1] + weight(f));
    }
    for (s, (a, b)) in lod.sections.iter_mut().zip(old) {
        s.face_lower = offsets[a];
        s.face_upper = offsets[b];
    }
    lod.face_bytes = offsets[offsets.len() - 1] as u32;
    Ok(())
}
