use super::{Animations, Header, Model, ModelInfo, io::Reader};
use serde_json::Value;

pub fn strip_animations(data: &[u8], op: &Value, label: &str) -> Result<Vec<u8>, String> {
    let run = || -> Result<Vec<u8>, String> {
        let expected = crate::op_json::strings(op, "sources")?;
        if expected.is_empty() {
            return Err("strip-animations \"sources\" is empty".into());
        }
        let header = Header::parse(data)?;
        let lods = header.resolutions.len();
        let mut r = Reader::new(data);
        r.pos = header.model_info_offset;
        ModelInfo::read(&mut r, lods)?;
        let start = r.pos;
        if r.u8()? != 1 {
            return Err("the model has no animations".into());
        }
        let animations = Animations::read(&mut r)?;
        if animations.classes.is_empty()
            || animations.classes.len() > 4096
            || animations.bones_to_anims.len() != lods
        {
            return Err("malformed animation block".into());
        }
        let sources: Vec<&[u8]> = animations
            .classes
            .iter()
            .map(|a| a.source.as_slice())
            .collect();
        let matched = |s: &[u8]| expected.iter().any(|e| crate::op_json::same(s, e));
        let found = |e: &str| sources.iter().any(|s| crate::op_json::same(s, e));
        if !sources.iter().all(|s| matched(s)) || !expected.iter().all(|e| found(e)) {
            let sources = listed(sources.iter().map(|s| crate::op_json::text(s)));
            let expected = listed(expected.iter().map(|s| s.to_string()));
            return Err(format!(
                "the model's animation sources are [{sources}], not [{expected}]"
            ));
        }
        let end = r.pos;
        let removed = end - start - 1;
        let mut ranges = Vec::new();
        let starts = r.u32s(lods)?;
        let ends = r.u32s(lods)?;
        for (&a, &b) in starts.iter().zip(&ends) {
            ranges.push((a as usize, b as usize));
        }
        ranges.sort_unstable();
        let mut previous = end.checked_add(9 * lods).ok_or("invalid LOD table")?;
        for (a, b) in ranges {
            if a < previous || b <= a || b > data.len() {
                return Err("invalid LOD address table".into());
            }
            previous = b;
        }
        let mut out = Vec::with_capacity(data.len() - removed);
        out.extend_from_slice(&data[..start]);
        out.push(0);
        out.extend_from_slice(&data[end..]);
        for i in 0..lods * 2 {
            let at = start + 1 + i * 4;
            let old = u32::from_le_bytes(
                out[at..at + 4]
                    .try_into()
                    .map_err(|_| "invalid LOD table")?,
            );
            let adjusted = old
                .checked_sub(u32::try_from(removed).map_err(|_| "model too large")?)
                .ok_or("invalid LOD address")?;
            out[at..at + 4].copy_from_slice(&adjusted.to_le_bytes());
        }
        if header.version <= 74 {
            let result = Model::parse(&out)?;
            if result.animations.is_some() {
                return Err("the stripped model still has animations".into());
            }
        }
        Ok(out)
    };
    run().map_err(|e| format!("{label}: {e}"))
}

fn listed(names: impl Iterator<Item = String>) -> String {
    let mut set: Vec<String> = Vec::new();
    for name in names {
        if !set.iter().any(|s| crate::op_json::same_text(s, &name)) {
            set.push(name);
        }
    }
    crate::culture_order::sorted(set).join(", ")
}
