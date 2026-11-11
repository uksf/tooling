use crate::{
    axis_common::lod_index,
    cut_seams::source_json,
    odol::{Lod, Section, lod_edit},
    op_json::{edit, latin1, same, string},
    section_faces::{in_section, selection, weight},
    uv_plan::{Island, area, pack, read, sum},
};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

const NO_CLAMP: u32 = 0x2000;
const CLAMP_U: u32 = 0x4000;
const CLAMP_V: u32 = 0x8000;

struct Ctx<'a> {
    faces: &'a [Vec<i32>],
    old: &'a [[f32; 2]],
    uv: &'a [[f32; 2]],
    owner: &'a HashMap<usize, &'a Island>,
}

impl Ctx<'_> {
    fn wrap(&self, s: &mut Section, fs: &[usize]) {
        let outside = fs.iter().flat_map(|&f| &self.faces[f]).any(|&v| {
            let p = self.uv[v as usize];
            p[0] < 0. || p[0] > 1. || p[1] < 0. || p[1] > 1.
        });
        if outside {
            s.special = (s.special & !(CLAMP_U | CLAMP_V)) | NO_CLAMP;
        }
    }

    fn on_b(&self, f: usize) -> Result<bool, String> {
        let sheets: HashSet<Option<bool>> = self.faces[f]
            .iter()
            .map(|v| self.owner.get(&(*v as usize)).map(|i| i.on_b))
            .collect();
        match sheets.into_iter().collect::<Vec<_>>().as_slice() {
            [one] => Ok(*one == Some(true)),
            _ => Err(format!(
                "uv-split face {f} spans islands on different sheets"
            )),
        }
    }

    fn area_over_tex(&self, aot: f32, fs: &[usize]) -> Result<f32, String> {
        let before = sum(fs.iter().map(|&f| area(self.old, &self.faces[f])));
        if before <= 1e-12 {
            return Ok(aot);
        }
        let ratio = sum(fs.iter().map(|&f| area(self.uv, &self.faces[f]))) / before;
        let result = aot / ratio;
        if ratio > 0. && ratio.is_finite() && result.is_finite() {
            Ok(result)
        } else {
            Err(format!(
                "uv-split collapses a section's UV area to {}",
                crate::dotnet_fmt::float(ratio)
            ))
        }
    }
}

fn set_aot(s: &mut Section, value: f32) -> Result<(), String> {
    *s.area_over_tex
        .first_mut()
        .ok_or("section has no area over texture")? = value;
    Ok(())
}

fn shift(lod: &mut Lod, n: usize, s: i32, join: Option<i32>) -> Result<(), String> {
    let sel = &mut lod.selections[n];
    let mut list: Vec<i32> = sel
        .sections
        .iter()
        .map(|&x| if x > s { x + 1 } else { x })
        .collect();
    if let Some(j) = join.filter(|_| list.contains(&s)) {
        list.push(j);
    }
    list.sort_unstable();
    lod_edit::set_selection_sections(sel, list)
}

fn section_of_face(faces: &[Vec<i32>], sections: &[Section], face: i32) -> Result<i32, String> {
    let face = usize::try_from(face)
        .map_err(|_| "uv-split moved a section holding a proxy with no face")?;
    let offset: i32 = faces.iter().take(face).map(|f| weight(f)).sum();
    sections
        .iter()
        .position(|x| offset >= x.face_lower && offset < x.face_upper)
        .map(|s| s as i32)
        .ok_or_else(|| format!("uv-split left proxy face {face} outside every section"))
}

