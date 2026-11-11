use crate::odol::Model;
use serde_json::Value;

pub fn string<'a>(op: &'a Value, name: &str) -> Result<&'a str, String> {
    op.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("op is missing string \"{name}\": {op}"))
}

pub fn optional_string<'a>(op: &'a Value, name: &str) -> Result<Option<&'a str>, String> {
    op.get(name).map(|_| string(op, name)).transpose()
}

pub fn int(op: &Value, name: &str, fallback: i32) -> Result<i32, String> {
    op.get(name).map_or(Ok(fallback), |v| {
        v.as_i64()
            .and_then(|n| i32::try_from(n).ok())
            .ok_or_else(|| format!("op needs \"{name}\" as an integer: {op}"))
    })
}

fn finite(v: &Value) -> Option<f32> {
    v.as_f64().map(|x| x as f32).filter(|x| x.is_finite())
}

pub fn float(op: &Value, name: &str) -> Result<f32, String> {
    op.get(name)
        .and_then(finite)
        .ok_or_else(|| format!("op is missing a finite number \"{name}\": {op}"))
}

pub fn floats(op: &Value, name: &str, count: usize) -> Result<Vec<f32>, String> {
    op.get(name)
        .and_then(Value::as_array)
        .filter(|a| a.len() == count)
        .and_then(|a| a.iter().map(finite).collect::<Option<Vec<_>>>())
        .ok_or_else(|| format!("op needs \"{name}\" as an array of {count} finite numbers: {op}"))
}

pub fn strings<'a>(op: &'a Value, name: &str) -> Result<Vec<&'a str>, String> {
    op.get(name)
        .and_then(Value::as_array)
        .and_then(|a| a.iter().map(Value::as_str).collect::<Option<Vec<_>>>())
        .ok_or_else(|| format!("op is missing string array \"{name}\": {op}"))
}

pub fn latin1(s: &str) -> Result<Vec<u8>, String> {
    s.chars()
        .map(|c| u8::try_from(c as u32).map_err(|_| "character outside Latin-1".to_owned()))
        .collect()
}

fn ordinal_upper(c: char) -> char {
    let mut upper = c.to_uppercase();
    let first = upper.next().unwrap_or(c);
    if upper.next().is_some() || (first.is_ascii() && !c.is_ascii()) {
        c
    } else {
        first
    }
}

pub fn same(a: &[u8], b: &str) -> bool {
    a.iter()
        .copied()
        .map(char::from)
        .map(ordinal_upper)
        .eq(b.chars().map(ordinal_upper))
}

fn invariant_lower(c: char) -> char {
    let mut lower = c.to_lowercase();
    match (lower.next(), lower.next()) {
        (Some(first), None) => first,
        _ => c,
    }
}

pub fn lower_invariant(s: &str) -> String {
    s.chars().map(invariant_lower).collect()
}

pub fn same_lowered(model: &[u8], lowered: &str) -> bool {
    model
        .iter()
        .copied()
        .map(char::from)
        .map(invariant_lower)
        .eq(lowered.chars())
}

pub fn contains(model: &[u8], filter: &str) -> bool {
    let n = filter.chars().count();
    n == 0 || model.windows(n).any(|w| same(w, filter))
}

pub fn text(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

pub fn edit(
    data: &[u8],
    label: &str,
    rewrites_faces: bool,
    change: impl FnOnce(&mut Model) -> Result<(), String>,
) -> Result<Vec<u8>, String> {
    let run = || -> Result<Vec<u8>, String> {
        let mut model = Model::parse(data)?;
        if model.write()? != data {
            return Err("the ODOL library does not re-serialise this model byte-identically; refusing to patch it".into());
        }
        change(&mut model)?;
        let output = model.write()?;
        let reparsed = Model::parse(&output)?;
        if rewrites_faces {
            for lod in &reparsed.lods {
                crate::face_ops::ranges(lod)?;
            }
        }
        Ok(output)
    };
    run().map_err(|e| format!("{label}: {e}"))
}

#[test]
fn same_matches_dotnet_ordinal_ignore_case() {
    for (model, manifest, equal) in [
        (0xff, "\u{178}", true),
        (0xff, "\u{ff}", true),
        (0xb5, "\u{39c}", true),
        (0xb5, "\u{3bc}", true),
        (0xdf, "\u{1e9e}", false),
        (0xdf, "SS", false),
        (0xdf, "\u{df}", true),
        (b's', "\u{17f}", false),
        (b'i', "\u{131}", false),
        (b'k', "\u{212a}", false),
    ] {
        assert_eq!(same(&[model], manifest), equal, "{model:#x} {manifest}");
    }
    assert!(contains(b"\\p\xe9x", "ÉX"));
    assert!(contains(b"ab", ""));
    assert!(!contains(b"ab", "abc"));
}

#[test]
fn same_lowered_matches_dotnet_lower_invariant_then_exact() {
    assert_eq!(lower_invariant(r"\PROXY\Ä"), r"\proxy\ä");
    assert!(same_lowered(&[b'C', 0xC9], &lower_invariant("cé")));
    assert!(!same_lowered(&[0xB5], &lower_invariant("\u{03BC}")));
    assert!(same(&[0xB5], "\u{03BC}"));
    assert!(same_lowered(b"k", &lower_invariant("\u{212A}")));
}
