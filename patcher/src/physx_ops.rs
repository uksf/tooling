use crate::op_json::{edit, float};
use serde_json::Value;

const GEOMETRY: f32 = 1e13;
const PHYSX: f32 = 4e13;
const NO_LOD: u8 = 255;
const SHIFTED: [usize; 11] = [0, 1, 2, 3, 4, 5, 7, 8, 9, 10, 11];
const GEOMETRY_SLOT: usize = 1;

fn bump(v: u8, at: usize) -> u8 {
    if v != NO_LOD && usize::from(v) >= at {
        v + 1
    } else {
        v
    }
}

pub fn add_physx(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let mass = float(op, "mass")?;
        if mass <= 0.0 || !(1.0 / mass).is_finite() {
            return Err("add-physx mass must be a positive number".into());
        }
        if model.lods.iter().any(|l| l.resolution == PHYSX) {
            return Err("model already has a PhysX LOD".into());
        }
        let g = model
            .lods
            .iter()
            .position(|l| l.resolution == GEOMETRY)
            .ok_or("model has no geometry LOD")?;
        let at = g + 1;
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
                "add-physx would push a special LOD index to {NO_LOD}, which means no LOD"
            ));
        }
        mi.geometry_simple = bump(mi.geometry_simple, at);
        mi.geometry_phys = bump(mi.geometry_phys, at);
        mi.min_shadow = u32::from(bump(min_shadow, at));
        for i in SHIFTED {
            mi.lod_index[i] = bump(mi.lod_index[i], at);
        }
        mi.lod_index[GEOMETRY_SLOT] = at as u8;
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
        let old = mi.total_mass;
        mi.total_mass = mass;
        mi.inv_mass = 1.0 / mass;
        if old > 0.0 {
            let scaled = mi.inv_inertia.map(|v| v.map(|x| x * old / mass));
            if scaled.iter().flatten().any(|x| !x.is_finite()) {
                return Err(format!(
                    "add-physx mass {} gives a non-finite inverse inertia",
                    crate::dotnet_fmt::float(mass)
                ));
            }
            mi.inv_inertia = scaled;
        }
        if let Some(an) = &mut model.animations {
            let missing = "animations do not cover the geometry LOD";
            let row = an.bones_to_anims.get(g).ok_or(missing)?.clone();
            an.bones_to_anims.insert(at, row);
            let row = an.anims_to_bones.get(g).ok_or(missing)?.clone();
            an.anims_to_bones.insert(at, row);
            let row = an.axes.get(g).ok_or(missing)?.clone();
            an.axes.insert(at, row);
        }
        let lod = model.lods[g].duplicate(PHYSX)?;
        let permanent = model.permanent[g];
        let loadable = model.loadable[g].clone();
        model.insert_lod(at, lod, permanent, loadable);
        Ok(())
    })
}
