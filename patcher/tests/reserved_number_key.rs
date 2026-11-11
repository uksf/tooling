use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::{fs, process::Command};

const CABLE: &[u8] = include_bytes!("data/cable.p3d");
const WRAPPED: &str = r#"{"$serde_json::private::Number":"1"}"#;
const REMOVE_FACES: &str = r#"{"op":"remove-faces","lod":"1","axis":[0,0,0,0,0,AXIS],"allBeyond":ALL,"anyBeyond":-100,"faces":90}"#;

fn hash(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn pbo(name: &[u8], data: &[u8]) -> Vec<u8> {
    let mut result = name.to_vec();
    result.push(0);
    for word in [0, data.len() as u32, 0, 0, data.len() as u32] {
        result.extend_from_slice(&word.to_le_bytes());
    }
    result.extend_from_slice(&[0; 21]);
    result.extend_from_slice(data);
    let digest = Sha1::digest(&result);
    result.push(0);
    result.extend_from_slice(&digest);
    result
}

struct Outcome {
    ok: bool,
    stderr: String,
    unchanged: bool,
    stateless: bool,
}

fn apply(case: &str, asset: &str, data: &[u8], format: &str, op: &str) -> Outcome {
    let root = std::env::temp_dir().join(format!("patcher-reserved-{}-{case}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("mod/addons")).unwrap();
    fs::create_dir_all(root.join("patches")).unwrap();
    let pristine = pbo(asset.as_bytes(), data);
    let target = root.join("mod/addons/x.pbo");
    fs::write(&target, &pristine).unwrap();
    let manifest = format!(
        r#"{{"format":{format},"pbo":"x.pbo","pristineSha256":"{}","assets":[{{"path":"{asset}","sha256":"{}","ops":[{op}]}}]}}"#,
        hash(&pristine),
        hash(data)
    );
    fs::write(root.join("patches/x.json"), manifest).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_uksf-patcher"))
        .arg("apply")
        .arg("--addons")
        .arg(root.join("mod/addons"))
        .arg("--patches")
        .arg(root.join("patches"))
        .arg("--state")
        .arg(root.join("state"))
        .output()
        .unwrap();
    let outcome = Outcome {
        ok: output.status.success(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        unchanged: fs::read(&target).unwrap() == pristine,
        stateless: !root.join("state").exists(),
    };
    fs::remove_dir_all(&root).unwrap();
    outcome
}

fn text_replace(case: &str, format: &str, op: &str) -> Outcome {
    apply(case, "a.txt", b"abc", format, op)
}

fn remove_faces(case: &str, axis: &str, all: &str) -> Outcome {
    let op = REMOVE_FACES.replace("AXIS", axis).replace("ALL", all);
    apply(case, "cable.p3d", CABLE, "1", &op)
}

fn assert_rejected(case: &str, outcome: Outcome) {
    assert!(!outcome.ok, "{case}");
    assert!(
        outcome
            .stderr
            .contains("\"$serde_json::private::Number\" is not supported"),
        "{case}: {}",
        outcome.stderr
    );
    assert!(outcome.unchanged && outcome.stateless, "{case}");
}

fn assert_applied(case: &str, outcome: Outcome) {
    assert!(outcome.ok, "{case}: {}", outcome.stderr);
    assert!(!outcome.unchanged, "{case}");
}

#[test]
fn a_wrapped_number_object_is_rejected_at_every_numeric_field() {
    let count = format!(r#"{{"op":"text-replace","find":"a","replace":"z","count":{WRAPPED}}}"#);
    assert_rejected("count", text_replace("count", "1", &count));
    let plain = r#"{"op":"text-replace","find":"a","replace":"z"}"#;
    assert_rejected("format", text_replace("format", WRAPPED, plain));
    assert_rejected("all", remove_faces("all", "1", WRAPPED));
    assert_rejected("axis", remove_faces("axis", WRAPPED, "-100"));
}

#[test]
fn ordinary_numbers_and_the_reserved_key_text_in_strings_still_apply() {
    let count = r#"{"op":"text-replace","find":"a","replace":"z","count":1}"#;
    assert_applied("plain-count", text_replace("plain-count", "1", count));
    assert_applied("plain-faces", remove_faces("plain-faces", "1", "-100"));
    let quoted =
        r#"{"op":"text-replace","find":"a","replace":"{\"$serde_json::private::Number\":\"1\"}"}"#;
    assert_applied("quoted", text_replace("quoted", "1", quoted));
    let bare = r#"{"op":"text-replace","find":"a","replace":"$serde_json::private::Number"}"#;
    assert_applied("bare", text_replace("bare", "1", bare));
}
