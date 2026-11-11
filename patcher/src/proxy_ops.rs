use crate::{
    lod_names::resolution,
    odol::{Lod, Model, Proxy},
};
use serde_json::Value;

fn string<'a>(op: &'a Value, key: &str) -> Result<&'a str, String> {
    op.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("op is missing string \"{key}\": {op}"))
}
fn same(a: &[u8], b: &[u8]) -> bool {
    crate::op_json::same(a, &crate::op_json::text(b))
}
fn index(model: &Model, name: &str) -> Result<usize, String> {
    let n = resolution(name)?;
    let matches: Vec<_> = model
        .lods
        .iter()
        .enumerate()
        .filter(|(_, l)| l.resolution == n || (n > 1000.0 && (l.resolution - n).abs() <= n * 1e-6))
        .collect();
    if matches.len() != 1 {
        return Err(format!("no {name} LOD"));
    }
    Ok(matches[0].0)
}
fn texture(lod: &Lod, section: i32) -> Result<&[u8], String> {
    let s = lod
        .sections
        .get(usize::try_from(section).map_err(|_| "invalid section")?)
        .ok_or("invalid section")?;
    if s.texture < 0 {
        Ok(b"")
    } else {
        Ok(lod
            .textures
            .get(s.texture as usize)
            .ok_or("invalid texture")?)
    }
}
fn key_eq(a: &Proxy, b: &Proxy) -> bool {
    a.sequence == b.sequence && same(&a.model, &b.model)
}
fn restore(model: &mut Model, op: &Value) -> Result<(), String> {
    let filter = string(op, "filter")?;
    let from_name = string(op, "from")?;
    let from = index(model, from_name)?;
    let sources: Vec<_> = model.lods[from]
        .proxies
        .iter()
        .filter(|p| crate::op_json::contains(&p.model, filter))
        .cloned()
        .collect();
    if sources.is_empty() {
        return Err(format!("no LOD {from_name} proxies match '{filter}'"));
    }
    let targets = op
        .get("lods")
        .and_then(Value::as_array)
        .ok_or("op is missing string array \"lods\"")?;
    for target in targets {
        let name = target.as_str().ok_or("invalid lod name")?;
        let dest = index(model, name)?;
        let missing: Vec<_> = sources
            .iter()
            .filter(|p| !model.lods[dest].proxies.iter().any(|q| key_eq(p, q)))
            .collect();
        if missing.is_empty() {
            return Err(format!("LOD {name} already has every matching proxy"));
        }
        for source in missing {
            let wanted = [
                &b"proxy:"[..],
                &source.model,
                format!(".{}", crate::dotnet_fmt::pad3(source.sequence)).as_bytes(),
            ]
            .concat();
            let selection = model.lods[dest]
                .selections
                .iter()
                .position(|s| same(&s.name, &wanted))
                .ok_or_else(|| {
                    format!(
                        "LOD {name} has no selection '{}'",
                        crate::op_json::text(&wanted)
                    )
                })?;
            let section = if let Some(sibling) = model.lods[dest]
                .proxies
                .iter()
                .find(|p| same(&p.model, &source.model))
            {
                sibling.section
            } else {
                let target_texture = texture(&model.lods[from], source.section)?;
                let mut found = None;
                for i in 0..model.lods[dest].sections.len() as i32 {
                    if same(texture(&model.lods[dest], i)?, target_texture) {
                        found = Some(i);
                        break;
                    }
                }
                found.ok_or_else(|| {
                    format!(
                        "LOD {name} has no section textured '{}'",
                        String::from_utf8_lossy(target_texture)
                    )
                })?
            };
            let mut proxy = source.clone();
            proxy.selection = i32::try_from(selection).map_err(|_| "selection index overflow")?;
            proxy.section = section;
            model.lods[dest].proxies.push(proxy);
        }
    }
    Ok(())
}
pub fn restore_lod_proxies(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    let run = || -> Result<Vec<u8>, String> {
        let mut model = Model::parse(data)?;
        if model.write()? != data {
            return Err("the ODOL library does not re-serialise this model byte-identically; refusing to patch it".into());
        }
        restore(&mut model, op)?;
        let result = model.write()?;
        Model::parse(&result)?;
        Ok(result)
    };
    run().map_err(|e| format!("{label}: {e}"))
}
