use crate::{lod_names::resolution, odol::Model};
use serde_json::Value;
use std::collections::HashSet;

pub fn remove_component(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    let run = || -> Result<Vec<u8>, String> {
        let mut model = Model::parse(data)?;
        if model.write()? != data {
            return Err("the ODOL library does not re-serialise this model byte-identically; refusing to patch it".into());
        }
        let name = op
            .get("lod")
            .and_then(Value::as_str)
            .ok_or("op is missing string \"lod\"")?;
        let wanted = resolution(name)?;
        let matches: Vec<_> = model
            .lods
            .iter()
            .enumerate()
            .filter(|(_, l)| {
                l.resolution == wanted
                    || (wanted > 1000.0 && (l.resolution - wanted).abs() <= wanted * 1e-6)
            })
            .map(|(i, _)| i)
            .collect();
        if matches.len() != 1 {
            return Err(format!("no {name} LOD"));
        }
        let lod = &mut model.lods[matches[0]];
        let names: HashSet<Vec<u8>> = op
            .get("components")
            .and_then(Value::as_array)
            .ok_or("op is missing string array \"components\"")?
            .iter()
            .map(|n| {
                n.as_str()
                    .ok_or("invalid component")
                    .and_then(|s| {
                        s.chars()
                            .map(|c| {
                                u8::try_from(c as u32).map_err(|_| "character outside Latin-1")
                            })
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .map(|v| v.to_ascii_lowercase())
            })
            .collect::<Result<_, _>>()?;
        let required = op
            .get("requireProxyFaces")
            .map(|v| v.as_bool().ok_or("invalid requireProxyFaces"))
            .transpose()?
            .unwrap_or(true);
        let faces: HashSet<i32> = lod
            .selections
            .iter()
            .filter(|s| s.name.to_ascii_lowercase().starts_with(b"proxy:"))
            .flat_map(|s| s.faces.iter().copied())
            .collect();
        for name in &names {
            let selected: Vec<_> = lod
                .selections
                .iter()
                .filter(|s| s.name.eq_ignore_ascii_case(name))
                .collect();
            let text = String::from_utf8_lossy(name);
            if selected.len() != 1 {
                return Err(format!(
                    "LOD {name_lod} has no unique selection '{text}'",
                    name_lod = name_str(op)
                ));
            }
            if !name.starts_with(b"component") {
                return Err(format!("'{text}' is not a component selection"));
            }
            if required && !selected[0].faces.iter().all(|f| faces.contains(f)) {
                return Err(format!(
                    "'{text}' has faces that are not proxy faces; set requireProxyFaces false to remove a real component"
                ));
            }
        }
        let mut indices = Vec::with_capacity(lod.selections.len());
        let mut next = 0;
        for selection in &lod.selections {
            if names.contains(&selection.name.to_ascii_lowercase()) {
                indices.push(None);
            } else {
                indices.push(Some(next));
                next += 1;
            }
        }
        for proxy in &mut lod.proxies {
            let old = usize::try_from(proxy.selection).map_err(|_| "invalid proxy selection")?;
            proxy.selection = indices
                .get(old)
                .ok_or("invalid proxy selection")?
                .ok_or("a proxy's selection cannot be removed")?;
        }
        lod.selections
            .retain(|s| !names.contains(&s.name.to_ascii_lowercase()));
        let result = model.write()?;
        Model::parse(&result)?;
        Ok(result)
    };
    run().map_err(|e| format!("{label}: {e}"))
}
fn name_str(op: &Value) -> &str {
    op.get("lod").and_then(Value::as_str).unwrap_or("")
}
