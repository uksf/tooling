use crate::{
    axis_common::lod_index,
    odol::{BoneWeight, Lod, Proxy, Section, edit_arrays::set_vertices, lod_edit},
    op_json::{edit, floats, latin1, same, string, text},
};
use serde_json::Value;

const CLIP_ALL: i32 = 63;
const PROXY_NORMAL: u32 = 525_312;
const PROXY_SECTION_SPECIAL: u32 = 268_443_648;

fn empty_bone() -> BoneWeight {
    BoneWeight {
        count: 0,
        data: [0; 8],
    }
}

fn point(lod: &mut Lod, p: [f32; 3], normal: u32) -> Result<i32, String> {
    lod_edit::append_vertex(lod, p, CLIP_ALL, normal.to_le_bytes(), empty_bone())
}

pub fn sequence(seq: i32) -> String {
    if seq < 0 {
        format!("-{:03}", seq.unsigned_abs())
    } else {
        format!("{seq:03}")
    }
}

pub(crate) fn proxy_section(face_bytes: i32) -> Section {
    Section {
        face_lower: 0,
        face_upper: face_bytes,
        min_bone: 0,
        bones: 0,
        unused: 0,
        texture: -1,
        special: PROXY_SECTION_SPECIAL,
        material: -1,
        material_name: Vec::new(),
        area_over_tex: vec![1.0, -1000.0],
        flag: 0,
        extra: Vec::new(),
    }
}

fn unique(lod: &Lod, name: &str) -> Result<Option<usize>, String> {
    let hits: Vec<_> = (0..lod.selections.len())
        .filter(|&i| same(&lod.selections[i].name, name))
        .collect();
    match hits.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(*one)),
        _ => Err(format!("LOD has several selections named '{name}'")),
    }
}

pub fn add_memory_proxies(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, true, |model| {
        let filter = string(op, "filter")?.to_ascii_lowercase();
        let src = lod_index(model, "0")?;
        let mi = lod_index(model, "memory")?;
        if !model.lods[mi].faces.is_empty() || !model.lods[mi].sections.is_empty() {
            return Err("memory LOD already has faces".into());
        }
        let mut sources: Vec<Proxy> = model.lods[src]
            .proxies
            .iter()
            .filter(|p| text(&p.model).to_ascii_lowercase().contains(&filter))
            .cloned()
            .collect();
        if sources.is_empty() {
            return Err(format!("no LOD 0 proxies match '{filter}'"));
        }
        sources.sort_by_key(|p| p.sequence);
        let mem = &mut model.lods[mi];
        let mut faces = Vec::new();
        let mut proxies = Vec::new();
        for sp in sources {
            let m = sp.transform;
            let pos = [m[9], m[10], m[11]];
            let v0 = point(mem, pos, PROXY_NORMAL)?;
            point(
                mem,
                [pos[0] + m[3], pos[1] + m[4], pos[2] + m[5]],
                PROXY_NORMAL,
            )?;
            let half = [0.5 * m[6], 0.5 * m[7], 0.5 * m[8]];
            point(
                mem,
                [pos[0] + half[0], pos[1] + half[1], pos[2] + half[2]],
                PROXY_NORMAL,
            )?;
            faces.push(vec![v0 + 1, v0, v0 + 2]);
            let name = format!("proxy:{}.{}", text(&sp.model), sequence(sp.sequence));
            let face = i32::try_from(faces.len() - 1).map_err(|_| "too many faces")?;
            mem.selections.push(lod_edit::new_selection(
                latin1(&name)?,
                false,
                &[],
                &[face],
                &[v0, v0 + 1, v0 + 2],
            )?);
            proxies.push(Proxy {
                selection: i32::try_from(mem.selections.len() - 1)
                    .map_err(|_| "too many selections")?,
                bone: -1,
                section: 0,
                ..sp
            });
        }
        let face_bytes = i32::try_from(16 * faces.len()).map_err(|_| "too many faces")?;
        mem.faces = faces;
        mem.face_bytes = face_bytes as u32;
        mem.face_flags = 0;
        mem.proxies = proxies;
        mem.sections = vec![proxy_section(face_bytes)];
        if mem.textures.is_empty() {
            mem.textures = vec![Vec::new()];
        }
        Ok(())
    })
}

pub fn add_memory_point(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let name = string(op, "name")?;
        let p = floats(op, "position", 3)?;
        let mi = lod_index(model, "memory")?;
        let mem = &mut model.lods[mi];
        if mem.selections.iter().any(|s| same(&s.name, name)) {
            return Err(format!("memory LOD already has selection '{name}'"));
        }
        let v = point(mem, [p[0], p[1], p[2]], 0)?;
        let selection = lod_edit::new_selection(latin1(name)?, false, &[], &[], &[v])?;
        mem.selections.push(selection);
        Ok(())
    })
}

pub fn set_memory_point(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let name = string(op, "name")?;
        let p = floats(op, "position", 3)?;
        let mi = lod_index(model, "memory")?;
        let mem = &mut model.lods[mi];
        let index =
            unique(mem, name)?.ok_or_else(|| format!("memory LOD has no selection '{name}'"))?;
        let vertices = &mem.selections[index].vertices;
        let [v] = vertices.as_slice() else {
            return Err(format!(
                "memory selection '{name}' has {} vertices, not one point",
                vertices.len()
            ));
        };
        let v = *v;
        let sharers: Vec<_> = mem
            .selections
            .iter()
            .enumerate()
            .filter(|(i, s)| *i != index && s.vertices.contains(&v))
            .map(|(_, s)| text(&s.name))
            .collect();
        if !sharers.is_empty() {
            return Err(format!(
                "memory point '{name}' is shared with {}",
                sharers.join(", ")
            ));
        }
        let mut positions = mem.vertices.clone();
        *positions
            .get_mut(v as usize)
            .ok_or("memory point is outside the LOD")? = [p[0], p[1], p[2]];
        set_vertices(mem, positions)?;
        lod_edit::refresh_rest(mem)
    })
}
