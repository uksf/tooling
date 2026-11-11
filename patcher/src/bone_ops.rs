use crate::odol::Model;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

fn string<'a>(op: &'a Value, field: &str) -> Result<&'a str, String> {
    op.get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("op is missing string \"{field}\": {op}"))
}
pub fn set_bone_parent(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    let run = || -> Result<Vec<u8>, String> {
        let mut model = Model::parse(data)?;
        if model.write()? != data {
            return Err("the ODOL library does not re-serialise this model byte-identically; refusing to patch it".into());
        }
        let name = string(op, "bone")?;
        let from = string(op, "from")?;
        let to = string(op, "to")?;
        let to_bytes = crate::op_json::latin1_lossy(to);
        let bones = &mut model.info.skeleton.bones;
        let matches: Vec<_> = bones
            .iter()
            .enumerate()
            .filter(|(_, b)| crate::op_json::same(&b.name, name))
            .map(|(i, _)| i)
            .collect();
        if matches.len() != 1 {
            return Err(format!("skeleton has no unique bone '{name}'"));
        }
        let index = matches[0];
        if !crate::op_json::same(&bones[index].parent, from) {
            return Err(format!(
                "bone '{name}' has parent '{}', the patch expects '{from}'",
                String::from_utf8_lossy(&bones[index].parent)
            ));
        }
        if !to.is_empty() && !bones.iter().any(|b| crate::op_json::same(&b.name, to)) {
            return Err(format!("new parent '{to}' is not a bone of the skeleton"));
        }
        bones[index].parent = to_bytes;
        let mut parents = HashMap::new();
        for bone in bones.iter() {
            if parents
                .insert(
                    crate::op_json::text(&bone.name).to_lowercase(),
                    crate::op_json::text(&bone.parent).to_lowercase(),
                )
                .is_some()
            {
                return Err("skeleton has duplicate bones".into());
            }
        }
        for start in parents.keys() {
            let mut seen = HashSet::new();
            let mut current = start;
            while !current.is_empty() {
                if !seen.insert(current) {
                    return Err(format!("the new parent makes a cycle through '{current}'"));
                }
                match parents.get(current) {
                    Some(next) => current = next,
                    None => break,
                }
            }
        }
        let result = model.write()?;
        Model::parse(&result)?;
        Ok(result)
    };
    run().map_err(|e| format!("{label}: {e}"))
}
