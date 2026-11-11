use crate::{
    bone_ops, component_ops, file_ops,
    manifest::Manifest,
    mlod_ops, model_ops,
    pbo::{Addition, Pbo, normalise},
    proxy_ops, remap_ops,
    state::sha256,
};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
};

const MODEL_OPS: &[&str] = &[
    "rtm-strip-bones",
    "set-bone-parent",
    "set-skeleton-name",
    "strip-animations",
    "remove-component",
    "convex-hull-component",
    "remove-degenerate-faces",
    "set-animation-source",
    "bind-selection",
    "translate-selection",
    "rebind-vertices",
    "add-translation",
    "remap-proxies",
    "add-memory-proxies",
    "restore-lod-proxies",
    "add-memory-point",
    "set-memory-point",
    "split-memory-point",
    "copy-selection-points",
    "split-selection",
    "cut-seams",
    "uv-split",
    "move-sections",
    "retexture",
    "set-section-texture",
    "add-physx",
    "stretch-along-axis",
    "remove-faces",
    "add-mesh",
    "rebuild-shadow-volume",
];

fn latin1(s: &str) -> Result<Vec<u8>, String> {
    s.chars()
        .map(|c| u8::try_from(c as u32).map_err(|_| "character outside Latin-1".to_owned()))
        .collect()
}

fn op_string<'a>(op: &'a Value, name: &str) -> Result<&'a str, String> {
    op.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("op is missing string \"{name}\": {op}"))
}

fn base(manifest: &Manifest) -> &std::path::Path {
    manifest.file.parent().unwrap_or(std::path::Path::new(""))
}

fn apply_op(data: &[u8], op: &Value, manifest: &Manifest, label: &str) -> Result<Vec<u8>, String> {
    let name = op_string(op, "op")?;
    match name {
        "text-replace" => {
            let find = op_string(op, "find")?;
            let replace = op_string(op, "replace")?;
            let count = op
                .get("count")
                .map(|v| {
                    v.as_u64()
                        .ok_or("text-replace count must be a non-negative integer")
                })
                .transpose()?
                .unwrap_or(1);
            file_ops::text_replace(
                data,
                find,
                replace,
                usize::try_from(count).map_err(|e| e.to_string())?,
            )
            .map_err(|e| format!("{label}: {e}"))
        }
        "replace-file" => {
            let source = op_string(op, "source")?;
            fs::read(
                manifest
                    .file
                    .parent()
                    .unwrap_or(std::path::Path::new(""))
                    .join(source),
            )
            .map_err(|_| format!("{label}: replace-file source '{source}' does not exist"))
        }
        "add-mesh" => crate::mesh_ops::add(
            data,
            op,
            label,
            manifest.file.parent().unwrap_or(std::path::Path::new("")),
        ),
        "rebuild-shadow-volume" => crate::shadow_ops::rebuild(data, op, label),
        "stretch-along-axis" => crate::stretch_ops::stretch(data, op, label),
        "remove-faces" => crate::face_ops::remove_faces(data, op, label),
        "split-memory-point" => crate::memory_ops::split_memory_point(data, op, label),
        "set-skeleton-name" => model_ops::set_skeleton_name(data, op, label),
        "set-bone-parent" => bone_ops::set_bone_parent(data, op, label),
        "remove-component" => component_ops::remove_component(data, op, label),
        "remove-degenerate-faces" => mlod_ops::remove_degenerate_faces(data, op, label),
        "restore-lod-proxies" => proxy_ops::restore_lod_proxies(data, op, label),
        "remap-proxies" => remap_ops::remap_proxies(data, op, label),
        "set-animation-source" => crate::anim_ops::set_animation_source(data, op, label),
        "add-translation" => crate::anim_ops::add_translation(data, op, label),
        "bind-selection" => crate::bind_ops::bind_selection(data, op, label),
        "rebind-vertices" => crate::bind_ops::rebind_vertices(data, op, label),
        "translate-selection" => crate::bind_ops::translate_selection(data, op, label),
        "add-memory-proxies" => crate::memory_edit_ops::add_memory_proxies(data, op, label),
        "add-memory-point" => crate::memory_edit_ops::add_memory_point(data, op, label),
        "set-memory-point" => crate::memory_edit_ops::set_memory_point(data, op, label),
        "copy-selection-points" => crate::selection_ops::copy_selection_points(data, op, label),
        "split-selection" => crate::selection_ops::split_selection(data, op, label),
        "move-sections" => crate::selection_ops::move_sections(data, op, label),
        "retexture" => crate::selection_ops::retexture(data, op, label),
        "set-section-texture" => crate::selection_ops::set_section_texture(data, op, label),
        "add-physx" => crate::physx_ops::add_physx(data, op, label),
        "cut-seams" => crate::cut_seams::cut_seams(data, op, label, base(manifest)),
        "uv-split" => crate::uv_split::uv_split(data, op, label, base(manifest)),
        "strip-animations" => crate::odol::strip::strip_animations(data, op, label),
        name if MODEL_OPS.contains(&name) => Err(format!("{label}: op not implemented: '{name}'")),
        _ => Err(format!(
            "{label}: unknown op '{name}' (known: replace-file, text-replace, {})",
            MODEL_OPS.join(", ")
        )),
    }
}

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
        let source_bytes = latin1(source).map_err(|e| format!("{label}: {e}"))?;
        let index = pbo
            .find(&source_bytes)
            .map_err(|e| format!("{label}: cannot parse: {e}"))?
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
        let source_bytes = latin1(source).map_err(|e| format!("{label}: {e}"))?;
        let index = pbo
            .find(&source_bytes)
            .map_err(|e| format!("{label}: cannot parse: {e}"))?
            .ok_or_else(|| format!("{}: asset '{source}' is not in the PBO", manifest.pbo))?;
        let mut target = Vec::new();
        if asset.copy_of.is_some() {
            target =
                pbo.entry_name(&latin1(&asset.path).map_err(|e| {
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
