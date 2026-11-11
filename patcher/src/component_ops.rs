use crate::{
    lod_names::resolution,
    odol::Model,
    op_json::{same, strings},
};
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
        let names = strings(op, "components")?;
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
                .filter(|s| same(&s.name, name))
                .collect();
            if selected.len() != 1 {
                return Err(format!(
                    "LOD {name_lod} has no unique selection '{name}'",
                    name_lod = name_str(op)
                ));
            }
            if !name.to_lowercase().starts_with("component") {
                return Err(format!("'{name}' is not a component selection"));
            }
            if required && !selected[0].faces.iter().all(|f| faces.contains(f)) {
                return Err(format!(
                    "'{name}' has faces that are not proxy faces; set requireProxyFaces false to remove a real component"
                ));
            }
        }
        let mut indices = Vec::with_capacity(lod.selections.len());
        let mut next = 0;
        for selection in &lod.selections {
            if names.iter().any(|n| same(&selection.name, n)) {
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
            .retain(|s| !names.iter().any(|n| same(&s.name, n)));
        let result = model.write()?;
        Model::parse(&result)?;
        Ok(result)
    };
    run().map_err(|e| format!("{label}: {e}"))
}
fn name_str(op: &Value) -> &str {
    op.get("lod").and_then(Value::as_str).unwrap_or("")
}
