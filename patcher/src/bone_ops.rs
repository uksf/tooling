use crate::odol::Model;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

fn string<'a>(op: &'a Value, field: &str) -> Result<&'a str, String> {
    op.get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("op is missing string \"{field}\": {op}"))
}
fn latin(s: &str) -> Result<Vec<u8>, String> {
    s.chars()
        .map(|c| u8::try_from(c as u32).map_err(|_| "character outside Latin-1".into()))
        .collect()
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
        let name_bytes = latin(name)?;
        let from_bytes = latin(from)?;
        let to_bytes = latin(to)?;
        let bones = &mut model.info.skeleton.bones;
        let matches: Vec<_> = bones
            .iter()
            .enumerate()
            .filter(|(_, b)| b.name.eq_ignore_ascii_case(&name_bytes))
            .map(|(i, _)| i)
            .collect();
        if matches.len() != 1 {
            return Err(format!("skeleton has no unique bone '{name}'"));
        }
        let index = matches[0];
        if !bones[index].parent.eq_ignore_ascii_case(&from_bytes) {
            return Err(format!(
                "bone '{name}' has parent '{}', the patch expects '{from}'",
                String::from_utf8_lossy(&bones[index].parent)
            ));
        }
        if !to.is_empty() && !bones.iter().any(|b| b.name.eq_ignore_ascii_case(&to_bytes)) {
            return Err(format!("new parent '{to}' is not a bone of the skeleton"));
        }
        bones[index].parent = to_bytes;
        let mut parents = HashMap::new();
        for bone in bones.iter() {
            if parents
                .insert(
                    bone.name.to_ascii_lowercase(),
                    bone.parent.to_ascii_lowercase(),
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
                    return Err(format!(
                        "the new parent makes a cycle through '{}'",
                        String::from_utf8_lossy(current)
                    ));
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
