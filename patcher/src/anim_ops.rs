use crate::{
    odol::Animation,
    op_json::{edit, float, floats, latin1, optional_string, same, string},
};
use serde_json::Value;

const ADDRESSES: [&str; 3] = ["clamp", "mirror", "loop"];
const TRANSLATION: u32 = 4;

fn address(name: &str) -> Result<u32, String> {
    ADDRESSES
        .iter()
        .position(|a| a.eq_ignore_ascii_case(name))
        .map(|i| i as u32)
        .ok_or_else(|| format!("address must be one of {}", ADDRESSES.join(", ")))
}

pub fn set_animation_source(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let name = string(op, "animation")?;
        let source = latin1(string(op, "to")?)?;
        let index = optional_string(op, "address")?.map(address).transpose()?;
        let mut hits = 0;
        for a in model
            .animations
            .iter_mut()
            .flat_map(|a| &mut a.classes)
            .filter(|a| same(&a.name, name))
        {
            a.source.clone_from(&source);
            if let Some(index) = index {
                a.source_address = index;
            }
            hits += 1;
        }
        if hits == 0 {
            return Err(format!("model has no animation '{name}'"));
        }
        Ok(())
    })
}

pub fn add_translation(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    edit(data, label, false, |model| {
        let name = string(op, "animation")?;
        let bone = string(op, "bone")?;
        let axis = floats(op, "axis", 3)?;
        let source_address = address(optional_string(op, "address")?.unwrap_or("clamp"))?;
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
        let min_value = float(op, "minValue")?;
        let max_value = float(op, "maxValue")?;
        let index = u32::try_from(animations.classes.len()).map_err(|_| "too many animations")?;
        animations.classes.push(Animation {
            kind: TRANSLATION,
            name: latin1(name)?,
            source: latin1(string(op, "animationSource")?)?,
            min_phase: min_value,
            max_phase: max_value,
            min_value,
            max_value,
            period: 0.0,
            init_phase: 0.0,
            source_address,
            angle0: 0.0,
            angle1: 0.0,
            offset0: float(op, "offset0")?,
            offset1: float(op, "offset1")?,
            axis_pos: [0.0; 3],
            axis_dir: [0.0; 3],
            angle: 0.0,
            axis_offset: 0.0,
            hide: 0.0,
            unused55: 0.0,
        });
        for j in 0..animations.bones_to_anims.len() {
            animations.bones_to_anims[j]
                .get_mut(skel)
                .ok_or("bone is outside the animation table")?
                .push(index);
            animations.anims_to_bones[j].push(skel as i32);
            animations.axes[j].push(Some([[0.0; 3], [axis[0], axis[1], axis[2]]]));
        }
        Ok(())
    })
}
