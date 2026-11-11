use crate::{
    axis_common::lod_index,
    odol::{Animation, edit_arrays},
    op_json::{edit, float, floats, latin1_lossy, same, string},
    section_faces::selection,
};
use serde_json::Value;

const HIDE: u32 = 9;

pub fn add_hide(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let name = string(op, "animation")?;
        let bone = string(op, "bone")?;
        let animations = model.animations.as_mut().ok_or("model has no animations")?;
        if animations.classes.iter().any(|a| same(&a.name, name)) {
            return Err(format!("model already has an animation '{name}'"));
        }
        let skel = model
            .info
            .skeleton
            .bones
            .iter()
            .position(|b| same(&b.name, bone))
            .ok_or_else(|| format!("skeleton has no bone '{bone}'"))?;
        let index = u32::try_from(animations.classes.len()).map_err(|_| "too many animations")?;
        animations.classes.push(Animation {
            kind: HIDE,
            name: latin1_lossy(name),
            source: latin1_lossy(string(op, "animationSource")?),
            min_phase: 0.0,
            max_phase: 1.0,
            min_value: 0.0,
            max_value: 1.0,
            period: 0.0,
            init_phase: 0.0,
            source_address: 0,
            angle0: 0.0,
            angle1: 0.0,
            offset0: 0.0,
            offset1: 0.0,
            axis_pos: [0.0; 3],
            axis_dir: [0.0; 3],
            angle: 0.0,
            axis_offset: 0.0,
            hide: 0.5,
            unused55: -1.0,
        });
        for j in 0..animations.bones_to_anims.len() {
            animations.bones_to_anims[j]
                .get_mut(skel)
                .ok_or("bone is outside the animation table")?
                .push(index);
            animations
                .anims_to_bones
                .get_mut(j)
                .ok_or("animation table is missing a LOD")?
                .push(skel as i32);
            animations
                .axes
                .get_mut(j)
                .ok_or("animation table is missing a LOD")?
                .push(None);
        }
        Ok(())
    })
}

fn rotation(model: &crate::odol::Model, name: &str) -> Result<usize, String> {
    let hits: Vec<usize> = model
        .animations
        .iter()
        .flat_map(|a| a.classes.iter().enumerate())
        .filter(|(_, a)| same(&a.name, name))
        .map(|(i, _)| i)
        .collect();
    match hits.as_slice() {
        [one]
            if model
                .animations
                .as_ref()
                .is_some_and(|a| a.classes[*one].kind <= 3) =>
        {
            Ok(*one)
        }
        _ => Err(format!("expected one rotation animation '{name}'")),
    }
}

pub fn set_rotation_range(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let name = string(op, "animation")?;
        let angle0 = float(op, "angle0")?;
        let angle1 = float(op, "angle1")?;
        let index = rotation(model, name)?;
        let class = &mut model
            .animations
            .as_mut()
            .ok_or("model has no animations")?
            .classes[index];
        class.angle0 = angle0;
        class.angle1 = angle1;
        Ok(())
    })
}

pub fn move_rotation_axis(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let name = string(op, "animation")?;
        let selection_name = string(op, "selection")?;
        let offset = floats(op, "offset", 3)?;
        if model.animations.is_none() {
            return Err("no animations".into());
        }
        let matches: Vec<usize> = model
            .animations
            .iter()
            .flat_map(|a| a.classes.iter().enumerate())
            .filter(|(_, a)| same(&a.name, name) && a.kind <= 3)
            .map(|(i, _)| i)
            .collect();
        let [index] = matches.as_slice() else {
            return Err(format!("expected one rotation animation '{name}'"));
        };
        let index = *index;
        let memory = lod_index(model, "memory")?;
        let memory = &mut model.lods[memory];
        let s = selection(memory, selection_name)?;
        let vertices = memory.selections[s].vertices.clone();
        let shared = memory
            .selections
            .iter()
            .enumerate()
            .any(|(i, o)| i != s && o.vertices.iter().any(|v| vertices.contains(v)));
        if vertices.len() != 2 || shared {
            return Err(format!(
                "axis selection '{selection_name}' must have two unshared points"
            ));
        }
        let mut positions = memory.vertices.clone();
        for &v in &vertices {
            let p = usize::try_from(v)
                .ok()
                .and_then(|v| positions.get_mut(v))
                .ok_or("axis point is outside the memory LOD")?;
            *p = [p[0] + offset[0], p[1] + offset[1], p[2] + offset[2]];
        }
        edit_arrays::set_vertices(memory, positions)?;
        let animations = model.animations.as_mut().ok_or("no animations")?;
        for group in &mut animations.axes {
            let axis = group
                .get_mut(index)
                .and_then(Option::as_mut)
                .ok_or_else(|| format!("invalid axis data for '{name}'"))?;
            axis[0] = [
                axis[0][0] + offset[0],
                axis[0][1] + offset[1],
                axis[0][2] + offset[2],
            ];
        }
        Ok(())
    })
}
