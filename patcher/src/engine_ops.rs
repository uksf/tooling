use crate::{
    bone_ops, component_ops, file_ops, manifest::Manifest, mlod_ops, model_ops, pbo::Pbo,
    proxy_ops, remap_ops,
};
use serde_json::Value;
use std::fs;

pub const MODEL_OPS: &[&str] = &[
    "rtm-strip-bones",
    "set-bone-parent",
    "set-skeleton-name",
    "strip-animations",
    "add-hide",
    "remove-component",
    "convex-hull-component",
    "remove-degenerate-faces",
    "set-animation-source",
    "set-rotation-range",
    "move-rotation-axis",
    "set-rotation-origin",
    "offset-uv",
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
    "add-selection-points",
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
    "isolate-memory-selection",
    "add-selection-lod",
    "add-points",
    "add-label-quads",
    "add-reverse-face",
    "rebuild-shadow-volume",
];

pub fn latin1_strict(s: &str) -> Result<Vec<u8>, String> {
    s.chars()
        .map(|c| u8::try_from(c as u32).map_err(|_| "character outside Latin-1".to_owned()))
        .collect()
}

fn lookup_path(s: &str) -> Option<Vec<u8>> {
    s.chars()
        .map(|c| {
            let mut lower = c.to_lowercase();
            let single = match (u8::try_from(c as u32), lower.next(), lower.next()) {
                (Ok(b), _, _) => return Some(b),
                (_, Some(l), None) => l,
                _ => return None,
            };
            u8::try_from(single as u32).ok()
        })
        .collect()
}

pub fn find_source(pbo: &Pbo, label: &str, source: &str) -> Result<Option<usize>, String> {
    lookup_path(source).map_or(Ok(None), |bytes| {
        pbo.find(&bytes)
            .map_err(|e| format!("{label}: cannot parse: {e}"))
    })
}

pub fn op_string<'a>(op: &'a Value, name: &str) -> Result<&'a str, String> {
    op.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("op is missing string \"{name}\": {op}"))
}

pub fn base(manifest: &Manifest) -> &std::path::Path {
    manifest.file.parent().unwrap_or(std::path::Path::new(""))
}

pub fn apply_op(
    data: &[u8],
    op: &Value,
    manifest: &Manifest,
    label: &str,
) -> Result<Vec<u8>, String> {
    let name = op_string(op, "op")?;
    match name {
        "text-replace" => {
            let find = op_string(op, "find")?;
            let replace = op_string(op, "replace")?;
            let count = crate::op_json::int(op, "count", 1)?;
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
        "add-hide" => crate::anim_edit_ops::add_hide(data, op, label),
        "set-rotation-range" => crate::anim_edit_ops::set_rotation_range(data, op, label),
        "move-rotation-axis" => crate::anim_edit_ops::move_rotation_axis(data, op, label),
        "set-rotation-origin" => crate::anim_edit_ops::set_rotation_origin(data, op, label),
        "offset-uv" => crate::uv_ops::offset_uv(data, op, label),
        "add-points" => crate::add_points_ops::add_points(data, op, label),
        "isolate-memory-selection" => crate::point_ops::isolate_memory_selection(data, op, label),
        "add-selection-points" => crate::point_ops::add_selection_points(data, op, label),
        "add-selection-lod" => crate::selection_lod_ops::add_selection_lod(data, op, label),
        "convex-hull-component" => crate::hull_ops::convex_hull_component(data, op, label),
        "add-reverse-face" => crate::reverse_face_ops::add_reverse_face(data, op, label),
        "add-label-quads" => crate::label_quads::add_label_quads(data, op, label),
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
