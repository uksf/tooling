use crate::{
    engine_ops::{apply_op, find_source, latin1_strict},
    manifest::Manifest,
    pbo::{Addition, Pbo, normalise},
    state::sha256,
};
use std::collections::{HashMap, HashSet};

pub fn patch(pristine: Vec<u8>, manifest: &Manifest) -> Result<Vec<u8>, String> {
    let pbo = Pbo::parse(pristine).map_err(|e| format!("{}: cannot parse: {e}", manifest.pbo))?;
    if !pbo.trailer_valid() {
        return Err(format!(
            "{}: the SHA-1 trailer is invalid; the PBO is damaged",
            manifest.pbo
        ));
    }
    let mut pinned = HashMap::new();
    for asset in &manifest.assets {
        let label = format!("{}:{}", manifest.pbo, asset.path);
        let source = asset.copy_of.as_deref().unwrap_or(&asset.path);
        let index = find_source(&pbo, &label, source)?
            .ok_or_else(|| format!("{}: asset '{source}' is not in the PBO", manifest.pbo))?;
        if let std::collections::hash_map::Entry::Vacant(entry) = pinned.entry(index) {
            let data = pbo
                .data(index)
                .map_err(|e| format!("{label}: cannot read the asset: {e}"))?;
            entry.insert(data);
        }
        let actual = sha256(&pinned[&index]);
        if actual != asset.sha {
            return Err(format!(
                "{}: asset '{source}' does not match its pin: expected sha256 {}, actual {actual}. Upstream changed the asset; review and re-pin the patch.",
                manifest.pbo, asset.sha
            ));
        }
    }
    let mut replacements = HashMap::new();
    let mut additions: Vec<Addition> = Vec::new();
    let mut removals = HashSet::new();
    for asset in &manifest.assets {
        let label = format!("{}:{}", manifest.pbo, asset.path);
        let source = asset.copy_of.as_deref().unwrap_or(&asset.path);
        let index = find_source(&pbo, &label, source)?
            .ok_or_else(|| format!("{}: asset '{source}' is not in the PBO", manifest.pbo))?;
        let mut target = Vec::new();
        if asset.copy_of.is_some() {
            target =
                pbo.entry_name(&latin1_strict(&asset.path).map_err(|e| {
                    format!("{label}: copy target is not a valid PBO entry name: {e}")
                })?);
            if target.is_empty()
                || target.iter().any(|b| *b < 32)
                || target.split(|b| *b == b'\\').any(|part| part.is_empty())
            {
                return Err(format!(
                    "{label}: copy target is not a valid PBO entry name"
                ));
            }
            if pbo
                .find(&target)
                .map_err(|e| format!("{label}: cannot parse: {e}"))?
                .is_some()
                || additions
                    .iter()
                    .any(|a| normalise(&a.name) == normalise(&target))
            {
                return Err(format!(
                    "{}: copy target '{}' already exists",
                    manifest.pbo, asset.path
                ));
            }
        } else if replacements.contains_key(&index) || removals.contains(&index) {
            return Err(format!(
                "{}: asset '{}' is listed twice",
                manifest.pbo, asset.path
            ));
        }
        let mut data = pinned[&index].clone();
        if asset.remove {
            removals.insert(index);
            continue;
        }
        for op in &asset.ops {
            data = apply_op(&data, op, manifest, &label)?;
        }
        if asset.copy_of.is_some() {
            additions.push(Addition {
                name: target,
                timestamp: pbo.entries[index].timestamp,
                data,
            });
        } else {
            replacements.insert(index, data);
        }
    }
    let output = pbo
        .rebuild(&replacements, &additions, &removals)
        .map_err(|e| format!("{}: cannot parse: {e}", manifest.pbo))?;
    if !additions.is_empty() || !removals.is_empty() {
        let after = Pbo::parse(output.clone()).map_err(|e| {
            format!(
                "{} after adding or removing entries: cannot parse: {e}",
                manifest.pbo
            )
        })?;
        if after.entries.len() != pbo.entries.len() + additions.len() - removals.len() {
            return Err(format!(
                "{}: rebuilt PBO has an incorrect entry count",
                manifest.pbo
            ));
        }
        for (index, entry) in pbo.entries.iter().enumerate() {
            let found = after.find(&entry.name)?;
            if found.is_some() == removals.contains(&index) {
                return Err(format!(
                    "{}: rebuilt PBO has incorrect entries",
                    manifest.pbo
                ));
            }
        }
        for addition in &additions {
            let index = after
                .find(&addition.name)?
                .ok_or_else(|| format!("{}: rebuilt PBO lost an added entry", manifest.pbo))?;
            if after.data(index)? != addition.data {
                return Err(format!(
                    "{}: rebuilt PBO changed an added entry",
                    manifest.pbo
                ));
            }
        }
    }
    Ok(output)
}
