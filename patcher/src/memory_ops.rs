use crate::{
    lod_names::resolution,
    odol::{self, Model},
};
use serde_json::Value;

fn string<'a>(op: &'a Value, key: &str) -> Result<&'a str, String> {
    op.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("op is missing string \"{key}\": {op}"))
}

pub fn split_memory_point(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    let run = || -> Result<Vec<u8>, String> {
        let mut model = Model::parse(data)?;
        if model.write()? != data {
            return Err("the ODOL library does not re-serialise this model byte-identically; refusing to patch it".into());
        }
        let name = string(op, "name")?;
        let donor_name = string(op, "boneFrom")?;
        let p = crate::op_json::floats(op, "position", 3)?;
        let position = [p[0], p[1], p[2]];
        let wanted = resolution("memory")?;
        let found: Vec<_> = model
            .lods
            .iter()
            .enumerate()
            .filter(|(_, l)| (l.resolution - wanted).abs() <= wanted * 1e-6)
            .map(|(i, _)| i)
            .collect();
        if found.len() != 1 {
            return Err("no memory LOD".into());
        }
        let lod = &mut model.lods[found[0]];
        let point = |name: &str| -> Result<(usize, usize), String> {
            let matches: Vec<_> = lod
                .selections
                .iter()
                .enumerate()
                .filter(|(_, s)| crate::op_json::same(&s.name, name))
                .collect();
            if matches.len() != 1 {
                return Err(format!("memory LOD has no unique selection '{name}'"));
            }
            let (i, s) = matches[0];
            if s.vertices.len() != 1 {
                return Err(format!(
                    "memory selection '{name}' has {} vertices, not one point",
                    s.vertices.len()
                ));
            }
            Ok((
                i,
                usize::try_from(s.vertices[0]).map_err(|_| "invalid vertex")?,
            ))
        };
        let (index, vertex) = point(name)?;
        let (_, donor) = point(donor_name)?;
        if !lod
            .selections
            .iter()
            .enumerate()
            .any(|(i, s)| i != index && s.vertices.contains(&(vertex as i32)))
        {
            return Err(format!(
                "memory point '{name}' is not shared; use set-memory-point"
            ));
        }
        odol::edit_arrays::append_memory(lod, donor, position, index)?;
        let result = model.write()?;
        Model::parse(&result)?;
        Ok(result)
    };
    run().map_err(|e| format!("{label}: {e}"))
}
