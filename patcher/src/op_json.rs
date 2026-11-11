use crate::{latin1_best_fit::BEST_FIT, odol::Model};
use serde_json::Value;

const RESERVED_KEY: &str = "$serde_json::private::Number";

pub fn parse(json: &[u8]) -> Result<Value, String> {
    let value = serde_json::from_slice(json).map_err(|e| e.to_string())?;
    if keys::has_reserved_key(json) {
        return Err(format!(
            "the object key \"{RESERVED_KEY}\" is not supported"
        ));
    }
    Ok(value)
}

#[path = "op_json_keys.rs"]
mod keys;

pub fn string<'a>(op: &'a Value, name: &str) -> Result<&'a str, String> {
    op.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("op is missing string \"{name}\": {op}"))
}

pub fn optional_string<'a>(op: &'a Value, name: &str) -> Result<Option<&'a str>, String> {
    op.get(name).map(|_| string(op, name)).transpose()
}

pub fn int32(v: &Value) -> Option<i32> {
    v.as_i64().and_then(|n| i32::try_from(n).ok())
}

pub fn int(op: &Value, name: &str, fallback: i32) -> Result<i32, String> {
    op.get(name).map_or(Ok(fallback), |v| {
        int32(v).ok_or_else(|| format!("op needs \"{name}\" as an integer: {op}"))
    })
}

pub fn required_int(op: &Value, name: &str) -> Result<i32, String> {
    match op.get(name) {
        Some(v) if v.is_number() => {
            int32(v).ok_or_else(|| format!("op needs \"{name}\" as an integer: {op}"))
        }
        _ => Err(format!("op is missing int \"{name}\": {op}")),
    }
}

pub fn optional_bool(op: &Value, name: &str) -> Result<Option<bool>, String> {
    op.get(name)
        .map(|v| {
            v.as_bool()
                .ok_or_else(|| format!("op needs \"{name}\" as true or false: {op}"))
        })
        .transpose()
}

pub fn raw_single(v: &Value) -> Option<f32> {
    v.as_number()?.to_string().parse::<f32>().ok()
}

pub fn single(v: &Value) -> Option<f32> {
    raw_single(v).filter(|x| x.is_finite())
}

pub fn float(op: &Value, name: &str) -> Result<f32, String> {
    op.get(name)
        .and_then(single)
        .ok_or_else(|| format!("op is missing a finite number \"{name}\": {op}"))
}

pub fn floats(op: &Value, name: &str, count: usize) -> Result<Vec<f32>, String> {
    op.get(name)
        .and_then(Value::as_array)
        .filter(|a| a.len() == count)
        .and_then(|a| a.iter().map(single).collect::<Option<Vec<_>>>())
        .ok_or_else(|| format!("op needs \"{name}\" as an array of {count} finite numbers: {op}"))
}

pub fn strings<'a>(op: &'a Value, name: &str) -> Result<Vec<&'a str>, String> {
    op.get(name)
        .and_then(Value::as_array)
        .and_then(|a| a.iter().map(Value::as_str).collect::<Option<Vec<_>>>())
        .ok_or_else(|| format!("op is missing string array \"{name}\": {op}"))
}

pub fn nullable_strings<'a>(op: &'a Value, name: &str) -> Result<Vec<Option<&'a str>>, String> {
    op.get(name)
        .and_then(Value::as_array)
        .and_then(|a| {
            a.iter()
                .map(|v| {
                    if v.is_null() {
                        Some(None)
                    } else {
                        v.as_str().map(Some)
                    }
                })
                .collect::<Option<Vec<_>>>()
        })
        .ok_or_else(|| format!("op is missing string array \"{name}\": {op}"))
}

pub fn latin1_unit(unit: u16) -> u8 {
    u8::try_from(unit).unwrap_or_else(|_| {
        BEST_FIT
            .binary_search_by_key(&unit, |pair| pair.0)
            .map_or(b'?', |i| BEST_FIT[i].1)
    })
}

pub fn latin1_lossy(s: &str) -> Vec<u8> {
    s.encode_utf16().map(latin1_unit).collect()
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

pub fn same_text(a: &str, b: &str) -> bool {
    a.chars()
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
    edit_unlabelled(data, rewrites_faces, change).map_err(|e| format!("{label}: {e}"))
}

pub fn edit_unlabelled(
    data: &[u8],
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
    run()
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

#[test]
fn json_fields_follow_system_text_json_getters() {
    let op: Value =
        serde_json::from_str(r#"{"a":1.5,"b":1099511627776,"c":-0,"d":"7","e":2147483647,"f":1.0000000596046447753906250001,"g":1e39,"h":null}"#)
            .unwrap();
    assert!(int(&op, "a", 0).is_err());
    assert!(int(&op, "b", 0).is_err());
    assert_eq!(int(&op, "c", 9), Ok(0));
    assert!(int(&op, "d", 0).is_err());
    assert_eq!(int(&op, "e", 0), Ok(i32::MAX));
    assert_eq!(int(&op, "missing", -1), Ok(-1));
    assert_eq!(float(&op, "f").map(f32::to_bits), Ok(0x3f80_0001));
    assert!(float(&op, "g").is_err());
    assert!(optional_bool(&op, "h").is_err());
    assert_eq!(optional_bool(&op, "missing"), Ok(None));
}

#[test]
fn parse_rejects_only_the_reserved_number_key() {
    let wrapped = br#"{"copies":[{"$serde_json::private::Number":"1"}]}"#;
    let err = parse(wrapped).unwrap_err();
    assert!(err.contains("is not supported"), "{err}");
    let spaced = br#"{"a":[{ "\u0024serde_json::private::Number" : "1"}]}"#;
    assert!(parse(spaced).is_err());
    let op = parse(br#"{"f":1.0000000596046447753906250001,"c":-0,"d":-0.0,"s":"\"$serde_json::private::Number\":"}"#).unwrap();
    assert_eq!(float(&op, "f").map(f32::to_bits), Ok(0x3f80_0001));
    assert_eq!(int(&op, "c", 9), Ok(0));
    assert!(int(&op, "d", 9).is_err());
    assert_eq!(float(&op, "d").map(f32::to_bits), Ok(0x8000_0000));
    assert_eq!(string(&op, "s"), Ok(r#""$serde_json::private::Number":"#));
}
