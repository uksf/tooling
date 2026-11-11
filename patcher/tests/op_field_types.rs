use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::{fs, path::PathBuf, process::Command};

const CABLE: &[u8] = include_bytes!("data/cable.p3d");

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

fn apply(case: &str, whole_parts: Option<Value>) -> (Option<i32>, String, bool, bool) {
    let root = std::env::temp_dir().join(format!("patcher-types-{}-{case}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("mod/addons")).unwrap();
    fs::create_dir_all(root.join("patches")).unwrap();
    let pristine = pbo(b"cable.p3d", CABLE);
    let target: PathBuf = root.join("mod/addons/x.pbo");
    fs::write(&target, &pristine).unwrap();
    let mut op = json!({"op":"remove-faces","lod":"1","axis":[0,0,0,0,0,1],"allBeyond":-100,"anyBeyond":-100,"faces":90});
    if let Some(value) = whole_parts {
        op["wholeParts"] = value;
    }
    let manifest = json!({"format":1,"pbo":"x.pbo","pristineSha256":hash(&pristine),"assets":[{"path":"cable.p3d","sha256":hash(CABLE),"ops":[op]}]});
    fs::write(root.join("patches/x.json"), manifest.to_string()).unwrap();
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
    let unchanged = fs::read(&target).unwrap() == pristine;
    let stateless = !root.join("state").exists();
    fs::remove_dir_all(&root).unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (output.status.code(), stderr, unchanged, stateless)
}

#[test]
fn whole_parts_of_the_wrong_type_is_rejected_without_writes() {
    for (case, value) in [
        ("string", json!("true")),
        ("number", json!(1)),
        ("null", Value::Null),
        ("array", json!([])),
        ("object", json!({})),
    ] {
        let (code, stderr, unchanged, stateless) = apply(case, Some(value));
        assert_eq!(code, Some(1), "{case}: {stderr}");
        assert!(
            stderr.contains("\"wholeParts\" as true or false"),
            "{case}: {stderr}"
        );
        assert!(unchanged && stateless, "{case}");
    }
    for (case, value) in [("omitted", None), ("false", Some(json!(false)))] {
        let (code, stderr, unchanged, _) = apply(case, value);
        assert_eq!(code, Some(0), "{case}: {stderr}");
        assert!(!unchanged, "{case}");
    }
}
