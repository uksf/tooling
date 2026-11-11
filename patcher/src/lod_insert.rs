use crate::odol::Model;

const NO_LOD: u8 = 255;
const SHIFTED: [usize; 11] = [0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11];
pub const ROADWAY: (usize, &str) = (9, "Roadway");
pub const HITPOINTS: (usize, &str) = (11, "Hitpoints");

pub fn matches(have: f32, wanted: f32) -> bool {
    have == wanted || (wanted > 1000.0 && (have - wanted).abs() <= wanted * 1e-6)
}

fn bump(v: u8, at: usize) -> u8 {
    if v != NO_LOD && usize::from(v) >= at {
        v + 1
    } else {
        v
    }
}

pub fn insert_clone(
    model: &mut Model,
    source: usize,
    resolution: f32,
    (slot, field): (usize, &str),
) -> Result<usize, String> {
    if model.lods.iter().any(|l| matches(l.resolution, resolution)) {
        return Err(format!("{field} LOD already exists"));
    }
    let at = source + 1;
    let mi = &mut model.info;
    let min_shadow = u8::try_from(mi.min_shadow).map_err(|_| "invalid min shadow LOD")?;
    let specials: Vec<u8> = [mi.geometry_simple, mi.geometry_phys, min_shadow]
        .into_iter()
        .chain(SHIFTED.iter().map(|&i| mi.lod_index[i]))
        .collect();
    if at >= usize::from(NO_LOD)
        || specials
            .iter()
            .any(|&v| v != NO_LOD && usize::from(v) >= at && v + 1 == NO_LOD)
    {
        return Err(format!(
            "inserting a LOD would push a special LOD index to {NO_LOD}, which means no LOD"
        ));
    }
    let lod = model.lods[source].duplicate(resolution)?;
    mi.geometry_simple = bump(mi.geometry_simple, at);
    mi.geometry_phys = bump(mi.geometry_phys, at);
    mi.min_shadow = u32::from(bump(min_shadow, at));
    for i in SHIFTED {
        mi.lod_index[i] = bump(mi.lod_index[i], at);
    }
    mi.lod_index[slot] = at as u8;
    for list in [
        &mut mi.preferred_shadow_volume,
        &mut mi.preferred_shadow_buffer,
        &mut mi.preferred_shadow_buffer_vis,
    ] {
        for x in list.iter_mut() {
            if *x >= at as i32 {
                *x += 1;
            }
        }
        list.insert(at.min(list.len()), -1);
    }
    if let Some(an) = &mut model.animations {
        let missing = "animations do not cover the source LOD";
        let row = an.bones_to_anims.get(source).ok_or(missing)?.clone();
        an.bones_to_anims.insert(at, row);
        let row = an.anims_to_bones.get(source).ok_or(missing)?.clone();
        an.anims_to_bones.insert(at, row);
        let row = an.axes.get(source).ok_or(missing)?.clone();
        an.axes.insert(at, row);
    }
    let permanent = model.permanent[source];
    let loadable = model.loadable[source].clone();
    model.insert_lod(at, lod, permanent, loadable);
    Ok(at)
}
