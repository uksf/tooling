use crate::{odol::Model, pbo::Pbo, state::sha256};
use std::fs;

const SPECIAL: &[(&str, f32)] = &[
    ("geometry", 1e13),
    ("geometrySimple", 2e13),
    ("geometryPhys", 4e13),
    ("geometryPhysOld", 3e13),
    ("viewPilot", 1100.0),
    ("memory", 1e15),
    ("landContact", 2e15),
    ("roadway", 3e15),
    ("paths", 4e15),
    ("hitpoints", 5e15),
    ("geometryView", 6e15),
    ("geometryFire", 7e15),
    ("viewGeometryCargo", 8e15),
    ("viewGeometryPilot", 1.3e16),
    ("viewGeometryGunner", 1.5e16),
];
fn matches(a: f32, b: f32) -> bool {
    a == b || (b > 1000.0 && (a - b).abs() <= b * 1e-6)
}
fn text(bytes: &[u8]) -> String {
    bytes.iter().map(|b| char::from(*b)).collect()
}
fn boolean(b: bool) -> &'static str {
    if b { "True" } else { "False" }
}

pub fn run(path: &str, asset: &str, lod_name: Option<&str>) -> Result<u8, String> {
    let pbo = Pbo::parse(fs::read(path).map_err(|e| format!("{path}: {e}"))?)?;
    let name = asset
        .chars()
        .map(|c| u8::try_from(c as u32))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| format!("{asset} is not in {path}"))?;
    let index = pbo
        .find(&name)?
        .ok_or_else(|| format!("{asset} is not in {path}"))?;
    let bytes = pbo
        .data(index)
        .map_err(|e| format!("{path}:{asset}: cannot read the asset: {e}"))?;
    let model = Model::parse(&bytes).map_err(|e| format!("{path}:{asset}: {e}"))?;
    let identical = model.write()? == bytes;
    println!(
        "ODOL v{}, {} LODs, {} bytes, sha256 {}, re-serialises identically: {}",
        model.version,
        model.lods.len(),
        bytes.len(),
        sha256(&bytes),
        boolean(identical)
    );
    let skeleton = &model.info.skeleton;
    println!(
        "skeleton '{}': {} bones",
        text(&skeleton.name),
        skeleton.bones.len()
    );
    for bone in &skeleton.bones {
        println!("  bone {} <- {}", text(&bone.name), text(&bone.parent));
    }
    if let Some(name) = lod_name {
        let resolution = crate::lod_names::resolution(name)?;
        let mut found = model
            .lods
            .iter()
            .filter(|l| matches(l.resolution, resolution));
        let lod = found
            .next()
            .ok_or_else(|| format!("LOD '{name}' not found"))?;
        if found.next().is_some() {
            return Err(format!("LOD '{name}' is ambiguous"));
        }
        let face_bytes: usize = lod
            .faces
            .iter()
            .map(|face| 1 + face.len() * if model.version >= 69 { 4 } else { 2 })
            .sum();
        println!(
            "faces {}, face bytes {face_bytes}, polygons unused {}/{}, face area {}",
            lod.faces.len(),
            lod.face_bytes,
            lod.face_flags,
            crate::dotnet_fmt::float(lod.face_area)
        );
        for section in &lod.sections {
            println!(
                "section {}..{} material {} texture {}",
                section.face_lower, section.face_upper, section.material, section.texture
            );
        }
        for selection in &lod.selections {
            let sections = selection
                .sections
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",");
            println!(
                "selection {}: {} faces, {} vertices, {} weights, sectional {}, sections [{sections}]",
                text(&selection.name),
                selection.faces.len(),
                selection.vertices.len(),
                selection.weights.len(),
                boolean(selection.is_sectional != 0)
            );
        }
    } else {
        for lod in &model.lods {
            let name = SPECIAL
                .iter()
                .find(|(_, r)| matches(lod.resolution, *r))
                .map(|(name, _)| (*name).to_owned())
                .unwrap_or_else(|| crate::dotnet_fmt::general(lod.resolution, 6));
            let components = lod
                .selections
                .iter()
                .filter(|s| text(&s.name).to_lowercase().starts_with("component"))
                .count();
            println!(
                "lod {name} ({}): {} vertices, {} faces, {} sections, {} selections ({components} components)",
                crate::dotnet_fmt::general(lod.resolution, 9),
                lod.vertices.len(),
                lod.faces.len(),
                lod.sections.len(),
                lod.selections.len()
            );
        }
    }
    Ok(0)
}
