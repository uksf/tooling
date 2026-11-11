use crate::odol::Model;
use serde_json::Value;
use std::collections::HashSet;

struct Rule {
    from: String,
    id: i32,
    to: String,
    to_id: i32,
}
fn path(value: &Value, key: &str) -> Result<String, String> {
    Ok(value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("op is missing string \"{key}\": {value}"))?
        .replace('/', "\\")
        .to_lowercase())
}
fn id(value: &Value, key: &str) -> Result<i32, String> {
    value
        .get(key)
        .map(|v| {
            v.as_i64()
                .and_then(|n| i32::try_from(n).ok())
                .ok_or_else(|| format!("invalid {key}"))
        })
        .transpose()
        .map(|n| n.unwrap_or(-1))
}
pub fn remap_proxies(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    let run = || -> Result<Vec<u8>, String> {
        let mut model = Model::parse(data)?;
        if model.write()? != data {
            return Err("the ODOL library does not re-serialise this model byte-identically; refusing to patch it".into());
        }
        let list = op
            .get("rules")
            .and_then(Value::as_array)
            .filter(|a| !a.is_empty())
            .ok_or("remap-proxies needs a non-empty \"rules\" array")?;
        let rules = list
            .iter()
            .map(|v| {
                Ok(Rule {
                    from: path(v, "model")?,
                    id: id(v, "id")?,
                    to: path(v, "toModel")?,
                    to_id: id(v, "toId")?,
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let mut keys = HashSet::new();
        for rule in &rules {
            if !keys.insert((&rule.from, rule.id)) {
                return Err(format!(
                    "remap-proxies has multiple rules for {}|{}",
                    rule.from, rule.id
                ));
            }
        }
        let mut used = HashSet::new();
        for lod in &mut model.lods {
            for proxy in &mut lod.proxies {
                if let Some(rule) = rules
                    .iter()
                    .find(|r| crate::op_json::same(&proxy.model, &r.from) && r.id == proxy.sequence)
                {
                    let bytes = rule
                        .to
                        .chars()
                        .map(|c| u8::try_from(c as u32).map_err(|_| "character outside Latin-1"))
                        .collect::<Result<Vec<_>, _>>()?;
                    proxy.model = bytes;
                    proxy.sequence = rule.to_id;
                    let selection = lod
                        .selections
                        .get_mut(
                            usize::try_from(proxy.selection)
                                .map_err(|_| "invalid proxy selection")?,
                        )
                        .ok_or("invalid proxy selection")?;
                    selection.name =
                        format!("proxy:{}.{}", rule.to, format_args!("{:03}", rule.to_id))
                            .chars()
                            .map(|c| {
                                u8::try_from(c as u32).map_err(|_| "character outside Latin-1")
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                    used.insert((&rule.from, rule.id));
                }
            }
        }
        let unused: Vec<_> = rules
            .iter()
            .filter(|r| !used.contains(&(&r.from, r.id)))
            .map(|r| format!("{}|{}", r.from, r.id))
            .collect();
        if !unused.is_empty() {
            return Err(format!("no proxy matches {}", unused.join(", ")));
        }
        let result = model.write()?;
        Model::parse(&result)?;
        Ok(result)
    };
    run().map_err(|e| format!("{label}: {e}"))
}