fn split(lod: &mut Lod, op: &Value, plan: &[Island]) -> Result<(), String> {
    let new_selection = string(op, "newSelection")?;
    let target = selection(lod, string(op, "selection")?)?;
    if lod.selections[target].is_sectional == 0 {
        return Err("uv-split selection is not sectional".into());
    }
    if lod.selections.iter().any(|n| same(&n.name, new_selection)) {
        return Err(format!("LOD already has selection '{new_selection}'"));
    }
    let mut owner = HashMap::new();
    for i in plan {
        for &v in &i.verts {
            owner.insert(v, i);
        }
    }
    let old = lod.uv.first().ok_or("LOD has no UV set")?.coords.clone();
    let mut uv = old.clone();
    let mut st = crate::odol::mesh_arrays::st(lod)?;
    for i in plan {
        for (k, &v) in i.verts.iter().enumerate() {
            uv[v] = match (&i.uv, i.bounds, i.pos) {
                (Some(rows), _, _) => [rows[k][0], rows[k][1]],
                (None, Some(b), Some(p)) => [
                    (old[v][0] - b[0]) * i.s + p[0],
                    (old[v][1] - b[1]) * i.s + p[1],
                ],
                _ => return Err("uv-split island has no placement".into()),
            };
            if let Some(rows) = &i.st {
                let slot = st
                    .get_mut(v * 8..v * 8 + 8)
                    .ok_or("uv-split needs tangents for every vertex")?;
                slot[..4].copy_from_slice(&pack(&rows[k][..3]));
                slot[4..].copy_from_slice(&pack(&rows[k][3..]));
            }
        }
    }
    lod_edit::write_uv(&mut lod.uv[0], &uv)?;
    lod_edit::set_st(lod, &st)?;
    lod.textures.push(latin1(string(op, "texture")?)?);
    let tex_b = i16::try_from(lod.textures.len() - 1).map_err(|_| "no room for a texture")?;
    let faces = lod.faces.clone();
    let ctx = Ctx {
        faces: &faces,
        old: &old,
        uv: &uv,
        owner: &owner,
    };
    let mut perm: Vec<usize> = (0..faces.len()).collect();
    let mut b_sections: Vec<i32> = Vec::new();
    let mut split: HashSet<i32> = HashSet::new();
    let sheet: HashSet<i16> = lod.selections[target]
        .sections
        .iter()
        .map(|&x| lod.sections.get(x as usize).map(|s| s.texture))
        .collect::<Option<_>>()
        .ok_or("selection names a missing section")?;
    for s in (0..lod.sections.len()).rev() {
        let in_sec = in_section(lod, &lod.sections[s]);
        if !in_sec
            .iter()
            .flat_map(|&f| &faces[f])
            .any(|v| owner.contains_key(&(*v as usize)))
        {
            continue;
        }
        if !sheet.contains(&lod.sections[s].texture) {
            return Err(format!(
                "uv-split plan touches section {s}, whose texture is not one the selection uses"
            ));
        }
        let mut a = Vec::new();
        let mut b = Vec::new();
        for &f in &in_sec {
            if ctx.on_b(f)? { &mut b } else { &mut a }.push(f);
        }
        let aot = *lod.sections[s]
            .area_over_tex
            .first()
            .ok_or("section has no area over texture")?;
        if !a.is_empty() {
            set_aot(&mut lod.sections[s], ctx.area_over_tex(aot, &a)?)?;
            ctx.wrap(&mut lod.sections[s], &a);
        }
        if b.is_empty() {
            continue;
        }
        for (k, &f) in a.iter().chain(&b).enumerate() {
            perm[in_sec[0] + k] = f;
        }
        let split_at =
            lod.sections[s].face_lower + a.iter().map(|&f| weight(&faces[f])).sum::<i32>();
        let mut sec_b = lod.sections[s].clone();
        sec_b.face_lower = split_at;
        sec_b.texture = tex_b;
        set_aot(&mut sec_b, ctx.area_over_tex(aot, &b)?)?;
        ctx.wrap(&mut sec_b, &b);
        let si = s as i32;
        if a.is_empty() {
            lod.sections[s] = sec_b;
            b_sections.push(si);
            let kept = lod.selections[target]
                .sections
                .iter()
                .copied()
                .filter(|&x| x != si)
                .collect();
            lod_edit::set_selection_sections(&mut lod.selections[target], kept)?;
            continue;
        }
        lod.sections[s].face_upper = split_at;
        lod.sections.insert(s + 1, sec_b);
        for n in 0..lod.selections.len() {
            shift(lod, n, si, (n != target).then_some(si + 1))?;
        }
        for p in lod.proxies.iter_mut().filter(|p| p.section > si) {
            p.section += 1;
        }
        b_sections = b_sections.iter().map(|x| x + 1).chain([si + 1]).collect();
        split = split.iter().map(|x| x + 1).chain([si, si + 1]).collect();
    }
    if b_sections.is_empty() {
        return Err("uv-split plan puts no faces on sheet B".into());
    }
    let mut inv = vec![0i32; perm.len()];
    for (k, &p) in perm.iter().enumerate() {
        inv[p] = k as i32;
    }
    lod.faces = perm.iter().map(|&k| faces[k].clone()).collect();
    for sel in lod.selections.iter_mut().filter(|n| !n.faces.is_empty()) {
        let mut mapped: Vec<i32> = sel.faces.iter().map(|&f| inv[f as usize]).collect();
        mapped.sort_unstable();
        lod_edit::set_selection_faces(sel, mapped)?;
    }
    for i in 0..lod.proxies.len() {
        if split.contains(&lod.proxies[i].section) {
            let sel = lod
                .selections
                .get(lod.proxies[i].selection as usize)
                .ok_or("proxy selection is missing")?;
            let face = sel.faces.first().copied().unwrap_or(-1);
            lod.proxies[i].section = section_of_face(&lod.faces, &lod.sections, face)?;
        }
    }
    b_sections.sort_unstable();
    let selection = lod_edit::new_selection(latin1(new_selection)?, true, &b_sections, &[], &[])?;
    lod.selections.push(selection);
    Ok(())
}

pub fn uv_split(
    data: &[u8],
    op: &Value,
    label: &str,
    base: &std::path::Path,
) -> Result<Vec<u8>, String> {
    edit(data, label, true, |model| {
        let li = lod_index(model, string(op, "lod")?)?;
        let lod = &mut model.lods[li];
        let plan = read(&source_json(op, base)?, lod.vertices.len())?;
        split(lod, op, &plan)?;
        lod_edit::refresh_rest(lod)
    })
}
